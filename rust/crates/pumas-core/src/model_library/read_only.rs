use std::path::{Path, PathBuf};

use crate::index::ModelIndex;
use crate::model_library::artifact_load_target::{
    mode_not_allowed_response, resolve_artifact_load_target_from_index,
};
use crate::models::{
    ModelLibrarySelectorSnapshot, ModelLibrarySelectorSnapshotRequest,
    PumasArtifactLoadTargetResolutionMode, ResolveModelArtifactLoadTargetRequest,
    ResolveModelArtifactLoadTargetResponse,
};
use crate::Result;

const DB_FILENAME: &str = "models.db";

/// Read-only model-library view for snapshot-style consumers.
///
/// This type opens the existing SQLite model index without claiming instance
/// ownership, starting watchers, creating schema, or running reconciliation.
pub struct PumasReadOnlyLibrary {
    library_root: PathBuf,
    index: ModelIndex,
}

impl PumasReadOnlyLibrary {
    pub fn open(library_root: impl Into<PathBuf>) -> Result<Self> {
        let library_root = library_root.into();
        let index = ModelIndex::open_read_only(library_root.join(DB_FILENAME))?;
        Ok(Self {
            library_root,
            index,
        })
    }

    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    pub fn model_library_selector_snapshot(
        &self,
        request: ModelLibrarySelectorSnapshotRequest,
    ) -> Result<ModelLibrarySelectorSnapshot> {
        self.index.list_model_library_selector_snapshot(&request)
    }

