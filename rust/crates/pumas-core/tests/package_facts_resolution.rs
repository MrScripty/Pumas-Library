use pumas_library::index::{
    DependencyProfileRecord, ModelDependencyBindingRecord, ModelPackageFactsCacheRowState,
    ModelPackageFactsCacheScope,
};
use pumas_library::models::{
    AssetValidationState, BackendHintLabel, BundleFormat, HuggingFaceEvidence,
    ImageGenerationFamilyLabel, ModelFileInfo, ModelMetadata, PackageArtifactKind,
    PackageFactStatus, PackageFactValueSource, ProcessorComponentKind,
    ResolvedModelPackageFactsSummary, StorageKind, PACKAGE_FACTS_CONTRACT_VERSION,
};
use pumas_library::ModelLibrary;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

async fn setup_library() -> (TempDir, ModelLibrary) {
    let temp_dir = TempDir::new().unwrap();
    let library = ModelLibrary::new(temp_dir.path()).await.unwrap();
    (temp_dir, library)
}

fn pinned_profile_spec(package: &str, version: &str) -> String {
    serde_json::json!({
        "python_packages": [
            {
                "name": package,
                "version": version
            }
        ]
    })
    .to_string()
}

fn write_minimal_gguf(path: &Path, metadata: &[Vec<u8>]) {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"GGUF");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&(metadata.len() as u64).to_le_bytes());
    for kv in metadata {
        bytes.extend_from_slice(kv);
    }
    std::fs::write(path, bytes).unwrap();
}

fn gguf_kv_string(key: &str, value: &str) -> Vec<u8> {
    let mut bytes = gguf_kv_header(key, 8);
    gguf_write_string(&mut bytes, value);
    bytes
}

fn gguf_kv_u32(key: &str, value: u32) -> Vec<u8> {
    let mut bytes = gguf_kv_header(key, 4);
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes
}

fn gguf_kv_u64(key: &str, value: u64) -> Vec<u8> {
    let mut bytes = gguf_kv_header(key, 10);
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes
}

fn gguf_kv_header(key: &str, value_type: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    gguf_write_string(&mut bytes, key);
    bytes.extend_from_slice(&value_type.to_le_bytes());
    bytes
}

fn gguf_write_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

async fn create_cache_test_model(library: &ModelLibrary, model_id: &str) {
    let model_dir = library.build_model_path("llm", "example", "cache-test");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type":"llama"}"#)
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("cache-test".to_string()),
        official_name: Some("Cache Test".to_string()),
        task_type_primary: Some("text_generation".to_string()),
        updated_date: Some("2026-05-02T00:00:00Z".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();
    library.rebuild_index().await.unwrap();
}

async fn create_selected_artifact_gguf_model(library: &ModelLibrary, model_id: &str) {
    let model_dir = library.build_model_path("llm", "llama", "multi-quant-gguf");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    write_minimal_gguf(
        &model_dir.join("model-Q4_K_M.gguf"),
        &[
            gguf_kv_string("general.architecture", "llama"),
            gguf_kv_u32("general.file_type", 13),
        ],
    );
    write_minimal_gguf(
        &model_dir.join("model-Q5_K_M.gguf"),
        &[
            gguf_kv_string("general.architecture", "llama"),
            gguf_kv_u32("general.file_type", 15),
        ],
    );

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("llama".to_string()),
        cleaned_name: Some("multi-quant-gguf".to_string()),
        official_name: Some("Multi Quant GGUF".to_string()),
        task_type_primary: Some("text_generation".to_string()),
        recommended_backend: Some("llama.cpp".to_string()),
        selected_artifact_id: Some("model-q5".to_string()),
        selected_artifact_files: Some(vec!["model-Q5_K_M.gguf".to_string()]),
        files: Some(vec![
            ModelFileInfo {
                name: "model-Q4_K_M.gguf".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
            ModelFileInfo {
                name: "model-Q5_K_M.gguf".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
        ]),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();
    library.rebuild_index().await.unwrap();
}

#[tokio::test]
async fn resolves_hf_transformers_package_facts_from_metadata_and_files() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/tiny-transformers";
    let model_dir = library.build_model_path("llm", "example", "tiny-transformers");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(
        model_dir.join("config.json"),
        r#"{
          "model_type": "llama",
          "architectures": ["LlamaForCausalLM"],
          "torch_dtype": "bfloat16",
          "auto_map": {
            "AutoConfig": "configuration_tiny.TinyConfig",
            "AutoModelForCausalLM": "modeling_tiny.TinyForCausalLM"
          }
        }"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("generation_config.json"),
        r#"{"max_new_tokens": 128, "temperature": 0.7}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(model_dir.join("tokenizer.json"), "{}")
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("requirements.txt"), "torch\n")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("tiny-transformers".to_string()),
        official_name: Some("Tiny Transformers".to_string()),
        files: Some(vec![ModelFileInfo {
            name: "model.safetensors".to_string(),
            original_name: None,
            size: None,
            sha256: None,
            blake3: None,
        }]),
        pipeline_tag: Some("text-generation".to_string()),
        task_type_primary: Some("text_generation".to_string()),
        input_modalities: Some(vec!["text".to_string()]),
        output_modalities: Some(vec!["text".to_string()]),
        recommended_backend: Some("transformers".to_string()),
        runtime_engine_hints: Some(vec!["vllm".to_string(), "mlx".to_string()]),
        custom_code_sources: Some(vec!["modeling_tiny.py".to_string()]),
        repo_id: Some("org/tiny-transformers".to_string()),
        huggingface_evidence: Some(HuggingFaceEvidence {
            repo_id: Some("org/fallback-transformers".to_string()),
            sibling_filenames: Some(vec![
                "README.md".to_string(),
                "config.json".to_string(),
                "model.safetensors".to_string(),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert_eq!(
        facts.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert_eq!(
        facts.task.task_type_primary.as_deref(),
        Some("text_generation")
    );
    assert_eq!(
        facts
            .transformers
            .as_ref()
            .and_then(|evidence| evidence.config_model_type.as_deref()),
        Some("llama")
    );
    assert_eq!(facts.generation_defaults.status, PackageFactStatus::Present);
    assert_eq!(
        facts
            .transformers
            .as_ref()
            .map(|evidence| evidence.auto_map.clone())
            .unwrap_or_default(),
        vec!["AutoConfig".to_string(), "AutoModelForCausalLM".to_string()]
    );
    assert_eq!(
        facts
            .components
            .iter()
            .find(|component| component.kind == ProcessorComponentKind::Tokenizer)
            .map(|component| component.status),
        Some(PackageFactStatus::Present)
    );
    assert!(facts.custom_code.requires_custom_code);
    assert_eq!(
        facts.custom_code.auto_map_sources,
        vec![
            "configuration_tiny.TinyConfig".to_string(),
            "modeling_tiny.TinyForCausalLM".to_string()
        ]
    );
    assert!(facts.custom_code.class_references.iter().any(|reference| {
        reference.kind == ProcessorComponentKind::Config
            && reference.class_name == "LlamaForCausalLM"
            && reference.source_path.as_deref() == Some("config.json")
    }));
    assert!(facts
        .artifact
        .selected_files
        .contains(&"config.json".to_string()));
    assert!(facts
        .artifact
        .sibling_files
        .contains(&"README.md".to_string()));
    assert!(facts
        .custom_code
        .dependency_manifests
        .contains(&"requirements.txt".to_string()));
    assert_eq!(
        facts
            .transformers
            .as_ref()
            .and_then(|evidence| evidence.source_repo_id.as_deref()),
        Some("org/tiny-transformers")
    );
    assert_eq!(
        facts
            .transformers
            .as_ref()
            .and_then(|evidence| evidence.source_revision.as_deref()),
        None
    );
    assert!(facts
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Transformers));
    assert!(facts
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Vllm));
    assert!(facts
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Mlx));
}

#[tokio::test]
async fn reports_invalid_generation_config_without_failing_package_resolution() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/bad-generation-config";
    let model_dir = library.build_model_path("llm", "example", "bad-generation-config");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type": "llama"}"#)
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("generation_config.json"), "{not-json")
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("bad-generation-config".to_string()),
        official_name: Some("Bad Generation Config".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert_eq!(facts.generation_defaults.status, PackageFactStatus::Invalid);
    assert!(facts
        .generation_defaults
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_generation_config_json"));
}

#[tokio::test]
async fn extracts_legacy_generation_defaults_from_config_when_generation_config_is_missing() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/legacy-generation-config";
    let model_dir = library.build_model_path("llm", "example", "legacy-generation-config");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(
        model_dir.join("config.json"),
        r#"{
          "model_type": "llama",
          "max_length": 2048,
          "temperature": 0.6,
          "top_p": 0.9
        }"#,
    )
    .await
    .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("legacy-generation-config".to_string()),
        official_name: Some("Legacy Generation Config".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();
    let defaults = facts
        .generation_defaults
        .defaults
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .unwrap();

    assert_eq!(facts.generation_defaults.status, PackageFactStatus::Present);
    assert_eq!(
        facts.generation_defaults.source_path.as_deref(),
        Some("config.json")
    );
    assert_eq!(
        defaults.get("max_length").and_then(|value| value.as_i64()),
        Some(2048)
    );
    assert_eq!(
        defaults.get("temperature").and_then(|value| value.as_f64()),
        Some(0.6)
    );
    assert!(facts
        .generation_defaults
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "legacy_config_generation_defaults"));
}

#[tokio::test]
async fn preserves_unsupported_backend_hints_as_raw_package_facts() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/ollama-hint";
    let model_dir = library.build_model_path("llm", "example", "ollama-hint");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("model.gguf"), "test")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("ollama-hint".to_string()),
        official_name: Some("Ollama Hint".to_string()),
        recommended_backend: Some("ollama".to_string()),
        runtime_engine_hints: Some(vec!["transformers".to_string()]),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert!(facts
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Transformers));
    assert!(!facts
        .backend_hints
        .accepted
        .iter()
        .any(|hint| serde_json::to_string(hint).unwrap() == "\"ollama\""));
    assert!(facts.backend_hints.raw.contains(&"ollama".to_string()));
    assert!(facts
        .backend_hints
        .unsupported
        .contains(&"ollama".to_string()));
}

