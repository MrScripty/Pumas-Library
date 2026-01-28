//! High-level shortcut management.
//!
//! Note: Currently implements Linux-specific shortcuts (.desktop files).
//! Windows support would require implementing .lnk file creation.

use super::desktop_entry::DesktopEntry;
use super::icon::IconManager;
use super::launch_script::LaunchScriptGenerator;
use crate::error::{PumasError, Result};
use crate::platform;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Shortcut state for a version.
#[derive(Debug, Clone)]
pub struct ShortcutState {
    /// Version tag.
    pub tag: String,
    /// Whether menu shortcut exists.
    pub menu: bool,
    /// Whether desktop shortcut exists.
    pub desktop: bool,
}

/// Result of shortcut operations.
#[derive(Debug, Clone)]
pub struct ShortcutResult {
    /// Whether the operation was successful.
    pub success: bool,
    /// Whether menu shortcut was created/removed.
    pub menu: bool,
    /// Whether desktop shortcut was created/removed.
    pub desktop: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// Current shortcut state.
    pub state: ShortcutState,
}

/// High-level shortcut manager.
pub struct ShortcutManager {
    /// Launcher root directory.
    script_dir: PathBuf,
    /// Icon manager.
    icon_manager: IconManager,
    /// Launch script generator.
    script_generator: LaunchScriptGenerator,
    /// Applications directory (~/.local/share/applications).
    apps_dir: PathBuf,
    /// Desktop directory (~/Desktop).
    desktop_dir: PathBuf,
    /// Known version paths.
    version_paths: HashMap<String, PathBuf>,
}

impl ShortcutManager {
    /// Create a new shortcut manager.
    ///
    /// # Arguments
    ///
    /// * `script_dir` - Launcher root directory
    pub fn new(script_dir: impl AsRef<Path>) -> Result<Self> {
        let script_dir = script_dir.as_ref().to_path_buf();
        let launcher_data = script_dir.join("launcher-data");

        // Icon paths
        let base_icon = script_dir.join("resources").join("icon.webp");
        let generated_icons_dir = launcher_data.join("generated-icons");

        // Script and profile paths
        let scripts_dir = launcher_data.join("shortcut-scripts");
        let profiles_dir = launcher_data.join("profiles");

        // Platform-specific directories (uses centralized platform module)
        let apps_dir = platform::apps_dir()?;
        let desktop_dir = platform::desktop_dir()?;

        Ok(Self {
            script_dir,
            icon_manager: IconManager::new(&base_icon, &generated_icons_dir),
            script_generator: LaunchScriptGenerator::new(&scripts_dir, &profiles_dir),
            apps_dir,
            desktop_dir,
            version_paths: HashMap::new(),
        })
    }

    /// Set known version paths.
    pub fn set_version_paths(&mut self, paths: HashMap<String, PathBuf>) {
        self.version_paths = paths;
    }

    /// Convert a version tag to a filesystem-safe slug.
    fn slugify_tag(&self, tag: &str) -> String {
        let safe: String = tag
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();

        safe.trim_matches(|c| c == '-' || c == '_')
            .to_string()
            .chars()
            .take(64) // Limit length
            .collect::<String>()
            .replace("--", "-")
    }

    /// Get shortcut state for a version.
    pub fn get_version_shortcut_state(&self, tag: &str) -> ShortcutState {
        let slug = self.slugify_tag(tag);

        let menu_path = self.apps_dir.join(format!("ComfyUI-{}.desktop", slug));
        let desktop_path = self.desktop_dir.join(format!("ComfyUI-{}.desktop", slug));

        ShortcutState {
            tag: tag.to_string(),
            menu: menu_path.exists(),
            desktop: desktop_path.exists(),
        }
    }

    /// Get shortcut states for all known versions.
    pub fn get_all_shortcut_states(&self) -> HashMap<String, ShortcutState> {
        self.version_paths
            .keys()
            .map(|tag| (tag.clone(), self.get_version_shortcut_state(tag)))
            .collect()
    }

