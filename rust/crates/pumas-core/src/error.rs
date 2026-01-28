//! Error types for Pumas Library.
//!
//! This module defines comprehensive error types that map to the Python exceptions
//! and provide meaningful error messages for the frontend.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Main error type for the Pumas library.
#[derive(Debug, Error)]
pub enum PumasError {
    // Network errors
    #[error("Network error: {message}")]
    Network {
        message: String,
        /// Optional cause description
        cause: Option<String>,
    },

    #[error("Request timeout after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Rate limited by {service}, retry after {retry_after_secs:?} seconds")]
    RateLimited {
        service: String,
        retry_after_secs: Option<u64>,
    },

    #[error("Circuit breaker open for {domain}")]
    CircuitBreakerOpen { domain: String },

    // Database errors
    #[error("Database error: {message}")]
    Database {
        message: String,
        #[source]
        source: Option<rusqlite::Error>,
    },

    // File system errors
    #[error("IO error at {path:?}: {message}")]
    Io {
        message: String,
        path: Option<PathBuf>,
        #[source]
        source: Option<std::io::Error>,
    },

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Failed to create symlink from {src} to {dest}: {reason}")]
    SymlinkFailed {
        src: PathBuf,
        dest: PathBuf,
        reason: String,
    },

    // Serialization errors
    #[error("JSON error: {message}")]
    Json {
        message: String,
        #[source]
        source: Option<serde_json::Error>,
    },

    // Version management errors
    #[error("Version not found: {tag}")]
    VersionNotFound { tag: String },

    #[error("Version already installed: {tag}")]
    VersionAlreadyInstalled { tag: String },

    #[error("Version installation failed: {message}")]
    InstallationFailed { message: String },

    #[error("Installation cancelled by user")]
    InstallationCancelled,

    #[error("Dependency installation failed: {message}")]
    DependencyFailed { message: String },

    #[error("Process launch failed for {app}: {message}")]
    LaunchFailed { app: String, message: String },

    #[error("Process not running: {app}")]
    ProcessNotRunning { app: String },

    // Model library errors
    #[error("Model not found: {model_id}")]
    ModelNotFound { model_id: String },

    #[error("Model import failed: {message}")]
    ImportFailed { message: String },

    #[error("Download failed for {url}: {message}")]
    DownloadFailed { url: String, message: String },

    #[error("Download cancelled")]
    DownloadCancelled,

    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("Invalid file type: expected {expected}, got {actual}")]
    InvalidFileType { expected: String, actual: String },

    // GitHub API errors
    #[error("GitHub API error: {message}")]
    GitHubApi { message: String, status_code: Option<u16> },

    #[error("Release not found: {tag}")]
    ReleaseNotFound { tag: String },

    // Configuration errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Invalid app ID: {0}")]
    InvalidAppId(String),

    // Validation errors
    #[error("Validation error for {field}: {message}")]
    Validation { field: String, message: String },

    #[error("Invalid version tag: {tag}")]
    InvalidVersionTag { tag: String },

    #[error("Invalid parameters: {message}")]
    InvalidParams { message: String },

    // Generic errors
    #[error("{0}")]
    Other(String),
}

/// Result type alias for Pumas operations.
pub type Result<T> = std::result::Result<T, PumasError>;

// Conversion implementations for common error types

impl From<std::io::Error> for PumasError {
    fn from(err: std::io::Error) -> Self {
        PumasError::Io {
            message: err.to_string(),
            path: None,
            source: Some(err),
        }
    }
}

impl From<serde_json::Error> for PumasError {
    fn from(err: serde_json::Error) -> Self {
        PumasError::Json {
            message: err.to_string(),
            source: Some(err),
        }
    }
}

impl From<rusqlite::Error> for PumasError {
    fn from(err: rusqlite::Error) -> Self {
        PumasError::Database {
            message: err.to_string(),
            source: Some(err),
        }
    }
}

impl From<reqwest::Error> for PumasError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            PumasError::Timeout(std::time::Duration::from_secs(0))
        } else {
            PumasError::Network {
                message: err.to_string(),
                cause: Some(err.to_string()),
            }
        }
    }
}

impl PumasError {
    /// Create an IO error with path and operation context.
    pub fn io(operation: impl Into<String>, path: impl Into<PathBuf>, err: std::io::Error) -> Self {
        PumasError::Io {
            message: format!("{}: {}", operation.into(), err),
            path: Some(path.into()),
            source: Some(err),
        }
    }

    /// Create an IO error with path context.
    pub fn io_with_path(err: std::io::Error, path: impl Into<PathBuf>) -> Self {
        PumasError::Io {
            message: err.to_string(),
            path: Some(path.into()),
            source: Some(err),
        }
    }

