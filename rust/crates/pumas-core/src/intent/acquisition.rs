use super::types::*;
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::{
    DownloadRequest, HuggingFaceClient, LfsFileInfo, ModelLibrary, SelectedArtifactIdentity,
};
use crate::models::{DownloadStatus, PackageArtifactKind, PumasModelRef};
use crate::{PumasApi, PumasError, Result};
use std::sync::Arc;

pub(crate) async fn prepare_intent_upstream_download(
    library: Arc<ModelLibrary>,
    client: Arc<HuggingFaceClient>,
    requirement: &ModelRequirement,
) -> Result<std::result::Result<crate::api::PreparedIntentDownload, ObservedModelState>> {
    let (selected, revision) =
        match select_intent_upstream_artifact(client.clone(), requirement).await? {
            Ok(selected) => selected,
            Err(observed) => return Ok(Err(observed)),
        };
    prepare_selected_intent_download(library, client, selected, revision).await
}

async fn select_intent_upstream_artifact(
    client: Arc<HuggingFaceClient>,
    requirement: &ModelRequirement,
) -> Result<std::result::Result<(SelectedUpstreamArtifact, DownloadRevision), ObservedModelState>> {
    let ModelSelector::UpstreamRepository {
        repository_id,
        revision: selector,
    } = &requirement.selector
    else {
        return Ok(Err(ObservedModelState::Unsupported {
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::UpstreamAcquisitionUnavailable,
                "selector",
                "upstream acquisition requires an upstream repository selector",
            )],
        }));
    };
    let repository_id = repository_id.clone();
    let selector = selector.clone();
    let artifact = requirement.artifact.clone();
    let invocation_client = client.clone();
    let selected_invocation = client
        .run_download_invocation(move |context| async move {
            let result = async {
                let resolve_client = invocation_client.clone();
                let resolve_repo = repository_id.clone();
                let revision = context
                    .run_fallible_async_named(
                        "resolve intent download revision",
                        move || async move {
                            observe_upstream_result(
                                resolve_client
                                    .resolve_download_revision(&resolve_repo, selector.as_deref())
                                    .await,
                            )
                        },
                    )
                    .await
                    .map_err(|error| {
                        tracing::warn!(%error, "intent revision resolution owner failed");
                        unavailable(
                            IntentDiagnosticCode::UpstreamResolutionFailed,
                            "selector.revision",
                            "immutable upstream revision resolution was interrupted",
                        )
                    })?
                    .and_then(|observed| observed)
                    .map_err(|error| {
                        tracing::warn!(%error, "intent revision resolution failed");
                        unavailable(
                            IntentDiagnosticCode::UpstreamResolutionFailed,
                            "selector.revision",
                            "failed to resolve an immutable upstream revision",
                        )
                    })?;
                let tree_client = invocation_client.clone();
                let tree_repo = repository_id.clone();
                let tree_revision = revision.clone();
                let tree = context
                    .run_fallible_async_named(
                        "inspect pinned intent repository",
                        move || async move {
                            observe_upstream_result(
                                tree_client
                                    .get_repo_files_at_revision(&tree_repo, &tree_revision)
                                    .await,
                            )
                        },
                    )
                    .await
                    .map_err(|error| {
                        tracing::warn!(%error, "intent repository inspection owner failed");
                        unavailable(
                            IntentDiagnosticCode::UpstreamResolutionFailed,
                            "selector.repository_id",
                            "pinned upstream repository inspection was interrupted",
                        )
                    })?
                    .and_then(|observed| observed)
                    .map_err(|error| {
                        tracing::warn!(%error, "intent repository inspection failed");
                        unavailable(
                            IntentDiagnosticCode::UpstreamResolutionFailed,
                            "selector.repository_id",
                            "failed to inspect the pinned upstream repository",
                        )
                    })?;
                let selected = select_artifact(
                    &repository_id,
                    &artifact,
                    &tree.lfs_files,
                    &tree.regular_files,
                    &revision,
                )
                .map_err(|observed| *observed)?;
                Ok((selected, revision))
            }
            .await;
            Ok(result)
        })
        .await;
    let selected = match observe_upstream_result(selected_invocation)? {
        Ok(selected) => selected,
        Err(error) => return Ok(Err(classify_admission_error(error))),
    };
    Ok(selected)
}

