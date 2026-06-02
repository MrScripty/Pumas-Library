use std::collections::BTreeSet;
use std::path::Path;

use crate::index::{
    classify_package_facts_cache_record, ModelIndex, ModelPackageFactsCacheRecord,
    ModelPackageFactsCacheRowState, ModelPackageFactsCacheScope,
};
use crate::models::{
    ModelArtifactState, ModelEntryPathState, PackageArtifactKind, PumasArtifactLoadPathKind,
    PumasArtifactLoadTarget, PumasArtifactLoadTargetDiagnostic,
    PumasArtifactLoadTargetDiagnosticCode, ResolveModelArtifactLoadTargetRequest,
    ResolveModelArtifactLoadTargetResponse, ResolvedArtifactFacts, ResolvedModelPackageFacts,
    ResolvedModelPackageFactsSummary,
};
use crate::Result;

pub(crate) fn resolve_artifact_load_target_from_index(
    index: &ModelIndex,
    request: ResolveModelArtifactLoadTargetRequest,
) -> Result<ResolveModelArtifactLoadTargetResponse> {
    if request.model_ref.model_id.trim().is_empty()
        || !request.model_ref.migration_diagnostics.is_empty()
    {
        return Ok(non_ready_response(
            ModelArtifactState::Missing,
            ModelEntryPathState::Missing,
            PumasArtifactLoadTargetDiagnosticCode::MissingModel,
            Some("model_ref"),
            "model_ref does not identify a resolved Pumas model",
        ));
    }

    if index.get(&request.model_ref.model_id)?.is_none() {
        return Ok(non_ready_response(
            ModelArtifactState::Missing,
            ModelEntryPathState::Missing,
            PumasArtifactLoadTargetDiagnosticCode::MissingModel,
            Some("model_ref.model_id"),
            "model record is not present in the Pumas index",
        ));
    }

    let selected_artifact_path =
        normalized_non_empty(request.model_ref.selected_artifact_path.as_deref());
    let selected_artifact_id = match normalized_selected_artifact_id(
        request.model_ref.selected_artifact_id.as_deref(),
    ) {
        Some(selected_artifact_id) => selected_artifact_id.to_string(),
        None => {
            let identity = selected_artifact_id_from_path(
                index,
                &request.model_ref.model_id,
                selected_artifact_path,
            )?;
            match identity {
                SelectedArtifactIdentity::Resolved(selected_artifact_id) => selected_artifact_id,
                SelectedArtifactIdentity::MissingIdentity => {
                    return Ok(missing_selected_artifact_response(
                        "model_ref.selected_artifact_id",
                        "model_ref must identify one selected artifact before load-target resolution",
                    ));
                }
                SelectedArtifactIdentity::PathNotIndexed => {
                    return Ok(non_ready_response(
                        ModelArtifactState::Missing,
                        ModelEntryPathState::Missing,
                        PumasArtifactLoadTargetDiagnosticCode::ArtifactMissing,
                        Some("model_ref.selected_artifact_path"),
                        "selected artifact path is not present in indexed package facts",
                    ));
                }
                SelectedArtifactIdentity::AmbiguousPath => {
                    return Ok(missing_selected_artifact_response(
                        "model_ref.selected_artifact_path",
                        "selected artifact path matches multiple indexed artifacts",
                    ));
                }
            }
        }
    };

    let summary_record = index.get_model_package_facts_cache(
        &request.model_ref.model_id,
        Some(&selected_artifact_id),
        ModelPackageFactsCacheScope::Summary,
    )?;
    let (summary_state, summary) = classify_package_facts_cache_record::<
        ResolvedModelPackageFactsSummary,
    >(Some(&selected_artifact_id), None, summary_record.as_ref());
    if let Some(summary) = summary {
        return Ok(response_from_summary(&request, summary));
    }
    if summary_state != ModelPackageFactsCacheRowState::Missing {
        return Ok(response_from_cache_state(summary_state));
    }

    let detail_record = index.get_model_package_facts_cache(
        &request.model_ref.model_id,
        Some(&selected_artifact_id),
        ModelPackageFactsCacheScope::Detail,
    )?;
    let (detail_state, facts) = classify_package_facts_cache_record::<ResolvedModelPackageFacts>(
        Some(&selected_artifact_id),
        None,
        detail_record.as_ref(),
    );
    if let Some(facts) = facts {
        return Ok(response_from_artifact(
            &request,
            facts.model_ref,
            facts.artifact,
            Some(facts.package_facts_contract_version),
        ));
    }

    Ok(response_from_cache_state(detail_state))
}

enum SelectedArtifactIdentity {
    Resolved(String),
    MissingIdentity,
    PathNotIndexed,
    AmbiguousPath,
}

