use super::*;
use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
use crate::server::AppState;
use axum::body::{to_bytes, Bytes};
use axum::extract::{OriginalUri, State};
use axum::http::StatusCode;
use pumas_app_manager::SizeCalculator;
use pumas_library::models::{
    RuntimeDeviceMode, RuntimeEndpointUrl, RuntimeProfileId, RuntimeProviderId,
    ServedModelLoadState, ServingEndpointStatus, ServingStatusSnapshot,
};
use pumas_library::{
    OnnxEmbeddingBackendKind, OnnxLoadOptions, OnnxLoadRequest, OnnxSessionManager, PluginLoader,
    ProviderBehavior,
};
use serde_json::{json, Value};
use std::time::Duration;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify, RwLock};

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
        router_profiles: Vec::new(),
        last_errors: Vec::new(),
    }
}

async fn gateway_test_state() -> (TempDir, Arc<AppState>) {
    gateway_test_state_with_onnx_backend(OnnxEmbeddingBackendKind::fake()).await
}

async fn gateway_test_state_with_onnx_backend(
    onnx_backend: OnnxEmbeddingBackendKind,
) -> (TempDir, Arc<AppState>) {
    gateway_test_state_with_clients(onnx_backend, reqwest::Client::new()).await
}

async fn gateway_test_state_with_clients(
    onnx_backend: OnnxEmbeddingBackendKind,
    gateway_http_client: reqwest::Client,
) -> (TempDir, Arc<AppState>) {
    let temp_dir = TempDir::new().unwrap();
    let launcher_root = temp_dir.path().to_path_buf();
    let api = crate::handlers::test_support::build_test_api(&launcher_root).await;
    let plugin_loader = PluginLoader::new_async(launcher_root.join("launcher-data/plugins"))
        .await
        .unwrap();
    let onnx_session_manager = OnnxSessionManager::new(onnx_backend, 2).unwrap();
    let state = Arc::new(AppState {
        catalog_projection: crate::catalog_projection::CatalogProjection::unavailable(),
        api,
        version_managers: Arc::new(RwLock::new(HashMap::new())),
        size_calculator: Arc::new(Mutex::new(
            SizeCalculator::new_with_cache(launcher_root.join("launcher-data/cache")).await,
        )),
        plugin_loader: Arc::new(plugin_loader),
        gateway_http_client,
        gateway_base_url: pumas_library::models::RuntimeEndpointUrl::parse(
            "http://127.0.0.1:3456/v1",
        )
        .unwrap(),
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

async fn record_llama_served_model(state: &AppState, endpoint_url: &str) {
    state
        .api
        .record_served_model(ServedModelStatus {
            model_id: "models/llama".to_string(),
            model_alias: Some("llama".to_string()),
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse("llama-cpu").unwrap(),
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: Some(8),
            keep_loaded: true,
            endpoint_url: Some(RuntimeEndpointUrl::parse(endpoint_url).unwrap()),
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        })
        .await
        .unwrap();
}

async fn spawn_gateway_response_server(status: StatusCode, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request_bytes = [0_u8; 1024];
        let _ = socket.read(&mut request_bytes).await;
        let reason = status.canonical_reason().unwrap_or("gateway response");
        let response = format!(
            "HTTP/1.1 {} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            status.as_u16(),
            reason,
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    format!("http://{addr}")
}

async fn spawn_hanging_gateway_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (_socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(300)).await;
    });
    format!("http://{addr}")
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
    assert!(!embeddings.generation);
    assert_eq!(
        embeddings.request_timeout,
        Some(OPENAI_GATEWAY_REQUEST_TIMEOUT)
    );
}

#[test]
fn generation_policy_is_duration_unbounded_for_all_generation_routes() {
    // Every registered generation route uses the shared duration-unbounded
    // transport: no client-wide, per-request total, response-read, or idle
    // deadline. Non-generation routes keep their independently bounded
    // budgets.
    for path in [
        "/v1/chat/completions",
        "/v1/completions",
        "/v1/images/generations",
    ] {
        let policy = openai_gateway_policy_for_path(path).unwrap();
        assert!(policy.generation, "{path}");
        assert_eq!(policy.request_timeout, None, "{path}");
    }
    for path in ["/v1/models", "/v1/embeddings"] {
        let policy = openai_gateway_policy_for_path(path).unwrap();
        assert!(!policy.generation, "{path}");
        assert_eq!(
            policy.request_timeout,
            Some(OPENAI_GATEWAY_REQUEST_TIMEOUT),
            "{path}"
        );
    }
    // The generation seam itself is connect-bounded only.
    assert_eq!(
        GENERATION_CONNECT_TIMEOUT,
        std::time::Duration::from_secs(10)
    );
    generation_http_client();
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
async fn openai_proxy_rejects_malformed_json_before_provider_dispatch() {
    let (_temp_dir, state) = gateway_test_state().await;

    let (status, body) = openai_proxy_bytes(
        state,
        "/v1/embeddings",
        Bytes::from_static(b"{\"model\":\"nomic\","),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("valid JSON")));
}

#[tokio::test]
async fn openai_proxy_redacts_provider_error_body_and_preserves_status() {
    let endpoint = spawn_gateway_response_server(
        StatusCode::SERVICE_UNAVAILABLE,
        r#"{"error":{"message":"credential hf_test_provider_secret failed for https://example.invalid/private-provider-url","type":"provider_error"}}"#,
    )
    .await;
    let (_temp_dir, state) = gateway_test_state().await;
    record_llama_served_model(&state, endpoint.as_str()).await;

    let (status, body) = openai_proxy_json(
        state,
        "/v1/embeddings",
        json!({
            "model": "llama",
            "input": "hello"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body.pointer("/error/message").and_then(Value::as_str),
        Some("A required operation is currently unavailable.")
    );
    assert_eq!(
        body.pointer("/error/type").and_then(Value::as_str),
        Some("pumas_error")
    );
    assert_eq!(
        body.pointer("/error/class").and_then(Value::as_str),
        Some("unavailable")
    );
    let encoded = body.to_string();
    assert!(!encoded.contains("hf_test_provider_secret"));
    assert!(!encoded.contains("private-provider-url"));
}

#[tokio::test(start_paused = true)]
async fn openai_proxy_maps_provider_timeout_to_bounded_gateway_error() {
    let endpoint = spawn_hanging_gateway_server().await;
    let (_temp_dir, state) = gateway_test_state().await;
    record_llama_served_model(&state, endpoint.as_str()).await;

    let request = tokio::spawn(openai_proxy_json(
        state,
        "/v1/embeddings",
        json!({
            "model": "llama",
            "input": "hello"
        }),
    ));
    tokio::task::yield_now().await;
    tokio::time::advance(OPENAI_GATEWAY_REQUEST_TIMEOUT + Duration::from_secs(1)).await;
    let (status, body) = request.await.unwrap();

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        body.pointer("/error/type").and_then(Value::as_str),
        Some("pumas_error")
    );
    assert!(body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .is_some_and(|message| !message.is_empty()));
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

#[tokio::test]
async fn router_discovery_is_unavailable_while_independent_current_target_can_route() {
    use pumas_library::models::{
        RouterCatalogState, RouterObservationState, RouterProfileSyncStatus,
    };
    let mut value = snapshot(vec![
        loaded_status("stale", "router", Some("stale-alias")),
        loaded_status("healthy", "other", None),
    ]);
    value.router_profiles.push(RouterProfileSyncStatus {
        profile_id: RuntimeProfileId::parse("router").unwrap(),
        generation: 1,
        observation_state: RouterObservationState::Unavailable,
        catalog_state: RouterCatalogState::Current,
        pending_model_ids: vec![],
        last_error: Some("synthetic disconnect".into()),
    });
    let response = openai_models_snapshot_response(value.clone());
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(body["error"]["code"], "endpoint_unavailable");
    assert_eq!(
        resolve_openai_served_model(value.clone(), "stale-alias"),
        OpenAiServedModelLookup::Unavailable
    );
    assert!(matches!(
        resolve_openai_served_model(value, "healthy"),
        OpenAiServedModelLookup::Found(_)
    ));
}

#[test]
fn uncertain_catalog_with_current_observation_cannot_discover_or_route() {
    use pumas_library::models::{
        RouterCatalogState, RouterObservationState, RouterProfileSyncStatus,
    };
    let mut value = snapshot(vec![loaded_status(
        "synthetic-model",
        "router",
        Some("synthetic-alias"),
    )]);
    value.router_profiles.push(RouterProfileSyncStatus {
        profile_id: RuntimeProfileId::parse("router").unwrap(),
        generation: 1,
        observation_state: RouterObservationState::Current,
        catalog_state: RouterCatalogState::Uncertain,
        pending_model_ids: Vec::new(),
        last_error: None,
    });
    assert_eq!(
        openai_models_snapshot_response(value.clone()).status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        resolve_openai_served_model(value, "synthetic-alias"),
        OpenAiServedModelLookup::Unavailable
    );
}

#[test]
fn image_capability_is_additive_and_excludes_text_models() {
    let text = loaded_status("text", "text-profile", None);
    assert!(openai_model_entry(text.clone())
        .get("capabilities")
        .is_none());
    let mut image = text;
    image.provider = RuntimeProviderId::Torch;
    assert_eq!(
        openai_model_entry(image)["capabilities"],
        json!(["image_generation"])
    );
}

/// Read one HTTP request (headers plus `Content-Length` body) from a test
/// backend socket and return the request head plus body as text.
async fn read_test_backend_request(socket: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert_ne!(n, 0, "test backend connection closed before request");
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(end) = buffer
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .map(|index| index + 4)
        {
            let head = String::from_utf8_lossy(&buffer[..end]).into_owned();
            let content_length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    (name.trim().eq_ignore_ascii_case("content-length"))
                        .then(|| value.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            while buffer.len() < end + content_length {
                let n = socket.read(&mut chunk).await.unwrap();
                assert_ne!(n, 0, "test backend connection closed before body");
                buffer.extend_from_slice(&chunk[..n]);
            }
            return String::from_utf8_lossy(&buffer[..end + content_length]).into_owned();
        }
        assert!(buffer.len() <= 1_048_576, "test request headers too large");
    }
}

async fn write_test_backend_response(socket: &mut TcpStream, status: StatusCode, body: &str) {
    let reason = status.canonical_reason().unwrap_or("test response");
    let response = format!(
        "HTTP/1.1 {} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        status.as_u16(),
        reason,
        body.len(),
        body
    );
    socket.write_all(response.as_bytes()).await.unwrap();
}

async fn record_torch_image_model(state: &AppState, endpoint_url: &str) {
    let mut model = loaded_status("image", "torch-profile", None);
    model.provider = RuntimeProviderId::Torch;
    model.endpoint_url = Some(RuntimeEndpointUrl::parse(endpoint_url).unwrap());
    state.api.record_served_model(model).await.unwrap();
}

const TORCH_PROTOCOL_3_HANDSHAKE: &str =
    r#"{"status":"ok","protocol":3,"capabilities":["image_generation"]}"#;
const TORCH_READY_IMAGE_SLOTS: &str = r#"{"slots":[{"slot_id":"image","model_name":"image","model_path":"/fixture","device":"cpu","state":"ready","model_type":"nunchaku-z-image-turbo"}]}"#;

/// A Torch sidecar stub: answers `/health` with the given handshake body and
/// holds `/api/images/generate` open until the gateway disconnects, counting
/// generation admissions. Unknown paths close without a response.
async fn spawn_torch_stub(
    handshake_body: &'static str,
    generate_hits: Arc<std::sync::atomic::AtomicUsize>,
) -> String {
    spawn_torch_stub_inner(handshake_body, generate_hits, None).await
}

async fn spawn_torch_stub_inner(
    handshake_body: &'static str,
    generate_hits: Arc<std::sync::atomic::AtomicUsize>,
    mut disconnected_tx: Option<tokio::sync::oneshot::Sender<()>>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(_) => break,
            };
            let request = read_test_backend_request(&mut socket).await;
            if request.starts_with("GET /health") {
                write_test_backend_response(&mut socket, StatusCode::OK, handshake_body).await;
            } else if request.starts_with("GET /api/slots") {
                write_test_backend_response(&mut socket, StatusCode::OK, TORCH_READY_IMAGE_SLOTS)
                    .await;
            } else if request.starts_with("POST /api/images/generate") {
                generate_hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // Hold the admitted request: dropping the gateway future must
                // close this socket, which the test observes as EOF.
                let mut chunk = [0u8; 1024];
                loop {
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                if let Some(tx) = disconnected_tx.take() {
                    let _ = tx.send(());
                }
            }
        }
    });
    endpoint
}

