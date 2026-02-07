//! HuggingFace client for model search, download, and metadata lookup.
//!
//! Provides integration with the HuggingFace Hub API:
//! - Model search with filters
//! - Repository metadata and file listing
//! - Download with progress tracking
//! - Metadata lookup by filename/hash

use crate::error::{PumasError, Result};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::download_store::{DownloadPersistence, PersistedDownload};
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
use tracing::{debug, error, info, warn};

/// HuggingFace API base URL.
const HF_API_BASE: &str = "https://huggingface.co/api";

/// HuggingFace Hub download base URL.
const HF_HUB_BASE: &str = "https://huggingface.co";

/// Cache TTL for repository file trees (24 hours).
const REPO_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// Client for HuggingFace Hub API operations.
pub struct HuggingFaceClient {
    /// HTTP client for API requests (has total timeout)
    client: Client,
    /// HTTP client for downloads (connect timeout only, no total timeout)
    download_client: Client,
    /// Cache directory for LFS file info (legacy JSON cache)
    cache_dir: PathBuf,
    /// Active downloads
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    /// SQLite search cache (optional)
    search_cache: Option<Arc<HfSearchCache>>,
    /// Download persistence for crash recovery (optional)
    persistence: Option<Arc<DownloadPersistence>>,
}

impl std::fmt::Debug for HuggingFaceClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HuggingFaceClient")
            .field("cache_dir", &self.cache_dir)
            .field("has_search_cache", &self.search_cache.is_some())
            .field("has_persistence", &self.persistence.is_some())
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
    /// Pause flag -- signals graceful stop without deleting .part file
    pause_flag: Arc<AtomicBool>,
    /// Error message if failed
    error: Option<String>,
    /// Destination directory (needed for resume after restart)
    dest_dir: PathBuf,
    /// Filename being downloaded (needed for resume after restart)
    filename: String,
    /// Original download request (needed for persistence/resume)
    download_request: Option<DownloadRequest>,
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

        // Separate client for downloads: connect timeout only, no total timeout.
        // The total timeout would kill multi-gigabyte downloads that take longer
        // than 30 seconds. The stream loop handles progress and cancellation.
        let download_client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .user_agent("pumas-library/1.0")
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create download HTTP client: {}", e),
                cause: None,
            })?;

        Ok(Self {
            client,
            download_client,
            cache_dir,
            downloads: Arc::new(RwLock::new(HashMap::new())),
            search_cache: None,
            persistence: None,
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

    /// Set the download persistence store.
    pub fn set_persistence(&mut self, persistence: Arc<DownloadPersistence>) {
        self.persistence = Some(persistence);
    }

    /// Restore persisted downloads from disk.
    ///
    /// Called during startup to recover paused/errored downloads from a previous session.
    /// Only restores entries whose `.part` file still exists on disk.
    pub async fn restore_persisted_downloads(&self) {
        let persistence = match &self.persistence {
            Some(p) => p,
            None => return,
        };

        let entries = persistence.load_all();
        if entries.is_empty() {
            return;
        }

        info!("Restoring {} persisted downloads", entries.len());
        let mut downloads = self.downloads.write().await;

        for entry in entries {
            let part_path = entry.dest_dir.join(format!(
                "{}{}",
                entry.filename,
                crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX
            ));

            if !part_path.exists() {
                // .part file was cleaned up externally, remove from persistence
                info!(
                    "Removing stale persisted download {} (no .part file)",
                    entry.download_id
                );
                let _ = persistence.remove(&entry.download_id);
                continue;
            }

            // Get actual file size for accurate progress
            let downloaded_bytes = std::fs::metadata(&part_path)
                .map(|m| m.len())
                .unwrap_or(0);

            let progress = entry
                .total_bytes
                .map(|total| downloaded_bytes as f32 / total as f32)
                .unwrap_or(0.0);

            info!(
                "Restoring download {}: {} ({} bytes on disk, status {:?})",
                entry.download_id, entry.repo_id, downloaded_bytes, entry.status
            );

            downloads.insert(
                entry.download_id.clone(),
                DownloadState {
                    download_id: entry.download_id,
                    repo_id: entry.repo_id,
                    status: entry.status,
                    progress,
                    downloaded_bytes,
                    total_bytes: entry.total_bytes,
                    speed: 0.0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    pause_flag: Arc::new(AtomicBool::new(false)),
                    error: None,
                    dest_dir: entry.dest_dir,
                    filename: entry.filename,
                    download_request: Some(entry.download_request),
                },
            );
        }
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

        let pause_flag = Arc::new(AtomicBool::new(false));

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
            pause_flag: pause_flag.clone(),
            error: None,
            dest_dir: dest_dir.to_path_buf(),
            filename: filename.clone(),
            download_request: Some(request.clone()),
        };

        self.downloads
            .write()
            .await
            .insert(download_id.clone(), state);

        // Persist download metadata for crash recovery
        if let Some(ref persistence) = self.persistence {
            let _ = persistence.save(&PersistedDownload {
                download_id: download_id.clone(),
                repo_id: request.repo_id.clone(),
                filename: filename.clone(),
                dest_dir: dest_dir.to_path_buf(),
                total_bytes,
                status: DownloadStatus::Queued,
                download_request: request.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        // Spawn download task (uses download_client which has no total timeout)
        let client = self.download_client.clone();
        let downloads = self.downloads.clone();
        let download_id_clone = download_id.clone();
        let repo_id = request.repo_id.clone();
        let dest_dir = dest_dir.to_path_buf();
        let persistence = self.persistence.clone();

        tokio::spawn(async move {
            let result = Self::run_download(
                client,
                downloads.clone(),
                &download_id_clone,
                &repo_id,
                &filename,
                &dest_dir,
                cancel_flag,
                pause_flag,
                persistence.clone(),
            )
            .await;

            if let Err(e) = result {
                // DownloadPaused is not a real error -- status already set by run_download
                if matches!(e, PumasError::DownloadPaused) {
                    info!("Download paused for {}/{}", repo_id, filename);
                    // Persistence already updated in run_download
                    return;
                }
                error!("Download failed for {}/{}: {}", repo_id, filename, e);
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(&download_id_clone) {
                    state.status = DownloadStatus::Error;
                    state.error = Some(e.to_string());
                }
                // Update persistence with error status (preserve for resume)
                if let Some(ref persistence) = persistence {
                    if let Ok(mut entries) = Ok::<Vec<_>, ()>(persistence.load_all()) {
                        if let Some(entry) = entries.iter_mut().find(|d| d.download_id == download_id_clone) {
                            entry.status = DownloadStatus::Error;
                            let _ = persistence.save(entry);
                        }
                    }
                }
            }
        });

        Ok(download_id)
    }

    /// Run the download in the background with retry and resume support.
    async fn run_download(
        client: Client,
        downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
        download_id: &str,
        repo_id: &str,
        filename: &str,
        dest_dir: &Path,
        cancel_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        persistence: Option<Arc<DownloadPersistence>>,
    ) -> Result<()> {
        use crate::config::NetworkConfig;
        use crate::network::RetryConfig;

        // Update status to downloading
        {
            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.status = DownloadStatus::Downloading;
            }
        }

        let url = format!("{}/{}/resolve/main/{}", HF_HUB_BASE, repo_id, filename);

        std::fs::create_dir_all(dest_dir)?;
        let dest_path = dest_dir.join(filename);
        let part_path = dest_dir.join(format!(
            "{}{}",
            filename,
            NetworkConfig::DOWNLOAD_TEMP_SUFFIX
        ));

        // Get total_bytes from DownloadState (set by start_download from repo tree)
        let total_bytes_expected = {
            let downloads = downloads.read().await;
            downloads.get(download_id).and_then(|s| s.total_bytes)
        };

        let retry_config = RetryConfig::new()
            .with_max_attempts(NetworkConfig::HF_DOWNLOAD_MAX_RETRIES)
            .with_base_delay(NetworkConfig::HF_DOWNLOAD_RETRY_BASE_DELAY);

        let mut last_error: Option<PumasError> = None;

        for attempt in 0..retry_config.max_attempts {
            // Check cancellation before each attempt
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = tokio::fs::remove_file(&part_path).await;
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Cancelled;
                }
                if let Some(ref persistence) = persistence {
                    let _ = persistence.remove(download_id);
                }
                return Err(PumasError::DownloadCancelled);
            }

            // Check pause before each attempt
            if pause_flag.load(Ordering::Relaxed) {
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Paused;
                }
                if let Some(ref persistence) = persistence {
                    Self::persist_status_update(persistence, download_id, DownloadStatus::Paused);
                }
                return Err(PumasError::DownloadPaused);
            }

            // Determine resume offset from existing .part file
            let resume_from_byte = tokio::fs::metadata(&part_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0);

            if attempt > 0 {
                warn!(
                    "Retry {}/{} for {}/{} (resuming from byte {})",
                    attempt + 1,
                    retry_config.max_attempts,
                    repo_id,
                    filename,
                    resume_from_byte
                );

                // Reset status to Downloading for the retry
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Downloading;
                    state.error = None;
                }
            }

            match Self::download_attempt(
                &client,
                &downloads,
                download_id,
                &url,
                &part_path,
                total_bytes_expected,
                resume_from_byte,
                &cancel_flag,
                &pause_flag,
            )
            .await
            {
                Ok(_) => {
                    // Rename .part to final path atomically
                    tokio::fs::rename(&part_path, &dest_path).await.map_err(
                        |e| PumasError::DownloadFailed {
                            url: url.clone(),
                            message: format!("Failed to rename temp file: {}", e),
                        },
                    )?;

                    // Update status to completed
                    let mut downloads = downloads.write().await;
                    if let Some(state) = downloads.get_mut(download_id) {
                        state.status = DownloadStatus::Completed;
                        state.progress = 1.0;
                    }

                    // Remove from persistence -- download is done
                    if let Some(ref persistence) = persistence {
                        let _ = persistence.remove(download_id);
                    }

                    return Ok(());
                }
                Err(e) => {
                    // Paused -- .part preserved, not a real error
                    if matches!(e, PumasError::DownloadPaused) {
                        if let Some(ref persistence) = persistence {
                            Self::persist_status_update(persistence, download_id, DownloadStatus::Paused);
                        }
                        return Err(e);
                    }

                    if !e.is_retryable() || cancel_flag.load(Ordering::Relaxed) {
                        // Only delete .part on cancel; preserve on other errors for resume
                        if cancel_flag.load(Ordering::Relaxed) {
                            let _ = tokio::fs::remove_file(&part_path).await;
                            if let Some(ref persistence) = persistence {
                                let _ = persistence.remove(download_id);
                            }
                        }
                        return Err(e);
                    }

                    warn!(
                        "Download attempt {}/{} failed for {}/{}: {}",
                        attempt + 1,
                        retry_config.max_attempts,
                        repo_id,
                        filename,
                        e
                    );
                    last_error = Some(e);

                    if attempt + 1 < retry_config.max_attempts {
                        let delay = retry_config.calculate_delay(attempt);
                        debug!("Waiting {:?} before retry", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        // All retries exhausted -- preserve .part for potential resume
        Err(last_error.unwrap_or_else(|| PumasError::DownloadFailed {
            url,
            message: "All retry attempts exhausted".to_string(),
        }))
    }

    /// Execute a single download attempt, optionally resuming from a byte offset.
    async fn download_attempt(
        client: &Client,
        downloads: &Arc<RwLock<HashMap<String, DownloadState>>>,
        download_id: &str,
        url: &str,
        part_path: &Path,
        total_bytes_expected: Option<u64>,
        resume_from_byte: u64,
        cancel_flag: &Arc<AtomicBool>,
        pause_flag: &Arc<AtomicBool>,
    ) -> Result<()> {
        use futures::StreamExt;

        let mut request = client.get(url);
        if resume_from_byte > 0 {
            request = request.header("Range", format!("bytes={}-", resume_from_byte));
            info!("Resuming download from byte {}", resume_from_byte);
        }

        let response = request.send().await.map_err(|e| PumasError::Network {
            message: format!("Download request failed: {}", e),
            cause: Some(e.to_string()),
        })?;

        let status = response.status();

        // Check for non-success responses (but 206 Partial Content is expected for resume)
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(PumasError::DownloadFailed {
                url: url.to_string(),
                message: format!("HTTP {}", status),
            });
        }

        // Determine if we're actually resuming
        let is_resuming =
            resume_from_byte > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
        if resume_from_byte > 0 && !is_resuming {
            warn!("Server does not support Range requests, restarting from zero");
        }

        // For resumed downloads, content_length is the remaining bytes.
        // Use total_bytes_expected for progress tracking.
        let effective_total = if is_resuming {
            total_bytes_expected
        } else {
            response.content_length().or(total_bytes_expected)
        };

        // Open file: append for resume, create for fresh start
        let mut file = if is_resuming {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(part_path)
                .await?
        } else {
            tokio::fs::File::create(part_path).await?
        };

        let mut downloaded: u64 = if is_resuming { resume_from_byte } else { 0 };
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            if cancel_flag.load(Ordering::Relaxed) {
                drop(file);
                let _ = tokio::fs::remove_file(part_path).await;

                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Cancelled;
                }

                return Err(PumasError::DownloadCancelled);
            }

            if pause_flag.load(Ordering::Relaxed) {
                file.flush().await?;
                drop(file);
                // Preserve .part file for resume

                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Paused;
                }

                return Err(PumasError::DownloadPaused);
            }

            let chunk = chunk.map_err(|e| PumasError::Network {
                message: format!("Download stream error: {}", e),
                cause: Some(e.to_string()),
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

            let progress = if let Some(total) = effective_total {
                downloaded as f32 / total as f32
            } else {
                0.0
            };

            let mut downloads = downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.downloaded_bytes = downloaded;
                state.total_bytes = effective_total;
                state.progress = progress;
                state.speed = speed;
            }
        }

        file.flush().await?;
        drop(file);

        // Verify download completeness
        if let Some(total) = effective_total {
            if downloaded != total {
                return Err(PumasError::Network {
                    message: format!(
                        "Incomplete download: got {} of {} bytes",
                        downloaded, total
                    ),
                    cause: None,
                });
            }
        }

        Ok(())
    }

    /// Helper: update status in persistence store (best-effort).
    fn persist_status_update(
        persistence: &DownloadPersistence,
        download_id: &str,
        status: DownloadStatus,
    ) {
        let entries = persistence.load_all();
        if let Some(mut entry) = entries.into_iter().find(|d| d.download_id == download_id) {
            entry.status = status;
            let _ = persistence.save(&entry);
        }
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
            // Remove from persistence -- cancelled downloads don't survive restart
            if let Some(ref persistence) = self.persistence {
                let _ = persistence.remove(download_id);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all downloads (active, paused, completed, etc.).
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
                eta_seconds: if state.speed > 0.0 && state.total_bytes.is_some() {
                    let remaining = state.total_bytes.unwrap().saturating_sub(state.downloaded_bytes);
                    Some(remaining as f64 / state.speed)
                } else {
                    None
                },
                error: state.error.clone(),
            })
            .collect()
    }

    /// Pause an active download. Preserves the `.part` file for later resume.
    pub async fn pause_download(&self, download_id: &str) -> Result<bool> {
        let downloads = self.downloads.read().await;
        if let Some(state) = downloads.get(download_id) {
            if state.status == DownloadStatus::Downloading
                || state.status == DownloadStatus::Queued
            {
                state.pause_flag.store(true, Ordering::Relaxed);
                drop(downloads);
                // Set transitional Pausing status
                let mut downloads = self.downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.status = DownloadStatus::Pausing;
                }
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Resume a paused or errored download from its `.part` file.
    pub async fn resume_download(&self, download_id: &str) -> Result<bool> {
        let (repo_id, filename, dest_dir, cancel_flag, pause_flag) = {
            let mut downloads = self.downloads.write().await;
            let state = match downloads.get_mut(download_id) {
                Some(s) => s,
                None => return Ok(false),
            };

            if state.status != DownloadStatus::Paused && state.status != DownloadStatus::Error {
                return Ok(false);
            }

            // Reset flags and status for re-download
            state.pause_flag.store(false, Ordering::Relaxed);
            state.cancel_flag.store(false, Ordering::Relaxed);
            state.status = DownloadStatus::Queued;
            state.error = None;
            state.speed = 0.0;

            (
                state.repo_id.clone(),
                state.filename.clone(),
                state.dest_dir.clone(),
                state.cancel_flag.clone(),
                state.pause_flag.clone(),
            )
        };

        // Update persistence to Queued status
        if let Some(ref persistence) = self.persistence {
            Self::persist_status_update(persistence, download_id, DownloadStatus::Queued);
        }

        // Re-spawn the download task
        let client = self.download_client.clone();
        let downloads = self.downloads.clone();
        let download_id_clone = download_id.to_string();
        let persistence = self.persistence.clone();

        tokio::spawn(async move {
            let result = Self::run_download(
                client,
                downloads.clone(),
                &download_id_clone,
                &repo_id,
                &filename,
                &dest_dir,
                cancel_flag,
                pause_flag,
                persistence.clone(),
            )
            .await;

            if let Err(e) = result {
                if matches!(e, PumasError::DownloadPaused) {
                    info!("Download paused for {}/{}", repo_id, filename);
                    return;
                }
                error!("Download failed for {}/{}: {}", repo_id, filename, e);
                let mut downloads = downloads.write().await;
                if let Some(state) = downloads.get_mut(&download_id_clone) {
                    state.status = DownloadStatus::Error;
                    state.error = Some(e.to_string());
                }
                // Update persistence with error status
                if let Some(ref persistence) = persistence {
                    Self::persist_status_update(persistence, &download_id_clone, DownloadStatus::Error);
                }
            }
        });

        Ok(true)
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
