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
            // Determine destination directory.
            // Normalize through ModelType to handle raw pipeline_tags (e.g. "text-to-audio" → "audio").
            let model_type_raw = request.model_type.as_deref().unwrap_or("unknown");
            let model_type_parsed: crate::model_library::ModelType =
                model_type_raw.parse().unwrap_or(crate::model_library::ModelType::Unknown);
            let model_type = model_type_parsed.as_str();
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

    /// List directories with interrupted downloads (`.part` files) that have
    /// no download persistence entry and no metadata.
    ///
    /// These are downloads that lost their tracking state (e.g. due to crash).
    /// Use `recover_download()` with the correct repo_id to resume them.
    pub fn list_interrupted_downloads(&self) -> Vec<model_library::InterruptedDownload> {
        let primary = self.primary();

        // Collect dest_dirs of all known persisted downloads
        let known_dirs: std::collections::HashSet<std::path::PathBuf> =
            if let Some(ref client) = primary.hf_client {
                if let Some(persistence) = client.persistence() {
                    persistence
                        .load_all()
                        .into_iter()
                        .map(|e| e.dest_dir)
                        .collect()
                } else {
                    std::collections::HashSet::new()
                }
            } else {
                std::collections::HashSet::new()
            };

        primary
            .model_importer
            .find_interrupted_downloads(&known_dirs)
    }

    /// Recover an interrupted download that lost its persistence state.
    ///
    /// Given the correct `repo_id` and the `dest_dir` path where the partial
    /// download exists, starts a new download targeting that directory. The
    /// download system handles `.part` file resume via HTTP Range headers and
    /// skips files that are already complete.
    pub async fn recover_download(
        &self,
        repo_id: &str,
        dest_dir: &str,
    ) -> Result<String> {
        let dest = std::path::Path::new(dest_dir);
        if !dest.is_dir() {
            return Err(PumasError::NotFound {
                resource: format!("directory: {}", dest_dir),
            });
        }

        let primary = self.primary();
        let client = primary.hf_client.as_ref().ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;

        // Parse repo_id into family/name
        let parts: Vec<&str> = repo_id.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(PumasError::Config {
                message: format!("Invalid repo_id format (expected 'owner/name'): {}", repo_id),
            });
        }
        let family = parts[0];
        let official_name = parts[1];

        // Determine model_type from directory path relative to library root
        let library_root = primary.model_library.library_root();
        let model_type = dest
            .strip_prefix(library_root)
            .ok()
            .and_then(|rel| rel.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .map(String::from);

        let request = model_library::DownloadRequest {
            repo_id: repo_id.to_string(),
            family: family.to_string(),
            official_name: official_name.to_string(),
            model_type,
            quant: None,
            filename: None,
            pipeline_tag: None,
        };

        client.start_download(&request, dest).await
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
        let hf_client = primary.hf_client.as_ref().ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;

        // Handle download-in-progress models: extract repo_id and fetch directly
        if let Some(repo_id) = model_id.strip_prefix("download:") {
            let model = hf_client.get_model_info(repo_id).await?;
            return Ok(models::ModelMetadata {
                repo_id: Some(model.repo_id),
                official_name: Some(model.name),
                model_type: Some(model.kind),
                download_url: Some(model.url),
                match_source: Some("hf".to_string()),
                match_method: Some("repo_id".to_string()),
                match_confidence: Some(1.0),
                ..Default::default()
            });
        }

        let library = &primary.model_library;

        // Load current metadata
        let model_dir = library.library_root().join(model_id);
        let current = library.load_metadata(&model_dir)?;

        let repo_id = current
            .as_ref()
            .and_then(|m| m.repo_id.clone())
            .or_else(|| {
                // model_id is "{type}/{owner}/{name}" — extract "{owner}/{name}" as repo_id
                let parts: Vec<&str> = model_id.splitn(3, '/').collect();
                if parts.len() == 3 {
                    Some(format!("{}/{}", parts[1], parts[2]))
                } else {
                    None
                }
            });

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

    // ========================================
    // HuggingFace Authentication
    // ========================================

    /// Set the HuggingFace authentication token.
    ///
    /// Persists to disk and updates the in-memory token for immediate use.
    pub async fn set_hf_token(&self, token: &str) -> Result<()> {
        if let Some(ref client) = self.primary().hf_client {
            client.set_auth_token(token).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Clear the HuggingFace authentication token.
    ///
    /// Removes the persisted token file and clears the in-memory value.
    pub async fn clear_hf_token(&self) -> Result<()> {
        if let Some(ref client) = self.primary().hf_client {
            client.clear_auth_token().await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Get current HuggingFace authentication status.
    ///
    /// Makes a lightweight API call to validate the token and retrieve
    /// the associated username.
    pub async fn get_hf_auth_status(&self) -> Result<model_library::HfAuthStatus> {
        if let Some(ref client) = self.primary().hf_client {
            client.get_auth_status().await
        } else {
            Ok(model_library::HfAuthStatus {
                authenticated: false,
                username: None,
                token_source: None,
            })
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
