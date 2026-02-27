//! Platform-specific file permission handling.
//!
//! This module provides cross-platform abstractions for file permissions,
//! particularly for making files executable.

use crate::error::Result;
use std::path::Path;
use tracing::debug;

/// Make a file executable.
///
/// # Platform Behavior
/// - **Linux/macOS**: Sets the executable bit (mode 0o755)
/// - **Windows**: No-op (Windows determines executability by file extension)
///
/// # Errors
/// Returns an error if the file doesn't exist or permissions can't be changed.
pub fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions)?;
        debug!("Set executable permissions on: {}", path.display());
    }

    #[cfg(windows)]
    {
        // Windows doesn't use executable bits - executability is determined by extension
        debug!("Skipping executable bit on Windows for: {}", path.display());
    }

    Ok(())
}

/// Check if a file has executable permissions.
///
/// # Platform Behavior
/// - **Linux/macOS**: Checks if any execute bit is set
/// - **Windows**: Returns true for common executable extensions (.exe, .bat, .cmd, .ps1)
pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mode = metadata.permissions().mode();
            // Check if any execute bit is set (user, group, or other)
            mode & 0o111 != 0
        } else {
            false
        }
    }

    #[cfg(windows)]
    {
        // On Windows, check file extension
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(ext_lower.as_str(), "exe" | "bat" | "cmd" | "ps1" | "com")
        } else {
            false
        }
    }
}

/// Set file permissions to be readable and writable by owner only.
///
/// # Platform Behavior
/// - **Linux/macOS**: Sets mode 0o600
/// - **Windows**: Uses standard file permissions (no special handling needed)
///
/// Useful for sensitive files like credentials or private keys.
pub fn set_private(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(path, permissions)?;
        debug!("Set private permissions (0600) on: {}", path.display());
    }

    #[cfg(windows)]
    {
        // Windows uses ACLs for fine-grained permissions
        // For now, we don't modify Windows permissions
        debug!(
            "Skipping private permission setting on Windows for: {}",
            path.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_set_executable() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_script.sh");
        File::create(&file_path).unwrap();

        // Should not panic
        set_executable(&file_path).unwrap();

        #[cfg(unix)]
        assert!(is_executable(&file_path));
    }

    #[test]
    fn test_is_executable_unix() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test_file");
            File::create(&file_path).unwrap();

            // Initially not executable
            assert!(!is_executable(&file_path));

            // Make it executable
            let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&file_path, perms).unwrap();

            assert!(is_executable(&file_path));
        }
    }

    #[test]
    fn test_is_executable_windows() {
        #[cfg(windows)]
        {
            let exe_path = Path::new("test.exe");
            let bat_path = Path::new("test.bat");
            let txt_path = Path::new("test.txt");

            assert!(is_executable(exe_path));
            assert!(is_executable(bat_path));
            assert!(!is_executable(txt_path));
        }
    }

    #[test]
    fn test_set_private() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("secret.txt");
        File::create(&file_path).unwrap();

        // Should not panic
        set_private(&file_path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
