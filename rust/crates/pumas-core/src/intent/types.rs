use crate::models::{
    AssetValidationState, PackageArtifactKind, PumasArtifactLoadPathKind, PumasModelRef,
    StorageKind, PUMAS_MODEL_REF_CONTRACT_VERSION,
};
use serde::{Deserialize, Serialize};

/// Selects either one indexed Pumas model or an upstream repository identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
#[non_exhaustive]
pub enum ModelSelector {
    LocalModel {
        model_ref: PumasModelRef,
    },
    UpstreamRepository {
        repository_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<String>,
    },
}

/// Constraints on the artifact selected from a model package.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ArtifactRequirement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<PackageArtifactKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_id: Option<String>,
}

/// Whether resolving a requirement may contact or acquire from upstream.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AcquisitionPolicy {
    LocalOnly,
    AllowUpstream,
}

/// A transport-independent description of the local model artifact a caller needs.
///
/// Public fields support serde and language adapters. Every intent operation performs
/// semantic validation; successful deserialization alone does not make a requirement valid.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelRequirement {
    pub selector: ModelSelector,
    #[serde(default)]
    pub artifact: ArtifactRequirement,
    pub acquisition_policy: AcquisitionPolicy,
}

/// Stable identity for one selected local artifact. Paths are deliberately excluded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ArtifactIdentity {
    pub model_ref: PumasModelRef,
}

/// Why a local indexed row was or was not accepted for a requirement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IntentDiagnosticCode {
    InvalidConsumerKey,
    InvalidModelReference,
    InvalidRepositoryId,
    InvalidRevision,
    InvalidArtifactConstraint,
    ContradictoryArtifactIdentity,
    UnsupportedArtifactFormat,
    UpstreamAcquisitionUnavailable,
    UpstreamResolutionFailed,
    UpstreamArtifactAmbiguous,
    UnsupportedUpstreamLayout,
    IntegrityEvidenceMissing,
    AcquisitionBlocked,
    AcquisitionFailed,
    DownloadCustodyUnavailable,
    RevisionMismatch,
    ArtifactMismatch,
    FormatMismatch,
    QuantizationMismatch,
    RevisionEvidenceMissing,
    ArtifactEvidenceMissing,
    FormatEvidenceMissing,
    QuantizationEvidenceMissing,
    PackageFactsMissing,
    PackageFactsInvalid,
    PackageFactsStale,
    ArtifactMissing,
    ArtifactPathInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct IntentDiagnostic {
    pub code: IntentDiagnosticCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
    pub message: String,
}

/// Positive evidence supporting a candidate match.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MatchEvidenceKind {
    LocalModelId,
    UpstreamRepositoryId,
    Revision,
    SelectedArtifactId,
    ArtifactFormat,
    Quantization,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MatchEvidence {
    pub kind: MatchEvidenceKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CandidateState {
    Ready,
    Incomplete,
    Unsatisfied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelCandidate {
    pub identity: ArtifactIdentity,
    pub state: CandidateState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub match_evidence: Vec<MatchEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<IntentDiagnostic>,
}

/// Current, bounded evidence used to return an available handle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ArtifactVerificationEvidence {
    pub validation_state: AssetValidationState,
    pub package_facts_contract_version: u32,
    pub source_fingerprint: String,
    pub observed_from_cache_at: String,
}

/// A currently available local artifact. This is an observation, not a lease.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelHandle {
    pub identity: ArtifactIdentity,
    pub artifact_kind: PackageArtifactKind,
    pub local_load_path: String,
    pub load_path_kind: PumasArtifactLoadPathKind,
    pub storage_kind: StorageKind,
    pub verification: ArtifactVerificationEvidence,
}

/// Current stage of managed acquisition for a resolved immutable requirement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AcquisitionStage {
    Queued,
    Transferring,
    Retrying,
    Paused,
    Verifying,
}

/// Bounded progress evidence from the existing managed download owner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelAcquisitionProgress {
    pub stage: AcquisitionStage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloaded_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_limit: Option<u32>,
}

/// Advisory correlation with the current download implementation.
///
/// Callers must use `resolved_requirement` as the stable status identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct AcquisitionHint {
    pub download_id: String,
}

