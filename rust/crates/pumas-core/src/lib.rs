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
pub mod onnx_runtime;
pub mod platform;
pub mod plugins;
pub mod process;
pub mod providers;
pub mod registry;
pub mod runtime_profiles;
pub mod serving;
pub mod system;

mod api;

// Re-export commonly used types
pub use cache::{CacheBackend, CacheConfig, CacheEntry, CacheMeta, CacheStats, SqliteCache};
pub use cancel::{CancellationToken, CancelledError};
pub use config::AppId;
pub use error::{PumasError, Result};
pub use index::{ModelIndex, ModelRecord, SearchResult};
pub use ipc::PumasLocalClient;
pub use launcher::{LauncherUpdater, PatchManager, UpdateApplyResult, UpdateCheckResult};
pub use metadata::MetadataManager;
pub use model_library::sharding::{self, ShardValidation};
pub use model_library::{
    BatchImportProgress, DownloadRequest, HfAuthStatus, HfSearchParams, HuggingFaceClient,
    ModelImporter, ModelLibrary, ModelMapper, PumasReadOnlyLibrary,
};
pub use models::{
    BundleComponentManifestEntry, BundleComponentState, BundleFormat, CommitInfo,
    EmbeddedMetadataResponse, LibraryModelMetadataResponse,
};
pub use onnx_runtime::{
    FakeOnnxEmbeddingBackend, OnnxEmbedding, OnnxEmbeddingBackend, OnnxEmbeddingPooling,
    OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor, OnnxEmbeddingRequest,
    OnnxEmbeddingResponse, OnnxEmbeddingUsage, OnnxExecutionProvider, OnnxLoadOptions,
    OnnxLoadRequest, OnnxModelId, OnnxModelPath, OnnxOutputTensorSelection, OnnxRuntimeError,
    OnnxRuntimeErrorCode, OnnxRuntimeSession, OnnxSessionManager, OnnxSessionState,
    OnnxSessionStatus, OnnxTokenizedBatch, OnnxTokenizedInput, OnnxTokenizer,
};
pub use plugins::{PluginConfig, PluginLoader};
pub use process::{ProcessInfo, ProcessManager};
pub use providers::{
    ExecutableArtifactFormat, OpenAiGatewayEndpoint, ProviderBehavior, ProviderBinaryLaunchTarget,
    ProviderGatewayAliasPolicy, ProviderInProcessRuntimeTarget, ProviderLaunchKind,
    ProviderManagedLaunchStrategy, ProviderManagedLaunchTarget, ProviderModelIdPolicy,
    ProviderRegistry, ProviderServingAdapterKind, ProviderServingPlacementPolicy,
    ProviderUnloadBehavior, ServingTask,
};
pub use runtime_profiles::{
    OllamaRuntimeProviderAdapter, RuntimeProviderAdapter, RuntimeProviderCapabilities,
};
pub use system::{
    check_brave, check_git, check_setproctitle, GpuInfo, GpuMonitor, ProcessResources,
    ResourceTracker, SystemCheckResult, SystemResourceSnapshot, SystemUtils,
};

// Re-export builder from api module
pub use api::PumasApiBuilder;

use std::path::PathBuf;
use std::sync::Arc;

use api::{PrimaryState, RuntimeTasks};

/// Main owning-instance API struct for Pumas operations.
///
/// This is the primary entry point for programmatic access to Pumas functionality.
/// It provides model library and system utilities for integrating Pumas functionality
/// into other applications.
///
/// `PumasApi` owns a Pumas Library instance. Same-device processes that want to
/// attach to an existing owner should use `PumasLocalClient` explicitly.
pub struct PumasApi {
    /// Root directory for launcher data (available in both modes)
    launcher_root: PathBuf,
    /// Internal mode dispatch
    inner: ApiInner,
    /// Keeps the filesystem watcher alive for the lifetime of this API.
    model_watcher: Option<model_library::ModelLibraryWatcher>,
    /// Owns primary background task handles for shutdown.
    runtime_tasks: RuntimeTasks,
}

/// Explicit name for an owning Pumas Library instance.
pub type PumasLibraryInstance = PumasApi;

/// Internal dispatch. `PumasApi` owns primary state; same-device client access
/// is exposed through `PumasLocalClient`.
enum ApiInner {
    Primary(Arc<PrimaryState>),
}

impl PumasApi {
    /// Get a reference to the primary state, or error if in client mode.
    fn try_primary(&self) -> Result<&Arc<PrimaryState>> {
        let ApiInner::Primary(state) = &self.inner;
        Ok(state)
    }

    /// Get a reference to the primary state. Panics if in client mode.
    /// Use only for methods that are guaranteed primary-only.
    fn primary(&self) -> &Arc<PrimaryState> {
        let ApiInner::Primary(state) = &self.inner;
        state
    }

    /// Returns true if this instance is the primary (owns full state).
    pub fn is_primary(&self) -> bool {
        true
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

    /// Create a PumasApi instance for the given launcher root.
    ///
    /// # Arguments
    ///
    /// * `launcher_root` - Path to the launcher root directory (containing launcher-data, etc.)
    ///
    /// If another process already owns the launcher root, this returns an
    /// error. Use `PumasLocalClient` for explicit same-device client access.
    pub async fn new(launcher_root: impl Into<PathBuf>) -> Result<Self> {
        Self::builder(launcher_root).build().await
    }

    /// Discover and connect to an existing pumas-core instance, or return an error
    /// if no libraries are registered.
    ///
    /// Open the default registered library as an owning instance.
    ///
    /// Host applications that need to attach to an already-running owner should
    /// call `PumasLocalClient::discover_ready_instances` and then
    /// `PumasLocalClient::connect` explicitly.
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
        self.runtime_tasks.shutdown();
        let _ = self.model_watcher.take();
        let ApiInner::Primary(ref state) = self.inner;
        // Best-effort: unregister instance from the global registry
        if let Some(ref reg) = state.registry {
            let _ = reg.unregister_instance(&self.launcher_root);
        }
        // Server handle is dropped automatically via IpcServerHandle::drop
    }
}

#[cfg(test)]
mod tests;
