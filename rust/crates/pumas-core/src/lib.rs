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
    BatchImportProgress, DownloadRequest, HfSearchParams, HuggingFaceClient, ModelImporter,
    ModelLibrary, ModelMapper,
};
pub use models::CommitInfo;
pub use plugins::{PluginConfig, PluginLoader};
pub use process::{ProcessInfo, ProcessManager};
pub use system::{
    check_brave, check_git, check_setproctitle, GpuInfo, GpuMonitor, ProcessResources,
    ResourceTracker, SystemCheckResult, SystemResourceSnapshot, SystemUtils,
};

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    Client {
        client: ipc::IpcClient,
    },
}

/// All state owned by a primary instance.
///
/// This is the full set of subsystems that were previously fields on `PumasApi`.
/// Wrapped in `Arc` so it can be shared with the IPC server dispatch.
pub(crate) struct PrimaryState {
    _state: Arc<RwLock<ApiState>>,
    network_manager: Arc<network::NetworkManager>,
    process_manager: Arc<RwLock<Option<process::ProcessManager>>>,
    system_utils: Arc<system::SystemUtils>,
    model_library: Arc<model_library::ModelLibrary>,
    model_mapper: model_library::ModelMapper,
    hf_client: Option<model_library::HuggingFaceClient>,
    model_importer: model_library::ModelImporter,
    conversion_manager: Arc<conversion::ConversionManager>,
    /// IPC server handle (Primary only). Protected by async Mutex for shutdown.
    server_handle: tokio::sync::Mutex<Option<ipc::IpcServerHandle>>,
    /// Global registry connection (best-effort, None if unavailable).
    registry: Option<registry::LibraryRegistry>,
}

