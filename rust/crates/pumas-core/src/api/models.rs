//! Model library methods on PumasApi.

use super::{reconcile_on_demand, reconcile_required_model_index, ReconcileScope};
use crate::error::{PumasError, Result};
use crate::index::{ModelRecord, SearchResult};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use serde_json::Value;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| crate::error::PumasError::io_with_path(err, path))
}

async fn validate_existing_local_file_path(file_path: &str) -> Result<std::path::PathBuf> {
    let trimmed = file_path.trim();
    if trimmed.is_empty() {
        return Err(PumasError::InvalidParams {
            message: "file_path is required".to_string(),
        });
    }

    let path = std::path::PathBuf::from(trimmed);
    let canonical = fs::canonicalize(&path)
        .await
        .map_err(|source| match source.kind() {
            ErrorKind::NotFound => PumasError::InvalidParams {
                message: format!("file_path not found: {}", path.display()),
            },
            _ => PumasError::io_with_path(source, &path),
        })?;

    let metadata = fs::metadata(&canonical)
        .await
        .map_err(|source| PumasError::io_with_path(source, &canonical))?;
    if metadata.is_file() {
        Ok(canonical)
    } else {
        Err(PumasError::InvalidParams {
            message: format!("file_path must reference a file: {}", canonical.display()),
        })
    }
}

pub(crate) async fn validate_existing_local_directory_path(
    dir_path: &str,
) -> Result<std::path::PathBuf> {
    let trimmed = dir_path.trim();
    if trimmed.is_empty() {
        return Err(PumasError::InvalidParams {
            message: "model_dir is required".to_string(),
        });
    }

    let path = std::path::PathBuf::from(trimmed);
    let canonical = fs::canonicalize(&path)
        .await
        .map_err(|source| match source.kind() {
            ErrorKind::NotFound => PumasError::InvalidParams {
                message: format!("model_dir not found: {}", path.display()),
            },
            _ => PumasError::io_with_path(source, &path),
        })?;

    let metadata = fs::metadata(&canonical)
        .await
        .map_err(|source| PumasError::io_with_path(source, &canonical))?;
    if metadata.is_dir() {
        Ok(canonical)
    } else {
        Err(PumasError::InvalidParams {
            message: format!(
                "model_dir must reference a directory: {}",
                canonical.display()
            ),
        })
    }
}

async fn load_model_metadata_or_default(
    library: Arc<model_library::ModelLibrary>,
    model_dir: std::path::PathBuf,
) -> Result<models::ModelMetadata> {
    tokio::task::spawn_blocking(move || Ok(library.load_metadata(&model_dir)?.unwrap_or_default()))
        .await
        .map_err(|err| {
            PumasError::Other(format!("Failed to join model metadata load task: {}", err))
        })?
}

async fn load_inference_snapshot(
    library: Arc<model_library::ModelLibrary>,
    model_dir: std::path::PathBuf,
    model_id: String,
) -> Result<(models::ModelMetadata, String)> {
    tokio::task::spawn_blocking(move || {
        let metadata = library.load_metadata(&model_dir)?.unwrap_or_default();
        let file_format = library
            .get_primary_model_file(&model_id)
            .and_then(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase())
            })
            .unwrap_or_default();
        Ok((metadata, file_format))
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join inference metadata snapshot task: {}",
            err
        ))
    })?
}

async fn load_effective_model_metadata(
    library: Arc<model_library::ModelLibrary>,
    model_id: String,
) -> Result<Option<models::ModelMetadata>> {
    tokio::task::spawn_blocking(move || library.get_effective_metadata(&model_id))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join effective model metadata task: {}",
                err
            ))
        })?
}

async fn load_model_count(library: Arc<model_library::ModelLibrary>) -> Result<usize> {
    tokio::task::spawn_blocking(move || library.model_count())
        .await
        .map_err(|err| PumasError::Other(format!("Failed to join model count task: {}", err)))?
}

async fn load_inference_settings_for_model(
    library: Arc<model_library::ModelLibrary>,
    model_id: String,
) -> Result<Vec<models::InferenceParamSchema>> {
    let model_dir = library.library_root().join(&model_id);

    if !path_exists(&model_dir).await? {
        return Err(crate::error::PumasError::Other(format!(
            "Model not found: {}",
            model_id
        )));
    }

    let (metadata, file_format) = load_inference_snapshot(library, model_dir, model_id).await?;

    if let Some(settings) = metadata.inference_settings {
        return Ok(settings);
    }

    Ok(models::resolve_inference_settings(&metadata, &file_format).unwrap_or_default())
}

