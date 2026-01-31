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
use crate::model_library::hf_cache::HfSearchCache;
use crate::model_library::naming::extract_base_name;
use crate::model_library::types::{
    DownloadRequest, DownloadStatus, HfMetadataResult, HfSearchParams, HuggingFaceModel,
    LfsFileInfo, ModelDownloadProgress, RepoFileTree,
};
use crate::models::DownloadOption;
use crate::network::{CacheStrategy, WebSource, WebSourceId};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// HuggingFace API base URL.
const HF_API_BASE: &str = "https://huggingface.co/api";

/// HuggingFace Hub download base URL.
const HF_HUB_BASE: &str = "https://huggingface.co";

/// Cache TTL for repository file trees (24 hours).
const REPO_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// Client for HuggingFace Hub API operations.
pub struct HuggingFaceClient {
    /// HTTP client
    client: Client,
    /// Cache directory for LFS file info (legacy JSON cache)
    cache_dir: PathBuf,
    /// Active downloads
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    /// SQLite search cache (optional)
    search_cache: Option<Arc<HfSearchCache>>,
}

impl std::fmt::Debug for HuggingFaceClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HuggingFaceClient")
            .field("cache_dir", &self.cache_dir)
            .field("has_search_cache", &self.search_cache.is_some())
            .finish()
    }
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
            search_cache: None,
        })
    }

    /// Create a new HuggingFace client with SQLite search cache.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for caching API responses (legacy JSON)
    /// * `search_cache` - SQLite search cache for intelligent caching
    pub fn with_cache(cache_dir: impl Into<PathBuf>, search_cache: Arc<HfSearchCache>) -> Result<Self> {
        let mut client = Self::new(cache_dir)?;
        client.search_cache = Some(search_cache);
        Ok(client)
    }

    /// Set the search cache after construction.
    pub fn set_search_cache(&mut self, cache: Arc<HfSearchCache>) {
        self.search_cache = Some(cache);
    }

    /// Get a reference to the search cache if available.
    pub fn search_cache(&self) -> Option<&Arc<HfSearchCache>> {
        self.search_cache.as_ref()
    }

    // ========================================
    // Search Operations
    // ========================================

    /// Search for models on HuggingFace with automatic caching.
    ///
    /// This method transparently handles caching:
    /// - Checks SQLite cache for recent search results
    /// - Falls back to HuggingFace API if cache miss or stale
    /// - Enriches results with download options (file sizes)
    /// - Caches results for future queries
    ///
    /// # Arguments
    ///
    /// * `params` - Search parameters
    pub async fn search(&self, params: &HfSearchParams) -> Result<Vec<HuggingFaceModel>> {
        // If we have a cache, use it transparently
        let cache = match &self.search_cache {
            Some(c) => c,
            None => {
                // No cache configured, use direct API
                return self.search_api(params).await;
            }
        };

        let limit = params.limit.unwrap_or(20);
        let offset = params.offset.unwrap_or(0);
        let kind = params.kind.as_deref();

        // Check cache for existing search results
        match cache.get_search_results(&params.query, kind, limit, offset) {
            Ok(Some(models)) => {
                info!(
                    "Cache hit for search '{}': {} models",
                    params.query,
                    models.len()
                );
                return Ok(models);
            }
            Ok(None) => {
                debug!("Cache miss for search '{}'", params.query);
            }
            Err(e) => {
                warn!("Cache error, falling back to API: {}", e);
            }
        }

        // Cache miss - perform API search
        let models = self.search_api(params).await?;

        // Enrich models with download options from cache or API
        let enriched = self.enrich_models_with_download_options(&models).await;

        // Cache the search results
        let repo_ids: Vec<String> = enriched.iter().map(|m| m.repo_id.clone()).collect();
        if let Err(e) = cache.cache_search_results(&params.query, kind, limit, offset, &repo_ids) {
            warn!("Failed to cache search results: {}", e);
        }

        // Cache individual model details
        for model in &enriched {
            if let Err(e) = cache.cache_repo_details(model) {
                warn!("Failed to cache repo details for {}: {}", model.repo_id, e);
            }
        }

        Ok(enriched)
    }

    /// Direct API search without caching (internal use).
    ///
    /// # Arguments
    ///
    /// * `params` - Search parameters
    async fn search_api(&self, params: &HfSearchParams) -> Result<Vec<HuggingFaceModel>> {
        let limit = params.limit.unwrap_or(20);
        let offset = params.offset.unwrap_or(0);

        // Build search URL
        // Note: full=true is required to get lastModified field in response
        let mut url = format!(
            "{}/models?search={}&limit={}&offset={}&full=true",
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

    /// Enrich models with download options (file sizes) from cache or API.
    async fn enrich_models_with_download_options(
        &self,
        models: &[HuggingFaceModel],
    ) -> Vec<HuggingFaceModel> {
        let mut enriched = Vec::with_capacity(models.len());

        for model in models {
            let mut model = model.clone();

            // Try to get download options from cache first
            if let Some(cache) = &self.search_cache {
                // Check if we need to refresh based on lastModified
                let needs_refresh = cache
                    .needs_refresh(&model.repo_id, model.release_date.as_deref())
                    .unwrap_or(true);

                if !needs_refresh {
                    // Use cached details
                    if let Ok(Some(cached)) = cache.get_repo_details(&model.repo_id) {
                        if !cached.download_options.is_empty() {
                            model.download_options = cached.download_options;
                            model.total_size_bytes = cached.total_size_bytes;
                            enriched.push(model);
                            continue;
                        }
                    }
                }
            }

            // Fetch from API
            match self.get_repo_files(&model.repo_id).await {
                Ok(tree) => {
                    let download_options = Self::extract_download_options_from_tree(&tree, &model.quants);
                    let total_size = tree.lfs_files.iter().map(|f| f.size).sum();

                    model.download_options = download_options;
                    model.total_size_bytes = Some(total_size);
                }
                Err(e) => {
                    debug!(
                        "Failed to fetch repo files for {}: {}",
                        model.repo_id, e
                    );
                    // Keep model without download options
                }
            }

            enriched.push(model);
        }

        enriched
    }

    /// Extract download options from repo file tree.
    fn extract_download_options_from_tree(
        tree: &RepoFileTree,
        quants: &[String],
    ) -> Vec<DownloadOption> {
        let mut options = Vec::new();

        // Build regex for quant pattern matching
        let quant_pattern = regex::Regex::new(r"[._-](Q\d+_[A-Z0-9_]+|fp16|fp32|bf16|int8|int4)[._-]?")
            .ok();

        for lfs_file in &tree.lfs_files {
            // Only include model files
            if !lfs_file.filename.ends_with(".gguf")
                && !lfs_file.filename.ends_with(".safetensors")
                && !lfs_file.filename.ends_with(".bin")
            {
                continue;
            }

            // Try to extract quant from filename
            let quant = if let Some(ref pattern) = quant_pattern {
                pattern
                    .captures(&lfs_file.filename)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string())
            } else {
                None
            };

            // If we found a quant, or the file matches a known quant
            if let Some(q) = quant {
                options.push(DownloadOption {
                    quant: q,
                    size_bytes: Some(lfs_file.size),
                });
            } else if quants.iter().any(|q| lfs_file.filename.contains(q)) {
                // Fallback: check if filename contains any of the known quants
                for q in quants {
                    if lfs_file.filename.contains(q) {
                        options.push(DownloadOption {
                            quant: q.clone(),
                            size_bytes: Some(lfs_file.size),
                        });
                        break;
                    }
                }
            } else if quants.is_empty() {
                // No quants specified, include file by name
                let name = lfs_file
                    .filename
                    .rsplit('/')
                    .next()
                    .unwrap_or(&lfs_file.filename);
                options.push(DownloadOption {
                    quant: name.to_string(),
                    size_bytes: Some(lfs_file.size),
                });
            }
        }

        // Sort by quant name for consistent ordering
        options.sort_by(|a, b| a.quant.cmp(&b.quant));
        options.dedup_by(|a, b| a.quant == b.quant);

        options
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
        let developer = result
            .model_id
            .split('/')
            .next()
            .unwrap_or("")
            .to_string();

        // Determine kind from pipeline_tag (default to "unknown")
        let kind = result.pipeline_tag.unwrap_or_else(|| "unknown".to_string());

        // Extract formats and quants from tags
        let formats: Vec<String> = result
            .tags
            .iter()
            .filter(|t| ["gguf", "safetensors", "pytorch", "onnx"].contains(&t.as_str()))
            .cloned()
            .collect();

        // Extract quants from tags first
        let mut quants: Vec<String> = result
            .tags
            .iter()
            .filter(|t| {
                t.starts_with("Q") && t.contains("_")
                    || ["fp16", "fp32", "bf16", "int8", "int4"].contains(&t.as_str())
            })
            .cloned()
            .collect();

        // If no quants from tags, extract from sibling filenames (GGUF models)
        if quants.is_empty() {
            quants = Self::extract_quants_from_filenames(&result.siblings);
        }

        // Build URL for the model page
        let url = format!("https://huggingface.co/{}", result.model_id);

        // Detect compatible inference engines based on formats
        let compatible_engines = crate::models::detect_compatible_engines(&formats);

        HuggingFaceModel {
            repo_id: result.model_id,
            name,
            developer,
            kind,
            formats,
            quants,
            download_options: vec![], // Populated by get_download_options
            url,
            release_date: result.last_modified,
            downloads: result.downloads,
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines,
        }
    }

    /// Extract quantization names from sibling filenames.
    ///
    /// Looks for patterns like Q4_K_M, Q8_0, etc. in GGUF/model filenames.
    fn extract_quants_from_filenames(siblings: &[HfSibling]) -> Vec<String> {
        use std::collections::HashSet;

        let quant_pattern = regex::Regex::new(r"[._-](Q\d+_[A-Z0-9_]+|fp16|fp32|bf16|int8|int4)[._-]?")
            .unwrap_or_else(|_| regex::Regex::new(r"$^").unwrap()); // fallback to never-match

        let mut quants: HashSet<String> = HashSet::new();

        for sibling in siblings {
            let filename = &sibling.rfilename;
            // Only check model files (gguf, safetensors, etc.)
            if filename.ends_with(".gguf")
                || filename.ends_with(".safetensors")
                || filename.ends_with(".bin")
            {
                for cap in quant_pattern.captures_iter(filename) {
                    if let Some(m) = cap.get(1) {
                        quants.insert(m.as_str().to_string());
                    }
                }
            }
        }

        let mut sorted: Vec<String> = quants.into_iter().collect();
        sorted.sort();
        sorted
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
            last_modified: None, // Would need separate API call to get this
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
            model_type: Some(best_match.kind.clone()),
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

// === WebSource trait implementations ===

impl WebSourceId for HuggingFaceClient {
    fn id(&self) -> &'static str {
        "huggingface"
    }

    fn domains(&self) -> &[&'static str] {
        &["huggingface.co"]
    }
}

impl CacheStrategy for HuggingFaceClient {
    fn default_ttl(&self) -> Duration {
        Duration::from_secs(REPO_CACHE_TTL_SECS)
    }

    fn allow_stale_on_offline(&self) -> bool {
        true
    }

    fn max_stale_age(&self) -> Option<Duration> {
        // Allow stale data up to 7 days old for HuggingFace
        Some(Duration::from_secs(7 * 24 * 60 * 60))
    }
}

#[async_trait]
impl WebSource for HuggingFaceClient {
    fn has_cache(&self, key: &str) -> bool {
        // Check if we have cached file tree for this repo
        let cache_path = self.get_cache_path(key, "files");
        cache_path.exists()
    }

    fn is_cache_fresh(&self, key: &str) -> bool {
        let cache_path = self.get_cache_path(key, "files");
        if !cache_path.exists() {
            return false;
        }

        // Check if cache is within TTL
        std::fs::metadata(&cache_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .map(|elapsed| elapsed.as_secs() < REPO_CACHE_TTL_SECS)
            .unwrap_or(false)
    }

    async fn on_network_restored(&self) {
        debug!("HuggingFace source: network restored");
        // Could trigger cache refresh here if needed
    }

    fn on_circuit_open(&self, domain: &str) {
        warn!("HuggingFace source: circuit breaker opened for {}", domain);
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
    /// Note: HuggingFace API returns this as snake_case "pipeline_tag"
    #[serde(default, rename = "pipeline_tag")]
    pipeline_tag: Option<String>,
    /// Requires full=true in API request to be populated
    #[serde(default)]
    last_modified: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    /// File list from repo (available with full=true)
    #[serde(default)]
    siblings: Vec<HfSibling>,
}

/// HuggingFace sibling file entry from search API.
#[derive(Debug, Deserialize)]
struct HfSibling {
    /// Relative filename in the repo
    rfilename: String,
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
            siblings: vec![],
        };

        let model = client.convert_search_result(mock_result);

        assert_eq!(model.repo_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(model.name, "Llama-2-7B-GGUF");
        assert_eq!(model.developer, "TheBloke");
        assert!(model.formats.contains(&"gguf".to_string()));
        assert!(model.quants.contains(&"Q4_K_M".to_string()));
    }
}