#[tokio::test]
async fn extracts_custom_generate_code_evidence_without_loading_python() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/custom-generate";
    let model_dir = library.build_model_path("llm", "example", "custom-generate");
    tokio::fs::create_dir_all(model_dir.join("custom_generate"))
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type":"llama"}"#)
        .await
        .unwrap();
    tokio::fs::write(
        model_dir.join("custom_generate/generate.py"),
        "def generate(): pass",
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("custom_generate/requirements.txt"),
        "accelerate\n",
    )
    .await
    .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("custom-generate".to_string()),
        official_name: Some("Custom Generate".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert!(facts.custom_code.requires_custom_code);
    assert!(facts
        .custom_code
        .custom_code_sources
        .contains(&"custom_generate/generate.py".to_string()));
    assert!(facts
        .custom_code
        .dependency_manifests
        .contains(&"custom_generate/requirements.txt".to_string()));
    assert!(facts
        .artifact
        .selected_files
        .contains(&"custom_generate/generate.py".to_string()));
}

#[tokio::test]
async fn resolves_canonical_model_refs_and_reports_unresolved_legacy_paths() {
    let temp_dir = TempDir::new().unwrap();
    let library = ModelLibrary::new(temp_dir.path().join("library"))
        .await
        .unwrap();
    let model_id = "llm/example/ref-model";
    let model_dir = library.build_model_path("llm", "example", "ref-model");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type":"llama"}"#)
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();
    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("ref-model".to_string()),
        official_name: Some("Ref Model".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();
    library.rebuild_index().await.unwrap();

    let by_id = library.resolve_pumas_model_ref(model_id).await.unwrap();
    assert_eq!(by_id.model_id, model_id);
    assert!(by_id.migration_diagnostics.is_empty());

    let by_file = library
        .resolve_pumas_model_ref(
            model_dir
                .join("model.safetensors")
                .to_string_lossy()
                .as_ref(),
        )
        .await
        .unwrap();
    assert_eq!(by_file.model_id, model_id);
    assert_eq!(
        by_file.selected_artifact_path.as_deref(),
        Some(
            model_dir
                .join("model.safetensors")
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .as_ref()
        )
    );

    let unknown_id = library
        .resolve_pumas_model_ref("llm/example/missing")
        .await
        .unwrap();
    assert_eq!(unknown_id.model_id, "");
    assert!(unknown_id
        .migration_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unknown_model_id"));

    let outside_path = temp_dir.path().join("outside-model.gguf");
    tokio::fs::write(&outside_path, "gguf").await.unwrap();
    let outside = library
        .resolve_pumas_model_ref(outside_path.to_string_lossy().as_ref())
        .await
        .unwrap();
    assert_eq!(outside.model_id, "");
    assert!(outside
        .migration_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "legacy_path_outside_library"));
}

#[tokio::test]
async fn resolves_model_refs_through_canonicalized_legacy_paths() {
    let temp_dir = TempDir::new().unwrap();
    let library = ModelLibrary::new(temp_dir.path().join("library"))
        .await
        .unwrap();
    let model_id = "llm/example/canonical-paths";
    let model_dir = library.build_model_path("llm", "example", "canonical-paths");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type":"llama"}"#)
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();
    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("canonical-paths".to_string()),
        official_name: Some("Canonical Paths".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();
    library.rebuild_index().await.unwrap();

    let traversed_path = model_dir
        .join("..")
        .join("canonical-paths")
        .join("model.safetensors");
    let traversed = library
        .resolve_pumas_model_ref(traversed_path.to_string_lossy().as_ref())
        .await
        .unwrap();
    assert_eq!(traversed.model_id, model_id);
    assert!(traversed.migration_diagnostics.is_empty());

    let symlink_path = temp_dir.path().join("linked-model.safetensors");
    let symlink_created = {
        let target = model_dir.join("model.safetensors");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, &symlink_path).unwrap();
            true
        }
        #[cfg(windows)]
        {
            match std::os::windows::fs::symlink_file(&target, &symlink_path) {
                Ok(()) => true,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                    eprintln!("skipping symlink resolution assertion: {err}");
                    false
                }
                Err(err) => panic!("failed to create symlink for model ref test: {err}"),
            }
        }
    };

    if symlink_created {
        let linked = library
            .resolve_pumas_model_ref(symlink_path.to_string_lossy().as_ref())
            .await
            .unwrap();
        assert_eq!(linked.model_id, model_id);
        assert!(linked.migration_diagnostics.is_empty());
    }

    let unindexed_dir = library
        .library_root()
        .join("llm")
        .join("example")
        .join("unindexed");
    tokio::fs::create_dir_all(&unindexed_dir).await.unwrap();
    tokio::fs::write(unindexed_dir.join("model.gguf"), "gguf")
        .await
        .unwrap();

    let unindexed = library
        .resolve_pumas_model_ref(unindexed_dir.join("model.gguf").to_string_lossy().as_ref())
        .await
        .unwrap();
    assert_eq!(unindexed.model_id, "");
    assert!(unindexed
        .migration_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "legacy_path_not_indexed"));

    let missing_inside_library = library
        .library_root()
        .join("llm")
        .join("example")
        .join("missing-allowed-root")
        .join("model.gguf");
    let missing_inside = library
        .resolve_pumas_model_ref(missing_inside_library.to_string_lossy().as_ref())
        .await
        .unwrap();
    assert_eq!(missing_inside.model_id, "");
    assert!(missing_inside
        .migration_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "legacy_path_unresolved"));
}

