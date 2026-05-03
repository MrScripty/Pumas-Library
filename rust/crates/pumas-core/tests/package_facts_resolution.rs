use pumas_library::index::{
    DependencyProfileRecord, ModelDependencyBindingRecord, ModelPackageFactsCacheScope,
};
use pumas_library::models::{
    BackendHintLabel, ModelFileInfo, ModelMetadata, PackageArtifactKind, PackageFactStatus,
    ProcessorComponentKind, ResolvedModelPackageFactsSummary,
};
use pumas_library::ModelLibrary;
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
    assert!(facts
        .artifact
        .selected_files
        .contains(&"config.json".to_string()));
    assert!(facts
        .custom_code
        .dependency_manifests
        .contains(&"requirements.txt".to_string()));
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
    #[cfg(unix)]
    std::os::unix::fs::symlink(model_dir.join("model.safetensors"), &symlink_path).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(model_dir.join("model.safetensors"), &symlink_path).unwrap();

    let linked = library
        .resolve_pumas_model_ref(symlink_path.to_string_lossy().as_ref())
        .await
        .unwrap();
    assert_eq!(linked.model_id, model_id);
    assert!(linked.migration_diagnostics.is_empty());

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
            "model.layers.0.weight": "model-00002-of-00002.safetensors"
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
            .filter(|component| component.kind == ProcessorComponentKind::Shard)
            .count(),
        2
    );
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
