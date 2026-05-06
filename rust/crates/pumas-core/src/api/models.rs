//! Model library methods on PumasApi.

use super::{reconcile_on_demand, ReconcileScope};
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
        load_model_count(primary.model_library.clone()).await
    }

    /// Get model-library status information for GUI polling.
    pub async fn get_library_status(&self) -> Result<models::LibraryStatusResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method("get_library_status", serde_json::json!({}))
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "get_inference_settings",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

        let library = self.primary().model_library.clone();
        let model_dir = library.library_root().join(model_id);

        if !path_exists(&model_dir).await? {
            return Err(crate::error::PumasError::Other(format!(
                "Model not found: {}",
                model_id
            )));
        }

        let (metadata, file_format) =
            load_inference_snapshot(library, model_dir, model_id.to_string()).await?;

        if let Some(settings) = metadata.inference_settings {
            return Ok(settings);
        }

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
        if self.try_client().is_some() {
            let _: serde_json::Value = self
                .call_client_method(
                    "update_inference_settings",
                    serde_json::json!({
                        "model_id": model_id,
                        "settings": settings,
                    }),
                )
                .await?;
            return Ok(());
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "update_model_notes",
                    serde_json::json!({
                        "model_id": model_id,
                        "notes": notes,
                    }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resolve_model_dependency_requirements",
                    serde_json::json!({
                        "model_id": model_id,
                        "platform_context": platform_context,
                        "backend_key": backend_key,
                    }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resolve_model_execution_descriptor",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

        self.primary()
            .model_library
            .resolve_model_execution_descriptor(model_id)
            .await
    }

    /// Resolve package facts for a model on demand.
    pub async fn resolve_model_package_facts(
        &self,
        model_id: &str,
    ) -> Result<models::ResolvedModelPackageFacts> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resolve_model_package_facts",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "list_model_library_updates_since",
                    serde_json::json!({ "cursor": cursor, "limit": limit }),
                )
                .await;
        }

        self.primary()
            .model_library
            .list_model_library_updates_since(cursor, limit)
            .await
    }

    /// Resolve a compact package-facts summary for a single model.
    pub async fn resolve_model_package_facts_summary(
        &self,
        model_id: &str,
    ) -> Result<models::ModelPackageFactsSummaryResult> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resolve_model_package_facts_summary",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

        self.primary()
            .model_library
            .resolve_model_package_facts_summary(model_id)
            .await
    }

    /// Return a bounded startup snapshot of cached package-facts summaries.
    pub async fn model_package_facts_summary_snapshot(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<models::ModelPackageFactsSummarySnapshot> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "model_package_facts_summary_snapshot",
                    serde_json::json!({ "limit": limit, "offset": offset }),
                )
                .await;
        }

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
        self.try_primary()?
            .model_library
            .model_library_selector_snapshot(request)
            .await
    }

    /// Resolve a canonical model id or legacy local path into a Pumas model ref.
    pub async fn resolve_pumas_model_ref(&self, input: &str) -> Result<models::PumasModelRef> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resolve_pumas_model_ref",
                    serde_json::json!({ "input": input }),
                )
                .await;
        }

        self.primary()
            .model_library
            .resolve_pumas_model_ref(input)
            .await
    }

    /// Audit dependency pin compliance across active model bindings.
    pub async fn audit_dependency_pin_compliance(
        &self,
    ) -> Result<model_library::DependencyPinAuditReport> {
        if self.try_client().is_some() {
            return self
                .call_client_method("audit_dependency_pin_compliance", serde_json::json!({}))
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "list_models_needing_review",
                    serde_json::json!({ "filter": filter }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "submit_model_review",
                    serde_json::json!({
                        "model_id": model_id,
                        "patch": patch,
                        "reviewer": reviewer,
                        "reason": reason,
                    }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "reset_model_review",
                    serde_json::json!({
                        "model_id": model_id,
                        "reviewer": reviewer,
                        "reason": reason,
                    }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "get_effective_model_metadata",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method("import_model", serde_json::json!({ "spec": spec }))
                .await;
        }

        self.primary().model_importer.import(spec).await
    }

    /// Import multiple models in batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<model_library::ModelImportSpec>,
    ) -> Vec<model_library::ModelImportResult> {
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default(
                    "import_models_batch",
                    serde_json::json!({ "specs": specs }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "import_external_diffusers_directory",
                    serde_json::json!({ "spec": spec }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method("import_model_in_place", serde_json::json!({ "spec": spec }))
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method("adopt_orphan_models", serde_json::json!({}))
                .await;
        }

        Ok(self.primary().model_importer.adopt_orphans(false).await)
    }

    /// Reclassify a single model (re-detect type and relocate directory if needed).
    pub async fn reclassify_model(&self, model_id: &str) -> Result<Option<String>> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "reclassify_model",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

        self.primary()
            .model_library
            .reclassify_model(model_id)
            .await
    }

    /// Reclassify all models in the library (re-detect types and relocate directories).
    pub async fn reclassify_all_models(&self) -> Result<model_library::ReclassifyResult> {
        if self.try_client().is_some() {
            return self
                .call_client_method("reclassify_all_models", serde_json::json!({}))
                .await;
        }

        self.primary().model_library.reclassify_all_models().await
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_existing_local_directory_path, validate_existing_local_file_path};
    use tempfile::TempDir;

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
}
