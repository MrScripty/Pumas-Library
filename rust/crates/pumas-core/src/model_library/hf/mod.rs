//! HuggingFace client for model search, download, and metadata lookup.
//!
//! Provides integration with the HuggingFace Hub API:
//! - Model search with filters
//! - Repository metadata and file listing
//! - Download with progress tracking
//! - Metadata lookup by filename/hash
//!
//! # Module Organization
//!
//! - [`types`] - Shared types, API response structs, and constants
//! - [`search`] - Model search with caching and enrichment
//! - [`metadata`] - Direct model info, repo file trees, and metadata lookup
//! - [`download`] - Download management with pause/resume/cancel
//! - [`auth`] - Authentication token management

mod auth;
mod download;
mod metadata;
mod search;
mod types;

pub use auth::HfAuthStatus;
pub use types::{
    AuxFilesCompleteCallback, AuxFilesCompleteInfo, DownloadCompletionCallback,
    DownloadCompletionInfo,
};
use types::{DownloadState, REPO_CACHE_TTL_SECS};

use crate::error::{PumasError, Result};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::download_store::DownloadPersistence;
use crate::model_library::hf_cache::HfSearchCache;
use crate::network::{CacheStrategy, WebSource, WebSourceId};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Client for HuggingFace Hub API operations.
pub struct HuggingFaceClient {
    /// HTTP client for API requests (has total timeout)
    pub(super) client: Client,
    /// HTTP client for downloads (connect timeout only, no total timeout)
    pub(super) download_client: Client,
    /// Cache directory for LFS file info (legacy JSON cache)
    pub(super) cache_dir: PathBuf,
    /// Active downloads
    pub(super) downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    /// SQLite search cache (optional)
    pub(super) search_cache: Option<Arc<HfSearchCache>>,
    /// Download persistence for crash recovery (optional)
    pub(super) persistence: Option<Arc<DownloadPersistence>>,
    /// Optional callback invoked when a download completes successfully.
    pub(super) completion_callback: Option<DownloadCompletionCallback>,
    /// Optional callback invoked after auxiliary files download but before weight files.
    pub(super) aux_complete_callback: Option<AuxFilesCompleteCallback>,
    /// Authentication token for accessing gated/private models.
    pub(super) auth_token: Arc<RwLock<Option<String>>>,
}

