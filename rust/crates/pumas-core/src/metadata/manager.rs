//! Metadata manager for JSON persistence.
//!
//! Manages metadata files:
//! - versions.json / versions-{app}.json
//! - models.json
//! - custom_nodes.json
//! - workflows.json
//! - github-releases.json / github-releases-{repo}.json
//! - Per-version config files

use crate::config::AppId;
use crate::metadata::atomic::{atomic_read_json, atomic_write_json};
use crate::{PumasError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::{debug, warn};

/// Metadata for an installed version.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstalledVersionMetadata {
    pub path: String,
    pub installed_date: String,
    #[serde(default)]
    pub python_version: Option<String>,
    pub release_tag: String,
    #[serde(default)]
    pub git_commit: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub release_notes: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub requirements_hash: Option<String>,
    #[serde(default)]
    pub dependencies_installed: Option<bool>,
}

/// Root structure for versions.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionsMetadata {
    #[serde(default)]
    pub installed: HashMap<String, InstalledVersionMetadata>,
    #[serde(default)]
    pub last_selected_version: Option<String>,
    #[serde(default)]
    pub default_version: Option<String>,
}

/// Metadata for a model file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetadata {
    pub path: String,
    pub size: u64,
    #[serde(default)]
    pub sha256: Option<String>,
    pub added_date: String,
    #[serde(default)]
    pub last_used: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub model_type: String,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub used_by_versions: Vec<String>,
    pub source: String,
    #[serde(default)]
    pub base_model: Option<String>,
}

/// Root structure for models.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelsMetadata {
    #[serde(flatten)]
    pub models: HashMap<String, ModelMetadata>,
}

/// Compatibility status for a custom node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityStatus {
    pub status: String, // "compatible", "incompatible", "unknown"
    pub checked_at: String,
    #[serde(default)]
    pub requirements_hash: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub conflicting_packages: Option<Vec<String>>,
}

/// Metadata for a custom node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomNodeMetadata {
    #[serde(default)]
    pub cache_repo: Option<String>,
    pub git_url: String,
    #[serde(default)]
    pub last_fetched: Option<String>,
    #[serde(default)]
    pub available_tags: Vec<String>,
    #[serde(default)]
    pub latest_commit: Option<String>,
    #[serde(default)]
    pub has_requirements: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub compatibility_cache: HashMap<String, CompatibilityStatus>,
}

/// Root structure for custom_nodes.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomNodesMetadata {
    #[serde(flatten)]
    pub nodes: HashMap<String, CustomNodeMetadata>,
}

/// Metadata for a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowMetadata {
    pub path: String,
    pub created_date: String,
    pub modified_date: String,
    #[serde(default)]
    pub used_by_versions: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required_nodes: Vec<String>,
    #[serde(default)]
    pub required_models: Vec<String>,
}

/// Root structure for workflows.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowsMetadata {
    #[serde(flatten)]
    pub workflows: HashMap<String, WorkflowMetadata>,
}

/// Custom node configuration within a version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionCustomNodeConfig {
    pub enabled: bool,
    pub git_commit: String,
    #[serde(default)]
    pub git_tag: Option<String>,
    pub install_date: String,
    #[serde(default)]
    pub compatibility_status: Option<String>,
    #[serde(default)]
    pub incompatibility_reason: Option<String>,
    #[serde(default)]
    pub conflicting_packages: Option<Vec<String>>,
    #[serde(default)]
    pub requirements_installed: bool,
}

/// Per-version configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionConfig {
    pub version: String,
    #[serde(default)]
    pub custom_nodes: HashMap<String, VersionCustomNodeConfig>,
    #[serde(default)]
    pub launch_args: Vec<String>,
    #[serde(default)]
    pub python_path: Option<String>,
    #[serde(default)]
    pub uv_path: Option<String>,
    #[serde(default)]
    pub requirements: HashMap<String, String>,
    #[serde(default)]
    pub requirements_hash: Option<String>,
}

/// Manager for all metadata files.
///
/// Provides thread-safe access to metadata through internal locking.
pub struct MetadataManager {
    launcher_root: PathBuf,
    /// Lock for serializing writes
    write_lock: RwLock<()>,
}

impl MetadataManager {
    /// Create a new metadata manager.
    pub fn new(launcher_root: impl Into<PathBuf>) -> Self {
        Self {
            launcher_root: launcher_root.into(),
            write_lock: RwLock::new(()),
        }
    }

    // ========================================
    // Path helpers
    // ========================================

    fn metadata_dir(&self) -> PathBuf {
        self.launcher_root.join("launcher-data").join("metadata")
    }

    fn cache_dir(&self) -> PathBuf {
        self.launcher_root.join("launcher-data").join("cache")
    }

    fn config_dir(&self) -> PathBuf {
        self.launcher_root
            .join("launcher-data")
            .join("config")
            .join("version-configs")
    }

