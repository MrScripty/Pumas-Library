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
//! Generate bindings using the UniFFI CLI:
//!
//! ```bash
//! # Generate Python bindings
//! uniffi-bindgen generate src/pumas.udl --language python --out-dir ./bindings/python
//!
//! # Generate Kotlin bindings
//! uniffi-bindgen generate src/pumas.udl --language kotlin --out-dir ./bindings/kotlin
//! ```

// Re-export types from pumas-core that are FFI-compatible
pub use pumas_library::models::{
    // Model management types
    DownloadOption,
    DownloadStatus,
    FtsSearchModel,
    ModelData,
    ModelDownloadProgress,
    ModelFileInfo,
    ModelHashes,
    ModelImportResult,
    ModelImportSpec,
    SecurityTier,
    // Enums
    DetectedFileType,
    ImportStage,
    MatchMethod,
};

// Response types (re-exported from responses module)
pub use pumas_library::models::{
    BaseResponse,
    CommitInfo,
    DeepScanProgress,
    DeleteModelResponse,
    DiskSpaceResponse,
    FtsSearchResponse,
    ImportBatchResponse,
    LaunchResponse,
    LauncherVersionResponse,
    LibraryStatusResponse,
    LinkInfo,
    LinksForModelResponse,
    ShortcutState,
    StatusResponse,
    // Response enums
    HealthStatus,
    LinkType as ResponseLinkType,
    SandboxType,
};

pub use pumas_library::model_library::{
    // Library types (re-exported from types module)
    ConflictResolution,
    DownloadRequest,
    HfMetadataResult,
    LfsFileInfo,
    MappingActionType,
    MappingConfig,
    MappingRule,
    ModelType,
    RepoFileTree,
    SandboxInfo,
};

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
            PumasError::LaunchFailed { app, message } => FfiError::Other(format!(
                "Launch failed for {}: {}",
                app, message
            )),
            PumasError::ProcessNotRunning { app } => FfiError::Other(format!(
                "Process not running: {}",
                app
            )),
            PumasError::ModelNotFound { model_id } => FfiError::Model {
                message: format!("Model not found: {}", model_id),
            },
            PumasError::ImportFailed { message } => FfiError::Model { message },
            PumasError::DownloadFailed { url, message } => FfiError::Download {
                message: format!("{}: {}", url, message),
            },
            PumasError::DownloadCancelled => FfiError::Cancelled,
            PumasError::HashMismatch { expected, actual } => FfiError::Validation {
                message: format!("Hash mismatch: expected {}, got {}", expected, actual),
            },
            PumasError::InvalidFileType { expected, actual } => FfiError::Validation {
                message: format!("Invalid file type: expected {}, got {}", expected, actual),
            },
            PumasError::GitHubApi { message, status_code } => FfiError::Network {
                message: format!("GitHub API error ({}): {}", status_code.unwrap_or(0), message),
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
}
