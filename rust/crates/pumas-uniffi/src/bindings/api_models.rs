use super::{
    FfiApiInner, FfiDeleteModelResponse, FfiError, FfiInferenceParamSchema, FfiModelImportResult,
    FfiModelImportSpec, FfiModelRecord, FfiPumasApi, FfiReclassifyResult, FfiSearchResult,
};

#[uniffi::export(async_runtime = "tokio")]
impl FfiPumasApi {
    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<FfiModelRecord>, FfiError> {
        let models = match &self.inner {
            FfiApiInner::Primary(api) => api.list_models().await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("list_models", serde_json::json!({}))
                    .await?
            }
        };
        Ok(models.into_iter().map(FfiModelRecord::from).collect())
    }

    /// Get a single model by its ID.
    pub async fn get_model(&self, model_id: String) -> Result<Option<FfiModelRecord>, FfiError> {
        let model = match &self.inner {
            FfiApiInner::Primary(api) => api.get_model(&model_id).await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("get_model", serde_json::json!({ "model_id": model_id }))
                    .await?
            }
        };
        Ok(model.map(FfiModelRecord::from))
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: String,
        limit: u64,
        offset: u64,
    ) -> Result<FfiSearchResult, FfiError> {
        let result = match &self.inner {
            FfiApiInner::Primary(api) => api
                .search_models(&query, limit as usize, offset as usize)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "search_models",
                    serde_json::json!({
                        "query": query,
                        "limit": limit,
                        "offset": offset,
                    }),
                )
                .await?
            }
        };
        Ok(FfiSearchResult::from(result))
    }

    /// Delete a model and all its links.
    pub async fn delete_model(&self, model_id: String) -> Result<FfiDeleteModelResponse, FfiError> {
        let resp = match &self.inner {
            FfiApiInner::Primary(api) => api
                .delete_model_with_cascade(&model_id)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "delete_model_with_cascade",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await?
            }
        };
        Ok(FfiDeleteModelResponse::from(resp))
    }

    /// Import a model from a local file path.
    pub async fn import_model(
        &self,
        spec: FfiModelImportSpec,
    ) -> Result<FfiModelImportResult, FfiError> {
        let core_spec = spec.into_core()?;
        let result = match &self.inner {
            FfiApiInner::Primary(api) => {
                api.import_model(&core_spec).await.map_err(FfiError::from)?
            }
            FfiApiInner::Client(_) => {
                self.call_client_method("import_model", serde_json::json!({ "spec": core_spec }))
                    .await?
            }
        };
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
                Ok(core_spec) => core_specs.push(core_spec),
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
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .import_models_batch(core_specs)
                .await
                .into_iter()
                .map(FfiModelImportResult::from)
                .collect(),
            FfiApiInner::Client(_) => match self
                .call_client_method::<Vec<pumas_library::model_library::ModelImportResult>>(
                    "import_models_batch",
                    serde_json::json!({ "specs": core_specs.clone() }),
                )
                .await
            {
                Ok(results) => results
                    .into_iter()
                    .map(FfiModelImportResult::from)
                    .collect(),
                Err(err) => core_specs
                    .into_iter()
                    .map(|spec| {
                        FfiModelImportResult::from(
                            pumas_library::model_library::ModelImportResult {
                                path: spec.path,
                                success: false,
                                model_id: None,
                                model_path: None,
                                error: Some(err.to_string()),
                                security_tier: None,
                            },
                        )
                    })
                    .collect(),
            },
        }
    }

    /// Rebuild the full-text search index for all models.
    pub async fn rebuild_model_index(&self) -> Result<u64, FfiError> {
        let count: usize = match &self.inner {
            FfiApiInner::Primary(api) => api.rebuild_model_index().await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("rebuild_model_index", serde_json::json!({}))
                    .await?
            }
        };
        Ok(count as u64)
    }

    /// Re-detect a model's type and move it to the correct directory if misclassified.
    ///
    /// Returns the new model_id if the model was reclassified, None if unchanged.
    pub async fn reclassify_model(&self, model_id: String) -> Result<Option<String>, FfiError> {
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .reclassify_model(&model_id)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "reclassify_model",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await
            }
        }
    }

    /// Re-detect and reclassify all models in the library.
    ///
    /// Scans every model, re-detects its type from file content, and moves
    /// any misclassified models to the correct directory.
    pub async fn reclassify_all_models(&self) -> Result<FfiReclassifyResult, FfiError> {
        let result = match &self.inner {
            FfiApiInner::Primary(api) => {
                api.reclassify_all_models().await.map_err(FfiError::from)?
            }
            FfiApiInner::Client(_) => {
                self.call_client_method("reclassify_all_models", serde_json::json!({}))
                    .await?
            }
        };
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
        let settings = match &self.inner {
            FfiApiInner::Primary(api) => api
                .get_inference_settings(&model_id)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "get_inference_settings",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await?
            }
        };
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
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .update_inference_settings(&model_id, core_settings)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                let _: serde_json::Value = self
                    .call_client_method(
                        "update_inference_settings",
                        serde_json::json!({
                            "model_id": model_id,
                            "settings": core_settings,
                        }),
                    )
                    .await?;
                Ok(())
            }
        }
    }
}
