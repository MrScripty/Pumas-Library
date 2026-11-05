//! Controlled local package publication, never native execution or ASR evidence.
use pumas_library::{
    models::{AssetValidationState, ImportState, ModelImportSpec, ModelMetadata, StorageKind},
    PumasApi,
};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

const REQUIRED: &[&str] = &[
    "config.json",
    "model.safetensors",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
];
const OPTIONAL: &[&str] = &[
    "added_tokens.json",
    "generation_config.json",
    "processor_config.json",
    "special_tokens_map.json",
];

fn tensor() -> Vec<u8> {
    let mut header = serde_json::to_vec(&json!({
        "controlled_fixture_weight": {"dtype":"F32", "shape":[1], "data_offsets":[0,4]}
    }))
    .unwrap();
    while !header.len().is_multiple_of(8) {
        header.push(b' ');
    }
    [
        (header.len() as u64).to_le_bytes().as_slice(),
        header.as_slice(),
        1.0_f32.to_le_bytes().as_slice(),
    ]
    .concat()
}

fn write_json(root: &Path, name: &str, value: Value) {
    std::fs::write(root.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
}

fn canonical_metadata(root: &Path) -> ModelMetadata {
    serde_json::from_slice(&std::fs::read(root.join("metadata.json")).unwrap()).unwrap()
}

fn package(root: &Path, optional: bool) {
    write_json(
        root,
        "config.json",
        json!({
            "model_type":"cohere_asr",
            "architectures":["CohereAsrForConditionalGeneration"],
            "auto_map":{"AutoConfig":"configuration_cohere_asr.CohereAsrConfig"}
        }),
    );
    write_json(
        root,
        "preprocessor_config.json",
        json!({"feature_extractor_type":"CohereAsrFeatureExtractor"}),
    );
    write_json(
        root,
        "tokenizer_config.json",
        json!({"tokenizer_class":"CohereAsrTokenizer"}),
    );
    write_json(
        root,
        "tokenizer.json",
        json!({
            "version":"1.0", "truncation":null, "padding":null,
            "added_tokens":[], "normalizer":null, "pre_tokenizer":null,
            "post_processor":null, "decoder":null,
            "model":{"type":"WordLevel", "vocab":{"[UNK]":0}, "unk_token":"[UNK]"}
        }),
    );
    std::fs::write(root.join("model.safetensors"), tensor()).unwrap();
    if optional {
        for name in OPTIONAL {
            write_json(root, name, json!({}));
        }
        write_json(
            root,
            "processor_config.json",
            json!({"processor_class":"CohereAsrProcessor"}),
        );
    }
}

fn spec(source: &Path) -> ModelImportSpec {
    ModelImportSpec {
        path: source.to_str().unwrap().into(),
        family: "controlled-local-cohere".into(),
        official_name: "Local Cohere publication fixture".into(),
        repo_id: None,
        model_type: None,
        subtype: None,
        tags: None,
        security_acknowledged: None,
    }
}

async fn api(root: &Path) -> PumasApi {
    PumasApi::builder(root)
        .with_registry(
            pumas_library::registry::LibraryRegistry::open_at(&root.join("registry.db")).unwrap(),
        )
        .auto_create_dirs(true)
        .with_connectivity_probe(false)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}

fn assert_local_selection(metadata: &ModelMetadata, names: &BTreeSet<String>) {
    assert_eq!(metadata.storage_kind, Some(StorageKind::LibraryOwned));
    assert_eq!(metadata.validation_state, Some(AssetValidationState::Valid));
    assert_eq!(metadata.import_state, Some(ImportState::Ready));
    assert_eq!(metadata.config_model_type.as_deref(), Some("cohere_asr"));
    assert_eq!(
        metadata.task_type_primary.as_deref(),
        Some("speech-to-text")
    );
    assert!(metadata.import_publication.as_ref().unwrap().confirmed);
    let selected = metadata.selected_artifact_id.as_deref().unwrap();
    let digest = selected.strip_prefix("local-cohere-sha256:").unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let files = metadata.selected_artifact_files.as_ref().unwrap();
    assert_eq!(files.len(), names.len());
    assert_eq!(files.iter().cloned().collect::<BTreeSet<_>>(), *names);
    assert_eq!(
        metadata.task_classification_source.as_deref(),
        Some("explicit-local-cohere-import")
    );
    assert_eq!(
        metadata.input_modalities.as_deref(),
        Some([String::from("audio")].as_slice())
    );
    assert_eq!(
        metadata.output_modalities.as_deref(),
        Some([String::from("text")].as_slice())
    );
    assert_eq!(metadata.metadata_needs_review, Some(true));
    assert!(metadata.repo_id.is_none());
    assert!(metadata.upstream_revision.is_none());
    assert!(metadata.huggingface_evidence.is_none());
    assert!(metadata.pipeline_tag.is_none());
}

#[tokio::test]
async fn publishes_only_closed_members_with_canonical_and_indexed_local_identity() {
    for optional in [false, true] {
        let root = tempfile::TempDir::new().unwrap();
        let source = tempfile::TempDir::new().unwrap();
        package(source.path(), optional);
        for name in [
            "modeling_cohere_asr.py",
            "tokenizer.model",
            "other.safetensors",
            "model.pt",
        ] {
            std::fs::write(source.path().join(name), b"unselected; never execute").unwrap();
        }
        write_json(
            source.path(),
            "metadata.json",
            json!({
                "repo_id":"forged/upstream", "upstream_revision":"forged",
                "selected_artifact_id":"caller-claimed", "production_available":true
            }),
        );
        std::fs::create_dir(source.path().join("nested")).unwrap();
        std::fs::write(source.path().join("nested/model.py"), b"never execute").unwrap();
        let api = api(root.path()).await;
        let imported = api.import_local_cohere(&spec(source.path())).await.unwrap();
        assert!(imported.success, "{imported:?}");
        let id = imported.model_id.unwrap();
        let library = api.model_library();
        let target = library.library_root().join(&id);
        let names: BTreeSet<_> = REQUIRED
            .iter()
            .chain(if optional { OPTIONAL } else { &[] })
            .map(|name| (*name).to_owned())
            .collect();
        // Observe the publication's actual canonical document, not a metadata
        // presentation/readiness projection with different fallback semantics.
        let canonical = canonical_metadata(&target);
        assert_eq!(
            canonical.model_id.as_deref(),
            Some(id.as_str()),
            "canonical ID"
        );
        assert_eq!(
            canonical.model_type.as_deref(),
            Some("audio"),
            "canonical type"
        );
        let row = library.index().get(&id).unwrap().unwrap();
        // These columns, deliberately absent from projected JSON, own identity.
        assert_eq!(row.id, id, "indexed ID column");
        assert_eq!(row.model_type, "audio", "indexed type column");
        let indexed: ModelMetadata = serde_json::from_value(row.metadata.clone()).unwrap();
        assert_local_selection(&canonical, &names);
        assert_local_selection(&indexed, &names);
        assert_eq!(canonical.selected_artifact_id, indexed.selected_artifact_id);
        // Also exercise the public publication-readiness observer; raw documents
        // alone must not conceal a refused or degraded canonical observation.
        let observed = library.load_metadata(&target).unwrap().unwrap();
        assert_local_selection(&observed, &names);
        assert_eq!(
            observed.selected_artifact_id,
            canonical.selected_artifact_id
        );
        #[cfg(feature = "test-support")]
        {
            let selected = canonical.selected_artifact_id.as_deref().unwrap();
            let (manifest, prepared_members) = library
                .test_prepare_cohere_artifact_use(&id, selected)
                .unwrap();
            assert_eq!(manifest.len(), 64);
            assert!(manifest.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(prepared_members.len(), names.len());
            assert_eq!(prepared_members.into_iter().collect::<BTreeSet<_>>(), names);

            // Legacy embedded identity may not contradict the owning ID column.
            let mut conflicting = row.clone();
            conflicting.metadata["model_id"] = json!("audio/foreign/replacement");
            library.index().upsert(&conflicting).unwrap();
            let refused = library
                .test_prepare_cohere_artifact_use(&id, selected)
                .is_err();
            library.index().upsert(&row).unwrap();
            assert!(refused, "conflicting legacy indexed identity was admitted");
        }
        for name in &names {
            assert_eq!(
                std::fs::read(target.join(name)).unwrap(),
                std::fs::read(source.path().join(name)).unwrap()
            );
        }
        for name in [
            "modeling_cohere_asr.py",
            "tokenizer.model",
            "other.safetensors",
            "model.pt",
            "nested",
        ] {
            assert!(
                !target.join(name).exists(),
                "copied unselected member {name}"
            );
        }
        let publication: Value = serde_json::from_slice(
            &std::fs::read(target.join(".pumas_import_publication.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(publication["state"], "confirmed");
        assert_eq!(publication["model_id"], id);
        assert_eq!(
            std::fs::read(source.path().join("model.safetensors")).unwrap(),
            tensor()
        );
        api.shutdown_instance().await.unwrap();
    }
}

async fn assert_refused(source: &Path) {
    assert_spec_refused(&spec(source)).await;
}

async fn assert_spec_refused(spec: &ModelImportSpec) {
    let root = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let result = api.import_local_cohere(spec).await;
    let refused = !result.as_ref().is_ok_and(|result| result.success);
    let rows = api.model_library().index().count().unwrap();
    let packages = api.model_library().model_dirs().count();
    api.shutdown_instance().await.unwrap();
    assert!(refused, "accepted unsupported local package: {result:?}");
    assert_eq!(rows, 0, "refusal published an indexed model");
    assert_eq!(packages, 0, "refusal published canonical model metadata");
}

#[tokio::test]
async fn caller_repository_and_non_audio_intent_cannot_mint_local_provenance() {
    let source = tempfile::TempDir::new().unwrap();
    package(source.path(), false);
    let mut claimed_repository = spec(source.path());
    claimed_repository.repo_id = Some("CohereLabs/cohere-transcribe-03-2026".into());
    assert_spec_refused(&claimed_repository).await;
    let mut wrong_intent = spec(source.path());
    wrong_intent.model_type = Some("llm".into());
    assert_spec_refused(&wrong_intent).await;
}

#[tokio::test]
async fn missing_members_and_invalid_native_descriptors_never_publish() {
    for name in REQUIRED {
        let source = tempfile::TempDir::new().unwrap();
        package(source.path(), false);
        std::fs::remove_file(source.path().join(name)).unwrap();
        assert_refused(source.path()).await;
    }
    for name in [
        "config.json",
        "preprocessor_config.json",
        "tokenizer_config.json",
        "processor_config.json",
    ] {
        let source = tempfile::TempDir::new().unwrap();
        package(source.path(), true);
        std::fs::write(source.path().join(name), b"not JSON").unwrap();
        assert_refused(source.path()).await;
    }
    for config in [
        json!({"model_type":"other", "architectures":["CohereAsrForConditionalGeneration"]}),
        json!({"model_type":"cohere_asr", "architectures":["UnselectedClass"]}),
        json!({"model_type":"cohere_asr", "architectures":["CohereAsrForConditionalGeneration"], "auto_map":{"AutoModel":"repo--custom.Model"}}),
        json!({"model_type":"cohere_asr", "architectures":["CohereAsrForConditionalGeneration"], "nested":{"auto_map":{"AutoConfig":"custom.Config"}}}),
    ] {
        let source = tempfile::TempDir::new().unwrap();
        package(source.path(), false);
        write_json(source.path(), "config.json", config);
        assert_refused(source.path()).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn linked_selected_members_and_linked_source_directory_are_refused() {
    use std::os::unix::fs::symlink;
    for name in ["model.safetensors", "config.json", "generation_config.json"] {
        let source = tempfile::TempDir::new().unwrap();
        let foreign = tempfile::TempDir::new().unwrap();
        package(source.path(), true);
        let target = foreign.path().join(name);
        std::fs::rename(source.path().join(name), &target).unwrap();
        symlink(&target, source.path().join(name)).unwrap();
        assert_refused(source.path()).await;
    }
    let source = tempfile::TempDir::new().unwrap();
    let aliases = tempfile::TempDir::new().unwrap();
    package(source.path(), false);
    let alias = aliases.path().join("cohere");
    symlink(source.path(), &alias).unwrap();
    assert_refused(&alias).await;
}

async fn selected_identity(source: &Path) -> String {
    let root = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let result = api.import_local_cohere(&spec(source)).await.unwrap();
    assert!(result.success, "{result:?}");
    let id = result.model_id.unwrap();
    let identity = canonical_metadata(&api.model_library().library_root().join(id))
        .selected_artifact_id
        .unwrap();
    api.shutdown_instance().await.unwrap();
    identity
}

#[tokio::test]
async fn local_selection_identity_depends_on_selected_bytes_not_source_path_or_siblings() {
    let first = tempfile::TempDir::new().unwrap();
    let second = tempfile::TempDir::new().unwrap();
    package(first.path(), true);
    package(second.path(), true);
    std::fs::write(second.path().join("unselected.py"), b"never execute").unwrap();
    let identity = selected_identity(first.path()).await;
    assert_eq!(identity, selected_identity(second.path()).await);
    write_json(
        second.path(),
        "generation_config.json",
        json!({"max_length":17}),
    );
    assert_ne!(identity, selected_identity(second.path()).await);
}
