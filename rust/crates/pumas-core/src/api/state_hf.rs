//! HuggingFace-specific helpers used by primary-state IPC dispatch.

use super::state::PrimaryState;
use crate::error::PumasError;
use crate::{model_library, models};
use std::collections::HashSet;
use std::sync::Arc;

async fn load_hf_model_snapshot(
    library: Arc<model_library::ModelLibrary>,
    model_dir: std::path::PathBuf,
    model_id: String,
) -> std::result::Result<(Option<models::ModelMetadata>, Option<std::path::PathBuf>), PumasError> {
    tokio::task::spawn_blocking(move || {
        let metadata = library.load_metadata(&model_dir)?;
        let primary_file = library.get_primary_model_file(&model_id);
        Ok((metadata, primary_file))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join HF model snapshot task: {}", err)))?
}

async fn load_model_metadata_or_default(
    library: Arc<model_library::ModelLibrary>,
    model_dir: std::path::PathBuf,
) -> std::result::Result<models::ModelMetadata, PumasError> {
    tokio::task::spawn_blocking(move || Ok(library.load_metadata(&model_dir)?.unwrap_or_default()))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join HF metadata refresh load task: {}",
                err
            ))
        })?
}

pub(super) async fn search_hf_models(
    primary: &PrimaryState,
    query: &str,
    kind: Option<&str>,
    limit: usize,
) -> std::result::Result<Vec<models::HuggingFaceModel>, PumasError> {
    search_hf_models_with_hydration(primary, query, kind, limit, limit).await
}

pub(super) async fn search_hf_models_with_hydration(
    primary: &PrimaryState,
    query: &str,
    kind: Option<&str>,
    limit: usize,
    hydrate_limit: usize,
) -> std::result::Result<Vec<models::HuggingFaceModel>, PumasError> {
    if let Some(ref client) = primary.hf_client {
        let params = model_library::HfSearchParams {
            query: query.to_string(),
            kind: kind.map(String::from),
            limit: Some(limit),
            hydrate_limit: Some(hydrate_limit.min(limit)),
            ..Default::default()
        };
        client.search(&params).await
    } else {
        Ok(vec![])
    }
}

pub(super) async fn get_hf_download_details(
    primary: &PrimaryState,
    repo_id: &str,
    quants: &[String],
) -> std::result::Result<models::HfDownloadDetails, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.get_download_details(repo_id, quants).await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
    }
}

pub(super) async fn start_hf_download(
    primary: &PrimaryState,
    request: &model_library::DownloadRequest,
) -> std::result::Result<String, PumasError> {
    use crate::api::hf::{apply_remote_model_metadata, normalized_download_hint};
    use tracing::{info, warn};

    let client = primary
        .hf_client
        .as_ref()
        .ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;

    let mut resolved_request = request.clone();
    let mut resolved_pipeline_tag =
        normalized_download_hint(resolved_request.pipeline_tag.as_deref()).map(ToOwned::to_owned);
    let mut remote_model = None;
    let mut huggingface_evidence = match client.get_model_snapshot(&request.repo_id).await {
        Ok((model, evidence)) => {
            remote_model = Some(model);
            Some(evidence)
        }
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
        if remote_model.is_none() {
            remote_model = Some(client.get_model_info(&request.repo_id).await?);
        }
        let model_info = remote_model.as_ref().expect("remote model must be present");
        if resolved_pipeline_tag.is_none() {
            resolved_pipeline_tag =
                normalized_download_hint(Some(model_info.kind.as_str())).map(ToOwned::to_owned);
        }
        if resolved_model_type.is_none() {
            resolved_model_type = crate::api::hf::resolve_model_type_from_hints_async(
                primary.model_library.index().clone(),
                vec![
                    normalized_download_hint(request.model_type.as_deref()).map(ToOwned::to_owned),
                    resolved_pipeline_tag.clone(),
                    normalized_download_hint(Some(model_info.kind.as_str())).map(ToOwned::to_owned),
                ],
            )
            .await?;
        }
    }
    if let Some(model_info) = remote_model.as_ref() {
        apply_remote_model_metadata(&mut resolved_request, model_info);
    } else if resolved_request.license_status.is_none() {
        resolved_request.license_status = Some("license_unknown".to_string());
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
    let model_type = resolved_model_type.unwrap_or_else(|| "unknown".to_string());
    let architecture_family = model_library::infer_architecture_family_for_download(
        &resolved_request,
        huggingface_evidence.as_ref(),
    );
    resolved_request.family = architecture_family.clone();
    let selected_artifact =
        model_library::SelectedArtifactIdentity::from_download_request(&resolved_request, None);
    resolved_request.model_type = Some(model_type.clone());
    let dest_dir = primary
        .model_library
        .prepare_artifact_download_destination(
            &model_type,
            &architecture_family,
            &selected_artifact.artifact_id,
        )?;
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
}

pub(super) async fn get_hf_download_progress(
    primary: &PrimaryState,
    download_id: &str,
) -> Option<models::ModelDownloadProgress> {
    if let Some(ref client) = primary.hf_client {
        client.get_download_progress(download_id).await
    } else {
        None
    }
}

pub(super) async fn cancel_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.cancel_download(download_id).await
    } else {
        Ok(false)
    }
}