#[tokio::test(start_paused = true)]
async fn generation_transport_survives_silence_beyond_former_policy() {
    // Chat and completions share the generic buffered gateway handler, client
    // seam, and response function, so one silent request per registered route
    // proves the shared construction. Silence past the former 120-second
    // policy is not a failure on either route.
    for (path, body) in [
        (
            "/v1/chat/completions",
            json!({"model": "llama", "messages": [{"role": "user", "content": "hi"}]}),
        ),
        ("/v1/completions", json!({"model": "llama", "prompt": "hi"})),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let _request = read_test_backend_request(&mut socket).await;
            tokio::time::sleep(Duration::from_secs(300)).await;
            write_test_backend_response(
                &mut socket,
                StatusCode::OK,
                r#"{"id":"gen-1","choices":[]}"#,
            )
            .await;
        });
        let (_temp_dir, state) = gateway_test_state().await;
        record_llama_served_model(&state, &format!("http://{addr}")).await;
        let request = tokio::spawn(openai_proxy_json(state, path, body));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(295)).await;
        // Still connected and silent well past the former 120-second policy:
        // no total, read, idle, or elapsed deadline has fired.
        tokio::pin!(request);
        assert!(futures::poll!(&mut request).is_pending(), "{path}");
        tokio::time::advance(Duration::from_secs(5)).await;
        let (status, _) = request.await.unwrap();
        assert_eq!(status, StatusCode::OK, "{path}");
        server.await.unwrap();
    }
}

