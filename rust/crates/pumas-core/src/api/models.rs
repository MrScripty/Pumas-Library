//! Model library methods on PumasApi.

use crate::error::Result;
use crate::index::{ModelRecord, SearchResult};
use crate::model_library;
use crate::models;
use crate::PumasApi;

impl PumasApi {
    // ========================================
    // Model Library Methods
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<ModelRecord>> {
        self.primary().model_library.list_models().await
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResult> {
        self.primary().model_library.search_models(query, limit, offset).await
    }

    /// Rebuild the model index from metadata files.
    pub async fn rebuild_model_index(&self) -> Result<usize> {
        self.primary().model_library.rebuild_index().await
    }

    /// Get a single model by ID.
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        self.primary().model_library.get_model(model_id).await
    }

    /// Mark a model's metadata as manually set (protected from auto-updates).
    pub async fn mark_model_metadata_as_manual(&self, model_id: &str) -> Result<()> {
        self.primary().model_library.mark_metadata_as_manual(model_id).await
    }

    /// Get inference settings schema for a model.
    ///
    /// If the model has persisted settings, returns those. Otherwise,
    /// lazily computes defaults from the model's type and file format
    /// without persisting them.
    pub async fn get_inference_settings(
        &self,
        model_id: &str,
    ) -> Result<Vec<models::InferenceParamSchema>> {
        let library = &self.primary().model_library;
        let model_dir = library.library_root().join(model_id);

        if !model_dir.exists() {
            return Err(crate::error::PumasError::Other(format!(
                "Model not found: {}",
                model_id
            )));
        }

        let metadata = library.load_metadata(&model_dir)?.unwrap_or_default();

        if let Some(settings) = metadata.inference_settings {
            return Ok(settings);
        }

        // Lazy defaults: compute from model type + file format without persisting
        let file_format = library
            .get_primary_model_file(model_id)
            .and_then(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
            })
            .unwrap_or_default();

