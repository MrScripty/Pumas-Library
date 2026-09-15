#![warn(unsafe_code)]

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, CandidateState, IntentDiagnosticCode, ModelRequirement,
    ModelSelector, ObservedModelState, QueryModelsOutcome,
};
use pumas_library::models::{PackageArtifactKind, PumasModelRef};
use pumas_library::PumasApi;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct RegistryTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl RegistryTestGuard {
    #[allow(unsafe_code)]
    fn new(root: &Path) -> Self {
        let lock = REGISTRY_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // SAFETY: Tests in this integration-test process serialize access to this variable.
        unsafe { std::env::set_var("PUMAS_REGISTRY_DB_PATH", root.join("registry.db")) };
        Self { _lock: lock }
    }
}

impl Drop for RegistryTestGuard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: Guarded by the same process-wide mutex used when setting the variable.
        unsafe { std::env::remove_var("PUMAS_REGISTRY_DB_PATH") };
    }
}

fn test_root() -> TempDir {
    let root = TempDir::new().unwrap();
    for relative in [
        "launcher-data/metadata",
        "launcher-data/cache",
        "launcher-data/logs",
        "shared-resources/models",
        "ollama-versions",
    ] {
        std::fs::create_dir_all(root.path().join(relative)).unwrap();
    }
    root
}

fn create_model(root: &Path, model_id: &str, repo_id: &str, revision: Option<&str>) {
    let model_dir = root.join("shared-resources/models").join(model_id);
    let artifact = model_dir.join("model.safetensors");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(&artifact, b"initial artifact bytes").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": model_id.split('/').nth(1).unwrap(),
            "model_type": "llm",
            "pipeline_tag": "text-generation",
            "official_name": model_id,
            "cleaned_name": model_id.replace('/', "-"),
            "repo_id": repo_id,
            "upstream_revision": revision,
            "selected_artifact_id": "model.safetensors",
            "entry_path": artifact.display().to_string(),
            "storage_kind": "library_owned",
            "validation_state": "valid",
            "files": [{"name": "model.safetensors"}]
        }))
        .unwrap(),
    )
    .unwrap();
}

async fn api(root: &TempDir) -> (RegistryTestGuard, PumasApi) {
    let guard = RegistryTestGuard::new(root.path());
    let api = PumasApi::builder(root.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    (guard, api)
}

fn local_requirement(model_id: &str) -> ModelRequirement {
    ModelRequirement {
        selector: ModelSelector::LocalModel {
            model_ref: PumasModelRef {
                model_id: model_id.to_string(),
                ..PumasModelRef::default()
            },
        },
        artifact: ArtifactRequirement::default(),
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    }
}

#[tokio::test]
async fn exact_local_match_returns_verified_available_handle_without_reconciliation() {
    let root = test_root();
    let model_id = "llm/intent/exact";
    create_model(root.path(), model_id, "example/exact", Some("commit-a"));
    let (_guard, api) = api(&root).await;
    // Finish startup reconciliation before testing the read-only intent path.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match api.rebuild_model_index().await {
                Err(pumas_library::PumasError::ModelIndexRefreshInProgress) => {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                result => break result.unwrap(),
            }
        }
    })
    .await
    .unwrap();
    assert!(
        root.path()
            .join("shared-resources/models")
            .join(model_id)
            .join("model.safetensors")
            .is_file(),
        "fixture was relocated by reconciliation"
    );
    api.resolve_model_package_facts(model_id).await.unwrap();
    assert!(api.get_model(model_id).await.unwrap().is_some());
    let before = api
        .list_model_library_updates_since(None, 100)
        .await
        .unwrap()
        .cursor;

    let outcome = api
        .intent()
        .get_model(&local_requirement(model_id))
        .await
        .unwrap();
    let ObservedModelState::Available { handle } = outcome else {
        panic!("expected available outcome, got {outcome:?}");
    };
    assert_eq!(handle.identity.model_ref.model_id, model_id);
    assert_eq!(
        handle.identity.model_ref.revision.as_deref(),
        Some("commit-a")
    );
    assert!(handle.identity.model_ref.selected_artifact_path.is_none());
    assert_eq!(handle.artifact_kind, PackageArtifactKind::Safetensors);
    assert_eq!(handle.verification.source_fingerprint.len(), 64);
    assert!(Path::new(&handle.local_load_path).is_file());

    let after = api
        .list_model_library_updates_since(Some(&before), 100)
        .await
        .unwrap();
    assert!(
        after.events.is_empty(),
        "intent read unexpectedly wrote update events"
    );
}

