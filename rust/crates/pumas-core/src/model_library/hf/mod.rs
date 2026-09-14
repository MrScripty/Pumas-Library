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
mod bundles;
mod download;
mod lifecycle;
mod metadata;
mod search;
mod types;

pub use auth::HfAuthStatus;
pub(crate) use lifecycle::TaskContext as DownloadInvocationContext;
pub use types::{
    AuxFilesCompleteCallback, AuxFilesCompleteInfo, DownloadCompletionCallback,
    DownloadCompletionInfo,
};
use types::{DownloadState, REPO_CACHE_TTL_SECS};

use download::DownloadPublicationOwner;
use lifecycle::{DestinationExecutionOwner, DownloadTaskOwner};

use crate::error::{PumasError, Result};
use crate::model_library::download_store::DownloadPersistence;
use crate::model_library::hf_cache::HfSearchCache;
use crate::network::{CacheStrategy, WebSource, WebSourceId};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{debug, info, warn};

/// An observation of this live owner's downloads, without reconciliation or I/O.
/// Absence here does not establish the absence of persisted acquisition custody.
#[derive(Debug, Clone)]
pub(crate) struct IntentDownloadSnapshot {
    pub(crate) owner_closed: bool,
    pub(crate) downloads: Vec<IntentDownloadObservation>,
}

#[derive(Debug, Clone)]
pub(crate) struct IntentDownloadObservation {
    pub(crate) download_id: String,
    pub(crate) model_ref: Option<crate::models::PumasModelRef>,
    pub(crate) repo_id: String,
    pub(crate) status: crate::models::DownloadStatus,
    pub(crate) downloaded_bytes: u64,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) blocked: bool,
}

/// Client for HuggingFace Hub API operations.
pub struct HuggingFaceClient {
    /// Configured mutation authority; absent for standalone search-only clients.
    pub(super) destination_root: Option<super::download_recovery::DownloadDestinationRoot>,
    /// HTTP client for API requests (has total timeout)
    pub(super) client: Client,
    /// HTTP client for downloads (connect timeout only, no total timeout)
    pub(super) download_client: Client,
    /// Cache directory for LFS file info (legacy JSON cache)
    pub(super) cache_dir: PathBuf,
    /// Active downloads
    pub(super) downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    /// Monotonic revision for download-state snapshots.
    pub(super) download_revision: Arc<AtomicU64>,
    /// Broadcast channel for download-state updates.
    pub(super) download_updates: broadcast::Sender<crate::models::ModelDownloadUpdateNotification>,
    /// Serializes download snapshot capture, revision assignment, and dispatch.
    pub(super) download_publications: Arc<DownloadPublicationOwner>,
    /// Owner of background download tasks and their blocking filesystem work.
    download_tasks: Arc<DownloadTaskOwner>,
    /// Only the public client requests closure on Drop; invocation snapshots
    /// borrow its configuration while their work belongs to `download_tasks`.
    owns_lifecycle: bool,
    /// Generation-scoped serialization of shared destination effects.
    destination_executions: Arc<DestinationExecutionOwner>,
    /// Per-destination mutexes so downloads targeting the same model folder
    /// execute sequentially instead of racing on shared files.
    pub(super) dest_locks:
        Arc<RwLock<HashMap<super::download_recovery::DestinationIdentity, Arc<Mutex<()>>>>>,
    /// SQLite search cache (optional)
    pub(super) search_cache: Option<Arc<HfSearchCache>>,
    /// Download persistence for crash recovery (optional)
    pub(super) persistence: Option<Arc<DownloadPersistence>>,
    /// Required metadata/index mutation for configured ordinary downloads.
    download_importer: Option<Arc<super::ModelImporter>>,
    /// Optional callback invoked when a download completes successfully.
    pub(super) completion_callback: Option<DownloadCompletionCallback>,
    /// Optional callback invoked after auxiliary files download but before weight files.
    pub(super) aux_complete_callback: Option<AuxFilesCompleteCallback>,
    /// Authentication token for accessing gated/private models.
    pub(super) auth_token: Arc<RwLock<Option<String>>>,
    #[cfg(test)]
    download_base_url: Option<String>,
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
    /// Observe only existing live state; unlike operational download reads this
    /// never reconciles workers, opens persistence, or publishes a snapshot.
    pub(crate) async fn intent_download_snapshot(&self) -> IntentDownloadSnapshot {
        let states = self.downloads.read().await;
        let mut downloads = states
            .values()
            .map(|state| {
                let model_ref = state.destination.as_ref().map(|destination| {
                    let selected_artifact_id = state.download_request.as_ref().map(|request| {
                        super::artifact_identity::SelectedArtifactIdentity::from_download_request_at_revision(
                            request,
                            Some(state.files.iter().map(|file| file.filename.clone()).collect()),
                            &state.revision,
                        )
                        .artifact_id
                    });
                    crate::models::PumasModelRef {
                        model_ref_contract_version: crate::models::PUMAS_MODEL_REF_CONTRACT_VERSION,
                        model_id: destination.capability().library_model_id(),
                        revision: state.revision.as_persisted().map(str::to_owned),
                        selected_artifact_id,
                        selected_artifact_path: None,
                        migration_diagnostics: Vec::new(),
                    }
                });
                IntentDownloadObservation {
                    download_id: state.download_id.clone(),
                    model_ref,
                    repo_id: state.repo_id.clone(),
                    status: state.status,
                    downloaded_bytes: state.downloaded_bytes,
                    total_bytes: state.total_bytes,
                    blocked: state.ambient_authority_blocked
                        || state.lifecycle_failure_unverified
                        || state.recovery_destination().is_some(),
                }
            })
            .collect::<Vec<_>>();
        downloads.sort_by(|left, right| left.download_id.cmp(&right.download_id));
        IntentDownloadSnapshot {
            owner_closed: self.download_tasks.is_closed(),
            downloads,
        }
    }