async fn prepare_selected_intent_download(
    library: Arc<ModelLibrary>,
    client: Arc<HuggingFaceClient>,
    selected: SelectedUpstreamArtifact,
    revision: DownloadRevision,
) -> Result<std::result::Result<crate::api::PreparedIntentDownload, ObservedModelState>> {
    let expected_id = selected.identity.artifact_id.clone();
    let expected_filename = selected.request.filename.clone();
    let expected_repository = selected.request.repo_id.clone();
    let expected_revision = revision.as_str().to_string();
    let expected_format = selected.kind;
    let prepared = match observe_upstream_result(
        PumasApi::prepare_hf_download_owned_at_revision(
            library,
            client,
            &selected.request,
            revision,
        )
        .await,
    )? {
        Ok(prepared) => prepared,
        Err(error) => return Ok(Err(classify_admission_error(error))),
    };
    if prepared.repository_id() != expected_repository
        || prepared.revision() != expected_revision
        || prepared.format() != Some(expected_format)
        || prepared.selected_artifact_id() != expected_id
        || prepared.filename() != expected_filename.as_deref()
    {
        return Err(PumasError::Validation {
            field: "intent.prepared_download".to_string(),
            message: "canonical download planning changed the selected immutable artifact"
                .to_string(),
        });
    }
    Ok(Ok(prepared))
}

pub(crate) async fn prepare_bound_intent_download(
    library: Arc<ModelLibrary>,
    client: Arc<HuggingFaceClient>,
    repository_id: &str,
    commit: &str,
    filename: &str,
    format: PackageArtifactKind,
    expected_model_ref: &PumasModelRef,
) -> Result<std::result::Result<crate::api::PreparedIntentDownload, ObservedModelState>> {
    let revision = DownloadRevision::from_commit(commit)?;
    if repository_id.split_once('/').is_none() {
        return Ok(Err(ObservedModelState::InvalidRequirement {
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::InvalidRepositoryId,
                "declaration.bound_target.repository_id",
                "durable bound target repository identity is invalid",
            )],
        }));
    }
    let request = download_request(repository_id, format, filename);
    let expected_identity = SelectedArtifactIdentity::from_download_request_at_revision(
        &request,
        Some(vec![filename.to_string()]),
        &revision,
    );
    if expected_model_ref.revision.as_deref() != Some(revision.as_str())
        || expected_model_ref.selected_artifact_id.as_deref()
            != Some(expected_identity.artifact_id.as_str())
        || expected_model_ref.selected_artifact_path.is_some()
    {
        return Ok(Err(ObservedModelState::InvalidRequirement {
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::ContradictoryArtifactIdentity,
                "declaration.bound_target",
                "durable bound target contradicts its immutable artifact identity",
            )],
        }));
    }
    let prepared = match observe_upstream_result(
        PumasApi::prepare_hf_download_owned_at_revision(library, client, &request, revision).await,
    )? {
        Ok(prepared) => prepared,
        Err(error) => return Ok(Err(classify_admission_error(error))),
    };
    if prepared.repository_id() != repository_id
        || prepared.revision() != commit
        || prepared.filename() != Some(filename)
        || prepared.format() != Some(format)
        || prepared.selected_artifact_id() != expected_identity.artifact_id
        || prepared.model_ref().model_id != expected_model_ref.model_id
    {
        return Ok(Err(ObservedModelState::Blocked {
            resolved_requirement: None,
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::AcquisitionBlocked,
                "declaration.bound_target",
                "pinned replay plan does not match its durable bound target",
            )],
        }));
    }
    Ok(Ok(prepared))
}