/// One immutable upstream artifact choice discovered before local acquisition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct UpstreamArtifactCandidate {
    pub repository_id: String,
    pub revision: String,
    pub selected_artifact_id: String,
    pub filenames: Vec<String>,
    pub format: PackageArtifactKind,
    /// Selector evidence supplied by the caller; readiness verifies local artifact facts later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "outcome")]
#[non_exhaustive]
pub enum QueryModelsOutcome {
    Matches { candidates: Vec<ModelCandidate> },
    InvalidRequirement { diagnostics: Vec<IntentDiagnostic> },
    Unsupported { diagnostics: Vec<IntentDiagnostic> },
    Unavailable { diagnostics: Vec<IntentDiagnostic> },
}

/// Snapshot of the requirement's current local state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state")]
#[non_exhaustive]
pub enum ObservedModelState {
    Available {
        handle: ModelHandle,
    },
    Missing {
        diagnostics: Vec<IntentDiagnostic>,
    },
    Ambiguous {
        candidates: Vec<ModelCandidate>,
    },
    UpstreamAmbiguous {
        candidates: Vec<UpstreamArtifactCandidate>,
        diagnostics: Vec<IntentDiagnostic>,
    },
    Incomplete {
        candidates: Vec<ModelCandidate>,
        diagnostics: Vec<IntentDiagnostic>,
    },
    Unsatisfied {
        candidates: Vec<ModelCandidate>,
        diagnostics: Vec<IntentDiagnostic>,
    },
    Acquiring {
        resolved_requirement: ModelRequirement,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        download_hint: Option<AcquisitionHint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        progress: Option<ModelAcquisitionProgress>,
    },
    Blocked {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolved_requirement: Option<ModelRequirement>,
        diagnostics: Vec<IntentDiagnostic>,
    },
    Failed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resolved_requirement: Option<ModelRequirement>,
        diagnostics: Vec<IntentDiagnostic>,
    },
    InvalidRequirement {
        diagnostics: Vec<IntentDiagnostic>,
    },
    Unsupported {
        diagnostics: Vec<IntentDiagnostic>,
    },
    Unavailable {
        diagnostics: Vec<IntentDiagnostic>,
    },
}

/// Result of resolving a requirement, including managed acquisition state when allowed.
pub type GetModelOutcome = ObservedModelState;

/// A durable request owned by one stable local consumer identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct EnsureModelRequest {
    pub consumer_key: String,
    pub requirement: ModelRequirement,
}

/// Stable, generation-scoped reference to one durable declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelEnsureRef {
    pub consumer_key: String,
    pub declaration_id: String,
    pub generation: String,
}

/// Public projection of one persisted desired-model declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelDeclaration {
    pub reference: ModelEnsureRef,
    pub requirement: ModelRequirement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_requirement: Option<ModelRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "outcome")]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)] // Typed outcomes keep available handles directly inspectable.
pub enum EnsureModelOutcome {
    Accepted {
        declaration: ModelDeclaration,
        state: ObservedModelState,
    },
    InvalidRequirement {
        diagnostics: Vec<IntentDiagnostic>,
    },
    Unsupported {
        diagnostics: Vec<IntentDiagnostic>,
    },
    Unavailable {
        diagnostics: Vec<IntentDiagnostic>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "outcome")]
#[non_exhaustive]
pub enum ReleaseModelOutcome {
    Released { reference: ModelEnsureRef },
    AlreadyAbsent { reference: ModelEnsureRef },
    Conflict { reference: ModelEnsureRef },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "outcome")]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)] // Status mirrors the domain observation without a public box.
pub enum GetEnsureStatusOutcome {
    Found {
        declaration: ModelDeclaration,
        state: ObservedModelState,
    },
    NotFound,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "outcome")]
#[non_exhaustive]
pub enum ListModelDeclarationsOutcome {
    Declarations { declarations: Vec<ModelDeclaration> },
}