    pub(super) fn hub_base_url(&self) -> &str {
        #[cfg(test)]
        if let Some(base) = self.download_base_url.as_deref() {
            return base;
        }
        types::HF_HUB_BASE
    }

    pub(super) fn api_base_url(&self) -> &str {
        #[cfg(test)]
        if let Some(base) = self.download_base_url.as_deref() {
            return base;
        }
        types::HF_API_BASE
    }

    fn clone_for_invocation(&self) -> Self {
        Self {
            destination_root: self.destination_root.clone(),
            client: self.client.clone(),
            download_client: self.download_client.clone(),
            cache_dir: self.cache_dir.clone(),
            downloads: self.downloads.clone(),
            download_revision: self.download_revision.clone(),
            download_updates: self.download_updates.clone(),
            download_publications: self.download_publications.clone(),
            download_tasks: self.download_tasks.clone(),
            owns_lifecycle: false,
            destination_executions: self.destination_executions.clone(),
            dest_locks: self.dest_locks.clone(),
            search_cache: self.search_cache.clone(),
            persistence: self.persistence.clone(),
            download_importer: self.download_importer.clone(),
            completion_callback: self.completion_callback.clone(),
            aux_complete_callback: self.aux_complete_callback.clone(),
            auth_token: self.auth_token.clone(),
            #[cfg(test)]
            download_base_url: self.download_base_url.clone(),
        }
    }

    pub(crate) async fn run_download_invocation<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(DownloadInvocationContext) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    {
        self.download_tasks.run_invocation(operation).await
    }