/// IPC dispatch implementation for the primary state.
///
/// Routes incoming JSON-RPC method calls to the appropriate PrimaryState methods.
/// Each method deserializes params, calls the real implementation, and serializes the result.
#[async_trait::async_trait]
impl ipc::server::IpcDispatch for PrimaryState {
    async fn dispatch(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, PumasError> {
        match method {
            "list_models" => {
                let models = self.model_library.list_models().await?;
                Ok(serde_json::to_value(models)?)
            }
            "search_models" => {
                let query = params["query"].as_str().unwrap_or("");
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;
                let offset = params["offset"].as_u64().unwrap_or(0) as usize;
                let result = self.model_library.search_models(query, limit, offset).await?;
                Ok(serde_json::to_value(result)?)
            }
            "get_model" => {
                let model_id = params["model_id"]
                    .as_str()
                    .ok_or_else(|| PumasError::InvalidParams {
                        message: "model_id is required".to_string(),
                    })?;
                let model = self.model_library.get_model(model_id).await?;
                Ok(serde_json::to_value(model)?)
            }
            "get_status" => {
                Ok(serde_json::json!({
                    "success": true,
                    "version": env!("CARGO_PKG_VERSION"),
                    "is_primary": true,
                }))
            }
            "ping" => Ok(serde_json::json!("pong")),
            // Conversion methods
            "start_conversion" => {
                let request: conversion::ConversionRequest =
                    serde_json::from_value(params).map_err(|e| PumasError::InvalidParams {
                        message: format!("Invalid conversion request: {e}"),
                    })?;
                let id = self.conversion_manager.start_conversion(request).await?;
                Ok(serde_json::json!({ "conversion_id": id }))
            }
            "get_conversion_progress" => {
                let id = params["conversion_id"]
                    .as_str()
                    .ok_or_else(|| PumasError::InvalidParams {
                        message: "conversion_id is required".to_string(),
                    })?;
                let progress = self.conversion_manager.get_progress(id);
                Ok(serde_json::to_value(progress)?)
            }
            "cancel_conversion" => {
                let id = params["conversion_id"]
                    .as_str()
                    .ok_or_else(|| PumasError::InvalidParams {
                        message: "conversion_id is required".to_string(),
                    })?;
                let cancelled = self.conversion_manager.cancel_conversion(id).await?;
                Ok(serde_json::json!({ "cancelled": cancelled }))
            }
            "list_conversions" => {
                let conversions = self.conversion_manager.list_conversions();
                Ok(serde_json::to_value(conversions)?)
            }
            "is_conversion_environment_ready" => {
                let ready = self.conversion_manager.is_environment_ready();
                Ok(serde_json::json!({ "ready": ready }))
            }
            "ensure_conversion_environment" => {
                self.conversion_manager.ensure_environment().await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "supported_quant_types" => {
                let types = self.conversion_manager.supported_quant_types();
                Ok(serde_json::to_value(types)?)
            }
            _ => Err(PumasError::InvalidParams {
                message: format!("Unknown IPC method: {}", method),
            }),
        }
    }
}

/// Internal state for the API.
struct ApiState {
    /// Whether background fetch has completed
    background_fetch_completed: bool,
}

/// Builder for configuring PumasApi initialization.
///
/// Use this for more control over API initialization options.
///
/// # Example
///
/// ```rust,ignore
/// use pumas_library::PumasApi;
///
/// let api = PumasApi::builder("./my-models")
///     .auto_create_dirs(true)
///     .with_hf_client(false)
///     .build()
///     .await?;
/// ```
pub struct PumasApiBuilder {
    launcher_root: PathBuf,
    auto_create_dirs: bool,
    enable_hf_client: bool,
    enable_process_manager: bool,
}

impl PumasApiBuilder {
    /// Create a new builder with the launcher root directory.
    pub fn new(launcher_root: impl Into<PathBuf>) -> Self {
        Self {
            launcher_root: launcher_root.into(),
            auto_create_dirs: false,
            enable_hf_client: true,
            enable_process_manager: true,
        }
    }

    /// Auto-create required directories if they don't exist.
    ///
    /// When enabled, the builder will create the following directories:
    /// - `launcher-data/`
    /// - `launcher-data/metadata/`
    /// - `launcher-data/cache/`
    /// - `launcher-data/mapping-configs/`
    /// - `shared-resources/models/`
    ///
    /// Default: `false` (directories must exist)
    pub fn auto_create_dirs(mut self, enable: bool) -> Self {
        self.auto_create_dirs = enable;
        self
    }

    /// Enable or disable HuggingFace client initialization.
    ///
    /// When disabled, HuggingFace search and download features will not be available.
    ///
    /// Default: `true`
    pub fn with_hf_client(mut self, enable: bool) -> Self {
        self.enable_hf_client = enable;
        self
    }

    /// Enable or disable process manager initialization.
    ///
    /// When disabled, ComfyUI process management features will not be available.
    ///
    /// Default: `true`
    pub fn with_process_manager(mut self, enable: bool) -> Self {
        self.enable_process_manager = enable;
        self
    }

    /// Create the required directory structure.
    fn create_directory_structure(launcher_root: &PathBuf) -> Result<()> {
        use std::fs;

        let dirs = [
            launcher_root.join("launcher-data"),
            launcher_root.join("launcher-data").join("metadata"),
            launcher_root.join("launcher-data").join("cache"),
            launcher_root.join("launcher-data").join("cache").join("hf"),
            launcher_root.join("launcher-data").join("mapping-configs"),
            launcher_root.join("launcher-data").join("logs"),
            launcher_root.join("shared-resources"),
            launcher_root.join("shared-resources").join("models"),
        ];

        for dir in &dirs {
            if !dir.exists() {
                fs::create_dir_all(dir).map_err(|e| PumasError::Io {
                    message: format!("Failed to create directory: {}", dir.display()),
                    path: Some(dir.clone()),
                    source: Some(e),
                })?;
            }
        }

        Ok(())
    }

    /// Build the PumasApi instance.
    pub async fn build(self) -> Result<PumasApi> {
        // Auto-create directories if requested
        if self.auto_create_dirs {
            // Create launcher_root if it doesn't exist
            if !self.launcher_root.exists() {
                std::fs::create_dir_all(&self.launcher_root).map_err(|e| PumasError::Io {
                    message: format!("Failed to create launcher root: {}", self.launcher_root.display()),
                    path: Some(self.launcher_root.clone()),
                    source: Some(e),
                })?;
            }
            Self::create_directory_structure(&self.launcher_root)?;
        } else {
            // Ensure the launcher root exists
            if !self.launcher_root.exists() {
                return Err(PumasError::Config {
                    message: format!("Launcher root does not exist: {}", self.launcher_root.display()),
                });
            }
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

        // Initialize process manager (if enabled)
        let process_manager = if self.enable_process_manager {
            match process::ProcessManager::new(&self.launcher_root, None) {
                Ok(mgr) => Arc::new(RwLock::new(Some(mgr))),
                Err(e) => {
                    tracing::warn!("Failed to initialize process manager: {}", e);
                    Arc::new(RwLock::new(None))
                }
            }
        } else {
            Arc::new(RwLock::new(None))
        };

        // Initialize system utilities
        let system_utils = Arc::new(system::SystemUtils::new(&self.launcher_root));

        // Initialize model library for AI model management
        let model_library_dir = self.launcher_root
            .join("shared-resources")
            .join("models");
        let mapping_config_dir = self.launcher_root
            .join("launcher-data")
            .join("mapping-configs");

        // Initialize HuggingFace client (if enabled)
        let mut hf_client = if self.enable_hf_client {
            let cache_dir = self.launcher_root
                .join("launcher-data")
                .join(config::PathsConfig::CACHE_DIR_NAME);
            let hf_cache_dir = cache_dir.join("hf");

            // Initialize SQLite search cache at shared-resources/cache/search.sqlite
            let search_cache_dir = self.launcher_root.join("shared-resources").join("cache");
            let search_cache_db = search_cache_dir.join("search.sqlite");
            let search_cache = match model_library::HfSearchCache::new(&search_cache_db) {
                Ok(cache) => Some(std::sync::Arc::new(cache)),
                Err(e) => {
                    tracing::warn!("Failed to initialize HuggingFace search cache: {}", e);
                    None
                }
            };

            // Initialize download persistence
            let data_dir = self.launcher_root.join("launcher-data");
            let download_persistence = std::sync::Arc::new(
                model_library::DownloadPersistence::new(&data_dir)
            );

            match model_library::HuggingFaceClient::new(&hf_cache_dir) {
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
            }
        } else {
            None
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
            self.launcher_root.clone(),
            model_library.clone(),
            Arc::new(model_library::ModelImporter::new(model_library.clone())),
        ));

        // Best-effort registry connection (failures don't block initialization)
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
                        quant: None,     // Download all files for this repo
                        filename: None,
                    };
                    // start_download skips files already on disk, so it will
                    // only download the missing shards
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

        Ok(PumasApi {
            launcher_root: self.launcher_root,
            inner: ApiInner::Primary(primary_state),
        })
    }
}

impl PumasApi {
    /// Get a reference to the primary state, or error if in client mode.
    fn try_primary(&self) -> Result<&Arc<PrimaryState>> {
        match &self.inner {
            ApiInner::Primary(state) => Ok(state),
            ApiInner::Client { .. } => Err(PumasError::Other(
                "This method is only available on primary instances".to_string(),
            )),
        }
    }