pub(crate) async fn acquire_upstream(
    library: Arc<ModelLibrary>,
    client: Arc<HuggingFaceClient>,
    resolver: &super::resolver::IntentResolver,
    requirement: &ModelRequirement,
    admit_if_absent: bool,
) -> Result<Option<ObservedModelState>> {
    let (selected, revision) =
        match select_intent_upstream_artifact(client.clone(), requirement).await? {
            Ok(selected) => selected,
            Err(observed) => return Ok(Some(observed)),
        };
    let pinned_selection = pinned_upstream_requirement(&selected, &revision, requirement);
    let local = resolver.resolve_local(&pinned_selection).await?;
    if matches!(local, ObservedModelState::Available { .. }) {
        return Ok(Some(local));
    }
    let selected_artifact_id = selected.identity.artifact_id.clone();
    let selected_repository = selected.request.repo_id.clone();
    let selected_revision = revision.as_str().to_string();
    let owner_snapshot = client.intent_download_snapshot().await;
    let matching = owner_snapshot
        .downloads
        .into_iter()
        .filter(|item| {
            item.repo_id == selected_repository
                && item.model_ref.as_ref().is_some_and(|actual| {
                    actual.revision.as_deref() == Some(selected_revision.as_str())
                        && actual.selected_artifact_id.as_deref()
                            == Some(selected_artifact_id.as_str())
                })
        })
        .collect::<Vec<_>>();
    match matching.len() {
        1 => {
            let observation = matching.into_iter().next().unwrap();
            return match project_download(observation, &requirement.artifact) {
                Some(observed) => Ok(Some(observed)),
                None => resolver.resolve_local(&pinned_selection).await.map(Some),
            };
        }
        0 => {}
        _ => {
            return Ok(Some(unavailable(
                IntentDiagnosticCode::DownloadCustodyUnavailable,
                "acquisition",
                "multiple owned downloads claim the same resolved requirement",
            )))
        }
    }
    if owner_snapshot.owner_closed {
        return Ok(Some(ObservedModelState::Blocked {
            resolved_requirement: None,
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::AcquisitionBlocked,
                "acquisition",
                "download ownership is closed",
            )],
        }));
    }
    if !admit_if_absent
        || !matches!(
            local,
            ObservedModelState::Missing { .. } | ObservedModelState::Unsatisfied { .. }
        )
    {
        return Ok(Some(local));
    }
    let prepared =
        match prepare_selected_intent_download(library.clone(), client.clone(), selected, revision)
            .await?
        {
            Ok(prepared) => prepared,
            Err(observed) => return Ok(Some(observed)),
        };
    let expected = prepared.model_ref().clone();
    let pinned_requirement = resolved_requirement(expected.clone(), &requirement.artifact);
    let owner_snapshot = client.intent_download_snapshot().await;
    let matching = owner_snapshot
        .downloads
        .into_iter()
        .filter(|item| {
            item.repo_id == prepared.repository_id()
                && item
                    .model_ref
                    .as_ref()
                    .is_some_and(|actual| model_ref_matches(&expected, actual))
        })
        .collect::<Vec<_>>();
    match matching.len() {
        1 => {
            let observation = matching.into_iter().next().unwrap();
            return match project_download(observation, &requirement.artifact) {
                Some(observed) => Ok(Some(observed)),
                None => resolver.resolve_local(&pinned_requirement).await.map(Some),
            };
        }
        0 => {}
        _ => {
            return Ok(Some(unavailable(
                IntentDiagnosticCode::DownloadCustodyUnavailable,
                "acquisition",
                "multiple owned downloads claim the same resolved requirement",
            )))
        }
    }
    if owner_snapshot.owner_closed {
        return Ok(Some(ObservedModelState::Blocked {
            resolved_requirement: Some(pinned_requirement),
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::AcquisitionBlocked,
                "acquisition",
                "download ownership is closed",
            )],
        }));
    }
    let local = resolver.resolve_local(&pinned_requirement).await?;
    if !matches!(
        local,
        ObservedModelState::Missing { .. } | ObservedModelState::Unsatisfied { .. }
    ) {
        return Ok(Some(local));
    }
    let download_id = match PumasApi::start_prepared_hf_download_owned(
        library,
        client.clone(),
        prepared,
        Some(&expected),
    )
    .await
    {
        Ok(download_id) => download_id,
        Err(error) => return Ok(Some(classify_admission_error(error))),
    };
    let snapshot = client.intent_download_snapshot().await;
    let Some(observation) = snapshot
        .downloads
        .into_iter()
        .find(|item| item.download_id == download_id)
    else {
        return Ok(Some(unavailable(
            IntentDiagnosticCode::DownloadCustodyUnavailable,
            "acquisition",
            "managed acquisition was admitted but its owned state is not observable",
        )));
    };
    match project_download(observation, &requirement.artifact) {
        Some(observed) => Ok(Some(observed)),
        None => resolver.resolve_local(&pinned_requirement).await.map(Some),
    }
}

