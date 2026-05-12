use pumas_library::models::{
    RuntimeDeviceMode, RuntimeLifecycleState, RuntimeProfilesSnapshot, RuntimeProviderId,
    RuntimeProviderMode, RUNTIME_PROFILES_SCHEMA_VERSION,
};
use pumas_library::{ProviderBehavior, RuntimeProviderCapabilities};

#[test]
fn onnx_runtime_snapshot_fixture_matches_runtime_profile_contract() {
    let fixture = include_str!("fixtures/runtime_profiles/onnx_runtime_snapshot.json");
    let snapshot: RuntimeProfilesSnapshot = serde_json::from_str(fixture).unwrap();

    assert_eq!(snapshot.schema_version, RUNTIME_PROFILES_SCHEMA_VERSION);
    assert_eq!(snapshot.profiles.len(), 1);
    assert_eq!(snapshot.routes.len(), 1);
    assert_eq!(snapshot.statuses.len(), 1);

    let profile = &snapshot.profiles[0];
    assert_eq!(profile.provider, RuntimeProviderId::OnnxRuntime);
    assert_eq!(profile.provider_mode, RuntimeProviderMode::OnnxServe);
    assert_eq!(profile.device.mode, RuntimeDeviceMode::Cpu);
    assert!(profile.endpoint_url.is_none());
    assert!(profile.port.is_none());

    let route = &snapshot.routes[0];
    assert_eq!(route.provider, RuntimeProviderId::OnnxRuntime);
    assert_eq!(route.profile_id, Some(profile.profile_id.clone()));
    assert!(route.auto_load);

    let status = &snapshot.statuses[0];
    assert_eq!(status.profile_id, profile.profile_id);
    assert_eq!(status.state, RuntimeLifecycleState::Stopped);
}

#[test]
fn onnx_runtime_provider_capabilities_fixture_shape_is_stable() {
    let capabilities =
        RuntimeProviderCapabilities::from_behavior(&ProviderBehavior::onnx_runtime());
    let encoded = serde_json::to_value(&capabilities).unwrap();

    assert_eq!(encoded["provider"], "onnx_runtime");
    assert_eq!(encoded["provider_modes"], serde_json::json!(["onnx_serve"]));
    assert_eq!(encoded["device_modes"], serde_json::json!(["auto", "cpu"]));
    assert_eq!(encoded["supports_managed_profiles"], true);
    assert_eq!(encoded["supports_external_profiles"], false);
    assert_eq!(encoded["supports_model_catalog"], true);
    assert_eq!(encoded["supports_dedicated_model_processes"], false);

    let decoded: RuntimeProviderCapabilities = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, capabilities);
}
