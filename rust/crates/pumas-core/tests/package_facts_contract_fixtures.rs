use pumas_library::models::{
    BackendHintLabel, HuggingFaceModel, ModelFactFamily, ModelLibraryChangeKind,
    ModelLibraryRefreshScope, ModelLibraryUpdateEvent, PackageArtifactKind, PackageFactStatus,
    ProcessorComponentKind, ResolvedModelPackageFacts, PACKAGE_FACTS_CONTRACT_VERSION,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("package_facts")
        .join(name)
}

fn load_fixture(name: &str) -> (Value, ResolvedModelPackageFacts) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: ResolvedModelPackageFacts =
        serde_json::from_str(&content).expect("fixture should match package facts contract");
    (raw, parsed)
}

fn load_update_event_fixture(name: &str) -> (Value, ModelLibraryUpdateEvent) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: ModelLibraryUpdateEvent =
        serde_json::from_str(&content).expect("fixture should match update event contract");
    (raw, parsed)
}

fn load_hf_search_fixture(name: &str) -> (Value, HuggingFaceModel) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: HuggingFaceModel =
        serde_json::from_str(&content).expect("fixture should match HF search contract");
    (raw, parsed)
}

#[test]
fn hf_text_generation_fixture_matches_contract() {
    let (raw, parsed) = load_fixture("hf_transformers_text_generation_package_facts.json");

    assert_eq!(
        parsed.package_facts_contract_version,
        PACKAGE_FACTS_CONTRACT_VERSION
    );
    assert_eq!(parsed.model_ref.model_id, "llm/example/tiny-transformers");
    assert_eq!(
        parsed.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert_eq!(
        parsed
            .components
            .iter()
            .find(|component| component.kind == ProcessorComponentKind::GenerationConfig)
            .map(|component| component.status),
        Some(PackageFactStatus::Present)
    );
    assert_eq!(
        parsed
            .transformers
            .as_ref()
            .and_then(|evidence| evidence.config_model_type.as_deref()),
        Some("llama")
    );
    assert_eq!(
        parsed
            .generation_defaults
            .defaults
            .as_ref()
            .and_then(|defaults| defaults.get("temperature"))
            .and_then(Value::as_f64),
        Some(0.7)
    );
    assert!(parsed.custom_code.requires_custom_code);
    assert!(parsed
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Transformers));
    assert!(parsed
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Vllm));
    assert!(parsed
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Mlx));

    let artifact = raw
        .get("artifact")
        .and_then(Value::as_object)
        .expect("artifact object should exist");
    assert!(
        artifact.get("validation_errors").is_none(),
        "empty optional validation errors should be omitted"
    );

    let backend_hints = raw
        .get("backend_hints")
        .and_then(Value::as_object)
        .expect("backend_hints object should exist");
    assert!(
        backend_hints.get("unsupported").is_none(),
        "empty optional unsupported hints should be omitted"
    );
}

#[test]
fn gguf_text_generation_fixture_matches_contract() {
    let (raw, parsed) = load_fixture("gguf_text_generation_package_facts.json");

    assert_eq!(
        parsed.package_facts_contract_version,
        PACKAGE_FACTS_CONTRACT_VERSION
    );
    assert_eq!(parsed.model_ref.model_id, "llm/llama/tiny-gguf");
    assert_eq!(parsed.artifact.artifact_kind, PackageArtifactKind::Gguf);
    assert!(
        parsed.transformers.is_none(),
        "GGUF fixture must not require HF/Transformers package evidence"
    );
    assert_eq!(
        parsed.task.task_type_primary.as_deref(),
        Some("text_generation")
    );
    assert!(parsed.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Quantization
            && component.message.as_deref() == Some("Q4_K_M")
    }));
    assert!(parsed
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::LlamaCpp));

    assert!(
        raw.get("sqlite").is_none() && raw.get("metadata_json").is_none(),
        "consumer fixtures must not expose Pumas SQLite/index internals"
    );
}

#[test]
fn gguf_embedding_fixture_matches_contract() {
    let (raw, parsed) = load_fixture("gguf_embedding_package_facts.json");

    assert_eq!(
        parsed.package_facts_contract_version,
        PACKAGE_FACTS_CONTRACT_VERSION
    );
    assert_eq!(
        parsed.model_ref.model_id,
        "embedding/qwen3/tiny-embedding-gguf"
    );
    assert_eq!(parsed.artifact.artifact_kind, PackageArtifactKind::Gguf);
    assert_eq!(
        parsed.task.pipeline_tag.as_deref(),
        Some("feature-extraction")
    );
    assert_eq!(parsed.task.task_type_primary.as_deref(), Some("embedding"));
    assert_eq!(parsed.task.output_modalities, vec!["embedding".to_string()]);
    assert!(parsed.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Quantization
            && component.message.as_deref() == Some("Q8_0")
    }));
    assert!(parsed
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::LlamaCpp));

    assert!(
        raw.get("sqlite").is_none() && raw.get("metadata_json").is_none(),
        "consumer fixtures must not expose Pumas SQLite/index internals"
    );
}

