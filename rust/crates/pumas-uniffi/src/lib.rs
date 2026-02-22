//! UniFFI bindings for pumas-core.
//!
//! This crate provides cross-language bindings for the pumas-core library,
//! enabling native access from Python, C#, Swift, Kotlin, Go, and Ruby.
//!
//! # Supported Languages
//!
//! - **Python** - Official UniFFI support
//! - **C#** - Via uniffi-bindgen-cs
//! - **Kotlin** - Official UniFFI support
//! - **Swift** - Official UniFFI support
//! - **Ruby** - Official UniFFI support
//! - **Go** - Via uniffi-bindgen-go
//!
//! # Usage
//!
//! Generate bindings using `--library` mode:
//!
//! ```bash
//! # Build the cdylib
//! cargo build -p pumas-uniffi --release
//!
//! # Generate Python bindings
//! pumas-uniffi-bindgen generate --library --language python \
//!     --out-dir ./bindings/python target/release/libpumas_uniffi.so
//!
//! # Generate C# bindings
//! uniffi-bindgen-cs --library --config crates/pumas-uniffi/uniffi.toml \
//!     --out-dir ./bindings/csharp target/release/libpumas_uniffi.so
//! ```

// =============================================================================
// Re-exported types from pumas-core (only types used by FfiPumasApi methods)
// =============================================================================

// Model management types used in API signatures
pub use pumas_library::models::{
    DownloadOption,
    DownloadStatus,
    ModelDownloadProgress,
    ModelImportResult,
    ModelImportSpec,
    SecurityTier,
};

// Response types used in API signatures
pub use pumas_library::models::{
    DeleteModelResponse,
    DiskSpaceResponse,
    StatusResponse,
    // System resource types (nested in SystemResourcesResponse)
    AppResources,
    AppResourceUsage,
    CpuResources,
    DiskResources,
    GpuResources,
    RamResources,
    SystemResources,
    SystemResourcesResponse,
};

// HuggingFace / model library types used in API signatures
pub use pumas_library::model_library::{
    DownloadRequest,
    HfMetadataResult,
    LfsFileInfo,
    RepoFileTree,
};

// Private imports — used by From impls but not exposed in API signatures
use pumas_library::models::HuggingFaceModel;
use pumas_library::{ModelRecord, PumasApi, SearchResult};
use std::sync::Arc;

/// FFI-friendly error type.
///
/// This is a simplified version of `PumasError` that can cross the FFI boundary.
/// Complex error types with embedded `std::io::Error` or `rusqlite::Error` are
/// converted to string representations.
#[derive(Debug, Clone, uniffi::Error, thiserror::Error)]
pub enum FfiError {
    #[error("Network error: {message}")]
    Network { message: String },

    #[error("Timeout: {message}")]
    Timeout { message: String },

    #[error("Rate limited: {message}")]
    RateLimited { message: String },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Version error: {message}")]
    Version { message: String },

    #[error("Model error: {message}")]
    Model { message: String },