pub(super) async fn pause_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.pause_download(download_id).await
    } else {
        Ok(false)
    }
}

pub(super) async fn resume_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.resume_download(download_id).await
    } else {
        Ok(false)
    }
}

pub(super) async fn list_hf_downloads(
    primary: &PrimaryState,
) -> Vec<models::ModelDownloadProgress> {
    if let Some(ref client) = primary.hf_client {
        client.list_downloads().await
    } else {
        vec![]
    }
}

pub(super) async fn list_interrupted_downloads(
    primary: &PrimaryState,
) -> Vec<model_library::InterruptedDownload> {
    let model_importer = primary.model_importer.clone();
    let persistence = primary
        .hf_client
        .as_ref()
        .and_then(|client| client.persistence().cloned());

    tokio::task::spawn_blocking(move || {
        let known_dirs: HashSet<std::path::PathBuf> = persistence
            .map(|persistence| {
                persistence
                    .load_all()
                    .into_iter()
                    .map(|entry| entry.dest_dir)
                    .collect()
            })
            .unwrap_or_default();

        model_importer.find_interrupted_downloads(&known_dirs)
    })
    .await
    .unwrap_or_default()
}

pub(super) async fn recover_download(
    primary: &PrimaryState,
    repo_id: &str,
    dest_dir: &str,
) -> std::result::Result<String, PumasError> {
    let dest =
        crate::api::hf::validate_existing_local_directory_lookup_path(dest_dir, "dest_dir").await?;

    let client = primary
        .hf_client
        .as_ref()
        .ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;

    let parts: Vec<&str> = repo_id.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(PumasError::Config {
            message: format!(
                "Invalid repo_id format (expected 'owner/name'): {}",
                repo_id
            ),
        });
    }

    let model_type = dest
        .strip_prefix(primary.model_library.library_root())
        .ok()
        .and_then(|rel| rel.components().next())
        .and_then(|c| c.as_os_str().to_str())
        .map(String::from);

    let request = model_library::DownloadRequest {
        repo_id: repo_id.to_string(),
        family: parts[0].to_string(),
        official_name: parts[1].to_string(),
        model_type,
        quant: None,
        filename: None,
        filenames: None,
        pipeline_tag: None,
        bundle_format: None,
        pipeline_class: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    };

    client.start_download(&request, &dest, None).await
}