// Expected upstream refusals are observed results. Filesystem/cache publication
// failures must still reach the task owner as failed effects; panics are retained
// by run_fallible_async_named independently of this result classification.
fn observe_upstream_result<T>(observed: Result<T>) -> Result<Result<T>> {
    match observed {
        Ok(value) => Ok(Ok(value)),
        Err(
            error @ (crate::PumasError::Network { .. }
            | crate::PumasError::DownloadLifecycleClosed
            | crate::PumasError::Timeout(_)
            | crate::PumasError::RateLimited { .. }
            | crate::PumasError::CircuitBreakerOpen { .. }
            | crate::PumasError::Validation { .. }
            | crate::PumasError::Json { .. }),
        ) => Ok(Err(error)),
        Err(error) => Err(error),
    }
}

pub(crate) async fn observe_acquisition(
    client: &HuggingFaceClient,
    requirement: &ModelRequirement,
) -> Result<Option<ObservedModelState>> {
    let ModelSelector::LocalModel { model_ref } = &requirement.selector else {
        return Ok(None);
    };
    let snapshot = client.intent_download_snapshot().await;
    let matching = snapshot
        .downloads
        .into_iter()
        .filter(|item| {
            item.model_ref.as_ref().is_some_and(|actual| {
                model_ref_matches(model_ref, actual)
                    && requirement
                        .artifact
                        .selected_artifact_id
                        .as_deref()
                        .is_none_or(|required| {
                            actual.selected_artifact_id.as_deref() == Some(required)
                        })
            })
        })
        .collect::<Vec<_>>();
    match matching.len() {
        0 => Ok(None),
        1 => Ok(project_download(
            matching.into_iter().next().unwrap(),
            &requirement.artifact,
        )),
        _ => Ok(Some(ObservedModelState::Unavailable {
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::DownloadCustodyUnavailable,
                "acquisition",
                "multiple owned downloads claim the same resolved requirement",
            )],
        })),
    }
}

fn model_ref_matches(required: &PumasModelRef, actual: &PumasModelRef) -> bool {
    required.model_id == actual.model_id
        && required
            .revision
            .as_deref()
            .is_none_or(|value| actual.revision.as_deref() == Some(value))
        && required
            .selected_artifact_id
            .as_deref()
            .is_none_or(|value| actual.selected_artifact_id.as_deref() == Some(value))
        && required
            .selected_artifact_path
            .as_deref()
            .is_none_or(|value| actual.selected_artifact_path.as_deref() == Some(value))
}

