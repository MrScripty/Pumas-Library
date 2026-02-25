//! Pumas Core - Headless library for AI model management and system utilities.
//!
//! This crate provides the core functionality for managing AI models and system
//! utilities. It can be used programmatically without any HTTP/RPC layer.
//!
//! For version management (ComfyUI versions, custom nodes), see the
//! `pumas-app-manager` crate.
//!
//! # Example
//!
//! ```rust,ignore
//! use pumas_library::PumasApi;
//!
//! #[tokio::main]
//! async fn main() -> pumas_library::Result<()> {
//!     let api = PumasApi::new("/path/to/pumas").await?;
//!
//!     // List models in the library
//!     let models = api.list_models().await?;
//!     println!("Found {} models", models.len());
//!
//!     // Search for models
//!     let search = api.search_models("llama", 10, 0).await?;
//!     println!("Search found {} results", search.total_count);
//!
//!     Ok(())
//! }
//! ```

// UniFFI scaffolding - generates the FFI type registry when uniffi feature is enabled
#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();

pub mod cache;
pub mod cancel;
pub mod config;
pub mod conversion;
pub mod error;
pub mod index;
pub mod ipc;
pub mod launcher;
pub mod metadata;
pub mod model_library;
pub mod models;
pub mod network;
pub mod platform;
pub mod plugins;
pub mod registry;
pub mod process;
pub mod system;

mod api;

// Re-export commonly used types
pub use cache::{CacheBackend, CacheConfig, CacheEntry, CacheMeta, CacheStats, SqliteCache};
pub use cancel::{CancellationToken, CancelledError};
pub use config::AppId;
pub use error::{PumasError, Result};
pub use index::{ModelIndex, ModelRecord, SearchResult};
pub use launcher::{LauncherUpdater, PatchManager, UpdateApplyResult, UpdateCheckResult};
pub use metadata::MetadataManager;
pub use model_library::sharding::{self, ShardValidation};
pub use model_library::{
    BatchImportProgress, DownloadRequest, HfAuthStatus, HfSearchParams, HuggingFaceClient,
    ModelImporter, ModelLibrary, ModelMapper,
};
pub use models::CommitInfo;
pub use plugins::{PluginConfig, PluginLoader};
pub use process::{ProcessInfo, ProcessManager};
pub use system::{
    check_brave, check_git, check_setproctitle, GpuInfo, GpuMonitor, ProcessResources,
    ResourceTracker, SystemCheckResult, SystemResourceSnapshot, SystemUtils,
};

// Re-export builder from api module
pub use api::PumasApiBuilder;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use api::{ApiState, PrimaryState};

/// Main API struct for Pumas operations.
///
/// This is the primary entry point for programmatic access to Pumas functionality.
/// It provides model library and system utilities for integrating Pumas functionality
/// into other applications.
///
/// Internally, `PumasApi` operates in one of two modes:
/// - **Primary**: Owns the full state (model library, network, processes, etc.)
///   and runs an IPC server for other instances to connect to.
/// - **Client**: Proxies calls to a running primary instance via TCP IPC.
///
/// The mode is transparent to callers — the public API is identical.
pub struct PumasApi {
    /// Root directory for launcher data (available in both modes)
    launcher_root: PathBuf,
    /// Internal mode dispatch
    inner: ApiInner,
}

/// Internal dispatch: Primary owns state, Client proxies via IPC.
enum ApiInner {
    Primary(Arc<PrimaryState>),
    Client,
}

impl PumasApi {
    /// Get a reference to the primary state, or error if in client mode.
    fn try_primary(&self) -> Result<&Arc<PrimaryState>> {
        match &self.inner {
            ApiInner::Primary(state) => Ok(state),
            ApiInner::Client => Err(PumasError::Other(
                "This method is only available on primary instances".to_string(),
            )),
        }
    }

    /// Get a reference to the primary state. Panics if in client mode.
    /// Use only for methods that are guaranteed primary-only.
    fn primary(&self) -> &Arc<PrimaryState> {
        match &self.inner {
            ApiInner::Primary(state) => state,
            ApiInner::Client => {
                panic!("BUG: primary-only method called on client instance")
            }
        }
    }

    /// Returns true if this instance is the primary (owns full state).
    pub fn is_primary(&self) -> bool {
        matches!(&self.inner, ApiInner::Primary(_))
    }