    /// Create shortcuts for a version.
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag
    /// * `version_dir` - Path to the version installation
    /// * `create_menu` - Whether to create menu shortcut
    /// * `create_desktop` - Whether to create desktop shortcut
    pub fn create_version_shortcuts(
        &self,
        tag: &str,
        version_dir: &Path,
        create_menu: bool,
        create_desktop: bool,
    ) -> Result<ShortcutResult> {
        let slug = self.slugify_tag(tag);

        // Validate version directory
        let venv_python = version_dir.join("venv").join("bin").join("python");
        let main_py = version_dir.join("main.py");

        if !venv_python.exists() || !main_py.exists() {
            return Ok(ShortcutResult {
                success: false,
                menu: false,
                desktop: false,
                error: Some(format!("Version {} is not installed or incomplete", tag)),
                state: self.get_version_shortcut_state(tag),
            });
        }

        // Install base icon
        let base_icon_name = match self.icon_manager.install_base_icon() {
            Ok(name) => name,
            Err(e) => {
                warn!("Failed to install base icon: {}", e);
                "comfyui".to_string()
            }
        };

        // Install version-specific icon for desktop
        let desktop_icon_name = match self.icon_manager.install_version_icon(tag, &slug) {
            Ok(name) => name,
            Err(e) => {
                warn!("Failed to install version icon: {}", e);
                base_icon_name.clone()
            }
        };

        // Generate launch script
        let launcher_script = self.script_generator.generate(tag, version_dir, &slug)?;

        let mut menu_created = false;
        let mut desktop_created = false;
        let mut errors = Vec::new();

        // Create menu shortcut
        if create_menu {
            match self.create_menu_shortcut(tag, &slug, &launcher_script, &base_icon_name) {
                Ok(()) => menu_created = true,
                Err(e) => {
                    warn!("Failed to create menu shortcut: {}", e);
                    errors.push(format!("Menu: {}", e));
                }
            }
        }

        // Create desktop shortcut
        if create_desktop {
            match self.create_desktop_shortcut(tag, &slug, &launcher_script, &desktop_icon_name) {
                Ok(()) => desktop_created = true,
                Err(e) => {
                    warn!("Failed to create desktop shortcut: {}", e);
                    errors.push(format!("Desktop: {}", e));
                }
            }
        }

        let success = (menu_created || !create_menu) && (desktop_created || !create_desktop);
        let error = if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        };

        info!(
            "Created shortcuts for {}: menu={}, desktop={}",
            tag, menu_created, desktop_created
        );