#[tokio::test]
async fn generation_disconnect_drops_pumas_request_without_replay() {
    // Dropping the Pumas gateway future closes the provider transport: the
    // test observes the backend EOF and exactly one provider hit. This proves
    // the Pumas-boundary cancellation signal only; it claims nothing about
    // external Ollama or llama.cpp worker cleanup, which stays with each
    // provider lifecycle owner.
    for path in ["/v1/chat/completions", "/v1/completions"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let backend = {
            let hits = hits.clone();
            tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let _request = read_test_backend_request(&mut socket).await;
                hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                started_tx.send(()).unwrap();
                let mut chunk = [0u8; 1024];
                loop {
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
            })
        };
        let (_temp_dir, state) = gateway_test_state().await;
        record_llama_served_model(&state, &format!("http://{addr}")).await;
        let body = if path.ends_with("chat/completions") {
            json!({"model": "llama", "messages": [{"role": "user", "content": "hi"}]})
        } else {
            json!({"model": "llama", "prompt": "hi"})
        };
        let request = tokio::spawn(handle_openai_proxy(
            State(state),
            OriginalUri(path.parse().unwrap()),
            Bytes::from(body.to_string()),
        ));
        tokio::time::timeout(Duration::from_secs(5), started_rx)
            .await
            .unwrap()
            .unwrap();
        request.abort();
        let _ = request.await;
        tokio::time::timeout(Duration::from_secs(5), backend)
            .await
            .expect("gateway retained the provider request after Pumas disconnect")
            .unwrap();
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "{path}: gateway replayed the provider request"
        );
    }
}

