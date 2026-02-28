use pumas_library::model_library::{
    DependencyValidationState, ModelDependencyRequirementsResolution, DEPENDENCY_CONTRACT_VERSION,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("dependency_requirements")
        .join(name)
}

fn load_fixture(name: &str) -> (Value, ModelDependencyRequirementsResolution) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: ModelDependencyRequirementsResolution =
        serde_json::from_str(&content).expect("fixture should match resolver contract type");
    (raw, parsed)
}

#[test]
fn resolved_fixture_matches_contract_and_includes_stable_audio_tools() {
    let (raw, parsed) = load_fixture("resolved.json");
    assert_eq!(
        parsed.dependency_contract_version,
        DEPENDENCY_CONTRACT_VERSION
    );
    assert_eq!(parsed.validation_state, DependencyValidationState::Resolved);
    assert_eq!(parsed.bindings.len(), 1);

    let has_stable_audio_tools = parsed.bindings[0]
        .requirements
        .iter()
        .any(|req| req.name == "stable-audio-tools" && req.exact_pin == "==1.0.0");
    assert!(has_stable_audio_tools);

    let torch_req = raw
        .get("bindings")
        .and_then(Value::as_array)
        .and_then(|bindings| bindings.first())
        .and_then(|binding| binding.get("requirements"))
        .and_then(Value::as_array)
        .and_then(|requirements| {
            requirements
                .iter()
                .find(|item| item.get("name").and_then(Value::as_str) == Some("torch"))
        })
        .expect("torch requirement should exist");

    assert!(
        torch_req.get("index_url").is_none(),
        "optional requirement fields should be omitted when absent"
    );
}

#[test]
fn unknown_profile_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("unknown_profile.json");
    assert_eq!(
        parsed.dependency_contract_version,
        DEPENDENCY_CONTRACT_VERSION
    );
    assert_eq!(
        parsed.validation_state,
        DependencyValidationState::UnknownProfile
    );
    assert!(parsed.bindings.is_empty());
    assert_eq!(parsed.validation_errors.len(), 1);
}

#[test]
fn invalid_profile_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("invalid_profile.json");
    assert_eq!(
        parsed.dependency_contract_version,
        DEPENDENCY_CONTRACT_VERSION
    );
    assert_eq!(
        parsed.validation_state,
        DependencyValidationState::InvalidProfile
    );
    assert_eq!(parsed.bindings.len(), 1);
    assert_eq!(parsed.bindings[0].env_id, None);
    assert!(parsed
        .validation_errors
        .iter()
        .any(|err| err.code == "unpinned_dependency"));
}

#[test]
fn profile_conflict_fixture_matches_contract() {
    let (_raw, parsed) = load_fixture("profile_conflict.json");
    assert_eq!(
        parsed.dependency_contract_version,
        DEPENDENCY_CONTRACT_VERSION
    );
    assert_eq!(
        parsed.validation_state,
        DependencyValidationState::ProfileConflict
    );
    assert_eq!(parsed.bindings.len(), 2);
    assert!(parsed
        .bindings
        .iter()
        .all(|binding| binding.validation_state == DependencyValidationState::ProfileConflict));
}