    #[error("Download error: {message}")]
    Download { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Launch failed: {message}")]
    Launch { message: String },

    #[error("Process error: {message}")]
    Process { message: String },

    #[error("Cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl From<pumas_library::PumasError> for FfiError {
    fn from(err: pumas_library::PumasError) -> Self {
        use pumas_library::PumasError;

        match err {
            PumasError::Network { message, .. } => FfiError::Network { message },
            PumasError::Timeout(duration) => FfiError::Timeout {
                message: format!("Request timed out after {:?}", duration),
            },
            PumasError::RateLimited {
                service,
                retry_after_secs,
            } => FfiError::RateLimited {
                message: format!(
                    "Rate limited by {}, retry after {:?} seconds",
                    service, retry_after_secs
                ),
            },
            PumasError::CircuitBreakerOpen { domain } => FfiError::Network {
                message: format!("Circuit breaker open for {}", domain),
            },
            PumasError::Database { message, .. } => FfiError::Database { message },
            PumasError::Io { message, path, .. } => FfiError::Io {
                message: match path {
                    Some(p) => format!("{}: {}", p.display(), message),
                    None => message,
                },
            },
            PumasError::FileNotFound(path) => FfiError::NotFound {
                resource: format!("File: {}", path.display()),
            },
            PumasError::NotFound { resource } => FfiError::NotFound { resource },
            PumasError::PermissionDenied(path) => FfiError::Io {
                message: format!("Permission denied: {}", path.display()),
            },
            PumasError::NotADirectory(path) => FfiError::Io {
                message: format!("Not a directory: {}", path.display()),
            },
            PumasError::SymlinkFailed { src, dest, reason } => FfiError::Io {
                message: format!(
                    "Failed to create symlink from {} to {}: {}",
                    src.display(),
                    dest.display(),
                    reason
                ),
            },
            PumasError::Json { message, .. } => FfiError::Io {
                message: format!("JSON error: {}", message),
            },
            PumasError::VersionNotFound { tag } => FfiError::Version {
                message: format!("Version not found: {}", tag),
            },
            PumasError::VersionAlreadyInstalled { tag } => FfiError::Version {
                message: format!("Version already installed: {}", tag),
            },
            PumasError::InstallationFailed { message } => FfiError::Version { message },
            PumasError::InstallationCancelled => FfiError::Cancelled,
            PumasError::DependencyFailed { message } => FfiError::Version { message },
            PumasError::LaunchFailed { app, message } => FfiError::Launch {
                message: format!("{}: {}", app, message),
            },
            PumasError::ProcessNotRunning { app } => FfiError::Process {
                message: format!("Process not running: {}", app),
            },
            PumasError::ModelNotFound { model_id } => FfiError::Model {
                message: format!("Model not found: {}", model_id),
            },
            PumasError::ImportFailed { message } => FfiError::Model { message },
            PumasError::DownloadFailed { url, message } => FfiError::Download {
                message: format!("{}: {}", url, message),
            },
            PumasError::DownloadCancelled | PumasError::DownloadPaused => FfiError::Cancelled,
            PumasError::HashMismatch { expected, actual } => FfiError::Validation {
                message: format!("Hash mismatch: expected {}, got {}", expected, actual),
            },
            PumasError::InvalidFileType { expected, actual } => FfiError::Validation {
                message: format!("Invalid file type: expected {}, got {}", expected, actual),
            },
            PumasError::GitHubApi { message, status_code } => FfiError::Network {
                message: format!(
                    "GitHub API error ({}): {}",
                    status_code.unwrap_or(0),
                    message
                ),
            },
            PumasError::ReleaseNotFound { tag } => FfiError::NotFound {
                resource: format!("Release: {}", tag),
            },
            PumasError::Config { message } => FfiError::Config { message },
            PumasError::InvalidAppId(id) => FfiError::Validation {
                message: format!("Invalid app ID: {}", id),
            },
            PumasError::Validation { field, message } => FfiError::Validation {
                message: format!("{}: {}", field, message),
            },
            PumasError::InvalidVersionTag { tag } => FfiError::Validation {
                message: format!("Invalid version tag: {}", tag),
            },
            PumasError::InvalidParams { message } => FfiError::Validation { message },
            PumasError::SharedInstanceLost { pid, port } => FfiError::Other(
                format!("Shared instance lost (PID {} on port {})", pid, port),
            ),
            PumasError::NoLibrariesRegistered => FfiError::Config {
                message: "No libraries registered".to_string(),
            },
            PumasError::ConversionFailed { message } => FfiError::Model { message },
            PumasError::ConversionCancelled => FfiError::Cancelled,
            PumasError::Other(message) => FfiError::Other(message),
        }
    }
}

/// Result type for FFI operations.
pub type FfiResult<T> = Result<T, FfiError>;

// UniFFI scaffolding - this generates the FFI glue code
uniffi::setup_scaffolding!();

/// Get the version of the pumas-uniffi bindings.
#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// =============================================================================
// FFI Wrapper Types
//
// These wrap pumas-core types that contain FFI-incompatible fields
// (HashMap, serde_json::Value, usize). HashMaps are converted to Vec of
// key-value records; serde_json::Value is serialized to a JSON string.
// =============================================================================

/// A key-value pair for model hashes (e.g. sha256, blake3).
///
/// Used instead of `HashMap<String, String>` which can't cross the FFI boundary.
#[derive(uniffi::Record)]
pub struct FfiHashEntry {
    pub key: String,
    pub value: String,
}

/// A quantization size entry mapping quant name to file size.
///
/// Used instead of `HashMap<String, u64>` which can't cross the FFI boundary.
#[derive(uniffi::Record)]
pub struct FfiQuantSize {
    pub quant: String,
    pub size_bytes: u64,
}

/// FFI-safe wrapper for `ModelRecord`.
///
/// `ModelRecord` contains `HashMap<String, String>` (hashes) and
/// `serde_json::Value` (metadata) which can't cross the FFI boundary.
/// Hashes are converted to `Vec<FfiHashEntry>`; metadata stays as a JSON string.
#[derive(uniffi::Record)]
pub struct FfiModelRecord {
    pub id: String,
    pub path: String,
    pub cleaned_name: String,
    pub official_name: String,
    pub model_type: String,
    pub tags: Vec<String>,
    /// Model hashes as key-value pairs (e.g. key="sha256", value="abc123...")
    pub hashes: Vec<FfiHashEntry>,
    /// Full model metadata as a JSON string (arbitrary nested structure)
    pub metadata_json: String,
    pub updated_at: String,
}

impl From<ModelRecord> for FfiModelRecord {
    fn from(r: ModelRecord) -> Self {
        Self {
            id: r.id,
            path: r.path,
            cleaned_name: r.cleaned_name,
            official_name: r.official_name,
            model_type: r.model_type,
            tags: r.tags,
            hashes: r
                .hashes
                .into_iter()
                .map(|(k, v)| FfiHashEntry { key: k, value: v })
                .collect(),
            metadata_json: r.metadata.to_string(),
            updated_at: r.updated_at,
        }
    }
}

/// FFI-safe wrapper for `SearchResult`.
///
/// `SearchResult` contains `usize` fields which aren't FFI-compatible.
#[derive(uniffi::Record)]
pub struct FfiSearchResult {
    pub models: Vec<FfiModelRecord>,
    pub total_count: u64,
    pub query_time_ms: f64,
    pub query: String,
}

impl From<SearchResult> for FfiSearchResult {
    fn from(r: SearchResult) -> Self {
        Self {
            models: r.models.into_iter().map(FfiModelRecord::from).collect(),
            total_count: r.total_count as u64,
            query_time_ms: r.query_time_ms,
            query: r.query,
        }
    }
}

/// FFI-safe wrapper for `HuggingFaceModel`.
///
/// `HuggingFaceModel` contains `HashMap<String, u64>` (quant_sizes) which
/// can't cross the FFI boundary. Converted to `Vec<FfiQuantSize>`.
#[derive(uniffi::Record)]
pub struct FfiHuggingFaceModel {
    pub repo_id: String,
    pub name: String,
    pub developer: String,
    pub kind: String,
    pub formats: Vec<String>,
    pub quants: Vec<String>,
    pub download_options: Vec<DownloadOption>,
    pub url: String,
    pub release_date: Option<String>,
    pub downloads: Option<u64>,
    pub total_size_bytes: Option<u64>,
    /// Quantization sizes as quant-name/size pairs. Empty if not available.
    pub quant_sizes: Vec<FfiQuantSize>,
    pub compatible_engines: Vec<String>,
}

impl From<HuggingFaceModel> for FfiHuggingFaceModel {
    fn from(m: HuggingFaceModel) -> Self {
        Self {
            repo_id: m.repo_id,
            name: m.name,
            developer: m.developer,
            kind: m.kind,
            formats: m.formats,
            quants: m.quants,
            download_options: m.download_options,
            url: m.url,
            release_date: m.release_date,
            downloads: m.downloads,
            total_size_bytes: m.total_size_bytes,
            quant_sizes: m
                .quant_sizes
                .map(|qs| {
                    qs.into_iter()
                        .map(|(k, v)| FfiQuantSize {
                            quant: k,
                            size_bytes: v,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            compatible_engines: m.compatible_engines,
        }
    }
}

// =============================================================================
// FfiApiConfig — Configuration record for API initialization
// =============================================================================

/// Configuration for creating an `FfiPumasApi` instance.
///
/// Use this with `FfiPumasApi.with_config()` for fine-grained control
/// over initialization.
#[derive(uniffi::Record)]
pub struct FfiApiConfig {
    /// Path to the directory containing `launcher-data/`, `shared-resources/`, etc.
    pub launcher_root: String,
    /// Create required directories if they don't exist.
    pub auto_create_dirs: bool,
    /// Enable HuggingFace model search and download.
    pub enable_hf: bool,
}

// =============================================================================
// FfiPumasApi — The main API object exposed to foreign languages
// =============================================================================

/// The main Pumas Library API handle.
///
/// Create an instance with `FfiPumasApi::new()` or `FfiPumasApi::with_config()`,
/// then call methods to manage models, search HuggingFace, etc.
///
/// # Example (Python)
///
/// ```python
/// api = await FfiPumasApi.new("/path/to/launcher-root")
/// models = await api.list_models()
/// results = await api.search_hf_models("llama", None, 10)
/// ```
#[derive(uniffi::Object)]
pub struct FfiPumasApi {
    inner: Arc<PumasApi>,
}

#[uniffi::export(async_runtime = "tokio")]
impl FfiPumasApi {
    /// Create a new API instance with default options.
    ///
    /// The `launcher_root` is the path to the directory containing
    /// `launcher-data/`, `shared-resources/`, etc.
    #[uniffi::constructor]
    pub async fn new(launcher_root: String) -> Result<Arc<Self>, FfiError> {
        let api = PumasApi::new(launcher_root).await.map_err(FfiError::from)?;
        Ok(Arc::new(Self {
            inner: Arc::new(api),
        }))
    }

    /// Create a new API instance with a configuration record.
    #[uniffi::constructor]
    pub async fn with_config(config: FfiApiConfig) -> Result<Arc<Self>, FfiError> {
        let api = PumasApi::builder(config.launcher_root)
            .auto_create_dirs(config.auto_create_dirs)
            .with_hf_client(config.enable_hf)
            .with_process_manager(false)
            .build()
            .await
            .map_err(FfiError::from)?;
        Ok(Arc::new(Self {
            inner: Arc::new(api),
        }))
    }

    // ========================================
    // Model Library Methods
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<FfiModelRecord>, FfiError> {
        let models = self.inner.list_models().await.map_err(FfiError::from)?;
        Ok(models.into_iter().map(FfiModelRecord::from).collect())
    }

    /// Get a single model by its ID.
    pub async fn get_model(&self, model_id: String) -> Result<Option<FfiModelRecord>, FfiError> {
        let model = self
            .inner
            .get_model(&model_id)
            .await
            .map_err(FfiError::from)?;
        Ok(model.map(FfiModelRecord::from))
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: String,
        limit: u64,
        offset: u64,
    ) -> Result<FfiSearchResult, FfiError> {
        let result = self
            .inner
            .search_models(&query, limit as usize, offset as usize)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiSearchResult::from(result))
    }

    /// Delete a model and all its links.
    pub async fn delete_model(&self, model_id: String) -> Result<DeleteModelResponse, FfiError> {
        self.inner
            .delete_model_with_cascade(&model_id)
            .await
            .map_err(FfiError::from)
    }

    /// Import a model from a local file path.
    pub async fn import_model(
        &self,
        spec: ModelImportSpec,
    ) -> Result<ModelImportResult, FfiError> {
        self.inner
            .import_model(&spec)
            .await
            .map_err(FfiError::from)
    }

    /// Import multiple models in a batch.
    ///
    /// Returns a result for each spec. Failures are captured per-item
    /// rather than aborting the entire batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<ModelImportSpec>,
    ) -> Vec<ModelImportResult> {
        self.inner.import_models_batch(specs).await
    }

    /// Rebuild the full-text search index for all models.
    ///
    /// Returns the number of models indexed.
    pub async fn rebuild_model_index(&self) -> Result<u64, FfiError> {
        self.inner
            .rebuild_model_index()
            .await
            .map(|n| n as u64)
            .map_err(FfiError::from)
    }

    // ========================================
    // HuggingFace Methods
    // ========================================

    /// Search for models on HuggingFace.
    ///
    /// - `query`: Search query string
    /// - `kind`: Optional model type filter (e.g. "text-generation")
    /// - `limit`: Maximum number of results
    pub async fn search_hf_models(
        &self,
        query: String,
        kind: Option<String>,
        limit: u64,
    ) -> Result<Vec<FfiHuggingFaceModel>, FfiError> {
        let models = self
            .inner
            .search_hf_models(&query, kind.as_deref(), limit as usize)
            .await
            .map_err(FfiError::from)?;
        Ok(models.into_iter().map(FfiHuggingFaceModel::from).collect())
    }

    /// Start downloading a model from HuggingFace.
    ///
    /// Returns a download ID that can be used to track progress or cancel.
    pub async fn start_hf_download(&self, request: DownloadRequest) -> Result<String, FfiError> {
        self.inner
            .start_hf_download(&request)
            .await
            .map_err(FfiError::from)
    }

    /// Get the progress of an active HuggingFace download.
    ///
    /// Returns `None` if the download ID is not found.
    pub async fn get_hf_download_progress(
        &self,
        download_id: String,
    ) -> Option<ModelDownloadProgress> {
        self.inner.get_hf_download_progress(&download_id).await
    }

    /// Cancel an active HuggingFace download.
    ///
    /// Returns `true` if a download was found and cancelled.
    pub async fn cancel_hf_download(&self, download_id: String) -> Result<bool, FfiError> {
        self.inner
            .cancel_hf_download(&download_id)
            .await
            .map_err(FfiError::from)
    }

    /// Look up HuggingFace metadata for a local model file.
    ///
    /// Attempts to match a file by hash or filename against HuggingFace repos.
    /// Returns `None` if no match is found.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: String,
    ) -> Result<Option<HfMetadataResult>, FfiError> {
        self.inner
            .lookup_hf_metadata_for_file(&file_path)
            .await
            .map_err(FfiError::from)
    }

    /// Get the file tree for a HuggingFace repository.
    ///
    /// Returns LFS files (with sizes and hashes) and regular files.
    pub async fn get_hf_repo_files(&self, repo_id: String) -> Result<RepoFileTree, FfiError> {
        self.inner
            .get_hf_repo_files(&repo_id)
            .await
            .map_err(FfiError::from)
    }

    // ========================================
    // System Info Methods
    // ========================================

    /// Check if the network is currently online.
    pub fn is_online(&self) -> bool {
        self.inner.is_online()
    }

    /// Get disk space information for the launcher root.
    pub async fn get_disk_space(&self) -> Result<DiskSpaceResponse, FfiError> {
        self.inner.get_disk_space().await.map_err(FfiError::from)
    }

    /// Get overall system status including running processes and resources.
    pub async fn get_status(&self) -> Result<StatusResponse, FfiError> {
        self.inner.get_status().await.map_err(FfiError::from)
    }

    /// Get current system resource usage (CPU, GPU, RAM, disk).
    pub async fn get_system_resources(&self) -> Result<SystemResourcesResponse, FfiError> {
        self.inner
            .get_system_resources()
            .await
            .map_err(FfiError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_error_conversion() {
        let err = pumas_library::PumasError::ModelNotFound {
            model_id: "test-model".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Model { .. }));
    }

    #[test]
    fn test_ffi_error_launch_variant() {
        let err = pumas_library::PumasError::LaunchFailed {
            app: "ollama".to_string(),
            message: "port in use".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Launch { .. }));
        if let FfiError::Launch { message } = ffi_err {
            assert!(message.contains("ollama"));
            assert!(message.contains("port in use"));
        }
    }

    #[test]
    fn test_ffi_error_process_variant() {
        let err = pumas_library::PumasError::ProcessNotRunning {
            app: "comfyui".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Process { .. }));
    }

