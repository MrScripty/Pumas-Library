//! Fast selector snapshot DTOs for model-library consumers.
//!
//! These contracts are intentionally list-oriented. They expose stable identity
//! and readiness state without requiring per-model detail hydration, package
//! fact regeneration, filesystem scans, or runtime selection.

use serde::{Deserialize, Serialize};

use super::{
    AssetValidationState, ModelPackageFactsSummaryStatus, PumasModelRef,
    ResolvedModelPackageFactsSummary, StorageKind,
};

/// Current wire contract version for selector snapshots.
pub const MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION: u32 = 1;

fn default_selector_snapshot_contract_version() -> u32 {
    MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION
}

/// Consumer-visible state of a selector row's executable entry path.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelEntryPathState {
    Ready,
    Missing,
    Partial,
    Invalid,
    Ambiguous,
    NeedsDetail,
    Stale,
}

/// Consumer-visible state of the selected model artifact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactState {
    Ready,
    Missing,
    Partial,
    Invalid,
    Ambiguous,
    NeedsDetail,
    Stale,
}

/// Detail freshness represented by a fast selector row.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelLibrarySelectorDetailState {
    Complete,
    SummaryOnly,
    NeedsPackageFacts,
    NeedsValidation,
    Stale,
    Error,
}

/// Bounded selector snapshot request.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibrarySelectorSnapshotRequest {
    #[serde(default)]
    pub offset: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub model_type: Option<String>,
    #[serde(default)]
    pub task_type_primary: Option<String>,
}

/// Single row in the fast model-library selector snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibrarySelectorSnapshotRow {
    pub model_id: String,
    pub model_ref: PumasModelRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_path: Option<String>,
    pub entry_path_state: ModelEntryPathState,
    pub artifact_state: ModelArtifactState,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type_primary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_backend: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_engine_hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_kind: Option<StorageKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_state: Option<AssetValidationState>,
    pub package_facts_summary_status: ModelPackageFactsSummaryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_facts_summary: Option<ResolvedModelPackageFactsSummary>,
    pub detail_state: ModelLibrarySelectorDetailState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl ModelLibrarySelectorSnapshotRow {
    /// Returns the executable entry path only when both entry and artifact state are ready.
    pub fn executable_entry_path(&self) -> Option<&str> {
        if self.entry_path_state == ModelEntryPathState::Ready
            && self.artifact_state == ModelArtifactState::Ready
        {
            self.entry_path.as_deref()
        } else {
            None
        }
    }

    pub fn is_executable_reference_ready(&self) -> bool {
        self.executable_entry_path().is_some()
    }
}

/// Cursored model-library selector snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibrarySelectorSnapshot {
    #[serde(default = "default_selector_snapshot_contract_version")]
    pub selector_snapshot_contract_version: u32,
    pub cursor: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<ModelLibrarySelectorSnapshotRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
}

