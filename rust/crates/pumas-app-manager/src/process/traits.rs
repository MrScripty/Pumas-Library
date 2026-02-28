//! App process manager trait.

use async_trait::async_trait;
use pumas_library::Result;
use std::path::PathBuf;

/// Status of a running app process.
#[derive(Debug, Clone, Default)]
pub struct ProcessStatus {
    /// Process ID if running.
    pub pid: Option<u32>,
    /// Port the app is listening on.
    pub port: Option<u16>,
    /// RAM usage in bytes.
    pub ram_bytes: Option<u64>,
    /// GPU memory usage in bytes.
    pub gpu_bytes: Option<u64>,
    /// Uptime in seconds.
    pub uptime_secs: Option<u64>,
    /// Whether the app is responding to health checks.
    pub healthy: bool,
}

/// Handle to a launched process.
#[derive(Debug)]
pub struct ProcessHandle {
    /// Whether launch was successful.
    pub success: bool,
    /// Log file path.
    pub log_file: Option<PathBuf>,
    /// Error message if launch failed.
    pub error: Option<String>,
    /// Whether the process is ready (health check passed).
    pub ready: bool,
}

/// Generic trait for managing app processes.
///
/// Provides a common interface for launching, stopping, and monitoring
/// different types of apps (Python-based, binary, Docker, etc.).
#[async_trait]
pub trait AppProcessManager: Send + Sync {
    /// Get the app ID this manager handles.
    fn app_id(&self) -> &str;

    /// Launch the app with the specified version.
    ///
    /// Returns a handle with status information.
    async fn launch(&self, version_tag: &str) -> Result<ProcessHandle>;

    /// Stop the running app.
    ///
    /// Returns true if a process was stopped.
    async fn stop(&self) -> Result<bool>;

    /// Check if the app is currently running.
    async fn is_running(&self) -> bool;

    /// Get the current process status.
    ///
    /// Returns None if not running.
    async fn get_status(&self) -> Option<ProcessStatus>;

    /// Get recent log lines.
    async fn get_logs(&self, lines: usize) -> Vec<String>;

    /// Get the path to the version directory.
    fn version_path(&self, version_tag: &str) -> PathBuf;

    /// Check if a version is installed.
    async fn is_version_installed(&self, version_tag: &str) -> bool;
}
