use super::types::*;
use crate::index::{ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope};
use crate::model_library::ModelLibrary;
use crate::models::{
    ModelArtifactState, ModelEntryPathState, ModelLibrarySelectorSnapshotRequest,
    ModelLibrarySelectorSnapshotRow, PackageArtifactKind, PumasArtifactConsumer,
    PumasArtifactLoadPathKind, PumasArtifactLoadTargetDiagnosticCode,
    PumasArtifactLoadTargetResolutionMode, ResolveModelArtifactLoadTargetRequest,
    ResolvedModelPackageFacts, PACKAGE_FACTS_CONTRACT_VERSION,
};
use crate::Result;
use std::sync::Arc;

/// Read-only intent resolution against one owned model library.
#[derive(Clone)]
pub(crate) struct IntentResolver {
    library: Arc<ModelLibrary>,
}

impl IntentResolver {
    pub(crate) fn new(library: Arc<ModelLibrary>) -> Self {
        Self { library }
    }

    /// Return local identity matches and the evidence supporting each decision.
    /// This operation performs no reconciliation, package-fact generation, or upstream access.
    pub(crate) async fn query_models(
        &self,
        requirement: &ModelRequirement,
    ) -> Result<QueryModelsOutcome> {
        let invalid = validate_requirement(requirement);
        if !invalid.is_empty() {
            return Ok(QueryModelsOutcome::InvalidRequirement {
                diagnostics: invalid,
            });
        }
        let unsupported = unsupported_diagnostics(requirement);
        if !unsupported.is_empty() {
            return Ok(QueryModelsOutcome::Unsupported {
                diagnostics: unsupported,
            });
        }

        match self.evaluate(requirement).await? {
            Observation::Current(items) => Ok(QueryModelsOutcome::Matches {
                candidates: items.into_iter().map(|item| item.candidate).collect(),
            }),
            Observation::Changed => Ok(QueryModelsOutcome::Unavailable {
                diagnostics: vec![changed_during_observation_diagnostic()],
            }),
        }
    }

    pub(crate) async fn resolve_local(
        &self,
        requirement: &ModelRequirement,
    ) -> Result<ObservedModelState> {
        let invalid = validate_requirement(requirement);
        if !invalid.is_empty() {
            return Ok(ObservedModelState::InvalidRequirement {
                diagnostics: invalid,
            });
        }
        let unsupported = unsupported_diagnostics(requirement);
        if !unsupported.is_empty() {
            return Ok(ObservedModelState::Unsupported {
                diagnostics: unsupported,
            });
        }

        let Observation::Current(evaluated) = self.evaluate(requirement).await? else {
            return Ok(ObservedModelState::Unavailable {
                diagnostics: vec![changed_during_observation_diagnostic()],
            });
        };
        let candidates = evaluated
            .iter()
            .map(|item| item.candidate.clone())
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Ok(ObservedModelState::Missing {
                diagnostics: vec![diagnostic(
                    IntentDiagnosticCode::ArtifactMissing,
                    "selector",
                    "no indexed local model has the requested identity",
                )],
            });
        }

        let ready = evaluated
            .iter()
            .filter_map(|item| item.handle.clone())
            .collect::<Vec<_>>();
        let incomplete = candidates
            .iter()
            .filter(|candidate| candidate.state == CandidateState::Incomplete)
            .count();
        if ready.len() > 1 {
            return Ok(ObservedModelState::Ambiguous { candidates });
        }
        if incomplete > 0 {
            return Ok(ObservedModelState::Incomplete {
                diagnostics: collect_diagnostics(&candidates, CandidateState::Incomplete),
                candidates,
            });
        }
        if let Some(handle) = ready.into_iter().next() {
            return Ok(ObservedModelState::Available { handle });
        }