    fn versions_path(&self, app_id: Option<AppId>) -> PathBuf {
        match app_id {
            Some(app) if app != AppId::ComfyUI => {
                let app_name = app.to_string().to_lowercase();
                self.metadata_dir()
                    .join(format!("versions-{}.json", app_name))
            }
            _ => self.metadata_dir().join("versions.json"),
        }
    }

    fn models_path(&self) -> PathBuf {
        self.metadata_dir().join("models.json")
    }

    fn custom_nodes_path(&self) -> PathBuf {
        self.metadata_dir().join("custom_nodes.json")
    }

    fn workflows_path(&self) -> PathBuf {
        self.metadata_dir().join("workflows.json")
    }

    fn github_releases_path(&self, repo: Option<&str>) -> PathBuf {
        match repo {
            Some(r) => {
                let safe_name = r.replace('/', "-");
                self.cache_dir()
                    .join(format!("github-releases-{}.json", safe_name))
            }
            None => self.cache_dir().join("github-releases.json"),
        }
    }

    fn version_config_path(&self, tag: &str) -> PathBuf {
        // Sanitize tag for filename
        let safe_tag = tag.replace(['/', '\\', ':'], "-");
        self.config_dir().join(format!("{}-config.json", safe_tag))
    }

    // ========================================
    // Versions metadata
    // ========================================

    /// Load versions metadata.
    pub fn load_versions(&self, app_id: Option<AppId>) -> Result<VersionsMetadata> {
        let path = self.versions_path(app_id);
        debug!("Loading versions from {}", path.display());

        match atomic_read_json(&path)? {
            Some(data) => Ok(data),
            None => Ok(VersionsMetadata::default()),
        }
    }

    /// Save versions metadata.
    pub fn save_versions(&self, data: &VersionsMetadata, app_id: Option<AppId>) -> Result<()> {
        let path = self.versions_path(app_id);
        let _lock = self.write_lock.write().map_err(|_| {
            PumasError::Other("Failed to acquire write lock for versions".to_string())
        })?;

        debug!("Saving versions to {}", path.display());
        atomic_write_json(&path, data, true)
    }

    /// Get a specific installed version's metadata.
    pub fn get_installed_version(
        &self,
        tag: &str,
        app_id: Option<AppId>,
    ) -> Result<Option<InstalledVersionMetadata>> {
        let versions = self.load_versions(app_id)?;
        Ok(versions.installed.get(tag).cloned())
    }

    /// Update a specific installed version's metadata.
    pub fn update_installed_version(
        &self,
        tag: &str,
        metadata: InstalledVersionMetadata,
        app_id: Option<AppId>,
    ) -> Result<()> {
        let mut versions = self.load_versions(app_id)?;
        versions.installed.insert(tag.to_string(), metadata);
        self.save_versions(&versions, app_id)
    }

    /// Remove an installed version from metadata.
    pub fn remove_installed_version(&self, tag: &str, app_id: Option<AppId>) -> Result<()> {
        let mut versions = self.load_versions(app_id)?;
        versions.installed.remove(tag);

        // Clear selection if this version was selected
        if versions.last_selected_version.as_deref() == Some(tag) {
            versions.last_selected_version = None;
        }
        if versions.default_version.as_deref() == Some(tag) {
            versions.default_version = None;
        }

        self.save_versions(&versions, app_id)
    }

    /// Set the last selected version.
    pub fn set_last_selected_version(
        &self,
        tag: Option<&str>,
        app_id: Option<AppId>,
    ) -> Result<()> {
        let mut versions = self.load_versions(app_id)?;
        versions.last_selected_version = tag.map(String::from);
        self.save_versions(&versions, app_id)
    }

    /// Set the default version.
    pub fn set_default_version(&self, tag: Option<&str>, app_id: Option<AppId>) -> Result<()> {
        let mut versions = self.load_versions(app_id)?;
        versions.default_version = tag.map(String::from);
        self.save_versions(&versions, app_id)
    }

    // ========================================
    // Models metadata
    // ========================================

    /// Load models metadata.
    pub fn load_models(&self) -> Result<ModelsMetadata> {
        let path = self.models_path();
        debug!("Loading models from {}", path.display());

        match atomic_read_json(&path)? {
            Some(data) => Ok(data),
            None => Ok(ModelsMetadata::default()),
        }
    }

    /// Save models metadata.
    pub fn save_models(&self, data: &ModelsMetadata) -> Result<()> {
        let path = self.models_path();
        let _lock = self.write_lock.write().map_err(|_| {
            PumasError::Other("Failed to acquire write lock for models".to_string())
        })?;

        debug!("Saving models to {}", path.display());
        atomic_write_json(&path, data, true)
    }

    // ========================================
    // Custom nodes metadata
    // ========================================

