//! System utilities for disk space, file manager, and browser operations.

use crate::error::{PumasError, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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
        // Try to open as a standalone webapp using Chromium-based browsers
        // These browsers support --app mode which opens without browser chrome
        let chromium_browsers = [
            "brave-browser",
            "brave",
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
            "microsoft-edge",
        ];

        // Create unique profile directory in /tmp (matches working shell script)
        // Using /tmp ensures complete isolation from any existing Brave profiles
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let profile_dir = std::env::temp_dir().join(format!("comfyui-profile-{}", timestamp));
        if let Err(e) = std::fs::create_dir_all(&profile_dir) {
            warn!("Failed to create profile directory: {}", e);
        }
        let user_data_dir = format!("--user-data-dir={}", profile_dir.display());

        for browser in &chromium_browsers {
            if command_exists(browser) {
                let app_url = format!("--app={}", url);
                debug!("Opening {} with {} in app mode (profile: {})", url, browser, profile_dir.display());

                // Launch as standalone app window - matches working shell script exactly
                match Command::new(browser)
                    .arg(&app_url)
                    .arg("--new-window")
                    .arg(&user_data_dir)
                    .arg("--class=ComfyUI-App")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        debug!("{} failed to launch: {}", browser, e);
                        continue;
                    }
                }
            }
        }

        // Fallback to xdg-open if no Chromium browser found
        debug!("No Chromium-based browser found, falling back to xdg-open");
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
        // Try to open as a standalone webapp using Chromium-based browsers
        let chromium_apps = [
            "Brave Browser",
            "Google Chrome",
            "Chromium",
            "Microsoft Edge",
        ];

        // Create profile directory for standalone app mode
        let profile_dir = self.script_dir.join("launcher-data").join("profiles").join("comfyui-app");
        if let Err(e) = std::fs::create_dir_all(&profile_dir) {
            warn!("Failed to create profile directory: {}", e);
        }
        let app_url = format!("--app={}", url);
        let user_data_dir = format!("--user-data-dir={}", profile_dir.display());

        for browser in &chromium_apps {
            // Check if the browser app exists
            let app_path = format!("/Applications/{}.app", browser);
            if std::path::Path::new(&app_path).exists() {
                debug!("Opening {} with {} in app mode", url, browser);

                // Use -n to open a new instance, --new-window for standalone app
                match Command::new("open")
                    .args(["-a", browser, "-n", "--args", "--new-window", &user_data_dir, &app_url])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        debug!("{} failed to launch: {}", browser, e);
                        continue;
                    }
                }
            }
        }

        // Fallback to default browser
        debug!("No Chromium-based browser found, falling back to default browser");
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
        // Try to open as a standalone webapp using Chromium-based browsers
        let chromium_browsers = [
            (
                r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
                "Brave",
            ),
            (
                r"C:\Program Files (x86)\BraveSoftware\Brave-Browser\Application\brave.exe",
                "Brave",
            ),
            (
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                "Chrome",
            ),
            (
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                "Chrome",
            ),
            (
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                "Edge",
            ),
        ];

        // Create profile directory for standalone app mode
        let profile_dir = self.script_dir.join("launcher-data").join("profiles").join("comfyui-app");
        if let Err(e) = std::fs::create_dir_all(&profile_dir) {
            warn!("Failed to create profile directory: {}", e);
        }
        let app_url = format!("--app={}", url);
        let user_data_dir = format!("--user-data-dir={}", profile_dir.display());

        for (browser_path, name) in &chromium_browsers {
            if std::path::Path::new(browser_path).exists() {
                debug!("Opening {} with {} in app mode", url, name);

                match Command::new(browser_path)
                    .arg(&app_url)
                    .arg("--new-window")
                    .arg("--new-instance")
                    .arg(&user_data_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(mut child) => {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        debug!("{} failed to launch: {}", name, e);
                        continue;
                    }
                }
            }
        }

        // Fallback to default browser
        debug!("No Chromium-based browser found, falling back to default browser");
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

// ============================================================================
// System Binary Detection
// ============================================================================

/// Check if a command exists in PATH.
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Result of a system check.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemCheckResult {
    /// Whether the check passed.
    pub available: bool,
    /// Path to the binary if found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Additional info about the check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<String>,
}

/// Check if git is available on the system.
pub fn check_git() -> SystemCheckResult {
    let available = command_exists("git");
    let path = if available {
        Command::new("which")
            .arg("git")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    } else {
        None
    };

    let info = if available {
        Command::new("git")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    } else {
        None
    };

    SystemCheckResult {
        available,
        path,
        info,
    }
}

/// Check if Brave browser is available on the system.
pub fn check_brave() -> SystemCheckResult {
    // Check common Brave binary names
    let brave_names = ["brave", "brave-browser", "brave-browser-stable"];

    for name in &brave_names {
        if command_exists(name) {
            let path = Command::new("which")
                .arg(name)
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string());

            return SystemCheckResult {
                available: true,
                path,
                info: Some(format!("Found as: {}", name)),
            };
        }
    }

    // Also check common installation paths on Linux
    #[cfg(target_os = "linux")]
    {
        let brave_paths = [
            "/usr/bin/brave-browser",
            "/usr/bin/brave",
            "/opt/brave.com/brave/brave",
            "/snap/bin/brave",
        ];

        for path in &brave_paths {
            if std::path::Path::new(path).exists() {
                return SystemCheckResult {
                    available: true,
                    path: Some(path.to_string()),
                    info: Some("Found at known path".to_string()),
                };
            }
        }
    }

    SystemCheckResult {
        available: false,
        path: None,
        info: None,
    }
}

/// Check if setproctitle Python package is available.
pub fn check_setproctitle() -> SystemCheckResult {
    // Try to import setproctitle in Python
    let result = Command::new("python3")
        .args(["-c", "import setproctitle; print(setproctitle.__file__)"])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string());

            SystemCheckResult {
                available: true,
                path,
                info: Some("Python package available".to_string()),
            }
        }
        _ => {
            // Try with python instead of python3
            let result = Command::new("python")
                .args(["-c", "import setproctitle; print(setproctitle.__file__)"])
                .output();

            match result {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string());

                    SystemCheckResult {
                        available: true,
                        path,
                        info: Some("Python package available".to_string()),
                    }
                }
                _ => SystemCheckResult {
                    available: false,
                    path: None,
                    info: Some("setproctitle package not installed".to_string()),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_check_git() {
        let result = check_git();
        // Git should generally be available in development environments
        // but we don't want to fail the test if it's not
        if result.available {
            assert!(result.path.is_some());
            assert!(result.info.is_some());
        }
    }

    #[test]
    fn test_check_brave() {
        let result = check_brave();
        // Brave may or may not be installed
        // Just verify the function runs without error
        assert!(result.available == result.path.is_some() || !result.available);
    }

    #[test]
    fn test_check_setproctitle() {
        let result = check_setproctitle();
        // setproctitle may or may not be installed
        // Just verify the function runs without error
        assert!(result.info.is_some() || result.available);
    }

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