        Ok(ObservedModelState::Unsatisfied {
            diagnostics: collect_diagnostics(&candidates, CandidateState::Unsatisfied),
            candidates,
        })
    }

    async fn evaluate(
        &self,
        requirement: &ModelRequirement,
    ) -> Result<Observation<Vec<EvaluatedCandidate>>> {
        let Observation::Current((rows, cursor)) = self.indexed_rows().await? else {
            return Ok(Observation::Changed);
        };
        let mut evaluated = Vec::new();
        for row in rows {
            let Some(mut evidence) = identity_match(requirement, &row) else {
                continue;
            };
            let mut diagnostics = Vec::new();
            evaluate_identity_constraints(requirement, &row, &mut evidence, &mut diagnostics);
            if diagnostics.iter().any(|item| {
                matches!(
                    item.code,
                    IntentDiagnosticCode::RevisionMismatch | IntentDiagnosticCode::FormatMismatch
                )
            }) {
                let cached = match row
                    .selected_artifact_id
                    .as_deref()
                    .or(row.model_ref.selected_artifact_id.as_deref())
                {
                    Some(artifact_id) => self.library.index().get_model_package_facts_cache(
                        &row.model_id,
                        Some(artifact_id),
                        ModelPackageFactsCacheScope::Detail,
                    )?,
                    None => None,
                };
                let source_diagnostic = match cached {
                    Some(cached) => self.cached_source_diagnostic(&cached, &row).await?,
                    None => Some(diagnostic(
                        IntentDiagnosticCode::PackageFactsMissing,
                        "package_facts",
                        "a proven mismatch requires current cached package facts",
                    )),
                };
                if let Some(source_diagnostic) = source_diagnostic {
                    evaluated.push(EvaluatedCandidate::without_handle(
                        row,
                        CandidateState::Incomplete,
                        evidence,
                        vec![source_diagnostic],
                    ));
                    continue;
                }
            }
            if diagnostics.iter().any(is_mismatch) {
                evaluated.push(EvaluatedCandidate::without_handle(
                    row,
                    CandidateState::Unsatisfied,
                    evidence,
                    diagnostics,
                ));
                continue;
            }
            if diagnostics.iter().any(is_incomplete) {
                evaluated.push(EvaluatedCandidate::without_handle(
                    row,
                    CandidateState::Incomplete,
                    evidence,
                    diagnostics,
                ));
                continue;
            }
            evaluated.push(
                self.evaluate_availability(requirement, row, evidence)
                    .await?,
            );
        }
        if self.observation_is_current(&cursor).await? {
            Ok(Observation::Current(evaluated))
        } else {
            Ok(Observation::Changed)
        }
    }

    async fn indexed_rows(
        &self,
    ) -> Result<Observation<(Vec<ModelLibrarySelectorSnapshotRow>, String)>> {
        let mut rows = Vec::new();
        let mut offset = 0_u32;
        let mut cursor = None;
        loop {
            let snapshot = self
                .library
                .model_library_selector_snapshot(ModelLibrarySelectorSnapshotRequest {
                    offset: Some(offset),
                    limit: Some(1000),
                    ..ModelLibrarySelectorSnapshotRequest::default()
                })
                .await?;
            match cursor.as_deref() {
                Some(expected) if expected != snapshot.cursor => return Ok(Observation::Changed),
                None => cursor = Some(snapshot.cursor.clone()),
                _ => {}
            }
            let count = snapshot.rows.len() as u32;
            rows.extend(snapshot.rows);
            offset = offset.saturating_add(count);
            if count < 1000
                || snapshot
                    .total_count
                    .is_some_and(|total| u64::from(offset) >= total)
            {
                break;
            }
        }
        Ok(Observation::Current((rows, cursor.unwrap_or_default())))
    }

    async fn observation_is_current(&self, expected_cursor: &str) -> Result<bool> {
        let snapshot = self
            .library
            .model_library_selector_snapshot(ModelLibrarySelectorSnapshotRequest {
                limit: Some(1),
                ..ModelLibrarySelectorSnapshotRequest::default()
            })
            .await?;
        Ok(snapshot.cursor == expected_cursor)
    }

    async fn cached_source_diagnostic(
        &self,
        cached: &ModelPackageFactsCacheRecord,
        row: &ModelLibrarySelectorSnapshotRow,
    ) -> Result<Option<IntentDiagnostic>> {
        if let Ok(facts) = decode_current_facts(cached) {
            if row.package_facts_summary.as_ref().is_some_and(|summary| {
                summary.artifact_kind != PackageArtifactKind::Unknown
                    && facts.artifact.artifact_kind != PackageArtifactKind::Unknown
                    && summary.artifact_kind != facts.artifact.artifact_kind
            }) {
                return Ok(Some(diagnostic(
                    IntentDiagnosticCode::PackageFactsStale,
                    "package_facts.artifact_kind",
                    "cached summary and detail disagree about artifact format",
                )));
            }
        }
        match self
            .library
            .cached_model_package_facts_are_current(
                cached,
                Some(&row.model_ref),
                row.repo_id.as_deref(),
            )
            .await
        {
            Ok(true) => Ok(None),
            Ok(false) => Ok(Some(stale_filesystem_diagnostic())),
            Err(crate::PumasError::ModelNotFound { .. })
            | Err(crate::PumasError::FileNotFound(_)) => Ok(Some(diagnostic(
                IntentDiagnosticCode::ArtifactMissing,
                "artifact",
                "indexed model package is no longer present on the filesystem",
            ))),
            Err(error) => Err(error),
        }
    }

    async fn evaluate_availability(
        &self,
        requirement: &ModelRequirement,
        row: ModelLibrarySelectorSnapshotRow,
        mut evidence: Vec<MatchEvidence>,
    ) -> Result<EvaluatedCandidate> {
        let selected_artifact_id = selected_artifact_id(requirement, &row);
        let Some(selected_artifact_id) = selected_artifact_id else {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Incomplete,
                evidence,
                vec![diagnostic(
                    IntentDiagnosticCode::ArtifactEvidenceMissing,
                    "artifact.selected_artifact_id",
                    "indexed evidence does not identify one selected artifact",
                )],
            ));
        };

        let cached = self.library.index().get_model_package_facts_cache(
            &row.model_id,
            Some(&selected_artifact_id),
            ModelPackageFactsCacheScope::Detail,
        )?;
        let Some(cached) = cached else {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Incomplete,
                evidence,
                vec![diagnostic(
                    IntentDiagnosticCode::PackageFactsMissing,
                    "package_facts",
                    "verified availability requires cached detail package facts",
                )],
            ));
        };
        let facts = match decode_current_facts(&cached) {
            Ok(facts) => facts,
            Err(diagnostic) => {
                return Ok(EvaluatedCandidate::without_handle(
                    row,
                    CandidateState::Incomplete,
                    evidence,
                    vec![diagnostic],
                ));
            }
        };
        let mut fact_diagnostics = Vec::new();
        evaluate_fact_constraints(requirement, &facts, &mut evidence, &mut fact_diagnostics);
        if fact_diagnostics.iter().any(is_mismatch) {
            if let Some(source_diagnostic) = self.cached_source_diagnostic(&cached, &row).await? {
                return Ok(EvaluatedCandidate::without_handle(
                    row,
                    CandidateState::Incomplete,
                    evidence,
                    vec![source_diagnostic],
                ));
            }
        }
        if fact_diagnostics.iter().any(is_mismatch) {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Unsatisfied,
                evidence,
                fact_diagnostics,
            ));
        }
        if !fact_diagnostics.is_empty() {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Incomplete,
                evidence,
                fact_diagnostics,
            ));
        }

        if row
            .package_facts_summary
            .as_ref()
            .is_some_and(|summary| summary.artifact_kind == PackageArtifactKind::Unknown)
        {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Incomplete,
                evidence,
                vec![diagnostic(
                    IntentDiagnosticCode::FormatEvidenceMissing,
                    "artifact.format",
                    "indexed summary lacks format evidence needed to verify the load target",
                )],
            ));
        }

        let response = self
            .library
            .resolve_model_artifact_load_target(ResolveModelArtifactLoadTargetRequest {
                model_ref: row.model_ref.clone(),
                expected_artifact_kind: requirement.artifact.format,
                caller_observed_entry_path: row.entry_path.clone(),
                caller_observed_package_facts_contract_version: Some(
                    PACKAGE_FACTS_CONTRACT_VERSION,
                ),
                resolution_mode: PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
                consumer: PumasArtifactConsumer {
                    consumer_name: "pumas-intent".to_string(),
                    task_kind: None,
                    runtime_family: None,
                },
            })
            .await?;
        let Some(target) = response.target else {
            let diagnostics = response
                .diagnostics
                .into_iter()
                .map(|item| diagnostic_from_load_target(item.code, item.field_path, item.message))
                .collect();
            return Ok(EvaluatedCandidate::without_handle(
                row,
                if response.artifact_state == ModelArtifactState::Invalid
                    || response.entry_path_state == ModelEntryPathState::Invalid
                {
                    CandidateState::Unsatisfied
                } else {
                    CandidateState::Incomplete
                },
                evidence,
                diagnostics,
            ));
        };

        if !target_coheres_with_facts(&target, &facts, &selected_artifact_id) {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Incomplete,
                evidence,
                vec![diagnostic(
                    IntentDiagnosticCode::PackageFactsStale,
                    "package_facts",
                    "cached package facts and resolved load target disagree",
                )],
            ));
        }
        if let Some(diagnostic) =
            current_target_path_diagnostic(&target.local_load_path, target.load_path_kind).await?
        {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Incomplete,
                evidence,
                vec![diagnostic],
            ));
        }
        if let Some(source_diagnostic) = self.cached_source_diagnostic(&cached, &row).await? {
            return Ok(EvaluatedCandidate::without_handle(
                row,
                CandidateState::Incomplete,
                evidence,
                vec![source_diagnostic],
            ));
        }

        let identity = ArtifactIdentity {
            model_ref: identity_model_ref(&row.model_ref),
        };
        let handle = ModelHandle {
            identity: identity.clone(),
            artifact_kind: target.artifact_kind,
            local_load_path: target.local_load_path,
            load_path_kind: target.load_path_kind,
            storage_kind: target.storage_kind,
            verification: ArtifactVerificationEvidence {
                validation_state: target.validation_state,
                package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION,
                source_fingerprint: cached.source_fingerprint,
                observed_from_cache_at: cached.updated_at,
            },
        };
        Ok(EvaluatedCandidate {
            candidate: ModelCandidate {
                identity,
                state: CandidateState::Ready,
                match_evidence: evidence,
                diagnostics: Vec::new(),
            },
            handle: Some(handle),
        })
    }
}