    pub fn resolve_model_artifact_load_target(
        &self,
        request: ResolveModelArtifactLoadTargetRequest,
    ) -> Result<ResolveModelArtifactLoadTargetResponse> {
        if request.resolution_mode == PumasArtifactLoadTargetResolutionMode::OwnerFresh {
            return Ok(mode_not_allowed_response());
        }

        resolve_artifact_load_target_from_index(&self.index, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope, ModelRecord};
    use crate::models::{
        AssetValidationState, BackendHintFacts, ModelArtifactState, ModelEntryPathState,
        PackageArtifactKind, PackageFactStatus, PumasArtifactConsumer, PumasArtifactLoadPathKind,
        PumasArtifactLoadTargetDiagnosticCode, PumasArtifactLoadTargetResolutionMode,
        PumasModelRef, ResolvedModelPackageFactsSummary, StorageKind, TaskEvidence,
        PACKAGE_FACTS_CONTRACT_VERSION,
    };
    use std::collections::HashMap;
    use std::path::Path;
    use std::time::Instant;
    use tempfile::TempDir;

    fn create_record(id: &str) -> ModelRecord {
        ModelRecord {
            id: id.to_string(),
            path: format!("models/{id}"),
            cleaned_name: "read_only_selector".to_string(),
            official_name: "Read Only Selector".to_string(),
            model_type: "llm".to_string(),
            tags: vec!["read-only".to_string()],
            hashes: HashMap::from([("sha256".to_string(), "abc123".to_string())]),
            metadata: serde_json::json!({
                "entry_path": "/models/read-only/model.gguf",
                "validation_state": "valid",
                "task_type_primary": "text-generation"
            }),
            updated_at: "2026-05-06T00:00:00Z".to_string(),
        }
    }

    fn artifact_request(
        model_id: &str,
        selected_artifact_id: Option<&str>,
        expected_artifact_kind: PackageArtifactKind,
        resolution_mode: PumasArtifactLoadTargetResolutionMode,
    ) -> ResolveModelArtifactLoadTargetRequest {
        ResolveModelArtifactLoadTargetRequest {
            model_ref: PumasModelRef {
                model_id: model_id.to_string(),
                selected_artifact_id: selected_artifact_id.map(ToOwned::to_owned),
                selected_artifact_path: selected_artifact_id
                    .map(|artifact_id| format!("{model_id}/{artifact_id}")),
                ..PumasModelRef::default()
            },
            expected_artifact_kind: Some(expected_artifact_kind),
            caller_observed_entry_path: None,
            caller_observed_package_facts_contract_version: None,
            resolution_mode,
            consumer: PumasArtifactConsumer {
                consumer_name: "test".to_string(),
                task_kind: Some("text_generation".to_string()),
                runtime_family: Some("llama.cpp".to_string()),
            },
        }
    }

    fn cache_summary(
        root: &Path,
        model_id: &str,
        selected_artifact_id: &str,
    ) -> ModelPackageFactsCacheRecord {
        let entry_path = root
            .join("pumas library")
            .join(model_id)
            .join(selected_artifact_id)
            .to_string_lossy()
            .into_owned();
        let summary = ResolvedModelPackageFactsSummary {
            package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION,
            model_ref: PumasModelRef {
                model_id: model_id.to_string(),
                selected_artifact_id: Some(selected_artifact_id.to_string()),
                selected_artifact_path: Some(format!("{model_id}/{selected_artifact_id}")),
                ..PumasModelRef::default()
            },
            artifact_kind: PackageArtifactKind::Gguf,
            entry_path,
            storage_kind: StorageKind::LibraryOwned,
            validation_state: AssetValidationState::Valid,
            task: TaskEvidence {
                task_type_primary: Some("text_generation".to_string()),
                ..TaskEvidence::default()
            },
            backend_hints: BackendHintFacts::default(),
            requires_custom_code: false,
            config_status: PackageFactStatus::Uninspected,
            tokenizer_status: PackageFactStatus::Uninspected,
            processor_status: PackageFactStatus::Uninspected,
            generation_config_status: PackageFactStatus::Uninspected,
            generation_defaults_status: PackageFactStatus::Uninspected,
            image_generation_family_evidence: Vec::new(),
            diffusers_pipeline_class: None,
            gguf_architecture: None,
            diagnostic_codes: Vec::new(),
        };

        ModelPackageFactsCacheRecord {
            model_id: model_id.to_string(),
            selected_artifact_id: selected_artifact_id.to_string(),
            cache_scope: ModelPackageFactsCacheScope::Summary,
            package_facts_contract_version: i64::from(PACKAGE_FACTS_CONTRACT_VERSION),
            producer_revision: None,
            source_fingerprint: "fixture-fingerprint".to_string(),
            facts_json: serde_json::to_string(&summary).unwrap(),
            cached_at: "2026-05-17T00:00:00Z".to_string(),
            updated_at: "2026-05-17T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn read_only_library_projects_selector_snapshot_without_owner_lifecycle() {
        let temp = TempDir::new().unwrap();
        let writer = ModelIndex::new(temp.path().join(DB_FILENAME)).unwrap();
        writer
            .upsert(&create_record("llm/example/read-only"))
            .unwrap();
        drop(writer);

        let read_only = PumasReadOnlyLibrary::open(temp.path()).unwrap();
        let snapshot = read_only
            .model_library_selector_snapshot(ModelLibrarySelectorSnapshotRequest::default())
            .unwrap();

        assert_eq!(read_only.library_root(), temp.path());
        assert_eq!(snapshot.rows.len(), 1);
        assert_eq!(snapshot.rows[0].model_id, "llm/example/read-only");
        assert_eq!(
            snapshot.rows[0].entry_path_state,
            ModelEntryPathState::Ready
        );
        assert_eq!(snapshot.rows[0].artifact_state, ModelArtifactState::Ready);
    }

    #[test]
    fn read_only_library_requires_existing_index() {
        let temp = TempDir::new().unwrap();

        let result = PumasReadOnlyLibrary::open(temp.path());

        assert!(result.is_err());
        assert!(!temp.path().join(DB_FILENAME).exists());
    }

    #[test]
    fn selector_snapshot_reports_warm_100_row_timing() {
        let temp = TempDir::new().unwrap();
        let writer = ModelIndex::new(temp.path().join(DB_FILENAME)).unwrap();
        for index in 0..100 {
            let id = format!("llm/example/perf-{index:03}");
            writer.upsert(&create_record(&id)).unwrap();
        }

        let request = ModelLibrarySelectorSnapshotRequest {
            limit: Some(100),
            ..ModelLibrarySelectorSnapshotRequest::default()
        };

        let direct_warm = writer
            .list_model_library_selector_snapshot(&request)
            .unwrap();
        assert_eq!(direct_warm.rows.len(), 100);
        let direct_started = Instant::now();
        let direct_snapshot = writer
            .list_model_library_selector_snapshot(&request)
            .unwrap();
        let direct_elapsed = direct_started.elapsed();
        assert_eq!(direct_snapshot.rows.len(), 100);
        drop(writer);

        let read_only = PumasReadOnlyLibrary::open(temp.path()).unwrap();
        let read_only_warm = read_only
            .model_library_selector_snapshot(request.clone())
            .unwrap();
        assert_eq!(read_only_warm.rows.len(), 100);
        let read_only_started = Instant::now();
        let read_only_snapshot = read_only.model_library_selector_snapshot(request).unwrap();
        let read_only_elapsed = read_only_started.elapsed();
        assert_eq!(read_only_snapshot.rows.len(), 100);

        eprintln!(
            "selector_snapshot_100_rows direct_ms={:.3} read_only_ms={:.3}",
            direct_elapsed.as_secs_f64() * 1000.0,
            read_only_elapsed.as_secs_f64() * 1000.0
        );
        assert!(direct_elapsed.as_millis() < 250);
        assert!(read_only_elapsed.as_millis() < 250);
    }

    #[test]
    fn read_only_library_resolves_cached_artifact_load_target() {
        let temp = TempDir::new().unwrap();
        let writer = ModelIndex::new(temp.path().join(DB_FILENAME)).unwrap();
        let model_id = "llm/example/read-only-target";
        let artifact_id = "model-q4.gguf";
        writer.upsert(&create_record(model_id)).unwrap();
        writer
            .upsert_model_package_facts_cache(&cache_summary(temp.path(), model_id, artifact_id))
            .unwrap();
        drop(writer);

        let read_only = PumasReadOnlyLibrary::open(temp.path()).unwrap();
        let response = read_only
            .resolve_model_artifact_load_target(artifact_request(
                model_id,
                Some(artifact_id),
                PackageArtifactKind::Gguf,
                PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
            ))
            .unwrap();

        assert!(response.is_ready());
        let target = response.target.expect("ready target should be present");
        assert_eq!(target.artifact_kind, PackageArtifactKind::Gguf);
        assert_eq!(target.load_path_kind, PumasArtifactLoadPathKind::File);
        assert_eq!(target.storage_kind, StorageKind::LibraryOwned);
        assert_eq!(target.validation_state, AssetValidationState::Valid);
        assert!(target.local_load_path.contains("pumas library"));
        assert!(target.local_load_path.ends_with("model-q4.gguf"));
    }

    #[test]
    fn read_only_library_rejects_owner_fresh_resolution_mode() {
        let temp = TempDir::new().unwrap();
        let writer = ModelIndex::new(temp.path().join(DB_FILENAME)).unwrap();
        writer
            .upsert(&create_record("llm/example/read-only-mode"))
            .unwrap();
        drop(writer);

        let read_only = PumasReadOnlyLibrary::open(temp.path()).unwrap();
        let response = read_only
            .resolve_model_artifact_load_target(artifact_request(
                "llm/example/read-only-mode",
                Some("model-q4.gguf"),
                PackageArtifactKind::Gguf,
                PumasArtifactLoadTargetResolutionMode::OwnerFresh,
            ))
            .unwrap();

        assert!(!response.is_ready());
        assert_eq!(response.artifact_state, ModelArtifactState::Stale);
        assert_eq!(response.entry_path_state, ModelEntryPathState::Stale);
        assert_eq!(
            response.diagnostics[0].code,
            PumasArtifactLoadTargetDiagnosticCode::ModeNotAllowed
        );
    }

    #[test]
    fn read_only_library_returns_kind_mismatch_without_target() {
        let temp = TempDir::new().unwrap();
        let writer = ModelIndex::new(temp.path().join(DB_FILENAME)).unwrap();
        let model_id = "llm/example/kind-mismatch";
        let artifact_id = "model-q4.gguf";
        writer.upsert(&create_record(model_id)).unwrap();
        writer
            .upsert_model_package_facts_cache(&cache_summary(temp.path(), model_id, artifact_id))
            .unwrap();
        drop(writer);

        let read_only = PumasReadOnlyLibrary::open(temp.path()).unwrap();
        let response = read_only
            .resolve_model_artifact_load_target(artifact_request(
                model_id,
                Some(artifact_id),
                PackageArtifactKind::DiffusersBundle,
                PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
            ))
            .unwrap();

        assert!(!response.is_ready());
        assert!(response.target.is_none());
        assert_eq!(response.artifact_state, ModelArtifactState::Ready);
        assert_eq!(response.entry_path_state, ModelEntryPathState::Ready);
        assert_eq!(
            response.diagnostics[0].code,
            PumasArtifactLoadTargetDiagnosticCode::ArtifactKindMismatch
        );
    }
}
