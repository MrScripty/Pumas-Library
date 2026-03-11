//! HuggingFace methods on PumasApi.

use crate::error::{PumasError, Result};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use std::collections::HashSet;
use std::path::Path;
use tracing::{info, warn};

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
        self.search_hf_models_with_hydration(query, kind, limit, limit)
            .await
    }

    /// Search for models on HuggingFace with a bounded network hydration budget.
    pub async fn search_hf_models_with_hydration(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
        hydrate_limit: usize,
    ) -> Result<Vec<models::HuggingFaceModel>> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "search_hf_models_with_hydration",
                    serde_json::json!({
                        "query": query,
                        "kind": kind,
                        "limit": limit,
                        "hydrate_limit": hydrate_limit,
                    }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            let params = model_library::HfSearchParams {
                query: query.to_string(),
                kind: kind.map(String::from),
                limit: Some(limit),
                hydrate_limit: Some(hydrate_limit.min(limit)),
                ..Default::default()
            };
            // search() handles caching transparently
            client.search(&params).await
        } else {
            Ok(vec![])
        }
    }

    /// Get exact download details for a single HuggingFace repository.
    pub async fn get_hf_download_details(
        &self,
        repo_id: &str,
        quants: &[String],
    ) -> Result<models::HfDownloadDetails> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "get_hf_download_details",
                    serde_json::json!({
                        "repo_id": repo_id,
                        "quants": quants,
                    }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            client.get_download_details(repo_id, quants).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(
        &self,
        request: &model_library::DownloadRequest,
    ) -> Result<String> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "start_hf_download",
                    serde_json::json!({ "request": request }),
                )
                .await;
        }

        let primary = self.primary();
        if let Some(ref client) = primary.hf_client {
            let mut resolved_request = request.clone();
            let mut resolved_pipeline_tag =
                normalized_download_hint(resolved_request.pipeline_tag.as_deref())
                    .map(ToOwned::to_owned);
            let mut huggingface_evidence = match client.get_model_evidence(&request.repo_id).await {
                Ok(evidence) => Some(evidence),
                Err(err) => {
                    warn!(
                        "Failed to capture HF evidence for {} before download: {}",
                        request.repo_id, err
                    );
                    None
                }
            };
            if let Some(remote_pipeline_tag) = huggingface_evidence
                .as_ref()
                .and_then(|evidence| normalized_download_hint(evidence.pipeline_tag.as_deref()))
            {
                resolved_pipeline_tag = Some(remote_pipeline_tag.to_string());
            }
            let mut resolved_model_type = if let Some(ref evidence) = huggingface_evidence {
                let resolved = model_library::resolve_model_type_from_huggingface_evidence(
                    primary.model_library.index(),
                    Some(&resolved_request.official_name),
                    resolved_pipeline_tag.as_deref(),
                    request.model_type.as_deref(),
                    Some(evidence),
                )?;
                (resolved.model_type != model_library::ModelType::Unknown)
                    .then(|| resolved.model_type.as_str().to_string())
            } else {
                None
            };

            if resolved_model_type.is_none() || resolved_pipeline_tag.is_none() {
                // Fall back to repo metadata only when the request does not already
                // carry enough information to place the download safely.
                let model_info = client.get_model_info(&request.repo_id).await?;
                if resolved_pipeline_tag.is_none() {
                    resolved_pipeline_tag =
                        normalized_download_hint(Some(model_info.kind.as_str()))
                            .map(ToOwned::to_owned);
                }
                if resolved_model_type.is_none() {
                    resolved_model_type = resolve_model_type_from_hints(
                        primary.model_library.index(),
                        [
                            normalized_download_hint(request.model_type.as_deref()),
                            resolved_pipeline_tag.as_deref(),
                            normalized_download_hint(Some(model_info.kind.as_str())),
                        ],
                    )?;
                }
            }

            let should_check_bundle = resolved_model_type
                .as_deref()
                .is_none_or(|model_type| model_type == "diffusion")
                || resolved_pipeline_tag.as_deref() == Some("text-to-image");
            if should_check_bundle {
                match client.classify_repo_bundle(&request.repo_id).await {
                    Ok(Some(bundle)) => {
                        if resolved_request.filename.is_some()
                            || resolved_request.filenames.is_some()
                            || resolved_request.quant.is_some()
                        {
                            info!(
                                "HF repo {} classified as {:?}; forcing full bundle download",
                                request.repo_id, bundle.bundle_format
                            );
                        }
                        resolved_request.filename = None;
                        resolved_request.filenames = None;
                        resolved_request.quant = None;
                        resolved_request.bundle_format = Some(bundle.bundle_format);
                        resolved_request.pipeline_class = Some(bundle.pipeline_class);
                        if resolved_pipeline_tag.is_none() {
                            resolved_pipeline_tag = Some("text-to-image".to_string());
                        }
                        if resolved_model_type.is_none() {
                            resolved_model_type = Some("diffusion".to_string());
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        warn!(
                            "Failed to classify HF repo {} as a bundle: {}",
                            request.repo_id, err
                        );
                    }
                }
            }

            resolved_request.pipeline_tag = resolved_pipeline_tag;

            // Determine destination directory.
            let model_type = resolved_model_type.unwrap_or_else(|| "unknown".to_string());
            resolved_request.model_type = Some(model_type.clone());
            let dest_dir = primary.model_library.build_model_path(
                &model_type,
                &resolved_request.family,
                &model_library::normalize_name(&resolved_request.official_name),
            );
            if model_type == "unknown" {
                warn!(
                    "Download {} is starting with unknown model_type after HF metadata lookup; destination={}",
                    request.repo_id,
                    dest_dir.display()
                );
            }
            if let Some(ref mut evidence) = huggingface_evidence {
                evidence.requested_model_type = request.model_type.clone();
                evidence.requested_pipeline_tag = request.pipeline_tag.clone();
                evidence.requested_quant = request.quant.clone();
            }
            client
                .start_download(&resolved_request, &dest_dir, huggingface_evidence)
                .await
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
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default(
                    "get_hf_download_progress",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            client.get_download_progress(download_id).await
        } else {
            None
        }
    }

    /// Cancel a HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: &str) -> Result<bool> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "cancel_hf_download",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            client.cancel_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// Pause a HuggingFace download, preserving the `.part` file for later resume.
    pub async fn pause_hf_download(&self, download_id: &str) -> Result<bool> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "pause_hf_download",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            client.pause_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// Resume a paused or errored HuggingFace download.
    pub async fn resume_hf_download(&self, download_id: &str) -> Result<bool> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resume_hf_download",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            client.resume_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// List all HuggingFace downloads (active, paused, completed, etc.).
    pub async fn list_hf_downloads(&self) -> Vec<models::ModelDownloadProgress> {
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default("list_hf_downloads", serde_json::json!({}))
                .await;
        }

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
        if self.try_client().is_some() {
            return self.call_client_method_blocking_or_default(
                "list_interrupted_downloads",
                serde_json::json!({}),
            );
        }

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
    pub async fn recover_download(&self, repo_id: &str, dest_dir: &str) -> Result<String> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "recover_download",
                    serde_json::json!({
                        "repo_id": repo_id,
                        "dest_dir": dest_dir,
                    }),
                )
                .await;
        }

        let dest = std::path::Path::new(dest_dir);
        if !dest.is_dir() {
            return Err(PumasError::NotFound {
                resource: format!("directory: {}", dest_dir),
            });
        }

        let primary = self.primary();
        let client = primary
            .hf_client
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })?;

        // Parse repo_id into family/name
        let parts: Vec<&str> = repo_id.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(PumasError::Config {
                message: format!(
                    "Invalid repo_id format (expected 'owner/name'): {}",
                    repo_id
                ),
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
            filenames: None,
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
        };

        client.start_download(&request, dest, None).await
    }

    /// Resume a partial download by choosing the correct action:
    /// - Resume an existing tracked paused/error download
    /// - Attach to an already active tracked download
    /// - Recover an orphan partial download
    ///
    /// Returns an action descriptor instead of failing hard so UI callers can
    /// surface precise next steps to users.
    pub async fn resume_partial_download(
        &self,
        repo_id: &str,
        dest_dir: &str,
    ) -> Result<models::PartialDownloadAction> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resume_partial_download",
                    serde_json::json!({
                        "repo_id": repo_id,
                        "dest_dir": dest_dir,
                    }),
                )
                .await;
        }

        let dest = Path::new(dest_dir);
        if !dest.is_dir() {
            return Ok(models::PartialDownloadAction {
                action: "none".to_string(),
                download_id: None,
                status: None,
                reason_code: Some("dest_dir_missing".to_string()),
                message: Some(format!("directory not found: {}", dest_dir)),
            });
        }

        let primary = self.primary();
        let client = match primary.hf_client.as_ref() {
            Some(client) => client,
            None => {
                return Ok(models::PartialDownloadAction {
                    action: "none".to_string(),
                    download_id: None,
                    status: None,
                    reason_code: Some("hf_client_unavailable".to_string()),
                    message: Some("HuggingFace client not initialized".to_string()),
                });
            }
        };

        if let Some(download_id) = client.find_download_id_by_dest_dir(dest).await {
            let status = client.get_download_status(&download_id).await;
            if let Some(status) = status {
                match status {
                    models::DownloadStatus::Paused | models::DownloadStatus::Error => {
                        match client.resume_download(&download_id).await {
                            Ok(true) => {
                                return Ok(models::PartialDownloadAction {
                                    action: "resume".to_string(),
                                    download_id: Some(download_id),
                                    status: Some(models::DownloadStatus::Queued),
                                    reason_code: None,
                                    message: None,
                                });
                            }
                            Ok(false) => {
                                return Ok(models::PartialDownloadAction {
                                    action: "none".to_string(),
                                    download_id: Some(download_id),
                                    status: Some(status),
                                    reason_code: Some("resume_rejected".to_string()),
                                    message: Some(format!(
                                        "tracked download cannot be resumed from status {:?}",
                                        status
                                    )),
                                });
                            }
                            Err(err) => {
                                let reason_code = partial_download_reason_code(&err).to_string();
                                return Ok(models::PartialDownloadAction {
                                    action: "none".to_string(),
                                    download_id: Some(download_id),
                                    status: Some(status),
                                    reason_code: Some(reason_code),
                                    message: Some(err.to_string()),
                                });
                            }
                        }
                    }
                    models::DownloadStatus::Queued
                    | models::DownloadStatus::Downloading
                    | models::DownloadStatus::Pausing
                    | models::DownloadStatus::Cancelling => {
                        return Ok(models::PartialDownloadAction {
                            action: "attach".to_string(),
                            download_id: Some(download_id),
                            status: Some(status),
                            reason_code: None,
                            message: None,
                        });
                    }
                    models::DownloadStatus::Completed => {
                        return Ok(models::PartialDownloadAction {
                            action: "none".to_string(),
                            download_id: Some(download_id),
                            status: Some(status),
                            reason_code: Some("already_completed".to_string()),
                            message: Some("tracked download is already completed".to_string()),
                        });
                    }
                    models::DownloadStatus::Cancelled => {
                        return Ok(models::PartialDownloadAction {
                            action: "none".to_string(),
                            download_id: Some(download_id),
                            status: Some(status),
                            reason_code: Some("already_cancelled".to_string()),
                            message: Some(
                                "tracked download was cancelled; start a new download".to_string(),
                            ),
                        });
                    }
                }
            }
        }

        match self.recover_download(repo_id, dest_dir).await {
            Ok(download_id) => Ok(models::PartialDownloadAction {
                action: "recover".to_string(),
                download_id: Some(download_id),
                status: Some(models::DownloadStatus::Queued),
                reason_code: None,
                message: None,
            }),
            Err(err) => {
                let reason_code = partial_download_reason_code(&err).to_string();
                Ok(models::PartialDownloadAction {
                    action: "none".to_string(),
                    download_id: None,
                    status: None,
                    reason_code: Some(reason_code),
                    message: Some(err.to_string()),
                })
            }
        }
    }

    /// Refetch metadata for a library model from HuggingFace.
    ///
    /// Uses the stored `repo_id` if available, otherwise falls back to
    /// filename-based lookup via `lookup_metadata()`. Returns the updated
    /// metadata on success.
    pub async fn refetch_metadata_from_hf(&self, model_id: &str) -> Result<models::ModelMetadata> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "refetch_metadata_from_hf",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

        let primary = self.primary();
        let hf_client = primary
            .hf_client
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })?;
        let library = &primary.model_library;

        // Handle download-in-progress models: extract repo_id and fetch directly
        if let Some(repo_id) = model_id.strip_prefix("download:") {
            let model = hf_client.get_model_info(repo_id).await?;
            let model_type = resolve_model_type_from_hints(
                library.index(),
                [Some(model.kind.as_str()), None, None],
            )?;
            return Ok(models::ModelMetadata {
                repo_id: Some(model.repo_id),
                official_name: Some(model.name),
                model_type,
                download_url: Some(model.url),
                match_source: Some("hf".to_string()),
                match_method: Some("repo_id".to_string()),
                match_confidence: Some(1.0),
                ..Default::default()
            });
        }

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
            let translated_model_type = resolve_model_type_from_hints(
                library.index(),
                [Some(model.kind.as_str()), None, None],
            )?;
            model_library::HfMetadataResult {
                repo_id: model.repo_id,
                official_name: Some(model.name),
                family: None,
                model_type: translated_model_type,
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
            let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "lookup_hf_metadata_for_file",
                    serde_json::json!({ "file_path": file_path }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            let path = std::path::Path::new(file_path);
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);
            client.lookup_metadata(filename, Some(path), None).await
        } else {
            Ok(None)
        }
    }

    /// Look up HuggingFace metadata for a local diffusers bundle directory.
    pub async fn lookup_hf_metadata_for_bundle_directory(
        &self,
        dir_path: &str,
    ) -> Result<Option<model_library::HfMetadataResult>> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "lookup_hf_metadata_for_bundle_directory",
                    serde_json::json!({ "dir_path": dir_path }),
                )
                .await;
        }

        let primary = self.primary();
        let Some(client) = primary.hf_client.as_ref() else {
            return Ok(None);
        };

        let bundle_root = Path::new(dir_path);
        let Some(hints) = model_library::get_diffusers_bundle_lookup_hints(bundle_root) else {
            return Ok(None);
        };

        let search_results = collect_bundle_lookup_candidates(client, &hints.bundle_name).await?;

        for candidate in rank_bundle_lookup_candidates(
            &hints.bundle_name,
            hints.name_or_path.as_deref(),
            &search_results,
        ) {
            if client
                .classify_repo_bundle(&candidate.repo_id)
                .await?
                .is_none()
            {
                continue;
            }

            let candidate_repo_id = candidate.repo_id.clone();
            let match_confidence = if is_exact_bundle_lookup_match(
                &hints.bundle_name,
                &candidate_repo_id,
                &candidate.name,
            ) {
                0.95
            } else {
                0.72
            };

            return Ok(Some(build_lookup_metadata_from_model(
                primary.model_library.index(),
                candidate,
                if match_confidence >= 0.9 {
                    "filename_exact"
                } else {
                    "filename_fuzzy"
                },
                match_confidence,
                hints
                    .name_or_path
                    .as_ref()
                    .filter(|repo_id| *repo_id != &candidate_repo_id)
                    .cloned(),
            )?));
        }

        if let Some((candidate, match_method, match_confidence)) = fallback_bundle_lookup_candidate(
            &hints.bundle_name,
            hints.name_or_path.as_deref(),
            &search_results,
        ) {
            let candidate_repo_id = candidate.repo_id.clone();
            return Ok(Some(build_lookup_metadata_from_model(
                primary.model_library.index(),
                candidate,
                match_method,
                match_confidence,
                hints
                    .name_or_path
                    .as_ref()
                    .filter(|repo_id| *repo_id != &candidate_repo_id)
                    .cloned(),
            )?));
        }

        let Some(base_repo_id) = hints.name_or_path.as_deref() else {
            return Ok(None);
        };
        if !looks_like_repo_id(base_repo_id) {
            return Ok(None);
        }

        match client.get_model_info(base_repo_id).await {
            Ok(model) => Ok(Some(build_lookup_metadata_from_model(
                primary.model_library.index(),
                model,
                "filename_fuzzy",
                0.55,
                None,
            )?)),
            Err(err) => {
                warn!(
                    "Failed to resolve diffusers bundle base model {} for {}: {}",
                    base_repo_id, dir_path, err
                );
                Ok(None)
            }
        }
    }

    // ========================================
    // HuggingFace Authentication
    // ========================================

    /// Set the HuggingFace authentication token.
    ///
    /// Persists to disk and updates the in-memory token for immediate use.
    pub async fn set_hf_token(&self, token: &str) -> Result<()> {
        if self.try_client().is_some() {
            let _: serde_json::Value = self
                .call_client_method("set_hf_token", serde_json::json!({ "token": token }))
                .await?;
            return Ok(());
        }

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
        if self.try_client().is_some() {
            let _: serde_json::Value = self
                .call_client_method("clear_hf_token", serde_json::json!({}))
                .await?;
            return Ok(());
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method("get_hf_auth_status", serde_json::json!({}))
                .await;
        }

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
    pub async fn get_hf_repo_files(&self, repo_id: &str) -> Result<model_library::RepoFileTree> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "get_hf_repo_files",
                    serde_json::json!({ "repo_id": repo_id }),
                )
                .await;
        }

        if let Some(ref client) = self.primary().hf_client {
            client.get_repo_files(repo_id).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }
}