struct EvaluatedCandidate {
    candidate: ModelCandidate,
    handle: Option<ModelHandle>,
}

enum Observation<T> {
    Current(T),
    Changed,
}

impl EvaluatedCandidate {
    fn without_handle(
        row: ModelLibrarySelectorSnapshotRow,
        state: CandidateState,
        match_evidence: Vec<MatchEvidence>,
        diagnostics: Vec<IntentDiagnostic>,
    ) -> Self {
        Self {
            candidate: ModelCandidate {
                identity: ArtifactIdentity {
                    model_ref: identity_model_ref(&row.model_ref),
                },
                state,
                match_evidence,
                diagnostics,
            },
            handle: None,
        }
    }
}

fn identity_match(
    requirement: &ModelRequirement,
    row: &ModelLibrarySelectorSnapshotRow,
) -> Option<Vec<MatchEvidence>> {
    match &requirement.selector {
        ModelSelector::LocalModel { model_ref } if model_ref.model_id == row.model_id => {
            Some(vec![MatchEvidence {
                kind: MatchEvidenceKind::LocalModelId,
                value: row.model_id.clone(),
            }])
        }
        ModelSelector::UpstreamRepository { repository_id, .. }
            if row.repo_id.as_deref() == Some(repository_id.as_str()) =>
        {
            Some(vec![MatchEvidence {
                kind: MatchEvidenceKind::UpstreamRepositoryId,
                value: repository_id.clone(),
            }])
        }
        _ => None,
    }
}

