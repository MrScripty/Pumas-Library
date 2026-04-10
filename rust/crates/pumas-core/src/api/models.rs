//! Model library methods on PumasApi.

use super::{reconcile_on_demand, ReconcileScope};
use crate::error::Result;
use crate::index::{ModelRecord, SearchResult};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

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
        primary.model_library.model_count()
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

        let model_count = primary.model_library.model_count()? as u32;
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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "get_inference_settings",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

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
        primary.model_library.get_effective_metadata(model_id)
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
        if self.try_client().is_some() {
            return self
                .call_client_method("import_model_in_place", serde_json::json!({ "spec": spec }))
                .await;
        }

        self.primary().model_importer.import_in_place(spec).await
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