#[tokio::test]
async fn missing_invalid_and_unsupported_requirements_remain_distinct() {
    let root = test_root();
    let (_guard, api) = api(&root).await;

    assert!(matches!(
        api.intent()
            .get_model(&local_requirement("llm/missing/model"))
            .await
            .unwrap(),
        ObservedModelState::Missing { .. }
    ));

    let invalid = local_requirement("../escape");
    let ObservedModelState::InvalidRequirement { diagnostics } =
        api.intent().get_model(&invalid).await.unwrap()
    else {
        panic!("expected invalid requirement");
    };
    assert_eq!(
        diagnostics[0].code,
        IntentDiagnosticCode::InvalidModelReference
    );
    assert!(matches!(
        api.intent()
            .get_model(&local_requirement("C:/outside/model"))
            .await
            .unwrap(),
        ObservedModelState::InvalidRequirement { .. }
    ));

    let unavailable = ModelRequirement {
        selector: ModelSelector::UpstreamRepository {
            repository_id: "example/model".to_string(),
            revision: None,
        },
        artifact: ArtifactRequirement {
            format: Some(PackageArtifactKind::Gguf),
            ..ArtifactRequirement::default()
        },
        acquisition_policy: AcquisitionPolicy::AllowUpstream,
    };
    let ObservedModelState::Unavailable { diagnostics } =
        api.intent().get_model(&unavailable).await.unwrap()
    else {
        panic!("expected unavailable acquisition");
    };
    assert_eq!(
        diagnostics[0].code,
        IntentDiagnosticCode::UpstreamAcquisitionUnavailable
    );

    let mut unknown_format = local_requirement("llm/example/model");
    unknown_format.artifact.format = Some(PackageArtifactKind::Unknown);
    let ObservedModelState::Unsupported { diagnostics } =
        api.intent().get_model(&unknown_format).await.unwrap()
    else {
        panic!("expected unsupported format");
    };
    assert_eq!(
        diagnostics[0].code,
        IntentDiagnosticCode::UnsupportedArtifactFormat
    );
}

#[tokio::test]
async fn upstream_selector_reports_ambiguity_and_query_evidence() {
    let root = test_root();
    create_model(
        root.path(),
        "llm/intent/a",
        "example/shared",
        Some("commit-a"),
    );
    create_model(
        root.path(),
        "llm/intent/b",
        "example/shared",
        Some("commit-a"),
    );
    let (_guard, api) = api(&root).await;
    api.resolve_model_package_facts("llm/intent/a")
        .await
        .unwrap();
    api.resolve_model_package_facts("llm/intent/b")
        .await
        .unwrap();
    let requirement = ModelRequirement {
        selector: ModelSelector::UpstreamRepository {
            repository_id: "example/shared".to_string(),
            revision: Some("commit-a".to_string()),
        },
        artifact: ArtifactRequirement {
            format: Some(PackageArtifactKind::Safetensors),
            ..ArtifactRequirement::default()
        },
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    };

    let QueryModelsOutcome::Matches { candidates } =
        api.intent().query_models(&requirement).await.unwrap()
    else {
        panic!("expected query matches");
    };
    assert_eq!(candidates.len(), 2);
    assert!(candidates
        .iter()
        .all(|candidate| candidate.state == CandidateState::Ready));
    assert!(matches!(
        api.intent().get_model_status(&requirement).await.unwrap(),
        ObservedModelState::Ambiguous { candidates } if candidates.len() == 2
    ));
}