#[tokio::test]
async fn extracts_processor_component_class_names_and_chat_templates() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "vlm/example/component-evidence";
    let model_dir = library.build_model_path("vlm", "example", "component-evidence");
    tokio::fs::create_dir_all(model_dir.join("chat_templates"))
        .await
        .unwrap();
    tokio::fs::write(
        model_dir.join("config.json"),
        r#"{"model_type":"llava","architectures":["LlavaForConditionalGeneration"]}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("tokenizer_config.json"),
        r#"{"tokenizer_class":"LlamaTokenizerFast"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("special_tokens_map.json"),
        r#"{"bos_token":"<s>","eos_token":"</s>"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("processor_config.json"),
        r#"{"processor_class":"LlavaProcessor"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("image_processor_config.json"),
        r#"{"image_processor_type":"CLIPImageProcessor"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("chat_templates/default.jinja"),
        "{{ messages }}",
    )
    .await
    .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("vlm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("component-evidence".to_string()),
        official_name: Some("Component Evidence".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Config
            && component.class_name.as_deref() == Some("LlavaForConditionalGeneration")
    }));
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::TokenizerConfig
            && component.class_name.as_deref() == Some("LlamaTokenizerFast")
    }));
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::SpecialTokensMap
            && component.relative_path.as_deref() == Some("special_tokens_map.json")
    }));
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Processor
            && component.class_name.as_deref() == Some("LlavaProcessor")
    }));
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::ImageProcessor
            && component.class_name.as_deref() == Some("CLIPImageProcessor")
    }));
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::ChatTemplate
            && component.relative_path.as_deref() == Some("chat_templates/default.jinja")
    }));
}

#[tokio::test]
async fn extracts_tokenizer_vocabulary_files_and_missing_diagnostics() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/tokenizer-vocab";
    let model_dir = library.build_model_path("llm", "example", "tokenizer-vocab");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type":"bert"}"#)
        .await
        .unwrap();
    tokio::fs::write(
        model_dir.join("tokenizer_config.json"),
        r#"{"tokenizer_class":"BertTokenizer"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("tokenizer.json"),
        r#"{
          "version": "1.0",
          "model": {"type": "BPE"},
          "normalizer": {"type": "Sequence"},
          "pre_tokenizer": {"type": "ByteLevel"}
        }"#,
    )
    .await
    .unwrap();
    tokio::fs::write(model_dir.join("vocab.txt"), "[PAD]\n[UNK]\n")
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("model.safetensors"), "test")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("tokenizer-vocab".to_string()),
        official_name: Some("Tokenizer Vocab".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Tokenizer
            && component.status == PackageFactStatus::Present
            && component.relative_path.as_deref() == Some("vocab.txt")
    }));
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Tokenizer
            && component.relative_path.as_deref() == Some("tokenizer.json")
            && component.message.as_deref().is_some_and(|message| {
                message.contains("model=BPE")
                    && message.contains("normalizer=Sequence")
                    && message.contains("pre_tokenizer=ByteLevel")
            })
    }));

    let missing_id = "llm/example/tokenizer-missing-vocab";
    let missing_dir = library.build_model_path("llm", "example", "tokenizer-missing-vocab");
    tokio::fs::create_dir_all(&missing_dir).await.unwrap();
    tokio::fs::write(missing_dir.join("config.json"), r#"{"model_type":"bert"}"#)
        .await
        .unwrap();
    tokio::fs::write(
        missing_dir.join("tokenizer_config.json"),
        r#"{"tokenizer_class":"BertTokenizer"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(missing_dir.join("model.safetensors"), "test")
        .await
        .unwrap();

    let missing_metadata = ModelMetadata {
        model_id: Some(missing_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("tokenizer-missing-vocab".to_string()),
        official_name: Some("Tokenizer Missing Vocab".to_string()),
        ..Default::default()
    };
    library
        .save_metadata(&missing_dir, &missing_metadata)
        .await
        .unwrap();

    let missing_facts = library
        .resolve_model_package_facts(missing_id)
        .await
        .unwrap();

    assert!(missing_facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Tokenizer
            && component.status == PackageFactStatus::Missing
            && component.message.as_deref().is_some_and(|message| {
                message.contains("without a known tokenizer vocabulary file")
            })
    }));
}