    /// Create a builder for PumasApi.
    ///
    /// Use the builder for more control over initialization options:
    /// - `auto_create_dirs`: Create required directories automatically
    /// - `with_hf_client`: Enable/disable HuggingFace integration
    /// - `with_process_manager`: Enable/disable process management
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let api = PumasApi::builder("./my-models")
    ///     .auto_create_dirs(true)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn builder(launcher_root: impl Into<PathBuf>) -> PumasApiBuilder {
        PumasApiBuilder::new(launcher_root)
    }

    /// Create a new PumasApi instance.
    ///
    /// # Arguments
    ///
    /// * `launcher_root` - Path to the launcher root directory (containing launcher-data, etc.)
    pub async fn new(launcher_root: impl Into<PathBuf>) -> Result<Self> {
        let launcher_root = launcher_root.into();

        // Ensure the launcher root exists
        if !launcher_root.exists() {
            return Err(PumasError::Config {
                message: format!("Launcher root does not exist: {}", launcher_root.display()),
            });
        }

        let state = Arc::new(RwLock::new(ApiState {
            background_fetch_completed: false,
        }));

        // Initialize network manager for connectivity checking
        let network_manager = Arc::new(
            network::NetworkManager::new().map_err(|e| PumasError::Config {
                message: format!("Failed to initialize network manager: {}", e),
            })?,
        );

        // Check initial connectivity (non-blocking, will update state)
        let nm_clone = network_manager.clone();
        tokio::spawn(async move {
            nm_clone.check_connectivity().await;
        });

        // Initialize process manager
        let process_manager = match process::ProcessManager::new(&launcher_root, None) {
            Ok(mgr) => Arc::new(RwLock::new(Some(mgr))),
            Err(e) => {
                tracing::warn!("Failed to initialize process manager: {}", e);
                Arc::new(RwLock::new(None))
            }
        };

        // Initialize system utilities
        let system_utils = Arc::new(system::SystemUtils::new(&launcher_root));

        // Initialize model library for AI model management
        // Uses shared-resources/models to match Python backend path
        let model_library_dir = launcher_root
            .join("shared-resources")
            .join("models");
        let mapping_config_dir = launcher_root
            .join("launcher-data")
            .join("mapping-configs");

        // Initialize HuggingFace client for model search/download (optional - external service)
        let cache_dir = launcher_root
            .join("launcher-data")
            .join(config::PathsConfig::CACHE_DIR_NAME);
        let hf_cache_dir = cache_dir.join("hf");

        // Initialize SQLite search cache at shared-resources/cache/search.sqlite
        let search_cache_dir = launcher_root.join("shared-resources").join("cache");
        let search_cache_db = search_cache_dir.join("search.sqlite");
        let search_cache = match model_library::HfSearchCache::new(&search_cache_db) {
            Ok(cache) => Some(std::sync::Arc::new(cache)),
            Err(e) => {
                tracing::warn!("Failed to initialize HuggingFace search cache: {}", e);
                None
            }
        };

        // Initialize download persistence
        let data_dir = launcher_root.join("launcher-data");
        let download_persistence = std::sync::Arc::new(
            model_library::DownloadPersistence::new(&data_dir)
        );

        let mut hf_client = match model_library::HuggingFaceClient::new(&hf_cache_dir) {
            Ok(mut client) => {
                // Attach search cache if available
                if let Some(cache) = search_cache {
                    client.set_search_cache(cache);
                }
                // Attach download persistence
                client.set_persistence(download_persistence);
                // Restore persisted downloads from previous session
                client.restore_persisted_downloads().await;
                Some(client)
            }
            Err(e) => {
                tracing::warn!("Failed to initialize HuggingFace client: {}", e);
                None
            }
        };

        // Initialize model library (required - core functionality)
        let model_library = model_library::ModelLibrary::new(&model_library_dir)
            .await
            .map_err(|e| PumasError::Config {
                message: format!("Model library initialization failed: {}", e),
            })?;
        let model_library = Arc::new(model_library);
        let model_mapper = model_library::ModelMapper::new(model_library.clone(), &mapping_config_dir);
        let model_importer = model_library::ModelImporter::new(model_library.clone());

        // Wire download completion -> in-place import (metadata + indexing)
        if let Some(ref mut client) = hf_client {
            let lib = model_library.clone();
            client.set_completion_callback(std::sync::Arc::new(move |info: model_library::DownloadCompletionInfo| {
                let lib = lib.clone();
                tokio::spawn(async move {
                    // Remove stale metadata from any previous partial download
                    // so import_in_place re-scans all files now present
                    let metadata_path = info.dest_dir.join("metadata.json");
                    if metadata_path.exists() {
                        tracing::info!("Removing stale metadata before re-import: {}", metadata_path.display());
                        let _ = tokio::fs::remove_file(&metadata_path).await;
                    }

                    let importer = model_library::ModelImporter::new(lib);
                    let spec = model_library::InPlaceImportSpec {
                        model_dir: info.dest_dir,
                        official_name: info.download_request.official_name,
                        family: info.download_request.family,
                        model_type: info.download_request.model_type,
                        repo_id: Some(info.download_request.repo_id.clone()),
                        known_sha256: info.known_sha256,
                        compute_hashes: false,
                        expected_files: Some(info.filenames.clone()),
                        pipeline_tag: info.download_request.pipeline_tag,
                    };
                    match importer.import_in_place(&spec).await {
                        Ok(r) if r.success => {
                            tracing::info!("Post-download import succeeded: {:?}", r.model_path);
                        }
                        Ok(r) => {
                            tracing::warn!("Post-download import failed: {:?}", r.error);
                        }
                        Err(e) => {
                            tracing::error!("Post-download import error: {}", e);
                        }
                    }
                });
            }));
        }

        // Initialize conversion manager
        let conversion_manager = Arc::new(conversion::ConversionManager::new(
            launcher_root.clone(),
            model_library.clone(),
            Arc::new(model_library::ModelImporter::new(model_library.clone())),
        ));

        // Best-effort registry connection
        let registry = match registry::LibraryRegistry::open() {
            Ok(reg) => Some(reg),
            Err(e) => {
                tracing::warn!("Failed to open global registry (non-fatal): {}", e);
                None
            }
        };

        // Spawn non-blocking orphan scan to adopt models missing metadata
        {
            let lib_clone = model_library.clone();
            tokio::spawn(async move {
                let importer = model_library::ModelImporter::new(lib_clone);
                let result = importer.adopt_orphans(false).await;
                if result.orphans_found > 0 {
                    tracing::info!(
                        "Startup orphan scan: found={}, adopted={}, errors={}",
                        result.orphans_found,
                        result.adopted,
                        result.errors.len()
                    );
                }
            });
        }

        let primary_state = Arc::new(PrimaryState {
            _state: state,
            network_manager,
            process_manager,
            system_utils,
            model_library,
            model_mapper,
            hf_client,
            model_importer,
            conversion_manager,
            server_handle: tokio::sync::Mutex::new(None),
            registry,
        });

        // Spawn one-time recovery for incomplete sharded models
        {
            let ps = primary_state.clone();
            tokio::spawn(async move {
                let recoveries = ps.model_importer.recover_incomplete_shards();
                if recoveries.is_empty() {
                    return;
                }
                tracing::info!(
                    "Found {} incomplete sharded model(s) to recover",
                    recoveries.len()
                );
                let Some(ref client) = ps.hf_client else {
                    tracing::warn!("Cannot recover incomplete shards: HF client not available");
                    return;
                };
                for recovery in recoveries {
                    let request = model_library::DownloadRequest {
                        repo_id: recovery.repo_id.clone(),
                        family: recovery.family,
                        official_name: recovery.official_name,
                        model_type: recovery.model_type,
                        quant: None,
                        filename: None,
                        pipeline_tag: None,
                    };
                    match client.start_download(&request, &recovery.model_dir).await {
                        Ok(id) => {
                            tracing::info!(
                                "Started shard recovery download {} for repo {}",
                                id,
                                recovery.repo_id,
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to start shard recovery for {}: {}",
                                recovery.repo_id,
                                e,
                            );
                        }
                    }
                }
            });
        }

        Ok(Self {
            launcher_root,
            inner: ApiInner::Primary(primary_state),
        })
    }

    /// Discover and connect to an existing pumas-core instance, or return an error
    /// if no libraries are registered.
    ///
    /// This is the entry point for host applications that don't know the library path.
    /// It checks the global registry for a registered library and a running instance:
    /// 1. If a running instance is found (alive PID), connects as a Client.
    /// 2. If a library is registered but no instance, creates a new Primary.
    /// 3. If no libraries are registered, returns `NoLibrariesRegistered`.
    pub async fn discover() -> Result<Self> {
        let registry = registry::LibraryRegistry::open().map_err(|e| {
            tracing::warn!("Failed to open registry for discovery: {}", e);
            PumasError::NoLibrariesRegistered
        })?;

        // Clean up stale entries first
        let _ = registry.cleanup_stale();

        let library = registry
            .get_default()?
            .ok_or(PumasError::NoLibrariesRegistered)?;

        // Check for a running instance
        if let Some(instance) = registry.get_instance(&library.path)? {
            if platform::is_process_alive(instance.pid) {
                let addr = std::net::SocketAddr::from((
                    std::net::Ipv4Addr::LOCALHOST,
                    instance.port,
                ));
                match ipc::IpcClient::connect(addr, instance.pid).await {
                    Ok(_client) => {
                        tracing::info!(
                            "Connected to existing instance (PID {} on port {})",
                            instance.pid,
                            instance.port
                        );
                        return Ok(Self {
                            launcher_root: library.path.clone(),
                            inner: ApiInner::Client,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to connect to instance PID {}: {}, creating new primary",
                            instance.pid,
                            e
                        );
                        // Stale entry, clean it up
                        let _ = registry.unregister_instance(&library.path);
                    }
                }
            } else {
                // PID is dead, clean up
                let _ = registry.unregister_instance(&library.path);
            }
        }

        // No running instance found — create a new Primary
        Self::new(&library.path).await
    }

    /// Start the IPC server and register this instance in the global registry.
    ///
    /// Call this after creating a Primary instance to enable instance convergence.
    /// Best-effort: failures are logged but don't affect the API.
    pub async fn start_ipc_server(&self) -> Result<u16> {
        let state = self.try_primary()?;

        let handle = ipc::IpcServer::start(state.clone()).await?;
        let port = handle.port;

        // Register in the global registry
        if let Some(ref reg) = state.registry {
            let library_name = self
                .launcher_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("pumas-library");
            let _ = reg.register(&self.launcher_root, library_name);
            let _ = reg.register_instance(
                &self.launcher_root,
                std::process::id(),
                port,
            );
        }

        *state.server_handle.lock().await = Some(handle);
        tracing::info!("IPC server started on port {}", port);

        Ok(port)
    }

    /// Get the launcher root directory.
    pub fn launcher_root(&self) -> &PathBuf {
        &self.launcher_root
    }

    /// Get the launcher-data directory path.
    pub fn launcher_data_dir(&self) -> PathBuf {
        self.launcher_root.join("launcher-data")
    }

    /// Get the metadata directory path.
    pub fn metadata_dir(&self) -> PathBuf {
        self.launcher_data_dir().join(config::PathsConfig::METADATA_DIR_NAME)
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> PathBuf {
        self.launcher_data_dir().join(config::PathsConfig::CACHE_DIR_NAME)
    }

    /// Get the shared resources directory path.
    pub fn shared_resources_dir(&self) -> PathBuf {
        self.launcher_root.join(config::PathsConfig::SHARED_RESOURCES_DIR_NAME)
    }

    /// Get the versions directory for a specific app.
    pub fn versions_dir(&self, app_id: AppId) -> PathBuf {
        self.launcher_root.join(app_id.versions_dir_name())
    }
}

impl Drop for PumasApi {
    fn drop(&mut self) {
        if let ApiInner::Primary(ref state) = self.inner {
            // Best-effort: unregister instance from the global registry
            if let Some(ref reg) = state.registry {
                let _ = reg.unregister_instance(&self.launcher_root);
            }
            // Server handle is dropped automatically via IpcServerHandle::drop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_api_creation() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        assert_eq!(api.launcher_root(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_api_paths() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        assert!(api.launcher_data_dir().ends_with("launcher-data"));
        assert!(api.metadata_dir().ends_with("metadata"));
        assert!(api.versions_dir(AppId::ComfyUI).ends_with("comfyui-versions"));
    }

    #[tokio::test]
    async fn test_get_status() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let status = api.get_status().await.unwrap();
        assert!(status.success);
    }

    #[tokio::test]
    async fn test_get_disk_space() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let disk = api.get_disk_space().await.unwrap();
        assert!(disk.success);
        assert!(disk.total > 0);
    }
}
