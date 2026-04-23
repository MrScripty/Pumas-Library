//! Platform-specific path utilities.
//!
//! This module provides functions to get platform-specific paths for:
//! - Python virtual environment executables
//! - Application menu/shortcut directories
//! - Desktop directories
//! - Icon storage locations

use crate::error::{PumasError, Result};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
static TEST_REGISTRY_DB_OVERRIDE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// Get the path to the Python executable within a virtual environment.
///
/// # Platform Behavior
/// - **Linux/macOS**: `{base}/venv/bin/python`
/// - **Windows**: `{base}/venv/Scripts/python.exe`
pub fn venv_python(base: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        base.join("venv").join("bin").join("python")
    }
    #[cfg(windows)]
    {
        base.join("venv").join("Scripts").join("python.exe")
    }
}

/// Get the path to pip within a virtual environment.
///
/// # Platform Behavior
/// - **Linux/macOS**: `{base}/venv/bin/pip`
/// - **Windows**: `{base}/venv/Scripts/pip.exe`
pub fn venv_pip(base: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        base.join("venv").join("bin").join("pip")
    }
    #[cfg(windows)]
    {
        base.join("venv").join("Scripts").join("pip.exe")
    }
}

/// Get the system applications/shortcuts directory.
///
/// # Platform Behavior
/// - **Linux**: `~/.local/share/applications` (XDG spec)
/// - **Windows**: `%APPDATA%/Microsoft/Windows/Start Menu/Programs`
/// - **macOS**: `/Applications` (stub - apps are typically .app bundles)
pub fn apps_dir() -> Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or_else(|| PumasError::Config {
            message: "Could not determine home directory".to_string(),
        })?;
        Ok(home.join(".local").join("share").join("applications"))
    }

    #[cfg(target_os = "windows")]
    {
        let data_dir = dirs::data_dir().ok_or_else(|| PumasError::Config {
            message: "Could not determine app data directory".to_string(),
        })?;
        Ok(data_dir
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs"))
    }

    #[cfg(target_os = "macos")]
    {
        // macOS apps are typically .app bundles in /Applications
        // For menu bar apps, we'd use different mechanisms
        Ok(PathBuf::from("/Applications"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(PumasError::Config {
            message: "Unsupported platform for apps directory".to_string(),
        })
    }
}

/// Get the user's desktop directory.
///
/// # Platform Behavior
/// Uses the `dirs` crate which handles platform differences:
/// - **Linux**: `~/Desktop` or XDG user dirs
/// - **Windows**: `C:\Users\{user}\Desktop`
/// - **macOS**: `~/Desktop`
pub fn desktop_dir() -> Result<PathBuf> {
    dirs::desktop_dir().ok_or_else(|| PumasError::Config {
        message: "Could not determine desktop directory".to_string(),
    })
}

/// Get the icon storage directory for the current platform.
///
/// # Platform Behavior
/// - **Linux**: `~/.local/share/icons/hicolor` (freedesktop icon theme)
/// - **Windows**: Returns the provided fallback (icons stored with app)
/// - **macOS**: Returns the provided fallback (icons embedded in .app bundles)
pub fn icon_theme_dir(fallback: &Path) -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            home.join(".local")
                .join("share")
                .join("icons")
                .join("hicolor")
        } else {
            fallback.to_path_buf()
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Windows and macOS don't use a system icon theme
        fallback.to_path_buf()
    }
}

/// Get the file extension for shortcuts on the current platform.
///
/// # Platform Behavior
/// - **Linux**: `desktop` (freedesktop .desktop files)
/// - **Windows**: `lnk` (Windows shortcut files)
/// - **macOS**: `app` (application bundles, though aliases are different)
pub fn shortcut_extension() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "desktop"
    }
    #[cfg(target_os = "windows")]
    {
        "lnk"
    }
    #[cfg(target_os = "macos")]
    {
        "app"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "shortcut"
    }
}

/// Get the file extension for launch scripts on the current platform.
///
/// # Platform Behavior
/// - **Linux/macOS**: `sh` (shell scripts)
/// - **Windows**: `ps1` (PowerShell scripts)
pub fn script_extension() -> &'static str {
    #[cfg(unix)]
    {
        "sh"
    }
    #[cfg(windows)]
    {
        "ps1"
    }
}

/// Render a filesystem path for user-facing display.
///
/// # Platform Behavior
/// - **Windows**: Strips verbatim prefixes and expands short names when Windows can resolve them
/// - **Linux/macOS**: Uses the path's native display form
pub fn platform_display_path(path: &Path) -> String {
    #[cfg(windows)]
    {
        normalize_windows_display_path(path).display().to_string()
    }

    #[cfg(not(windows))]
    {
        path.display().to_string()
    }
}

#[cfg(windows)]
fn normalize_windows_display_path(path: &Path) -> PathBuf {
    expand_windows_long_path(&strip_windows_verbatim_prefix(path))
        .unwrap_or_else(|| strip_windows_verbatim_prefix(path))
}