#[tokio::test]
async fn extracts_weight_index_and_shard_component_evidence() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/sharded-transformers";
    let model_dir = library.build_model_path("llm", "example", "sharded-transformers");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type":"llama"}"#)
        .await
        .unwrap();
    tokio::fs::write(
        model_dir.join("model.safetensors.index.json"),
        r#"{
          "metadata": {"total_size": 24},
          "weight_map": {
            "model.embed_tokens.weight": "model-00001-of-00002.safetensors",
            "model.layers.0.weight": "model-00002-of-00002.safetensors",
            "model.layers.1.weight": "model-00003-of-00003.safetensors"
          }
        }"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("model-00001-of-00002.safetensors"),
        "shard-1",
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("model-00002-of-00002.safetensors"),
        "shard-2",
    )
    .await
    .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("sharded-transformers".to_string()),
        official_name: Some("Sharded Transformers".to_string()),
        files: Some(vec![
            ModelFileInfo {
                name: "model.safetensors.index.json".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
            ModelFileInfo {
                name: "model-00001-of-00002.safetensors".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
            ModelFileInfo {
                name: "model-00002-of-00002.safetensors".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
        ]),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::WeightIndex
            && component.relative_path.as_deref() == Some("model.safetensors.index.json")
    }));
    assert_eq!(
        facts
            .components
            .iter()
            .filter(|component| {
                component.kind == ProcessorComponentKind::Shard
                    && component.status == PackageFactStatus::Present
            })
            .count(),
        2
    );
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Shard
            && component.status == PackageFactStatus::Missing
            && component.relative_path.as_deref() == Some("model-00003-of-00003.safetensors")
            && component
                .message
                .as_deref()
                .is_some_and(|message| message.contains("declared by model.safetensors.index.json"))
    }));
    assert!(facts
        .artifact
        .selected_files
        .contains(&"model.safetensors.index.json".to_string()));
}

#[tokio::test]
async fn extracts_quantization_component_evidence_from_config_and_filenames() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/quantized-package";
    let model_dir = library.build_model_path("llm", "example", "quantized-package");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(
        model_dir.join("config.json"),
        r#"{
          "model_type": "llama",
          "quantization_config": {
            "quant_method": "bitsandbytes",
            "load_in_4bit": true
          }
        }"#,
    )
    .await
    .unwrap();
    tokio::fs::write(model_dir.join("model-Q4_K_M.gguf"), "gguf")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("quantized-package".to_string()),
        official_name: Some("Quantized Package".to_string()),
        files: Some(vec![ModelFileInfo {
            name: "model-Q4_K_M.gguf".to_string(),
            original_name: None,
            size: None,
            sha256: None,
            blake3: None,
        }]),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Quantization
            && component.relative_path.as_deref() == Some("config.json")
            && component.message.as_deref() == Some("bitsandbytes")
    }));
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Quantization
            && component.relative_path.as_deref() == Some("model-Q4_K_M.gguf")
            && component.message.as_deref() == Some("Q4_K_M")
    }));
}

#[tokio::test]
async fn keeps_gguf_companion_facts_distinct_from_transformers_evidence() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "vlm/llava/gguf-mmproj";
    let model_dir = library.build_model_path("vlm", "llava", "gguf-mmproj");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(model_dir.join("model-Q4_K_M.gguf"), "gguf")
        .await
        .unwrap();
    tokio::fs::write(model_dir.join("mmproj-model-f16.gguf"), "mmproj")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("vlm".to_string()),
        family: Some("llava".to_string()),
        cleaned_name: Some("gguf-mmproj".to_string()),
        official_name: Some("GGUF MMProj".to_string()),
        task_type_primary: Some("image_text_to_text".to_string()),
        recommended_backend: Some("llama.cpp".to_string()),
        files: Some(vec![
            ModelFileInfo {
                name: "model-Q4_K_M.gguf".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
            ModelFileInfo {
                name: "mmproj-model-f16.gguf".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
        ]),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert_eq!(facts.artifact.artifact_kind, PackageArtifactKind::Gguf);
    assert_eq!(
        facts.artifact.companion_artifacts,
        vec!["mmproj-model-f16.gguf".to_string()]
    );
    assert!(
        facts.transformers.is_none(),
        "GGUF companion evidence must not imply HF/Transformers package evidence"
    );
    assert!(facts
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::LlamaCpp));
}

#[tokio::test]
async fn extracts_diffusers_package_evidence_from_bundle_files() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "image/stable-diffusion/tiny-sd";
    let model_dir = library.build_model_path("image", "stable-diffusion", "tiny-sd");
    tokio::fs::create_dir_all(model_dir.join("scheduler"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(model_dir.join("unet"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(model_dir.join("vae"))
        .await
        .unwrap();
    tokio::fs::write(
        model_dir.join("model_index.json"),
        r#"{
  "_class_name": "StableDiffusionPipeline",
  "_diffusers_version": "0.32.0",
  "_name_or_path": "synthetic/tiny-sd",
  "scheduler": ["diffusers", "EulerDiscreteScheduler"],
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"]
}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("scheduler/scheduler_config.json"),
        r#"{"model_type":"euler_scheduler"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        model_dir.join("unet/config.json"),
        r#"{"model_type":"unet_2d_condition"}"#,
    )
    .await
    .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("image".to_string()),
        family: Some("stable-diffusion".to_string()),
        cleaned_name: Some("tiny-sd".to_string()),
        official_name: Some("Tiny SD".to_string()),
        bundle_format: Some(BundleFormat::DiffusersDirectory),
        storage_kind: Some(StorageKind::LibraryOwned),
        validation_state: Some(AssetValidationState::Valid),
        task_type_primary: Some("image_generation".to_string()),
        recommended_backend: Some("diffusers".to_string()),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();
    let diffusers = facts
        .diffusers
        .expect("diffusers evidence should be present");

    assert_eq!(
        facts.artifact.artifact_kind,
        PackageArtifactKind::DiffusersBundle
    );
    assert_eq!(
        diffusers.pipeline_class.as_deref(),
        Some("StableDiffusionPipeline")
    );
    assert_eq!(
        diffusers
            .family_evidence
            .first()
            .map(|evidence| evidence.family),
        Some(ImageGenerationFamilyLabel::StableDiffusion)
    );
    assert!(diffusers.components.iter().any(|component| {
        component.config_path.as_deref() == Some("unet/config.json")
            && component.config_model_type.as_deref() == Some("unet_2d_condition")
    }));
    assert!(facts
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Diffusers));
}

#[tokio::test]
async fn extracts_header_derived_gguf_package_evidence() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/llama/tiny-header-gguf";
    let model_dir = library.build_model_path("llm", "llama", "tiny-header-gguf");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    write_minimal_gguf(
        &model_dir.join("model-Q4_K_M.gguf"),
        &[
            gguf_kv_string("general.architecture", "llama"),
            gguf_kv_u32("general.file_type", 13),
            gguf_kv_u64("llama.context_length", 4096),
            gguf_kv_u64("llama.embedding_length", 4096),
        ],
    );

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("llama".to_string()),
        cleaned_name: Some("tiny-header-gguf".to_string()),
        official_name: Some("Tiny Header GGUF".to_string()),
        task_type_primary: Some("text_generation".to_string()),
        recommended_backend: Some("llama.cpp".to_string()),
        files: Some(vec![ModelFileInfo {
            name: "model-Q4_K_M.gguf".to_string(),
            original_name: None,
            size: None,
            sha256: None,
            blake3: None,
        }]),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();
    let gguf = facts.gguf.expect("gguf evidence should be present");

    assert_eq!(facts.artifact.artifact_kind, PackageArtifactKind::Gguf);
    assert_eq!(gguf.architecture.as_deref(), Some("llama"));
    assert_eq!(gguf.quantization.as_deref(), Some("MOSTLY_Q4_K_M"));
    assert_eq!(gguf.value_source, Some(PackageFactValueSource::Header));
    assert_eq!(gguf.context_length, Some(4096));
    assert_eq!(gguf.embedding_length, Some(4096));
}