fn project_download(
    observation: crate::model_library::IntentDownloadObservation,
    artifact: &ArtifactRequirement,
) -> Option<ObservedModelState> {
    let resolved_requirement = observation
        .model_ref
        .map(|model_ref| resolved_requirement(model_ref, artifact));
    let progress = ModelAcquisitionProgress {
        stage: if observation.blocked {
            AcquisitionStage::Paused
        } else {
            match observation.status {
                DownloadStatus::Queued => AcquisitionStage::Queued,
                DownloadStatus::Downloading | DownloadStatus::Pausing => {
                    AcquisitionStage::Transferring
                }
                DownloadStatus::Paused => AcquisitionStage::Paused,
                DownloadStatus::Completed => AcquisitionStage::Verifying,
                DownloadStatus::Cancelling | DownloadStatus::Cancelled | DownloadStatus::Error => {
                    AcquisitionStage::Paused
                }
            }
        },
        downloaded_bytes: Some(observation.downloaded_bytes),
        total_bytes: observation.total_bytes,
        retry_attempt: None,
        retry_limit: None,
    };
    let hint = Some(AcquisitionHint {
        download_id: observation.download_id,
    });
    let Some(resolved_requirement) = resolved_requirement else {
        return Some(unavailable(
            IntentDiagnosticCode::DownloadCustodyUnavailable,
            "acquisition",
            "owned download state does not expose a stable resolved model identity",
        ));
    };
    if observation.blocked {
        return Some(ObservedModelState::Blocked {
            resolved_requirement: Some(resolved_requirement),
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::AcquisitionBlocked,
                "acquisition",
                "managed acquisition custody requires explicit lifecycle recovery",
            )],
        });
    }
    match observation.status {
        DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Pausing => {
            Some(ObservedModelState::Acquiring {
                resolved_requirement,
                download_hint: hint,
                progress: Some(progress),
            })
        }
        DownloadStatus::Paused | DownloadStatus::Cancelling => Some(ObservedModelState::Blocked {
            resolved_requirement: Some(resolved_requirement),
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::AcquisitionBlocked,
                "acquisition",
                "managed acquisition requires lifecycle recovery or resume",
            )],
        }),
        DownloadStatus::Error | DownloadStatus::Cancelled => Some(failed(
            IntentDiagnosticCode::AcquisitionFailed,
            "acquisition",
            "managed acquisition failed",
            Some(resolved_requirement),
        )),
        // Completion is an acquisition observation, not an availability proof.
        // Resolve current indexed package facts through the local contract.
        DownloadStatus::Completed => None,
    }
}

fn resolved_requirement(
    model_ref: PumasModelRef,
    artifact: &ArtifactRequirement,
) -> ModelRequirement {
    let mut artifact = artifact.clone();
    artifact.selected_artifact_id = model_ref.selected_artifact_id.clone();
    ModelRequirement {
        artifact,
        selector: ModelSelector::LocalModel { model_ref },
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    }
}

fn pinned_upstream_requirement(
    selected: &SelectedUpstreamArtifact,
    revision: &DownloadRevision,
    original: &ModelRequirement,
) -> ModelRequirement {
    let mut artifact = original.artifact.clone();
    artifact.selected_artifact_id = Some(selected.identity.artifact_id.clone());
    ModelRequirement {
        selector: ModelSelector::UpstreamRepository {
            repository_id: selected.request.repo_id.clone(),
            revision: Some(revision.as_str().to_string()),
        },
        artifact,
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    }
}

struct SelectedUpstreamArtifact {
    request: DownloadRequest,
    identity: SelectedArtifactIdentity,
    kind: PackageArtifactKind,
}

