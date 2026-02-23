//! Link registry for tracking created symlinks/hardlinks.
//!
//! Maintains a record of all links created for model mapping,
//! enabling cascade delete when models are removed.

use crate::error::Result;
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::types::{LinkEntry, LinkType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tracks all symlinks/hardlinks created for model mapping.
///
/// This enables:
/// - Cascade delete when a model is removed
/// - Finding all links pointing to a specific model
/// - Cleaning up broken links
#[derive(Debug)]
pub struct LinkRegistry {
    /// Path to the registry JSON file
    registry_path: PathBuf,
    /// In-memory cache of links
    links: Arc<RwLock<LinkData>>,
}

/// Internal link storage structure.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct LinkData {
    /// Links indexed by target path
    #[serde(default)]
    by_target: HashMap<PathBuf, LinkEntry>,
    /// Index: model_id -> list of target paths
    #[serde(default)]
    by_model: HashMap<String, Vec<PathBuf>>,
}

impl LinkRegistry {
    /// Create a new link registry.
    ///
    /// # Arguments
    ///
    /// * `registry_path` - Path to store the registry JSON file
    pub fn new(registry_path: impl Into<PathBuf>) -> Self {
        Self {
            registry_path: registry_path.into(),
            links: Arc::new(RwLock::new(LinkData::default())),
        }
    }

    /// Load the registry from disk.
    pub async fn load(&self) -> Result<()> {
        if !self.registry_path.exists() {
            return Ok(());
        }

        let data: Option<LinkData> = atomic_read_json(&self.registry_path)?;
        if let Some(data) = data {
            *self.links.write().await = data;
        }

        Ok(())
    }

    /// Save the registry to disk.
    pub async fn save(&self) -> Result<()> {
        let data = self.links.read().await.clone();
        atomic_write_json(&self.registry_path, &data, false)?;
        Ok(())
    }

    /// Register a new link.
    ///
    /// # Arguments
    ///
    /// * `entry` - Link entry to register
    pub async fn register(&self, entry: LinkEntry) -> Result<()> {
        let mut data = self.links.write().await;

        // Add to by_model index
        data.by_model
            .entry(entry.model_id.clone())
            .or_default()
            .push(entry.target.clone());

        // Add to by_target
        data.by_target.insert(entry.target.clone(), entry);

        drop(data);
        self.save().await
    }

    /// Unregister a link by target path.
    ///
    /// # Arguments
    ///
    /// * `target` - Target path of the link to remove
    pub async fn unregister(&self, target: impl AsRef<Path>) -> Result<Option<LinkEntry>> {
        let target = target.as_ref();
        let mut data = self.links.write().await;

        if let Some(entry) = data.by_target.remove(target) {
            // Remove from by_model index
            if let Some(targets) = data.by_model.get_mut(&entry.model_id) {
                targets.retain(|t| t != target);
                if targets.is_empty() {
                    data.by_model.remove(&entry.model_id);
                }
            }

            drop(data);
            self.save().await?;
            return Ok(Some(entry));
        }

        Ok(None)
    }

