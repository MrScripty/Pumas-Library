//! Platform abstraction layer for cross-platform compatibility.
//!
//! This module centralizes all platform-specific code to make it easy to find,
//! maintain, and extend. All `#[cfg]` blocks for OS-specific behavior should
//! live in this module rather than scattered throughout the codebase.
//!
//! # Architecture
//!
//! Each submodule handles a specific cross-platform concern:
//! - `paths` - Platform-specific directory and file paths
//! - `permissions` - File permission handling (executable bits, etc.)
//! - `process` - Process management (signals, termination)
//!
//! # Supported Platforms
//!
//! - **Linux**: Full support
//! - **Windows**: Full support
//! - **macOS**: Architecture ready, implementation pending

pub mod paths;
pub mod permissions;
pub mod process;

// Re-export commonly used items
pub use paths::{apps_dir, desktop_dir, venv_python};
pub use permissions::set_executable;
pub use process::{
    find_processes_by_cmdline, is_process_alive, terminate_process, terminate_process_tree,
};

/// Returns the current platform name.
pub fn current_platform() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "unknown"
    }
}

/// Returns true if the current platform is supported.
pub fn is_supported_platform() -> bool {
    cfg!(any(target_os = "linux", target_os = "windows", target_os = "macos"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_platform() {
        let platform = current_platform();
        assert!(["linux", "windows", "macos", "unknown"].contains(&platform));
    }

    #[test]
    fn test_is_supported_platform() {
        // Should be true on Linux, Windows, or macOS
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        assert!(is_supported_platform());
    }
}