#[tokio::test]
async fn scopes_package_facts_cache_to_selected_artifact_id() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/llama/multi-quant-gguf";
    create_selected_artifact_gguf_model(&library, model_id).await;

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert_eq!(
        facts.model_ref.selected_artifact_id.as_deref(),
        Some("model-q5")
    );
    assert!(facts
        .model_ref
        .selected_artifact_path
        .as_deref()
        .is_some_and(|path| path.ends_with("model-Q5_K_M.gguf")));
    let gguf = facts.gguf.expect("gguf evidence should be present");
    assert_eq!(gguf.quantization.as_deref(), Some("MOSTLY_Q5_K_M"));

    let detail_row = library
        .index()
        .get_model_package_facts_cache(
            model_id,
            Some("model-q5"),
            ModelPackageFactsCacheScope::Detail,
        )
        .unwrap()
        .expect("selected artifact detail cache row");
    let summary_row = library
        .index()
        .get_model_package_facts_cache(
            model_id,
            Some("model-q5"),
            ModelPackageFactsCacheScope::Summary,
        )
        .unwrap()
        .expect("selected artifact summary cache row");

    assert_eq!(detail_row.selected_artifact_id, "model-q5");
    assert_eq!(summary_row.selected_artifact_id, "model-q5");
    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn extracts_adapter_package_evidence() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/lora-adapter";
    let model_dir = library.build_model_path("llm", "example", "lora-adapter");
    tokio::fs::create_dir_all(&model_dir).await.unwrap();
    tokio::fs::write(
        model_dir.join("adapter_config.json"),
        r#"{"peft_type":"LORA","base_model_name_or_path":"org/base-model"}"#,
    )
    .await
    .unwrap();
    tokio::fs::write(model_dir.join("adapter_model.safetensors"), "adapter")
        .await
        .unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("lora-adapter".to_string()),
        official_name: Some("LoRA Adapter".to_string()),
        files: Some(vec![
            ModelFileInfo {
                name: "adapter_config.json".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
            ModelFileInfo {
                name: "adapter_model.safetensors".to_string(),
                original_name: None,
                size: None,
                sha256: None,
                blake3: None,
            },
        ]),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();

    assert_eq!(facts.artifact.artifact_kind, PackageArtifactKind::Adapter);
    assert!(facts.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Adapter
            && component.relative_path.as_deref() == Some("adapter_config.json")
            && component.class_name.as_deref() == Some("LORA")
    }));
    assert!(facts
        .artifact
        .selected_files
        .contains(&"adapter_model.safetensors".to_string()));
}

#[tokio::test]
async fn reuses_fresh_package_facts_detail_cache() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let first = library.resolve_model_package_facts(model_id).await.unwrap();
    let cached = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let mut cached_facts = first.clone();
    cached_facts.task.task_type_primary = Some("cached_text_generation".to_string());
    let mut cached_row = cached.clone();
    cached_row.facts_json = serde_json::to_string(&cached_facts).unwrap();
    cached_row.updated_at = "2026-05-02T00:01:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&cached_row)
        .unwrap());

    let resolved = library.resolve_model_package_facts(model_id).await.unwrap();

    assert_eq!(
        resolved.task.task_type_primary.as_deref(),
        Some("cached_text_generation")
    );
}

#[tokio::test]
async fn persists_compact_package_facts_summary_cache() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let facts = library.resolve_model_package_facts(model_id).await.unwrap();
    let summary_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .unwrap();
    let detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let summary =
        serde_json::from_str::<ResolvedModelPackageFactsSummary>(&summary_row.facts_json).unwrap();

    assert_eq!(summary.model_ref.model_id, model_id);
    assert_eq!(summary.artifact_kind, facts.artifact.artifact_kind);
    assert_eq!(summary.storage_kind, facts.artifact.storage_kind);
    assert_eq!(summary.validation_state, facts.artifact.validation_state);
    assert_eq!(
        summary.task.task_type_primary.as_deref(),
        Some("text_generation")
    );
    assert_eq!(summary.config_status, PackageFactStatus::Present);
    assert_eq!(summary.tokenizer_status, PackageFactStatus::Uninspected);
    assert_eq!(summary.generation_config_status, PackageFactStatus::Missing);
    let summary_json = serde_json::from_str::<serde_json::Value>(&summary_row.facts_json).unwrap();
    assert!(summary_json.get("source_fingerprint").is_none());
    assert_eq!(
        summary_row.source_fingerprint,
        detail_row.source_fingerprint
    );
    assert!(summary_row.facts_json.len() < detail_row.facts_json.len());
    assert!(!summary_row.facts_json.contains("\"components\""));
    assert!(!summary_row.facts_json.contains("\"generation_defaults\""));
}