#[tokio::test]
async fn revision_format_and_unknown_revision_have_typed_outcomes() {
    let root = test_root();
    create_model(
        root.path(),
        "llm/intent/pinned",
        "example/pinned",
        Some("commit-a"),
    );
    create_model(root.path(), "llm/intent/unpinned", "example/unpinned", None);
    let (_guard, api) = api(&root).await;
    for model_id in ["llm/intent/pinned", "llm/intent/unpinned"] {
        api.resolve_model_package_facts(model_id).await.unwrap();
    }

    let mut revision_mismatch = local_requirement("llm/intent/pinned");
    let ModelSelector::LocalModel { model_ref } = &mut revision_mismatch.selector else {
        unreachable!()
    };
    model_ref.revision = Some("commit-b".to_string());
    assert!(matches!(
        api.intent().get_model(&revision_mismatch).await.unwrap(),
        ObservedModelState::Unsatisfied { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::RevisionMismatch)
    ));

    let mut format_mismatch = local_requirement("llm/intent/pinned");
    format_mismatch.artifact.format = Some(PackageArtifactKind::Gguf);
    assert!(matches!(
        api.intent().get_model(&format_mismatch).await.unwrap(),
        ObservedModelState::Unsatisfied { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::FormatMismatch)
    ));

    let mut unknown_revision = local_requirement("llm/intent/unpinned");
    let ModelSelector::LocalModel { model_ref } = &mut unknown_revision.selector else {
        unreachable!()
    };
    model_ref.revision = Some("commit-a".to_string());
    assert!(matches!(
        api.intent().get_model(&unknown_revision).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::RevisionEvidenceMissing)
    ));

    let mut path_mismatch = local_requirement("llm/intent/pinned");
    let ModelSelector::LocalModel { model_ref } = &mut path_mismatch.selector else {
        unreachable!()
    };
    model_ref.selected_artifact_path = Some("/different/artifact.safetensors".to_string());
    assert!(matches!(
        api.intent().get_model(&path_mismatch).await.unwrap(),
        ObservedModelState::Unsatisfied { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::ArtifactMismatch)
    ));

    let mut no_quantization_evidence = local_requirement("llm/intent/pinned");
    no_quantization_evidence.artifact.quantization = Some("q4_k_m".to_string());
    assert!(matches!(
        api.intent().get_model(&no_quantization_evidence).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::QuantizationEvidenceMissing)
    ));
}

#[tokio::test]
async fn missing_facts_and_filesystem_change_never_report_available() {
    let root = test_root();
    create_model(
        root.path(),
        "llm/intent/incomplete",
        "example/incomplete",
        None,
    );
    create_model(root.path(), "llm/intent/changed", "example/changed", None);
    create_model(root.path(), "llm/intent/deleted", "example/deleted", None);
    let (_guard, api) = api(&root).await;

    assert!(matches!(
        api.intent().get_model(&local_requirement("llm/intent/incomplete")).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::PackageFactsMissing)
    ));

    api.resolve_model_package_facts("llm/intent/changed")
        .await
        .unwrap();
    std::fs::write(
        root.path()
            .join("shared-resources/models/llm/intent/changed/model.safetensors"),
        b"replacement bytes after indexing",
    )
    .unwrap();
    assert!(matches!(
        api.intent().get_model_status(&local_requirement("llm/intent/changed")).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::PackageFactsStale)
    ));

    api.resolve_model_package_facts("llm/intent/deleted")
        .await
        .unwrap();
    std::fs::remove_file(
        root.path()
            .join("shared-resources/models/llm/intent/deleted/model.safetensors"),
    )
    .unwrap();
    assert!(!matches!(
        api.intent()
            .get_model_status(&local_requirement("llm/intent/deleted"))
            .await
            .unwrap(),
        ObservedModelState::Available { .. }
    ));
}