        Ok(ShortcutResult {
            success,
            menu: menu_created,
            desktop: desktop_created,
            error,
            state: self.get_version_shortcut_state(tag),
        })
    }

    /// Create a menu shortcut.
    fn create_menu_shortcut(
        &self,
        tag: &str,
        slug: &str,
        launcher_script: &Path,
        icon_name: &str,
    ) -> Result<()> {
        fs::create_dir_all(&self.apps_dir).map_err(|e| PumasError::Io {
            message: "create applications directory".to_string(),
            path: Some(self.apps_dir.clone()),
            source: Some(e),
        })?;

        let desktop_path = self.apps_dir.join(format!("ComfyUI-{}.desktop", slug));

        let entry = DesktopEntry::builder()
            .name(format!("ComfyUI {}", tag))
            .comment(format!("Launch ComfyUI {}", tag))
            .exec(format!("bash \"{}\"", launcher_script.display()))
            .icon(icon_name)
            .terminal(false)
            .build();

        entry.write_to_file(&desktop_path)?;

        Ok(())
    }

    /// Create a desktop shortcut.
    fn create_desktop_shortcut(
        &self,
        tag: &str,
        slug: &str,
        launcher_script: &Path,
        icon_name: &str,
    ) -> Result<()> {
        fs::create_dir_all(&self.desktop_dir).map_err(|e| PumasError::Io {
            message: "create Desktop directory".to_string(),
            path: Some(self.desktop_dir.clone()),
            source: Some(e),
        })?;

        let desktop_path = self.desktop_dir.join(format!("ComfyUI-{}.desktop", slug));

        // Desktop shortcut just shows "ComfyUI" (icon already has version)
        let entry = DesktopEntry::builder()
            .name("ComfyUI")
            .comment(format!("Launch ComfyUI {}", tag))
            .exec(format!("bash \"{}\"", launcher_script.display()))
            .icon(icon_name)
            .terminal(false)
            .build();

        entry.write_to_file(&desktop_path)?;

        Ok(())
    }

    /// Remove shortcuts for a version.
    pub fn remove_version_shortcuts(
        &self,
        tag: &str,
        remove_menu: bool,
        remove_desktop: bool,
    ) -> Result<ShortcutResult> {
        let slug = self.slugify_tag(tag);

        if remove_menu {
            let menu_path = self.apps_dir.join(format!("ComfyUI-{}.desktop", slug));
            if menu_path.exists() {
                fs::remove_file(&menu_path).map_err(|e| PumasError::Io {
                    message: "remove menu shortcut".to_string(),
                    path: Some(menu_path.to_path_buf()),
                    source: Some(e),
                })?;
            }
        }

        if remove_desktop {
            let desktop_path = self.desktop_dir.join(format!("ComfyUI-{}.desktop", slug));
            if desktop_path.exists() {
                fs::remove_file(&desktop_path).map_err(|e| PumasError::Io {
                    message: "remove desktop shortcut".to_string(),
                    path: Some(desktop_path.to_path_buf()),
                    source: Some(e),
                })?;
            }
        }

        // If no shortcuts remain, clean up launcher script and icons
        let state = self.get_version_shortcut_state(tag);
        if !state.menu && !state.desktop {
            let _ = self.script_generator.remove(&slug);
            let _ = self.icon_manager.remove_icon(&format!("comfyui-{}", slug));
        }

        info!(
            "Removed shortcuts for {}: menu={}, desktop={}",
            tag, remove_menu, remove_desktop
        );

        Ok(ShortcutResult {
            success: true,
            menu: !remove_menu,
            desktop: !remove_desktop,
            error: None,
            state: self.get_version_shortcut_state(tag),
        })
    }

    /// Set shortcut state for a version.
    ///
    /// Convenience method that creates or removes shortcuts as needed.
    pub fn set_version_shortcuts(
        &self,
        tag: &str,
        version_dir: &Path,
        enabled: bool,
        menu: bool,
        desktop: bool,
    ) -> Result<ShortcutResult> {
        if enabled {
            self.create_version_shortcuts(tag, version_dir, menu, desktop)
        } else {
            self.remove_version_shortcuts(tag, menu, desktop)
        }
    }

    /// Toggle menu shortcut for a version.
    pub fn toggle_menu_shortcut(&self, tag: &str, version_dir: &Path) -> Result<ShortcutResult> {
        let state = self.get_version_shortcut_state(tag);
        self.set_version_shortcuts(tag, version_dir, !state.menu, true, false)
    }

    /// Toggle desktop shortcut for a version.
    pub fn toggle_desktop_shortcut(&self, tag: &str, version_dir: &Path) -> Result<ShortcutResult> {
        let state = self.get_version_shortcut_state(tag);
        self.set_version_shortcuts(tag, version_dir, !state.desktop, false, true)
    }

    /// Check if menu shortcut exists (legacy API).
    pub fn menu_exists(&self) -> bool {
        self.apps_dir.join("ComfyUI.desktop").exists()
    }

    /// Check if desktop shortcut exists (legacy API).
    pub fn desktop_exists(&self) -> bool {
        self.desktop_dir.join("ComfyUI.desktop").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_slugify_tag() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ShortcutManager::new(temp_dir.path()).unwrap();

        assert_eq!(manager.slugify_tag("v1.0.0"), "v1-0-0");
        assert_eq!(manager.slugify_tag("  v2.0.0-beta  "), "v2-0-0-beta");
        assert_eq!(manager.slugify_tag("V3.0.0"), "v3-0-0");
    }

    #[test]
    fn test_shortcut_state() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ShortcutManager::new(temp_dir.path()).unwrap();

        let state = manager.get_version_shortcut_state("v1.0.0");

        assert_eq!(state.tag, "v1.0.0");
        assert!(!state.menu);
        assert!(!state.desktop);
    }

    #[test]
    fn test_create_shortcuts_missing_version() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ShortcutManager::new(temp_dir.path()).unwrap();

        let version_dir = temp_dir.path().join("versions").join("v1.0.0");
        fs::create_dir_all(&version_dir).unwrap();

        // No venv or main.py - should fail
        let result = manager
            .create_version_shortcuts("v1.0.0", &version_dir, true, true)
            .unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