fn evaluate_identity_constraints(
    requirement: &ModelRequirement,
    row: &ModelLibrarySelectorSnapshotRow,
    evidence: &mut Vec<MatchEvidence>,
    diagnostics: &mut Vec<IntentDiagnostic>,
) {
    if let ModelSelector::LocalModel { model_ref } = &requirement.selector {
        if let Some(required_path) = non_empty(model_ref.selected_artifact_path.as_deref()) {
            match non_empty(row.model_ref.selected_artifact_path.as_deref())
                .or_else(|| non_empty(row.entry_path.as_deref()))
            {
                Some(actual) if actual == required_path => {}
                Some(_) => diagnostics.push(diagnostic(
                    IntentDiagnosticCode::ArtifactMismatch,
                    "selector.model_ref.selected_artifact_path",
                    "indexed selected artifact path does not match the supplied model reference",
                )),
                None => diagnostics.push(diagnostic(
                    IntentDiagnosticCode::ArtifactEvidenceMissing,
                    "selector.model_ref.selected_artifact_path",
                    "indexed model has no selected artifact path evidence",
                )),
            }
        }
    }

    if let Some(required) = requested_revision(requirement) {
        match non_empty(row.model_ref.revision.as_deref()) {
            Some(actual) if actual == required => evidence.push(MatchEvidence {
                kind: MatchEvidenceKind::Revision,
                value: actual.to_string(),
            }),
            Some(_) => diagnostics.push(diagnostic(
                IntentDiagnosticCode::RevisionMismatch,
                "selector.revision",
                "indexed model revision does not satisfy the requested revision",
            )),
            None => diagnostics.push(diagnostic(
                IntentDiagnosticCode::RevisionEvidenceMissing,
                "selector.revision",
                "indexed model has no revision evidence",
            )),
        }
    }

    if let Some(required) = requested_artifact_id(requirement) {
        match non_empty(row.selected_artifact_id.as_deref())
            .or_else(|| non_empty(row.model_ref.selected_artifact_id.as_deref()))
        {
            Some(actual) if actual == required => evidence.push(MatchEvidence {
                kind: MatchEvidenceKind::SelectedArtifactId,
                value: actual.to_string(),
            }),
            Some(_) => diagnostics.push(diagnostic(
                IntentDiagnosticCode::ArtifactMismatch,
                "artifact.selected_artifact_id",
                "indexed selected artifact does not satisfy the requested artifact identity",
            )),
            None => diagnostics.push(diagnostic(
                IntentDiagnosticCode::ArtifactEvidenceMissing,
                "artifact.selected_artifact_id",
                "indexed model has no selected artifact identity evidence",
            )),
        }
    }

    if let Some(required) = requirement.artifact.format {
        if let Some(summary) = row.package_facts_summary.as_ref() {
            if summary.artifact_kind == required {
                evidence.push(MatchEvidence {
                    kind: MatchEvidenceKind::ArtifactFormat,
                    value: artifact_kind_name(required),
                });
            } else if summary.artifact_kind != PackageArtifactKind::Unknown {
                diagnostics.push(diagnostic(
                    IntentDiagnosticCode::FormatMismatch,
                    "artifact.format",
                    "indexed artifact format does not satisfy the requirement",
                ));
            }
        }
    }
}

