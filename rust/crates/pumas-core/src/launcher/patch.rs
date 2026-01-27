//! Patch manager for ComfyUI process naming.
//!
//! Manages patching of ComfyUI's main.py with setproctitle for process identification.

use crate::error::{PumasError, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Manages main.py patching for ComfyUI versions.
pub struct PatchManager {
    /// Path to ComfyUI root directory (legacy single installation).
    comfyui_dir: PathBuf,
    /// Path to main.py (legacy single installation).
    main_py: PathBuf,
    /// Path to versions directory for multi-version support.
    versions_dir: Option<PathBuf>,
}

impl PatchManager {
    /// Create a new PatchManager.
    ///
    /// # Arguments
    ///
    /// * `comfyui_dir` - Path to ComfyUI root directory
    /// * `main_py` - Path to main.py (legacy single installation)
    /// * `versions_dir` - Optional path to versions directory for multi-version support
    pub fn new(
        comfyui_dir: impl AsRef<Path>,
        main_py: impl AsRef<Path>,
        versions_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            comfyui_dir: comfyui_dir.as_ref().to_path_buf(),
            main_py: main_py.as_ref().to_path_buf(),
            versions_dir,
        }
    }

    /// Build the server title for process naming.
    ///
    /// # Arguments
    ///
    /// * `tag` - Optional version tag (e.g., v0.2.0)
    fn build_server_title(&self, tag: Option<&str>) -> String {
        match tag {
            Some(t) => format!("ComfyUI Server - {}", t),
            None => "ComfyUI Server".to_string(),
        }
    }

    /// Get the target main.py path for a given version tag.
    ///
    /// # Arguments
    ///
    /// * `tag` - Optional version tag; if None, uses legacy main.py path
    fn get_target_main_py(&self, tag: Option<&str>) -> Option<PathBuf> {
        if let Some(t) = tag {
            if let Some(ref versions_dir) = self.versions_dir {
                let main_py = versions_dir.join(t).join("main.py");
                if main_py.exists() {
                    return Some(main_py);
                }
                debug!("main.py not found for version {} at {:?}", t, main_py);
                return None;
            }
        }

        // Fallback to legacy single installation
        if self.main_py.exists() {
            return Some(self.main_py.clone());
        }

        debug!("No main.py found at {:?}", self.main_py);
        None
    }

    /// Check if main.py is patched with setproctitle.
    ///
    /// # Arguments
    ///
    /// * `tag` - Optional version tag to check
    pub fn is_patched(&self, tag: Option<&str>) -> bool {
        let main_py = match self.get_target_main_py(tag) {
            Some(p) => p,
            None => return false,
        };

        self.check_patched(&main_py, None)
    }

    /// Check if a specific main.py is patched.
    ///
    /// # Arguments
    ///
    /// * `main_py` - Path to main.py
    /// * `expected_title` - Optional exact title to look for
    fn check_patched(&self, main_py: &Path, expected_title: Option<&str>) -> bool {
        let content = match std::fs::read_to_string(main_py) {
            Ok(c) => c,
            Err(e) => {
                debug!("Error reading {:?} to check patch state: {}", main_py, e);
                return false;
            }
        };

        if let Some(title) = expected_title {
            // Check for exact title
            content.contains(&format!("setproctitle.setproctitle(\"{}\")", title))
                || content.contains(&format!("setproctitle.setproctitle('{}')", title))
        } else {
            // Check for any ComfyUI Server setproctitle call
            let pattern = Regex::new(r#"setproctitle\.setproctitle\(["']ComfyUI Server[^"']*["']\)"#)
                .unwrap();
            pattern.is_match(&content)
        }
    }

    /// Toggle the patch state for a version.
    ///
    /// If patched, removes the patch. If not patched, applies it.
    ///
    /// # Arguments
    ///
    /// * `tag` - Optional version tag to patch
    ///
    /// # Returns
    ///
    /// `true` if now patched, `false` if now unpatched
    pub fn toggle_patch(&self, tag: Option<&str>) -> Result<bool> {
        let main_py = match self.get_target_main_py(tag) {
            Some(p) => p,
            None => {
                return Err(PumasError::NotFound {
                    resource: format!("main.py for version {:?}", tag),
                });
            }
        };

        if self.check_patched(&main_py, None) {
            // Currently patched - revert
            self.revert_patch(&main_py, tag)?;
            Ok(false)
        } else {
            // Not patched - apply
            self.apply_patch(&main_py, tag)?;
            Ok(true)
        }
    }

    /// Apply the setproctitle patch to main.py.
    ///
    /// # Arguments
    ///
    /// * `main_py` - Path to main.py
    /// * `tag` - Optional version tag for the process title
    fn apply_patch(&self, main_py: &Path, tag: Option<&str>) -> Result<()> {
        let server_title = self.build_server_title(tag);
        let expected_line = format!("setproctitle.setproctitle(\"{}\")", server_title);

        let content = std::fs::read_to_string(main_py).map_err(|e| PumasError::Io {
            message: format!("Failed to read main.py: {}", e),
            path: Some(main_py.to_path_buf()),
            source: Some(e),
        })?;

        // Already patched with correct title
        if content.contains(&expected_line) {
            debug!("main.py already patched with correct title");
            return Ok(());
        }

        // Create backup
        let backup = main_py.with_extension("py.bak");
        if !backup.exists() {
            std::fs::copy(main_py, &backup).map_err(|e| PumasError::Io {
                message: format!("Failed to create backup: {}", e),
                path: Some(backup.clone()),
                source: Some(e),
            })?;
            debug!("Created backup at {:?}", backup);
        }

        // Check if an older patch exists and upgrade it
        let pattern =
            Regex::new(r#"setproctitle\.setproctitle\(["']ComfyUI Server[^"']*["']\)"#).unwrap();

        let new_content = if pattern.is_match(&content) {
            // Upgrade existing patch
            info!("Upgrading existing patch to include version");
            pattern.replace(&content, expected_line.as_str()).to_string()
        } else {
            // Insert new patch code
            let insert_code = format!(
                r#"
try:
    import setproctitle
    setproctitle.setproctitle("{}")
except ImportError:
    pass
"#,
                server_title
            );

            if content.contains("if __name__ == \"__main__\":") {
                content.replace(
                    "if __name__ == \"__main__\":",
                    &format!("{}if __name__ == \"__main__\":", insert_code),
                )
            } else {
                format!("{}{}", content, insert_code)
            }
        };

        std::fs::write(main_py, new_content).map_err(|e| PumasError::Io {
            message: format!("Failed to write patched main.py: {}", e),
            path: Some(main_py.to_path_buf()),
            source: Some(e),
        })?;

        info!("Applied setproctitle patch to {:?}", main_py);
        Ok(())
    }

    /// Revert the setproctitle patch from main.py.
    ///
    /// # Arguments
    ///
    /// * `main_py` - Path to main.py
    /// * `tag` - Optional version tag (used for downloading original if needed)
    fn revert_patch(&self, main_py: &Path, tag: Option<&str>) -> Result<()> {
        // Try backup first
        let backup = main_py.with_extension("py.bak");
        if backup.exists() {
            std::fs::copy(&backup, main_py).map_err(|e| PumasError::Io {
                message: format!("Failed to restore from backup: {}", e),
                path: Some(main_py.to_path_buf()),
                source: Some(e),
            })?;
            std::fs::remove_file(&backup).ok();
            info!("Reverted main.py from backup");
            return Ok(());
        }

        // Try git checkout (if this version is a git repo)
        let repo_dir = main_py.parent().unwrap_or(Path::new("."));
        if repo_dir.join(".git").exists() {
            let result = std::process::Command::new("git")
                .args(["-C", &repo_dir.to_string_lossy(), "checkout", "--", "main.py"])
                .output();

            if let Ok(output) = result {
                if output.status.success() {
                    info!("Reverted main.py via git checkout");
                    return Ok(());
                }
            }
            debug!("Git checkout failed, trying download");
        }

        // Try downloading from GitHub using curl (synchronous)
        let ref_name = tag.unwrap_or("master");
        let url = format!(
            "https://raw.githubusercontent.com/comfyanonymous/ComfyUI/{}/main.py",
            ref_name
        );

        info!("Downloading original main.py from {}", url);

        // Use curl for synchronous download
        let output = std::process::Command::new("curl")
            .args(["-fsSL", &url])
            .output()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to download main.py: {}", e),
                cause: Some(e.to_string()),
            })?;

        if !output.status.success() {
            return Err(PumasError::Network {
                message: format!("Failed to download main.py: curl exit code {}", output.status),
                cause: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            });
        }

        std::fs::write(main_py, &output.stdout).map_err(|e| PumasError::Io {
            message: format!("Failed to write main.py: {}", e),
            path: Some(main_py.to_path_buf()),
            source: Some(e),
        })?;

        info!("Reverted main.py by downloading from GitHub");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_build_server_title() {
        let temp_dir = TempDir::new().unwrap();
        let patch_mgr = PatchManager::new(
            temp_dir.path(),
            temp_dir.path().join("main.py"),
            None,
        );

        assert_eq!(patch_mgr.build_server_title(None), "ComfyUI Server");
        assert_eq!(
            patch_mgr.build_server_title(Some("v0.2.0")),
            "ComfyUI Server - v0.2.0"
        );
    }

    #[test]
    fn test_is_patched_no_file() {
        let temp_dir = TempDir::new().unwrap();
        let patch_mgr = PatchManager::new(
            temp_dir.path(),
            temp_dir.path().join("main.py"),
            None,
        );

        assert!(!patch_mgr.is_patched(None));
    }

    #[test]
    fn test_is_patched_unpatched_file() {
        let temp_dir = TempDir::new().unwrap();
        let main_py = temp_dir.path().join("main.py");

        std::fs::write(&main_py, r#"
if __name__ == "__main__":
    main()
"#).unwrap();

        let patch_mgr = PatchManager::new(
            temp_dir.path(),
            &main_py,
            None,
        );

        assert!(!patch_mgr.is_patched(None));
    }

    #[test]
    fn test_is_patched_patched_file() {
        let temp_dir = TempDir::new().unwrap();
        let main_py = temp_dir.path().join("main.py");

        std::fs::write(&main_py, r#"
try:
    import setproctitle
    setproctitle.setproctitle("ComfyUI Server")
except ImportError:
    pass

if __name__ == "__main__":
    main()
"#).unwrap();

        let patch_mgr = PatchManager::new(
            temp_dir.path(),
            &main_py,
            None,
        );

        assert!(patch_mgr.is_patched(None));
    }

    #[test]
    fn test_apply_patch() {
        let temp_dir = TempDir::new().unwrap();
        let main_py = temp_dir.path().join("main.py");

        std::fs::write(&main_py, r#"
import sys

if __name__ == "__main__":
    main()
"#).unwrap();

        let patch_mgr = PatchManager::new(
            temp_dir.path(),
            &main_py,
            None,
        );

        assert!(!patch_mgr.is_patched(None));
        patch_mgr.apply_patch(&main_py, None).unwrap();
        assert!(patch_mgr.is_patched(None));

        // Check backup was created
        assert!(main_py.with_extension("py.bak").exists());
    }
}