impl PumasApi {
    // ========================================
    // Model Library Methods
    // ========================================

    /// List models. CatalogQuery reads the acknowledged index only; Full reconciles.
    pub async fn list_models(&self) -> Result<Vec<ModelRecord>> {
        if let crate::ApiInner::Catalog(state) = &self.inner {
            let crate::CatalogQueryResponse::List(result) =
                state.query(crate::CatalogQueryRequest::List).await?
            else {
                unreachable!()
            };
            return Ok(result);
        }
        self.try_primary()?;
        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::AllModels,
            "api-list-models",
        )
        .await?;
        primary.model_library.list_models().await
    }

    /// Search models. Full uses FTS; CatalogQuery uses literal case-insensitive
    /// substring matching of indexed ID, names, type and tags, without reconciliation.
    pub async fn search_models(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResult> {
        if let crate::ApiInner::Catalog(state) = &self.inner {
            let crate::CatalogQueryResponse::Search(result) = state
                .query(crate::CatalogQueryRequest::Search {
                    query: query.into(),
                    limit,
                    offset,
                })
                .await?
            else {
                unreachable!()
            };
            return Ok(result);
        }
        self.try_primary()?;
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
        self.try_primary()?;
        let primary = self.primary();
        reconcile_required_model_index(primary.as_ref(), "api-rebuild-model-index").await?;
        load_model_count(primary.model_library.clone()).await
    }

    /// Get model-library status information for GUI polling.
    pub async fn get_library_status(&self) -> Result<models::LibraryStatusResponse> {
        self.try_primary()?;
        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::AllModels,
            "api-get-library-status",
        )
        .await?;

        let model_count = load_model_count(primary.model_library.clone()).await? as u32;
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
    pub async fn validate_file_type(
        &self,
        file_path: &str,
    ) -> Result<models::FileTypeValidationResponse> {
        self.try_primary()?;
        let path = match validate_existing_local_file_path(file_path).await {
            Ok(path) => path,
            Err(err) => {
                return Ok(models::FileTypeValidationResponse {
                    success: false,
                    error: Some(err.to_string()),
                    valid: false,
                    detected_type: "error".to_string(),
                });
            }
        };
        tokio::task::spawn_blocking(move || {
            let response = match model_library::identify_model_type(&path) {
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
            };

            Ok(response)
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join validate_file_type task: {}", e)))?
    }

    /// Get a single model by ID.
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        if let crate::ApiInner::Catalog(state) = &self.inner {
            let crate::CatalogQueryResponse::Get(result) = state
                .query(crate::CatalogQueryRequest::Get {
                    model_id: model_id.into(),
                })
                .await?
            else {
                unreachable!()
            };
            return Ok(result);
        }
        self.try_primary()?;
        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::Model(model_id.to_string()),
            "api-get-model",
        )
        .await?;
        primary.model_library.get_model(model_id).await
    }

    /// Observe a canonical ID and report a reclassification performed by this read.
    ///
    /// Requires the Full instance profile; indexed CatalogQuery reads retain `get_model`.
    /// Existing `get_model` semantics are unchanged. A replacement is an explicit
    /// selection refresh hint, not an alias. Missing means no indexed row at this path;
    /// earlier moves are not reconstructed. Overlapping reconciliation returns a
    /// typed conflict rather than an ambiguous missing result.
    pub async fn lookup_model(&self, model_id: &str) -> Result<models::ModelLookupReport> {
        if model_id.len() > 4096 || !crate::intent::valid_relative_identity(model_id) {
            return Err(PumasError::InvalidParams {
                message: "model_id must be a canonical relative model path".into(),
            });
        }
        let primary = self.try_primary()?;
        let changes = super::reconciliation::reconcile_model_lookup(primary, model_id).await?;
        let resolution = if let Some((_, replacement_model_id)) = changes
            .into_iter()
            .find(|(from, to)| from == model_id && from != to)
        {
            models::ModelLookupResolution::Reclassified {
                replacement_model_id,
            }
        } else if primary.model_library.get_model(model_id).await?.is_some() {
            models::ModelLookupResolution::Found
        } else {
            models::ModelLookupResolution::Missing
        };
        Ok(models::ModelLookupReport {
            contract_version: models::MODEL_LOOKUP_CONTRACT_VERSION,
            requested_model_id: model_id.to_string(),
            resolution,
        })
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
        self.try_primary()?;
        load_inference_settings_for_model(
            self.primary().model_library.clone(),
            model_id.to_string(),
        )
        .await
    }

    /// Get inference settings schemas for multiple models without calling the
    /// public single-model API per row.
    pub async fn get_inference_settings_batch(
        &self,
        model_ids: Vec<String>,
    ) -> Result<Vec<models::ModelInferenceSettingsBatchItem>> {
        self.try_primary()?;
        let library = self.primary().model_library.clone();
        let mut items = Vec::with_capacity(model_ids.len());
        for model_id in model_ids {
            match load_inference_settings_for_model(library.clone(), model_id.clone()).await {
                Ok(settings) => items.push(models::ModelInferenceSettingsBatchItem {
                    model_id,
                    settings,
                    error: None,
                }),
                Err(err) => items.push(models::ModelInferenceSettingsBatchItem {
                    model_id,
                    settings: Vec::new(),
                    error: Some(err.to_string()),
                }),
            }
        }
        Ok(items)
    }

    /// Replace the inference settings schema for a model.
    ///
    /// Pass an empty `Vec` to clear all settings (reverts to lazy defaults).
    pub async fn update_inference_settings(
        &self,
        model_id: &str,
        settings: Vec<models::InferenceParamSchema>,
    ) -> Result<()> {
        self.try_primary()?;
        let library = self.primary().model_library.clone();
        let model_dir = library.library_root().join(model_id);

        if !path_exists(&model_dir).await? {
            return Err(crate::error::PumasError::Other(format!(
                "Model not found: {}",
                model_id
            )));
        }

        let mut metadata =
            load_model_metadata_or_default(library.clone(), model_dir.clone()).await?;

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

    /// Update user-authored markdown notes for a model.
    pub async fn update_model_notes(
        &self,
        model_id: &str,
        notes: Option<String>,
    ) -> Result<models::UpdateModelNotesResponse> {
        self.try_primary()?;
        let library = self.primary().model_library.clone();
        let model_dir = library.library_root().join(model_id);

        if !path_exists(&model_dir).await? {
            return Ok(models::UpdateModelNotesResponse {
                success: false,
                error: Some(format!("Model not found: {}", model_id)),
                model_id: model_id.to_string(),
                notes: None,
            });
        }

        let mut metadata =
            load_model_metadata_or_default(library.clone(), model_dir.clone()).await?;
        let normalized_notes = notes.and_then(|value| {
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        });
        metadata.notes = normalized_notes.clone();
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        library.save_metadata(&model_dir, &metadata).await?;
        library.index_model_dir(&model_dir).await?;

        Ok(models::UpdateModelNotesResponse {
            success: true,
            error: None,
            model_id: model_id.to_string(),
            notes: normalized_notes,
        })
    }

    /// Resolve deterministic dependency requirements for a model in a specific runtime context.
    pub async fn resolve_model_dependency_requirements(
        &self,
        model_id: &str,
        platform_context: &str,
        backend_key: Option<&str>,
    ) -> Result<model_library::ModelDependencyRequirementsResolution> {
        self.try_primary()?;
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
        self.try_primary()?;
        self.primary()
            .model_library
            .resolve_model_execution_descriptor(model_id)
            .await
    }

    /// Resolve a selected model artifact into an approved runtime load target.
    pub async fn resolve_model_artifact_load_target(
        &self,
        request: models::ResolveModelArtifactLoadTargetRequest,
    ) -> Result<models::ResolveModelArtifactLoadTargetResponse> {
        self.try_primary()?;
        self.try_primary()?
            .model_library
            .resolve_model_artifact_load_target(request)
            .await
    }

    /// Resolve cheap execution descriptors for multiple models.
    pub async fn resolve_model_execution_descriptors_batch(
        &self,
        model_ids: Vec<String>,
    ) -> Result<Vec<models::ModelExecutionDescriptorBatchItem>> {
        self.try_primary()?;
        self.primary()
            .model_library
            .resolve_model_execution_descriptors_batch(&model_ids)
            .await
    }

    /// Resolve package facts for a model on demand.
    pub async fn resolve_model_package_facts(
        &self,
        model_id: &str,
    ) -> Result<models::ResolvedModelPackageFacts> {
        self.try_primary()?;
        self.primary()
            .model_library
            .resolve_model_package_facts(model_id)
            .await
    }

    /// List model-library updates after an optional producer cursor.
    pub async fn list_model_library_updates_since(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<models::ModelLibraryUpdateFeed> {
        self.try_primary()?;
        self.primary()
            .model_library
            .list_model_library_updates_since(cursor, limit)
            .await
    }

    /// Recover model-library updates after a snapshot cursor before live delivery.
    pub async fn subscribe_model_library_updates_since(
        &self,
        cursor: &str,
    ) -> Result<models::ModelLibraryUpdateSubscription> {
        self.try_primary()?;
        self.try_primary()?
            .model_library
            .subscribe_model_library_updates_since(cursor)
            .await
    }

    /// Subscribe to model-library updates from a snapshot cursor on the owning instance.
    pub async fn subscribe_model_library_update_stream_since(
        &self,
        cursor: &str,
    ) -> Result<model_library::ModelLibraryUpdateSubscriber> {
        self.try_primary()?;
        self.try_primary()?
            .model_library
            .subscribe_model_library_update_stream_since(cursor)
            .await
    }

    /// Resolve a compact package-facts summary for a single model.
    pub async fn resolve_model_package_facts_summary(
        &self,
        model_id: &str,
    ) -> Result<models::ModelPackageFactsSummaryResult> {
        self.try_primary()?;
        self.primary()
            .model_library
            .resolve_model_package_facts_summary(model_id)
            .await
    }

    /// Resolve package-facts summaries for multiple models.
    pub async fn resolve_model_package_facts_summaries(
        &self,
        model_ids: Vec<String>,
    ) -> Result<Vec<models::ModelPackageFactsSummaryBatchItem>> {
        self.try_primary()?;
        self.primary()
            .model_library
            .resolve_model_package_facts_summaries(&model_ids)
            .await
    }

    /// Return a bounded startup snapshot of cached package-facts summaries.
    pub async fn model_package_facts_summary_snapshot(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<models::ModelPackageFactsSummarySnapshot> {
        self.try_primary()?;
        self.primary()
            .model_library
            .model_package_facts_summary_snapshot(limit, offset)
            .await
    }

    /// Return a direct in-process model-library selector snapshot.
    ///
    /// This intentionally does not proxy through transparent IPC. External
    /// local-client transport will be exposed through an explicit client API.
    pub async fn model_library_selector_snapshot(
        &self,
        request: models::ModelLibrarySelectorSnapshotRequest,
    ) -> Result<models::ModelLibrarySelectorSnapshot> {
        self.try_primary()?;
        self.try_primary()?
            .model_library
            .model_library_selector_snapshot(request)
            .await
    }

    /// Resolve a canonical model id or legacy local path into a Pumas model ref.
    pub async fn resolve_pumas_model_ref(&self, input: &str) -> Result<models::PumasModelRef> {
        self.try_primary()?;
        self.primary()
            .model_library
            .resolve_pumas_model_ref(input)
            .await
    }

    /// Audit dependency pin compliance across active model bindings.
    pub async fn audit_dependency_pin_compliance(
        &self,
    ) -> Result<model_library::DependencyPinAuditReport> {
        self.try_primary()?;
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
        self.try_primary()?;
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
        self.try_primary()?;
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
        self.try_primary()?;
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
        self.try_primary()?;
        let primary = self.primary();
        let _ = reconcile_on_demand(
            primary.as_ref(),
            ReconcileScope::Model(model_id.to_string()),
            "api-get-effective-metadata",
        )
        .await?;
        load_effective_model_metadata(primary.model_library.clone(), model_id.to_string()).await
    }

    /// Import a model from a local path.
    pub async fn import_model(
        &self,
        spec: &model_library::ModelImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        self.try_primary()?;
        self.primary().model_importer.import(spec).await
    }

    /// Import multiple models in batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<model_library::ModelImportSpec>,
    ) -> Vec<model_library::ModelImportResult> {
        let _ = self.primary();
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
        self.try_primary()?;
        self.primary()
            .model_importer
            .import_external_diffusers_directory(spec)
            .await
    }

    /// Classify import paths without creating any library state.
    pub async fn classify_model_import_paths(
        &self,
        paths: &[String],
    ) -> Result<Vec<model_library::ImportPathClassification>> {
        self.try_primary()?;
        let paths = paths.to_vec();
        tokio::task::spawn_blocking(move || {
            Ok(paths
                .iter()
                .map(model_library::classify_import_path)
                .collect())
        })
        .await
        .map_err(|e| {
            PumasError::Other(format!(
                "Failed to join classify_model_import_paths task: {}",
                e
            ))
        })?
    }

    /// Import a model in-place (files already in library directory).
    ///
    /// Creates `metadata.json` and indexes without copying. Idempotent.
    pub async fn import_model_in_place(
        &self,
        spec: &model_library::InPlaceImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        self.try_primary()?;
        let mut validated_spec = spec.clone();
        validated_spec.model_dir =
            validate_existing_local_directory_path(spec.model_dir.to_string_lossy().as_ref())
                .await?;

        self.primary()
            .model_importer
            .import_in_place(&validated_spec)
            .await
    }

    /// Scan for and adopt orphan model directories.
    ///
    /// Finds directories in the library with model files but no `metadata.json`,
    /// creates metadata from directory structure and file type detection, and
    /// indexes the models.
    pub async fn adopt_orphan_models(&self) -> Result<model_library::OrphanScanResult> {
        self.try_primary()?;
        Ok(self.primary().model_importer.adopt_orphans(false).await)
    }

    /// Reclassify a single model (re-detect type and relocate directory if needed).
    pub async fn reclassify_model(&self, model_id: &str) -> Result<Option<String>> {
        self.try_primary()?;
        self.primary()
            .model_library
            .reclassify_model(model_id)
            .await
    }

    /// Reclassify all models in the library (re-detect types and relocate directories).
    pub async fn reclassify_all_models(&self) -> Result<model_library::ReclassifyResult> {
        self.try_primary()?;
        self.primary().model_library.reclassify_all_models().await
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_existing_local_directory_path, validate_existing_local_file_path};
    use crate::api::reconciliation::{ReconcileIntent, StartOutcome};
    use crate::api::ReconcileScope;
    use crate::error::PumasError;
    use crate::models::ModelMetadata;
    use crate::PumasApi;
    use tempfile::TempDir;

    async fn lookup_fixture(api: &PumasApi, model_id: &str) -> Vec<u8> {
        let dir = api.primary().model_library.library_root().join(model_id);
        std::fs::create_dir_all(&dir).unwrap();
        let header = br#"{"weight":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let bytes = [
            (header.len() as u64).to_le_bytes().as_slice(),
            header.as_slice(),
            1.0_f32.to_le_bytes().as_slice(),
        ]
        .concat();
        std::fs::write(dir.join("weights.safetensors"), &bytes).unwrap();
        let metadata = ModelMetadata {
            schema_version: Some(1),
            model_id: Some(model_id.into()),
            model_type: Some(model_id.split('/').next().unwrap().into()),
            family: Some("fixture".into()),
            official_name: Some("lookup".into()),
            cleaned_name: Some("lookup".into()),
            subtype: Some("stale-subtype".into()),
            ..Default::default()
        };
        api.primary()
            .model_library
            .save_metadata(&dir, &metadata)
            .await
            .unwrap();
        bytes
    }

    #[tokio::test]
    async fn lookup_model_reports_actual_reclassification_and_preserves_legacy_lookup() {
        use crate::models::ModelLookupResolution;
        let root = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(root.path(), None).await;
        let old = "llm/fixture/lookup";
        let replacement = "unknown/fixture/lookup";
        let bytes = lookup_fixture(&api, old).await;
        let report = api.lookup_model(old).await.unwrap();
        assert_eq!(report.contract_version, 1);
        assert_eq!(report.requested_model_id, old);
        assert_eq!(
            report.resolution,
            ModelLookupResolution::Reclassified {
                replacement_model_id: replacement.into()
            }
        );
        assert!(!api
            .primary()
            .model_library
            .library_root()
            .join(old)
            .exists());
        assert_eq!(
            std::fs::read(
                api.primary()
                    .model_library
                    .library_root()
                    .join(replacement)
                    .join("weights.safetensors")
            )
            .unwrap(),
            bytes
        );
        assert!(api.get_model(old).await.unwrap().is_none());
        assert!(api.get_model(replacement).await.unwrap().is_some());
        assert_eq!(
            api.lookup_model(old).await.unwrap().resolution,
            ModelLookupResolution::Missing,
            "no alias history is retained after the reporting read"
        );
    }

    #[tokio::test]
    async fn lookup_model_unchanged_reads_and_missing_are_explicit() {
        use crate::models::ModelLookupResolution;
        let root = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(root.path(), None).await;
        let id = "unknown/fixture/lookup";
        let bytes = lookup_fixture(&api, id).await;
        for _ in 0..3 {
            assert_eq!(
                api.lookup_model(id).await.unwrap().resolution,
                ModelLookupResolution::Found
            );
            assert_eq!(api.get_model(id).await.unwrap().unwrap().id, id);
        }
        assert_eq!(
            std::fs::read(
                api.primary()
                    .model_library
                    .library_root()
                    .join(id)
                    .join("weights.safetensors")
            )
            .unwrap(),
            bytes
        );
        assert_eq!(
            api.lookup_model("unknown/fixture/missing")
                .await
                .unwrap()
                .resolution,
            ModelLookupResolution::Missing
        );
        for invalid in ["", "../escape", "/absolute"] {
            assert!(matches!(
                api.lookup_model(invalid).await,
                Err(PumasError::InvalidParams { .. })
            ));
        }
    }

    #[tokio::test]
    async fn lookup_model_in_flight_returns_conflict_instead_of_missing() {
        let root = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(root.path(), None).await;
        let scope = ReconcileScope::AllModels;
        let primary = api.primary();
        let run = match primary
            .reconciliation
            .try_start(&scope, ReconcileIntent::Forced)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("fixture must admit reconciliation"),
        };
        assert!(matches!(
            api.lookup_model("unknown/fixture/missing").await,
            Err(PumasError::ModelIndexRefreshInProgress)
        ));
        run.finish_failure().await;
    }

    #[tokio::test]
    async fn lookup_model_busy_custody_is_a_conflict_without_poisoning_shutdown() {
        let root = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(root.path(), None).await;
        lookup_fixture(&api, "llm/fixture/lookup").await;
        let custody = crate::model_library::DownloadDestinationRoot::open(
            api.primary().model_library.library_root(),
        )
        .unwrap();
        let grant = custody.try_acquire_execution_grant().unwrap();
        assert!(matches!(
            api.lookup_model("llm/fixture/lookup").await,
            Err(PumasError::DownloadRootBusy)
        ));
        drop(grant);
        api.primary().runtime_tasks.shutdown_owned().await.unwrap();
    }

    #[tokio::test]
    async fn validate_existing_local_file_path_canonicalizes_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("model.gguf");
        std::fs::write(&file_path, b"gguf").unwrap();

        let validated = validate_existing_local_file_path(file_path.to_string_lossy().as_ref())
            .await
            .unwrap();

        assert_eq!(validated, file_path.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn validate_existing_local_directory_path_canonicalizes_existing_directory() {
        let temp_dir = TempDir::new().unwrap();

        let validated =
            validate_existing_local_directory_path(temp_dir.path().to_string_lossy().as_ref())
                .await
                .unwrap();

        assert_eq!(validated, temp_dir.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn get_inference_settings_batch_reports_per_model_errors() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::builder(temp_dir.path())
            .with_registry(
                crate::registry::LibraryRegistry::open_at(&temp_dir.path().join("registry.db"))
                    .unwrap(),
            )
            .with_connectivity_probe(false)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let model_id = "llm/batch/inference";
        let model_dir = api.shared_resources_dir().join("models").join(model_id);
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.gguf"), b"GGUF").unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("batch".to_string()),
            official_name: Some("inference".to_string()),
            cleaned_name: Some("inference".to_string()),
            ..Default::default()
        };
        api.primary()
            .model_library
            .save_metadata(&model_dir, &metadata)
            .await
            .unwrap();

        let items = api
            .get_inference_settings_batch(vec![
                model_id.to_string(),
                "llm/batch/missing".to_string(),
            ])
            .await
            .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].model_id, model_id);
        assert!(!items[0].settings.is_empty());
        assert!(items[0].error.is_none());
        assert!(items[1].settings.is_empty());
        assert!(items[1].error.is_some());
    }

    #[tokio::test]
    async fn rebuild_model_index_reports_in_flight_reconciliation() {
        let temp_dir = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(temp_dir.path(), None).await;
        let scope = ReconcileScope::AllModels;
        let primary = api.primary();
        let ready = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let release = std::sync::Arc::new(tokio::sync::Notify::new());
        let run = match primary
            .reconciliation
            .try_start(&scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("fresh coordinator must admit the held reconciliation"),
        };
        let holder = tokio::spawn({
            let ready = ready.clone();
            let release = release.clone();
            async move {
                ready.wait().await;
                release.notified().await;
                run.finish_failure().await;
            }
        });
        ready.wait().await;

        let error = api.rebuild_model_index().await.unwrap_err();
        assert!(matches!(error, PumasError::ModelIndexRefreshInProgress));

        release.notify_one();
        holder.await.unwrap();
    }
}