pub(crate) fn resolve_model_type_from_hints<const N: usize>(
    index: &crate::index::ModelIndex,
    hints: [Option<&str>; N],
) -> Result<Option<String>> {
    let mut seen = HashSet::new();
    for raw_hint in hints.into_iter().flatten() {
        let normalized_hint = raw_hint.trim().to_lowercase();
        if normalized_hint.is_empty() || !seen.insert(normalized_hint.clone()) {
            continue;
        }
        if let Some(model_type) = index.resolve_model_type_hint(&normalized_hint)? {
            return Ok(Some(model_type));
        }
    }
    Ok(None)
}

pub(crate) fn normalized_download_hint(hint: Option<&str>) -> Option<&str> {
    hint.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(crate) fn build_lookup_metadata_from_model(
    index: &crate::index::ModelIndex,
    model: models::HuggingFaceModel,
    match_method: &str,
    match_confidence: f64,
    base_model: Option<String>,
) -> Result<model_library::HfMetadataResult> {
    let model_type = resolve_model_type_from_hints(index, [Some(model.kind.as_str()), None, None])?;
    Ok(model_library::HfMetadataResult {
        repo_id: model.repo_id,
        official_name: Some(model.name),
        family: None,
        model_type,
        subtype: None,
        variant: None,
        precision: None,
        tags: vec![],
        base_model,
        download_url: Some(model.url),
        description: None,
        match_confidence,
        match_method: match_method.to_string(),
        requires_confirmation: match_confidence < 0.8,
        hash_mismatch: false,
        matched_filename: None,
        pending_full_verification: false,
        fast_hash: None,
        expected_sha256: None,
    })
}

pub(crate) fn rank_bundle_lookup_candidates(
    bundle_name: &str,
    hinted_repo_id: Option<&str>,
    candidates: &[models::HuggingFaceModel],
) -> Vec<models::HuggingFaceModel> {
    let mut ranked = candidates.to_vec();
    ranked.sort_by(|left, right| {
        let left_score = bundle_lookup_score(bundle_name, hinted_repo_id, left);
        let right_score = bundle_lookup_score(bundle_name, hinted_repo_id, right);
        right_score
            .cmp(&left_score)
            .then_with(|| {
                right
                    .downloads
                    .unwrap_or(0)
                    .cmp(&left.downloads.unwrap_or(0))
            })
            .then_with(|| left.repo_id.cmp(&right.repo_id))
    });
    ranked
}

pub(crate) async fn collect_bundle_lookup_candidates(
    client: &model_library::HuggingFaceClient,
    bundle_name: &str,
) -> Result<Vec<models::HuggingFaceModel>> {
    let mut merged = Vec::new();
    let mut seen_repo_ids = HashSet::new();

    for query in bundle_lookup_query_variants(bundle_name) {
        for kind in [Some("text-to-image"), None] {
            let results = client
                .search(&model_library::HfSearchParams {
                    query: query.clone(),
                    kind: kind.map(str::to_string),
                    limit: Some(20),
                    hydrate_limit: Some(10),
                    ..Default::default()
                })
                .await?;

            for candidate in results {
                if seen_repo_ids.insert(candidate.repo_id.clone()) {
                    merged.push(candidate);
                }
            }
        }
    }

    Ok(merged)
}

fn bundle_lookup_query_variants(bundle_name: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();

    let mut push = |value: String| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        let normalized = trimmed.to_lowercase();
        if seen.insert(normalized) {
            queries.push(trimmed.to_string());
        }
    };

    push(bundle_name.to_string());
    push(bundle_name.replace(['-', '_'], " "));
    push(
        bundle_name
            .chars()
            .map(|ch| {
                if ch == '-' || ch == '_' {
                    ' '
                } else if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                    ch
                } else {
                    ' '
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    );

    queries
}

pub(crate) fn fallback_bundle_lookup_candidate(
    bundle_name: &str,
    hinted_repo_id: Option<&str>,
    candidates: &[models::HuggingFaceModel],
) -> Option<(models::HuggingFaceModel, &'static str, f64)> {
    let ranked = rank_bundle_lookup_candidates(bundle_name, hinted_repo_id, candidates);
    let candidate = ranked
        .into_iter()
        .find(|candidate| bundle_lookup_score(bundle_name, hinted_repo_id, candidate) >= 35)?;

    let exact = is_exact_bundle_lookup_match(bundle_name, &candidate.repo_id, &candidate.name);
    Some((
        candidate,
        if exact {
            "filename_exact"
        } else {
            "filename_fuzzy"
        },
        if exact { 0.84 } else { 0.62 },
    ))
}

fn bundle_lookup_score(
    bundle_name: &str,
    hinted_repo_id: Option<&str>,
    candidate: &models::HuggingFaceModel,
) -> i32 {
    let mut score = 0;
    if is_exact_bundle_lookup_match(bundle_name, &candidate.repo_id, &candidate.name) {
        score += 100;
    }

    let normalized_bundle = normalize_bundle_lookup_key(bundle_name);
    let repo_basename = repo_basename(&candidate.repo_id);
    let normalized_repo_basename = normalize_bundle_lookup_key(repo_basename);
    if !normalized_bundle.is_empty() && normalized_repo_basename.contains(&normalized_bundle) {
        score += 25;
    }

    if candidate.kind == "text-to-image" {
        score += 10;
    }

    if hinted_repo_id.is_some_and(|repo_id| repo_id == candidate.repo_id) {
        score += 5;
    }

    score
}

pub(crate) fn is_exact_bundle_lookup_match(
    bundle_name: &str,
    repo_id: &str,
    model_name: &str,
) -> bool {
    let normalized_bundle = normalize_bundle_lookup_key(bundle_name);
    if normalized_bundle.is_empty() {
        return false;
    }

    let repo_match = normalize_bundle_lookup_key(repo_basename(repo_id)) == normalized_bundle;
    let name_match = normalize_bundle_lookup_key(model_name) == normalized_bundle;
    repo_match || name_match
}

fn normalize_bundle_lookup_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn repo_basename(repo_id: &str) -> &str {
    repo_id.rsplit('/').next().unwrap_or(repo_id)
}

pub(crate) fn looks_like_repo_id(value: &str) -> bool {
    let mut parts = value.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None) if !owner.trim().is_empty() && !name.trim().is_empty()
    )
}

