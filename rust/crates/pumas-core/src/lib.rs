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
pub mod process;
pub mod registry;
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
pub use models::{
    BundleComponentManifestEntry, BundleComponentState, BundleFormat, CommitInfo,
    EmbeddedMetadataResponse, LibraryModelMetadataResponse,
};
pub use plugins::{PluginConfig, PluginLoader};
pub use process::{ProcessInfo, ProcessManager};
pub use system::{
    check_brave, check_git, check_setproctitle, GpuInfo, GpuMonitor, ProcessResources,
    ResourceTracker, SystemCheckResult, SystemResourceSnapshot, SystemUtils,
};

// Re-export builder from api module
pub use api::PumasApiBuilder;

use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::sync::Arc;

use api::PrimaryState;

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
    /// Keeps the filesystem watcher alive for the lifetime of this API.
    model_watcher: Option<model_library::ModelLibraryWatcher>,
}

/// Internal dispatch: Primary owns state, Client proxies via IPC.
enum ApiInner {
    Primary(Arc<PrimaryState>),
    Client(Arc<ipc::IpcClient>),
}

impl PumasApi {
    /// Get a reference to the primary state, or error if in client mode.
    fn try_primary(&self) -> Result<&Arc<PrimaryState>> {
        match &self.inner {
            ApiInner::Primary(state) => Ok(state),
            ApiInner::Client(_) => Err(PumasError::Other(
                "This method is only available on primary instances".to_string(),
            )),
        }
    }

    fn try_client(&self) -> Option<&Arc<ipc::IpcClient>> {
        match &self.inner {
            ApiInner::Client(client) => Some(client),
            ApiInner::Primary(_) => None,
        }
    }

    /// Get a reference to the primary state. Panics if in client mode.
    /// Use only for methods that are guaranteed primary-only.
    fn primary(&self) -> &Arc<PrimaryState> {
        match &self.inner {
            ApiInner::Primary(state) => state,
            ApiInner::Client(_) => {
                panic!("BUG: primary-only method called on client instance")
            }
        }
    }

