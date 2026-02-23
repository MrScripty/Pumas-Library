//! HuggingFace methods on PumasApi.

use crate::error::{PumasError, Result};
use crate::model_library;
use crate::models;
use crate::PumasApi;

impl PumasApi {
    // ========================================
    // HuggingFace Methods
    // ========================================

    /// Search for models on HuggingFace.
    ///
    /// Uses intelligent caching to minimize API calls:
    /// - Cached results are returned immediately if fresh (< 24 hours)
    /// - Model details including download sizes are enriched from cache
    /// - Falls back to API when cache is stale or missing
    pub async fn search_hf_models(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<models::HuggingFaceModel>> {
        if let Some(ref client) = self.primary().hf_client {
            let params = model_library::HfSearchParams {
                query: query.to_string(),
                kind: kind.map(String::from),
                limit: Some(limit),
                ..Default::default()
            };
            // search() handles caching transparently
            client.search(&params).await
        } else {
            Ok(vec![])
        }
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(
        &self,
        request: &model_library::DownloadRequest,
    ) -> Result<String> {
        if let Some(ref client) = self.primary().hf_client {
            // Determine destination directory
            let model_type = request.model_type.as_deref().unwrap_or("unknown");
            let dest_dir = self.primary().model_library.build_model_path(
                model_type,
                &request.family,
                &model_library::normalize_name(&request.official_name),
            );
            client.start_download(request, &dest_dir).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Get download progress for a HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: &str,
    ) -> Option<models::ModelDownloadProgress> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_download_progress(download_id).await
        } else {
            None
        }
    }

    /// Cancel a HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: &str) -> Result<bool> {
        if let Some(ref client) = self.primary().hf_client {
            client.cancel_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// Pause a HuggingFace download, preserving the `.part` file for later resume.
    pub async fn pause_hf_download(&self, download_id: &str) -> Result<bool> {
        if let Some(ref client) = self.primary().hf_client {
            client.pause_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// Resume a paused or errored HuggingFace download.
    pub async fn resume_hf_download(&self, download_id: &str) -> Result<bool> {
        if let Some(ref client) = self.primary().hf_client {
            client.resume_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// List all HuggingFace downloads (active, paused, completed, etc.).
    pub async fn list_hf_downloads(&self) -> Vec<models::ModelDownloadProgress> {
        if let Some(ref client) = self.primary().hf_client {
            client.list_downloads().await
        } else {
            vec![]
        }
    }

    /// Refetch metadata for a library model from HuggingFace.
    ///
    /// Uses the stored `repo_id` if available, otherwise falls back to
    /// filename-based lookup via `lookup_metadata()`. Returns the updated
    /// metadata on success.
    pub async fn refetch_metadata_from_hf(
        &self,
        model_id: &str,
    ) -> Result<models::ModelMetadata> {
        let primary = self.primary();
        let library = &primary.model_library;

        // Load current metadata
        let model_dir = library.library_root().join(model_id);
        let current = library.load_metadata(&model_dir)?;

        // Determine repo_id: from stored metadata or via filename lookup
        let hf_client = primary.hf_client.as_ref().ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;

        let repo_id = current.as_ref().and_then(|m| m.repo_id.clone());

        let hf_result = if let Some(ref repo_id) = repo_id {
            // Fetch model info directly by repo_id (bypasses search cache)
            let model = hf_client.get_model_info(repo_id).await?;
            model_library::HfMetadataResult {
                repo_id: model.repo_id,
                official_name: Some(model.name),
                family: None,
                model_type: Some(model.kind),
                subtype: None,
                variant: None,
                precision: None,
                tags: vec![],
                base_model: None,
                download_url: Some(model.url),
                description: None,
                match_confidence: 1.0,
                match_method: "repo_id".to_string(),
                requires_confirmation: false,
                hash_mismatch: false,
                matched_filename: None,
                pending_full_verification: false,
                fast_hash: None,
                expected_sha256: None,
            }
        } else {
            // Fallback: use filename-based lookup
            let primary_file = library.get_primary_model_file(model_id);
            let file_path = primary_file.ok_or_else(|| PumasError::NotFound {
                resource: format!("primary model file for: {}", model_id),
            })?;
            let filename = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            hf_client
                .lookup_metadata(filename, Some(&file_path), None)
                .await?
                .ok_or_else(|| PumasError::NotFound {
                    resource: format!("HuggingFace metadata for: {}", model_id),
                })?
        };

        // Update stored metadata (force=true to bypass manual guard)
        library
            .update_metadata_from_hf(model_id, &hf_result, true)
            .await?;

        // Return the freshly-updated metadata
        let updated = library.load_metadata(&model_dir)?.unwrap_or_default();
        Ok(updated)
    }

    /// Look up HuggingFace metadata for a local file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: &str,
    ) -> Result<Option<model_library::HfMetadataResult>> {
        if let Some(ref client) = self.primary().hf_client {
            let path = std::path::Path::new(file_path);
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);
            client.lookup_metadata(filename, Some(path), None).await
        } else {
            Ok(None)
        }
    }

    /// Get repository file tree from HuggingFace.
    pub async fn get_hf_repo_files(
        &self,
        repo_id: &str,
    ) -> Result<model_library::RepoFileTree> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_repo_files(repo_id).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }
}
