use pumas_library::models::{
    BackendHintLabel, PackageArtifactKind, PackageFactStatus, ProcessorComponentKind,
    ResolvedModelPackageFacts, PACKAGE_FACTS_CONTRACT_VERSION,
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
fn package_fact_status_defaults_to_uninspected() {
    assert_eq!(PackageFactStatus::default(), PackageFactStatus::Uninspected);
}