    #[test]
    fn test_ffi_model_record_conversion() {
        let mut hashes = std::collections::HashMap::new();
        hashes.insert("sha256".to_string(), "abc123".to_string());

        let record = ModelRecord {
            id: "test-id".to_string(),
            path: "/models/test".to_string(),
            cleaned_name: "test-model".to_string(),
            official_name: "Test Model".to_string(),
            model_type: "llm".to_string(),
            tags: vec!["tag1".to_string()],
            hashes,
            metadata: serde_json::json!({"key": "value"}),
            updated_at: "2025-01-01".to_string(),
        };

        let ffi_record = FfiModelRecord::from(record);
        assert_eq!(ffi_record.id, "test-id");
        assert_eq!(ffi_record.official_name, "Test Model");
        assert_eq!(ffi_record.hashes.len(), 1);
        assert_eq!(ffi_record.hashes[0].key, "sha256");
        assert_eq!(ffi_record.hashes[0].value, "abc123");
        assert!(ffi_record.metadata_json.contains("key"));
    }

    #[test]
    fn test_ffi_search_result_conversion() {
        let result = SearchResult {
            models: vec![],
            total_count: 42,
            query_time_ms: 1.5,
            query: "test".to_string(),
        };

        let ffi_result = FfiSearchResult::from(result);
        assert_eq!(ffi_result.total_count, 42);
        assert_eq!(ffi_result.query, "test");
    }

