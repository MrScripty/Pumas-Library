use pumas_library::models::PumasModelRef;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("model_refs")
        .join(name)
}

fn load_fixture(name: &str) -> (Value, PumasModelRef) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: PumasModelRef =
        serde_json::from_str(&content).expect("fixture should match model-ref contract");
    (raw, parsed)
}

#[test]
fn unresolved_legacy_path_fixture_preserves_graph_intent() {
    let (raw, parsed) = load_fixture("unresolved_legacy_path.json");

    assert_eq!(parsed.model_id, "");
    assert!(parsed.selected_artifact_id.is_none());
    assert!(parsed.selected_artifact_path.is_none());
    assert_eq!(parsed.migration_diagnostics.len(), 1);
    assert_eq!(
        parsed.migration_diagnostics[0].code,
        "legacy_path_unresolved"
    );
    assert_eq!(
        parsed.migration_diagnostics[0].input.as_deref(),
        Some("/legacy/graphs/missing-model.gguf")
    );
    assert!(
        raw.get("replacement_model_id").is_none(),
        "unresolved refs must not encode guessed replacements"
    );
}