    /// Convert to a JSON-RPC error code.
    ///
    /// Standard JSON-RPC error codes:
    /// - -32700: Parse error
    /// - -32600: Invalid Request
    /// - -32601: Method not found
    /// - -32602: Invalid params
    /// - -32603: Internal error
    ///
    /// Custom error codes (application-defined, -32000 to -32099):
    /// - -32000: Network/connectivity error
    /// - -32001: Version not found
    /// - -32002: Model not found
    /// - -32003: Installation failed
    /// - -32004: Cancelled by user
    /// - -32005: Validation error
    pub fn to_rpc_error_code(&self) -> i32 {
        match self {
            PumasError::Network { .. }
            | PumasError::Timeout(_)
            | PumasError::RateLimited { .. }
            | PumasError::CircuitBreakerOpen { .. } => -32000,

            PumasError::VersionNotFound { .. } | PumasError::ReleaseNotFound { .. } => -32001,

            PumasError::ModelNotFound { .. } => -32002,

            PumasError::InstallationFailed { .. }
            | PumasError::DependencyFailed { .. }
            | PumasError::LaunchFailed { .. }
            | PumasError::ImportFailed { .. }
            | PumasError::DownloadFailed { .. } => -32003,

            PumasError::InstallationCancelled | PumasError::DownloadCancelled => -32004,

            PumasError::Validation { .. }
            | PumasError::InvalidVersionTag { .. }
            | PumasError::InvalidAppId(_)
            | PumasError::InvalidFileType { .. }
            | PumasError::HashMismatch { .. } => -32005,

            PumasError::InvalidParams { .. } => -32602, // Standard JSON-RPC invalid params

            // All other errors are internal errors
            _ => -32603,
        }
    }

    /// Check if this error should trigger a retry.
    ///
    /// Note: RateLimited errors are NOT retryable because:
    /// 1. The API won't work until the rate limit window resets
    /// 2. Retrying immediately wastes quota and extends the rate limit
    /// 3. The caller should use cached data or wait for the retry_after period
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PumasError::Network { .. }
                | PumasError::Timeout(_)
            // RateLimited is intentionally NOT retryable - see doc comment above
        )
    }
}

/// Extension trait for `Result<T, std::io::Error>` to easily add path context.
///
/// This trait reduces the boilerplate of mapping IO errors to `PumasError::Io`
/// with path information, replacing 110+ repetitive `.map_err()` calls.
///
/// # Example
///
/// ```ignore
/// use pumas_core::error::IoResultExt;
///
/// // Before (verbose):
/// std::fs::read_to_string(&path).map_err(|e| PumasError::Io {
///     message: format!("Failed to read file: {}", e),
///     path: Some(path.clone()),
///     source: Some(e),
/// })?;
///
/// // After (concise):
/// std::fs::read_to_string(&path).with_path(&path)?;
///
/// // With operation context:
/// std::fs::read_to_string(&path).with_context("reading config", &path)?;
/// ```
pub trait IoResultExt<T> {
    /// Add path context to an IO error.
    fn with_path(self, path: impl AsRef<Path>) -> Result<T>;

    /// Add operation and path context to an IO error.
    fn with_context(self, operation: impl Into<String>, path: impl AsRef<Path>) -> Result<T>;
}

impl<T> IoResultExt<T> for std::result::Result<T, std::io::Error> {
    fn with_path(self, path: impl AsRef<Path>) -> Result<T> {
        self.map_err(|e| PumasError::io_with_path(e, path.as_ref()))
    }

    fn with_context(self, operation: impl Into<String>, path: impl AsRef<Path>) -> Result<T> {
        self.map_err(|e| PumasError::io(operation, path.as_ref(), e))
    }
}

