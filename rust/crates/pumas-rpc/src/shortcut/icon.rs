//! Icon management for shortcuts.
//!
//! Handles:
//! - Installing icons to the XDG icon directories
//! - Generating version-specific icons with overlays (requires external tools)
//! - Icon cache updates

use pumas_library::error::{PumasError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, warn};

/// Standard icon sizes for XDG icon theme.
const ICON_SIZES: [u32; 4] = [256, 128, 64, 48];

/// Icon manager for shortcut icons.
pub struct IconManager {
    /// Path to the base icon file.
    base_icon: PathBuf,
    /// Directory for generated icons.
    generated_icons_dir: PathBuf,
    /// User's icon theme directory.
    icon_theme_dir: PathBuf,
}

impl IconManager {
    /// Create a new icon manager.
    ///
    /// # Arguments
    ///
    /// * `base_icon` - Path to the base icon (webp or png)
    /// * `generated_icons_dir` - Directory for generated icons
    pub fn new(base_icon: impl AsRef<Path>, generated_icons_dir: impl AsRef<Path>) -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let icon_theme_dir = home.join(".local").join("share").join("icons").join("hicolor");

        Self {
            base_icon: base_icon.as_ref().to_path_buf(),
            generated_icons_dir: generated_icons_dir.as_ref().to_path_buf(),
            icon_theme_dir,
        }
    }

    /// Check if the base icon exists.
    pub fn base_icon_exists(&self) -> bool {
        self.base_icon.exists()
    }

    /// Install the base icon for the application.
    ///
    /// Installs the icon at various sizes in the XDG icon directories.
    ///
    /// # Returns
    ///
    /// The icon name to use in .desktop files.
    pub fn install_base_icon(&self) -> Result<String> {
        let icon_name = "comfyui";

        if !self.base_icon.exists() {
            return Err(PumasError::NotFound {
                resource: format!("Base icon: {}", self.base_icon.display()),
            });
        }

        // Try to install at various sizes using ImageMagick
        let mut conversion_success = false;

        for size in ICON_SIZES {
            if let Err(e) = self.install_icon_at_size(&self.base_icon, icon_name, size) {
                debug!("Failed to install icon at size {}: {}", size, e);
            } else {
                conversion_success = true;
            }
        }

        // Fallback: copy to scalable directory
        if !conversion_success {
            self.install_scalable_icon(&self.base_icon, icon_name)?;
        }

        // Update icon cache
        self.update_icon_cache();

        // Try xdg-icon-resource as alternative
        self.try_xdg_icon_resource(&self.base_icon, icon_name);

        Ok(icon_name.to_string())
    }

    /// Install a version-specific icon.
    ///
    /// If ImageMagick is available, generates an icon with the version label overlay.
    /// Otherwise, uses the base icon.
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag to display
    /// * `slug` - Filesystem-safe version identifier
    ///
    /// # Returns
    ///
    /// The icon name to use in .desktop files.
    pub fn install_version_icon(&self, tag: &str, slug: &str) -> Result<String> {
        let icon_name = format!("comfyui-{}", slug);

        // Ensure generated icons directory exists
        fs::create_dir_all(&self.generated_icons_dir).map_err(|e| PumasError::Io {
            message: "create generated icons directory".to_string(),
            path: Some(self.generated_icons_dir.clone()),
            source: Some(e),
        })?;

        // Try to generate version-specific icon with overlay
        let source_icon = if let Some(generated) = self.generate_version_icon(tag, slug)? {
            generated
        } else {
            // Fall back to base icon
            self.base_icon.clone()
        };

        if !source_icon.exists() {
            return Ok("comfyui".to_string()); // Fall back to base icon name
        }

        // Install at various sizes
        let mut conversion_success = false;

        for size in ICON_SIZES {
            if let Err(e) = self.install_icon_at_size(&source_icon, &icon_name, size) {
                debug!("Failed to install version icon at size {}: {}", size, e);
            } else {
                conversion_success = true;
            }
        }

        if !conversion_success {
            self.install_scalable_icon(&source_icon, &icon_name)?;
        }

        self.update_icon_cache();
        self.try_xdg_icon_resource(&source_icon, &icon_name);

        Ok(icon_name)
    }

    /// Generate a version-specific icon with an overlay.
    ///
    /// Uses ImageMagick if available to add a version banner to the icon.
    fn generate_version_icon(&self, tag: &str, slug: &str) -> Result<Option<PathBuf>> {
        if !self.base_icon.exists() {
            return Ok(None);
        }

        // Check if ImageMagick is available
        if Command::new("convert").arg("-version").output().is_err() {
            debug!("ImageMagick not available, skipping version icon generation");
            return Ok(None);
        }

        let output_path = self.generated_icons_dir.join(format!("comfyui-{}.png", slug));
        let label = tag.trim_start_matches('v');

        // Use ImageMagick to create icon with version overlay
        // This creates a semi-transparent banner across the middle with the version text
        let result = Command::new("convert")
            .arg(&self.base_icon)
            .args(["-resize", "256x256"])
            .args([
                "-fill",
                "rgba(0,0,0,0.75)",
                "-draw",
                &format!("rectangle 10,100 246,156"),
            ])
            .args([
                "-fill",
                "white",
                "-font",
                "DejaVu-Sans-Bold",
                "-pointsize",
                "32",
                "-gravity",
                "center",
                "-annotate",
                "+0+0",
                label,
            ])
            .arg(&output_path)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                debug!("Generated version icon at {:?}", output_path);
                Ok(Some(output_path))
            }
            Ok(output) => {
                debug!(
                    "ImageMagick failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                Ok(None)
            }
            Err(e) => {
                debug!("Failed to run ImageMagick: {}", e);
                Ok(None)
            }
        }
    }

    /// Install an icon at a specific size.
    fn install_icon_at_size(&self, source: &Path, name: &str, size: u32) -> Result<()> {
        let icon_dir = self.icon_theme_dir.join(format!("{}x{}", size, size)).join("apps");

        fs::create_dir_all(&icon_dir).map_err(|e| PumasError::Io {
            message: "create icon directory".to_string(),
            path: Some(icon_dir.clone()),
            source: Some(e),
        })?;

        let dest = icon_dir.join(format!("{}.png", name));

        // Use ImageMagick to resize
        let result = Command::new("convert")
            .arg(source)
            .args(["-resize", &format!("{}x{}", size, size)])
            .arg(&dest)
            .output();

        match result {
            Ok(output) if output.status.success() => {
                debug!("Installed icon at {:?}", dest);
                Ok(())
            }
            Ok(output) => Err(PumasError::Other(format!(
                "convert failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))),
            Err(e) => Err(PumasError::Other(format!("Failed to run convert: {}", e))),
        }
    }

    /// Install an icon to the scalable directory (as fallback).
    fn install_scalable_icon(&self, source: &Path, name: &str) -> Result<()> {
        let icon_dir = self.icon_theme_dir.join("scalable").join("apps");

        fs::create_dir_all(&icon_dir).map_err(|e| PumasError::Io {
            message: "create scalable icon directory".to_string(),
            path: Some(icon_dir.clone()),
            source: Some(e),
        })?;

        let extension = source.extension().unwrap_or_default().to_string_lossy();
        let dest = icon_dir.join(format!("{}.{}", name, extension));

        fs::copy(source, &dest).map_err(|e| PumasError::Io {
            message: "copy icon".to_string(),
            path: Some(dest.clone()),
            source: Some(e),
        })?;

        // Create PNG symlink for compatibility
        if extension != "png" {
            let png_link = icon_dir.join(format!("{}.png", name));
            let _ = fs::remove_file(&png_link); // Remove existing if any
            if let Err(e) = std::os::unix::fs::symlink(&dest, &png_link) {
                debug!("Failed to create PNG symlink: {}", e);
            }
        }

        Ok(())
    }

    /// Update the GTK icon cache.
    fn update_icon_cache(&self) {
        let _ = Command::new("gtk-update-icon-cache")
            .args(["-f", "-t"])
            .arg(&self.icon_theme_dir)
            .output();
    }

    /// Try using xdg-icon-resource for icon installation.
    fn try_xdg_icon_resource(&self, source: &Path, name: &str) {
        let _ = Command::new("xdg-icon-resource")
            .args(["install", "--novendor", "--size", "256"])
            .arg(source)
            .arg(name)
            .output();
    }

    /// Remove an installed icon.
    pub fn remove_icon(&self, name: &str) -> Result<()> {
        // Remove from all size directories
        for size in ICON_SIZES {
            let icon_path = self
                .icon_theme_dir
                .join(format!("{}x{}", size, size))
                .join("apps")
                .join(format!("{}.png", name));

            if icon_path.exists() {
                let _ = fs::remove_file(&icon_path);
            }
        }

        // Remove from scalable directory
        let scalable_dir = self.icon_theme_dir.join("scalable").join("apps");
        for ext in ["png", "webp", "svg"] {
            let icon_path = scalable_dir.join(format!("{}.{}", name, ext));
            if icon_path.exists() {
                let _ = fs::remove_file(&icon_path);
            }
        }

        // Remove generated icon if exists
        let generated_path = self.generated_icons_dir.join(format!("{}.png", name));
        if generated_path.exists() {
            let _ = fs::remove_file(&generated_path);
        }

        self.update_icon_cache();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_icon_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let base_icon = temp_dir.path().join("icon.png");
        let generated_dir = temp_dir.path().join("generated");

        let manager = IconManager::new(&base_icon, &generated_dir);

        assert!(!manager.base_icon_exists());
    }

    #[test]
    fn test_base_icon_exists() {
        let temp_dir = TempDir::new().unwrap();
        let base_icon = temp_dir.path().join("icon.png");
        let generated_dir = temp_dir.path().join("generated");

        // Create a dummy icon
        fs::write(&base_icon, "dummy").unwrap();

        let manager = IconManager::new(&base_icon, &generated_dir);

        assert!(manager.base_icon_exists());
    }
}