#[tokio::test]
async fn package_facts_cache_migration_dry_run_reports_missing_rows() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let report = library
        .generate_package_facts_cache_migration_dry_run_report()
        .await
        .unwrap();

    assert_eq!(
        report.target_package_facts_contract_version,
        PACKAGE_FACTS_CONTRACT_VERSION
    );
    assert_eq!(report.total_models, 1);
    assert_eq!(report.fresh_count, 0);
    assert_eq!(report.missing_count, 1);
    assert_eq!(report.regenerate_detail_count, 1);
    assert_eq!(report.regenerate_summary_count, 1);
    assert_eq!(report.error_count, 0);
    let item = report
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("dry-run item for cache test model");
    assert_eq!(item.detail_state, ModelPackageFactsCacheRowState::Missing);
    assert_eq!(item.summary_state, ModelPackageFactsCacheRowState::Missing);
    assert!(item.source_fingerprint.is_some());
    assert!(item.will_regenerate_detail);
    assert!(item.will_regenerate_summary);
    assert!(!item.will_delete_obsolete_rows);

    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .is_none());
    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn package_facts_cache_migration_dry_run_with_artifacts_writes_reports_and_index() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let report = library
        .generate_package_facts_cache_migration_dry_run_report_with_artifacts()
        .await
        .unwrap();

    let json_report_path = PathBuf::from(report.machine_readable_report_path.unwrap());
    let markdown_report_path = PathBuf::from(report.human_readable_report_path.unwrap());
    assert!(json_report_path.exists());
    assert!(markdown_report_path.exists());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(json_report_path).unwrap()).unwrap();
    assert_eq!(json["total_models"], 1);
    assert_eq!(json["regenerate_detail_count"], 1);
    assert_eq!(json["regenerate_summary_count"], 1);
    let markdown = std::fs::read_to_string(markdown_report_path).unwrap();
    assert!(markdown.contains("Package-Facts Cache Migration Dry-Run Report"));
    assert!(markdown.contains("Regenerate Detail Rows"));
    assert!(markdown.contains(model_id));

    let reports = library.list_migration_reports().unwrap();
    assert!(reports
        .iter()
        .any(|artifact| artifact.report_kind == "package_facts_cache_dry_run"));
}

#[tokio::test]
async fn package_facts_cache_migration_dry_run_reports_partial_download_without_hydrating() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/partial-cache-test";
    let partial_dir = library.build_model_path("llm", "example", "partial-cache-test");
    std::fs::create_dir_all(&partial_dir).unwrap();
    std::fs::write(partial_dir.join("model.safetensors.part"), b"partial").unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("partial-cache-test".to_string()),
        official_name: Some("Partial Cache Test".to_string()),
        match_source: Some("download_partial".to_string()),
        selected_artifact_id: Some("model-partial".to_string()),
        selected_artifact_files: Some(vec!["model.safetensors".to_string()]),
        ..Default::default()
    };
    library
        .upsert_index_from_metadata(&partial_dir, &metadata)
        .unwrap();

    let report = library
        .generate_package_facts_cache_migration_dry_run_report()
        .await
        .unwrap();

    assert_eq!(report.total_models, 1);
    assert_eq!(report.blocked_partial_download_count, 1);
    assert_eq!(report.regenerate_detail_count, 0);
    assert_eq!(report.regenerate_summary_count, 0);
    assert_eq!(report.error_count, 0);
    let item = report
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("dry-run item for partial download model");
    assert!(item.blocked_partial_download);
    assert_eq!(item.selected_artifact_id.as_deref(), Some("model-partial"));
    assert_eq!(
        item.selected_artifact_path.as_deref(),
        Some("model.safetensors")
    );
    assert_eq!(item.detail_state, ModelPackageFactsCacheRowState::Missing);
    assert_eq!(item.summary_state, ModelPackageFactsCacheRowState::Missing);
    assert!(item.source_fingerprint.is_none());
    assert!(!item.will_regenerate_detail);
    assert!(!item.will_regenerate_summary);
    assert!(item.error.is_none());
}

#[tokio::test]
async fn package_facts_cache_migration_execution_regenerates_missing_rows_and_clears_checkpoint() {
    let (temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let report = library
        .execute_package_facts_cache_migration_with_checkpoint()
        .await
        .unwrap();

    assert!(!report.resumed_from_checkpoint);
    assert_eq!(report.planned_work_count, 1);
    assert_eq!(report.regenerated_detail_count, 1);
    assert_eq!(report.regenerated_summary_count, 1);
    assert_eq!(report.deleted_obsolete_row_count, 0);
    assert_eq!(report.skipped_partial_download_count, 0);
    assert_eq!(report.error_count, 0);
    assert_eq!(report.results.len(), 1);
    let result = &report.results[0];
    assert_eq!(result.model_id, model_id);
    assert_eq!(result.action, "completed");
    assert!(result.regenerated_detail);
    assert!(result.regenerated_summary);
    assert!(result.planned_source_fingerprint.is_some());
    assert!(result.written_source_fingerprint.is_some());
    assert_eq!(
        result.planned_source_fingerprint,
        result.written_source_fingerprint
    );

    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .is_some());
    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .is_some());
    assert!(!temp_dir
        .path()
        .join(".package_facts_cache_migration_checkpoint.json")
        .exists());
}

#[tokio::test]
async fn package_facts_cache_migration_execution_deletes_obsolete_default_rows() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/llama/multi-quant-gguf";
    create_selected_artifact_gguf_model(&library, model_id).await;

    library.resolve_model_package_facts(model_id).await.unwrap();
    let selected_detail_row = library
        .index()
        .get_model_package_facts_cache(
            model_id,
            Some("model-q5"),
            ModelPackageFactsCacheScope::Detail,
        )
        .unwrap()
        .unwrap();
    let selected_summary_row = library
        .index()
        .get_model_package_facts_cache(
            model_id,
            Some("model-q5"),
            ModelPackageFactsCacheScope::Summary,
        )
        .unwrap()
        .unwrap();
    let mut obsolete_detail_row = selected_detail_row.clone();
    obsolete_detail_row.selected_artifact_id = String::new();
    obsolete_detail_row.updated_at = "2026-05-02T00:07:00Z".to_string();
    library
        .index()
        .upsert_model_package_facts_cache(&obsolete_detail_row)
        .unwrap();
    let mut obsolete_summary_row = selected_summary_row.clone();
    obsolete_summary_row.selected_artifact_id = String::new();
    obsolete_summary_row.updated_at = "2026-05-02T00:07:00Z".to_string();
    library
        .index()
        .upsert_model_package_facts_cache(&obsolete_summary_row)
        .unwrap();

    let report = library
        .execute_package_facts_cache_migration_with_checkpoint()
        .await
        .unwrap();

    assert_eq!(report.planned_work_count, 1);
    assert_eq!(report.regenerated_detail_count, 0);
    assert_eq!(report.regenerated_summary_count, 0);
    assert_eq!(report.deleted_obsolete_row_count, 2);
    assert_eq!(report.error_count, 0);
    assert_eq!(report.results[0].deleted_obsolete_rows, 2);
    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .is_none());
    assert!(library
        .index()
        .get_model_package_facts_cache(
            model_id,
            Some("model-q5"),
            ModelPackageFactsCacheScope::Detail
        )
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn package_facts_cache_migration_execution_reports_partial_download_skip() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/partial-cache-test";
    let partial_dir = library.build_model_path("llm", "example", "partial-cache-test");
    std::fs::create_dir_all(&partial_dir).unwrap();
    std::fs::write(partial_dir.join("model.safetensors.part"), b"partial").unwrap();

    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some("llm".to_string()),
        family: Some("example".to_string()),
        cleaned_name: Some("partial-cache-test".to_string()),
        official_name: Some("Partial Cache Test".to_string()),
        match_source: Some("download_partial".to_string()),
        selected_artifact_id: Some("model-partial".to_string()),
        selected_artifact_files: Some(vec!["model.safetensors".to_string()]),
        ..Default::default()
    };
    library
        .upsert_index_from_metadata(&partial_dir, &metadata)
        .unwrap();

    let report = library
        .execute_package_facts_cache_migration_with_checkpoint()
        .await
        .unwrap();

    assert_eq!(report.planned_work_count, 1);
    assert_eq!(report.regenerated_detail_count, 0);
    assert_eq!(report.regenerated_summary_count, 0);
    assert_eq!(report.skipped_partial_download_count, 1);
    assert_eq!(report.error_count, 0);
    let result = &report.results[0];
    assert_eq!(result.model_id, model_id);
    assert_eq!(result.action, "skipped_partial_download");
    assert_eq!(
        result.selected_artifact_id.as_deref(),
        Some("model-partial")
    );
    assert!(result.skipped_partial_download);
    assert!(result.error.is_none());
}