    #[test]
    fn test_ffi_huggingface_model_quant_sizes() {
        let mut quant_sizes = std::collections::HashMap::new();
        quant_sizes.insert("Q4_K_M".to_string(), 4_200_000_000u64);
        quant_sizes.insert("Q8_0".to_string(), 8_100_000_000u64);

        let model = HuggingFaceModel {
            repo_id: "test/model".to_string(),
            name: "Test".to_string(),
            developer: "dev".to_string(),
            kind: "llm".to_string(),
            formats: vec![],
            quants: vec![],
            download_options: vec![],
            url: "https://example.com".to_string(),
            release_date: None,
            downloads: None,
            total_size_bytes: None,
            quant_sizes: Some(quant_sizes),
            compatible_engines: vec![],
        };

        let ffi_model = FfiHuggingFaceModel::from(model);
        assert_eq!(ffi_model.quant_sizes.len(), 2);

        let q4 = ffi_model
            .quant_sizes
            .iter()
            .find(|qs| qs.quant == "Q4_K_M");
        assert!(q4.is_some());
        assert_eq!(q4.unwrap().size_bytes, 4_200_000_000);
    }

    #[test]
    fn test_ffi_huggingface_model_no_quant_sizes() {
        let model = HuggingFaceModel {
            repo_id: "test/model".to_string(),
            name: "Test".to_string(),
            developer: "dev".to_string(),
            kind: "llm".to_string(),
            formats: vec![],
            quants: vec![],
            download_options: vec![],
            url: "https://example.com".to_string(),
            release_date: None,
            downloads: None,
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines: vec![],
        };

        let ffi_model = FfiHuggingFaceModel::from(model);
        assert!(ffi_model.quant_sizes.is_empty());
    }
}
