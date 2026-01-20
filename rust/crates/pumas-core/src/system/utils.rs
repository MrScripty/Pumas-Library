//! System utilities for disk space, file manager, and browser operations.

use crate::error::{PumasError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, warn};

/// Disk space information.
#[derive(Debug, Clone)]
pub struct DiskSpaceInfo {
    /// Total space in bytes.
    pub total: u64,
    /// Used space in bytes.
    pub used: u64,
    /// Free space in bytes.
    pub free: u64,
    /// Usage percentage (0.0 - 100.0).
    pub percent: f32,
}

/// System utilities for the launcher.
pub struct SystemUtils {
    /// Root directory for the launcher.
    script_dir: PathBuf,
}

impl SystemUtils {
    /// Create a new SystemUtils instance.
    ///
    /// # Arguments
    ///
    /// * `script_dir` - Path to the launcher root directory
    pub fn new(script_dir: impl AsRef<Path>) -> Self {
        Self {
            script_dir: script_dir.as_ref().to_path_buf(),
        }
    }

    /// Get the script directory.
    pub fn script_dir(&self) -> &Path {
        &self.script_dir
    }

    /// Get disk space information for the launcher directory.
    ///
    /// Returns information about the disk containing the launcher root.
    pub fn get_disk_space(&self) -> Result<DiskSpaceInfo> {
        self.get_disk_space_for_path(&self.script_dir)
    }

    /// Get disk space information for a specific path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to check disk space for
    pub fn get_disk_space_for_path(&self, path: &Path) -> Result<DiskSpaceInfo> {
        use sysinfo::Disks;

        let disks = Disks::new_with_refreshed_list();
        let path_str = path.to_string_lossy();

        // Find the disk that contains this path (longest matching mount point)
        let mut best_match: Option<(&sysinfo::Disk, usize)> = None;

        for disk in disks.list() {
            let mount_point = disk.mount_point().to_string_lossy();
            if path_str.starts_with(mount_point.as_ref()) {
                let match_len = mount_point.len();
                if best_match.map_or(true, |(_, len)| match_len > len) {
                    best_match = Some((disk, match_len));
                }
            }
        }

        if let Some((disk, _)) = best_match {
            let total = disk.total_space();
            let free = disk.available_space();
            let used = total.saturating_sub(free);
            let percent = if total > 0 {
                (used as f32 / total as f32) * 100.0
            } else {
                0.0
            };

            return Ok(DiskSpaceInfo {
                total,
                used,
                free,
                percent: (percent * 10.0).round() / 10.0, // Round to 1 decimal
            });
        }

        // Fallback to first disk if no match found
        if let Some(disk) = disks.list().first() {
            let total = disk.total_space();
            let free = disk.available_space();
            let used = total.saturating_sub(free);
            let percent = if total > 0 {
                (used as f32 / total as f32) * 100.0
            } else {
                0.0
            };

            return Ok(DiskSpaceInfo {
                total,
                used,
                free,
                percent: (percent * 10.0).round() / 10.0,
            });
        }

        Err(PumasError::Other(
            "Could not determine disk space".to_string(),
        ))
    }

    /// Open a filesystem path in the user's file manager.
    ///
    /// Cross-platform support:
    /// - Linux: Uses `xdg-open`
    /// - macOS: Uses `open`
    /// - Windows: Uses `explorer`
    ///
    /// # Arguments
    ///
    /// * `path` - Path to open (absolute or relative to launcher root)
    ///
    /// # Security
    ///
    /// The path is validated to prevent directory traversal attacks.
    pub fn open_path(&self, path: &str) -> Result<()> {
        let safe_path = self.sanitize_path(path)?;

        if !safe_path.exists() {
            return Err(PumasError::NotFound {
                resource: format!("Path: {}", safe_path.display()),
            });
        }

        self.open_in_file_manager(&safe_path)
    }