    async fn call_client_method<T>(&self, method: &str, params: serde_json::Value) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let client = self.try_client().ok_or_else(|| {
            PumasError::Other(format!(
                "IPC method {method} requested on a primary instance"
            ))
        })?;
        let value = client.call(method, params).await?;
        serde_json::from_value(value).map_err(|err| PumasError::Json {
            message: format!("Failed to decode IPC response for {method}: {err}"),
            source: Some(err),
        })
    }

    async fn call_client_method_or_default<T>(&self, method: &str, params: serde_json::Value) -> T
    where
        T: DeserializeOwned + Default,
    {
        match self.call_client_method(method, params).await {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!("Client IPC call {} failed: {}", method, err);
                T::default()
            }
        }
    }

    fn call_client_method_blocking<T>(&self, method: &str, params: serde_json::Value) -> Result<T>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let client = self.try_client().ok_or_else(|| {
            PumasError::Other(format!(
                "Blocking IPC method {method} requested on a primary instance"
            ))
        })?;
        let value = client.call_blocking(method, params)?;
        serde_json::from_value(value).map_err(|err| PumasError::Json {
            message: format!("Failed to decode IPC response for {method}: {err}"),
            source: Some(err),
        })
    }

    fn call_client_method_blocking_or_default<T>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> T
    where
        T: DeserializeOwned + Default + Send + 'static,
    {
        match self.call_client_method_blocking(method, params) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!("Blocking client IPC call {} failed: {}", method, err);
                T::default()
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
        Self::builder(launcher_root).build().await
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
                if instance.status == registry::InstanceStatus::Claiming {
                    return Err(PumasError::PrimaryInstanceBusy {
                        library_path: instance.library_path,
                        pid: instance.pid,
                        status: instance.status.as_str().to_string(),
                    });
                }
                let addr =
                    std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, instance.port));
                match ipc::IpcClient::connect(addr, instance.pid).await {
                    Ok(client) => {
                        tracing::info!(
                            "Connected to existing instance (PID {} on port {})",
                            instance.pid,
                            instance.port
                        );
                        return Ok(Self {
                            launcher_root: library.path.clone(),
                            inner: ApiInner::Client(Arc::new(client)),
                            model_watcher: None,
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

    /// Start the IPC server and promote any pending startup claim to a ready instance row.
    ///
    /// Primary construction already calls this. Repeated calls are idempotent and
    /// return the existing port.
    pub async fn start_ipc_server(&self) -> Result<u16> {
        let state = self.try_primary()?;
        let mut server_handle = state.server_handle.lock().await;
        if let Some(existing) = server_handle.as_ref() {
            return Ok(existing.port);
        }

        let handle = ipc::IpcServer::start(state.clone()).await?;
        let port = handle.port;

        if let Some(ref reg) = state.registry {
            let library_name = self
                .launcher_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("pumas-library");
            reg.register(&self.launcher_root, library_name)?;

            let mut claim = state.instance_claim.lock().await;
            if let Some(claim) = claim.take() {
                reg.mark_instance_ready(&claim.library_path, &claim.claim_token, port)?;
            } else {
                reg.register_instance(&self.launcher_root, std::process::id(), port)?;
            }
        }

        *server_handle = Some(handle);
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
        self.launcher_data_dir()
            .join(config::PathsConfig::METADATA_DIR_NAME)
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> PathBuf {
        self.launcher_data_dir()
            .join(config::PathsConfig::CACHE_DIR_NAME)
    }

    /// Get the shared resources directory path.
    pub fn shared_resources_dir(&self) -> PathBuf {
        self.launcher_root
            .join(config::PathsConfig::SHARED_RESOURCES_DIR_NAME)
    }

    /// Get the versions directory for a specific app.
    pub fn versions_dir(&self, app_id: AppId) -> PathBuf {
        self.launcher_root.join(app_id.versions_dir_name())
    }
}

impl Drop for PumasApi {
    fn drop(&mut self) {
        let _ = self.model_watcher.take();
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
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct RegistryTestGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl RegistryTestGuard {
        fn new(root: &std::path::Path) -> Self {
            let lock = REGISTRY_TEST_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("registry test lock poisoned");
            crate::platform::paths::set_test_registry_db_path(Some(
                root.join("registry-test")
                    .join(config::RegistryConfig::DB_FILENAME),
            ));
            Self { _lock: lock }
        }
    }

    impl Drop for RegistryTestGuard {
        fn drop(&mut self) {
            crate::platform::paths::set_test_registry_db_path(None);
        }
    }

    #[tokio::test]
    async fn test_api_creation() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(temp_dir.path());
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        assert_eq!(api.launcher_root(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_api_paths() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(temp_dir.path());
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        assert!(api.launcher_data_dir().ends_with("launcher-data"));
        assert!(api.metadata_dir().ends_with("metadata"));
        assert!(api
            .versions_dir(AppId::ComfyUI)
            .ends_with("comfyui-versions"));
    }

    #[tokio::test]
    async fn test_get_status() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(temp_dir.path());
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let status = api.get_status().await.unwrap();
        assert!(status.success);
    }

    #[tokio::test]
    async fn test_get_disk_space() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(temp_dir.path());
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let disk = api.get_disk_space().await.unwrap();
        assert!(disk.success);
        assert!(disk.total > 0);
    }

    #[tokio::test]
    async fn test_new_rejects_second_primary_for_same_root() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(temp_dir.path());
        let _primary = PumasApi::new(temp_dir.path()).await.unwrap();

        let err = match PumasApi::new(temp_dir.path()).await {
            Ok(_) => panic!("expected second primary to be rejected"),
            Err(err) => err,
        };
        assert!(matches!(err, PumasError::PrimaryInstanceBusy { .. }));
    }

    #[tokio::test]
    async fn test_start_ipc_server_is_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(temp_dir.path());
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let first_port = api.start_ipc_server().await.unwrap();
        let second_port = api.start_ipc_server().await.unwrap();
        assert_eq!(first_port, second_port);
    }

    #[tokio::test]
    async fn test_discover_returns_working_client_for_basic_ipc_methods() {
        let temp_dir = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(temp_dir.path());
        let _primary = PumasApi::new(temp_dir.path()).await.unwrap();

        let client = PumasApi::discover().await.unwrap();
        assert!(!client.is_primary());

        let models = tokio::time::timeout(std::time::Duration::from_secs(10), client.list_models())
            .await
            .expect("list_models timed out")
            .unwrap();
        assert!(models.is_empty());

        let search = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.search_models("", 10, 0),
        )
        .await
        .expect("search_models timed out")
        .unwrap();
        assert!(search.models.is_empty());

        let status = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.get_library_status(),
        )
        .await
        .expect("get_library_status timed out")
        .unwrap();
        assert!(status.success);

        let processes = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.get_running_processes(),
        )
        .await
        .expect("get_running_processes timed out");
        assert!(processes.is_empty());

        let _ = client.is_online();
        let _ = client.list_conversions();

        let disk =
            tokio::time::timeout(std::time::Duration::from_secs(10), client.get_disk_space())
                .await
                .expect("get_disk_space timed out")
                .unwrap();
        assert!(disk.success);
    }
}
