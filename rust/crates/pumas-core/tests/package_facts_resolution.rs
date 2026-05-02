use pumas_library::models::{
    BackendHintLabel, ModelFileInfo, ModelMetadata, PackageArtifactKind, PackageFactStatus,
    ProcessorComponentKind,
};
use pumas_library::ModelLibrary;
use tempfile::TempDir;

async fn setup_library() -> (TempDir, ModelLibrary) {
    let temp_dir = TempDir::new().unwrap();
    let library = ModelLibrary::new(temp_dir.path()).await.unwrap();
    (temp_dir, library)
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