fn select_artifact(
    repository_id: &str,
    requirement: &ArtifactRequirement,
    lfs_files: &[LfsFileInfo],
    regular_files: &[String],
    revision: &DownloadRevision,
) -> std::result::Result<SelectedUpstreamArtifact, Box<ObservedModelState>> {
    let format = requirement.format;
    if matches!(
        format,
        Some(
            PackageArtifactKind::HfCompatibleDirectory
                | PackageArtifactKind::DiffusersBundle
                | PackageArtifactKind::Adapter
                | PackageArtifactKind::Shard
                | PackageArtifactKind::Unknown
        )
    ) {
        return Err(Box::new(unsupported_layout()));
    }
    let mut matches = lfs_files
        .iter()
        .filter_map(|file| artifact_kind_for_filename(&file.filename).map(|kind| (file, kind)))
        .filter(|(_, kind)| format.is_none_or(|required| required == *kind))
        .filter(|(file, _)| {
            requirement
                .quantization
                .as_deref()
                .is_none_or(|quantization| {
                    normalized_token(&file.filename).contains(&normalized_token(quantization))
                })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.0.filename.cmp(&right.0.filename));

    if let Some(required_id) = non_empty(requirement.selected_artifact_id.as_deref()) {
        matches.retain(|(file, kind)| {
            let request = download_request(repository_id, *kind, &file.filename);
            SelectedArtifactIdentity::from_download_request_at_revision(
                &request,
                Some(vec![file.filename.clone()]),
                revision,
            )
            .artifact_id
                == required_id
        });
    }
    let has_directory_manifest = regular_files
        .iter()
        .chain(lfs_files.iter().map(|file| &file.filename))
        .any(|filename| {
            matches!(
                filename.rsplit('/').next(),
                Some("config.json" | "model_index.json")
            ) || filename.ends_with(".safetensors.index.json")
        });
    let rejected_directory_artifact = has_directory_manifest
        && matches
            .iter()
            .any(|(_, kind)| *kind == PackageArtifactKind::Safetensors);
    if rejected_directory_artifact {
        matches.retain(|(_, kind)| *kind != PackageArtifactKind::Safetensors);
        if matches.is_empty() {
            return Err(Box::new(unsupported_layout()));
        }
    }
    match matches.as_slice() {
        [] => {
            let unverified_match = regular_files.iter().any(|filename| {
                artifact_kind_for_filename(filename).is_some_and(|kind| {
                    format.is_none_or(|required| required == kind)
                        && requirement.quantization.as_deref().is_none_or(|quantization| {
                            normalized_token(filename).contains(&normalized_token(quantization))
                        })
                })
            });
            if unverified_match {
                Err(Box::new(ObservedModelState::Unsupported {
                    diagnostics: vec![diagnostic(
                        IntentDiagnosticCode::IntegrityEvidenceMissing,
                        "artifact",
                        "the matching upstream artifact has no trusted LFS size and SHA-256 evidence",
                    )],
                }))
            } else {
                Err(Box::new(ObservedModelState::Unsatisfied {
                    candidates: Vec::new(),
                    diagnostics: vec![diagnostic(
                        IntentDiagnosticCode::ArtifactMissing,
                        "artifact",
                        "the pinned upstream repository has no supported artifact satisfying the requirement",
                    )],
                }))
            }
        }
        [(file, kind)] => {
            let request = download_request(repository_id, *kind, &file.filename);
            let identity = SelectedArtifactIdentity::from_download_request_at_revision(
                &request,
                Some(vec![file.filename.clone()]),
                revision,
            );
            Ok(SelectedUpstreamArtifact {
                request,
                identity,
                kind: *kind,
            })
        }
        _ => Err(Box::new(ObservedModelState::UpstreamAmbiguous {
            candidates: matches
                .into_iter()
                .map(|(file, kind)| {
                    let request = download_request(repository_id, kind, &file.filename);
                    let identity = SelectedArtifactIdentity::from_download_request_at_revision(
                        &request,
                        Some(vec![file.filename.clone()]),
                        revision,
                    );
                    UpstreamArtifactCandidate {
                        repository_id: repository_id.to_string(),
                        revision: revision.as_str().to_string(),
                        selected_artifact_id: identity.artifact_id,
                        filenames: vec![file.filename.clone()],
                        format: kind,
                        quantization_hint: requirement.quantization.clone(),
                    }
                })
                .collect(),
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::UpstreamArtifactAmbiguous,
                "artifact",
                "multiple pinned upstream artifacts satisfy the requirement; select one artifact id",
            )],
        })),
    }
}

