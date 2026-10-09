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
//! - **Windows**: Native filesystem authority, durable publication, and desktop support
//! - **macOS**: Native filesystem authority, durable publication, and desktop support

// No shipping audio policy is enabled; retained for the qualified child owner.
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[allow(dead_code)]
pub(crate) mod audio_read_boundary;
/// A read-only capability query. Never enables Landlock or changes host policy.
/// Complete enforcement is mandatory for installed production audio.
pub(crate) fn require_audio_read_confinement() -> std::io::Result<()> {
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    {
        audio_read_boundary::AudioReadBoundary::supported_abi().map(|_| ())
    }
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "installed audio read confinement supports Linux x86_64 only",
        ))
    }
}

pub(crate) mod capability_fs;
pub mod filesystem;
#[cfg(target_os = "linux")]
pub mod linux_group;
pub mod managed_child;
pub mod paths;
pub mod permissions;
pub mod process;
#[cfg(target_os = "linux")]
pub(crate) mod runtime_listener;
// Internal lifetime composition; crash recovery still requires complete effect custody.
#[allow(dead_code)]
pub(crate) mod store_lifetime;

// Re-export commonly used items
pub use paths::{
    apps_dir, desktop_dir, platform_display_path, pumas_config_dir, registry_db_path, venv_python,
};
pub use permissions::set_executable;
pub use process::{
    configure_detached_command, find_processes_by_cmdline, is_process_alive, terminate_process,
    terminate_process_tree,
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
    cfg!(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    fn audio_read_confinement_refuses_unsupported_targets() {
        assert_eq!(
            require_audio_read_confinement().unwrap_err().kind(),
            std::io::ErrorKind::Unsupported
        );
    }

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