fn selected_artifact_id_from_path(
    index: &ModelIndex,
    model_id: &str,
    selected_artifact_path: Option<&str>,
) -> Result<SelectedArtifactIdentity> {
    let Some(selected_artifact_path) = selected_artifact_path else {
        return Ok(SelectedArtifactIdentity::MissingIdentity);
    };

    let mut matches = BTreeSet::new();
    collect_matching_selected_artifact_ids(
        &mut matches,
        selected_artifact_path,
        index.list_model_package_facts_cache(model_id, ModelPackageFactsCacheScope::Summary)?,
    );
    collect_matching_selected_artifact_ids(
        &mut matches,
        selected_artifact_path,
        index.list_model_package_facts_cache(model_id, ModelPackageFactsCacheScope::Detail)?,
    );

    let mut matches = matches.into_iter();
    match (matches.next(), matches.next()) {
        (None, _) => Ok(SelectedArtifactIdentity::PathNotIndexed),
        (Some(selected_artifact_id), None) => {
            Ok(SelectedArtifactIdentity::Resolved(selected_artifact_id))
        }
        (Some(_), Some(_)) => Ok(SelectedArtifactIdentity::AmbiguousPath),
    }
}

fn collect_matching_selected_artifact_ids(
    matches: &mut BTreeSet<String>,
    selected_artifact_path: &str,
    records: Vec<ModelPackageFactsCacheRecord>,
) {
    for record in records {
        if let Some((record_path, selected_artifact_id)) =
            selected_artifact_identity_from_cache_record(&record)
        {
            if record_path == selected_artifact_path {
                matches.insert(selected_artifact_id);
            }
        }
    }
}

fn selected_artifact_identity_from_cache_record(
    record: &ModelPackageFactsCacheRecord,
) -> Option<(String, String)> {
    match record.cache_scope {
        ModelPackageFactsCacheScope::Summary => {
            let summary =
                serde_json::from_str::<ResolvedModelPackageFactsSummary>(&record.facts_json)
                    .ok()?;
            selected_artifact_identity_from_model_ref(
                record.selected_artifact_id.as_str(),
                summary.model_ref,
            )
        }
        ModelPackageFactsCacheScope::Detail => {
            let facts =
                serde_json::from_str::<ResolvedModelPackageFacts>(&record.facts_json).ok()?;
            selected_artifact_identity_from_model_ref(
                record.selected_artifact_id.as_str(),
                facts.model_ref,
            )
        }
    }
}

fn selected_artifact_identity_from_model_ref(
    row_selected_artifact_id: &str,
    model_ref: crate::models::PumasModelRef,
) -> Option<(String, String)> {
    let selected_artifact_path =
        normalized_non_empty(model_ref.selected_artifact_path.as_deref())?.to_string();
    let selected_artifact_id = normalized_selected_artifact_id(Some(row_selected_artifact_id))
        .or_else(|| normalized_selected_artifact_id(model_ref.selected_artifact_id.as_deref()))?
        .to_string();
    Some((selected_artifact_path, selected_artifact_id))
}

pub(crate) fn mode_not_allowed_response() -> ResolveModelArtifactLoadTargetResponse {
    non_ready_response(
        ModelArtifactState::Stale,
        ModelEntryPathState::Stale,
        PumasArtifactLoadTargetDiagnosticCode::ModeNotAllowed,
        Some("resolution_mode"),
        "PumasReadOnlyLibrary cannot perform owner-fresh artifact resolution",
    )
}

pub(crate) fn library_unavailable_response() -> ResolveModelArtifactLoadTargetResponse {
    non_ready_response(
        ModelArtifactState::Stale,
        ModelEntryPathState::Stale,
        PumasArtifactLoadTargetDiagnosticCode::LibraryUnavailable,
        Some("model_ref.model_id"),
        "model library could not refresh indexed state before load-target resolution",
    )
}

fn response_from_summary(
    request: &ResolveModelArtifactLoadTargetRequest,
    summary: ResolvedModelPackageFactsSummary,
) -> ResolveModelArtifactLoadTargetResponse {
    response_from_artifact(
        request,
        summary.model_ref,
        ResolvedArtifactFacts {
            artifact_kind: summary.artifact_kind,
            entry_path: summary.entry_path,
            storage_kind: summary.storage_kind,
            validation_state: summary.validation_state,
            validation_errors: Vec::new(),
            companion_artifacts: Vec::new(),
            sibling_files: Vec::new(),
            selected_files: Vec::new(),
            logical_size: None,
        },
        Some(summary.package_facts_contract_version),
    )
}