#[tokio::test]
async fn malformed_cached_identity_is_incomplete_instead_of_available() {
    let root = test_root();
    let model_id = "llm/intent/corrupt-cache";
    create_model(root.path(), model_id, "example/corrupt-cache", None);
    let (_guard, api) = api(&root).await;
    api.resolve_model_package_facts(model_id).await.unwrap();

    let database = root.path().join("shared-resources/models/models.db");
    let connection = rusqlite::Connection::open(database).unwrap();
    let facts_json: String = connection
        .query_row(
            "SELECT facts_json FROM model_package_facts_cache WHERE model_id = ?1 AND cache_scope = 'detail'",
            [model_id],
            |row| row.get(0),
        )
        .unwrap();
    let mut facts: serde_json::Value = serde_json::from_str(&facts_json).unwrap();
    facts["model_ref"]["model_id"] = serde_json::Value::String("llm/intent/other".to_string());
    connection
        .execute(
            "UPDATE model_package_facts_cache SET facts_json = ?1 WHERE model_id = ?2 AND cache_scope = 'detail'",
            rusqlite::params![serde_json::to_string(&facts).unwrap(), model_id],
        )
        .unwrap();
    drop(connection);

    assert!(matches!(
        api.intent().get_model(&local_requirement(model_id)).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::PackageFactsStale)
    ));

    // A surviving quantization value is not evidence when its producer marked
    // the GGUF inspection invalid.
    let mut facts: serde_json::Value = serde_json::from_str(&facts_json).unwrap();
    facts["gguf"] = serde_json::json!({
        "status": "invalid",
        "quantization": "Q4_K_M"
    });
    let connection =
        rusqlite::Connection::open(root.path().join("shared-resources/models/models.db")).unwrap();
    connection
        .execute(
            "UPDATE model_package_facts_cache SET facts_json = ?1 WHERE model_id = ?2 AND cache_scope = 'detail'",
            rusqlite::params![serde_json::to_string(&facts).unwrap(), model_id],
        )
        .unwrap();
    drop(connection);
    let mut requirement = local_requirement(model_id);
    requirement.artifact.quantization = Some("Q4_K_M".to_string());
    assert!(matches!(
        api.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::QuantizationEvidenceMissing)
    ));
}

#[tokio::test]
async fn contradictory_cached_revision_and_unknown_format_are_incomplete() {
    let root = test_root();
    let model_id = "llm/intent/contradictory";
    create_model(
        root.path(),
        model_id,
        "example/contradictory",
        Some("commit-b"),
    );
    let (_guard, api) = api(&root).await;
    api.resolve_model_package_facts(model_id).await.unwrap();
    let connection =
        rusqlite::Connection::open(root.path().join("shared-resources/models/models.db")).unwrap();
    connection.execute(
        "UPDATE model_package_facts_cache SET facts_json = json_set(facts_json, '$.model_ref.revision', 'commit-a') WHERE model_id = ?1 AND cache_scope = 'summary'",
        [model_id],
    ).unwrap();
    let mut requirement = local_requirement(model_id);
    let ModelSelector::LocalModel { model_ref } = &mut requirement.selector else {
        unreachable!()
    };
    model_ref.revision = Some("commit-a".to_string());
    assert!(matches!(
        api.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::PackageFactsStale)
    ));

    let ModelSelector::LocalModel { model_ref } = &mut requirement.selector else {
        unreachable!()
    };
    model_ref.revision = Some("commit-b".to_string());
    assert!(matches!(
        api.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::PackageFactsStale)
    ));

    connection.execute(
        "UPDATE model_package_facts_cache SET facts_json = json_remove(facts_json, '$.model_ref.revision') WHERE model_id = ?1 AND cache_scope = 'summary'",
        [model_id],
    ).unwrap();
    connection.execute(
        "UPDATE model_package_facts_cache SET facts_json = json_set(facts_json, '$.model_ref.revision', 'commit-a') WHERE model_id = ?1 AND cache_scope = 'detail'",
        [model_id],
    ).unwrap();
    assert!(matches!(
        api.intent().get_model(&local_requirement(model_id)).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::PackageFactsStale)
    ));
    connection.execute(
        "UPDATE model_package_facts_cache SET facts_json = json_remove(facts_json, '$.model_ref.revision') WHERE model_id = ?1 AND cache_scope = 'detail'",
        [model_id],
    ).unwrap();
    connection.execute(
        "UPDATE model_package_facts_cache SET facts_json = json_set(facts_json, '$.artifact.artifact_kind', 'unknown') WHERE model_id = ?1 AND cache_scope = 'detail'",
        [model_id],
    ).unwrap();
    let mut requirement = local_requirement(model_id);
    requirement.artifact.format = Some(PackageArtifactKind::Safetensors);
    assert!(matches!(
        api.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::FormatEvidenceMissing)
    ));
    connection.execute(
        "UPDATE model_package_facts_cache SET facts_json = json_set(facts_json, '$.artifact_kind', 'unknown') WHERE model_id = ?1 AND cache_scope = 'summary'",
        [model_id],
    ).unwrap();
    connection.execute(
        "UPDATE model_package_facts_cache SET facts_json = json_set(facts_json, '$.artifact.artifact_kind', 'safetensors') WHERE model_id = ?1 AND cache_scope = 'detail'",
        [model_id],
    ).unwrap();
    assert!(matches!(
        api.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::FormatEvidenceMissing)
    ));
}