fn evaluate_fact_constraints(
    requirement: &ModelRequirement,
    facts: &ResolvedModelPackageFacts,
    evidence: &mut Vec<MatchEvidence>,
    diagnostics: &mut Vec<IntentDiagnostic>,
) {
    if facts.artifact.artifact_kind == PackageArtifactKind::Gguf {
        match facts.gguf.as_ref().map(|gguf| gguf.status) {
            Some(crate::models::PackageFactStatus::Present) => {}
            Some(crate::models::PackageFactStatus::Invalid) => diagnostics.push(diagnostic(
                IntentDiagnosticCode::PackageFactsInvalid,
                "package_facts.gguf",
                "cached GGUF inspection reports an invalid artifact",
            )),
            _ => diagnostics.push(diagnostic(
                IntentDiagnosticCode::PackageFactsMissing,
                "package_facts.gguf",
                "cached package facts lack a successful GGUF inspection",
            )),
        }
    }

    if let Some(required) = requirement.artifact.format {
        if facts.artifact.artifact_kind == required {
            if !evidence
                .iter()
                .any(|item| item.kind == MatchEvidenceKind::ArtifactFormat)
            {
                evidence.push(MatchEvidence {
                    kind: MatchEvidenceKind::ArtifactFormat,
                    value: artifact_kind_name(required),
                });
            }
        } else if facts.artifact.artifact_kind == PackageArtifactKind::Unknown {
            diagnostics.push(diagnostic(
                IntentDiagnosticCode::FormatEvidenceMissing,
                "artifact.format",
                "cached package facts have no artifact format evidence",
            ));
        } else {
            diagnostics.push(diagnostic(
                IntentDiagnosticCode::FormatMismatch,
                "artifact.format",
                "cached package facts do not satisfy the requested artifact format",
            ));
        }
    }

    if let Some(required) = non_empty(requirement.artifact.quantization.as_deref()) {
        match facts.gguf.as_ref().and_then(|gguf| {
            (gguf.status == crate::models::PackageFactStatus::Present
                && gguf.value_source == Some(crate::models::PackageFactValueSource::Header))
            .then(|| non_empty(gguf.quantization.as_deref()))
            .flatten()
        }) {
            Some(actual)
                if normalized_quantization(actual) == normalized_quantization(required) =>
            {
                evidence.push(MatchEvidence {
                    kind: MatchEvidenceKind::Quantization,
                    value: actual.to_string(),
                })
            }
            Some(_) => diagnostics.push(diagnostic(
                IntentDiagnosticCode::QuantizationMismatch,
                "artifact.quantization",
                "cached quantization evidence does not satisfy the requirement",
            )),
            None => diagnostics.push(diagnostic(
                IntentDiagnosticCode::QuantizationEvidenceMissing,
                "artifact.quantization",
                "cached package facts have no quantization evidence",
            )),
        }
    }
}