impl std::fmt::Debug for HuggingFaceClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HuggingFaceClient")
            .field("cache_dir", &self.cache_dir)
            .field("has_search_cache", &self.search_cache.is_some())
            .field("has_persistence", &self.persistence.is_some())
            .field("has_auth_token", &"<redacted>")
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

        let initial_token = auth::resolve_token_from_disk().map(|(token, source)| {
            info!("HuggingFace auth token found from {}", source);
            token
        });

        Ok(Self {
            client,
            download_client,
            cache_dir,
            downloads: Arc::new(RwLock::new(HashMap::new())),
            search_cache: None,
            persistence: None,
            completion_callback: None,
            aux_complete_callback: None,
            auth_token: Arc::new(RwLock::new(initial_token)),
        })
    }

    /// Create a new HuggingFace client with SQLite search cache.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for caching API responses (legacy JSON)
    /// * `search_cache` - SQLite search cache for intelligent caching
    pub fn with_cache(
        cache_dir: impl Into<PathBuf>,
        search_cache: Arc<HfSearchCache>,
    ) -> Result<Self> {
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

    /// Get a reference to the download persistence store if available.
    pub fn persistence(&self) -> Option<&Arc<DownloadPersistence>> {
        self.persistence.as_ref()
    }

    /// Set a callback that fires when a download completes successfully.
    ///
    /// Used to trigger in-place import (metadata creation + indexing) after download.
    pub fn set_completion_callback(&mut self, callback: DownloadCompletionCallback) {
        self.completion_callback = Some(callback);
    }

    /// Set a callback that fires after auxiliary files download but before weight files begin.
    ///
    /// Used to create a preliminary metadata stub so the model appears in the library
    /// index while weights are still downloading.
    pub fn set_aux_complete_callback(&mut self, callback: AuxFilesCompleteCallback) {
        self.aux_complete_callback = Some(callback);
    }

    // ========================================
    // Authentication
    // ========================================

    /// Set the HuggingFace authentication token.
    ///
    /// Persists to disk at `{pumas_config_dir}/hf_token` and updates the
    /// in-memory token for immediate use by subsequent API calls.
    pub async fn set_auth_token(&self, token: &str) -> Result<()> {
        auth::save_token(token)?;
        *self.auth_token.write().await = Some(token.trim().to_string());
        info!("HuggingFace auth token saved");
        Ok(())
    }

    /// Clear the HuggingFace authentication token.
    ///
    /// Removes the persisted token file and clears the in-memory value.
    pub async fn clear_auth_token(&self) -> Result<()> {
        auth::clear_token()?;
        *self.auth_token.write().await = None;
        info!("HuggingFace auth token cleared");
        Ok(())
    }

    /// Get current authentication status by calling the HF whoami endpoint.
    ///
    /// Makes a lightweight API call to validate the token and retrieve
    /// the associated username. Returns unauthenticated status if no
    /// token is configured or if the token is invalid.
    pub async fn get_auth_status(&self) -> Result<HfAuthStatus> {
        let token = {
            let guard = self.auth_token.read().await;
            match guard.as_ref() {
                Some(t) => t.clone(),
                None => {
                    return Ok(HfAuthStatus {
                        authenticated: false,
                        username: None,
                        token_source: None,
                    });
                }
            }
        };

        let response = self
            .client
            .get(auth::HF_WHOAMI_URL)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                let username = body.get("name").and_then(|v| v.as_str()).map(String::from);
                let source = self.resolve_token_source().await;
                Ok(HfAuthStatus {
                    authenticated: true,
                    username,
                    token_source: Some(source),
                })
            }
            _ => Ok(HfAuthStatus {
                authenticated: false,
                username: None,
                token_source: None,
            }),
        }
    }

    /// Get the current Bearer header value for authenticated requests.
    pub(super) async fn auth_header_value(&self) -> Option<String> {
        let guard = self.auth_token.read().await;
        guard.as_ref().map(|t| format!("Bearer {}", t))
    }

    /// Determine where the current token was resolved from.
    async fn resolve_token_source(&self) -> String {
        if let Ok(path) = auth::hf_token_path() {
            if path.exists() {
                return "pumas_config".to_string();
            }
        }
        if std::env::var("HF_TOKEN").is_ok() {
            return "env_var".to_string();
        }
        "hf_cache".to_string()
    }

    // ========================================
    // Cache Helpers
    // ========================================

    pub(super) fn get_cache_path(&self, repo_id: &str, suffix: &str) -> PathBuf {
        let safe_name = repo_id.replace('/', "_");
        self.cache_dir
            .join(format!("hf_{}_{}.json", safe_name, suffix))
    }

    pub(super) fn read_cache<T: for<'de> Deserialize<'de>>(
        &self,
        path: &Path,
    ) -> Result<Option<T>> {
        atomic_read_json(path)
    }

    pub(super) fn write_cache<T: Serialize>(&self, path: &Path, data: &T) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::types::HfSearchResult;
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, HuggingFaceClient) {
        let temp_dir = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(temp_dir.path()).unwrap();
        (temp_dir, client)
    }

    #[test]
    fn test_filename_confidence() {
        let (_temp, _client) = setup();

        // Exact match
        assert_eq!(
            HuggingFaceClient::compute_filename_confidence("llama", "llama"),
            1.0
        );

        // Substring match
        let confidence = HuggingFaceClient::compute_filename_confidence("llama", "llama-2-7b");
        assert!(confidence > 0.7);

        // Partial word match
        let confidence =
            HuggingFaceClient::compute_filename_confidence("llama-7b", "llama-2-7b-chat");
        assert!(confidence > 0.3); // Lower threshold for partial matches

        // No match
        let confidence = HuggingFaceClient::compute_filename_confidence("gpt", "llama");
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
        let (_temp, _client) = setup();

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
            config: None,
        };

        let model = HuggingFaceClient::convert_search_result(mock_result);

        assert_eq!(model.repo_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(model.name, "Llama-2-7B-GGUF");
        assert_eq!(model.developer, "TheBloke");
        assert!(model.formats.contains(&"gguf".to_string()));
        assert!(model.quants.contains(&"Q4_K_M".to_string()));
    }
}