#[tokio::test]
async fn corrupt_gguf_inspection_never_returns_available_without_quantization_constraint() {
    let root = test_root();
    let model_id = "llm/intent/invalid-gguf";
    create_model(root.path(), model_id, "example/invalid-gguf", None);
    let model_dir = root.path().join("shared-resources/models").join(model_id);
    std::fs::remove_file(model_dir.join("model.safetensors")).unwrap();
    std::fs::write(model_dir.join("model.gguf"), b"not a GGUF header").unwrap();
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(model_dir.join("metadata.json")).unwrap()).unwrap();
    metadata["selected_artifact_id"] = serde_json::json!("model.gguf");
    metadata["entry_path"] = serde_json::json!(model_dir.join("model.gguf").display().to_string());
    metadata["files"] = serde_json::json!([{"name": "model.gguf"}]);
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    let (_guard, api) = api(&root).await;
    api.resolve_model_package_facts(model_id).await.unwrap();
    assert!(matches!(
        api.intent().get_model(&local_requirement(model_id)).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::PackageFactsInvalid)
    ));
}

#[tokio::test]
async fn gguf_header_quantization_matches_canonical_label_but_filename_is_not_proof() {
    let root = test_root();
    for (model_id, header_quant) in [
        ("llm/intent/header-quant", true),
        ("llm/intent/filename-quant", false),
    ] {
        create_model(root.path(), model_id, "example/quant", None);
        let model_dir = root.path().join("shared-resources/models").join(model_id);
        std::fs::remove_file(model_dir.join("model.safetensors")).unwrap();
        let filename = "model-Q4_K_M.gguf";
        let mut bytes = b"GGUF".to_vec();
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&u64::from(header_quant).to_le_bytes());
        if header_quant {
            let key = b"general.file_type";
            bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
            bytes.extend_from_slice(key);
            bytes.extend_from_slice(&4_u32.to_le_bytes());
            bytes.extend_from_slice(&13_u32.to_le_bytes());
        }
        std::fs::write(model_dir.join(filename), bytes).unwrap();
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(model_dir.join("metadata.json")).unwrap())
                .unwrap();
        metadata["selected_artifact_id"] = serde_json::json!(filename);
        metadata["entry_path"] = serde_json::json!(model_dir.join(filename).display().to_string());
        metadata["files"] = serde_json::json!([{"name": filename}]);
        std::fs::write(
            model_dir.join("metadata.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
    }
    let (_guard, api) = api(&root).await;
    for model_id in ["llm/intent/header-quant", "llm/intent/filename-quant"] {
        api.resolve_model_package_facts(model_id).await.unwrap();
    }
    let mut requirement = local_requirement("llm/intent/header-quant");
    requirement.artifact.format = Some(PackageArtifactKind::Gguf);
    requirement.artifact.quantization = Some("Q4_K_M".to_string());
    assert!(matches!(
        api.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Available { .. }
    ));
    let query = api.intent().query_models(&requirement).await.unwrap();
    let QueryModelsOutcome::Matches { candidates } = query else {
        panic!("expected matches")
    };
    assert!(candidates[0]
        .match_evidence
        .iter()
        .any(|item| item.value == "MOSTLY_Q4_K_M"));
    let ModelSelector::LocalModel { model_ref } = &mut requirement.selector else {
        unreachable!()
    };
    model_ref.model_id = "llm/intent/filename-quant".to_string();
    assert!(matches!(
        api.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::QuantizationEvidenceMissing)
    ));
}