#[tokio::test]
async fn package_facts_cache_migration_execution_recomputes_fingerprint_on_resume() {
    let (temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let dry_run = library
        .generate_package_facts_cache_migration_dry_run_report()
        .await
        .unwrap();
    let item = dry_run
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("dry-run item for cache test model");
    let planned_source_fingerprint = item.source_fingerprint.clone();
    let checkpoint_path = temp_dir
        .path()
        .join(".package_facts_cache_migration_checkpoint.json");
    let checkpoint = serde_json::json!({
        "created_at": "2026-05-02T00:08:00Z",
        "updated_at": "2026-05-02T00:08:00Z",
        "pending_work": [
            {
                "model_id": model_id,
                "selected_artifact_id": null,
                "target_package_facts_contract_version": PACKAGE_FACTS_CONTRACT_VERSION,
                "source_fingerprint": planned_source_fingerprint,
                "regenerate_detail": true,
                "regenerate_summary": true,
                "delete_obsolete_rows": false,
                "skip_partial_download": false
            }
        ],
        "completed_results": []
    });
    std::fs::write(
        &checkpoint_path,
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();
    let model_dir = library.build_model_path("llm", "example", "cache-test");
    tokio::fs::write(model_dir.join("config.json"), r#"{"model_type":"mistral"}"#)
        .await
        .unwrap();

    let report = library
        .execute_package_facts_cache_migration_with_checkpoint()
        .await
        .unwrap();

    assert!(report.resumed_from_checkpoint);
    assert_eq!(report.planned_work_count, 1);
    assert_eq!(report.error_count, 0);
    let result = &report.results[0];
    assert_eq!(
        result.planned_source_fingerprint,
        planned_source_fingerprint
    );
    assert!(result.written_source_fingerprint.is_some());
    assert_ne!(
        result.written_source_fingerprint,
        result.planned_source_fingerprint
    );
    let detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    assert_eq!(
        Some(detail_row.source_fingerprint),
        result.written_source_fingerprint
    );
    assert!(!checkpoint_path.exists());
}

#[tokio::test]
async fn package_facts_cache_migration_dry_run_reports_stale_and_invalid_rows() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    library.resolve_model_package_facts(model_id).await.unwrap();
    let detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let summary_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .unwrap();
    let invalid_detail_payload =
        serde_json::json!({"not": "resolved_model_package_facts"}).to_string();
    let mut invalid_detail_row = detail_row.clone();
    invalid_detail_row.facts_json = invalid_detail_payload.clone();
    invalid_detail_row.updated_at = "2026-05-02T00:05:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&invalid_detail_row)
        .unwrap());
    let mut stale_summary_row = summary_row.clone();
    stale_summary_row.package_facts_contract_version =
        i64::from(PACKAGE_FACTS_CONTRACT_VERSION) - 1;
    stale_summary_row.updated_at = "2026-05-02T00:05:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&stale_summary_row)
        .unwrap());

    let report = library
        .generate_package_facts_cache_migration_dry_run_report()
        .await
        .unwrap();

    assert_eq!(report.total_models, 1);
    assert_eq!(report.fresh_count, 0);
    assert_eq!(report.missing_count, 0);
    assert_eq!(report.stale_contract_count, 1);
    assert_eq!(report.invalid_json_count, 1);
    assert_eq!(report.regenerate_detail_count, 1);
    assert_eq!(report.regenerate_summary_count, 1);
    assert_eq!(report.error_count, 0);
    let item = report
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("dry-run item for cache test model");
    assert_eq!(
        item.detail_state,
        ModelPackageFactsCacheRowState::InvalidJson
    );
    assert_eq!(
        item.summary_state,
        ModelPackageFactsCacheRowState::StaleContract
    );
    assert!(item.will_regenerate_detail);
    assert!(item.will_regenerate_summary);

    let unchanged_detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let unchanged_summary_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .unwrap();
    assert_eq!(unchanged_detail_row.facts_json, invalid_detail_payload);
    assert_eq!(
        unchanged_summary_row.package_facts_contract_version,
        stale_summary_row.package_facts_contract_version
    );
}