#[cfg(windows)]
fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    let raw = path.display().to_string();
    if let Some(stripped) = raw.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", stripped))
    } else if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

#[cfg(windows)]
fn expand_windows_long_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;

    let input: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `input` is null-terminated and lives for the duration of the
    // call. Passing a null output pointer with size 0 is the documented query
    // form for retrieving the required buffer length.
    let required = unsafe { GetLongPathNameW(input.as_ptr(), std::ptr::null_mut(), 0) };
    if required == 0 {
        return None;
    }

    let mut buffer = vec![0u16; required as usize + 1];
    // SAFETY: `buffer` is allocated with the size requested by Windows plus
    // space for the trailing null. Both input and output buffers remain valid
    // for the duration of the call.
    let written =
        unsafe { GetLongPathNameW(input.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
    if written == 0 {
        return None;
    }

    buffer.truncate(written as usize);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

/// Check if a command exists in the system PATH.
///
/// # Platform Behavior
/// - **Linux/macOS**: Uses `which` command
/// - **Windows**: Uses `where` command
pub fn command_exists(cmd: &str) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Get the Pumas global configuration directory.
///
/// This is the well-known location for cross-process shared state
/// like the library registry database.
///
/// # Platform Behavior
/// - **Linux**: `~/.config/pumas` (XDG_CONFIG_HOME)
/// - **Windows**: `%APPDATA%\pumas`
/// - **macOS**: `~/Library/Application Support/pumas`
pub fn pumas_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().ok_or_else(|| PumasError::Config {
        message: "Could not determine platform config directory".to_string(),
    })?;
    Ok(config_dir.join(crate::config::RegistryConfig::APP_CONFIG_DIR_NAME))
}

/// Get the path to the global library registry database.
///
/// Returns `{pumas_config_dir}/registry.db`.
pub fn registry_db_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("PUMAS_REGISTRY_DB_PATH") {
        return Ok(PathBuf::from(path));
    }

    #[cfg(test)]
    if let Some(path) = TEST_REGISTRY_DB_OVERRIDE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("test registry override lock poisoned")
        .clone()
    {
        return Ok(path);
    }

    Ok(pumas_config_dir()?.join(crate::config::RegistryConfig::DB_FILENAME))
}

#[cfg(test)]
pub fn set_test_registry_db_path(path: Option<PathBuf>) {
    *TEST_REGISTRY_DB_OVERRIDE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("test registry override lock poisoned") = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_venv_python_path() {
        let base = PathBuf::from("/test/version");
        let python = venv_python(&base);

        #[cfg(unix)]
        assert!(python.to_string_lossy().contains("bin/python"));

        #[cfg(windows)]
        assert!(python.to_string_lossy().contains("Scripts\\python.exe"));
    }

    #[test]
    fn test_shortcut_extension() {
        let ext = shortcut_extension();

        #[cfg(target_os = "linux")]
        assert_eq!(ext, "desktop");

        #[cfg(target_os = "windows")]
        assert_eq!(ext, "lnk");

        #[cfg(target_os = "macos")]
        assert_eq!(ext, "app");
    }

    #[test]
    fn test_script_extension() {
        let ext = script_extension();

        #[cfg(unix)]
        assert_eq!(ext, "sh");

        #[cfg(windows)]
        assert_eq!(ext, "ps1");
    }

    #[test]
    fn test_apps_dir() {
        // Should not panic on supported platforms
        let result = apps_dir();

        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        assert!(result.is_ok());
    }

    #[test]
    fn test_desktop_dir() {
        // Should work on most systems with a desktop environment
        let result = desktop_dir();
        // May fail in headless environments, so just check it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_pumas_config_dir_contains_pumas() {
        let dir = pumas_config_dir().unwrap();
        assert!(
            dir.to_string_lossy().contains("pumas"),
            "Config dir should contain 'pumas': {:?}",
            dir
        );
    }

    #[test]
    fn test_registry_db_path_ends_with_db() {
        let path = registry_db_path().unwrap();
        assert!(
            path.to_string_lossy().ends_with("registry.db"),
            "Registry path should end with registry.db: {:?}",
            path
        );
    }

    #[test]
    fn test_registry_db_path_respects_env_override() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let override_path = temp_dir.path().join("custom-registry.db");

        // SAFETY: Unit tests mutate this process-local environment variable
        // within a single test scope and restore it before returning.
        unsafe {
            std::env::set_var("PUMAS_REGISTRY_DB_PATH", &override_path);
        }

        let resolved = registry_db_path().unwrap();
        assert_eq!(resolved, override_path);

        // SAFETY: Paired restoration for the scoped override above.
        unsafe {
            std::env::remove_var("PUMAS_REGISTRY_DB_PATH");
        }
    }
}