#[tokio::test]
async fn lost_generation_response_is_uncertain_and_not_replayed() {
    // The provider accepts the request and then the transport dies before any
    // terminal result. The gateway reports unavailability exactly once: the
    // outcome stays unknown (neither success nor proven cancellation) and the
    // request is never replayed.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    tokio::spawn({
        let hits = hits.clone();
        async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = read_test_backend_request(&mut socket).await;
                // Close without responding: a lost terminal result.
            }
        }
    });
    let (_temp_dir, state) = gateway_test_state().await;
    record_llama_served_model(&state, &format!("http://{addr}")).await;
    let (status, body) = openai_proxy_json(
        state,
        "/v1/chat/completions",
        json!({"model": "llama", "messages": [{"role": "user", "content": "hi"}]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        body.pointer("/error/type").and_then(Value::as_str),
        Some("pumas_error")
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "gateway replayed a request whose outcome is unknown"
    );
}

#[tokio::test]
async fn image_admission_rejects_incompatible_sidecar_without_generation() {
    // A protocol-2 sidecar speaks the old lifetime: admission rechecks the
    // live handshake and never reaches generation. A sidecar without the
    // image capability is rejected as unsupported instead.
    for (handshake_body, expected_status, expected_code) in [
        (
            r#"{"status":"ok","protocol":2,"capabilities":["image_generation"]}"#,
            StatusCode::BAD_GATEWAY,
            "backend_failure",
        ),
        (
            r#"{"status":"ok","protocol":3,"capabilities":[]}"#,
            StatusCode::BAD_REQUEST,
            "unsupported_model",
        ),
    ] {
        let generate_hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let endpoint = spawn_torch_stub(handshake_body, generate_hits.clone()).await;
        let (_temp_dir, state) = gateway_test_state().await;
        record_torch_image_model(&state, endpoint.as_str()).await;
        let (status, body) = openai_proxy_json(
            state,
            "/v1/images/generations",
            json!({"model": "image", "prompt": "a bird", "width": 512, "height": 512}),
        )
        .await;
        assert_eq!(status, expected_status, "{handshake_body}");
        assert_eq!(
            body.pointer("/error/code").and_then(Value::as_str),
            Some(expected_code),
            "{handshake_body}"
        );
        assert_eq!(
            generate_hits.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "{handshake_body}: incompatible sidecar reached generation"
        );
    }
}

/// Minimal PNG bytes carrying the given `IHDR` dimensions for contract tests.
fn test_png_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&width.to_be_bytes());
    png.extend_from_slice(&height.to_be_bytes());
    png.extend_from_slice(&[8, 2, 0, 0, 0]);
    png
}

