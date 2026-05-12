use super::*;
use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
use crate::server::AppState;
use axum::body::{to_bytes, Bytes};
use axum::extract::{OriginalUri, State};
use axum::http::StatusCode;
use pumas_app_manager::{CustomNodesManager, SizeCalculator};
use pumas_library::models::{
    RuntimeDeviceMode, RuntimeProfileId, RuntimeProviderId, ServedModelLoadState,
    ServingEndpointStatus, ServingStatusSnapshot,
};
use pumas_library::{
    OnnxEmbeddingBackendKind, OnnxLoadOptions, OnnxLoadRequest, OnnxSessionManager, PluginLoader,
    ProviderBehavior,
};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tempfile::TempDir;
use tokio::sync::{Mutex, RwLock};

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

async fn gateway_test_state() -> (TempDir, Arc<AppState>) {
    gateway_test_state_with_onnx_backend(OnnxEmbeddingBackendKind::fake()).await
}

async fn gateway_test_state_with_onnx_backend(
    onnx_backend: OnnxEmbeddingBackendKind,
) -> (TempDir, Arc<AppState>) {
    let temp_dir = TempDir::new().unwrap();
    let launcher_root = temp_dir.path().to_path_buf();
    let api = crate::handlers::test_support::build_test_api(&launcher_root).await;
    let plugin_loader = PluginLoader::new_async(launcher_root.join("launcher-data/plugins"))
        .await
        .unwrap();
    let onnx_session_manager = OnnxSessionManager::new(onnx_backend, 2).unwrap();
    let state = Arc::new(AppState {
        api,
        version_managers: Arc::new(RwLock::new(HashMap::new())),
        custom_nodes_manager: Arc::new(CustomNodesManager::new(
            launcher_root.join("comfyui-versions"),
        )),
        size_calculator: Arc::new(Mutex::new(
            SizeCalculator::new_with_cache(launcher_root.join("launcher-data/cache")).await,
        )),
        shortcut_manager: Arc::new(RwLock::new(None)),
        plugin_loader: Arc::new(plugin_loader),
        gateway_http_client: reqwest::Client::new(),
        provider_registry: ProviderRegistry::builtin(),
        llama_cpp_router_client: LlamaCppRouterClient::new(reqwest::Client::new()),
        ollama_client_factory: OllamaClientFactory::new(
            pumas_app_manager::OllamaHttpClients::new().unwrap(),
        ),
        onnx_session_manager,
    });
    (temp_dir, state)
}