    /// Get a reference to the primary state. Panics if in client mode.
    /// Use only for methods that are guaranteed primary-only.
    fn primary(&self) -> &Arc<PrimaryState> {
        match &self.inner {
            ApiInner::Primary(state) => state,
            ApiInner::Client { .. } => {
                panic!("BUG: primary-only method called on client instance")
            }
        }
    }

    /// Returns true if this instance is the primary (owns full state).
    pub fn is_primary(&self) -> bool {
        matches!(&self.inner, ApiInner::Primary(_))
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
                    Ok(client) => {
                        tracing::info!(
                            "Connected to existing instance (PID {} on port {})",
                            instance.pid,
                            instance.port
                        );
                        return Ok(Self {
                            launcher_root: library.path.clone(),
                            inner: ApiInner::Client { client },
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

    // ========================================
    // Network Connectivity
    // ========================================

    /// Check if network is currently online.
    pub fn is_online(&self) -> bool {
        self.primary().network_manager.is_online()
    }

    /// Get current network connectivity state.
    pub fn connectivity_state(&self) -> network::ConnectivityState {
        self.primary().network_manager.connectivity()
    }

    /// Check network connectivity (performs actual probe).
    pub async fn check_connectivity(&self) -> network::ConnectivityState {
        self.primary().network_manager.check_connectivity().await
    }

    /// Get detailed network status including circuit breaker states.
    pub async fn network_status(&self) -> network::NetworkStatus {
        self.primary().network_manager.status().await
    }

    /// Get the network manager for advanced operations.
    pub fn network_manager(&self) -> &Arc<network::NetworkManager> {
        &self.primary().network_manager
    }

    /// Get the model library for direct access.
    pub fn model_library(&self) -> &Arc<model_library::ModelLibrary> {
        &self.primary().model_library
    }

    // ========================================
    // Status & System Methods (stubs for now)
    // ========================================

    /// Get overall system status.
    ///
    /// Note: This returns basic status. Version-specific status (shortcuts, active version)
    /// should be obtained through pumas-app-manager in the RPC layer.
    pub async fn get_status(&self) -> Result<models::StatusResponse> {
        // Get actual running status
        let comfyui_running = self.is_comfyui_running().await;
        let ollama_running = self.is_ollama_running().await;
        let last_launch_error = self.get_last_launch_error().await;
        let last_launch_log = self.get_last_launch_log().await;

        // Get app resources for running apps
        let app_resources = {
            let mgr_lock = self.primary().process_manager.read().await;
            if let Some(ref mgr) = *mgr_lock {
                let comfyui_resources = if comfyui_running {
                    mgr.aggregate_app_resources().map(|r| models::AppResourceUsage {
                        // Convert from GB (f32) to bytes (u64) for frontend
                        gpu_memory: Some((r.gpu_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                        ram_memory: Some((r.ram_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                    })
                } else {
                    None
                };

                let ollama_resources = if ollama_running {
                    mgr.aggregate_ollama_resources().map(|r| models::AppResourceUsage {
                        gpu_memory: Some((r.gpu_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                        ram_memory: Some((r.ram_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                    })
                } else {
                    None
                };

                if comfyui_resources.is_some() || ollama_resources.is_some() {
                    Some(models::AppResources {
                        comfyui: comfyui_resources,
                        ollama: ollama_resources,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Debug: log app_resources before returning
        if let Some(ref res) = app_resources {
            tracing::debug!("get_status: app_resources = comfyui={:?}, ollama={:?}",
                  res.comfyui.as_ref().map(|r| (r.ram_memory, r.gpu_memory)),
                  res.ollama.as_ref().map(|r| (r.ram_memory, r.gpu_memory)));
        } else {
            tracing::debug!("get_status: app_resources = None");
        }

        Ok(models::StatusResponse {
            success: true,
            error: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            deps_ready: true,
            patched: false,
            menu_shortcut: false,
            desktop_shortcut: false,
            shortcut_version: None,
            message: if comfyui_running {
                "ComfyUI running".to_string()
            } else if ollama_running {
                "Ollama running".to_string()
            } else {
                "Ready".to_string()
            },
            comfyui_running,
            ollama_running,
            last_launch_error,
            last_launch_log,
            app_resources,
        })
    }

    /// Get disk space information.
    pub async fn get_disk_space(&self) -> Result<models::DiskSpaceResponse> {
        use sysinfo::Disks;

        let disks = Disks::new_with_refreshed_list();

        // Find the disk containing the launcher root
        let launcher_root_str = self.launcher_root.to_string_lossy();

        for disk in disks.list() {
            let mount_point = disk.mount_point().to_string_lossy();
            if launcher_root_str.starts_with(mount_point.as_ref()) {
                let total = disk.total_space();
                let free = disk.available_space();
                let used = total.saturating_sub(free);
                let percent = if total > 0 {
                    (used as f32 / total as f32) * 100.0
                } else {
                    0.0
                };

                return Ok(models::DiskSpaceResponse {
                    success: true,
                    error: None,
                    total,
                    used,
                    free,
                    percent,
                });
            }
        }

        // Fallback: use first disk
        if let Some(disk) = disks.list().first() {
            let total = disk.total_space();
            let free = disk.available_space();
            let used = total.saturating_sub(free);
            let percent = if total > 0 {
                (used as f32 / total as f32) * 100.0
            } else {
                0.0
            };

            return Ok(models::DiskSpaceResponse {
                success: true,
                error: None,
                total,
                used,
                free,
                percent,
            });
        }

        Err(PumasError::Other("Could not determine disk space".into()))
    }

    /// Get system resources (CPU, GPU, RAM, disk).
    pub async fn get_system_resources(&self) -> Result<models::SystemResourcesResponse> {
        use sysinfo::{System, Disks};

        let mut sys = System::new_all();
        sys.refresh_all();

        // CPU
        let cpu_usage = sys.global_cpu_usage();

        // RAM
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let ram_usage = if total_memory > 0 {
            (used_memory as f32 / total_memory as f32) * 100.0
        } else {
            0.0
        };

        // Disk
        let disks = Disks::new_with_refreshed_list();
        let (disk_total, disk_free) = if let Some(disk) = disks.list().first() {
            (disk.total_space(), disk.available_space())
        } else {
            (0, 0)
        };
        let disk_usage = if disk_total > 0 {
            ((disk_total - disk_free) as f32 / disk_total as f32) * 100.0
        } else {
            0.0
        };

        // GPU - use ResourceTracker's NvidiaSmiMonitor for real GPU stats
        let gpu = if let Some(ref mgr) = *self.primary().process_manager.read().await {
            let tracker = mgr.resource_tracker();
            match tracker.get_system_resources() {
                Ok(snapshot) => models::GpuResources {
                    usage: snapshot.gpu_usage,
                    memory: snapshot.gpu_memory_used,
                    memory_total: snapshot.gpu_memory_total,
                    temp: snapshot.gpu_temp,
                },
                Err(_) => models::GpuResources {
                    usage: 0.0,
                    memory: 0,
                    memory_total: 0,
                    temp: None,
                },
            }
        } else {
            models::GpuResources {
                usage: 0.0,
                memory: 0,
                memory_total: 0,
                temp: None,
            }
        };

        Ok(models::SystemResourcesResponse {
            success: true,
            error: None,
            resources: models::SystemResources {
                cpu: models::CpuResources {
                    usage: cpu_usage,
                    temp: None,
                },
                gpu,
                ram: models::RamResources {
                    usage: ram_usage,
                    total: total_memory,
                },
                disk: models::DiskResources {
                    usage: disk_usage,
                    total: disk_total,
                    free: disk_free,
                },
            },
        })
    }

    // ========================================
    // Process Management Methods
    // ========================================

    /// Check if ComfyUI is currently running.
    pub async fn is_comfyui_running(&self) -> bool {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_running()
        } else {
            false
        }
    }

    /// Get running processes with resource information.
    pub async fn get_running_processes(&self) -> Vec<process::ProcessInfo> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.get_processes_with_resources()
        } else {
            vec![]
        }
    }

    /// Update the version paths for process detection.
    ///
    /// This should be called by the RPC layer after obtaining version information
    /// from the VersionManager. Without this, PID file detection will only check
    /// the root-level PID file and may miss version-specific PID files.
    pub async fn set_process_version_paths(&self, version_paths: std::collections::HashMap<String, PathBuf>) {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.set_version_paths(version_paths);
        } else {
            tracing::warn!("PumasApi.set_process_version_paths: process manager not initialized");
        }
    }

    /// Stop all running ComfyUI processes.
    pub async fn stop_comfyui(&self) -> Result<bool> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.stop_all()
        } else {
            Ok(false)
        }
    }

    /// Check if Ollama is currently running.
    pub async fn is_ollama_running(&self) -> bool {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_ollama_running()
        } else {
            false
        }
    }

    /// Stop Ollama processes.
    pub async fn stop_ollama(&self) -> Result<bool> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.stop_ollama()
        } else {
            Ok(false)
        }
    }

    /// Launch an Ollama version from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_ollama(&self, tag: &str, version_dir: &std::path::Path) -> Result<models::LaunchResponse> {
        if !version_dir.exists() {
            return Ok(models::LaunchResponse {
                success: false,
                error: Some(format!("Version directory does not exist: {}", version_dir.display())),
                log_path: None,
                ready: None,
            });
        }

        let proc_mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref pm) = *proc_mgr_lock {
            let log_dir = self.launcher_data_dir().join("logs");
            let result = pm.launch_ollama(tag, version_dir, Some(&log_dir));

            Ok(models::LaunchResponse {
                success: result.success,
                error: result.error,
                log_path: result.log_path.map(|p| p.to_string_lossy().to_string()),
                ready: Some(result.ready),
            })
        } else {
            Ok(models::LaunchResponse {
                success: false,
                error: Some("Process manager not initialized".to_string()),
                log_path: None,
                ready: None,
            })
        }
    }

    /// Launch a specific version from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_version(&self, tag: &str, version_dir: &std::path::Path) -> Result<models::LaunchResponse> {
        if !version_dir.exists() {
            return Ok(models::LaunchResponse {
                success: false,
                error: Some(format!("Version directory does not exist: {}", version_dir.display())),
                log_path: None,
                ready: None,
            });
        }

        let proc_mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref pm) = *proc_mgr_lock {
            let log_dir = self.launcher_data_dir().join("logs");
            let result = pm.launch_version(tag, version_dir, Some(&log_dir));

            Ok(models::LaunchResponse {
                success: result.success,
                error: result.error,
                log_path: result.log_path.map(|p| p.to_string_lossy().to_string()),
                ready: Some(result.ready),
            })
        } else {
            Ok(models::LaunchResponse {
                success: false,
                error: Some("Process manager not initialized".to_string()),
                log_path: None,
                ready: None,
            })
        }
    }

    /// Get the last launch log path.
    pub async fn get_last_launch_log(&self) -> Option<String> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.last_launch_log().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        }
    }

    /// Get the last launch error.
    pub async fn get_last_launch_error(&self) -> Option<String> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.last_launch_error()
        } else {
            None
        }
    }