    /// Get all links for a specific model.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model ID to find links for
    pub async fn get_links_for_model(&self, model_id: &str) -> Vec<LinkEntry> {
        let data = self.links.read().await;

        data.by_model
            .get(model_id)
            .map(|targets| {
                targets
                    .iter()
                    .filter_map(|t| data.by_target.get(t).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Remove all links for a specific model.
    ///
    /// This is used for cascade delete when a model is removed.
    /// Returns the list of target paths that were removed.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model ID to remove all links for
    pub async fn remove_all_for_model(&self, model_id: &str) -> Result<Vec<PathBuf>> {
        let mut data = self.links.write().await;

        let targets = data.by_model.remove(model_id).unwrap_or_default();

        // Remove from by_target
        for target in &targets {
            data.by_target.remove(target);
        }

        drop(data);
        self.save().await?;

        Ok(targets)
    }

    /// Get a link entry by target path.
    pub async fn get_by_target(&self, target: impl AsRef<Path>) -> Option<LinkEntry> {
        let data = self.links.read().await;
        data.by_target.get(target.as_ref()).cloned()
    }

    /// Check if a target path is registered.
    pub async fn contains_target(&self, target: impl AsRef<Path>) -> bool {
        let data = self.links.read().await;
        data.by_target.contains_key(target.as_ref())
    }

    /// Get all registered links.
    pub async fn get_all(&self) -> Vec<LinkEntry> {
        let data = self.links.read().await;
        data.by_target.values().cloned().collect()
    }

    /// Find and remove broken links (links to non-existent files).
    ///
    /// Returns the list of broken links that were removed.
    pub async fn cleanup_broken(&self) -> Result<Vec<LinkEntry>> {
        let mut broken = Vec::new();

        {
            let data = self.links.read().await;
            for (target, entry) in &data.by_target {
                // Check if the link target still exists
                if !target.exists() {
                    broken.push(entry.clone());
                } else if target.symlink_metadata().is_ok() {
                    // For symlinks, check if the source still exists
                    if target.is_symlink() && !entry.source.exists() {
                        broken.push(entry.clone());
                    }
                }
            }
        }

        // Remove broken links
        for entry in &broken {
            self.unregister(&entry.target).await?;
        }

        Ok(broken)
    }

    /// Get links for a specific app.
    pub async fn get_links_for_app(&self, app_id: &str) -> Vec<LinkEntry> {
        let data = self.links.read().await;
        data.by_target
            .values()
            .filter(|e| e.app_id == app_id)
            .cloned()
            .collect()
    }

    /// Get links for a specific app version.
    pub async fn get_links_for_app_version(
        &self,
        app_id: &str,
        version: Option<&str>,
    ) -> Vec<LinkEntry> {
        let data = self.links.read().await;
        data.by_target
            .values()
            .filter(|e| e.app_id == app_id && e.app_version.as_deref() == version)
            .cloned()
            .collect()
    }

    /// Get the count of registered links.
    pub async fn count(&self) -> usize {
        let data = self.links.read().await;
        data.by_target.len()
    }

    /// Clear all registry entries.
    pub async fn clear(&self) -> Result<()> {
        let mut data = self.links.write().await;
        data.by_target.clear();
        data.by_model.clear();
        drop(data);
        self.save().await
    }
}

/// Create a new link entry with the current timestamp for tracking in the registry.
pub fn create_link_entry(
    model_id: &str,
    source: impl Into<PathBuf>,
    target: impl Into<PathBuf>,
    link_type: LinkType,
    app_id: &str,
    app_version: Option<&str>,
) -> LinkEntry {
    LinkEntry {
        model_id: model_id.to_string(),
        source: source.into(),
        target: target.into(),
        link_type,
        created_at: chrono::Utc::now().to_rfc3339(),
        app_id: app_id.to_string(),
        app_version: app_version.map(String::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_registry() -> (TempDir, LinkRegistry) {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("links.json");
        let registry = LinkRegistry::new(registry_path);
        (temp_dir, registry)
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let (_temp, registry) = setup_registry().await;

        let entry = create_link_entry(
            "model1",
            "/library/model1/file.gguf",
            "/app/models/file.gguf",
            LinkType::Symlink,
            "comfyui",
            Some("0.6.0"),
        );

        registry.register(entry.clone()).await.unwrap();

        // Get by target
        let retrieved = registry.get_by_target(&entry.target).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().model_id, "model1");

        // Get by model
        let links = registry.get_links_for_model("model1").await;
        assert_eq!(links.len(), 1);
    }

    #[tokio::test]
    async fn test_unregister() {
        let (_temp, registry) = setup_registry().await;

        let entry = create_link_entry(
            "model1",
            "/library/model1/file.gguf",
            "/app/models/file.gguf",
            LinkType::Symlink,
            "comfyui",
            None,
        );

        registry.register(entry.clone()).await.unwrap();
        assert_eq!(registry.count().await, 1);

        registry.unregister(&entry.target).await.unwrap();
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_remove_all_for_model() {
        let (_temp, registry) = setup_registry().await;

        // Register multiple links for same model
        for i in 0..3 {
            let entry = create_link_entry(
                "model1",
                format!("/library/model1/file{}.gguf", i),
                format!("/app/models/file{}.gguf", i),
                LinkType::Symlink,
                "comfyui",
                None,
            );
            registry.register(entry).await.unwrap();
        }

        assert_eq!(registry.count().await, 3);

        let removed = registry.remove_all_for_model("model1").await.unwrap();
        assert_eq!(removed.len(), 3);
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("links.json");

        // Create and populate registry
        {
            let registry = LinkRegistry::new(&registry_path);
            let entry = create_link_entry(
                "model1",
                "/library/model1/file.gguf",
                "/app/models/file.gguf",
                LinkType::Symlink,
                "comfyui",
                None,
            );
            registry.register(entry).await.unwrap();
        }

        // Create new registry and load
        {
            let registry = LinkRegistry::new(&registry_path);
            registry.load().await.unwrap();
            assert_eq!(registry.count().await, 1);
        }
    }

    #[tokio::test]
    async fn test_get_links_for_app() {
        let (_temp, registry) = setup_registry().await;

        let entry1 = create_link_entry(
            "model1",
            "/lib/m1",
            "/app1/m1",
            LinkType::Symlink,
            "app1",
            None,
        );
        let entry2 = create_link_entry(
            "model2",
            "/lib/m2",
            "/app2/m2",
            LinkType::Symlink,
            "app2",
            None,
        );

        registry.register(entry1).await.unwrap();
        registry.register(entry2).await.unwrap();

        let app1_links = registry.get_links_for_app("app1").await;
        assert_eq!(app1_links.len(), 1);
        assert_eq!(app1_links[0].model_id, "model1");
    }
}