fn response_from_artifact(
    request: &ResolveModelArtifactLoadTargetRequest,
    resolved_model_ref: crate::models::PumasModelRef,
    artifact: ResolvedArtifactFacts,
    package_facts_contract_version: Option<u32>,
) -> ResolveModelArtifactLoadTargetResponse {
    if selected_artifact_path_mismatch(request, &resolved_model_ref) {
        return non_ready_response(
            ModelArtifactState::Stale,
            ModelEntryPathState::Stale,
            PumasArtifactLoadTargetDiagnosticCode::SelectedArtifactMismatch,
            Some("model_ref.selected_artifact_path"),
            "selected artifact path no longer matches cached package facts",
        );
    }

    if let Some(observed_version) = request.caller_observed_package_facts_contract_version {
        if Some(observed_version) != package_facts_contract_version {
            return non_ready_response(
                ModelArtifactState::Stale,
                ModelEntryPathState::Stale,
                PumasArtifactLoadTargetDiagnosticCode::StalePackageFacts,
                Some("caller_observed_package_facts_contract_version"),
                "caller-observed package facts contract version is stale",
            );
        }
    }

    if let Some(observed_entry_path) = request.caller_observed_entry_path.as_deref() {
        if observed_entry_path != artifact.entry_path {
            return non_ready_response(
                ModelArtifactState::Stale,
                ModelEntryPathState::Stale,
                PumasArtifactLoadTargetDiagnosticCode::SelectedArtifactMismatch,
                Some("caller_observed_entry_path"),
                "caller-observed entry path no longer matches cached package facts",
            );
        }
    }

    if let Some(expected_kind) = request.expected_artifact_kind {
        if expected_kind != artifact.artifact_kind {
            return non_ready_response(
                ModelArtifactState::Ready,
                ModelEntryPathState::Ready,
                PumasArtifactLoadTargetDiagnosticCode::ArtifactKindMismatch,
                Some("expected_artifact_kind"),
                "selected artifact kind does not match the requested runtime family",
            );
        }
    }

    let artifact_state = artifact_state(artifact.validation_state);
    let entry_path_state = entry_path_state(&artifact.entry_path, artifact_state);
    if entry_path_state != ModelEntryPathState::Ready {
        let diagnostic = if artifact_state == ModelArtifactState::Invalid {
            PumasArtifactLoadTargetDiagnosticCode::InvalidArtifact
        } else {
            diagnostic_for_entry_path_state(entry_path_state)
        };
        return non_ready_response(
            artifact_state,
            entry_path_state,
            diagnostic,
            Some("target.local_load_path"),
            "selected artifact does not have a loadable local entry path",
        );
    }

    ResolveModelArtifactLoadTargetResponse {
        artifact_state,
        entry_path_state,
        target: Some(PumasArtifactLoadTarget {
            model_ref: resolved_model_ref,
            artifact_kind: artifact.artifact_kind,
            local_load_path: artifact.entry_path,
            load_path_kind: load_path_kind(artifact.artifact_kind),
            library_root_id: None,
            storage_kind: artifact.storage_kind,
            validation_state: artifact.validation_state,
            content_fingerprint: None,
            package_facts_contract_version,
        }),
        diagnostics: Vec::new(),
    }
}

fn response_from_cache_state(
    state: ModelPackageFactsCacheRowState,
) -> ResolveModelArtifactLoadTargetResponse {
    match state {
        ModelPackageFactsCacheRowState::Missing => non_ready_response(
            ModelArtifactState::NeedsDetail,
            ModelEntryPathState::NeedsDetail,
            PumasArtifactLoadTargetDiagnosticCode::ArtifactNeedsDetail,
            Some("model_ref"),
            "selected artifact needs cached package facts before load-target resolution",
        ),
        ModelPackageFactsCacheRowState::StaleContract
        | ModelPackageFactsCacheRowState::StaleFingerprint => non_ready_response(
            ModelArtifactState::Stale,
            ModelEntryPathState::Stale,
            PumasArtifactLoadTargetDiagnosticCode::StalePackageFacts,
            Some("model_ref"),
            "cached package facts are stale for the selected artifact",
        ),
        ModelPackageFactsCacheRowState::InvalidJson => non_ready_response(
            ModelArtifactState::Invalid,
            ModelEntryPathState::Invalid,
            PumasArtifactLoadTargetDiagnosticCode::InvalidPackageFacts,
            Some("model_ref"),
            "cached package facts cannot be decoded",
        ),
        ModelPackageFactsCacheRowState::WrongSelectedArtifact => non_ready_response(
            ModelArtifactState::Stale,
            ModelEntryPathState::Stale,
            PumasArtifactLoadTargetDiagnosticCode::SelectedArtifactMismatch,
            Some("model_ref.selected_artifact_id"),
            "cached package facts refer to a different selected artifact",
        ),
        ModelPackageFactsCacheRowState::Fresh => non_ready_response(
            ModelArtifactState::Invalid,
            ModelEntryPathState::Invalid,
            PumasArtifactLoadTargetDiagnosticCode::InvalidPackageFacts,
            Some("model_ref"),
            "fresh cache state did not include decoded package facts",
        ),
    }
}