impl ModelLibrarySelectorSnapshot {
    pub fn empty(cursor: impl Into<String>) -> Self {
        Self {
            selector_snapshot_contract_version: MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION,
            cursor: cursor.into(),
            rows: Vec::new(),
            total_count: Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PUMAS_MODEL_REF_CONTRACT_VERSION;

    fn selector_row(
        entry_path_state: ModelEntryPathState,
        artifact_state: ModelArtifactState,
    ) -> ModelLibrarySelectorSnapshotRow {
        ModelLibrarySelectorSnapshotRow {
            model_id: "llm/example/model-q4".to_string(),
            model_ref: PumasModelRef {
                model_id: "llm/example/model-q4".to_string(),
                selected_artifact_id: Some("model-q4.gguf".to_string()),
                selected_artifact_path: Some("llm/example/model-q4/model-q4.gguf".to_string()),
                ..PumasModelRef::default()
            },
            repo_id: Some("example/model".to_string()),
            selected_artifact_id: Some("model-q4.gguf".to_string()),
            selected_artifact_path: Some("llm/example/model-q4/model-q4.gguf".to_string()),
            entry_path: Some("/tmp/pumas/models/llm/example/model-q4/model-q4.gguf".to_string()),
            entry_path_state,
            artifact_state,
            display_name: "Example Model Q4".to_string(),
            model_type: Some("llm".to_string()),
            tags: vec!["gguf".to_string()],
            indexed_path: Some("llm/example/model-q4".to_string()),
            task_type_primary: Some("text-generation".to_string()),
            pipeline_tag: Some("text-generation".to_string()),
            recommended_backend: Some("llama.cpp".to_string()),
            runtime_engine_hints: vec!["llama.cpp".to_string()],
            storage_kind: Some(StorageKind::LibraryOwned),
            validation_state: Some(AssetValidationState::Valid),
            package_facts_summary_status: ModelPackageFactsSummaryStatus::Cached,
            package_facts_summary: None,
            detail_state: ModelLibrarySelectorDetailState::SummaryOnly,
            updated_at: Some("2026-05-06T00:00:00Z".to_string()),
        }
    }

    #[test]
    fn entry_path_is_executable_only_when_entry_and_artifact_are_ready() {
        let ready = selector_row(ModelEntryPathState::Ready, ModelArtifactState::Ready);
        assert!(ready.is_executable_reference_ready());
        assert_eq!(
            ready.executable_entry_path(),
            Some("/tmp/pumas/models/llm/example/model-q4/model-q4.gguf")
        );

        let cases = [
            (ModelEntryPathState::Missing, ModelArtifactState::Ready),
            (ModelEntryPathState::Partial, ModelArtifactState::Ready),
            (ModelEntryPathState::Invalid, ModelArtifactState::Ready),
            (ModelEntryPathState::Ambiguous, ModelArtifactState::Ready),
            (ModelEntryPathState::NeedsDetail, ModelArtifactState::Ready),
            (ModelEntryPathState::Stale, ModelArtifactState::Ready),
            (ModelEntryPathState::Ready, ModelArtifactState::Missing),
            (ModelEntryPathState::Ready, ModelArtifactState::Partial),
            (ModelEntryPathState::Ready, ModelArtifactState::Invalid),
            (ModelEntryPathState::Ready, ModelArtifactState::Ambiguous),
            (ModelEntryPathState::Ready, ModelArtifactState::NeedsDetail),
            (ModelEntryPathState::Ready, ModelArtifactState::Stale),
        ];

        for (entry_state, artifact_state) in cases {
            let row = selector_row(entry_state, artifact_state);
            assert!(
                !row.is_executable_reference_ready(),
                "entry={entry_state:?} artifact={artifact_state:?}"
            );
            assert_eq!(row.executable_entry_path(), None);
        }
    }

    #[test]
    fn selector_snapshot_uses_snake_case_wire_shape() {
        let snapshot = ModelLibrarySelectorSnapshot {
            selector_snapshot_contract_version: MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION,
            cursor: "model-library-updates:42".to_string(),
            rows: vec![selector_row(
                ModelEntryPathState::Ready,
                ModelArtifactState::Ready,
            )],
            total_count: Some(1),
        };

        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["selector_snapshot_contract_version"], 1);
        assert_eq!(value["rows"][0]["entry_path_state"], "ready");
        assert_eq!(value["rows"][0]["artifact_state"], "ready");
        assert_eq!(value["rows"][0]["detail_state"], "summary_only");
        assert_eq!(
            value["rows"][0]["model_ref"]["model_ref_contract_version"],
            1
        );

        let parsed: ModelLibrarySelectorSnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, snapshot);
    }

    #[test]
    fn legacy_model_refs_default_to_current_contract_version() {
        let parsed: PumasModelRef = serde_json::from_value(serde_json::json!({
            "model_id": "llm/example/model"
        }))
        .unwrap();

        assert_eq!(
            parsed.model_ref_contract_version,
            PUMAS_MODEL_REF_CONTRACT_VERSION
        );
        assert_eq!(PumasModelRef::default().model_ref_contract_version, 1);
    }
}