#[test]
fn unsupported_ollama_hint_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("unsupported_ollama_hint_package_facts.json");

    assert_eq!(parsed.artifact.artifact_kind, PackageArtifactKind::Gguf);
    assert!(parsed.backend_hints.accepted.is_empty());
    assert_eq!(parsed.backend_hints.raw, vec!["ollama".to_string()]);
    assert_eq!(parsed.backend_hints.unsupported, vec!["ollama".to_string()]);
}

#[test]
fn invalid_generation_config_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("invalid_generation_config_package_facts.json");

    assert_eq!(
        parsed.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert_eq!(
        parsed.generation_defaults.status,
        PackageFactStatus::Invalid
    );
    assert!(parsed
        .generation_defaults
        .diagnostics
        .iter()
        .any(
            |diagnostic| diagnostic.code == "invalid_generation_config_json"
                && diagnostic.path.as_deref() == Some("generation_config.json")
        ));
    assert!(parsed
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Transformers));
}

#[test]
fn missing_tokenizer_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("missing_tokenizer_package_facts.json");

    assert_eq!(
        parsed.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert!(parsed.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Tokenizer
            && component.status == PackageFactStatus::Missing
            && component.message.as_deref().is_some_and(|message| {
                message.contains("without a known tokenizer vocabulary file")
            })
    }));
    assert!(parsed.components.iter().any(|component| component.kind
        == ProcessorComponentKind::TokenizerConfig
        && component.class_name.as_deref() == Some("LlamaTokenizer")));
}

#[test]
fn custom_code_required_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("custom_code_required_package_facts.json");

    assert_eq!(
        parsed.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert!(parsed.custom_code.requires_custom_code);
    assert!(parsed
        .custom_code
        .custom_code_sources
        .contains(&"custom_generate/generate.py".to_string()));
    assert!(parsed
        .custom_code
        .custom_code_sources
        .contains(&"modeling_tiny.py".to_string()));
    assert!(parsed
        .custom_code
        .auto_map_sources
        .contains(&"modeling_tiny.TinyForCausalLM".to_string()));
    assert!(parsed
        .custom_code
        .dependency_manifests
        .contains(&"custom_generate/requirements.txt".to_string()));
    assert!(parsed.transformers.as_ref().is_some_and(|evidence| evidence
        .auto_map
        .contains(&"AutoModelForCausalLM".to_string())));
}

#[test]
fn remote_search_mlx_vllm_hint_fixture_matches_contract() {
    let (raw, parsed) = load_hf_search_fixture("remote_search_mlx_vllm_hint.json");

    assert_eq!(parsed.repo_id, "org/tiny-transformers-safetensors");
    assert_eq!(parsed.formats, vec!["safetensors".to_string()]);
    assert_eq!(
        parsed.compatible_engines,
        vec![
            "transformers".to_string(),
            "vllm".to_string(),
            "mlx".to_string()
        ]
    );
    assert!(
        raw.get("model_ref").is_none() && raw.get("artifact").is_none(),
        "remote search hints must not masquerade as installed-model package facts"
    );
}

#[test]
fn package_fact_status_defaults_to_uninspected() {
    assert_eq!(PackageFactStatus::default(), PackageFactStatus::Uninspected);
}

#[test]
fn model_library_update_event_fixture_matches_contract() {
    let (raw, parsed) =
        load_update_event_fixture("model_library_package_facts_modified_event.json");

    assert_eq!(parsed.cursor, "0000000000000001");
    assert_eq!(parsed.model_id, "llm/example/tiny-transformers");
    assert_eq!(
        parsed.change_kind,
        ModelLibraryChangeKind::PackageFactsModified
    );
    assert_eq!(parsed.fact_family, ModelFactFamily::PackageFacts);
    assert_eq!(parsed.refresh_scope, ModelLibraryRefreshScope::Detail);
    assert_eq!(parsed.selected_artifact_id.as_deref(), Some("main"));

    assert_eq!(
        raw.get("change_kind").and_then(Value::as_str),
        Some("package_facts_modified")
    );
    assert_eq!(
        raw.get("refresh_scope").and_then(Value::as_str),
        Some("detail")
    );
}