pub(crate) fn partial_download_reason_code(err: &PumasError) -> &'static str {
    match err {
        PumasError::NotFound { .. } => "dest_dir_missing",
        PumasError::ModelNotFound { .. } => "repo_not_found",
        PumasError::RateLimited { .. } => "rate_limited",
        PumasError::PermissionDenied(_) => "permission_denied",
        PumasError::Timeout(_)
        | PumasError::Network { .. }
        | PumasError::CircuitBreakerOpen { .. } => "network_error",
        PumasError::Config { message } if message.contains("Invalid repo_id format") => {
            "invalid_repo_id"
        }
        PumasError::Config { .. } => "hf_client_unavailable",
        _ => "recover_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HuggingFaceModel;

    #[test]
    fn test_partial_download_reason_code_maps_invalid_repo_id() {
        let err = PumasError::Config {
            message: "Invalid repo_id format (expected 'owner/name'): bad".to_string(),
        };
        assert_eq!(partial_download_reason_code(&err), "invalid_repo_id");
    }

    #[test]
    fn test_partial_download_reason_code_maps_network_errors() {
        let err = PumasError::Network {
            message: "connection dropped".to_string(),
            cause: None,
        };
        assert_eq!(partial_download_reason_code(&err), "network_error");
    }

    #[test]
    fn test_normalized_download_hint_rejects_unknown_values() {
        assert_eq!(normalized_download_hint(Some("unknown")), None);
        assert_eq!(normalized_download_hint(Some("  ")), None);
        assert_eq!(
            normalized_download_hint(Some("text-generation")),
            Some("text-generation")
        );
    }

    #[test]
    fn ranks_exact_bundle_repo_name_ahead_of_base_model_hint() {
        let ranked = rank_bundle_lookup_candidates(
            "tiny-sd-turbo",
            Some("stabilityai/sd-turbo"),
            &[
                hf_model("stabilityai/sd-turbo", "sd-turbo", 10),
                hf_model("cc-nms/tiny-sd-turbo", "tiny-sd-turbo", 1),
            ],
        );

        assert_eq!(ranked[0].repo_id, "cc-nms/tiny-sd-turbo");
    }

    #[test]
    fn exact_bundle_match_normalizes_separator_variants() {
        assert!(is_exact_bundle_lookup_match(
            "tiny-sd-turbo",
            "cc-nms/tiny_sd_turbo",
            "Tiny SD Turbo"
        ));
    }

    #[test]
    fn bundle_lookup_falls_back_to_exact_non_bundle_repo_match() {
        let fallback = fallback_bundle_lookup_candidate(
            "Juggernaut-X-v10",
            None,
            &[
                hf_model("foo/bar", "bar", 100),
                hf_model("RunDiffusion/Juggernaut-X-v10", "Juggernaut-X-v10", 5),
            ],
        )
        .unwrap();

        assert_eq!(fallback.0.repo_id, "RunDiffusion/Juggernaut-X-v10");
        assert_eq!(fallback.1, "filename_exact");
        assert_eq!(fallback.2, 0.84);
    }

    #[test]
    fn bundle_lookup_fallback_rejects_weak_unrelated_results() {
        let fallback = fallback_bundle_lookup_candidate(
            "Juggernaut-X-v10",
            None,
            &[
                hf_model("foo/bar", "bar", 100),
                hf_model("baz/qux", "qux", 50),
            ],
        );

        assert!(fallback.is_none());
    }

    #[test]
    fn bundle_lookup_query_variants_include_spaced_name_once() {
        let queries = bundle_lookup_query_variants("Juggernaut-X_v10");

        assert_eq!(queries[0], "Juggernaut-X_v10");
        assert!(queries.iter().any(|query| query == "Juggernaut X v10"));
        assert_eq!(
            queries
                .iter()
                .filter(|query| query.as_str() == "Juggernaut X v10")
                .count(),
            1
        );
    }

    fn hf_model(repo_id: &str, name: &str, downloads: u64) -> HuggingFaceModel {
        HuggingFaceModel {
            repo_id: repo_id.to_string(),
            name: name.to_string(),
            developer: String::new(),
            kind: "text-to-image".to_string(),
            formats: Vec::new(),
            quants: Vec::new(),
            download_options: Vec::new(),
            url: format!("https://huggingface.co/{}", repo_id),
            release_date: None,
            downloads: Some(downloads),
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines: Vec::new(),
        }
    }
}
