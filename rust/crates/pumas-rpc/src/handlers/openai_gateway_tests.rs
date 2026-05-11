use super::*;
use pumas_library::models::{
    RuntimeDeviceMode, RuntimeProfileId, RuntimeProviderId, ServedModelLoadState,
    ServingEndpointStatus, ServingStatusSnapshot,
};
use pumas_library::ProviderBehavior;

fn loaded_status(model_id: &str, profile_id: &str, model_alias: Option<&str>) -> ServedModelStatus {
    ServedModelStatus {
        model_id: model_id.to_string(),
        model_alias: model_alias.map(str::to_string),
        provider: RuntimeProviderId::LlamaCpp,
        profile_id: RuntimeProfileId::parse(profile_id).unwrap(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: RuntimeDeviceMode::Auto,
        device_id: None,
        gpu_layers: None,
        tensor_split: None,
        context_size: None,
        keep_loaded: true,
        endpoint_url: None,
        memory_bytes: None,
        loaded_at: None,
        last_error: None,
    }
}

fn snapshot(served_models: Vec<ServedModelStatus>) -> ServingStatusSnapshot {
    ServingStatusSnapshot {
        schema_version: 1,
        cursor: "serving:1".to_string(),
        endpoint: ServingEndpointStatus::not_configured(),
        served_models,
        last_errors: Vec::new(),
    }
}

#[test]
fn openai_lookup_routes_unique_alias_before_base_model_id() {
    let result = resolve_openai_served_model(
        snapshot(vec![
            loaded_status("models/example", "llama-cpu", Some("example-cpu")),
            loaded_status("models/example", "llama-gpu", Some("example-gpu")),
        ]),
        "example-gpu",
    );

    match result {
        OpenAiServedModelLookup::Found(status) => {
            assert_eq!(status.profile_id.as_str(), "llama-gpu");
        }
        other => panic!("expected a routed model, got {other:?}"),
    }
}

#[test]
fn openai_lookup_rejects_ambiguous_base_model_id() {
    let result = resolve_openai_served_model(
        snapshot(vec![
            loaded_status("models/example", "llama-cpu", Some("example-cpu")),
            loaded_status("models/example", "llama-gpu", Some("example-gpu")),
        ]),
        "models/example",
    );

    match result {
        OpenAiServedModelLookup::Ambiguous { code, message } => {
            assert_eq!(code, ModelServeErrorCode::AmbiguousModelRouting);
            assert!(message.contains("multiple served instances"));
        }
        other => panic!("expected ambiguous routing, got {other:?}"),
    }
}

#[test]
fn openai_lookup_rejects_duplicate_aliases() {
    let result = resolve_openai_served_model(
        snapshot(vec![
            loaded_status("models/one", "llama-cpu", Some("shared")),
            loaded_status("models/two", "llama-gpu", Some("shared")),
        ]),
        "shared",
    );

    match result {
        OpenAiServedModelLookup::Ambiguous { code, message } => {
            assert_eq!(code, ModelServeErrorCode::DuplicateModelAlias);
            assert!(message.contains("multiple served instances"));
        }
        other => panic!("expected duplicate alias ambiguity, got {other:?}"),
    }
}

#[test]
fn provider_request_model_id_keeps_llama_cpp_catalog_id() {
    let registry = ProviderRegistry::builtin();
    let mut llama = loaded_status("models/example", "llama-gpu", Some("example-gpu"));
    llama.provider = RuntimeProviderId::LlamaCpp;
    assert_eq!(
        provider_request_model_id(&llama, &registry),
        "models/example"
    );

    let mut ollama = loaded_status("models/example", "ollama-default", Some("example-gpu"));
    ollama.provider = RuntimeProviderId::Ollama;
    assert_eq!(provider_request_model_id(&ollama, &registry), "example-gpu");
}

#[test]
fn openai_gateway_policy_for_path_maps_proxy_routes() {
    assert_eq!(
        openai_gateway_policy_for_path("/v1/chat/completions").map(|policy| policy.endpoint),
        Some(OpenAiGatewayEndpoint::ChatCompletions)
    );
    assert_eq!(
        openai_gateway_policy_for_path("/v1/completions").map(|policy| policy.endpoint),
        Some(OpenAiGatewayEndpoint::Completions)
    );
    assert_eq!(
        openai_gateway_policy_for_path("/v1/embeddings").map(|policy| policy.endpoint),
        Some(OpenAiGatewayEndpoint::Embeddings)
    );
    assert_eq!(openai_gateway_policy_for_path("/v1/audio"), None);
}

#[test]
fn openai_gateway_policy_for_path_has_explicit_limits() {
    let embeddings = openai_gateway_policy_for_path("/v1/embeddings").unwrap();
    assert_eq!(
        embeddings.max_request_body_bytes,
        OPENAI_EMBEDDINGS_BODY_BYTES
    );
    assert_eq!(embeddings.request_timeout, OPENAI_GATEWAY_REQUEST_TIMEOUT);
}

#[test]
fn provider_endpoint_capability_comes_from_registry_behavior() {
    let mut behavior = ProviderBehavior::ollama();
    behavior.openai_endpoints = vec![
        OpenAiGatewayEndpoint::Models,
        OpenAiGatewayEndpoint::Embeddings,
    ];
    let registry = ProviderRegistry::from_behaviors([behavior]);

    assert!(provider_supports_openai_gateway_endpoint(
        RuntimeProviderId::Ollama,
        OpenAiGatewayEndpoint::Embeddings,
        &registry
    ));
    assert!(!provider_supports_openai_gateway_endpoint(
        RuntimeProviderId::Ollama,
        OpenAiGatewayEndpoint::ChatCompletions,
        &registry
    ));
}