fn download_request(
    repository_id: &str,
    kind: PackageArtifactKind,
    filename: &str,
) -> DownloadRequest {
    let (owner, name) = repository_id
        .split_once('/')
        .expect("validated upstream repository identity");
    DownloadRequest {
        repo_id: repository_id.to_string(),
        family: owner.to_string(),
        official_name: name.to_string(),
        model_type: (kind == PackageArtifactKind::Gguf).then(|| "llm".to_string()),
        // The exact filename owns transfer selection. Quantization remains an
        // intent constraint verified from local artifact facts after import.
        quant: None,
        filename: Some(filename.to_string()),
        filenames: None,
        pipeline_tag: None,
        bundle_format: None,
        pipeline_class: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    }
}

fn artifact_kind_for_filename(filename: &str) -> Option<PackageArtifactKind> {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".gguf") {
        Some(PackageArtifactKind::Gguf)
    } else if lower.ends_with(".onnx") {
        Some(PackageArtifactKind::Onnx)
    } else if lower.ends_with(".safetensors") {
        Some(PackageArtifactKind::Safetensors)
    } else {
        None
    }
}

fn normalized_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn unavailable(code: IntentDiagnosticCode, field: &str, message: &str) -> ObservedModelState {
    ObservedModelState::Unavailable {
        diagnostics: vec![diagnostic(code, field, message)],
    }
}

fn failed(
    code: IntentDiagnosticCode,
    field: &str,
    message: &str,
    resolved_requirement: Option<ModelRequirement>,
) -> ObservedModelState {
    ObservedModelState::Failed {
        resolved_requirement,
        diagnostics: vec![diagnostic(code, field, message)],
    }
}

pub(super) fn classify_admission_error(error: crate::PumasError) -> ObservedModelState {
    tracing::warn!(%error, "intent acquisition admission failed");
    match error {
        crate::PumasError::DownloadLifecycleClosed
        | crate::PumasError::DownloadRootBusy
        | crate::PumasError::DownloadPaused => ObservedModelState::Blocked {
            resolved_requirement: None,
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::AcquisitionBlocked,
                "acquisition",
                "managed acquisition is blocked by its lifecycle owner",
            )],
        },
        crate::PumasError::Validation { ref field, .. } if field.starts_with("download_") => {
            ObservedModelState::Blocked {
                resolved_requirement: None,
                diagnostics: vec![diagnostic(
                    IntentDiagnosticCode::DownloadCustodyUnavailable,
                    "acquisition",
                    "managed acquisition custody could not be established",
                )],
            }
        }
        crate::PumasError::Network { .. }
        | crate::PumasError::Timeout(_)
        | crate::PumasError::RateLimited { .. }
        | crate::PumasError::CircuitBreakerOpen { .. } => unavailable(
            IntentDiagnosticCode::UpstreamResolutionFailed,
            "acquisition",
            "upstream became unavailable before admission",
        ),
        _ => failed(
            IntentDiagnosticCode::AcquisitionFailed,
            "acquisition",
            "managed acquisition was not admitted",
            None,
        ),
    }
}