        Ok(models::default_inference_settings(
            metadata.model_type.as_deref().unwrap_or(""),
            &file_format,
            metadata.subtype.as_deref(),
        )
        .unwrap_or_default())
    }

    /// Replace the inference settings schema for a model.
    ///
    /// Pass an empty `Vec` to clear all settings (reverts to lazy defaults).
    pub async fn update_inference_settings(
        &self,
        model_id: &str,
        settings: Vec<models::InferenceParamSchema>,
    ) -> Result<()> {
        let library = &self.primary().model_library;
        let model_dir = library.library_root().join(model_id);

        if !model_dir.exists() {
            return Err(crate::error::PumasError::Other(format!(
                "Model not found: {}",
                model_id
            )));
        }

        let mut metadata = library.load_metadata(&model_dir)?.unwrap_or_default();

        metadata.inference_settings = if settings.is_empty() {
            None
        } else {
            Some(settings)
        };
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        library.save_metadata(&model_dir, &metadata).await?;
        library.index_model_dir(&model_dir).await?;

        Ok(())
    }

    /// Import a model from a local path.
    pub async fn import_model(
        &self,
        spec: &model_library::ModelImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        self.primary().model_importer.import(spec).await
    }

    /// Import multiple models in batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<model_library::ModelImportSpec>,
    ) -> Vec<model_library::ModelImportResult> {
        self.primary().model_importer.batch_import(specs, None).await
    }

    /// Import a model in-place (files already in library directory).
    ///
    /// Creates `metadata.json` and indexes without copying. Idempotent.
    pub async fn import_model_in_place(
        &self,
        spec: &model_library::InPlaceImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        self.primary().model_importer.import_in_place(spec).await
    }

    /// Scan for and adopt orphan model directories.
    ///
    /// Finds directories in the library with model files but no `metadata.json`,
    /// creates metadata from directory structure and file type detection, and
    /// indexes the models.
    pub async fn adopt_orphan_models(&self) -> Result<model_library::OrphanScanResult> {
        Ok(self.primary().model_importer.adopt_orphans(false).await)
    }

    // ========================================
    // Link Management
    // ========================================

    /// Get the health status of model links for a version.
    ///
    /// Returns information about total links, healthy links, broken links, etc.
    pub async fn get_link_health(&self, _version_tag: Option<&str>) -> Result<models::LinkHealthResponse> {
        let registry = self.primary().model_library.link_registry().read().await;
        let all_links = registry.get_all().await;

        let mut healthy = 0;
        let mut broken: Vec<String> = Vec::new();

        for link in &all_links {
            // Check if symlink target exists
            if link.target.is_symlink() {
                if link.source.exists() {
                    healthy += 1;
                } else {
                    broken.push(link.target.to_string_lossy().to_string());
                }
            } else if link.target.exists() {
                // Hardlink or copy - just check if target exists
                healthy += 1;
            } else {
                broken.push(link.target.to_string_lossy().to_string());
            }
        }

        Ok(models::LinkHealthResponse {
            success: true,
            error: None,
            status: if broken.is_empty() { "healthy".to_string() } else { "degraded".to_string() },
            total_links: all_links.len(),
            healthy_links: healthy,
            broken_links: broken,
            orphaned_links: vec![],
            warnings: vec![],
            errors: vec![],
        })
    }

    /// Clean up broken model links.
    ///
    /// Returns the number of broken links that were removed.
    pub async fn clean_broken_links(&self) -> Result<models::CleanBrokenLinksResponse> {
        let registry = self.primary().model_library.link_registry().write().await;
        let broken = registry.cleanup_broken().await?;

        // Also remove the actual broken symlinks from the filesystem
        for entry in &broken {
            if entry.target.exists() || entry.target.is_symlink() {
                let _ = std::fs::remove_file(&entry.target);
            }
        }

        Ok(models::CleanBrokenLinksResponse {
            success: true,
            cleaned: broken.len(),
        })
    }

    /// Get all links for a specific model.
    pub async fn get_links_for_model(&self, model_id: &str) -> Result<models::LinksForModelResponse> {
        let registry = self.primary().model_library.link_registry().read().await;
        let links = registry.get_links_for_model(model_id).await;

        let link_info: Vec<models::LinkInfo> = links
            .into_iter()
            .map(|l| models::LinkInfo {
                source: l.source.to_string_lossy().to_string(),
                target: l.target.to_string_lossy().to_string(),
                link_type: format!("{:?}", l.link_type).to_lowercase(),
                app_id: l.app_id,
                app_version: l.app_version,
                created_at: l.created_at,
            })
            .collect();

        Ok(models::LinksForModelResponse {
            success: true,
            links: link_info,
        })
    }

    /// Delete a model and cascade delete all its links.
    pub async fn delete_model_with_cascade(&self, model_id: &str) -> Result<models::DeleteModelResponse> {
        self.primary().model_library.delete_model(model_id, true).await?;
        Ok(models::DeleteModelResponse {
            success: true,
            error: None,
        })
    }

    /// Preview model mapping for a version without applying it.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path,
    /// typically obtained as `version_dir.join("models")` from pumas-app-manager.
    pub async fn preview_model_mapping(
        &self,
        version_tag: &str,
        models_path: &std::path::Path,
    ) -> Result<models::MappingPreviewResponse> {
        if !models_path.exists() {
            return Ok(models::MappingPreviewResponse {
                success: false,
                error: Some(format!("Version models directory not found: {}", models_path.display())),
                preview: None,
            });
        }

        let preview = self.primary().model_mapper.preview_mapping("comfyui", Some(version_tag), models_path).await?;

        Ok(models::MappingPreviewResponse {
            success: true,
            error: None,
            preview: Some(models::MappingPreviewData {
                creates: preview.creates.len(),
                skips: preview.skips.len(),
                conflicts: preview.conflicts.len(),
                broken: preview.broken.len(),
            }),
        })
    }

    /// Apply model mapping for a version.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path,
    /// typically obtained as `version_dir.join("models")` from pumas-app-manager.
    pub async fn apply_model_mapping(
        &self,
        version_tag: &str,
        models_path: &std::path::Path,
    ) -> Result<models::MappingApplyResponse> {
        if !models_path.exists() {
            std::fs::create_dir_all(models_path)?;
        }

        let result = self.primary().model_mapper.apply_mapping("comfyui", Some(version_tag), models_path).await?;

        Ok(models::MappingApplyResponse {
            success: true,
            error: None,
            created: result.created,
            updated: 0,
            errors: result.errors.iter().map(|(p, e)| format!("{}: {}", p.display(), e)).collect(),
        })
    }

    /// Perform incremental sync of models for a version.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path.
    pub async fn sync_models_incremental(
        &self,
        version_tag: &str,
        models_path: &std::path::Path,
    ) -> Result<models::SyncModelsResponse> {
        // Incremental sync is essentially the same as apply_mapping
        // but we could add additional logic here for detecting changes
        let result = self.apply_model_mapping(version_tag, models_path).await?;

        Ok(models::SyncModelsResponse {
            success: result.success,
            error: result.error,
            synced: result.created,
            errors: result.errors,
        })
    }

    /// Reclassify a single model (re-detect type and relocate directory if needed).
    pub async fn reclassify_model(&self, model_id: &str) -> Result<Option<String>> {
        self.primary().model_library.reclassify_model(model_id).await
    }

    /// Reclassify all models in the library (re-detect types and relocate directories).
    pub async fn reclassify_all_models(&self) -> Result<model_library::ReclassifyResult> {
        self.primary().model_library.reclassify_all_models().await
    }
}