pub(crate) fn normalized_quantization(value: &str) -> String {
    let upper = value.to_ascii_uppercase();
    upper.strip_prefix("MOSTLY_").unwrap_or(&upper).to_string()
}

fn decode_current_facts(
    cached: &ModelPackageFactsCacheRecord,
) -> std::result::Result<ResolvedModelPackageFacts, IntentDiagnostic> {
    if cached.package_facts_contract_version != i64::from(PACKAGE_FACTS_CONTRACT_VERSION) {
        return Err(diagnostic(
            IntentDiagnosticCode::PackageFactsStale,
            "package_facts.contract_version",
            "cached package facts use a stale contract version",
        ));
    }
    serde_json::from_str(&cached.facts_json).map_err(|_| {
        diagnostic(
            IntentDiagnosticCode::PackageFactsInvalid,
            "package_facts",
            "cached detail package facts cannot be decoded",
        )
    })
}

fn stale_filesystem_diagnostic() -> IntentDiagnostic {
    diagnostic(
        IntentDiagnosticCode::PackageFactsStale,
        "package_facts.inspection_manifest",
        "current local package source or observed identity does not match cached package facts",
    )
}

fn target_coheres_with_facts(
    target: &crate::models::PumasArtifactLoadTarget,
    facts: &ResolvedModelPackageFacts,
    selected_artifact_id: &str,
) -> bool {
    target.artifact_kind == facts.artifact.artifact_kind
        && target.local_load_path == facts.artifact.entry_path
        && target.storage_kind == facts.artifact.storage_kind
        && target.validation_state == facts.artifact.validation_state
        && non_empty(target.model_ref.selected_artifact_id.as_deref()) == Some(selected_artifact_id)
        && non_empty(facts.model_ref.selected_artifact_id.as_deref()) == Some(selected_artifact_id)
        && target.model_ref.model_id == facts.model_ref.model_id
}

async fn current_target_path_diagnostic(
    path: &str,
    expected_kind: PumasArtifactLoadPathKind,
) -> Result<Option<IntentDiagnostic>> {
    let path = std::path::Path::new(path);
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(diagnostic(
                IntentDiagnosticCode::ArtifactMissing,
                "handle.local_load_path",
                "selected artifact is no longer present on the filesystem",
            )));
        }
        Err(error) => return Err(crate::PumasError::io_with_path(error, path)),
    };
    let expected_shape = match expected_kind {
        PumasArtifactLoadPathKind::Directory => metadata.is_dir(),
        PumasArtifactLoadPathKind::File => metadata.is_file(),
    };
    Ok((!expected_shape).then(|| {
        diagnostic(
            IntentDiagnosticCode::ArtifactPathInvalid,
            "handle.local_load_path",
            "selected artifact path no longer has the indexed filesystem kind",
        )
    }))
}

