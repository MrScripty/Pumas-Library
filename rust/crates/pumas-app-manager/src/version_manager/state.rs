//! Version state tracking.
//!
//! Manages the state of installed, active, and default versions.
//! Handles state persistence and validation.

use pumas_library::config::AppId;
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use crate::version_manager::ValidationResult;
use pumas_library::{PumasError, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Tracks the state of all versions.
pub struct VersionState {
    /// Root directory for launcher.
    launcher_root: PathBuf,
    /// Application ID.
    app_id: AppId,
    /// Metadata manager for persistence.
    metadata_manager: Arc<MetadataManager>,
    /// Set of installed version tags (cached).
    installed_tags: HashSet<String>,
    /// Currently active version (session-specific).
    active_version: Option<String>,
    /// Default version from metadata.
    default_version: Option<String>,
}

impl VersionState {
    /// Create a new version state tracker.
    pub async fn new(
        launcher_root: &PathBuf,
        app_id: AppId,
        metadata_manager: Arc<MetadataManager>,
    ) -> Result<Self> {
        let mut state = Self {
            launcher_root: launcher_root.clone(),
            app_id,
            metadata_manager,
            installed_tags: HashSet::new(),
            active_version: None,
            default_version: None,
        };

        state.initialize().await?;
        Ok(state)
    }

    /// Initialize state from metadata and filesystem.
    async fn initialize(&mut self) -> Result<()> {
        // Load metadata
        let versions = self.metadata_manager.load_versions(Some(self.app_id))?;

        // Cache installed tags
        self.installed_tags = versions.installed.keys().cloned().collect();
        self.default_version = versions.default_version.clone();

        // Initialize active version
        self.active_version = self.determine_active_version(&versions)?;

        debug!(
            "Initialized version state: {} installed, active={:?}, default={:?}",
            self.installed_tags.len(),
            self.active_version,
            self.default_version
        );

        Ok(())
    }

    /// Determine which version should be active.
    ///
    /// Priority:
    /// 1. Read from .active-version file (if still valid)
    /// 2. Default version from metadata
    /// 3. Last selected version from metadata
    /// 4. Newest installed version
    fn determine_active_version(
        &self,
        versions: &pumas_library::metadata::VersionsMetadata,
    ) -> Result<Option<String>> {
        // 1. Check .active-version file
        let active_file = self.launcher_root.join(".active-version");
        if active_file.exists() {
            if let Ok(tag) = std::fs::read_to_string(&active_file) {
                let tag = tag.trim().to_string();
                if !tag.is_empty() && self.installed_tags.contains(&tag) {
                    debug!("Active version from file: {}", tag);
                    return Ok(Some(tag));
                }
            }
        }

        // 2. Default version
        if let Some(ref default) = versions.default_version {
            if self.installed_tags.contains(default) {
                debug!("Active version from default: {}", default);
                return Ok(Some(default.clone()));
            }
        }

        // 3. Last selected version
        if let Some(ref last) = versions.last_selected_version {
            if self.installed_tags.contains(last) {
                debug!("Active version from last selected: {}", last);
                return Ok(Some(last.clone()));
            }
        }

        // 4. Newest installed version (lexicographically, which works for semver with v prefix)
        if !self.installed_tags.is_empty() {
            let mut sorted: Vec<_> = self.installed_tags.iter().cloned().collect();
            sorted.sort();
            sorted.reverse(); // Newest first
            let newest = sorted.into_iter().next();
            debug!("Active version from newest: {:?}", newest);
            return Ok(newest);
        }

        Ok(None)
    }

    /// Refresh state from disk.
    pub fn refresh(&mut self) -> Result<()> {
        let versions = self.metadata_manager.load_versions(Some(self.app_id))?;
        self.installed_tags = versions.installed.keys().cloned().collect();
        self.default_version = versions.default_version.clone();

        // Re-validate active version
        if let Some(ref active) = self.active_version {
            if !self.installed_tags.contains(active) {
                self.active_version = self.determine_active_version(&versions)?;
            }
        } else {
            self.active_version = self.determine_active_version(&versions)?;
        }

        Ok(())
    }

    // ========================================
    // Getters
    // ========================================

    /// Get list of installed version tags.
    pub fn get_installed_tags(&self) -> Vec<String> {
        let mut tags: Vec<_> = self.installed_tags.iter().cloned().collect();
        tags.sort();
        tags.reverse(); // Newest first
        tags
    }

    /// Get the currently active version.
    pub fn get_active_version(&self) -> Option<String> {
        self.active_version.clone()
    }

    /// Get the default version.
    pub fn get_default_version(&self) -> Option<String> {
        self.default_version.clone()
    }

    /// Check if a version is installed.
    pub fn is_installed(&self, tag: &str) -> bool {
        self.installed_tags.contains(tag)
    }

    /// Get the path to a version's directory.
    pub fn get_version_path(&self, tag: &str) -> Option<PathBuf> {
        if self.is_installed(tag) {
            Some(
                self.launcher_root
                    .join(self.app_id.versions_dir_name())
                    .join(tag),
            )
        } else {
            None
        }
    }

    // ========================================
    // Setters
    // ========================================

    /// Set the active version.
    pub fn set_active_version(&mut self, tag: &str) -> Result<bool> {
        if !self.is_installed(tag) {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }

        // Update in-memory state
        self.active_version = Some(tag.to_string());

        // Write to .active-version file
        let active_file = self.launcher_root.join(".active-version");
        std::fs::write(&active_file, tag).map_err(|e| PumasError::Io {
            message: format!("Failed to write active version file: {}", e),
            path: Some(active_file),
            source: Some(e),
        })?;

        // Update last_selected_version in metadata
        self.metadata_manager
            .set_last_selected_version(Some(tag), Some(self.app_id))?;

        info!("Set active version: {}", tag);
        Ok(true)
    }

    /// Set the default version.
    pub fn set_default_version(&mut self, tag: Option<&str>) -> Result<bool> {
        if let Some(t) = tag {
            if !self.is_installed(t) {
                return Err(PumasError::VersionNotFound {
                    tag: t.to_string(),
                });
            }
        }

        // Update in-memory state
        self.default_version = tag.map(String::from);

        // Update metadata
        self.metadata_manager
            .set_default_version(tag, Some(self.app_id))?;

        info!("Set default version: {:?}", tag);
        Ok(true)
    }

    /// Add a new installed version.
    pub fn add_installed_version(
        &mut self,
        tag: &str,
        metadata: InstalledVersionMetadata,
    ) -> Result<()> {
        self.metadata_manager
            .update_installed_version(tag, metadata, Some(self.app_id))?;
        self.installed_tags.insert(tag.to_string());
        debug!("Added installed version: {}", tag);
        Ok(())
    }

    /// Remove an installed version.
    pub fn remove_installed_version(&mut self, tag: &str) -> Result<()> {
        self.metadata_manager
            .remove_installed_version(tag, Some(self.app_id))?;
        self.installed_tags.remove(tag);

        // Clear active if it was this version
        if self.active_version.as_deref() == Some(tag) {
            self.active_version = None;
            // Clear .active-version file
            let active_file = self.launcher_root.join(".active-version");
            if active_file.exists() {
                let _ = std::fs::remove_file(&active_file);
            }
        }

        // Clear default if it was this version
        if self.default_version.as_deref() == Some(tag) {
            self.default_version = None;
        }

        debug!("Removed installed version: {}", tag);
        Ok(())
    }

    // ========================================
    // Validation
    // ========================================

    /// Validate all installations and remove incomplete ones.
    pub fn validate_installations(&mut self) -> Result<ValidationResult> {
        let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
        let mut removed_tags = Vec::new();
        let mut orphaned_dirs = Vec::new();

        // Check metadata entries against filesystem
        let metadata = self.metadata_manager.load_versions(Some(self.app_id))?;
        for (tag, info) in &metadata.installed {
            let version_path = versions_dir.join(&info.path);
            if !self.is_version_complete(&version_path) {
                warn!("Incomplete installation found: {} at {}", tag, version_path.display());
                removed_tags.push(tag.clone());
            }
        }

        // Remove incomplete installations
        for tag in &removed_tags {
            let version_path = versions_dir.join(tag);
            if version_path.exists() {
                info!("Removing incomplete installation: {}", tag);
                let _ = std::fs::remove_dir_all(&version_path);
            }
            self.remove_installed_version(tag)?;
        }

        // Check for orphaned directories (exist on disk but not in metadata)
        if versions_dir.exists() {
            for entry in std::fs::read_dir(&versions_dir).map_err(|e| PumasError::Io {
                message: format!("Failed to read versions directory: {}", e),
                path: Some(versions_dir.clone()),
                source: Some(e),
            })? {
                if let Ok(entry) = entry {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    if entry.path().is_dir() && !self.installed_tags.contains(&dir_name) {
                        warn!("Orphaned version directory found: {}", dir_name);
                        orphaned_dirs.push(entry.path());
                    }
                }
            }
        }

        let valid_count = self.installed_tags.len();
        info!(
            "Validation complete: {} valid, {} removed, {} orphaned",
            valid_count,
            removed_tags.len(),
            orphaned_dirs.len()
        );

        Ok(ValidationResult {
            removed_tags,
            orphaned_dirs,
            valid_count,
        })
    }

    /// Check if a version installation is complete.
    fn is_version_complete(&self, version_path: &PathBuf) -> bool {
        if !version_path.exists() {
            return false;
        }

        // Required files/directories for a complete installation
        let required = match self.app_id {
            AppId::ComfyUI => vec![
                version_path.join("main.py"),
                version_path.join("venv"),
                version_path.join("venv").join("bin").join("python"),
            ],
            AppId::Ollama => {
                // Ollama is a binary, different structure
                vec![version_path.join("ollama")]
            }
            _ => {
                // Generic check - just need the directory to exist
                vec![version_path.clone()]
            }
        };

        required.iter().all(|p| p.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_state() -> (VersionState, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create required directories
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("comfyui-versions")).unwrap();

        let metadata_manager = Arc::new(MetadataManager::new(temp_dir.path()));
        metadata_manager.ensure_directories().unwrap();

        let state = VersionState::new(&temp_dir.path().to_path_buf(), AppId::ComfyUI, metadata_manager)
            .await
            .unwrap();

        (state, temp_dir)
    }

    #[tokio::test]
    async fn test_empty_state() {
        let (state, _temp) = create_test_state().await;
        assert!(state.get_installed_tags().is_empty());
        assert!(state.get_active_version().is_none());
        assert!(state.get_default_version().is_none());
    }

    #[tokio::test]
    async fn test_add_installed_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory with required files
        let version_dir = temp.path().join("comfyui-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        std::fs::create_dir_all(version_dir.join("venv/bin")).unwrap();
        std::fs::write(version_dir.join("venv/bin/python"), "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };

        state.add_installed_version("v1.0.0", metadata).unwrap();

        assert!(state.is_installed("v1.0.0"));
        assert_eq!(state.get_installed_tags(), vec!["v1.0.0".to_string()]);
    }

    #[tokio::test]
    async fn test_set_active_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory
        let version_dir = temp.path().join("comfyui-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        std::fs::create_dir_all(version_dir.join("venv/bin")).unwrap();
        std::fs::write(version_dir.join("venv/bin/python"), "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();

        // Set active
        state.set_active_version("v1.0.0").unwrap();
        assert_eq!(state.get_active_version(), Some("v1.0.0".to_string()));

        // Check file was written
        let active_file = temp.path().join(".active-version");
        assert!(active_file.exists());
        assert_eq!(std::fs::read_to_string(active_file).unwrap(), "v1.0.0");
    }

    #[tokio::test]
    async fn test_set_active_version_not_installed() {
        let (mut state, _temp) = create_test_state().await;

        let result = state.set_active_version("v1.0.0");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_default_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory
        let version_dir = temp.path().join("comfyui-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        std::fs::create_dir_all(version_dir.join("venv/bin")).unwrap();
        std::fs::write(version_dir.join("venv/bin/python"), "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();

        // Set default
        state.set_default_version(Some("v1.0.0")).unwrap();
        assert_eq!(state.get_default_version(), Some("v1.0.0".to_string()));

        // Clear default
        state.set_default_version(None).unwrap();
        assert_eq!(state.get_default_version(), None);
    }

    #[tokio::test]
    async fn test_remove_installed_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory
        let version_dir = temp.path().join("comfyui-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        std::fs::create_dir_all(version_dir.join("venv/bin")).unwrap();
        std::fs::write(version_dir.join("venv/bin/python"), "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();
        state.set_active_version("v1.0.0").unwrap();
        state.set_default_version(Some("v1.0.0")).unwrap();

        // Remove
        state.remove_installed_version("v1.0.0").unwrap();

        assert!(!state.is_installed("v1.0.0"));
        assert!(state.get_active_version().is_none());
        assert!(state.get_default_version().is_none());
    }

    #[tokio::test]
    async fn test_validate_installations() {
        let (mut state, temp) = create_test_state().await;

        // Add a version in metadata but don't create files (incomplete)
        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();

        // Validate - should remove incomplete
        let result = state.validate_installations().unwrap();
        assert!(result.removed_tags.contains(&"v1.0.0".to_string()));
        assert!(!state.is_installed("v1.0.0"));
    }
}