#[tokio::test]
async fn package_facts_cache_migration_dry_run_scopes_selected_artifact_rows() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/llama/multi-quant-gguf";
    create_selected_artifact_gguf_model(&library, model_id).await;

    library.resolve_model_package_facts(model_id).await.unwrap();
    let selected_detail_row = library
        .index()
        .get_model_package_facts_cache(
            model_id,
            Some("model-q5"),
            ModelPackageFactsCacheScope::Detail,
        )
        .unwrap()
        .unwrap();
    let selected_summary_row = library
        .index()
        .get_model_package_facts_cache(
            model_id,
            Some("model-q5"),
            ModelPackageFactsCacheScope::Summary,
        )
        .unwrap()
        .unwrap();
    let mut obsolete_detail_row = selected_detail_row.clone();
    obsolete_detail_row.selected_artifact_id = String::new();
    obsolete_detail_row.updated_at = "2026-05-02T00:06:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&obsolete_detail_row)
        .unwrap());
    let mut obsolete_summary_row = selected_summary_row.clone();
    obsolete_summary_row.selected_artifact_id = String::new();
    obsolete_summary_row.updated_at = "2026-05-02T00:06:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&obsolete_summary_row)
        .unwrap());

    let report = library
        .generate_package_facts_cache_migration_dry_run_report()
        .await
        .unwrap();

    assert_eq!(report.total_models, 1);
    assert_eq!(report.fresh_count, 0);
    assert_eq!(report.missing_count, 0);
    assert_eq!(report.regenerate_detail_count, 0);
    assert_eq!(report.regenerate_summary_count, 0);
    assert_eq!(report.delete_obsolete_row_count, 2);
    let item = report
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("dry-run item for selected artifact model");
    assert_eq!(item.selected_artifact_id.as_deref(), Some("model-q5"));
    assert!(item
        .selected_artifact_path
        .as_deref()
        .is_some_and(|path| path.ends_with("model-Q5_K_M.gguf")));
    assert_eq!(item.detail_state, ModelPackageFactsCacheRowState::Fresh);
    assert_eq!(item.summary_state, ModelPackageFactsCacheRowState::Fresh);
    assert!(item.will_delete_obsolete_rows);
    assert_eq!(item.obsolete_empty_selected_artifact_rows, 2);

    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .is_some());
    assert!(library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn dependency_binding_changes_refresh_package_fact_cache_fingerprints() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let first = library.resolve_model_package_facts(model_id).await.unwrap();
    let first_detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let first_summary_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .unwrap();
    assert_eq!(
        first_summary_row.source_fingerprint,
        first_detail_row.source_fingerprint
    );

    let mut stale_facts = first.clone();
    stale_facts.task.task_type_primary = Some("stale_cached_task".to_string());
    let mut stale_row = first_detail_row.clone();
    stale_row.facts_json = serde_json::to_string(&stale_facts).unwrap();
    stale_row.updated_at = "2026-05-02T00:01:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&stale_row)
        .unwrap());

    assert!(library
        .index()
        .upsert_dependency_profile(&DependencyProfileRecord {
            profile_id: "torch-cpu".to_string(),
            profile_version: 1,
            profile_hash: None,
            environment_kind: "python".to_string(),
            spec_json: pinned_profile_spec("torch", "==2.5.1"),
            created_at: "2026-05-02T00:02:00Z".to_string(),
        })
        .unwrap());
    assert!(library
        .index()
        .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
            binding_id: "cache-test-binding".to_string(),
            model_id: model_id.to_string(),
            profile_id: "torch-cpu".to_string(),
            profile_version: 1,
            binding_kind: "required_core".to_string(),
            backend_key: Some("transformers".to_string()),
            platform_selector: None,
            status: "active".to_string(),
            priority: 100,
            attached_by: Some("test".to_string()),
            attached_at: "2026-05-02T00:02:00Z".to_string(),
            profile_hash: None,
            environment_kind: None,
            spec_json: None,
        })
        .unwrap());

    let resolved = library.resolve_model_package_facts(model_id).await.unwrap();
    let refreshed_detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let refreshed_summary_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Summary)
        .unwrap()
        .unwrap();

    assert_eq!(
        resolved.task.task_type_primary.as_deref(),
        Some("text_generation")
    );
    assert_ne!(
        refreshed_detail_row.source_fingerprint,
        first_detail_row.source_fingerprint
    );
    assert_eq!(
        refreshed_summary_row.source_fingerprint,
        refreshed_detail_row.source_fingerprint
    );
    assert!(!refreshed_detail_row
        .facts_json
        .contains("stale_cached_task"));
}

#[tokio::test]
async fn list_search_and_rebuild_skip_package_facts_detail_cache() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    library.resolve_model_package_facts(model_id).await.unwrap();
    let detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let invalid_detail_payload =
        serde_json::json!({"not": "resolved_model_package_facts"}).to_string();
    let mut invalid_detail_row = detail_row.clone();
    invalid_detail_row.facts_json = invalid_detail_payload.clone();
    invalid_detail_row.updated_at = "2026-05-02T00:03:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&invalid_detail_row)
        .unwrap());

    let listed = library.list_models().await.unwrap();
    assert!(listed.iter().any(|model| model.id == model_id));

    let searched = library.search_models("cache", 10, 0).await.unwrap();
    assert!(searched.models.iter().any(|model| model.id == model_id));

    assert_eq!(library.rebuild_index().await.unwrap(), 1);
    let unchanged_detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    assert_eq!(unchanged_detail_row.facts_json, invalid_detail_payload);
}

#[tokio::test]
async fn recovers_from_invalid_package_facts_detail_cache_payload() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    library.resolve_model_package_facts(model_id).await.unwrap();
    let detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let invalid_detail_payload =
        serde_json::json!({"not": "resolved_model_package_facts"}).to_string();
    let mut invalid_detail_row = detail_row.clone();
    invalid_detail_row.facts_json = invalid_detail_payload.clone();
    invalid_detail_row.updated_at = "2026-05-02T00:04:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&invalid_detail_row)
        .unwrap());

    let resolved = library.resolve_model_package_facts(model_id).await.unwrap();
    let recovered_detail_row = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();

    assert_eq!(
        resolved.task.task_type_primary.as_deref(),
        Some("text_generation")
    );
    assert_ne!(recovered_detail_row.facts_json, invalid_detail_payload);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&recovered_detail_row.facts_json).unwrap(),
        serde_json::to_value(resolved).unwrap()
    );
}

#[tokio::test]
async fn regenerates_stale_package_facts_detail_cache() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let first = library.resolve_model_package_facts(model_id).await.unwrap();
    let cached = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();
    let mut stale_facts = first.clone();
    stale_facts.task.task_type_primary = Some("stale_cached_task".to_string());
    let mut stale_row = cached.clone();
    stale_row.source_fingerprint = "stale-fingerprint".to_string();
    stale_row.facts_json = serde_json::to_string(&stale_facts).unwrap();
    stale_row.updated_at = "2026-05-02T00:01:00Z".to_string();
    assert!(library
        .index()
        .upsert_model_package_facts_cache(&stale_row)
        .unwrap());

    let resolved = library.resolve_model_package_facts(model_id).await.unwrap();
    let refreshed = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();

    assert_eq!(
        resolved.task.task_type_primary.as_deref(),
        Some("text_generation")
    );
    assert_ne!(refreshed.source_fingerprint, "stale-fingerprint");
    assert!(!refreshed.facts_json.contains("stale_cached_task"));
}

#[tokio::test]
async fn concurrent_package_facts_requests_share_cache_path() {
    let (_temp_dir, library) = setup_library().await;
    let model_id = "llm/example/cache-test";
    create_cache_test_model(&library, model_id).await;

    let left_library = library.clone();
    let right_library = library.clone();
    let (left, right) = tokio::join!(
        left_library.resolve_model_package_facts(model_id),
        right_library.resolve_model_package_facts(model_id)
    );
    let left = left.unwrap();
    let right = right.unwrap();
    let cached = library
        .index()
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .unwrap();

    assert_eq!(left, right);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cached.facts_json).unwrap(),
        serde_json::to_value(left).unwrap()
    );
}
