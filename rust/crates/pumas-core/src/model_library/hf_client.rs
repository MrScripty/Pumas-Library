//! HuggingFace client for model search, download, and metadata lookup.
//!
//! Provides integration with the HuggingFace Hub API:
//! - Model search with filters
//! - Repository metadata and file listing
//! - Download with progress tracking
//! - Metadata lookup by filename/hash

use crate::error::{PumasError, Result};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::hashing::compute_fast_hash;
use crate::model_library::naming::extract_base_name;
use crate::model_library::types::{
    DownloadRequest, DownloadStatus, HfMetadataResult, HfSearchParams, HuggingFaceModel,
    LfsFileInfo, ModelDownloadProgress, RepoFileTree,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

/// HuggingFace API base URL.
const HF_API_BASE: &str = "https://huggingface.co/api";

/// HuggingFace Hub download base URL.
const HF_HUB_BASE: &str = "https://huggingface.co";

/// Cache TTL for repository file trees (24 hours).
const REPO_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// Client for HuggingFace Hub API operations.
#[derive(Debug)]
pub struct HuggingFaceClient {
    /// HTTP client
    client: Client,
    /// Cache directory for LFS file info
    cache_dir: PathBuf,
    /// Active downloads
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
}

/// Internal state for an active download.
struct DownloadState {
    /// Download ID
    download_id: String,
    /// Repository ID
    repo_id: String,
    /// Current status
    status: DownloadStatus,
    /// Progress (0.0-1.0)
    progress: f32,
    /// Downloaded bytes
    downloaded_bytes: u64,
    /// Total bytes
    total_bytes: Option<u64>,
    /// Download speed (bytes/sec)
    speed: f64,
    /// Cancellation flag
    cancel_flag: Arc<AtomicBool>,
    /// Error message if failed
    error: Option<String>,
}

impl std::fmt::Debug for DownloadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadState")
            .field("download_id", &self.download_id)
            .field("repo_id", &self.repo_id)
            .field("status", &self.status)
            .field("progress", &self.progress)
            .finish()
    }
}

