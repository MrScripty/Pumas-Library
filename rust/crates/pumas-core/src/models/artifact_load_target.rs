//! Artifact load-target DTOs for exact selected-artifact execution.
//!
//! These contracts let consumers ask Pumas to resolve a selected artifact into
//! an approved local load target without learning Pumas storage layout.

use serde::{Deserialize, Serialize};

use super::{
    AssetValidationState, ModelArtifactState, ModelEntryPathState, PackageArtifactKind,
    PumasModelRef, StorageKind,
};

/// Request mode for artifact load-target resolution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PumasArtifactLoadTargetResolutionMode {
    /// Owner instance may perform freshness work before resolving.
    OwnerFresh,
    /// Resolve only from indexed/cache state without mutation or background work.
    ReadOnlyIndexed,
}

/// Runtime consumer metadata for audit and diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PumasArtifactConsumer {
    pub consumer_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_family: Option<String>,
}

/// Request to resolve one selected Pumas artifact into a load target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ResolveModelArtifactLoadTargetRequest {
    pub model_ref: PumasModelRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_artifact_kind: Option<PackageArtifactKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_observed_entry_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_observed_package_facts_contract_version: Option<u32>,
    pub resolution_mode: PumasArtifactLoadTargetResolutionMode,
    pub consumer: PumasArtifactConsumer,
}

/// Filesystem shape that should be handed to the runtime worker.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PumasArtifactLoadPathKind {
    Directory,
    File,
}

/// Pumas-approved local target for one selected artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PumasArtifactLoadTarget {
    pub model_ref: PumasModelRef,
    pub artifact_kind: PackageArtifactKind,
    pub local_load_path: String,
    pub load_path_kind: PumasArtifactLoadPathKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_root_id: Option<String>,
    pub storage_kind: StorageKind,
    pub validation_state: AssetValidationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_facts_contract_version: Option<u32>,
}

/// Stable diagnostic code for non-ready load-target responses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PumasArtifactLoadTargetDiagnosticCode {
    MissingModel,
    MissingSelectedArtifact,
    SelectedArtifactMismatch,
    ArtifactMissing,
    ArtifactPartial,
    ArtifactNeedsDetail,
    ArtifactPathMissing,
    ArtifactPathNotLoadable,
    ArtifactKindMismatch,
    InvalidArtifact,
    InvalidPackageFacts,
    StalePackageFacts,
    LibraryUnavailable,
    ModeNotAllowed,
}

/// Human-readable diagnostic attached to a typed load-target response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PumasArtifactLoadTargetDiagnostic {
    pub code: PumasArtifactLoadTargetDiagnosticCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
    pub message: String,
}

/// Result of resolving a selected Pumas artifact into a runtime load target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ResolveModelArtifactLoadTargetResponse {
    pub artifact_state: ModelArtifactState,
    pub entry_path_state: ModelEntryPathState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<PumasArtifactLoadTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<PumasArtifactLoadTargetDiagnostic>,
}

impl ResolveModelArtifactLoadTargetResponse {
    /// Returns true only when the response contains an execution-ready target.
    pub fn is_ready(&self) -> bool {
        self.artifact_state == ModelArtifactState::Ready
            && self.entry_path_state == ModelEntryPathState::Ready
            && self.target.is_some()
    }
}
