use super::{
    canonicalize_existing_local_path_string, FfiDeleteModelResponse, FfiError,
    FfiInferenceParamSchema, FfiModelImportResult, FfiModelImportSpec, FfiModelRecord, FfiPumasApi,
    FfiReclassifyResult, FfiSearchResult,
};

#[uniffi::export(async_runtime = "tokio")]
impl FfiPumasApi {
    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<FfiModelRecord>, FfiError> {
        let models = self.primary().list_models().await.map_err(FfiError::from)?;
        Ok(models.into_iter().map(FfiModelRecord::from).collect())
    }

    /// Get a single model by its ID.
    pub async fn get_model(&self, model_id: String) -> Result<Option<FfiModelRecord>, FfiError> {
        let model = self
            .primary()
            .get_model(&model_id)
            .await
            .map_err(FfiError::from)?;
        Ok(model.map(FfiModelRecord::from))
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: String,
        limit: u64,
        offset: u64,
    ) -> Result<FfiSearchResult, FfiError> {
        let result = self
            .primary()
            .search_models(&query, limit as usize, offset as usize)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiSearchResult::from(result))
    }

    /// Delete a model and all its links.
    pub async fn delete_model(&self, model_id: String) -> Result<FfiDeleteModelResponse, FfiError> {
        let resp = self
            .primary()
            .delete_model_with_cascade(&model_id)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiDeleteModelResponse::from(resp))
    }

    /// Import a model from a local file path.
    pub async fn import_model(
        &self,
        spec: FfiModelImportSpec,
    ) -> Result<FfiModelImportResult, FfiError> {
        let mut core_spec = spec.into_core()?;
        core_spec.path = canonicalize_existing_local_path_string(core_spec.path, "path").await?;
        let result = self
            .primary()
            .import_model(&core_spec)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiModelImportResult::from(result))
    }

    /// Import multiple models in a batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<FfiModelImportSpec>,
    ) -> Vec<FfiModelImportResult> {
        let mut core_specs = Vec::with_capacity(specs.len());
        for spec in specs {
            match spec.into_core() {
                Ok(mut core_spec) => {
                    match canonicalize_existing_local_path_string(core_spec.path, "path").await {
                        Ok(path) => {
                            core_spec.path = path;
                            core_specs.push(core_spec);
                        }
                        Err(err) => {
                            return vec![FfiModelImportResult {
                                path: String::new(),
                                success: false,
                                model_path: None,
                                error: Some(err.to_string()),
                                security_tier: None,
                            }];
                        }
                    }
                }
                Err(err) => {
                    return vec![FfiModelImportResult {
                        path: String::new(),
                        success: false,
                        model_path: None,
                        error: Some(err.to_string()),
                        security_tier: None,
                    }];
                }
            }
        }
        self.primary()
            .import_models_batch(core_specs)
            .await
            .into_iter()
            .map(FfiModelImportResult::from)
            .collect()
    }

    /// Rebuild the full-text search index for all models.
    pub async fn rebuild_model_index(&self) -> Result<u64, FfiError> {
        let count = self
            .primary()
            .rebuild_model_index()
            .await
            .map_err(FfiError::from)?;
        Ok(count as u64)
    }

    /// Re-detect a model's type and move it to the correct directory if misclassified.
    ///
    /// Returns the new model_id if the model was reclassified, None if unchanged.
    pub async fn reclassify_model(&self, model_id: String) -> Result<Option<String>, FfiError> {
        self.primary()
            .reclassify_model(&model_id)
            .await
            .map_err(FfiError::from)
    }

    /// Re-detect and reclassify all models in the library.
    ///
    /// Scans every model, re-detects its type from file content, and moves
    /// any misclassified models to the correct directory.
    pub async fn reclassify_all_models(&self) -> Result<FfiReclassifyResult, FfiError> {
        let result = self
            .primary()
            .reclassify_all_models()
            .await
            .map_err(FfiError::from)?;
        Ok(FfiReclassifyResult::from(result))
    }

    /// Get the inference settings schema for a model.
    ///
    /// Returns the stored settings if present, otherwise lazily computes
    /// defaults based on model type and format.
    pub async fn get_inference_settings(
        &self,
        model_id: String,
    ) -> Result<Vec<FfiInferenceParamSchema>, FfiError> {
        let settings = self
            .primary()
            .get_inference_settings(&model_id)
            .await
            .map_err(FfiError::from)?;
        Ok(settings
            .into_iter()
            .map(FfiInferenceParamSchema::from)
            .collect())
    }

    /// Replace the inference settings schema for a model.
    pub async fn update_inference_settings(
        &self,
        model_id: String,
        settings: Vec<FfiInferenceParamSchema>,
    ) -> Result<(), FfiError> {
        let core_settings: Vec<pumas_library::models::InferenceParamSchema> =
            settings.into_iter().map(Into::into).collect();
        self.primary()
            .update_inference_settings(&model_id, core_settings)
            .await
            .map_err(FfiError::from)
    }
}