    // ========================================
    // System Utility Methods
    // ========================================

    /// Open a path in the file manager.
    pub fn open_path(&self, path: &str) -> Result<()> {
        self.primary().system_utils.open_path(path)
    }

    /// Open a URL in the default browser.
    pub fn open_url(&self, url: &str) -> Result<()> {
        self.primary().system_utils.open_url(url)
    }

    /// Open a directory in the file manager.
    ///
    /// The caller (RPC layer) can use this with a version directory path
    /// obtained from pumas-app-manager's VersionManager.
    pub fn open_directory(&self, dir: &std::path::Path) -> Result<()> {
        if !dir.exists() {
            return Err(PumasError::NotFound {
                resource: format!("Directory: {}", dir.display()),
            });
        }
        self.primary().system_utils.open_path(&dir.to_string_lossy())
    }

    // ========================================
    // Background fetch tracking
    // ========================================

    /// Check if background fetch has completed.
    pub async fn has_background_fetch_completed(&self) -> bool {
        self.primary()._state.read().await.background_fetch_completed
    }

    /// Reset the background fetch flag.
    pub async fn reset_background_fetch_flag(&self) {
        self.primary()._state.write().await.background_fetch_completed = false;
    }

    // ========================================
    // Model Library Methods
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<ModelRecord>> {
        self.primary().model_library.list_models().await
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResult> {
        self.primary().model_library.search_models(query, limit, offset).await
    }

    /// Rebuild the model index from metadata files.
    pub async fn rebuild_model_index(&self) -> Result<usize> {
        self.primary().model_library.rebuild_index().await
    }

    /// Get a single model by ID.
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        self.primary().model_library.get_model(model_id).await
    }

