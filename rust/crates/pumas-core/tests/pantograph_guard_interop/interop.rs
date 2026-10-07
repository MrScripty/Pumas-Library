use super::selected_audio_execution::SelectedAudioLoad;
use super::*;
use base64::Engine as _;
use pumas_library::index::{ModelPackageFactsCacheScope, ModelRecord};
use pumas_library::model_library::ModelLibrary;
use pumas_library::models as wire;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::HashMap;

const MODEL_ID: &str = "audio/cohere/synthetic-asr";
const REVISION: &str = "synthetic-revision-r1";

// This is the host's wire adaptation: remove Pumas's additional model-ref
// contract-version field, then deserialize into actual Pantograph contracts.
// It performs no guard validation and invents no producer evidence.
fn project<T: DeserializeOwned>(value: &impl serde::Serialize) -> T {
    fn strip_ref_version(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if map.contains_key("model_id") {
                    map.remove("model_ref_contract_version");
                }
                for child in map.values_mut() {
                    strip_ref_version(child);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    strip_ref_version(child);
                }
            }
            _ => {}
        }
    }
    let mut value = serde_json::to_value(value).unwrap();
    strip_ref_version(&mut value);
    serde_json::from_value(value).unwrap()
}

struct Fixture {
    _directory: tempfile::TempDir,
    library: ModelLibrary,
    package_path: std::path::PathBuf,
    request: InferenceExecutionRequest,
    target: PumasArtifactLoadTarget,
    decision: BackendExecutionDecision,
}

async fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let library = ModelLibrary::new(directory.path().join("models"))
        .await
        .unwrap();
    let package_path = library.library_root().join(MODEL_ID);
    std::fs::create_dir_all(&package_path).unwrap();
    let files = [
        ("config.json", json!({"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}).to_string()),
        ("preprocessor_config.json", json!({"feature_extractor_type":"WhisperFeatureExtractor","sampling_rate":16000}).to_string()),
        ("tokenizer_config.json", json!({"tokenizer_class":"PreTrainedTokenizerFast"}).to_string()),
        ("generation_config.json", json!({"max_length":64}).to_string()),
        // Deliberately synthetic and non-loadable; no model runtime is enabled.
        ("model.safetensors", "synthetic weight bytes for package inspection only".into()),
    ];
    for (name, content) in &files {
        std::fs::write(package_path.join(name), content).unwrap();
    }
    let metadata = wire::ModelMetadata {
        model_id: Some(MODEL_ID.into()),
        model_type: Some("audio".into()),
        family: Some("cohere".into()),
        cleaned_name: Some("synthetic-asr".into()),
        official_name: Some("Synthetic Cohere ASR inspection fixture".into()),
        config_model_type: Some("cohere_asr".into()),
        repo_id: Some("synthetic/cohere-asr-fixture".into()),
        upstream_revision: Some(REVISION.into()),
        selected_artifact_id: Some("hf-main".into()),
        selected_artifact_files: Some(files.iter().map(|(name, _)| (*name).into()).collect()),
        expected_files: Some(files.iter().map(|(name, _)| (*name).into()).collect()),
        storage_kind: Some(wire::StorageKind::LibraryOwned),
        validation_state: Some(wire::AssetValidationState::Valid),
        task_type_primary: Some("audio_transcription".into()),
        recommended_backend: Some("transformers".into()),
        runtime_engine_hints: Some(vec!["transformers".into()]),
        model_card: Some(HashMap::from([
            ("pipeline_tag".into(), json!("automatic-speech-recognition")),
            ("library_name".into(), json!("transformers")),
        ])),
        ..Default::default()
    };
    std::fs::write(
        package_path.join("metadata.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    // Register synthetic on-disk managed state through the public index API;
    // exercise the production package resolver and target resolver below.
    library
        .index()
        .upsert(&ModelRecord {
            id: MODEL_ID.into(),
            path: package_path.to_string_lossy().into_owned(),
            cleaned_name: "synthetic-asr".into(),
            official_name: "Synthetic Cohere ASR inspection fixture".into(),
            model_type: "audio".into(),
            tags: vec![],
            hashes: HashMap::new(),
            metadata: serde_json::to_value(metadata).unwrap(),
            updated_at: "2026-10-07T00:00:00Z".into(),
        })
        .unwrap();
    let raw_package = library.resolve_model_package_facts(MODEL_ID).await.unwrap();
    assert_eq!(raw_package.model_ref.revision.as_deref(), Some(REVISION));
    let response = library
        .resolve_model_artifact_load_target(wire::ResolveModelArtifactLoadTargetRequest {
            model_ref: raw_package.model_ref.clone(),
            expected_artifact_kind: None,
            caller_observed_entry_path: None,
            caller_observed_package_facts_contract_version: None,
            resolution_mode: wire::PumasArtifactLoadTargetResolutionMode::OwnerFresh,
            consumer: wire::PumasArtifactConsumer {
                consumer_name: "pantograph-guard-interop-test".into(),
                task_kind: Some("audio_transcription".into()),
                runtime_family: Some("pytorch.cpu".into()),
            },
        })
        .await
        .unwrap();
    assert!(response.is_ready(), "actual Pumas response: {response:?}");
    let mut target: PumasArtifactLoadTarget = project(&response.target.unwrap());
    assert_eq!(target.local_load_path, package_path.to_string_lossy());
    assert_eq!(target.load_path_kind, PumasArtifactLoadPathKind::Directory);
    assert!(target
        .content_fingerprint
        .as_deref()
        .unwrap()
        .starts_with("pumas-package-observation-v1:sha256:"));
    let package: ResolvedModelPackageFacts = project(&raw_package);
    // Compile and call the actual pinned host normalization, preserving a
    // path-free scheduler identity and the producer's known revision/artifact.
    let mut selected = package.model_ref.clone();
    selected.selected_artifact_path = None;
    let package = normalize_runtime_host_package_fact_identity(&selected, package);
    target.model_ref.selected_artifact_path = None;
    let model = package.model_ref.clone();
    let device = InferenceDeviceId::parse("cpu").unwrap();
    let runtime = RuntimeVariantId::parse("pytorch.cpu").unwrap();
    let decision = BackendExecutionDecision {
        selected_backend_id: BackendId::parse("pytorch").unwrap(),
        selected_runtime_variant_id: runtime.clone(),
        selected_device_class: InferenceDeviceClass::Cpu,
        selected_device_id: Some(device.clone()),
        device_decision: DeviceResolutionDecision {
            policy: InferenceDevicePolicy::Auto,
            runtime_variant_id: runtime,
            selected_device_class: InferenceDeviceClass::Cpu,
            selected_device_id: Some(device),
            diagnostics: vec![],
        },
        selected_task_id: Some(InferenceTaskId::AudioTranscription),
        selected_model_ref: Some(model.clone()),
        diagnostics: vec![],
        dependency_readiness: vec![],
        selection_policy_trace: None,
    };
    let request = InferenceExecutionRequest {
        request_id: Some("pumas-to-pantograph-synthetic-asr".into()),
        task_id: InferenceTaskId::AudioTranscription,
        model_ref: Some(model),
        model_name: None,
        resolved_model_package_facts: Some(package),
        input: InferenceExecutionInput::AudioTranscription {
            request: AudioTranscriptionRequest {
                model: MODEL_ID.into(),
                audio: Some(EncodedAudio {
                    data_base64: base64::engine::general_purpose::STANDARD.encode(tiny_wav()),
                    mime_type: "audio/wav".into(),
                    sample_rate_hz: Some(16000),
                }),
                audio_ref: None,
                language: Some("en".into()),
                prompt: None,
                task: Some("transcribe".into()),
                chunk_length_s: Some(0.5),
                extra_options: serde_json::Value::Null,
            },
        },
        generation_options: None,
        extra_options: serde_json::Value::Null,
    };
    Fixture {
        _directory: directory,
        library,
        package_path,
        request,
        target,
        decision,
    }
}

fn tiny_wav() -> Vec<u8> {
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend(38u32.to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(16000u32.to_le_bytes());
    wav.extend(32000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(2u32.to_le_bytes());
    wav.extend(0i16.to_le_bytes());
    wav
}

#[tokio::test]
async fn actual_pumas_managed_cohere_wire_passes_actual_pantograph_guard() {
    let fixture = fixture().await;
    SelectedAudioLoad::validate(&fixture.request, &fixture.target, &fixture.decision)
        .await
        .unwrap();
}

fn cache_snapshot(fixture: &Fixture, artifact_id: &str) -> serde_json::Value {
    let row = |scope| {
        fixture
            .library
            .index()
            .get_model_package_facts_cache(MODEL_ID, Some(artifact_id), scope)
            .unwrap()
    };
    json!({
        "summary": row(ModelPackageFactsCacheScope::Summary),
        "detail": row(ModelPackageFactsCacheScope::Detail),
    })
}

async fn actual_guard_cache_scope_and_resolution_mode(
    input_scope: ModelPackageFactsCacheScope,
    mode: wire::PumasArtifactLoadTargetResolutionMode,
) {
    let fixture = fixture().await;
    let artifact_id = fixture
        .target
        .model_ref
        .selected_artifact_id
        .as_deref()
        .unwrap();
    // Get Summary through its production API as well as the full package facts
    // already produced for the execution request. No cache facts are invented.
    let summary = fixture
        .library
        .resolve_model_package_facts_summary(MODEL_ID)
        .await
        .unwrap()
        .summary
        .unwrap();
    let summary_row = fixture
        .library
        .index()
        .get_model_package_facts_cache(
            MODEL_ID,
            Some(artifact_id),
            ModelPackageFactsCacheScope::Summary,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&summary_row.facts_json).unwrap(),
        serde_json::to_value(summary).unwrap(),
        "Summary row must come from the production summary API"
    );
    let row = fixture
        .library
        .index()
        .get_model_package_facts_cache(MODEL_ID, Some(artifact_id), input_scope)
        .unwrap()
        .unwrap();
    // Retain one unmodified production row to make cache evidence scope
    // unambiguous. These operations only affect this synthetic fixture database.
    fixture
        .library
        .index()
        .delete_model_package_facts_cache(MODEL_ID)
        .unwrap();
    fixture
        .library
        .index()
        .upsert_model_package_facts_cache(&row)
        .unwrap();
    let before = cache_snapshot(&fixture, artifact_id);
    match input_scope {
        ModelPackageFactsCacheScope::Summary => assert!(before["detail"].is_null()),
        ModelPackageFactsCacheScope::Detail => assert!(before["summary"].is_null()),
    }
    let indexed = matches!(
        mode,
        wire::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed
    );
    let mode_name = format!("{mode:?}");
    let response = fixture
        .library
        .resolve_model_artifact_load_target(wire::ResolveModelArtifactLoadTargetRequest {
            model_ref: project(&fixture.request.model_ref.as_ref().unwrap()),
            expected_artifact_kind: None,
            caller_observed_entry_path: None,
            caller_observed_package_facts_contract_version: None,
            resolution_mode: mode,
            consumer: wire::PumasArtifactConsumer {
                consumer_name: "pantograph-guard-interop-matrix".into(),
                task_kind: Some("audio_transcription".into()),
                runtime_family: Some("pytorch.cpu".into()),
            },
        })
        .await
        .unwrap();
    assert!(
        response.is_ready(),
        "{input_scope:?}/{mode_name}: {response:?}"
    );
    let after = cache_snapshot(&fixture, artifact_id);
    let accepted_scope = if indexed {
        assert_eq!(
            before, after,
            "ReadOnlyIndexed must not mutate any cache row"
        );
        input_scope
    } else {
        // OwnerFresh invokes the production detail observation, which repairs
        // the Summary row before the summary-first shared resolver runs.
        assert!(!after["summary"].is_null());
        assert!(!after["detail"].is_null());
        ModelPackageFactsCacheScope::Summary
    };
    let accepted_key = match accepted_scope {
        ModelPackageFactsCacheScope::Summary => "summary",
        ModelPackageFactsCacheScope::Detail => "detail",
    };
    // Round-trip the whole actual resolver response DTO. The guard target comes
    // exclusively from this serialized production response, never a literal.
    let response_bytes = serde_json::to_vec(&response).unwrap();
    let decoded: wire::ResolveModelArtifactLoadTargetResponse =
        serde_json::from_slice(&response_bytes).unwrap();
    let produced_target = decoded.target.unwrap();
    assert_eq!(
        produced_target.content_fingerprint.as_deref(),
        after[accepted_key]["source_fingerprint"].as_str()
    );
    assert_eq!(
        produced_target.model_ref.revision.as_deref(),
        Some(REVISION)
    );
    assert_eq!(
        produced_target.local_load_path,
        fixture.package_path.to_string_lossy()
    );
    let mut target: PumasArtifactLoadTarget = project(&produced_target);
    // This is the existing host's scheduler privacy projection only. Physical
    // target path, revision, artifact, fingerprint, and descriptor remain intact.
    target.model_ref.selected_artifact_path = fixture
        .decision
        .selected_model_ref
        .as_ref()
        .unwrap()
        .selected_artifact_path
        .clone();
    SelectedAudioLoad::validate(&fixture.request, &target, &fixture.decision)
        .await
        .unwrap();
    println!(
        "actual producer matrix: input_scope={input_scope:?}, mode={mode_name}, accepted_scope={accepted_scope:?}, indexed_cache_unchanged={indexed}"
    );
}

#[tokio::test]
async fn actual_producer_summary_owner_fresh_target_passes_actual_guard() {
    actual_guard_cache_scope_and_resolution_mode(
        ModelPackageFactsCacheScope::Summary,
        wire::PumasArtifactLoadTargetResolutionMode::OwnerFresh,
    )
    .await;
}

#[tokio::test]
async fn actual_producer_detail_owner_fresh_repairs_summary_and_passes_actual_guard() {
    actual_guard_cache_scope_and_resolution_mode(
        ModelPackageFactsCacheScope::Detail,
        wire::PumasArtifactLoadTargetResolutionMode::OwnerFresh,
    )
    .await;
}

#[tokio::test]
async fn actual_producer_summary_indexed_target_passes_actual_guard_without_cache_mutation() {
    actual_guard_cache_scope_and_resolution_mode(
        ModelPackageFactsCacheScope::Summary,
        wire::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
    )
    .await;
}

#[tokio::test]
async fn actual_producer_detail_indexed_target_passes_actual_guard_without_cache_mutation() {
    actual_guard_cache_scope_and_resolution_mode(
        ModelPackageFactsCacheScope::Detail,
        wire::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
    )
    .await;
}

#[tokio::test]
async fn actual_guard_rejects_primary_weight_path_despite_directory_descriptor() {
    let mut fixture = fixture().await;
    fixture.target.local_load_path = fixture
        .package_path
        .join("model.safetensors")
        .to_string_lossy()
        .into_owned();
    let error = SelectedAudioLoad::validate(&fixture.request, &fixture.target, &fixture.decision)
        .await
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("existing absolute directory"),
        "{error}"
    );
}

#[tokio::test]
async fn actual_guard_rejects_missing_or_blank_fingerprint() {
    let fixture = fixture().await;
    for fingerprint in [None, Some("".into()), Some("  \t".into())] {
        let mut target = fixture.target.clone();
        target.content_fingerprint = fingerprint;
        let error = SelectedAudioLoad::validate(&fixture.request, &target, &fixture.decision)
            .await
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("known selected content fingerprint"),
            "{error}"
        );
    }
}

#[tokio::test]
async fn actual_guard_rejects_revision_identity_and_contract_mismatches() {
    let fixture = fixture().await;
    for mode in [
        "revision_missing",
        "revision_wrong",
        "model",
        "artifact",
        "selected_path",
        "contract",
        "storage",
    ] {
        let mut target = fixture.target.clone();
        match mode {
            "revision_missing" => target.model_ref.revision = None,
            "revision_wrong" => target.model_ref.revision = Some("different-revision".into()),
            "model" => target.model_ref.model_id = "audio/cohere/another".into(),
            "artifact" => target.model_ref.selected_artifact_id = Some("other-artifact".into()),
            "selected_path" => target.model_ref.selected_artifact_path = Some("other/path".into()),
            "contract" => target.package_facts_contract_version = Some(1),
            "storage" => target.storage_kind = ModelStorageKind::Unknown,
            _ => unreachable!(),
        }
        assert!(
            SelectedAudioLoad::validate(&fixture.request, &target, &fixture.decision)
                .await
                .is_err(),
            "{mode}"
        );
    }
}

#[tokio::test]
async fn owner_reobservation_changes_wire_fingerprint_and_remains_admissible() {
    let mut fixture = fixture().await;
    let old = fixture.target.content_fingerprint.clone();
    std::fs::write(
        fixture.package_path.join("generation_config.json"),
        b"{\"max_length\":128}",
    )
    .unwrap();
    let raw_package = fixture
        .library
        .resolve_model_package_facts(MODEL_ID)
        .await
        .unwrap();
    let response = fixture
        .library
        .resolve_model_artifact_load_target(wire::ResolveModelArtifactLoadTargetRequest {
            model_ref: raw_package.model_ref.clone(),
            expected_artifact_kind: None,
            caller_observed_entry_path: None,
            caller_observed_package_facts_contract_version: None,
            resolution_mode: wire::PumasArtifactLoadTargetResolutionMode::OwnerFresh,
            consumer: wire::PumasArtifactConsumer {
                consumer_name: "pantograph-guard-interop-test".into(),
                task_kind: Some("audio_transcription".into()),
                runtime_family: Some("pytorch.cpu".into()),
            },
        })
        .await
        .unwrap();
    assert!(response.is_ready(), "{response:?}");
    fixture.target = project(&response.target.unwrap());
    fixture.target.model_ref.selected_artifact_path = None;
    let package: ResolvedModelPackageFacts = project(&raw_package);
    fixture.request.resolved_model_package_facts =
        Some(normalize_runtime_host_package_fact_identity(
            fixture.request.model_ref.as_ref().unwrap(),
            package,
        ));
    assert_ne!(old, fixture.target.content_fingerprint);
    SelectedAudioLoad::validate(&fixture.request, &fixture.target, &fixture.decision)
        .await
        .unwrap();
}