fn test_base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in data.chunks(3) {
        let mut triple = 0u32;
        for &byte in chunk {
            triple = (triple << 8) | byte as u32;
        }
        triple <<= 8 * (3 - chunk.len());
        let quads = [18, 12, 6, 0].map(|shift| ALPHABET[((triple >> shift) & 63) as usize]);
        let pad = 3 - chunk.len();
        for (index, &quad) in quads.iter().enumerate() {
            output.push(if index >= 4 - pad { '=' } else { quad as char });
        }
    }
    output
}

#[tokio::test]
async fn image_transport_survives_silence_beyond_former_policy() {
    // The image route uses the same duration-unbounded generation policy as
    // text: a connected, silent sidecar past the former 600-second class of
    // deadline still completes. Controlled time replaces the wall-clock wait.
    let png = test_base64(&test_png_bytes(512, 512));
    let result_body = json!({
        "png_base64": png.clone(),
        "seed": 7,
        "steps": 8,
        "guidance": 0.0,
        "memory_policy": "sequential_cpu_offload",
        "duration_seconds": 1.5,
        "peak_vram_bytes": 123456,
    })
    .to_string();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let admitted = Arc::new(Notify::new());
    let backend_admitted = admitted.clone();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_test_backend_request(&mut socket).await;
            if request.starts_with("GET /health") {
                write_test_backend_response(
                    &mut socket,
                    StatusCode::OK,
                    TORCH_PROTOCOL_3_HANDSHAKE,
                )
                .await;
            } else if request.starts_with("GET /api/slots") {
                write_test_backend_response(&mut socket, StatusCode::OK, TORCH_READY_IMAGE_SLOTS)
                    .await;
            } else if request.starts_with("POST /api/images/generate") {
                backend_admitted.notify_one();
                tokio::time::sleep(Duration::from_secs(300)).await;
                write_test_backend_response(&mut socket, StatusCode::OK, &result_body).await;
            }
        }
    });
    let (_temp_dir, state) = gateway_test_state().await;
    record_torch_image_model(&state, endpoint.as_str()).await;
    let mut request = tokio::spawn(openai_proxy_json(
        state,
        "/v1/images/generations",
        json!({"model": "image", "prompt": "a bird", "width": 512, "height": 512}),
    ));
    tokio::select! {
        () = admitted.notified() => {}
        result = &mut request => panic!("image request ended before backend admission: {result:?}"),
    }
    // Pause only after the request is admitted. Pausing before admission can
    // auto-advance the deliberately bounded connection-establishment timer.
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(295)).await;
    // Still connected and silent: elapsed time alone never fails an admitted
    // generation.
    assert!(!request.is_finished());
    tokio::time::advance(Duration::from_secs(5)).await;
    let (status, body) = request.await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"][0]["b64_json"], png);
    assert_eq!(body["metadata"]["seed"], 7);
    assert!(body["metadata"].get("peak_vram_bytes").is_none());
}