fn optional_real_fixture_load_request(model_id: &str) -> Option<OnnxLoadRequest> {
    let root = std::env::var_os("PUMAS_ONNX_REAL_MODEL_ROOT")?;
    let model_path = std::env::var_os("PUMAS_ONNX_REAL_MODEL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("onnx/model_fp16.onnx"));
    Some(
        OnnxLoadRequest::parse(
            PathBuf::from(root),
            model_path,
            model_id,
            OnnxLoadOptions::default(),
        )
        .unwrap(),
    )
}

async fn record_onnx_served_model(state: &AppState) {
    state
        .api
        .record_served_model(ServedModelStatus {
            model_id: "embeddings/nomic".to_string(),
            model_alias: Some("nomic".to_string()),
            provider: RuntimeProviderId::OnnxRuntime,
            profile_id: RuntimeProfileId::parse("onnx-cpu").unwrap(),
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: Some(4),
            keep_loaded: true,
            endpoint_url: None,
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        })
        .await
        .unwrap();
}

async fn load_onnx_session(temp_dir: &TempDir, state: &AppState) {
    let model_root = temp_dir.path().join("onnx-fixture");
    std::fs::create_dir_all(&model_root).unwrap();
    std::fs::write(model_root.join("model.onnx"), b"fake").unwrap();
    let request = OnnxLoadRequest::parse(
        &model_root,
        "model.onnx",
        "embeddings/nomic",
        OnnxLoadOptions::cpu(4).unwrap(),
    )
    .unwrap();
    state.onnx_session_manager.load(request).await.unwrap();
}

async fn openai_proxy_json(state: Arc<AppState>, path: &str, body: Value) -> (StatusCode, Value) {
    let response = handle_openai_proxy(
        State(state),
        OriginalUri(path.parse().unwrap()),
        Bytes::from(body.to_string()),
    )
    .await;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn openai_proxy_bytes(state: Arc<AppState>, path: &str, body: Bytes) -> (StatusCode, Value) {
    let response =
        handle_openai_proxy(State(state), OriginalUri(path.parse().unwrap()), body).await;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
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

#[tokio::test]
async fn openai_proxy_routes_onnx_embeddings_in_process() {
    let (temp_dir, state) = gateway_test_state().await;
    load_onnx_session(&temp_dir, &state).await;
    record_onnx_served_model(&state).await;

    let (status, body) = openai_proxy_json(
        state,
        "/v1/embeddings",
        json!({
            "model": "nomic",
            "input": ["search_document: hello world"],
            "dimensions": 4
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.get("object").and_then(Value::as_str), Some("list"));
    assert_eq!(body.get("model").and_then(Value::as_str), Some("nomic"));
    assert_eq!(
        body.pointer("/data/0/object").and_then(Value::as_str),
        Some("embedding")
    );
    assert_eq!(
        body.pointer("/data/0/embedding")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(4)
    );
    assert_eq!(
        body.pointer("/usage/total_tokens").and_then(Value::as_u64),
        Some(3)
    );
}

#[tokio::test]
async fn openai_proxy_smokes_real_onnx_embedding_fixture() {
    let Some(load_request) = optional_real_fixture_load_request("embeddings/nomic") else {
        return;
    };
    let (_temp_dir, state) =
        gateway_test_state_with_onnx_backend(OnnxEmbeddingBackendKind::real()).await;
    state.onnx_session_manager.load(load_request).await.unwrap();
    record_onnx_served_model(&state).await;

    let (status, body) = openai_proxy_json(
        state,
        "/v1/embeddings",
        json!({
            "model": "nomic",
            "input": ["search_query: hello world"],
            "dimensions": 256
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.get("object").and_then(Value::as_str), Some("list"));
    assert_eq!(body.get("model").and_then(Value::as_str), Some("nomic"));
    let embedding = body
        .pointer("/data/0/embedding")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(embedding.len(), 256);
    assert!(embedding.iter().all(|value| {
        value
            .as_f64()
            .is_some_and(|component| component.is_finite())
    }));
    assert!(body
        .pointer("/usage/total_tokens")
        .and_then(Value::as_u64)
        .is_some_and(|tokens| tokens > 0));
}

#[tokio::test]
async fn openai_proxy_rejects_chat_for_onnx_embedding_provider() {
    let (_temp_dir, state) = gateway_test_state().await;
    record_onnx_served_model(&state).await;

    let (status, body) = openai_proxy_json(
        state,
        "/v1/chat/completions",
        json!({
            "model": "nomic",
            "messages": []
        }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body.pointer("/error/code").and_then(Value::as_str),
        Some("endpoint_unavailable")
    );
}

#[tokio::test]
async fn openai_proxy_maps_onnx_not_loaded_to_openai_error() {
    let (_temp_dir, state) = gateway_test_state().await;
    record_onnx_served_model(&state).await;

    let (status, body) = openai_proxy_json(
        state,
        "/v1/embeddings",
        json!({
            "model": "nomic",
            "input": "search_document: hello"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body.pointer("/error/code").and_then(Value::as_str),
        Some("model_not_found")
    );
}

#[tokio::test]
async fn openai_proxy_rejects_oversized_embedding_body_before_json() {
    let (_temp_dir, state) = gateway_test_state().await;
    let body = Bytes::from(vec![b'x'; OPENAI_EMBEDDINGS_BODY_BYTES + 1]);

    let (status, body) = openai_proxy_bytes(state, "/v1/embeddings", body).await;

    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("request body exceeds")));
}

#[tokio::test]
async fn openai_proxy_rejects_unknown_embedding_model() {
    let (_temp_dir, state) = gateway_test_state().await;

    let (status, body) = openai_proxy_json(
        state,
        "/v1/embeddings",
        json!({
            "model": "missing",
            "input": "hello"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("model is not served")));
}

#[tokio::test]
async fn openai_proxy_rejects_ambiguous_embedding_alias() {
    let (_temp_dir, state) = gateway_test_state().await;
    record_onnx_served_model(&state).await;
    state
        .api
        .record_served_model(ServedModelStatus {
            model_id: "embeddings/other".to_string(),
            model_alias: Some("nomic".to_string()),
            provider: RuntimeProviderId::OnnxRuntime,
            profile_id: RuntimeProfileId::parse("onnx-cpu-alt").unwrap(),
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: Some(4),
            keep_loaded: true,
            endpoint_url: None,
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        })
        .await
        .unwrap();

    let (status, body) = openai_proxy_json(
        state,
        "/v1/embeddings",
        json!({
            "model": "nomic",
            "input": "hello"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body.pointer("/error/code").and_then(Value::as_str),
        Some("duplicate_model_alias")
    );
}