pub(crate) fn validate_requirement(requirement: &ModelRequirement) -> Vec<IntentDiagnostic> {
    let mut diagnostics = Vec::new();
    match &requirement.selector {
        ModelSelector::LocalModel { model_ref } => {
            if model_ref.model_ref_contract_version != PUMAS_MODEL_REF_CONTRACT_VERSION
                || !model_ref.migration_diagnostics.is_empty()
                || !valid_relative_identity(&model_ref.model_id)
            {
                diagnostics.push(diagnostic(
                    IntentDiagnosticCode::InvalidModelReference,
                    "selector.model_ref",
                    "model_ref must contain a current, resolved, relative Pumas model identity",
                ));
            }
            validate_optional_token(
                model_ref.revision.as_deref(),
                "selector.model_ref.revision",
                IntentDiagnosticCode::InvalidRevision,
                &mut diagnostics,
            );
            validate_optional_artifact_id(
                model_ref.selected_artifact_id.as_deref(),
                "selector.model_ref.selected_artifact_id",
                &mut diagnostics,
            );
            validate_optional_token(
                model_ref.selected_artifact_path.as_deref(),
                "selector.model_ref.selected_artifact_path",
                IntentDiagnosticCode::InvalidArtifactConstraint,
                &mut diagnostics,
            );
            if let (Some(reference_id), Some(required_id)) = (
                non_empty(model_ref.selected_artifact_id.as_deref()),
                non_empty(requirement.artifact.selected_artifact_id.as_deref()),
            ) {
                if reference_id != required_id {
                    diagnostics.push(diagnostic(
                        IntentDiagnosticCode::ContradictoryArtifactIdentity,
                        "artifact.selected_artifact_id",
                        "artifact identity contradicts selector.model_ref.selected_artifact_id",
                    ));
                }
            }
        }
        ModelSelector::UpstreamRepository {
            repository_id,
            revision,
        } => {
            if !valid_repository_id(repository_id) {
                diagnostics.push(diagnostic(
                    IntentDiagnosticCode::InvalidRepositoryId,
                    "selector.repository_id",
                    "repository_id must be a non-empty owner/name identity",
                ));
            }
            validate_optional_token(
                revision.as_deref(),
                "selector.revision",
                IntentDiagnosticCode::InvalidRevision,
                &mut diagnostics,
            );
        }
    }

    validate_optional_artifact_id(
        requirement.artifact.selected_artifact_id.as_deref(),
        "artifact.selected_artifact_id",
        &mut diagnostics,
    );
    validate_optional_token(
        requirement.artifact.quantization.as_deref(),
        "artifact.quantization",
        IntentDiagnosticCode::InvalidArtifactConstraint,
        &mut diagnostics,
    );
    diagnostics
}

pub(crate) fn unsupported_diagnostics(requirement: &ModelRequirement) -> Vec<IntentDiagnostic> {
    let mut diagnostics = Vec::new();
    if requirement.artifact.format == Some(PackageArtifactKind::Unknown) {
        diagnostics.push(diagnostic(
            IntentDiagnosticCode::UnsupportedArtifactFormat,
            "artifact.format",
            "unknown is evidence absence, not a supported artifact format constraint",
        ));
    }
    diagnostics
}

pub(crate) fn diagnostic(
    code: IntentDiagnosticCode,
    field_path: &str,
    message: &str,
) -> IntentDiagnostic {
    IntentDiagnostic {
        code,
        field_path: Some(field_path.to_string()),
        message: message.to_string(),
    }
}

fn validate_optional_token(
    value: Option<&str>,
    field: &str,
    code: IntentDiagnosticCode,
    diagnostics: &mut Vec<IntentDiagnostic>,
) {
    if value.is_some_and(|value| value.trim().is_empty() || value.chars().any(char::is_control)) {
        diagnostics.push(diagnostic(
            code,
            field,
            "value must be non-empty and contain no control characters",
        ));
    }
}

fn validate_optional_artifact_id(
    value: Option<&str>,
    field: &str,
    diagnostics: &mut Vec<IntentDiagnostic>,
) {
    if let Some(value) = value {
        if !valid_relative_identity(value) {
            diagnostics.push(diagnostic(
                IntentDiagnosticCode::InvalidArtifactConstraint,
                field,
                "artifact id must be a non-empty relative identity without traversal",
            ));
        }
    }
}

fn valid_repository_id(value: &str) -> bool {
    let mut parts = value.split('/');
    matches!((parts.next(), parts.next(), parts.next()), (Some(owner), Some(name), None) if valid_segment(owner) && valid_segment(name))
}

fn valid_relative_identity(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\\')
        && !has_windows_drive_prefix(value)
        && value.split('/').all(valid_segment)
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
}

pub(crate) fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then_some(value)
    })
}