    /// Open a URL in the default system browser.
    ///
    /// # Arguments
    ///
    /// * `url` - URL to open (must start with http:// or https://)
    ///
    /// # Security
    ///
    /// Only http and https URLs are allowed.
    pub fn open_url(&self, url: &str) -> Result<()> {
        // Validate URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(PumasError::Validation {
                field: "url".to_string(),
                message: "Only http/https URLs are allowed".to_string(),
            });
        }

        // Try webbrowser crate first (would need to add as dependency)
        // For now, use platform-specific commands

        #[cfg(target_os = "linux")]
        {
            self.open_url_linux(url)
        }

        #[cfg(target_os = "macos")]
        {
            self.open_url_macos(url)
        }

        #[cfg(target_os = "windows")]
        {
            self.open_url_windows(url)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(PumasError::Other(
                "Unsupported platform for opening URLs".to_string(),
            ))
        }
    }

    /// Sanitize a path to prevent directory traversal attacks.
    ///
    /// Returns an absolute path within the launcher directory.
    fn sanitize_path(&self, path: &str) -> Result<PathBuf> {
        let path = Path::new(path);

        // If path is absolute, check it's within allowed directories
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.script_dir.join(path)
        };

        // Canonicalize to resolve any .. or . components
        let canonical = resolved.canonicalize().map_err(|e| PumasError::Io {
            message: format!("canonicalize path: {}", e),
            path: Some(resolved.clone()),
            source: Some(e),
        })?;

        // Check if path is within allowed directories
        let script_dir_canonical = self
            .script_dir
            .canonicalize()
            .unwrap_or_else(|_| self.script_dir.clone());

        // Allow paths within script_dir or home directory
        let home_dir = dirs::home_dir().unwrap_or_default();

        if canonical.starts_with(&script_dir_canonical) || canonical.starts_with(&home_dir) {
            Ok(canonical)
        } else {
            warn!(
                "Rejected path outside allowed directories: {}",
                canonical.display()
            );
            Err(PumasError::Validation {
                field: "path".to_string(),
                message: "Path is outside allowed directories".to_string(),
            })
        }
    }

    /// Open a path in the file manager.
    fn open_in_file_manager(&self, path: &Path) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.open_in_file_manager_linux(path)
        }

        #[cfg(target_os = "macos")]
        {
            self.open_in_file_manager_macos(path)
        }

        #[cfg(target_os = "windows")]
        {
            self.open_in_file_manager_windows(path)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(PumasError::Other(
                "Unsupported platform for file manager".to_string(),
            ))
        }
    }

    #[cfg(target_os = "linux")]
    fn open_in_file_manager_linux(&self, path: &Path) -> Result<()> {
        // Try xdg-open first
        let result = Command::new("xdg-open").arg(path).spawn();

        match result {
            Ok(mut child) => {
                // Don't wait for the process - it should run independently
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => {
                debug!("xdg-open failed: {}", e);

                // Try common file managers as fallback
                for fm in &["nautilus", "dolphin", "thunar", "pcmanfm", "nemo"] {
                    if let Ok(mut child) = Command::new(fm).arg(path).spawn() {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                        return Ok(());
                    }
                }

                Err(PumasError::Other(format!(
                    "Failed to open file manager: {}",
                    e
                )))
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn open_in_file_manager_macos(&self, path: &Path) -> Result<()> {
        let result = Command::new("open").arg(path).spawn();

        match result {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => Err(PumasError::Other(format!(
                "Failed to open file manager: {}",
                e
            ))),
        }
    }

    #[cfg(target_os = "windows")]
    fn open_in_file_manager_windows(&self, path: &Path) -> Result<()> {
        let result = Command::new("explorer").arg(path).spawn();

        match result {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => Err(PumasError::Other(format!(
                "Failed to open file manager: {}",
                e
            ))),
        }
    }

    #[cfg(target_os = "linux")]
    fn open_url_linux(&self, url: &str) -> Result<()> {
        let result = Command::new("xdg-open").arg(url).spawn();

        match result {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => {
                debug!("xdg-open failed for URL: {}", e);
                Err(PumasError::Other(format!("Failed to open browser: {}", e)))
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn open_url_macos(&self, url: &str) -> Result<()> {
        let result = Command::new("open").arg(url).spawn();

        match result {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => Err(PumasError::Other(format!("Failed to open browser: {}", e))),
        }
    }

    #[cfg(target_os = "windows")]
    fn open_url_windows(&self, url: &str) -> Result<()> {
        let result = Command::new("cmd").args(["/c", "start", "", url]).spawn();

        match result {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                Ok(())
            }
            Err(e) => Err(PumasError::Other(format!("Failed to open browser: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_disk_space() {
        let temp_dir = TempDir::new().unwrap();
        let utils = SystemUtils::new(temp_dir.path());

        let disk_info = utils.get_disk_space().unwrap();

        assert!(disk_info.total > 0);
        assert!(disk_info.free > 0);
        assert!(disk_info.percent >= 0.0 && disk_info.percent <= 100.0);
    }

    #[test]
    fn test_sanitize_path_within_script_dir() {
        let temp_dir = TempDir::new().unwrap();
        let utils = SystemUtils::new(temp_dir.path());

        // Create a file to test
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test").unwrap();

        let result = utils.sanitize_path("test.txt");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_file.canonicalize().unwrap());
    }

    #[test]
    fn test_url_validation() {
        let temp_dir = TempDir::new().unwrap();
        let utils = SystemUtils::new(temp_dir.path());

        // Valid URLs (will fail to actually open but pass validation)
        assert!(utils.open_url("file:///etc/passwd").is_err());
        assert!(utils.open_url("javascript:alert(1)").is_err());
        assert!(utils.open_url("ftp://example.com").is_err());
    }
}
