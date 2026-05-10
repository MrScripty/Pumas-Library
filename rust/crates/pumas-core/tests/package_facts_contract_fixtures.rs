use pumas_library::index::{ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope};
use pumas_library::models::{
    BackendHintLabel, DiffusersComponentRole, HuggingFaceModel, ImageGenerationFamilyLabel,
    ModelFactFamily, ModelLibraryChangeKind, ModelLibraryRefreshScope, ModelLibraryUpdateEvent,
    PackageArtifactKind, PackageFactStatus, ProcessorComponentKind, ResolvedModelPackageFacts,
    ResolvedModelPackageFactsSummary, PACKAGE_FACTS_CONTRACT_VERSION,
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

fn load_cache_fixture(name: &str) -> (Value, ModelPackageFactsCacheRecord) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: ModelPackageFactsCacheRecord =
        serde_json::from_str(&content).expect("fixture should match cache record contract");
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
            .and_then(|evidence| evidence.source_repo_id.as_deref()),
        Some("org/tiny-transformers")
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
    assert!(parsed.custom_code.class_references.iter().any(|reference| {
        reference.kind == ProcessorComponentKind::Config
            && reference.class_name == "LlamaForCausalLM"
            && reference.source_path.as_deref() == Some("config.json")
    }));
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
    assert!(parsed
        .artifact
        .sibling_files
        .contains(&"README.md".to_string()));
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
fn diffusers_text_to_image_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("diffusers_sd_text_to_image_package_facts.json");

    assert_eq!(
        parsed.package_facts_contract_version,
        PACKAGE_FACTS_CONTRACT_VERSION
    );
    assert_eq!(
        parsed.artifact.artifact_kind,
        PackageArtifactKind::DiffusersBundle
    );
    assert!(
        parsed.transformers.is_none(),
        "Diffusers fixture should use structured diffusers evidence instead of masquerading as Transformers"
    );
    let diffusers = parsed
        .diffusers
        .as_ref()
        .expect("diffusers evidence should be present");
    assert_eq!(
        diffusers.pipeline_class.as_deref(),
        Some("StableDiffusionPipeline")
    );
    assert!(diffusers.family_evidence.iter().any(|evidence| {
        evidence.family == ImageGenerationFamilyLabel::StableDiffusion
            && evidence.source_path.as_deref() == Some("model_index.json")
    }));
    assert!(diffusers.components.iter().any(|component| {
        component.role == DiffusersComponentRole::Scheduler
            && component.config_path.as_deref() == Some("scheduler/scheduler_config.json")
    }));
    assert!(parsed
        .backend_hints
        .accepted
        .contains(&BackendHintLabel::Diffusers));

    let summary = ResolvedModelPackageFactsSummary::from(&parsed);
    assert_eq!(
        summary.diffusers_pipeline_class.as_deref(),
        Some("StableDiffusionPipeline")
    );
    assert_eq!(
        summary
            .image_generation_family_evidence
            .first()
            .map(|evidence| evidence.family),
        Some(ImageGenerationFamilyLabel::StableDiffusion)
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
fn hf_rerank_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("hf_rerank_package_facts.json");

    assert_eq!(parsed.model_ref.model_id, "reranker/qwen3/tiny-reranker");
    assert_eq!(
        parsed.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert_eq!(parsed.task.pipeline_tag.as_deref(), Some("text-ranking"));
    assert_eq!(parsed.task.task_type_primary.as_deref(), Some("rerank"));
    assert_eq!(parsed.task.output_modalities, vec!["score".to_string()]);
    assert!(parsed.transformers.as_ref().is_some_and(|evidence| evidence
        .architectures
        .contains(&"Qwen3ForSequenceClassification".to_string())));
}

#[test]
fn hf_multimodal_processor_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("hf_multimodal_processor_package_facts.json");

    assert_eq!(parsed.model_ref.model_id, "vlm/llava/tiny-multimodal");
    assert_eq!(
        parsed.artifact.artifact_kind,
        PackageArtifactKind::HfCompatibleDirectory
    );
    assert_eq!(
        parsed.task.pipeline_tag.as_deref(),
        Some("image-text-to-text")
    );
    assert_eq!(
        parsed.task.input_modalities,
        vec!["image".to_string(), "text".to_string()]
    );
    assert!(parsed.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::Processor
            && component.class_name.as_deref() == Some("LlavaProcessor")
    }));
    assert!(parsed.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::ImageProcessor
            && component.class_name.as_deref() == Some("CLIPImageProcessor")
    }));
    assert!(parsed.components.iter().any(|component| {
        component.kind == ProcessorComponentKind::ChatTemplate
            && component.relative_path.as_deref() == Some("chat_template.jinja")
    }));
}

#[test]
fn stale_package_facts_fixture_matches_cache_contract() {
    let (_raw, parsed) = load_cache_fixture("stale_package_facts.json");

    assert_eq!(parsed.model_id, "llm/example/stale-cache");
    assert_eq!(parsed.cache_scope, ModelPackageFactsCacheScope::Detail);
    assert_ne!(
        parsed.package_facts_contract_version,
        i64::from(PACKAGE_FACTS_CONTRACT_VERSION),
        "fixture must model a stale cache contract version"
    );
    assert_eq!(parsed.source_fingerprint, "stale-source-fingerprint");

    let cached_facts: ResolvedModelPackageFacts =
        serde_json::from_str(&parsed.facts_json).expect("facts_json should remain decodable");
    assert_eq!(cached_facts.model_ref.model_id, parsed.model_id);
    assert_eq!(
        cached_facts.package_facts_contract_version,
        PACKAGE_FACTS_CONTRACT_VERSION
    );
}

#[test]
fn invalid_cached_package_facts_fixture_matches_recovery_contract() {
    let (_raw, parsed) = load_cache_fixture("invalid_cached_package_facts.json");

    assert_eq!(parsed.model_id, "llm/example/invalid-cache");
    assert_eq!(parsed.cache_scope, ModelPackageFactsCacheScope::Detail);
    assert_eq!(
        parsed.package_facts_contract_version,
        i64::from(PACKAGE_FACTS_CONTRACT_VERSION)
    );
    assert_eq!(parsed.source_fingerprint, "fresh-source-fingerprint");

    let cached_value: Value =
        serde_json::from_str(&parsed.facts_json).expect("facts_json should remain valid JSON");
    assert_eq!(
        cached_value.get("not").and_then(Value::as_str),
        Some("resolved_model_package_facts")
    );
    assert!(
        serde_json::from_str::<ResolvedModelPackageFacts>(&parsed.facts_json).is_err(),
        "fixture must model malformed cached detail that recovery bypasses"
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
