//! Model library methods on PumasApi.

use super::{reconcile_on_demand, ReconcileScope};
use crate::error::{PumasError, Result};
use crate::index::{ModelRecord, SearchResult};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::time::{sleep, Duration};

impl PumasApi {
    // ========================================
    // Model Library Methods
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<ModelRecord>> {
        if self.try_client().is_some() {
            return self
                .call_client_method("list_models", serde_json::json!({}))
                .await;
        }

        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::AllModels,
            "api-list-models",
        )
        .await?;
        primary.model_library.list_models().await
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResult> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "search_models",
                    serde_json::json!({
                        "query": query,
                        "limit": limit,
                        "offset": offset,
                    }),
                )
                .await;
        }

        let primary = self.primary();

        if query.trim().is_empty() {
            let _ = reconcile_on_demand(
                primary.as_ref(),
                ReconcileScope::AllModels,
                "api-search-empty-query",
            )
            .await?;
            return primary
                .model_library
                .search_models(query, limit, offset)
                .await;
        }

        let mut result = primary
            .model_library
            .search_models(query, limit, offset)
            .await?;
        let mut model_ids = HashSet::new();
        for model in &result.models {
            model_ids.insert(model.id.clone());
        }

        let mut reconciled_any = false;
        for model_id in model_ids {
            if reconcile_on_demand(
                primary.as_ref(),
                ReconcileScope::Model(model_id),
                "api-search-model-hit",
            )
            .await?
            {
                reconciled_any = true;
            }
        }

        if reconciled_any {
            result = primary
                .model_library
                .search_models(query, limit, offset)
                .await?;
        }

        Ok(result)
    }

    /// Rebuild and reconcile the model index.
    ///
    /// This forces a full-scope reconciliation pass so SQLite remains the
    /// source-of-truth for both metadata-backed models and metadata-less
    /// partial downloads staged from persisted/HF download data.
    pub async fn rebuild_model_index(&self) -> Result<usize> {
        let primary = self.primary();
        primary.reconciliation.mark_dirty_all().await;
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::AllModels,
            "api-rebuild-model-index",
        )
        .await?;
        Ok(primary.model_library.list_models().await?.len())
    }

    /// Get model-library status information for GUI polling.
    pub async fn get_library_status(&self) -> Result<models::LibraryStatusResponse> {
        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::AllModels,
            "api-get-library-status",
        )
        .await?;

        let model_count = primary.model_library.list_models().await?.len() as u32;
        let pending_lookups = primary.model_library.get_pending_lookups().await?.len() as u32;

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
        if self.try_client().is_some() {
            return self
                .call_client_method("get_model", serde_json::json!({ "model_id": model_id }))
                .await;
        }

        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::Model(model_id.to_string()),
            "api-get-model",
        )
        .await?;
        primary.model_library.get_model(model_id).await
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

        Ok(models::resolve_inference_settings(&metadata, &file_format).unwrap_or_default())
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

    /// Resolve a runtime execution descriptor for a model.
    pub async fn resolve_model_execution_descriptor(
        &self,
        model_id: &str,
    ) -> Result<models::ModelExecutionDescriptor> {
        self.primary()
            .model_library
            .resolve_model_execution_descriptor(model_id)
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
        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::Model(model_id.to_string()),
            "api-get-effective-metadata",
        )
        .await?;
        primary.model_library.get_effective_metadata(model_id)
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

    /// Register an external diffusers directory without copying its contents.
    pub async fn import_external_diffusers_directory(
        &self,
        spec: &model_library::ExternalDiffusersImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        self.primary()
            .model_importer
            .import_external_diffusers_directory(spec)
            .await
    }

    /// Classify import paths without creating any library state.
    pub fn classify_model_import_paths(
        &self,
        paths: &[String],
    ) -> Vec<model_library::ImportPathClassification> {
        paths
            .iter()
            .map(model_library::classify_import_path)
            .collect()
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
        let primary = self.primary();
        let mut report = primary
            .model_library
            .execute_migration_with_checkpoint()
            .await?;
        let mutated = relocate_skipped_partial_downloads(primary.as_ref(), &mut report).await?;
        if mutated {
            recompute_execution_report_counts(&mut report);
            // Rewrite artifacts so UI/opened report JSON reflects post-move outcomes.
            primary
                .model_library
                .rewrite_migration_execution_report(&report)?;
        }
        Ok(report)
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

fn split_model_id(model_id: &str) -> Option<(&str, &str, &str)> {
    let mut parts = model_id.splitn(3, '/');
    let model_type = parts.next()?;
    let family = parts.next()?;
    let cleaned_name = parts.next()?;
    Some((model_type, family, cleaned_name))
}

fn update_download_marker(
    target_dir: &Path,
    target_model_type: &str,
    target_family: &str,
) -> Result<()> {
    let marker_path = target_dir.join(".pumas_download");
    if !marker_path.exists() {
        return Ok(());
    }
    let marker_text = std::fs::read_to_string(&marker_path)?;
    let mut marker_json: Value =
        serde_json::from_str(&marker_text).map_err(|err| PumasError::Json {
            message: format!("Failed to parse download marker JSON: {}", err),
            source: None,
        })?;
    let Some(marker_obj) = marker_json.as_object_mut() else {
        return Err(PumasError::Validation {
            field: "download_marker".to_string(),
            message: "Expected .pumas_download to be a JSON object".to_string(),
        });
    };
    marker_obj.insert(
        "model_type".to_string(),
        Value::String(target_model_type.to_string()),
    );
    marker_obj.insert(
        "family".to_string(),
        Value::String(target_family.to_string()),
    );
    std::fs::write(&marker_path, serde_json::to_string_pretty(&marker_json)?)?;
    Ok(())
}

fn cleanup_empty_parent_dirs_after_move(source_dir: &Path, library_root: &Path) {
    let mut current = source_dir.parent();
    while let Some(dir) = current {
        if dir == library_root {
            break;
        }
        if std::fs::remove_dir(dir).is_err() {
            break;
        }
        current = dir.parent();
    }
}

async fn wait_for_download_pause(
    client: &model_library::HuggingFaceClient,
    download_id: &str,
) -> Result<()> {
    for _ in 0..80 {
        match client.get_download_status(download_id).await {
            Some(models::DownloadStatus::Paused)
            | Some(models::DownloadStatus::Error)
            | Some(models::DownloadStatus::Cancelled)
            | Some(models::DownloadStatus::Completed) => return Ok(()),
            Some(models::DownloadStatus::Downloading)
            | Some(models::DownloadStatus::Queued)
            | Some(models::DownloadStatus::Pausing)
            | Some(models::DownloadStatus::Cancelling) => {
                sleep(Duration::from_millis(250)).await;
            }
            None => {
                return Err(PumasError::NotFound {
                    resource: format!("download_id {}", download_id),
                });
            }
        }
    }

    Err(PumasError::Other(format!(
        "Timed out waiting for download {} to pause before migration move",
        download_id
    )))
}

async fn relocate_skipped_partial_downloads(
    primary: &super::state::PrimaryState,
    report: &mut model_library::MigrationExecutionReport,
) -> Result<bool> {
    let mut mutated = false;
    for row in &mut report.results {
        if row.action != "skipped_partial_download" {
            continue;
        }

        let Some((target_model_type, target_family, target_cleaned_name)) =
            split_model_id(&row.target_model_id)
        else {
            row.action = "partial_move_error".to_string();
            row.error = Some(format!("Invalid target model_id: {}", row.target_model_id));
            mutated = true;
            continue;
        };

        let source_dir = primary.model_library.library_root().join(&row.model_id);
        let target_dir = primary.model_library.build_model_path(
            target_model_type,
            target_family,
            target_cleaned_name,
        );
        if !source_dir.exists() {
            row.action = "missing_source".to_string();
            row.error = Some(format!(
                "Source directory not found: {}",
                source_dir.display()
            ));
            mutated = true;
            continue;
        }
        if target_dir.exists() {
            row.action = "blocked_collision".to_string();
            row.error = Some(format!("Target already exists: {}", target_dir.display()));
            mutated = true;
            continue;
        }

        let mut moved = false;
        let mut relocated_download_id: Option<String> = None;
        let mut resume_after_move = false;
        let mut attempted_pause = false;

        let move_result: Result<()> = async {
            let (download_id, was_active) = if let Some(ref client) = primary.hf_client {
                let persisted = client
                    .persistence()
                    .map(|p| p.load_all())
                    .unwrap_or_default();
                if let Some(entry) = persisted.iter().find(|entry| entry.dest_dir == source_dir) {
                    let download_id = entry.download_id.clone();
                    let status = client.get_download_status(&download_id).await;
                    let was_active = matches!(
                        status,
                        Some(models::DownloadStatus::Queued)
                            | Some(models::DownloadStatus::Downloading)
                            | Some(models::DownloadStatus::Pausing)
                    );
                    if was_active {
                        attempted_pause = true;
                        let _ = client.pause_download(&download_id).await?;
                        wait_for_download_pause(client, &download_id).await?;
                    }
                    (Some(download_id), was_active)
                } else {
                    (None, false)
                }
            } else {
                (None, false)
            };
            resume_after_move = was_active;
            std::fs::create_dir_all(
                target_dir
                    .parent()
                    .ok_or_else(|| PumasError::Other("Target parent missing".to_string()))?,
            )?;
            std::fs::rename(&source_dir, &target_dir)?;
            moved = true;

            update_download_marker(&target_dir, target_model_type, target_family)?;

            if let Some(download_id) = download_id {
                if let Some(ref client) = primary.hf_client {
                    client
                        .relocate_download_destination(
                            &download_id,
                            &target_dir,
                            Some(target_model_type),
                            Some(target_family),
                        )
                        .await?;
                    relocated_download_id = Some(download_id);
                }
            }

            if let Some(record) = primary.model_library.index().get(&row.model_id)? {
                let mut metadata: model_library::ModelMetadata =
                    serde_json::from_value(record.metadata.clone()).unwrap_or_default();
                metadata.model_id = Some(row.target_model_id.clone());
                metadata.model_type = Some(target_model_type.to_string());
                metadata.family = Some(target_family.to_string());
                metadata.cleaned_name = Some(target_cleaned_name.to_string());
                metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
                primary
                    .model_library
                    .upsert_index_from_metadata(&target_dir, &metadata)?;
                let _ = primary.model_library.index().delete(&row.model_id)?;
            }

            cleanup_empty_parent_dirs_after_move(&source_dir, primary.model_library.library_root());
            Ok(())
        }
        .await;

        match move_result {
            Ok(()) => {
                if resume_after_move {
                    if let (Some(client), Some(download_id)) =
                        (primary.hf_client.as_ref(), relocated_download_id.as_ref())
                    {
                        let _ = client.resume_download(download_id).await?;
                    }
                }
                row.action = "moved_partial".to_string();
                row.error = None;
                mutated = true;
            }
            Err(err) => {
                if moved && target_dir.exists() {
                    let _ = std::fs::rename(&target_dir, &source_dir);
                }
                if let (Some(client), Some(download_id)) =
                    (primary.hf_client.as_ref(), relocated_download_id.as_ref())
                {
                    let rollback_source = split_model_id(&row.model_id);
                    let _ = client
                        .relocate_download_destination(
                            download_id,
                            &source_dir,
                            rollback_source.map(|(model_type, _, _)| model_type),
                            rollback_source.map(|(_, family, _)| family),
                        )
                        .await;
                }
                if attempted_pause && resume_after_move {
                    if let (Some(client), Some(download_id)) =
                        (primary.hf_client.as_ref(), relocated_download_id.as_ref())
                    {
                        let _ = client.resume_download(download_id).await;
                    }
                }
                row.action = "partial_move_error".to_string();
                row.error = Some(err.to_string());
                mutated = true;
            }
        }
    }

    Ok(mutated)
}

fn recompute_execution_report_counts(report: &mut model_library::MigrationExecutionReport) {
    report.completed_move_count = 0;
    report.skipped_move_count = 0;
    report.error_count = 0;
    for row in &report.results {
        match row.action.as_str() {
            "moved" | "already_migrated" | "moved_partial" => report.completed_move_count += 1,
            "blocked_collision" | "missing_source" | "skipped_partial_download" => {
                report.skipped_move_count += 1
            }
            _ => report.error_count += 1,
        }
    }
    if !report.referential_integrity_ok {
        report.error_count += report.referential_integrity_errors.len();
    }
}