    /// Mark a model's metadata as manually set (protected from auto-updates).
    pub async fn mark_model_metadata_as_manual(&self, model_id: &str) -> Result<()> {
        self.primary().model_library.mark_metadata_as_manual(model_id).await
    }

    /// Import a model from a local path.
    pub async fn import_model(
        &self,
        spec: &model_library::ModelImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        self.primary().model_importer.import(spec).await
    }

    /// Import multiple models in batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<model_library::ModelImportSpec>,
    ) -> Vec<model_library::ModelImportResult> {
        self.primary().model_importer.batch_import(specs, None).await
    }

    /// Import a model in-place (files already in library directory).
    ///
    /// Creates `metadata.json` and indexes without copying. Idempotent.
    pub async fn import_model_in_place(
        &self,
        spec: &model_library::InPlaceImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        self.primary().model_importer.import_in_place(spec).await
    }

    /// Scan for and adopt orphan model directories.
    ///
    /// Finds directories in the library with model files but no `metadata.json`,
    /// creates metadata from directory structure and file type detection, and
    /// indexes the models.
    pub async fn adopt_orphan_models(&self) -> Result<model_library::OrphanScanResult> {
        Ok(self.primary().model_importer.adopt_orphans(false).await)
    }

    /// Search for models on HuggingFace.
    ///
    /// Uses intelligent caching to minimize API calls:
    /// - Cached results are returned immediately if fresh (< 24 hours)
    /// - Model details including download sizes are enriched from cache
    /// - Falls back to API when cache is stale or missing
    pub async fn search_hf_models(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<models::HuggingFaceModel>> {
        if let Some(ref client) = self.primary().hf_client {
            let params = model_library::HfSearchParams {
                query: query.to_string(),
                kind: kind.map(String::from),
                limit: Some(limit),
                ..Default::default()
            };
            // search() handles caching transparently
            client.search(&params).await
        } else {
            Ok(vec![])
        }
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(
        &self,
        request: &model_library::DownloadRequest,
    ) -> Result<String> {
        if let Some(ref client) = self.primary().hf_client {
            // Determine destination directory
            let model_type = request.model_type.as_deref().unwrap_or("unknown");
            let dest_dir = self.primary().model_library.build_model_path(
                model_type,
                &request.family,
                &model_library::normalize_name(&request.official_name),
            );
            client.start_download(request, &dest_dir).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Get download progress for a HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: &str,
    ) -> Option<models::ModelDownloadProgress> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_download_progress(download_id).await
        } else {
            None
        }
    }

    /// Cancel a HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: &str) -> Result<bool> {
        if let Some(ref client) = self.primary().hf_client {
            client.cancel_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// Pause a HuggingFace download, preserving the `.part` file for later resume.
    pub async fn pause_hf_download(&self, download_id: &str) -> Result<bool> {
        if let Some(ref client) = self.primary().hf_client {
            client.pause_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// Resume a paused or errored HuggingFace download.
    pub async fn resume_hf_download(&self, download_id: &str) -> Result<bool> {
        if let Some(ref client) = self.primary().hf_client {
            client.resume_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// List all HuggingFace downloads (active, paused, completed, etc.).
    pub async fn list_hf_downloads(&self) -> Vec<models::ModelDownloadProgress> {
        if let Some(ref client) = self.primary().hf_client {
            client.list_downloads().await
        } else {
            vec![]
        }
    }

    /// Look up HuggingFace metadata for a local file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: &str,
    ) -> Result<Option<model_library::HfMetadataResult>> {
        if let Some(ref client) = self.primary().hf_client {
            let path = std::path::Path::new(file_path);
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);
            client.lookup_metadata(filename, Some(path), None).await
        } else {
            Ok(None)
        }
    }

    /// Get repository file tree from HuggingFace.
    pub async fn get_hf_repo_files(
        &self,
        repo_id: &str,
    ) -> Result<model_library::RepoFileTree> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_repo_files(repo_id).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    // ========================================
    // Link Management
    // ========================================

    /// Get the health status of model links for a version.
    ///
    /// Returns information about total links, healthy links, broken links, etc.
    pub async fn get_link_health(&self, _version_tag: Option<&str>) -> Result<models::LinkHealthResponse> {
        let registry = self.primary().model_library.link_registry().read().await;
        let all_links = registry.get_all().await;

        let mut healthy = 0;
        let mut broken: Vec<String> = Vec::new();

        for link in &all_links {
            // Check if symlink target exists
            if link.target.is_symlink() {
                if link.source.exists() {
                    healthy += 1;
                } else {
                    broken.push(link.target.to_string_lossy().to_string());
                }
            } else if link.target.exists() {
                // Hardlink or copy - just check if target exists
                healthy += 1;
            } else {
                broken.push(link.target.to_string_lossy().to_string());
            }
        }

        Ok(models::LinkHealthResponse {
            success: true,
            error: None,
            status: if broken.is_empty() { "healthy".to_string() } else { "degraded".to_string() },
            total_links: all_links.len(),
            healthy_links: healthy,
            broken_links: broken,
            orphaned_links: vec![],
            warnings: vec![],
            errors: vec![],
        })
    }

    /// Clean up broken model links.
    ///
    /// Returns the number of broken links that were removed.
    pub async fn clean_broken_links(&self) -> Result<models::CleanBrokenLinksResponse> {
        let registry = self.primary().model_library.link_registry().write().await;
        let broken = registry.cleanup_broken().await?;

        // Also remove the actual broken symlinks from the filesystem
        for entry in &broken {
            if entry.target.exists() || entry.target.is_symlink() {
                let _ = std::fs::remove_file(&entry.target);
            }
        }

        Ok(models::CleanBrokenLinksResponse {
            success: true,
            cleaned: broken.len(),
        })
    }

    /// Get all links for a specific model.
    pub async fn get_links_for_model(&self, model_id: &str) -> Result<models::LinksForModelResponse> {
        let registry = self.primary().model_library.link_registry().read().await;
        let links = registry.get_links_for_model(model_id).await;

        let link_info: Vec<models::LinkInfo> = links
            .into_iter()
            .map(|l| models::LinkInfo {
                source: l.source.to_string_lossy().to_string(),
                target: l.target.to_string_lossy().to_string(),
                link_type: format!("{:?}", l.link_type).to_lowercase(),
                app_id: l.app_id,
                app_version: l.app_version,
                created_at: l.created_at,
            })
            .collect();

        Ok(models::LinksForModelResponse {
            success: true,
            links: link_info,
        })
    }

    /// Delete a model and cascade delete all its links.
    pub async fn delete_model_with_cascade(&self, model_id: &str) -> Result<models::DeleteModelResponse> {
        self.primary().model_library.delete_model(model_id, true).await?;
        Ok(models::DeleteModelResponse {
            success: true,
            error: None,
        })
    }

    /// Preview model mapping for a version without applying it.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path,
    /// typically obtained as `version_dir.join("models")` from pumas-app-manager.
    pub async fn preview_model_mapping(
        &self,
        version_tag: &str,
        models_path: &std::path::Path,
    ) -> Result<models::MappingPreviewResponse> {
        if !models_path.exists() {
            return Ok(models::MappingPreviewResponse {
                success: false,
                error: Some(format!("Version models directory not found: {}", models_path.display())),
                preview: None,
            });
        }

        let preview = self.primary().model_mapper.preview_mapping("comfyui", Some(version_tag), models_path).await?;

        Ok(models::MappingPreviewResponse {
            success: true,
            error: None,
            preview: Some(models::MappingPreviewData {
                creates: preview.creates.len(),
                skips: preview.skips.len(),
                conflicts: preview.conflicts.len(),
                broken: preview.broken.len(),
            }),
        })
    }

    /// Apply model mapping for a version.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path,
    /// typically obtained as `version_dir.join("models")` from pumas-app-manager.
    pub async fn apply_model_mapping(
        &self,
        version_tag: &str,
        models_path: &std::path::Path,
    ) -> Result<models::MappingApplyResponse> {
        if !models_path.exists() {
            std::fs::create_dir_all(models_path)?;
        }

        let result = self.primary().model_mapper.apply_mapping("comfyui", Some(version_tag), models_path).await?;

        Ok(models::MappingApplyResponse {
            success: true,
            error: None,
            created: result.created,
            updated: 0,
            errors: result.errors.iter().map(|(p, e)| format!("{}: {}", p.display(), e)).collect(),
        })
    }

    /// Perform incremental sync of models for a version.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path.
    pub async fn sync_models_incremental(
        &self,
        version_tag: &str,
        models_path: &std::path::Path,
    ) -> Result<models::SyncModelsResponse> {
        // Incremental sync is essentially the same as apply_mapping
        // but we could add additional logic here for detecting changes
        let result = self.apply_model_mapping(version_tag, models_path).await?;

        Ok(models::SyncModelsResponse {
            success: result.success,
            error: result.error,
            synced: result.created,
            errors: result.errors,
        })
    }

    // ========================================
    // Launcher Updater Methods
    // ========================================

    /// Get launcher version information.
    pub fn get_launcher_version(&self) -> serde_json::Value {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.get_version_info()
    }

    /// Check for launcher updates via GitHub.
    pub async fn check_launcher_updates(&self, force_refresh: bool) -> launcher::UpdateCheckResult {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.check_for_updates(force_refresh).await
    }

    /// Apply launcher update by pulling latest changes and rebuilding.
    pub async fn apply_launcher_update(&self) -> launcher::UpdateApplyResult {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.apply_update().await
    }

    /// Restart the launcher by spawning a new process.
    pub fn restart_launcher(&self) -> Result<bool> {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.restart_launcher()
    }

    // ========================================
    // Patch Manager Methods
    // ========================================

    /// Check if ComfyUI main.py is patched with setproctitle.
    pub fn is_patched(&self, tag: Option<&str>) -> bool {
        let comfyui_dir = self.launcher_root.join("ComfyUI");
        let main_py = comfyui_dir.join("main.py");
        let versions_dir = Some(self.versions_dir(AppId::ComfyUI));

        let patch_mgr = launcher::PatchManager::new(&comfyui_dir, &main_py, versions_dir);
        patch_mgr.is_patched(tag)
    }

    /// Toggle the setproctitle patch for a ComfyUI version.
    ///
    /// Returns `true` if now patched, `false` if now unpatched.
    pub fn toggle_patch(&self, tag: Option<&str>) -> Result<bool> {
        let comfyui_dir = self.launcher_root.join("ComfyUI");
        let main_py = comfyui_dir.join("main.py");
        let versions_dir = Some(self.versions_dir(AppId::ComfyUI));

        let patch_mgr = launcher::PatchManager::new(&comfyui_dir, &main_py, versions_dir);
        patch_mgr.toggle_patch(tag)
    }

    // ========================================
    // Model Format Conversion Methods
    // ========================================

    /// Start a model format conversion (GGUF <-> Safetensors).
    ///
    /// Returns a conversion ID for tracking progress.
    pub async fn start_conversion(
        &self,
        request: conversion::ConversionRequest,
    ) -> Result<String> {
        self.primary().conversion_manager.start_conversion(request).await
    }

    /// Get progress for a specific conversion.
    pub fn get_conversion_progress(
        &self,
        conversion_id: &str,
    ) -> Option<conversion::ConversionProgress> {
        self.primary().conversion_manager.get_progress(conversion_id)
    }

    /// Cancel a running conversion.
    pub async fn cancel_conversion(&self, conversion_id: &str) -> Result<bool> {
        self.primary().conversion_manager.cancel_conversion(conversion_id).await
    }

    /// List all tracked conversions (active and recently completed).
    pub fn list_conversions(&self) -> Vec<conversion::ConversionProgress> {
        self.primary().conversion_manager.list_conversions()
    }

    /// Check if the Python conversion environment is ready.
    pub fn is_conversion_environment_ready(&self) -> bool {
        self.primary().conversion_manager.is_environment_ready()
    }

    /// Ensure the Python conversion environment is set up.
    pub async fn ensure_conversion_environment(&self) -> Result<()> {
        self.primary().conversion_manager.ensure_environment().await
    }

    /// Get the list of supported quantization types for conversion.
    pub fn supported_quant_types(&self) -> Vec<conversion::QuantOption> {
        self.primary().conversion_manager.supported_quant_types()
    }

    // ========================================
    // System Check Methods
    // ========================================

    /// Check if git is available on the system.
    pub fn check_git(&self) -> system::SystemCheckResult {
        system::check_git()
    }

    /// Check if Brave browser is available on the system.
    pub fn check_brave(&self) -> system::SystemCheckResult {
        system::check_brave()
    }

    /// Check if setproctitle Python package is available.
    pub fn check_setproctitle(&self) -> system::SystemCheckResult {
        system::check_setproctitle()
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