    /// Protect one download mutation phase without retaining exclusion in idle clients.
    pub(crate) async fn protect_download_mutation(
        &self,
        context: &DownloadInvocationContext,
    ) -> Result<DownloadInvocationContext> {
        match self.destination_root.clone() {
            Some(root) => context.with_root_grant(root).await,
            None => Err(crate::PumasError::Config {
                message: "Download mutation requires a configured library root".into(),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_test_download_base_url(&mut self, base_url: String) {
        self.download_base_url = Some(base_url);
    }

    pub(crate) fn configure_download_destination_root(
        &mut self,
        path: &std::path::Path,
    ) -> Result<()> {
        let root = super::download_recovery::DownloadDestinationRoot::open(path)?;
        self.set_download_destination_root(root);
        Ok(())
    }

    pub(crate) fn set_download_destination_root(
        &mut self,
        root: super::download_recovery::DownloadDestinationRoot,
    ) {
        self.destination_root = Some(root);
    }

    /// Create a new HuggingFace client.
    ///
    /// Standalone clients support search and inspection. Download mutation
    /// requires held model-root authority configured by the `PumasApi` builder;
    /// supplying persistence alone does not grant that authority.
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

        let downloads = Arc::new(RwLock::new(HashMap::new()));
        let download_revision = Arc::new(AtomicU64::new(0));
        let download_updates = broadcast::channel(64).0;
        let download_publications = Arc::new(DownloadPublicationOwner::new(
            downloads.clone(),
            download_revision.clone(),
            download_updates.clone(),
        ));

        Ok(Self {
            destination_root: None,
            client,
            download_client,
            cache_dir,
            downloads,
            download_revision,
            download_updates,
            download_publications,
            download_tasks: Arc::new(DownloadTaskOwner::new()),
            owns_lifecycle: true,
            destination_executions: Arc::new(DestinationExecutionOwner::new()),
            dest_locks: Arc::new(RwLock::new(HashMap::new())),
            search_cache: None,
            persistence: None,
            download_importer: None,
            completion_callback: None,
            aux_complete_callback: None,
            auth_token: Arc::new(RwLock::new(initial_token)),
            #[cfg(test)]
            download_base_url: None,
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

    pub(crate) fn set_download_importer(&mut self, importer: Arc<super::ModelImporter>) {
        self.download_importer = Some(importer);
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
        let token = token.trim().to_string();
        let token_for_disk = token.clone();
        tokio::task::spawn_blocking(move || auth::save_token(&token_for_disk))
            .await
            .map_err(|e| {
                PumasError::Other(format!("Failed to join set_auth_token task: {}", e))
            })??;
        *self.auth_token.write().await = Some(token);
        info!("HuggingFace auth token saved");
        Ok(())
    }

    /// Clear the HuggingFace authentication token.
    ///
    /// Removes the persisted token file and clears the in-memory value.
    pub async fn clear_auth_token(&self) -> Result<()> {
        tokio::task::spawn_blocking(auth::clear_token)
            .await
            .map_err(|e| {
                PumasError::Other(format!("Failed to join clear_auth_token task: {}", e))
            })??;
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
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
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
}

impl Drop for HuggingFaceClient {
    fn drop(&mut self) {
        if self.owns_lifecycle {
            let downloads = self.downloads.clone();
            let publications = self.download_publications.clone();
            self.download_tasks.request_shutdown(move || {
                download::project_download_shutdown(downloads, publications)
            });
        }
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

    #[tokio::test]
    async fn intent_snapshot_does_not_open_store_reconcile_or_publish() {
        let (temp, mut client) = setup();
        client
            .configure_download_destination_root(temp.path())
            .unwrap();
        client.set_persistence(Arc::new(DownloadPersistence::new(temp.path())));
        let store_path = temp.path().join("downloads.json");
        std::fs::write(&store_path, b"deliberately invalid store").unwrap();
        let mut notifications = client.subscribe_download_updates();
        let revision = client
            .download_revision
            .load(std::sync::atomic::Ordering::SeqCst);
        let observed = client.intent_download_snapshot().await;
        assert!(!observed.owner_closed);
        assert!(observed.downloads.is_empty());
        assert_eq!(
            std::fs::read(&store_path).unwrap(),
            b"deliberately invalid store"
        );
        assert!(!temp.path().join(".downloads.lock").exists());
        assert_eq!(
            client
                .download_revision
                .load(std::sync::atomic::Ordering::SeqCst),
            revision
        );
        assert!(matches!(
            notifications.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        assert!(client.download_tasks.is_empty());
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
            card_data: None,
        };

        let model = HuggingFaceClient::convert_search_result(mock_result);

        assert_eq!(model.repo_id, "TheBloke/Llama-2-7B-GGUF");
        assert_eq!(model.name, "Llama-2-7B-GGUF");
        assert_eq!(model.developer, "TheBloke");
        assert!(model.formats.contains(&"gguf".to_string()));
        assert!(model.quants.contains(&"Q4_K_M".to_string()));
    }
}