#[tokio::test]
async fn image_rejects_oversized_whole_payload() {
    // The whole-payload bound applies to the entire received sidecar body:
    // a 13 MiB success body fails closed even though each required field is
    // valid on its own.
    let padding = "a".repeat(13 * 1024 * 1024);
    let oversized = json!({
        "png_base64": test_base64(&test_png_bytes(512, 512)),
        "seed": 7,
        "steps": 8,
        "guidance": 0.0,
        "memory_policy": "sequential_cpu_offload",
        "duration_seconds": 1.5,
        "padding": padding,
    })
    .to_string();
    assert!(oversized.len() > 12 * 1024 * 1024);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_test_backend_request(&mut socket).await;
            if request.starts_with("GET /health") {
                write_test_backend_response(
                    &mut socket,
                    StatusCode::OK,
                    TORCH_PROTOCOL_3_HANDSHAKE,
                )
                .await;
            } else if request.starts_with("GET /api/slots") {
                write_test_backend_response(&mut socket, StatusCode::OK, TORCH_READY_IMAGE_SLOTS)
                    .await;
            } else if request.starts_with("POST /api/images/generate") {
                write_test_backend_response(&mut socket, StatusCode::OK, &oversized).await;
            }
        }
    });
    let (_temp_dir, state) = gateway_test_state().await;
    record_torch_image_model(&state, endpoint.as_str()).await;
    let (status, body) = openai_proxy_json(
        state,
        "/v1/images/generations",
        json!({"model": "image", "prompt": "a bird", "width": 512, "height": 512}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        body.pointer("/error/code").and_then(Value::as_str),
        Some("invalid_backend_response")
    );
}

