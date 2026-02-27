//! Plugin configuration loader.
//!
//! Loads plugin configurations from JSON files in the plugins directory.

use super::schema::PluginConfig;
use crate::error::{PumasError, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

/// Loads and manages plugin configurations.
pub struct PluginLoader {
    /// Directory containing plugin JSON files.
    plugins_dir: PathBuf,
    /// Cached plugin configurations.
    plugins: Arc<RwLock<HashMap<String, PluginConfig>>>,
}

impl PluginLoader {
    /// Create a new plugin loader for the given directory.
    ///
    /// The directory will be created if it doesn't exist.
    pub fn new(plugins_dir: impl AsRef<Path>) -> Result<Self> {
        let plugins_dir = plugins_dir.as_ref().to_path_buf();

        // Create plugins directory if it doesn't exist
        if !plugins_dir.exists() {
            std::fs::create_dir_all(&plugins_dir).map_err(|e| PumasError::Io {
                message: format!("Failed to create plugins directory: {}", e),
                path: Some(plugins_dir.clone()),
                source: Some(e),
            })?;
        }

        let loader = Self {
            plugins_dir,
            plugins: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load plugins on creation
        loader.reload()?;

        Ok(loader)
    }

    /// Reload all plugins from disk.
    ///
    /// This will clear the cache and reload all plugin configurations.
    pub fn reload(&self) -> Result<usize> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| PumasError::Other(format!("Failed to acquire plugins lock: {}", e)))?;

        plugins.clear();

        // Read all .json files in the plugins directory
        let entries = std::fs::read_dir(&self.plugins_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to read plugins directory: {}", e),
            path: Some(self.plugins_dir.clone()),
            source: Some(e),
        })?;

        let mut loaded_count = 0;

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();

            // Only process .json files
            if path.extension().map(|e| e != "json").unwrap_or(true) {
                continue;
            }

            match self.load_plugin_file(&path) {
                Ok(config) => {
                    info!("Loaded plugin: {} ({})", config.display_name, config.id);
                    plugins.insert(config.id.clone(), config);
                    loaded_count += 1;
                }
                Err(e) => {
                    warn!("Failed to load plugin from {}: {}", path.display(), e);
                }
            }
        }

        debug!(
            "Loaded {} plugins from {}",
            loaded_count,
            self.plugins_dir.display()
        );

        Ok(loaded_count)
    }

    /// Load a single plugin configuration file.
    fn load_plugin_file(&self, path: &Path) -> Result<PluginConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| PumasError::Io {
            message: format!("Failed to read plugin file: {}", e),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;

        let config: PluginConfig =
            serde_json::from_str(&content).map_err(|e| PumasError::Json {
                message: format!(
                    "Failed to parse plugin config from {}: {}",
                    path.display(),
                    e
                ),
                source: None,
            })?;

        // Validate required fields
        if config.id.is_empty() {
            return Err(PumasError::Config {
                message: format!("Plugin in {} has empty id", path.display()),
            });
        }

        if config.display_name.is_empty() {
            return Err(PumasError::Config {
                message: format!("Plugin '{}' has empty display_name", config.id),
            });
        }

        Ok(config)
    }

    /// Get a plugin by ID.
    pub fn get(&self, id: &str) -> Option<PluginConfig> {
        self.plugins
            .read()
            .ok()
            .and_then(|plugins| plugins.get(id).cloned())
    }

    /// Get all loaded plugins.
    pub fn get_all(&self) -> Vec<PluginConfig> {
        self.plugins
            .read()
            .map(|plugins| plugins.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all enabled plugins, sorted by sidebar priority.
    pub fn get_enabled(&self) -> Vec<PluginConfig> {
        let mut plugins: Vec<_> = self
            .plugins
            .read()
            .map(|plugins| {
                plugins
                    .values()
                    .filter(|p| p.enabled_by_default)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        plugins.sort_by_key(|p| p.sidebar_priority);
        plugins
    }

    /// Get the plugins directory path.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Check if a plugin exists.
    pub fn exists(&self, id: &str) -> bool {
        self.plugins
            .read()
            .map(|plugins| plugins.contains_key(id))
            .unwrap_or(false)
    }

    /// Get the number of loaded plugins.
    pub fn count(&self) -> usize {
        self.plugins.read().map(|p| p.len()).unwrap_or(0)
    }

    /// Write a default plugin config file (for initial setup).
    pub fn write_default_config(&self, config: &PluginConfig) -> Result<PathBuf> {
        let path = self.plugins_dir.join(format!("{}.json", config.id));

        let content = serde_json::to_string_pretty(config).map_err(|e| PumasError::Json {
            message: format!("Failed to serialize plugin config: {}", e),
            source: None,
        })?;

        std::fs::write(&path, content).map_err(|e| PumasError::Io {
            message: format!("Failed to write plugin config: {}", e),
            path: Some(path.clone()),
            source: Some(e),
        })?;

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_plugin_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(format!("{}.json", name));
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_loader_creation() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");

        let loader = PluginLoader::new(&plugins_dir).unwrap();
        assert!(plugins_dir.exists());
        assert_eq!(loader.count(), 0);
    }

    #[test]
    fn test_load_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        create_test_plugin_file(
            &plugins_dir,
            "test-app",
            r#"{
                "id": "test-app",
                "displayName": "Test App",
                "installationType": "binary",
                "sidebarPriority": 10
            }"#,
        );

        let loader = PluginLoader::new(&plugins_dir).unwrap();
        assert_eq!(loader.count(), 1);

        let plugin = loader.get("test-app").unwrap();
        assert_eq!(plugin.display_name, "Test App");
        assert_eq!(plugin.sidebar_priority, 10);
    }

    #[test]
    fn test_get_enabled_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        create_test_plugin_file(
            &plugins_dir,
            "app-c",
            r#"{
                "id": "app-c",
                "displayName": "App C",
                "installationType": "binary",
                "sidebarPriority": 30
            }"#,
        );

        create_test_plugin_file(
            &plugins_dir,
            "app-a",
            r#"{
                "id": "app-a",
                "displayName": "App A",
                "installationType": "binary",
                "sidebarPriority": 10
            }"#,
        );

        create_test_plugin_file(
            &plugins_dir,
            "app-b",
            r#"{
                "id": "app-b",
                "displayName": "App B",
                "installationType": "binary",
                "sidebarPriority": 20,
                "enabledByDefault": false
            }"#,
        );

        let loader = PluginLoader::new(&plugins_dir).unwrap();
        let enabled = loader.get_enabled();

        // Should only have 2 enabled plugins (app-b is disabled)
        assert_eq!(enabled.len(), 2);
        // Should be sorted by priority
        assert_eq!(enabled[0].id, "app-a");
        assert_eq!(enabled[1].id, "app-c");
    }

    #[test]
    fn test_reload() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let loader = PluginLoader::new(&plugins_dir).unwrap();
        assert_eq!(loader.count(), 0);

        // Add a plugin file
        create_test_plugin_file(
            &plugins_dir,
            "new-app",
            r#"{
                "id": "new-app",
                "displayName": "New App",
                "installationType": "binary"
            }"#,
        );

        // Reload
        let loaded = loader.reload().unwrap();
        assert_eq!(loaded, 1);
        assert!(loader.exists("new-app"));
    }

    #[test]
    fn test_invalid_plugin_ignored() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        // Valid plugin
        create_test_plugin_file(
            &plugins_dir,
            "valid",
            r#"{
                "id": "valid",
                "displayName": "Valid App",
                "installationType": "binary"
            }"#,
        );

        // Invalid JSON
        create_test_plugin_file(&plugins_dir, "invalid", "{ not valid json }");

        // Missing required field
        create_test_plugin_file(
            &plugins_dir,
            "missing-id",
            r#"{
                "displayName": "Missing ID",
                "installationType": "binary"
            }"#,
        );

        let loader = PluginLoader::new(&plugins_dir).unwrap();
        // Should only load the valid plugin
        assert_eq!(loader.count(), 1);
        assert!(loader.exists("valid"));
    }
}