    /// Load custom nodes metadata.
    pub fn load_custom_nodes(&self) -> Result<CustomNodesMetadata> {
        let path = self.custom_nodes_path();
        debug!("Loading custom nodes from {}", path.display());

        match atomic_read_json(&path)? {
            Some(data) => Ok(data),
            None => Ok(CustomNodesMetadata::default()),
        }
    }

    /// Save custom nodes metadata.
    pub fn save_custom_nodes(&self, data: &CustomNodesMetadata) -> Result<()> {
        let path = self.custom_nodes_path();
        let _lock = self.write_lock.write().map_err(|_| {
            PumasError::Other("Failed to acquire write lock for custom nodes".to_string())
        })?;

        debug!("Saving custom nodes to {}", path.display());
        atomic_write_json(&path, data, true)
    }

    // ========================================
    // Workflows metadata
    // ========================================

    /// Load workflows metadata.
    pub fn load_workflows(&self) -> Result<WorkflowsMetadata> {
        let path = self.workflows_path();
        debug!("Loading workflows from {}", path.display());

        match atomic_read_json(&path)? {
            Some(data) => Ok(data),
            None => Ok(WorkflowsMetadata::default()),
        }
    }

    /// Save workflows metadata.
    pub fn save_workflows(&self, data: &WorkflowsMetadata) -> Result<()> {
        let path = self.workflows_path();
        let _lock = self.write_lock.write().map_err(|_| {
            PumasError::Other("Failed to acquire write lock for workflows".to_string())
        })?;

        debug!("Saving workflows to {}", path.display());
        atomic_write_json(&path, data, true)
    }

    // ========================================
    // GitHub releases cache
    // ========================================

    /// Load GitHub releases cache.
    pub fn load_github_releases(
        &self,
        repo: Option<&str>,
    ) -> Result<Option<crate::models::GitHubReleasesCache>> {
        let path = self.github_releases_path(repo);
        debug!("Loading GitHub releases from {}", path.display());
        atomic_read_json(&path)
    }

    /// Save GitHub releases cache.
    pub fn save_github_releases(
        &self,
        data: &crate::models::GitHubReleasesCache,
        repo: Option<&str>,
    ) -> Result<()> {
        let path = self.github_releases_path(repo);
        let _lock = self.write_lock.write().map_err(|_| {
            PumasError::Other("Failed to acquire write lock for GitHub releases".to_string())
        })?;

        debug!("Saving GitHub releases to {}", path.display());
        atomic_write_json(&path, data, false) // No backup for cache files
    }

    // ========================================
    // Version config
    // ========================================

    /// Load version-specific configuration.
    pub fn load_version_config(&self, tag: &str) -> Result<Option<VersionConfig>> {
        let path = self.version_config_path(tag);
        debug!("Loading version config from {}", path.display());
        atomic_read_json(&path)
    }

    /// Save version-specific configuration.
    pub fn save_version_config(&self, tag: &str, config: &VersionConfig) -> Result<()> {
        let path = self.version_config_path(tag);
        let _lock = self.write_lock.write().map_err(|_| {
            PumasError::Other("Failed to acquire write lock for version config".to_string())
        })?;

        debug!("Saving version config to {}", path.display());
        atomic_write_json(&path, config, true)
    }