#[tokio::test]
async fn image_admission_reports_unavailable_sidecar_without_generation() {
    // A connection-establishment failure is an unavailable outcome, never
    // proof that admitted generation was cancelled or completed.
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let (_temp_dir, state) = gateway_test_state().await;
    record_torch_image_model(&state, endpoint.as_str()).await;
    let (status, body) = openai_proxy_json(
        state,
        "/v1/images/generations",
        json!({"model": "image", "prompt": "a bird", "width": 512, "height": 512}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        body.pointer("/error/code").and_then(Value::as_str),
        Some("backend_failure")
    );
}

#[tokio::test]
async fn image_admission_rejects_replaced_runtime_before_generation() {
    // The sidecar is replaced behind its profile while the gateway is
    // mid-admission: the live handshake succeeds, but the re-resolved
    // endpoint no longer matches the admitted one, so the stale observation
    // is not admitted and generation is never reached.
    let generate_hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let replacement_endpoint = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let (_temp_dir, state) = gateway_test_state().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let primary_endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn({
        let state = state.clone();
        let generate_hits = generate_hits.clone();
        async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = read_test_backend_request(&mut socket).await;
                if request.starts_with("GET /health") {
                    let mut replacement = loaded_status("image", "torch-profile", None);
                    replacement.provider = RuntimeProviderId::Torch;
                    replacement.endpoint_url =
                        Some(RuntimeEndpointUrl::parse(&replacement_endpoint).unwrap());
                    state.api.record_served_model(replacement).await.unwrap();
                    write_test_backend_response(
                        &mut socket,
                        StatusCode::OK,
                        TORCH_PROTOCOL_3_HANDSHAKE,
                    )
                    .await;
                } else if request.starts_with("GET /api/slots") {
                    write_test_backend_response(
                        &mut socket,
                        StatusCode::OK,
                        TORCH_READY_IMAGE_SLOTS,
                    )
                    .await;
                } else if request.starts_with("POST /api/images/generate") {
                    generate_hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    write_test_backend_response(
                        &mut socket,
                        StatusCode::CONFLICT,
                        r#"{"detail":{"code":"runtime_busy"}}"#,
                    )
                    .await;
                }
            }
        }
    });
    record_torch_image_model(&state, primary_endpoint.as_str()).await;
    let (status, body) = openai_proxy_json(
        state,
        "/v1/images/generations",
        json!({"model": "image", "prompt": "a bird", "width": 512, "height": 512}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body.pointer("/error/code").and_then(Value::as_str),
        Some("endpoint_unavailable")
    );
    assert_eq!(
        generate_hits.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "replaced sidecar reached generation"
    );
}

#[test]
fn image_admission_rejects_same_endpoint_process_replacement() {
    let endpoint = RuntimeEndpointUrl::parse("http://127.0.0.1:9000").unwrap();
    let before = OwnedRuntimeProfileObservation {
        generation: 1,
        pid: Some(100),
        state: RuntimeLifecycleState::Running,
        endpoint_url: endpoint.clone(),
        model_path: Some(PathBuf::from("/models/image")),
        context_size: None,
    };
    let mut replacement = before.clone();
    replacement.generation = 2;
    replacement.pid = Some(101);

    assert!(!torch_process_is_stable(
        Some(&before),
        Some(&replacement),
        &endpoint,
    ));
    assert!(torch_process_is_stable(
        Some(&before),
        Some(&before),
        &endpoint,
    ));
}

#[tokio::test]
async fn image_client_disconnect_closes_backend_request() {
    // Dropping the Pumas image future closes the sidecar request: the stub
    // observes EOF on the held generation socket with exactly one admission.
    // This proves Pumas-boundary disconnect propagation and retained admission
    // accounting; it claims nothing about sidecar worker cleanup, which stays
    // with the Torch execution owner.
    let generate_hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (disconnected_tx, disconnected_rx) = tokio::sync::oneshot::channel();
    let endpoint = spawn_torch_stub_inner(
        TORCH_PROTOCOL_3_HANDSHAKE,
        generate_hits.clone(),
        Some(disconnected_tx),
    )
    .await;
    let (_root, state) = gateway_test_state().await;
    record_torch_image_model(&state, endpoint.as_str()).await;
    let request = tokio::spawn(handle_openai_proxy(
        State(state),
        OriginalUri("/v1/images/generations".parse().unwrap()),
        Bytes::from(
            json!({"model": "image", "prompt": "a bird", "width": 512, "height": 512}).to_string(),
        ),
    ));
    tokio::time::timeout(Duration::from_secs(10), async {
        while generate_hits.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("gateway never admitted the image request");
    request.abort();
    let _ = request.await;
    tokio::time::timeout(Duration::from_secs(10), disconnected_rx)
        .await
        .expect("gateway retained the sidecar request after Pumas disconnect")
        .unwrap();
    assert_eq!(
        generate_hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "Gateway replayed the image request after client disconnect"
    );
}
