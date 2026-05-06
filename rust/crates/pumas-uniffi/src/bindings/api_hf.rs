use super::{
    validate_existing_local_directory_path_string, validate_existing_local_file_path_string,
    FfiDownloadRequest, FfiError, FfiHfMetadataResult, FfiHuggingFaceModel, FfiInterruptedDownload,
    FfiModelDownloadProgress, FfiPumasApi, FfiRepoFileTree,
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
        let models = self
            .primary()
            .search_hf_models(&query, kind.as_deref(), limit as usize)
            .await
            .map_err(FfiError::from)?;
        Ok(models.into_iter().map(FfiHuggingFaceModel::from).collect())
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(&self, request: FfiDownloadRequest) -> Result<String, FfiError> {
        let core_req = request.into_core()?;
        self.primary()
            .start_hf_download(&core_req)
            .await
            .map_err(FfiError::from)
    }

    /// Get the progress of an active HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: String,
    ) -> Option<FfiModelDownloadProgress> {
        let progress = self.primary().get_hf_download_progress(&download_id).await;
        progress.map(FfiModelDownloadProgress::from)
    }

    /// Cancel an active HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: String) -> Result<bool, FfiError> {
        self.primary()
            .cancel_hf_download(&download_id)
            .await
            .map_err(FfiError::from)
    }

    /// List interrupted downloads that lost their persistence state.
    pub async fn list_interrupted_downloads(&self) -> Vec<FfiInterruptedDownload> {
        let downloads = self.primary().list_interrupted_downloads().await;
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
        let dest_dir = validate_existing_local_directory_path_string(dest_dir, "dest_dir").await?;
        self.primary()
            .recover_download(&repo_id, &dest_dir)
            .await
            .map_err(FfiError::from)
    }

    /// Look up HuggingFace metadata for a local model file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: String,
    ) -> Result<Option<FfiHfMetadataResult>, FfiError> {
        let file_path = validate_existing_local_file_path_string(file_path, "file_path").await?;
        let result = self
            .primary()
            .lookup_hf_metadata_for_file(&file_path)
            .await
            .map_err(FfiError::from)?;
        Ok(result.map(FfiHfMetadataResult::from))
    }

    /// Get the file tree for a HuggingFace repository.
    pub async fn get_hf_repo_files(&self, repo_id: String) -> Result<FfiRepoFileTree, FfiError> {
        let tree = self
            .primary()
            .get_hf_repo_files(&repo_id)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiRepoFileTree::from(tree))
    }
}
