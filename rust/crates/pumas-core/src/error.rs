//! Error types for Pumas Library.
//!
//! This module defines comprehensive error types that map to the Python exceptions
//! and provide meaningful error messages for the frontend.

use std::path::PathBuf;
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

    #[error("Dependency installation failed: {message}")]
    DependencyInstallFailed { message: String },

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
            | PumasError::DependencyInstallFailed { .. }
            | PumasError::LaunchFailed { .. }
            | PumasError::ImportFailed { .. }
            | PumasError::DownloadFailed { .. } => -32003,

            PumasError::InstallationCancelled | PumasError::DownloadCancelled => -32004,

            PumasError::Validation { .. }
            | PumasError::InvalidVersionTag { .. }
            | PumasError::InvalidAppId(_)
            | PumasError::InvalidFileType { .. }
            | PumasError::HashMismatch { .. } => -32005,

            // All other errors are internal errors
            _ => -32603,
        }
    }

    /// Check if this error should trigger a retry.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PumasError::Network { .. }
                | PumasError::Timeout(_)
                | PumasError::RateLimited { .. }
        )
    }
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
    fn test_retryable_errors() {
        assert!(PumasError::Timeout(std::time::Duration::from_secs(5)).is_retryable());
        assert!(!PumasError::VersionNotFound {
            tag: "v1.0.0".into()
        }
        .is_retryable());
    }
}