fn unsupported_layout() -> ObservedModelState {
    ObservedModelState::Unsupported {
        diagnostics: vec![diagnostic(
            IntentDiagnosticCode::UnsupportedUpstreamLayout,
            "artifact.format",
            "this upstream package layout cannot yet be selected without fabricating a local artifact path",
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    fn lfs(filename: &str) -> LfsFileInfo {
        LfsFileInfo {
            filename: filename.to_string(),
            size: 1,
            sha256: "a".repeat(64),
        }
    }

    #[test]
    fn upstream_refusals_are_observations_but_cache_io_failures_remain_failed_effects() {
        let refused = observe_upstream_result::<()>(Err(crate::PumasError::Network {
            message: "HTTP 403".to_string(),
            cause: None,
        }));
        assert!(matches!(
            refused,
            Ok(Err(crate::PumasError::Network { .. }))
        ));
        let cache_failure = crate::PumasError::io_with_path(
            std::io::Error::other("cache publication failed"),
            std::path::Path::new("cache.json"),
        );
        assert!(matches!(
            observe_upstream_result::<()>(Err(cache_failure)),
            Err(crate::PumasError::Io { .. })
        ));
    }

    #[test]
    fn completed_download_defers_to_local_availability_resolution() {
        let observation = crate::model_library::IntentDownloadObservation {
            download_id: "completed".to_string(),
            model_ref: Some(PumasModelRef {
                model_ref_contract_version: crate::models::PUMAS_MODEL_REF_CONTRACT_VERSION,
                model_id: "llm/example/model".to_string(),
                revision: Some(COMMIT.to_string()),
                selected_artifact_id: Some("artifact".to_string()),
                selected_artifact_path: None,
                migration_diagnostics: Vec::new(),
            }),
            repo_id: "example/model".to_string(),
            status: DownloadStatus::Completed,
            downloaded_bytes: 1,
            total_bytes: Some(1),
            blocked: false,
        };
        assert!(project_download(observation, &ArtifactRequirement::default()).is_none());
    }

    #[test]
    fn upstream_selection_reports_actionable_ambiguity_and_accepts_selected_id() {
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        let files = [lfs("alpha-Q4_K_M.gguf"), lfs("beta-Q4_K_M.gguf")];
        let requirement = ArtifactRequirement {
            format: Some(PackageArtifactKind::Gguf),
            quantization: Some("Q4_K_M".to_string()),
            ..ArtifactRequirement::default()
        };
        let ambiguous = select_artifact("acme/model", &requirement, &files, &[], &revision)
            .err()
            .unwrap();
        let ObservedModelState::UpstreamAmbiguous { candidates, .. } = *ambiguous else {
            panic!("multiple supported artifacts must remain ambiguous")
        };
        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .all(|candidate| candidate.revision == COMMIT));
        assert_ne!(
            candidates[0].selected_artifact_id,
            candidates[1].selected_artifact_id
        );

        let selected_id = candidates[0].selected_artifact_id.clone();
        let selected = select_artifact(
            "acme/model",
            &ArtifactRequirement {
                selected_artifact_id: Some(selected_id),
                ..requirement
            },
            &files,
            &[],
            &revision,
        )
        .unwrap();
        assert_eq!(
            selected.request.filename.as_deref(),
            Some(candidates[0].filenames[0].as_str())
        );
    }

    #[test]
    fn upstream_selection_filters_quantization_without_guessing_directory_layouts() {
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        let files = [lfs("model-Q4_K_M.gguf"), lfs("model-Q8_0.gguf")];
        let selected = select_artifact(
            "acme/model",
            &ArtifactRequirement {
                format: Some(PackageArtifactKind::Gguf),
                quantization: Some("q4-k-m".to_string()),
                selected_artifact_id: None,
            },
            &files,
            &[],
            &revision,
        )
        .unwrap();
        assert_eq!(
            selected.request.filename.as_deref(),
            Some("model-Q4_K_M.gguf")
        );

        assert!(matches!(
            select_artifact(
                "acme/model",
                &ArtifactRequirement {
                    format: Some(PackageArtifactKind::HfCompatibleDirectory),
                    ..ArtifactRequirement::default()
                },
                &files,
                &[],
                &revision,
            ),
            Err(observed) if matches!(*observed, ObservedModelState::Unsupported { .. })
        ));

        let safetensors = [lfs("model.safetensors")];
        assert!(matches!(
            select_artifact(
                "acme/model",
                &ArtifactRequirement {
                    format: Some(PackageArtifactKind::Safetensors),
                    ..ArtifactRequirement::default()
                },
                &safetensors,
                &["config.json".to_string()],
                &revision,
            ),
            Err(observed) if matches!(*observed, ObservedModelState::Unsupported { .. })
        ));
        assert!(select_artifact(
            "acme/model",
            &ArtifactRequirement {
                format: Some(PackageArtifactKind::Safetensors),
                ..ArtifactRequirement::default()
            },
            &safetensors,
            &[],
            &revision,
        )
        .is_ok());
    }
}
