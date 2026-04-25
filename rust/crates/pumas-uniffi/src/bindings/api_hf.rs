use super::{
    FfiApiInner, FfiDownloadRequest, FfiError, FfiHfMetadataResult, FfiHuggingFaceModel,
    FfiInterruptedDownload, FfiModelDownloadProgress, FfiPumasApi, FfiRepoFileTree,
};

#[uniffi::export(async_runtime = "tokio")]
impl FfiPumasApi {
    /// Search for models on HuggingFace.
    pub async fn search_hf_models(
        &self,
        query: String,
        kind: Option<String>,
        limit: u64,
    ) -> Result<Vec<FfiHuggingFaceModel>, FfiError> {
        let models = match &self.inner {
            FfiApiInner::Primary(api) => api
                .search_hf_models(&query, kind.as_deref(), limit as usize)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "search_hf_models",
                    serde_json::json!({
                        "query": query,
                        "kind": kind,
                        "limit": limit,
                    }),
                )
                .await?
            }
        };
        Ok(models.into_iter().map(FfiHuggingFaceModel::from).collect())
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(&self, request: FfiDownloadRequest) -> Result<String, FfiError> {
        let core_req = request.into_core()?;
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .start_hf_download(&core_req)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "start_hf_download",
                    serde_json::json!({ "request": core_req }),
                )
                .await
            }
        }
    }

    /// Get the progress of an active HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: String,
    ) -> Option<FfiModelDownloadProgress> {
        let progress = match &self.inner {
            FfiApiInner::Primary(api) => api.get_hf_download_progress(&download_id).await,
            FfiApiInner::Client(_) => self
                .call_client_method(
                    "get_hf_download_progress",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await
                .ok()
                .flatten(),
        };
        progress.map(FfiModelDownloadProgress::from)
    }

    /// Cancel an active HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: String) -> Result<bool, FfiError> {
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .cancel_hf_download(&download_id)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "cancel_hf_download",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await
            }
        }
    }

    /// List interrupted downloads that lost their persistence state.
    pub async fn list_interrupted_downloads(&self) -> Vec<FfiInterruptedDownload> {
        let downloads: Vec<pumas_library::model_library::InterruptedDownload> = match &self.inner {
            FfiApiInner::Primary(api) => api.list_interrupted_downloads().await,
            FfiApiInner::Client(_) => self
                .call_client_method_blocking("list_interrupted_downloads", serde_json::json!({}))
                .unwrap_or_default(),
        };
        downloads
            .into_iter()
            .map(FfiInterruptedDownload::from)
            .collect()
    }

    /// Recover an interrupted download by providing the correct repo_id.
    pub async fn recover_download(
        &self,
        repo_id: String,
        dest_dir: String,
    ) -> Result<String, FfiError> {
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .recover_download(&repo_id, &dest_dir)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "recover_download",
                    serde_json::json!({
                        "repo_id": repo_id,
                        "dest_dir": dest_dir,
                    }),
                )
                .await
            }
        }
    }

    /// Look up HuggingFace metadata for a local model file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: String,
    ) -> Result<Option<FfiHfMetadataResult>, FfiError> {
        let result = match &self.inner {
            FfiApiInner::Primary(api) => api
                .lookup_hf_metadata_for_file(&file_path)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "lookup_hf_metadata_for_file",
                    serde_json::json!({ "file_path": file_path }),
                )
                .await?
            }
        };
        Ok(result.map(FfiHfMetadataResult::from))
    }

    /// Get the file tree for a HuggingFace repository.
    pub async fn get_hf_repo_files(&self, repo_id: String) -> Result<FfiRepoFileTree, FfiError> {
        let tree = match &self.inner {
            FfiApiInner::Primary(api) => api
                .get_hf_repo_files(&repo_id)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "get_hf_repo_files",
                    serde_json::json!({ "repo_id": repo_id }),
                )
                .await?
            }
        };
        Ok(FfiRepoFileTree::from(tree))
    }
}