impl HuggingFaceClient {
    /// Create a new HuggingFace client.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for caching API responses
    pub fn new(cache_dir: impl Into<PathBuf>) -> Result<Self> {
        let cache_dir = cache_dir.into();
        std::fs::create_dir_all(&cache_dir)?;

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("pumas-library/1.0")
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: None,
            })?;

        Ok(Self {
            client,
            cache_dir,
            downloads: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    // ========================================
    // Search Operations
    // ========================================

    /// Search for models on HuggingFace.
    ///
    /// # Arguments
    ///
    /// * `params` - Search parameters
    pub async fn search(&self, params: &HfSearchParams) -> Result<Vec<HuggingFaceModel>> {
        let limit = params.limit.unwrap_or(20);
        let offset = params.offset.unwrap_or(0);

        // Build search URL
        let mut url = format!(
            "{}/models?search={}&limit={}&offset={}",
            HF_API_BASE,
            urlencoding::encode(&params.query),
            limit,
            offset
        );

        // Add kind filter
        if let Some(ref kind) = params.kind {
            let pipeline_tag = match kind.as_str() {
                "text-generation" | "llm" => "text-generation",
                "text-to-image" | "diffusion" => "text-to-image",
                "image-to-image" => "image-to-image",
                "automatic-speech-recognition" | "audio" => "automatic-speech-recognition",
                _ => kind,
            };
            url.push_str(&format!("&pipeline_tag={}", pipeline_tag));
        }

        // Execute request
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
                message: format!("HuggingFace API request failed: {}", e),
                cause: Some(e.to_string()),
            })?;

        if !response.status().is_success() {
            return Err(PumasError::Network {
                message: format!("HuggingFace API returned {}", response.status()),
                cause: None,
            });
        }

        let results: Vec<HfSearchResult> = response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse HuggingFace response: {}", e),
            source: None,
        })?;

        // Convert to our model type
        let models: Vec<HuggingFaceModel> = results
            .into_iter()
            .map(|r| self.convert_search_result(r))
            .collect();

        Ok(models)
    }

    /// Convert HF search result to our model type.
    fn convert_search_result(&self, result: HfSearchResult) -> HuggingFaceModel {
        // Extract name from modelId (after the /)
        let name = result
            .model_id
            .split('/')
            .last()
            .unwrap_or(&result.model_id)
            .to_string();

        // Extract developer from modelId (before the /)
        let developer = result.model_id.split('/').next().map(String::from);

        // Determine kind from pipeline_tag
        let kind = result.pipeline_tag.clone();

        // Extract formats and quants from tags
        let formats: Vec<String> = result
            .tags
            .iter()
            .filter(|t| ["gguf", "safetensors", "pytorch", "onnx"].contains(&t.as_str()))
            .cloned()
            .collect();

        let quants: Vec<String> = result
            .tags
            .iter()
            .filter(|t| {
                t.starts_with("Q") && t.contains("_")
                    || ["fp16", "fp32", "bf16", "int8", "int4"].contains(&t.as_str())
            })
            .cloned()
            .collect();

        HuggingFaceModel {
            repo_id: result.model_id,
            name,
            developer,
            kind,
            formats: if formats.is_empty() {
                None
            } else {
                Some(formats)
            },
            quants: if quants.is_empty() {
                None
            } else {
                Some(quants)
            },
            download_options: None, // Populated by get_download_options
            url: None,
            release_date: result.last_modified,
            downloads: result.downloads,
            total_size_bytes: None,
            quant_sizes: None,
        }
    }

    // ========================================
    // Repository Information
    // ========================================

    /// Get repository file tree with LFS information.
    ///
    /// Results are cached for 24 hours.
    pub async fn get_repo_files(&self, repo_id: &str) -> Result<RepoFileTree> {
        // Check cache first
        let cache_file = self.get_cache_path(repo_id, "files");
        if let Some(cached) = self.read_cache::<RepoFileTree>(&cache_file)? {
            // Check if cache is still valid
            if let Ok(meta) = std::fs::metadata(&cache_file) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        if elapsed.as_secs() < REPO_CACHE_TTL_SECS {
                            return Ok(cached);
                        }
                    }
                }
            }
        }

        // Fetch from API
        let url = format!("{}/api/models/{}/tree/main", HF_HUB_BASE, repo_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
                message: format!("Failed to fetch repo tree: {}", e),
                cause: None,
            })?;

        if !response.status().is_success() {
            return Err(PumasError::Network {
                message: format!("HuggingFace API returned {}", response.status()),
                cause: None,
            });
        }

        let files: Vec<HfFileEntry> = response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse file tree: {}", e),
            source: None,
        })?;

        // Separate LFS and regular files
        let mut lfs_files = Vec::new();
        let mut regular_files = Vec::new();

        for file in files {
            if let Some(lfs) = file.lfs {
                lfs_files.push(LfsFileInfo {
                    filename: file.path,
                    size: lfs.size,
                    sha256: lfs.oid,
                });
            } else {
                regular_files.push(file.path);
            }
        }

        let tree = RepoFileTree {
            repo_id: repo_id.to_string(),
            lfs_files,
            regular_files,
            cached_at: chrono::Utc::now().to_rfc3339(),
        };

        // Cache the result
        self.write_cache(&cache_file, &tree)?;

        Ok(tree)
    }

    // ========================================
    // Metadata Lookup
    // ========================================

    /// Look up model metadata by filename and optional file path.
    ///
    /// Uses a hybrid approach:
    /// 1. Search by base filename
    /// 2. Compute fast hash for top candidates
    /// 3. Verify with LFS SHA256 if available
    ///
    /// # Arguments
    ///
    /// * `filename` - Model filename
    /// * `file_path` - Optional local file path for hash verification
    /// * `timeout` - Request timeout
    pub async fn lookup_metadata(
        &self,
        filename: &str,
        file_path: Option<&Path>,
        _timeout: Option<Duration>,
    ) -> Result<Option<HfMetadataResult>> {
        let base_name = extract_base_name(filename);

        // Search for candidates
        let params = HfSearchParams {
            query: base_name.clone(),
            limit: Some(5),
            ..Default::default()
        };

        let candidates = self.search(&params).await?;

        if candidates.is_empty() {
            return Ok(None);
        }

        // If we have a local file, try to verify by hash
        if let Some(path) = file_path {
            // Compute fast hash for filtering
            let fast_hash = compute_fast_hash(path).ok();

            // Try to match against top candidates
            for candidate in candidates.iter().take(2) {
                if let Ok(result) = self
                    .verify_candidate(&candidate.repo_id, filename, path, fast_hash.as_deref())
                    .await
                {
                    if let Some(result) = result {
                        return Ok(Some(result));
                    }
                }
            }
        }

        // Fall back to best filename match
        let best_match = &candidates[0];
        let confidence = self.compute_filename_confidence(&base_name, &best_match.name);

        Ok(Some(HfMetadataResult {
            repo_id: best_match.repo_id.clone(),
            official_name: Some(best_match.name.clone()),
            family: None, // Would need more analysis
            model_type: best_match.kind.clone(),
            subtype: None,
            variant: None,
            precision: None,
            tags: vec![],
            base_model: None,
            download_url: Some(format!(
                "{}/{}/resolve/main/{}",
                HF_HUB_BASE, best_match.repo_id, filename
            )),
            description: None,
            match_confidence: confidence,
            match_method: if confidence > 0.9 {
                "filename_exact"
            } else {
                "filename_fuzzy"
            }
            .to_string(),
            requires_confirmation: confidence < 0.6,
            hash_mismatch: false,
            matched_filename: Some(filename.to_string()),
            pending_full_verification: true,
            fast_hash: None,
            expected_sha256: None,
        }))
    }

    /// Verify a candidate repository against a local file.
    async fn verify_candidate(
        &self,
        repo_id: &str,
        filename: &str,
        _file_path: &Path,
        fast_hash: Option<&str>,
    ) -> Result<Option<HfMetadataResult>> {
        // Get repo files to find LFS hash
        let tree = self.get_repo_files(repo_id).await?;

        // Find matching file
        let matching_file = tree.lfs_files.iter().find(|f| {
            f.filename == filename
                || f.filename.ends_with(filename)
                || filename.ends_with(&f.filename)
        });

        if let Some(lfs_file) = matching_file {
            // We have an LFS file with SHA256
            // For now, just return it as a potential match
            // Full verification would require reading the entire file

            return Ok(Some(HfMetadataResult {
                repo_id: repo_id.to_string(),
                official_name: None,
                family: None,
                model_type: None,
                subtype: None,
                variant: None,
                precision: None,
                tags: vec![],
                base_model: None,
                download_url: Some(format!(
                    "{}/{}/resolve/main/{}",
                    HF_HUB_BASE, repo_id, lfs_file.filename
                )),
                description: None,
                match_confidence: 0.8, // High confidence from LFS match
                match_method: "lfs_match".to_string(),
                requires_confirmation: false,
                hash_mismatch: false,
                matched_filename: Some(lfs_file.filename.clone()),
                pending_full_verification: true,
                fast_hash: fast_hash.map(String::from),
                expected_sha256: Some(lfs_file.sha256.clone()),
            }));
        }

        Ok(None)
    }

    /// Compute filename match confidence.
    fn compute_filename_confidence(&self, query: &str, candidate: &str) -> f64 {
        let query_lower = query.to_lowercase();
        let candidate_lower = candidate.to_lowercase();

        if query_lower == candidate_lower {
            return 1.0;
        }

        if candidate_lower.contains(&query_lower) || query_lower.contains(&candidate_lower) {
            return 0.8;
        }

        // Simple word overlap score
        let query_words: std::collections::HashSet<_> =
            query_lower.split(|c: char| !c.is_alphanumeric()).collect();
        let candidate_words: std::collections::HashSet<_> = candidate_lower
            .split(|c: char| !c.is_alphanumeric())
            .collect();

        let intersection = query_words.intersection(&candidate_words).count();
        let union = query_words.union(&candidate_words).count();

        if union > 0 {
            intersection as f64 / union as f64
        } else {
            0.0
        }
    }

    // ========================================
    // Download Operations
    // ========================================

    /// Start a model download.
    ///
    /// Returns a download ID for tracking progress.
    pub async fn start_download(
        &self,
        request: &DownloadRequest,
        dest_dir: &Path,
    ) -> Result<String> {
        let download_id = uuid::Uuid::new_v4().to_string();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Get file info
        let tree = self.get_repo_files(&request.repo_id).await?;

        // Find the file to download
        let filename = if let Some(ref f) = request.filename {
            f.clone()
        } else if let Some(ref quant) = request.quant {
            // Find file matching quantization
            tree.lfs_files
                .iter()
                .find(|f| f.filename.contains(quant))
                .map(|f| f.filename.clone())
                .ok_or_else(|| PumasError::ModelNotFound {
                    model_id: format!("{}:{}", request.repo_id, quant),
                })?
        } else {
            // Get largest model file
            tree.lfs_files
                .iter()
                .max_by_key(|f| f.size)
                .map(|f| f.filename.clone())
                .ok_or_else(|| PumasError::ModelNotFound {
                    model_id: request.repo_id.clone(),
                })?
        };

        let total_bytes = tree
            .lfs_files
            .iter()
            .find(|f| f.filename == filename)
            .map(|f| f.size);

        // Create download state
        let state = DownloadState {
            download_id: download_id.clone(),
            repo_id: request.repo_id.clone(),
            status: DownloadStatus::Queued,
            progress: 0.0,
            downloaded_bytes: 0,
            total_bytes,
            speed: 0.0,
            cancel_flag: cancel_flag.clone(),
            error: None,
        };

        self.downloads
            .write()
            .await
            .insert(download_id.clone(), state);

        // Spawn download task
        let client = self.client.clone();
        let downloads = self.downloads.clone();
        let download_id_clone = download_id.clone();
        let repo_id = request.repo_id.clone();
        let dest_dir = dest_dir.to_path_buf();

        tokio::spawn(async move {
            let result = Self::run_download(
                client,
                downloads.clone(),
                &download_id_clone,
                &repo_id,
                &filename,
                &dest_dir,
                cancel_flag,
            )
            .await;

            if let Err(e) = result {
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(&download_id_clone) {
                    state.status = DownloadStatus::Error;
                    state.error = Some(e.to_string());
                }
            }
        });

        Ok(download_id)
    }

    /// Run the download in the background.
    async fn run_download(
        client: Client,
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        download_id: &str,
        repo_id: &str,
        filename: &str,
        dest_dir: &Path,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<()> {
        // Update status to downloading
        {
            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.status = DownloadStatus::Downloading;
            }
        }

        let url = format!("{}/{}/resolve/main/{}", HF_HUB_BASE, repo_id, filename);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| PumasError::DownloadFailed {
                url: url.clone(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(PumasError::DownloadFailed {
                url,
                message: format!("HTTP {}", response.status()),
            });
        }

        let total_bytes = response.content_length();

        // Create destination file
        std::fs::create_dir_all(dest_dir)?;
        let dest_path = dest_dir.join(filename);
        let mut file = tokio::fs::File::create(&dest_path).await?;

        // Download with progress tracking
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            // Check cancellation
            if cancel_flag.load(Ordering::Relaxed) {
                // Clean up partial file
                drop(file);
                let _ = tokio::fs::remove_file(&dest_path).await;

                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Cancelled;
                }

                return Err(PumasError::DownloadCancelled);
            }

            let chunk = chunk.map_err(|e| PumasError::DownloadFailed {
                url: url.clone(),
                message: e.to_string(),
            })?;

            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Update progress
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                downloaded as f64 / elapsed
            } else {
                0.0
            };

            let progress = if let Some(total) = total_bytes {
                downloaded as f32 / total as f32
            } else {
                0.0
            };

            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.downloaded_bytes = downloaded;
                state.total_bytes = total_bytes;
                state.progress = progress;
                state.speed = speed;
            }
        }

        file.flush().await?;

        // Update status to completed
        {
            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.status = DownloadStatus::Completed;
                state.progress = 1.0;
            }
        }

        Ok(())
    }

    /// Get download progress.
    pub async fn get_download_progress(&self, download_id: &str) -> Option<ModelDownloadProgress> {
        let downloads = self.downloads.read().await;
        downloads.get(download_id).map(|state| ModelDownloadProgress {
            download_id: state.download_id.clone(),
            repo_id: Some(state.repo_id.clone()),
            status: state.status,
            progress: Some(state.progress),
            downloaded_bytes: Some(state.downloaded_bytes),
            total_bytes: state.total_bytes,
            speed: Some(state.speed),
            eta_seconds: if state.speed > 0.0 && state.total_bytes.is_some() {
                let remaining = state.total_bytes.unwrap() - state.downloaded_bytes;
                Some(remaining as f64 / state.speed)
            } else {
                None
            },
            error: state.error.clone(),
        })
    }

    /// Cancel a download.
    pub async fn cancel_download(&self, download_id: &str) -> Result<bool> {
        let downloads = self.downloads.read().await;
        if let Some(state) = downloads.get(download_id) {
            state.cancel_flag.store(true, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List active downloads.
    pub async fn list_downloads(&self) -> Vec<ModelDownloadProgress> {
        let downloads = self.downloads.read().await;
        downloads
            .values()
            .map(|state| ModelDownloadProgress {
                download_id: state.download_id.clone(),
                repo_id: Some(state.repo_id.clone()),
                status: state.status,
                progress: Some(state.progress),
                downloaded_bytes: Some(state.downloaded_bytes),
                total_bytes: state.total_bytes,
                speed: Some(state.speed),
                eta_seconds: None,
                error: state.error.clone(),
            })
            .collect()
    }

    // ========================================
    // Cache Helpers
    // ========================================

    fn get_cache_path(&self, repo_id: &str, suffix: &str) -> PathBuf {
        let safe_name = repo_id.replace('/', "_");
        self.cache_dir.join(format!("hf_{}_{}.json", safe_name, suffix))
    }

    fn read_cache<T: for<'de> Deserialize<'de>>(&self, path: &Path) -> Result<Option<T>> {
        atomic_read_json(path)
    }

    fn write_cache<T: Serialize>(&self, path: &Path, data: &T) -> Result<()> {
        atomic_write_json(path, data, false)
    }
}

/// HuggingFace search result from API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HfSearchResult {
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
}

/// HuggingFace file entry from tree API.
#[derive(Debug, Deserialize)]
struct HfFileEntry {
    path: String,
    #[serde(default)]
    lfs: Option<HfLfsInfo>,
}

/// LFS information from HuggingFace.
#[derive(Debug, Deserialize)]
struct HfLfsInfo {
    oid: String,
    size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, HuggingFaceClient) {
        let temp_dir = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(temp_dir.path()).unwrap();
        (temp_dir, client)
    }

    #[test]
    fn test_filename_confidence() {
        let (_temp, client) = setup();

        // Exact match
        assert_eq!(client.compute_filename_confidence("llama", "llama"), 1.0);

        // Substring match
        let confidence = client.compute_filename_confidence("llama", "llama-2-7b");
        assert!(confidence > 0.7);

        // Partial word match
        let confidence = client.compute_filename_confidence("llama-7b", "llama-2-7b-chat");
        assert!(confidence > 0.3); // Lower threshold for partial matches

        // No match
        let confidence = client.compute_filename_confidence("gpt", "llama");
        assert!(confidence < 0.3);
    }

    #[test]
    fn test_cache_path() {
        let (_temp, client) = setup();

        let path = client.get_cache_path("TheBloke/Llama-2-7B-GGUF", "files");
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("TheBloke_Llama-2-7B-GGUF"));
    }

    #[tokio::test]
    async fn test_search_converts_results() {
        // This test verifies the conversion logic without making actual API calls
        let (_temp, client) = setup();

        let mock_result = HfSearchResult {
            model_id: "TheBloke/Llama-2-7B-GGUF".to_string(),
            tags: vec![
                "gguf".to_string(),
                "Q4_K_M".to_string(),
                "llama".to_string(),
            ],
            pipeline_tag: Some("text-generation".to_string()),
            last_modified: Some("2024-01-01".to_string()),
            downloads: Some(10000),
        };

        let model = client.convert_search_result(mock_result);

        assert_eq!(model.repo_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(model.name, "Llama-2-7B-GGUF");
        assert_eq!(model.developer, Some("TheBloke".to_string()));
        assert!(model.formats.as_ref().unwrap().contains(&"gguf".to_string()));
        assert!(model.quants.as_ref().unwrap().contains(&"Q4_K_M".to_string()));
    }
}