pub(super) async fn resume_partial_download(
    primary: &PrimaryState,
    repo_id: &str,
    dest_dir: &str,
) -> std::result::Result<models::PartialDownloadAction, PumasError> {
    let dest =
        match crate::api::hf::validate_existing_local_directory_lookup_path(dest_dir, "dest_dir")
            .await
        {
            Ok(dest) => dest,
            Err(PumasError::InvalidParams { .. } | PumasError::NotFound { .. }) => {
                return Ok(models::PartialDownloadAction {
                    action: "none".to_string(),
                    download_id: None,
                    status: None,
                    reason_code: Some("dest_dir_missing".to_string()),
                    message: Some(format!("directory not found: {}", dest_dir)),
                });
            }
            Err(err) => return Err(err),
        };

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

    if let Some(download_id) = client.find_download_id_by_dest_dir(&dest).await {
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
                            return Ok(models::PartialDownloadAction {
                                action: "none".to_string(),
                                download_id: Some(download_id),
                                status: Some(status),
                                reason_code: Some(
                                    crate::api::hf::partial_download_reason_code(&err).to_string(),
                                ),
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

    match recover_download(primary, repo_id, dest_dir).await {
        Ok(download_id) => Ok(models::PartialDownloadAction {
            action: "recover".to_string(),
            download_id: Some(download_id),
            status: Some(models::DownloadStatus::Queued),
            reason_code: None,
            message: None,
        }),
        Err(err) => Ok(models::PartialDownloadAction {
            action: "none".to_string(),
            download_id: None,
            status: None,
            reason_code: Some(crate::api::hf::partial_download_reason_code(&err).to_string()),
            message: Some(err.to_string()),
        }),
    }
}

pub(super) async fn refetch_metadata_from_hf(
    primary: &PrimaryState,
    model_id: &str,
) -> std::result::Result<models::ModelMetadata, PumasError> {
    use crate::api::hf::serialize_model_card_json;

    let hf_client = primary
        .hf_client
        .as_ref()
        .ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;
    let library = &primary.model_library;

    if let Some(repo_id) = model_id.strip_prefix("download:") {
        let model = hf_client.get_model_info(repo_id).await?;
        let model_type = crate::api::hf::resolve_model_type_from_hints_async(
            library.index().clone(),
            vec![Some(model.kind.clone()), None, None],
        )
        .await?;
        return Ok(models::ModelMetadata {
            repo_id: Some(model.repo_id),
            official_name: Some(model.name),
            model_type,
            download_url: Some(model.url),
            release_date: model.release_date,
            model_card: model.model_card,
            license_status: model
                .license
                .or_else(|| Some("license_unknown".to_string())),
            match_source: Some("hf".to_string()),
            match_method: Some("repo_id".to_string()),
            match_confidence: Some(1.0),
            ..Default::default()
        });
    }

    let model_dir = library.library_root().join(model_id);
    let (current, primary_file) = load_hf_model_snapshot(
        primary.model_library.clone(),
        model_dir.clone(),
        model_id.to_string(),
    )
    .await?;

    let repo_id = current
        .as_ref()
        .and_then(|m| m.repo_id.clone())
        .or_else(|| {
            let parts: Vec<&str> = model_id.splitn(3, '/').collect();
            if parts.len() == 3 {
                Some(format!("{}/{}", parts[1], parts[2]))
            } else {
                None
            }
        });

    let hf_result = if let Some(ref repo_id) = repo_id {
        let model = hf_client.get_model_info(repo_id).await?;
        let translated_model_type = crate::api::hf::resolve_model_type_from_hints_async(
            library.index().clone(),
            vec![Some(model.kind.clone()), None, None],
        )
        .await?;
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
            release_date: model.release_date,
            model_card_json: serialize_model_card_json(model.model_card.as_ref()),
            license_status: model
                .license
                .or_else(|| Some("license_unknown".to_string())),
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

    library
        .update_metadata_from_hf(model_id, &hf_result, true)
        .await?;

    let updated = load_model_metadata_or_default(primary.model_library.clone(), model_dir).await?;
    Ok(updated)
}

pub(super) async fn lookup_hf_metadata_for_file(
    primary: &PrimaryState,
    file_path: &str,
) -> std::result::Result<Option<model_library::HfMetadataResult>, PumasError> {
    if let Some(ref client) = primary.hf_client {
        let path = crate::api::hf::validate_existing_local_file_lookup_path(file_path, "file_path")
            .await?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);
        client.lookup_metadata(filename, Some(&path), None).await
    } else {
        Ok(None)
    }
}

pub(super) async fn lookup_hf_metadata_for_bundle_directory(
    primary: &PrimaryState,
    dir_path: &str,
) -> std::result::Result<Option<model_library::HfMetadataResult>, PumasError> {
    let Some(client) = primary.hf_client.as_ref() else {
        return Ok(None);
    };

    let dir_path =
        crate::api::hf::validate_existing_local_directory_lookup_path(dir_path, "dir_path").await?;
    let dir_path_for_lookup = dir_path.clone();
    let hints = tokio::task::spawn_blocking(move || {
        model_library::get_diffusers_bundle_lookup_hints(&dir_path_for_lookup)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join bundle lookup hint extraction task: {}",
            err
        ))
    })?;
    let Some(hints) = hints else {
        return Ok(None);
    };

    let search_results =
        crate::api::hf::collect_bundle_lookup_candidates(client, &hints.bundle_name).await?;

    for candidate in crate::api::hf::rank_bundle_lookup_candidates(
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
        let match_confidence = if crate::api::hf::is_exact_bundle_lookup_match(
            &hints.bundle_name,
            &candidate_repo_id,
            &candidate.name,
        ) {
            0.95
        } else {
            0.72
        };

        return Ok(Some(crate::api::hf::build_lookup_metadata_from_model(
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

    if let Some((candidate, match_method, match_confidence)) =
        crate::api::hf::fallback_bundle_lookup_candidate(
            &hints.bundle_name,
            hints.name_or_path.as_deref(),
            &search_results,
        )
    {
        let candidate_repo_id = candidate.repo_id.clone();
        return Ok(Some(crate::api::hf::build_lookup_metadata_from_model(
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
    if !crate::api::hf::looks_like_repo_id(base_repo_id) {
        return Ok(None);
    }

    match client.get_model_info(base_repo_id).await {
        Ok(model) => Ok(Some(crate::api::hf::build_lookup_metadata_from_model(
            primary.model_library.index(),
            model,
            "filename_fuzzy",
            0.55,
            None,
        )?)),
        Err(err) => {
            tracing::warn!(
                "Failed to resolve diffusers bundle base model {} for {}: {}",
                base_repo_id,
                dir_path.display(),
                err
            );
            Ok(None)
        }
    }
}

pub(super) async fn get_hf_repo_files(
    primary: &PrimaryState,
    repo_id: &str,
) -> std::result::Result<model_library::RepoFileTree, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.get_repo_files(repo_id).await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
    }
}

pub(super) async fn set_hf_token(
    primary: &PrimaryState,
    token: &str,
) -> std::result::Result<(), PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.set_auth_token(token).await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
    }
}

pub(super) async fn clear_hf_token(primary: &PrimaryState) -> std::result::Result<(), PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.clear_auth_token().await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
    }
}

pub(super) async fn get_hf_auth_status(
    primary: &PrimaryState,
) -> std::result::Result<model_library::HfAuthStatus, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.get_auth_status().await
    } else {
        Ok(model_library::HfAuthStatus {
            authenticated: false,
            username: None,
            token_source: None,
        })
    }
}