    /// Delete version-specific configuration.
    pub fn delete_version_config(&self, tag: &str) -> Result<()> {
        let path = self.version_config_path(tag);
        let _lock = self.write_lock.write().map_err(|_| {
            PumasError::Other("Failed to acquire write lock for version config".to_string())
        })?;

        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| PumasError::Io {
                message: format!("Failed to delete version config {}", path.display()),
                path: Some(path.clone()),
                source: Some(e),
            })?;
            debug!("Deleted version config: {}", path.display());
        }

        Ok(())
    }

    // ========================================
    // Utility methods
    // ========================================

    /// Ensure all metadata directories exist.
    pub fn ensure_directories(&self) -> Result<()> {
        let dirs = [self.metadata_dir(), self.cache_dir(), self.config_dir()];

        for dir in dirs {
            if !dir.exists() {
                std::fs::create_dir_all(&dir).map_err(|e| PumasError::Io {
                    message: format!("Failed to create directory {}", dir.display()),
                    path: Some(dir.clone()),
                    source: Some(e),
                })?;
                debug!("Created directory: {}", dir.display());
            }
        }

        Ok(())
    }

    /// Clean up stale metadata entries.
    ///
    /// Removes metadata for versions that no longer exist on disk.
    pub fn cleanup_stale_versions(&self, app_id: Option<AppId>) -> Result<Vec<String>> {
        let mut versions = self.load_versions(app_id)?;
        let mut removed = Vec::new();

        let versions_dir = match app_id {
            Some(AppId::Ollama) => self.launcher_root.join("ollama-versions"),
            _ => self.launcher_root.join("comfyui-versions"),
        };

        // Check each installed version
        let tags_to_check: Vec<_> = versions.installed.keys().cloned().collect();
        for tag in tags_to_check {
            if let Some(metadata) = versions.installed.get(&tag) {
                let version_path = versions_dir.join(&metadata.path);
                if !version_path.exists() {
                    warn!(
                        "Removing stale version metadata: {} (path: {})",
                        tag,
                        version_path.display()
                    );
                    versions.installed.remove(&tag);
                    removed.push(tag);
                }
            }
        }

        if !removed.is_empty() {
            // Clear selections if they point to removed versions
            if let Some(ref selected) = versions.last_selected_version {
                if removed.contains(selected) {
                    versions.last_selected_version = None;
                }
            }
            if let Some(ref default) = versions.default_version {
                if removed.contains(default) {
                    versions.default_version = None;
                }
            }

            self.save_versions(&versions, app_id)?;
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (MetadataManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = MetadataManager::new(temp_dir.path());
        manager.ensure_directories().unwrap();
        (manager, temp_dir)
    }

    #[test]
    fn test_versions_crud() {
        let (manager, _temp) = create_test_manager();

        // Initially empty
        let versions = manager.load_versions(None).unwrap();
        assert!(versions.installed.is_empty());

        // Add a version
        let metadata = InstalledVersionMetadata {
            path: "v0.1.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v0.1.0".to_string(),
            ..Default::default()
        };
        manager
            .update_installed_version("v0.1.0", metadata.clone(), None)
            .unwrap();

        // Verify it exists
        let loaded = manager.get_installed_version("v0.1.0", None).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().release_tag, "v0.1.0");

        // Remove it
        manager.remove_installed_version("v0.1.0", None).unwrap();
        let loaded = manager.get_installed_version("v0.1.0", None).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_version_selection() {
        let (manager, _temp) = create_test_manager();

        // Add a version
        let metadata = InstalledVersionMetadata {
            path: "v0.1.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v0.1.0".to_string(),
            ..Default::default()
        };
        manager
            .update_installed_version("v0.1.0", metadata, None)
            .unwrap();

        // Set selections
        manager
            .set_last_selected_version(Some("v0.1.0"), None)
            .unwrap();
        manager.set_default_version(Some("v0.1.0"), None).unwrap();

        // Verify
        let versions = manager.load_versions(None).unwrap();
        assert_eq!(versions.last_selected_version, Some("v0.1.0".to_string()));
        assert_eq!(versions.default_version, Some("v0.1.0".to_string()));

        // Remove version should clear selections
        manager.remove_installed_version("v0.1.0", None).unwrap();
        let versions = manager.load_versions(None).unwrap();
        assert_eq!(versions.last_selected_version, None);
        assert_eq!(versions.default_version, None);
    }

    #[test]
    fn test_app_specific_versions() {
        let (manager, _temp) = create_test_manager();

        // Add ComfyUI version
        let comfy_metadata = InstalledVersionMetadata {
            path: "v0.1.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v0.1.0".to_string(),
            ..Default::default()
        };
        manager
            .update_installed_version("v0.1.0", comfy_metadata, None)
            .unwrap();

        // Add Ollama version
        let ollama_metadata = InstalledVersionMetadata {
            path: "v0.5.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v0.5.0".to_string(),
            ..Default::default()
        };
        manager
            .update_installed_version("v0.5.0", ollama_metadata, Some(AppId::Ollama))
            .unwrap();

        // Verify they're in separate files
        let comfy_versions = manager.load_versions(None).unwrap();
        let ollama_versions = manager.load_versions(Some(AppId::Ollama)).unwrap();

        assert!(comfy_versions.installed.contains_key("v0.1.0"));
        assert!(!comfy_versions.installed.contains_key("v0.5.0"));

        assert!(ollama_versions.installed.contains_key("v0.5.0"));
        assert!(!ollama_versions.installed.contains_key("v0.1.0"));
    }

    #[test]
    fn test_version_config() {
        let (manager, _temp) = create_test_manager();

        let config = VersionConfig {
            version: "v0.1.0".to_string(),
            custom_nodes: HashMap::new(),
            launch_args: vec!["--listen".to_string()],
            python_path: Some("/usr/bin/python3".to_string()),
            uv_path: None,
            requirements: HashMap::new(),
            requirements_hash: None,
        };

        manager.save_version_config("v0.1.0", &config).unwrap();

        let loaded = manager.load_version_config("v0.1.0").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.version, "v0.1.0");
        assert_eq!(loaded.launch_args, vec!["--listen"]);

        // Delete config
        manager.delete_version_config("v0.1.0").unwrap();
        let loaded = manager.load_version_config("v0.1.0").unwrap();
        assert!(loaded.is_none());
    }
}