fn diagnostic_from_load_target(
    code: PumasArtifactLoadTargetDiagnosticCode,
    field_path: Option<String>,
    message: String,
) -> IntentDiagnostic {
    let intent_code = match code {
        PumasArtifactLoadTargetDiagnosticCode::MissingModel
        | PumasArtifactLoadTargetDiagnosticCode::ArtifactMissing
        | PumasArtifactLoadTargetDiagnosticCode::ArtifactPathMissing => {
            IntentDiagnosticCode::ArtifactMissing
        }
        PumasArtifactLoadTargetDiagnosticCode::InvalidArtifact
        | PumasArtifactLoadTargetDiagnosticCode::InvalidPackageFacts
        | PumasArtifactLoadTargetDiagnosticCode::ArtifactPathNotLoadable => {
            IntentDiagnosticCode::PackageFactsInvalid
        }
        PumasArtifactLoadTargetDiagnosticCode::ArtifactKindMismatch => {
            IntentDiagnosticCode::FormatMismatch
        }
        PumasArtifactLoadTargetDiagnosticCode::StalePackageFacts
        | PumasArtifactLoadTargetDiagnosticCode::SelectedArtifactMismatch => {
            IntentDiagnosticCode::PackageFactsStale
        }
        _ => IntentDiagnosticCode::PackageFactsMissing,
    };
    IntentDiagnostic {
        code: intent_code,
        field_path,
        message,
    }
}

fn requested_revision(requirement: &ModelRequirement) -> Option<&str> {
    match &requirement.selector {
        ModelSelector::LocalModel { model_ref } => non_empty(model_ref.revision.as_deref()),
        ModelSelector::UpstreamRepository { revision, .. } => non_empty(revision.as_deref()),
    }
}

fn requested_artifact_id(requirement: &ModelRequirement) -> Option<&str> {
    non_empty(requirement.artifact.selected_artifact_id.as_deref()).or_else(|| {
        match &requirement.selector {
            ModelSelector::LocalModel { model_ref } => {
                non_empty(model_ref.selected_artifact_id.as_deref())
            }
            ModelSelector::UpstreamRepository { .. } => None,
        }
    })
}

fn selected_artifact_id(
    requirement: &ModelRequirement,
    row: &ModelLibrarySelectorSnapshotRow,
) -> Option<String> {
    requested_artifact_id(requirement)
        .map(ToOwned::to_owned)
        .or_else(|| row.selected_artifact_id.clone())
        .or_else(|| row.model_ref.selected_artifact_id.clone())
}

fn is_mismatch(diagnostic: &IntentDiagnostic) -> bool {
    matches!(
        diagnostic.code,
        IntentDiagnosticCode::RevisionMismatch
            | IntentDiagnosticCode::ArtifactMismatch
            | IntentDiagnosticCode::FormatMismatch
            | IntentDiagnosticCode::QuantizationMismatch
    )
}

fn is_incomplete(diagnostic: &IntentDiagnostic) -> bool {
    !is_mismatch(diagnostic)
}

fn collect_diagnostics(
    candidates: &[ModelCandidate],
    state: CandidateState,
) -> Vec<IntentDiagnostic> {
    candidates
        .iter()
        .filter(|candidate| candidate.state == state)
        .flat_map(|candidate| candidate.diagnostics.clone())
        .collect()
}

fn artifact_kind_name(kind: PackageArtifactKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn changed_during_observation_diagnostic() -> IntentDiagnostic {
    diagnostic(
        IntentDiagnosticCode::PackageFactsStale,
        "model_library.cursor",
        "model-library state changed while the intent snapshot was being read",
    )
}

fn identity_model_ref(model_ref: &crate::models::PumasModelRef) -> crate::models::PumasModelRef {
    let mut identity = model_ref.clone();
    identity.selected_artifact_path = None;
    identity
}
