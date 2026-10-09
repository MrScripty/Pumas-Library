//! HuggingFace-specific helpers used by primary-state IPC dispatch.

use super::state::PrimaryState;
use crate::error::PumasError;
use crate::{model_library, models};
use std::collections::HashSet;
use std::sync::Arc;

fn hf_client_unavailable() -> PumasError {
    PumasError::Config {
        message: "HuggingFace client not initialized".to_string(),
    }
}

fn require_hf_client(
    primary: &PrimaryState,
) -> std::result::Result<&model_library::HuggingFaceClient, PumasError> {
    primary
        .hf_client
        .as_deref()
        .ok_or_else(hf_client_unavailable)
}

async fn finish_interrupted_download_scan<T>(
    task: tokio::task::JoinHandle<T>,
) -> std::result::Result<T, PumasError> {
    task.await.map_err(|error| {
        PumasError::Other(format!(
            "Failed to join interrupted-download scan task: {error}"
        ))
    })
}

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
    let client = require_hf_client(primary)?;
    // Explicit local discovery through the existing search caller. Ordinary HF
    // search and all acquisition identity checks keep their existing live path.
    if let Some(query) = query.strip_prefix("cache:") {
        return client
            .search_cached_model_details(&model_library::HfSearchParams {
                query: query.to_string(),
                kind: kind.map(String::from),
                limit: Some(limit),
                ..Default::default()
            })
            .await;
    }
    let params = model_library::HfSearchParams {
        query: query.to_string(),
        kind: kind.map(String::from),
        limit: Some(limit),
        hydrate_limit: Some(hydrate_limit.min(limit)),
        ..Default::default()
    };
    client.search(&params).await
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
    let client = primary
        .hf_client
        .clone()
        .ok_or_else(hf_client_unavailable)?;
    crate::PumasApi::start_hf_download_owned(primary.model_library.clone(), client, request).await
}

pub(super) async fn get_hf_download_progress(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<Option<models::ModelDownloadProgress>, PumasError> {
    Ok(require_hf_client(primary)?
        .get_download_progress(download_id)
        .await)
}

pub(super) async fn cancel_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    require_hf_client(primary)?
        .cancel_download(download_id)
        .await
}

pub(super) async fn pause_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    require_hf_client(primary)?
        .pause_download(download_id)
        .await
}

pub(super) async fn resume_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    require_hf_client(primary)?
        .resume_download(download_id)
        .await
}

pub(super) async fn list_hf_downloads(
    primary: &PrimaryState,
) -> std::result::Result<Vec<models::ModelDownloadProgress>, PumasError> {
    Ok(require_hf_client(primary)?.list_downloads().await)
}

pub(super) async fn list_interrupted_downloads(
    primary: &PrimaryState,
) -> std::result::Result<Vec<model_library::InterruptedDownload>, PumasError> {
    let model_importer = primary.model_importer.clone();
    let persistence = primary
        .hf_client
        .as_ref()
        .and_then(|client| client.persistence().cloned());

    let task = tokio::task::spawn_blocking(move || {
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
    });
    finish_interrupted_download_scan(task).await
}

pub(super) async fn recover_download(
    primary: &PrimaryState,
    repo_id: &str,
    dest_dir: &str,
) -> std::result::Result<String, PumasError> {
    let client = primary
        .hf_client
        .clone()
        .ok_or_else(hf_client_unavailable)?;
    crate::PumasApi::recover_download_owned(
        primary.model_library.clone(),
        client,
        repo_id,
        dest_dir,
    )
    .await
}

pub(super) async fn resume_partial_download(
    primary: &PrimaryState,
    repo_id: &str,
    dest_dir: &str,
) -> std::result::Result<models::PartialDownloadAction, PumasError> {
    let Some(client) = primary.hf_client.clone() else {
        return Ok(models::PartialDownloadAction {
            action: "none".to_string(),
            download_id: None,
            status: None,
            reason_code: Some("hf_client_unavailable".to_string()),
            message: Some("HuggingFace client not initialized".to_string()),
        });
    };
    crate::PumasApi::resume_partial_download_owned(
        primary.model_library.clone(),
        client,
        repo_id,
        dest_dir,
    )
    .await
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

#[cfg(test)]
mod download_lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn ipc_download_mutations_share_closed_invocation_admission() {
        use crate::ipc::server::IpcDispatch;

        let temp = tempfile::TempDir::new().unwrap();
        let api = crate::api::hf::tests::recovery_api_fixture(temp.path(), None).await;
        api.shutdown_downloads().await.unwrap();
        let request = serde_json::json!({
            "repo_id": "shutdown-fixture/model",
            "family": "shutdown-fixture",
            "official_name": "model",
            "filename": "weights.gguf",
        });
        for (method, params) in [
            ("start_hf_download", serde_json::json!({"request": request})),
            (
                "recover_download",
                serde_json::json!({"repo_id": "shutdown-fixture/model", "dest_dir": "/missing-shutdown-fixture"}),
            ),
            (
                "resume_partial_download",
                serde_json::json!({"repo_id": "shutdown-fixture/model", "dest_dir": "/missing-shutdown-fixture"}),
            ),
            (
                "pause_hf_download",
                serde_json::json!({"download_id": "missing"}),
            ),
            (
                "resume_hf_download",
                serde_json::json!({"download_id": "missing"}),
            ),
            (
                "cancel_hf_download",
                serde_json::json!({"download_id": "missing"}),
            ),
        ] {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                api.primary().dispatch(method, params),
            )
            .await
            .expect("closed IPC admission must not start metadata or directory effects");
            assert!(
                matches!(result, Err(PumasError::DownloadLifecycleClosed)),
                "{method}: {result:?}"
            );
        }
        assert!(!api
            .primary()
            .model_library
            .library_root()
            .join("llm")
            .exists());
    }

    #[tokio::test]
    async fn interrupted_scan_join_failure_is_not_an_empty_success() {
        let task = tokio::task::spawn_blocking(|| {
            panic!("synthetic interrupted-scan panic");
        });

        assert!(finish_interrupted_download_scan(task).await.is_err());
    }
}