fn missing_selected_artifact_response(
    field_path: &str,
    message: &str,
) -> ResolveModelArtifactLoadTargetResponse {
    non_ready_response(
        ModelArtifactState::Ambiguous,
        ModelEntryPathState::Ambiguous,
        PumasArtifactLoadTargetDiagnosticCode::MissingSelectedArtifact,
        Some(field_path),
        message,
    )
}

fn non_ready_response(
    artifact_state: ModelArtifactState,
    entry_path_state: ModelEntryPathState,
    code: PumasArtifactLoadTargetDiagnosticCode,
    field_path: Option<&str>,
    message: &str,
) -> ResolveModelArtifactLoadTargetResponse {
    ResolveModelArtifactLoadTargetResponse {
        artifact_state,
        entry_path_state,
        target: None,
        diagnostics: vec![PumasArtifactLoadTargetDiagnostic {
            code,
            field_path: field_path.map(ToOwned::to_owned),
            message: message.to_string(),
        }],
    }
}

fn selected_artifact_path_mismatch(
    request: &ResolveModelArtifactLoadTargetRequest,
    resolved: &crate::models::PumasModelRef,
) -> bool {
    match (
        normalized_non_empty(request.model_ref.selected_artifact_path.as_deref()),
        normalized_non_empty(resolved.selected_artifact_path.as_deref()),
    ) {
        (Some(request_path), Some(resolved_path)) => request_path != resolved_path,
        _ => false,
    }
}

fn normalized_selected_artifact_id(value: Option<&str>) -> Option<&str> {
    normalized_non_empty(value)
}

fn normalized_non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn artifact_state(validation_state: crate::models::AssetValidationState) -> ModelArtifactState {
    match validation_state {
        crate::models::AssetValidationState::Valid => ModelArtifactState::Ready,
        crate::models::AssetValidationState::Degraded => ModelArtifactState::Partial,
        crate::models::AssetValidationState::Invalid => ModelArtifactState::Invalid,
    }
}

fn entry_path_state(entry_path: &str, artifact_state: ModelArtifactState) -> ModelEntryPathState {
    if entry_path.trim().is_empty() {
        return ModelEntryPathState::Missing;
    }
    if !Path::new(entry_path).is_absolute() {
        return ModelEntryPathState::Invalid;
    }

    match artifact_state {
        ModelArtifactState::Ready => ModelEntryPathState::Ready,
        ModelArtifactState::Missing => ModelEntryPathState::Missing,
        ModelArtifactState::Partial => ModelEntryPathState::Partial,
        ModelArtifactState::Invalid => ModelEntryPathState::Invalid,
        ModelArtifactState::Ambiguous => ModelEntryPathState::Ambiguous,
        ModelArtifactState::NeedsDetail => ModelEntryPathState::NeedsDetail,
        ModelArtifactState::Stale => ModelEntryPathState::Stale,
    }
}

fn diagnostic_for_entry_path_state(
    entry_path_state: ModelEntryPathState,
) -> PumasArtifactLoadTargetDiagnosticCode {
    match entry_path_state {
        ModelEntryPathState::Missing => PumasArtifactLoadTargetDiagnosticCode::ArtifactPathMissing,
        ModelEntryPathState::Partial => PumasArtifactLoadTargetDiagnosticCode::ArtifactPartial,
        ModelEntryPathState::Invalid => {
            PumasArtifactLoadTargetDiagnosticCode::ArtifactPathNotLoadable
        }
        ModelEntryPathState::Ambiguous => {
            PumasArtifactLoadTargetDiagnosticCode::SelectedArtifactMismatch
        }
        ModelEntryPathState::NeedsDetail => {
            PumasArtifactLoadTargetDiagnosticCode::ArtifactNeedsDetail
        }
        ModelEntryPathState::Stale => PumasArtifactLoadTargetDiagnosticCode::StalePackageFacts,
        ModelEntryPathState::Ready => PumasArtifactLoadTargetDiagnosticCode::InvalidArtifact,
    }
}

fn load_path_kind(artifact_kind: PackageArtifactKind) -> PumasArtifactLoadPathKind {
    match artifact_kind {
        PackageArtifactKind::DiffusersBundle | PackageArtifactKind::HfCompatibleDirectory => {
            PumasArtifactLoadPathKind::Directory
        }
        PackageArtifactKind::Gguf
        | PackageArtifactKind::Safetensors
        | PackageArtifactKind::Onnx
        | PackageArtifactKind::Adapter
        | PackageArtifactKind::Shard
        | PackageArtifactKind::Unknown => PumasArtifactLoadPathKind::File,
    }
}
