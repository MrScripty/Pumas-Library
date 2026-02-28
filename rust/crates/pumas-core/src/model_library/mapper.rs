//! Model mapper for linking models to application directories.
//!
//! Creates symlinks (or hardlinks/copies as fallback) from the canonical
//! model library to application-specific model directories.

use crate::error::{PumasError, Result};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::library::ModelLibrary;
use crate::model_library::link_registry::{create_link_entry, LinkRegistry};
use crate::model_library::types::{
    ConflictResolution, LinkType, MappingAction, MappingActionType, MappingConfig, MappingPreview,
    MappingRule, SandboxInfo,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use walkdir::WalkDir;

/// Model mapper for creating links between library and applications.
pub struct ModelMapper {
    /// Reference to the model library
    library: Arc<ModelLibrary>,
    /// Directory containing mapping configurations
    config_dir: PathBuf,
    /// Link registry for tracking created links
    link_registry: Arc<RwLock<LinkRegistry>>,
}

impl ModelMapper {
    /// Create a new model mapper.
    ///
    /// # Arguments
    ///
    /// * `library` - Reference to the model library
    /// * `config_dir` - Directory containing mapping configuration files
    pub fn new(library: Arc<ModelLibrary>, config_dir: impl Into<PathBuf>) -> Self {
        let link_registry = library.link_registry().clone();
        Self {
            library,
            config_dir: config_dir.into(),
            link_registry,
        }
    }

    // ========================================
    // Configuration Management
    // ========================================

    /// Load mapping configuration for an app/version combination.
    ///
    /// Searches for configs with these precedence (highest to lowest):
    /// 1. {app}_{version}_custom.json
    /// 2. {app}_{version}_default.json
    /// 3. {app}_*_custom.json
    /// 4. {app}_*_default.json
    pub fn load_config(
        &self,
        app_id: &str,
        version: Option<&str>,
    ) -> Result<Option<MappingConfig>> {
        let configs = self.find_matching_configs(app_id, version)?;

        if configs.is_empty() {
            return Ok(None);
        }

        // Merge configs (later configs override earlier ones)
        let mut iter = configs.into_iter();
        let mut merged = iter.next().unwrap();

        for config in iter {
            // Merge mappings
            for rule in config.mappings {
                // Check if this rule replaces an existing one
                if let Some(existing) = merged
                    .mappings
                    .iter_mut()
                    .find(|r| r.target_dir == rule.target_dir)
                {
                    *existing = rule;
                } else {
                    merged.mappings.push(rule);
                }
            }
        }

        Ok(Some(merged))
    }

    /// Find all matching config files sorted by specificity.
    fn find_matching_configs(
        &self,
        app_id: &str,
        version: Option<&str>,
    ) -> Result<Vec<MappingConfig>> {
        let mut configs: Vec<(MappingConfig, u8)> = Vec::new();

        if !self.config_dir.exists() {
            return Ok(vec![]);
        }

        for entry in std::fs::read_dir(&self.config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let filename = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");

            // Parse filename pattern: {app}_{version}_{variant}
            let parts: Vec<&str> = filename.split('_').collect();
            if parts.len() < 2 {
                continue;
            }

            let file_app = parts[0];
            let file_version = parts.get(1).copied().unwrap_or("*");
            let file_variant = parts.get(2).copied().unwrap_or("default");

            // Check if this config matches
            if file_app != app_id {
                continue;
            }

            let version_matches =
                file_version == "*" || version.is_none() || version == Some(file_version);

            if !version_matches {
                continue;
            }

            // Calculate specificity score
            let specificity = self.calculate_specificity(file_version, file_variant, version);

            if let Ok(Some(config)) = atomic_read_json::<MappingConfig>(&path) {
                configs.push((config, specificity));
            }
        }

        // Sort by specificity (higher is better)
        configs.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(configs.into_iter().map(|(c, _)| c).collect())
    }

    /// Calculate specificity score for config precedence.
    fn calculate_specificity(
        &self,
        file_version: &str,
        file_variant: &str,
        requested_version: Option<&str>,
    ) -> u8 {
        let mut score = 0u8;

        // Exact version match is more specific than wildcard
        if file_version != "*" && requested_version == Some(file_version) {
            score += 4;
        }

        // Custom variant is more specific than default
        if file_variant == "custom" {
            score += 2;
        }

        score
    }

    /// Save a mapping configuration.
    pub fn save_config(&self, config: &MappingConfig) -> Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;

        let variant = config.variant.as_deref().unwrap_or("default");
        let filename = format!("{}_{}_{}.json", config.app, config.version, variant);
        let path = self.config_dir.join(filename);

        atomic_write_json(&path, config, false)
    }

    /// Create and persist a default ComfyUI mapping configuration.
    ///
    /// Generates rules for standard ComfyUI model directories (checkpoints, loras,
    /// vae, controlnet, clip, embeddings, upscale_models) and saves the config to disk.
    pub fn create_default_comfyui_config(
        &self,
        version: &str,
        _comfyui_models_path: &Path,
    ) -> Result<MappingConfig> {
        let config = MappingConfig {
            app: "comfyui".to_string(),
            version: version.to_string(),
            variant: Some("default".to_string()),
            mappings: vec![MappingRule {
                target_dir: ".".to_string(),
                model_types: None,
                subtypes: None,
                families: None,
                tags: None,
                exclude_tags: None,
            }],
        };

        self.save_config(&config)?;
        Ok(config)
    }

    // ========================================
    // Mapping Operations
    // ========================================

    /// Preview mapping operations without executing them.
    ///
    /// # Arguments
    ///
    /// * `app_id` - Application ID
    /// * `version` - Application version (optional)
    /// * `app_models_root` - Root directory for app's models
    pub async fn preview_mapping(
        &self,
        app_id: &str,
        version: Option<&str>,
        app_models_root: &Path,
    ) -> Result<MappingPreview> {
        let config = self
            .load_config(app_id, version)?
            .ok_or_else(|| PumasError::Config {
                message: format!("No mapping config found for {} {:?}", app_id, version),
            })?;

        let mut preview = MappingPreview::new();

        // Get excluded model IDs for this app
        let excluded_ids: std::collections::HashSet<String> = self
            .library
            .index()
            .get_excluded_model_ids(app_id)?
            .into_iter()
            .collect();

        // Get all models from library
        let models = self.library.list_models().await?;

        for model in models {
            // Skip models excluded from linking for this app
            if excluded_ids.contains(&model.id) {
                continue;
            }
            let model_name = if model.official_name.is_empty() {
                model.cleaned_name.clone()
            } else {
                model.official_name.clone()
            };

            // Find matching rules for this model
            for rule in &config.mappings {
                if !self.matches_rule(&model, rule) {
                    continue;
                }

                // Get model files
                let model_dir = self.library.library_root().join(&model.path);
                let files = self.get_model_files(&model_dir)?;

                for file_path in files {
                    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    let target_path = app_models_root.join(&rule.target_dir).join(filename);

                    let action = if target_path.exists() {
                        if target_path.is_symlink() {
                            // Check if it points to our file
                            if let Ok(link_target) = std::fs::read_link(&target_path) {
                                if link_target == file_path {
                                    MappingAction {
                                        action: MappingActionType::SkipExists,
                                        model_id: model.id.clone(),
                                        model_name: model_name.clone(),
                                        source: file_path.clone(),
                                        target: target_path,
                                        reason: Some("Link already exists".to_string()),
                                    }
                                } else {
                                    MappingAction {
                                        action: MappingActionType::SkipConflict,
                                        model_id: model.id.clone(),
                                        model_name: model_name.clone(),
                                        source: file_path.clone(),
                                        target: target_path,
                                        reason: Some("Different file exists".to_string()),
                                    }
                                }
                            } else {
                                MappingAction {
                                    action: MappingActionType::RemoveBroken,
                                    model_id: model.id.clone(),
                                    model_name: model_name.clone(),
                                    source: file_path.clone(),
                                    target: target_path,
                                    reason: Some("Broken symlink".to_string()),
                                }
                            }
                        } else {
                            MappingAction {
                                action: MappingActionType::SkipConflict,
                                model_id: model.id.clone(),
                                model_name: model_name.clone(),
                                source: file_path.clone(),
                                target: target_path,
                                reason: Some("Regular file exists".to_string()),
                            }
                        }
                    } else {
                        MappingAction {
                            action: MappingActionType::Create,
                            model_id: model.id.clone(),
                            model_name: model_name.clone(),
                            source: file_path.clone(),
                            target: target_path,
                            reason: None,
                        }
                    };

                    match action.action {
                        MappingActionType::Create => preview.creates.push(action),
                        MappingActionType::SkipExists => preview.skips.push(action),
                        MappingActionType::SkipConflict => preview.conflicts.push(action),
                        MappingActionType::RemoveBroken => preview.broken.push(action),
                    }
                }
            }
        }

        Ok(preview)
    }

    /// Apply mapping for an application.
    ///
    /// # Arguments
    ///
    /// * `app_id` - Application ID
    /// * `version` - Application version
    /// * `app_models_root` - Root directory for app's models
    pub async fn apply_mapping(
        &self,
        app_id: &str,
        version: Option<&str>,
        app_models_root: &Path,
    ) -> Result<MappingResult> {
        let preview = self
            .preview_mapping(app_id, version, app_models_root)
            .await?;

        let mut result = MappingResult {
            created: 0,
            skipped: preview.skips.len(),
            conflicts: preview.conflicts.len(),
            broken_removed: 0,
            errors: Vec::new(),
        };

        // Remove broken links
        for action in &preview.broken {
            if let Err(e) = std::fs::remove_file(&action.target) {
                result.errors.push((action.target.clone(), e.to_string()));
            } else {
                result.broken_removed += 1;
            }
        }

        // Create new links
        for action in preview.creates {
            if let Err(e) = self
                .create_link(&action, app_id, version.map(String::from))
                .await
            {
                result.errors.push((action.target, e.to_string()));
            } else {
                result.created += 1;
            }
        }

        Ok(result)
    }

    /// Apply mapping with per-path conflict resolution strategies.
    ///
    /// Like `apply_mapping`, but accepts a map of target paths to resolution
    /// strategies (Skip, Overwrite, Rename) for handling conflicting files.
    pub async fn apply_mapping_with_resolutions(
        &self,
        app_id: &str,
        version: Option<&str>,
        app_models_root: &Path,
        resolutions: &HashMap<PathBuf, ConflictResolution>,
    ) -> Result<MappingResult> {
        let preview = self
            .preview_mapping(app_id, version, app_models_root)
            .await?;

        let mut result = MappingResult {
            created: 0,
            skipped: preview.skips.len(),
            conflicts: 0,
            broken_removed: 0,
            errors: Vec::new(),
        };

        // Handle broken links
        for action in &preview.broken {
            if let Err(e) = std::fs::remove_file(&action.target) {
                result.errors.push((action.target.clone(), e.to_string()));
            } else {
                result.broken_removed += 1;
            }
        }

        // Handle creates
        for action in preview.creates {
            if let Err(e) = self
                .create_link(&action, app_id, version.map(String::from))
                .await
            {
                result.errors.push((action.target, e.to_string()));
            } else {
                result.created += 1;
            }
        }

        // Handle conflicts with resolutions
        for action in preview.conflicts {
            let resolution = resolutions
                .get(&action.target)
                .copied()
                .unwrap_or(ConflictResolution::Skip);

            match resolution {
                ConflictResolution::Skip => {
                    result.conflicts += 1;
                }
                ConflictResolution::Overwrite => {
                    // Remove existing and create link
                    if let Err(e) = std::fs::remove_file(&action.target) {
                        result.errors.push((action.target.clone(), e.to_string()));
                        continue;
                    }
                    if let Err(e) = self
                        .create_link(&action, app_id, version.map(String::from))
                        .await
                    {
                        result.errors.push((action.target, e.to_string()));
                    } else {
                        result.created += 1;
                    }
                }
                ConflictResolution::Rename => {
                    // Create with modified name
                    let new_target = self.get_renamed_path(&action.target);
                    let renamed_action = MappingAction {
                        target: new_target,
                        ..action
                    };
                    if let Err(e) = self
                        .create_link(&renamed_action, app_id, version.map(String::from))
                        .await
                    {
                        result.errors.push((renamed_action.target, e.to_string()));
                    } else {
                        result.created += 1;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Create a link for a mapping action.
    async fn create_link(
        &self,
        action: &MappingAction,
        app_id: &str,
        app_version: Option<String>,
    ) -> Result<()> {
        // Ensure target directory exists
        if let Some(parent) = action.target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try symlink first
        let link_type = self.create_symlink_or_copy(&action.source, &action.target)?;

        // Register the link
        let entry = create_link_entry(
            &action.model_id,
            &action.source,
            &action.target,
            link_type,
            app_id,
            app_version.as_deref(),
        );

        let registry = self.link_registry.write().await;
        registry.register(entry).await?;

        Ok(())
    }

    /// Create a symlink, falling back to hardlink or copy.
    fn create_symlink_or_copy(&self, source: &Path, target: &Path) -> Result<LinkType> {
        // Try symlink first
        #[cfg(unix)]
        {
            if std::os::unix::fs::symlink(source, target).is_ok() {
                return Ok(LinkType::Symlink);
            }
        }

        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_file(source, target).is_ok() {
                return Ok(LinkType::Symlink);
            }
        }

        // Try hardlink (only works on same filesystem)
        if std::fs::hard_link(source, target).is_ok() {
            return Ok(LinkType::Hardlink);
        }

        // Fall back to copy
        std::fs::copy(source, target)?;
        Ok(LinkType::Copy)
    }

    /// Get a renamed path to avoid conflict.
    fn get_renamed_path(&self, path: &Path) -> PathBuf {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let mut counter = 1;
        loop {
            let new_name = if ext.is_empty() {
                format!("{}_{}", stem, counter)
            } else {
                format!("{}_{}.{}", stem, counter, ext)
            };

            let new_path = path.with_file_name(new_name);
            if !new_path.exists() {
                return new_path;
            }
            counter += 1;
        }
    }

    /// Check if a model matches a mapping rule.
    fn matches_rule(&self, model: &crate::index::ModelRecord, rule: &MappingRule) -> bool {
        // Model type filter (AND logic)
        if let Some(ref types) = rule.model_types {
            if !types
                .iter()
                .any(|t| t.to_lowercase() == model.model_type.to_lowercase())
            {
                return false;
            }
        }

        // Subtype filter (AND logic)
        if let Some(ref subtypes) = rule.subtypes {
            let model_subtype = model
                .metadata
                .get("subtype")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !subtypes
                .iter()
                .any(|s| s.to_lowercase() == model_subtype.to_lowercase())
            {
                return false;
            }
        }

        // Family filter (AND logic)
        if let Some(ref families) = rule.families {
            let model_family = model
                .metadata
                .get("family")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !families
                .iter()
                .any(|f| f.to_lowercase() == model_family.to_lowercase())
            {
                return false;
            }
        }

        // Tags filter (OR logic - match any)
        if let Some(ref tags) = rule.tags {
            if !tags.is_empty() {
                let model_tags: std::collections::HashSet<_> =
                    model.tags.iter().map(|t| t.to_lowercase()).collect();
                let has_match = tags.iter().any(|t| model_tags.contains(&t.to_lowercase()));
                if !has_match {
                    return false;
                }
            }
        }

        // Exclude tags filter (AND NOT logic)
        if let Some(ref exclude_tags) = rule.exclude_tags {
            let model_tags: std::collections::HashSet<_> =
                model.tags.iter().map(|t| t.to_lowercase()).collect();
            let has_excluded = exclude_tags
                .iter()
                .any(|t| model_tags.contains(&t.to_lowercase()));
            if has_excluded {
                return false;
            }
        }

        true
    }

    /// Get model files from a model directory.
    fn get_model_files(&self, model_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in WalkDir::new(model_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let filename = entry.file_name().to_string_lossy();

            // Skip metadata files
            if filename == "metadata.json" || filename == "overrides.json" {
                continue;
            }

            // Only include model file types
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if ["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"].contains(&ext.as_str()) {
                files.push(entry.path().to_path_buf());
            }
        }

        Ok(files)
    }

    // ========================================
    // Sandbox Detection
    // ========================================

    /// Detect if running in a sandboxed environment.
    pub fn detect_sandbox() -> SandboxInfo {
        // Check for Flatpak
        if Path::new("/.flatpak-info").exists() {
            return SandboxInfo {
                sandbox_type: "flatpak".to_string(),
                is_sandboxed: true,
                required_permissions: vec![
                    "--filesystem=host".to_string(),
                    "--filesystem=/path/to/models".to_string(),
                ],
            };
        }

        // Check for Snap
        if std::env::var("SNAP").is_ok() {
            return SandboxInfo {
                sandbox_type: "snap".to_string(),
                is_sandboxed: true,
                required_permissions: vec!["personal-files".to_string()],
            };
        }

        // Check for Docker
        if Path::new("/.dockerenv").exists() {
            return SandboxInfo {
                sandbox_type: "docker".to_string(),
                is_sandboxed: true,
                required_permissions: vec!["-v /path/to/models:/models".to_string()],
            };
        }

        SandboxInfo::default()
    }

    /// Check if library and app are on the same filesystem.
    pub fn check_cross_filesystem(&self, app_models_root: &Path) -> Result<bool> {
        // On Unix, compare device IDs from filesystem metadata
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let library_stat = std::fs::metadata(self.library.library_root())?;
            let app_stat = std::fs::metadata(app_models_root).or_else(|_| {
                app_models_root
                    .parent()
                    .map(std::fs::metadata)
                    .unwrap_or_else(|| {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "Cannot determine app filesystem",
                        ))
                    })
            })?;
            Ok(library_stat.dev() != app_stat.dev())
        }

        // On Windows, compare drive letter / path prefix
        #[cfg(not(unix))]
        {
            let lib_root = self.library.library_root().components().next();
            let app_root = app_models_root.components().next();
            Ok(lib_root != app_root)
        }
    }
}

/// Result of a mapping operation.
#[derive(Debug, Clone, Default)]
pub struct MappingResult {
    /// Number of links created
    pub created: usize,
    /// Number of links skipped (already exist)
    pub skipped: usize,
    /// Number of conflicts
    pub conflicts: usize,
    /// Number of broken links removed
    pub broken_removed: usize,
    /// Errors encountered
    pub errors: Vec<(PathBuf, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::ModelRecord;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, Arc<ModelLibrary>, ModelMapper) {
        let temp_dir = TempDir::new().unwrap();
        let library_path = temp_dir.path().join("library");
        let config_path = temp_dir.path().join("config");

        std::fs::create_dir_all(&library_path).unwrap();
        std::fs::create_dir_all(&config_path).unwrap();

        let library = Arc::new(ModelLibrary::new(&library_path).await.unwrap());
        let mapper = ModelMapper::new(library.clone(), &config_path);

        (temp_dir, library, mapper)
    }

    fn create_mock_model_record(model_type: &str, subtype: &str, tags: Vec<&str>) -> ModelRecord {
        let metadata = serde_json::json!({
            "subtype": subtype,
            "family": "test"
        });

        ModelRecord {
            id: "test/model".to_string(),
            path: "/test/model".to_string(),
            cleaned_name: "test_model".to_string(),
            official_name: "Test Model".to_string(),
            model_type: model_type.to_string(),
            tags: tags.into_iter().map(String::from).collect(),
            hashes: HashMap::new(),
            metadata,
            updated_at: "2024-01-01".to_string(),
        }
    }

    #[tokio::test]
    async fn test_rule_matching_model_type() {
        let (_temp, _library, mapper) = setup().await;

        let rule = MappingRule {
            target_dir: "checkpoints".to_string(),
            model_types: Some(vec!["diffusion".to_string()]),
            subtypes: None,
            families: None,
            tags: None,
            exclude_tags: None,
        };

        let diffusion_model = create_mock_model_record("diffusion", "checkpoints", vec![]);
        let llm_model = create_mock_model_record("llm", "checkpoints", vec![]);

        assert!(mapper.matches_rule(&diffusion_model, &rule));
        assert!(!mapper.matches_rule(&llm_model, &rule));
    }

    #[tokio::test]
    async fn test_rule_matching_tags() {
        let (_temp, _library, mapper) = setup().await;

        let rule = MappingRule {
            target_dir: "loras".to_string(),
            model_types: None,
            subtypes: None,
            families: None,
            tags: Some(vec!["anime".to_string(), "realistic".to_string()]),
            exclude_tags: None,
        };

        let anime_model = create_mock_model_record("diffusion", "loras", vec!["anime"]);
        let plain_model = create_mock_model_record("diffusion", "loras", vec![]);

        assert!(mapper.matches_rule(&anime_model, &rule));
        assert!(!mapper.matches_rule(&plain_model, &rule));
    }

    #[tokio::test]
    async fn test_rule_matching_exclude_tags() {
        let (_temp, _library, mapper) = setup().await;

        let rule = MappingRule {
            target_dir: "checkpoints".to_string(),
            model_types: None,
            subtypes: None,
            families: None,
            tags: None,
            exclude_tags: Some(vec!["nsfw".to_string()]),
        };

        let safe_model = create_mock_model_record("diffusion", "checkpoints", vec!["anime"]);
        let nsfw_model = create_mock_model_record("diffusion", "checkpoints", vec!["nsfw"]);

        assert!(mapper.matches_rule(&safe_model, &rule));
        assert!(!mapper.matches_rule(&nsfw_model, &rule));
    }

    #[tokio::test]
    async fn test_save_and_load_config() {
        let (_temp, _library, mapper) = setup().await;

        let config = MappingConfig {
            app: "comfyui".to_string(),
            version: "0.6.0".to_string(),
            variant: Some("custom".to_string()),
            mappings: vec![MappingRule {
                target_dir: "checkpoints".to_string(),
                model_types: Some(vec!["diffusion".to_string()]),
                subtypes: None,
                families: None,
                tags: None,
                exclude_tags: None,
            }],
        };

        mapper.save_config(&config).unwrap();

        let loaded = mapper
            .load_config("comfyui", Some("0.6.0"))
            .unwrap()
            .unwrap();
        assert_eq!(loaded.app, "comfyui");
        assert_eq!(loaded.mappings.len(), 1);
    }

    #[test]
    fn test_sandbox_detection() {
        let sandbox = ModelMapper::detect_sandbox();
        // In a normal test environment, should not be sandboxed
        // (This might vary depending on CI environment)
        // Just verify it returns valid structure
        assert!(!sandbox.sandbox_type.is_empty());
    }

    #[tokio::test]
    async fn test_renamed_path() {
        let (_temp, _library, mapper) = setup().await;

        let path = PathBuf::from("/test/model.gguf");
        let renamed = mapper.get_renamed_path(&path);

        assert!(renamed.to_string_lossy().contains("model_1.gguf"));
    }
}