/// Macro for creating IO error mappers inline.
///
/// This is useful when you need a closure for `.map_err()` but want
/// concise syntax.
///
/// # Example
///
/// ```ignore
/// use pumas_core::io_err;
///
/// // Create an error mapper with just path:
/// std::fs::read(&path).map_err(io_err!(&path))?;
///
/// // Create an error mapper with operation and path:
/// std::fs::write(&path, data).map_err(io_err!("writing data", &path))?;
/// ```
#[macro_export]
macro_rules! io_err {
    ($path:expr) => {
        |e: std::io::Error| $crate::error::PumasError::io_with_path(e, $path)
    };
    ($op:expr, $path:expr) => {
        |e: std::io::Error| $crate::error::PumasError::io($op, $path, e)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PumasError::VersionNotFound {
            tag: "v1.0.0".into(),
        };
        assert_eq!(err.to_string(), "Version not found: v1.0.0");
    }

    #[test]
    fn test_error_display_network() {
        let err = PumasError::Network {
            message: "Connection refused".into(),
            cause: Some("tcp error".into()),
        };
        assert_eq!(err.to_string(), "Network error: Connection refused");
    }

    #[test]
    fn test_error_display_rate_limited() {
        let err = PumasError::RateLimited {
            service: "GitHub".into(),
            retry_after_secs: Some(60),
        };
        assert!(err.to_string().contains("Rate limited"));
        assert!(err.to_string().contains("GitHub"));
    }

    #[test]
    fn test_error_display_hash_mismatch() {
        let err = PumasError::HashMismatch {
            expected: "abc123".into(),
            actual: "def456".into(),
        };
        assert!(err.to_string().contains("abc123"));
        assert!(err.to_string().contains("def456"));
    }

    #[test]
    fn test_rpc_error_codes() {
        assert_eq!(
            PumasError::VersionNotFound {
                tag: "v1.0.0".into()
            }
            .to_rpc_error_code(),
            -32001
        );
        assert_eq!(
            PumasError::InstallationCancelled.to_rpc_error_code(),
            -32004
        );
    }

    #[test]
    fn test_rpc_error_codes_all_categories() {
        // Network errors -> -32000
        assert_eq!(
            PumasError::Network {
                message: "test".into(),
                cause: None
            }
            .to_rpc_error_code(),
            -32000
        );
        assert_eq!(
            PumasError::CircuitBreakerOpen {
                domain: "api.example.com".into()
            }
            .to_rpc_error_code(),
            -32000
        );

        // Version/release not found -> -32001
        assert_eq!(
            PumasError::ReleaseNotFound { tag: "v1.0".into() }.to_rpc_error_code(),
            -32001
        );

        // Model not found -> -32002
        assert_eq!(
            PumasError::ModelNotFound {
                model_id: "model123".into()
            }
            .to_rpc_error_code(),
            -32002
        );

        // Installation/download failures -> -32003
        assert_eq!(
            PumasError::InstallationFailed {
                message: "test".into()
            }
            .to_rpc_error_code(),
            -32003
        );
        assert_eq!(
            PumasError::DownloadFailed {
                url: "http://example.com".into(),
                message: "failed".into()
            }
            .to_rpc_error_code(),
            -32003
        );

        // Cancelled -> -32004
        assert_eq!(PumasError::DownloadCancelled.to_rpc_error_code(), -32004);

        // Validation errors -> -32005
        assert_eq!(
            PumasError::Validation {
                field: "tag".into(),
                message: "invalid".into()
            }
            .to_rpc_error_code(),
            -32005
        );
        assert_eq!(
            PumasError::InvalidAppId("unknown".into()).to_rpc_error_code(),
            -32005
        );

        // Invalid params (JSON-RPC standard) -> -32602
        assert_eq!(
            PumasError::InvalidParams {
                message: "missing field".into()
            }
            .to_rpc_error_code(),
            -32602
        );

        // Internal errors -> -32603
        assert_eq!(
            PumasError::Other("unknown error".into()).to_rpc_error_code(),
            -32603
        );
    }

    #[test]
    fn test_retryable_errors() {
        assert!(PumasError::Timeout(std::time::Duration::from_secs(5)).is_retryable());
        assert!(!PumasError::VersionNotFound {
            tag: "v1.0.0".into()
        }
        .is_retryable());
    }

    #[test]
    fn test_retryable_errors_comprehensive() {
        // Retryable
        assert!(PumasError::Network {
            message: "test".into(),
            cause: None
        }
        .is_retryable());

        // Not retryable - RateLimited should NOT be retryable because:
        // 1. The API won't work until the rate limit window resets
        // 2. Retrying immediately wastes quota
        assert!(!PumasError::RateLimited {
            service: "GitHub".into(),
            retry_after_secs: Some(60)
        }
        .is_retryable());

        // Not retryable
        assert!(!PumasError::CircuitBreakerOpen {
            domain: "api.example.com".into()
        }
        .is_retryable());
        assert!(!PumasError::InstallationCancelled.is_retryable());
        assert!(!PumasError::ModelNotFound {
            model_id: "test".into()
        }
        .is_retryable());
        assert!(!PumasError::PermissionDenied(PathBuf::from("/test")).is_retryable());
    }

    #[test]
    fn test_io_error_helper() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let pumas_err = PumasError::io("reading file", "/test/path", io_err);

        assert!(pumas_err.to_string().contains("reading file"));
    }

    #[test]
    fn test_io_with_path_helper() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let pumas_err = PumasError::io_with_path(io_err, "/restricted/path");

        match pumas_err {
            PumasError::Io { path, .. } => {
                assert_eq!(path, Some(PathBuf::from("/restricted/path")));
            }
            _ => panic!("Expected Io error variant"),
        }
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let pumas_err: PumasError = io_err.into();

        match pumas_err {
            PumasError::Io { path, source, .. } => {
                assert!(path.is_none());
                assert!(source.is_some());
            }
            _ => panic!("Expected Io error variant"),
        }
    }

    #[test]
    fn test_from_json_error() {
        let json_str = "{ invalid json }";
        let json_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let pumas_err: PumasError = json_err.into();

        match pumas_err {
            PumasError::Json { message, source } => {
                assert!(!message.is_empty());
                assert!(source.is_some());
            }
            _ => panic!("Expected Json error variant"),
        }
    }
}
