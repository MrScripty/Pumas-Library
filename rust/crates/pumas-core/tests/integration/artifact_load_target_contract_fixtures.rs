use pumas_library::models::{
    AssetValidationState, ModelArtifactState, ModelEntryPathState, PackageArtifactKind,
    PumasArtifactLoadPathKind, PumasArtifactLoadTargetDiagnosticCode,
    PumasArtifactLoadTargetResolutionMode, ResolveModelArtifactLoadTargetRequest,
    ResolveModelArtifactLoadTargetResponse, StorageKind,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("artifact_load_targets")
        .join(name)
}

fn load_request_fixture(name: &str) -> (Value, ResolveModelArtifactLoadTargetRequest) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: ResolveModelArtifactLoadTargetRequest =
        serde_json::from_str(&content).expect("fixture should match request contract");
    (raw, parsed)
}

fn load_response_fixture(name: &str) -> (Value, ResolveModelArtifactLoadTargetResponse) {
    let path = fixture_path(name);
    let content = fs::read_to_string(&path).expect("fixture should be readable");
    let raw: Value = serde_json::from_str(&content).expect("fixture should be valid json");
    let parsed: ResolveModelArtifactLoadTargetResponse =
        serde_json::from_str(&content).expect("fixture should match response contract");
    (raw, parsed)
}

#[test]
fn diffusers_owner_fresh_request_fixture_matches_contract() {
    let (raw, parsed) = load_request_fixture("diffusers_owner_fresh_request.json");

    assert_eq!(parsed.model_ref.model_id, "image/stable-diffusion/tiny-sd");
    assert_eq!(
        parsed.model_ref.selected_artifact_id.as_deref(),
        Some("tiny-sd")
    );
    assert_eq!(
        parsed.expected_artifact_kind,
        Some(PackageArtifactKind::DiffusersBundle)
    );
    assert_eq!(
        parsed.resolution_mode,
        PumasArtifactLoadTargetResolutionMode::OwnerFresh
    );
    assert_eq!(parsed.consumer.consumer_name, "pantograph");
    assert_eq!(
        parsed.consumer.task_kind.as_deref(),
        Some("image_generation")
    );
    assert_eq!(
        parsed.consumer.runtime_family.as_deref(),
        Some("pytorch.diffusers")
    );
    assert!(
        raw.get("selected_artifact_id").is_none(),
        "selected artifact identity must stay inside model_ref"
    );
}

#[test]
fn diffusers_ready_response_fixture_matches_contract() {
    let (raw, parsed) = load_response_fixture("diffusers_ready_response.json");

    assert_eq!(parsed.artifact_state, ModelArtifactState::Ready);
    assert_eq!(parsed.entry_path_state, ModelEntryPathState::Ready);
    assert!(parsed.is_ready());

    let target = parsed.target.expect("ready fixture should include target");
    assert_eq!(target.artifact_kind, PackageArtifactKind::DiffusersBundle);
    assert_eq!(target.load_path_kind, PumasArtifactLoadPathKind::Directory);
    assert_eq!(target.storage_kind, StorageKind::LibraryOwned);
    assert_eq!(target.validation_state, AssetValidationState::Valid);
    assert_eq!(target.package_facts_contract_version, Some(2));
    assert!(target.content_fingerprint.is_none());

    let target_json = raw
        .get("target")
        .and_then(Value::as_object)
        .expect("target should be an object");
    assert!(
        target_json.get("content_fingerprint").is_none(),
        "content_fingerprint is optional and must not be required on ready targets"
    );
}

#[test]
fn read_only_owner_fresh_rejection_fixture_matches_contract() {
    let (_raw, parsed) = load_response_fixture("read_only_owner_fresh_rejected_response.json");

    assert_eq!(parsed.artifact_state, ModelArtifactState::Stale);
    assert_eq!(parsed.entry_path_state, ModelEntryPathState::Stale);
    assert!(!parsed.is_ready());
    assert!(parsed.target.is_none());
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(
        parsed.diagnostics[0].code,
        PumasArtifactLoadTargetDiagnosticCode::ModeNotAllowed
    );
    assert_eq!(
        parsed.diagnostics[0].field_path.as_deref(),
        Some("resolution_mode")
    );
}
