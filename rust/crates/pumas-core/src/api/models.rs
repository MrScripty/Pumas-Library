//! Model library methods on PumasApi.

use crate::error::Result;
use crate::index::{ModelRecord, SearchResult};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
        self.primary()
            .model_library
            .search_models(query, limit, offset)
            .await
    }

    /// Rebuild the model index from metadata files.
    pub async fn rebuild_model_index(&self) -> Result<usize> {
        self.primary().model_library.rebuild_index().await
    }

    /// Get model-library status information for GUI polling.
    pub async fn get_library_status(&self) -> Result<models::LibraryStatusResponse> {
        let model_count = self.primary().model_library.list_models().await?.len() as u32;
        let pending_lookups = self
            .primary()
            .model_library
            .get_pending_lookups()
            .await?
            .len() as u32;

        Ok(models::LibraryStatusResponse {
            success: true,
            error: None,
            indexing: false,
            deep_scan_in_progress: false,
            model_count,
            pending_lookups: Some(pending_lookups),
            deep_scan_progress: None,
        })
    }

    /// Validate model file type using content detection (magic bytes/header parsing).
    pub fn validate_file_type(&self, file_path: &str) -> models::FileTypeValidationResponse {
        let path = Path::new(file_path);
        if !path.exists() || !path.is_file() {
            return models::FileTypeValidationResponse {
                success: false,
                error: Some(format!("File not found: {}", file_path)),
                valid: false,
                detected_type: "error".to_string(),
            };
        }

        match model_library::identify_model_type(path) {
            Ok(info) => {
                let detected_type = info.format.as_str().to_string();
                let valid = info.format != model_library::FileFormat::Unknown;
                models::FileTypeValidationResponse {
                    success: true,
                    error: None,
                    valid,
                    detected_type,
                }
            }
            Err(err) => models::FileTypeValidationResponse {
                success: false,
                error: Some(err.to_string()),
                valid: false,
                detected_type: "error".to_string(),
            },
        }
    }

    /// Get a single model by ID.
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        self.primary().model_library.get_model(model_id).await
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

    /// Resolve deterministic dependency requirements for a model in a specific runtime context.
    pub async fn resolve_model_dependency_requirements(
        &self,
        model_id: &str,
        platform_context: &str,
        backend_key: Option<&str>,
    ) -> Result<model_library::ModelDependencyRequirementsResolution> {
        self.primary()
            .model_library
            .resolve_model_dependency_requirements(model_id, platform_context, backend_key)
            .await
    }

    /// Audit dependency pin compliance across active model bindings.
    pub async fn audit_dependency_pin_compliance(
        &self,
    ) -> Result<model_library::DependencyPinAuditReport> {
        self.primary()
            .model_library
            .audit_dependency_pin_compliance()
            .await
    }

    /// List models that currently require metadata review.
    pub async fn list_models_needing_review(
        &self,
        filter: Option<model_library::ModelReviewFilter>,
    ) -> Result<Vec<model_library::ModelReviewItem>> {
        self.primary()
            .model_library
            .list_models_needing_review(filter)
            .await
    }

    /// Submit a metadata review patch for a model.
    pub async fn submit_model_review(
        &self,
        model_id: &str,
        patch: Value,
        reviewer: &str,
        reason: Option<&str>,
    ) -> Result<model_library::SubmitModelReviewResult> {
        self.primary()
            .model_library
            .submit_model_review(model_id, patch, reviewer, reason)
            .await
    }

    /// Reset a model's review edits to baseline metadata.
    pub async fn reset_model_review(
        &self,
        model_id: &str,
        reviewer: &str,
        reason: Option<&str>,
    ) -> Result<bool> {
        self.primary()
            .model_library
            .reset_model_review(model_id, reviewer, reason)
            .await
    }

    /// Get effective metadata (`baseline + active overlay`) for a model.
    pub async fn get_effective_model_metadata(
        &self,
        model_id: &str,
    ) -> Result<Option<models::ModelMetadata>> {
        self.primary()
            .model_library
            .get_effective_metadata(model_id)
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
        self.primary()
            .model_importer
            .batch_import(specs, None)
            .await
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
    pub async fn get_link_health(
        &self,
        _version_tag: Option<&str>,
    ) -> Result<models::LinkHealthResponse> {
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
            status: if broken.is_empty() {
                "healthy".to_string()
            } else {
                "degraded".to_string()
            },
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
    pub async fn get_links_for_model(
        &self,
        model_id: &str,
    ) -> Result<models::LinksForModelResponse> {
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
    pub async fn delete_model_with_cascade(
        &self,
        model_id: &str,
    ) -> Result<models::DeleteModelResponse> {
        self.primary()
            .model_library
            .delete_model(model_id, true)
            .await?;
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
                error: Some(format!(
                    "Version models directory not found: {}",
                    models_path.display()
                )),
                to_create: vec![],
                to_skip_exists: vec![],
                conflicts: vec![],
                broken_to_remove: vec![],
                total_actions: 0,
                warnings: vec![],
                errors: vec![],
            });
        }

        // Ensure default mapping config is up to date (idempotent write)
        let primary = self.primary();
        primary
            .model_mapper
            .create_default_comfyui_config("*", models_path)?;

        let preview = primary
            .model_mapper
            .preview_mapping("comfyui", Some(version_tag), models_path)
            .await?;

        let to_action_info = |a: &crate::model_library::MappingAction| models::MappingActionInfo {
            model_id: a.model_id.clone(),
            model_name: a.model_name.clone(),
            source_path: a.source.display().to_string(),
            target_path: a.target.display().to_string(),
            reason: a.reason.clone().unwrap_or_default(),
        };

        let to_create: Vec<_> = preview.creates.iter().map(to_action_info).collect();
        let to_skip_exists: Vec<_> = preview.skips.iter().map(to_action_info).collect();
        let conflicts: Vec<_> = preview.conflicts.iter().map(to_action_info).collect();
        let broken_to_remove: Vec<_> = preview
            .broken
            .iter()
            .map(|a| models::BrokenLinkEntry {
                target_path: a.target.display().to_string(),
                existing_target: a.source.display().to_string(),
                reason: a.reason.clone().unwrap_or_default(),
            })
            .collect();
        let total_actions = to_create.len() + broken_to_remove.len();

        Ok(models::MappingPreviewResponse {
            success: true,
            error: None,
            to_create,
            to_skip_exists,
            conflicts,
            broken_to_remove,
            total_actions,
            warnings: vec![],
            errors: vec![],
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

        // Ensure default mapping config is up to date (idempotent write)
        let primary = self.primary();
        primary
            .model_mapper
            .create_default_comfyui_config("*", models_path)?;

        let result = primary
            .model_mapper
            .apply_mapping("comfyui", Some(version_tag), models_path)
            .await?;

        Ok(models::MappingApplyResponse {
            success: true,
            error: None,
            links_created: result.created,
            links_removed: result.broken_removed,
            total_links: result.created + result.skipped,
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
            synced: result.links_created,
            errors: vec![],
        })
    }

    /// Apply model mapping with per-path conflict resolutions.
    pub async fn sync_with_resolutions(
        &self,
        version_tag: &str,
        models_path: &Path,
        resolutions: HashMap<String, model_library::ConflictResolution>,
    ) -> Result<models::SyncWithResolutionsResponse> {
        if !models_path.exists() {
            std::fs::create_dir_all(models_path)?;
        }

        let primary = self.primary();
        primary
            .model_mapper
            .create_default_comfyui_config("*", models_path)?;

        let resolution_count = |kind: model_library::ConflictResolution| {
            resolutions.values().filter(|value| **value == kind).count()
        };
        let overwrite_count = resolution_count(model_library::ConflictResolution::Overwrite);
        let rename_count = resolution_count(model_library::ConflictResolution::Rename);

        let typed_resolutions: HashMap<PathBuf, model_library::ConflictResolution> = resolutions
            .into_iter()
            .map(|(target, resolution)| (PathBuf::from(target), resolution))
            .collect();

        let result = primary
            .model_mapper
            .apply_mapping_with_resolutions(
                "comfyui",
                Some(version_tag),
                models_path,
                &typed_resolutions,
            )
            .await?;

        let errors: Vec<String> = result
            .errors
            .iter()
            .map(|(path, err)| format!("{}: {}", path.display(), err))
            .collect();
        let success = errors.is_empty();
        let error = if success {
            None
        } else {
            Some(format!("{} mapping operation(s) failed", errors.len()))
        };

        Ok(models::SyncWithResolutionsResponse {
            success,
            error,
            links_created: result.created,
            links_skipped: result.skipped + result.conflicts,
            links_renamed: rename_count,
            overwrites: overwrite_count,
            errors,
        })
    }

    /// Return whether library and app version paths are on different filesystems.
    pub fn get_cross_filesystem_warning(
        &self,
        app_models_path: &Path,
    ) -> models::CrossFilesystemWarningResponse {
        let primary = self.primary();
        let library_root = primary.model_library.library_root().display().to_string();
        let app_path = app_models_path.display().to_string();

        match primary.model_mapper.check_cross_filesystem(app_models_path) {
            Ok(cross_filesystem) if cross_filesystem => models::CrossFilesystemWarningResponse {
                success: true,
                error: None,
                cross_filesystem: true,
                library_path: Some(library_root),
                app_path: Some(app_path),
                warning: Some(
                    "Model library and app version directory are on different filesystems."
                        .to_string(),
                ),
                recommendation: Some(
                    "Prefer keeping both directories on the same filesystem for best link behavior."
                        .to_string(),
                ),
            },
            Ok(_) => models::CrossFilesystemWarningResponse {
                success: true,
                error: None,
                cross_filesystem: false,
                library_path: Some(library_root),
                app_path: Some(app_path),
                warning: None,
                recommendation: None,
            },
            Err(err) => models::CrossFilesystemWarningResponse {
                success: false,
                error: Some(err.to_string()),
                cross_filesystem: false,
                library_path: Some(library_root),
                app_path: Some(app_path),
                warning: None,
                recommendation: None,
            },
        }
    }

    /// Reclassify a single model (re-detect type and relocate directory if needed).
    pub async fn reclassify_model(&self, model_id: &str) -> Result<Option<String>> {
        self.primary()
            .model_library
            .reclassify_model(model_id)
            .await
    }

    /// Reclassify all models in the library (re-detect types and relocate directories).
    pub async fn reclassify_all_models(&self) -> Result<model_library::ReclassifyResult> {
        self.primary().model_library.reclassify_all_models().await
    }

    /// Generate a non-mutating migration dry-run report for metadata v2 cutover.
    pub async fn generate_model_migration_dry_run_report(
        &self,
    ) -> Result<model_library::MigrationDryRunReport> {
        self.primary()
            .model_library
            .generate_migration_dry_run_report_with_artifacts()
    }

    /// Execute checkpointed metadata v2 migration moves.
    pub async fn execute_model_migration(&self) -> Result<model_library::MigrationExecutionReport> {
        self.primary()
            .model_library
            .execute_migration_with_checkpoint()
            .await
    }

    /// List migration report artifacts from the report index (newest-first).
    pub async fn list_model_migration_reports(
        &self,
    ) -> Result<Vec<model_library::MigrationReportArtifact>> {
        self.primary().model_library.list_migration_reports()
    }

    /// Delete a migration report artifact pair (JSON + Markdown) and index entry.
    pub async fn delete_model_migration_report(&self, report_path: &str) -> Result<bool> {
        self.primary()
            .model_library
            .delete_migration_report(report_path)
    }

    /// Prune migration report history to `keep_latest` entries.
    pub async fn prune_model_migration_reports(&self, keep_latest: usize) -> Result<usize> {
        self.primary()
            .model_library
            .prune_migration_reports(keep_latest)
    }

    // ========================================
    // Link Exclusion
    // ========================================

    /// Toggle whether a model is excluded from app linking.
    pub fn set_model_link_exclusion(
        &self,
        model_id: &str,
        app_id: &str,
        excluded: bool,
    ) -> Result<models::BaseResponse> {
        self.primary()
            .model_library
            .index()
            .set_link_exclusion(model_id, app_id, excluded)?;
        Ok(models::BaseResponse::success())
    }

    /// Get all model IDs excluded from linking for a given app.
    pub fn get_link_exclusions(&self, app_id: &str) -> Result<models::LinkExclusionsResponse> {
        let excluded = self
            .primary()
            .model_library
            .index()
            .get_excluded_model_ids(app_id)?;
        Ok(models::LinkExclusionsResponse {
            success: true,
            error: None,
            excluded_model_ids: excluded,
        })
    }
}
