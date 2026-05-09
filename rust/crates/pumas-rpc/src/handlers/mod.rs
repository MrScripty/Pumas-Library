//! JSON-RPC request handlers, split by domain.

mod conversion;
mod custom_nodes;
mod links;
mod models;
mod ollama;
mod plugins;
mod process;
mod runtime_profiles;
mod serving;
mod shared;
mod shortcuts;
mod status;
mod torch;
mod versions;

use crate::server::AppState;
use crate::wrapper::wrap_response;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::{
    stream::{self, BoxStream},
    StreamExt,
};
use pumas_library::models::{
    ModelDownloadUpdateNotification, ModelLibraryUpdateNotification, ModelServeErrorCode,
    RuntimeProfileUpdateFeed, ServedModelLoadState, ServedModelStatus, ServingStatusSnapshot,
    ServingStatusUpdateFeed, StatusTelemetryUpdateNotification,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, warn};

const MODEL_LIBRARY_UPDATE_STREAM_LIMIT: usize = 250;

pub(crate) use shared::{
    detect_sandbox_environment, extract_safetensors_header, get_bool_param, get_i64_param,
    get_str_param, get_version_manager, parse_params, path_exists, read_utf8_file,
    require_str_param, require_version_manager, sync_version_paths_to_process_manager,
    validate_existing_local_directory_path, validate_existing_local_file_path,
    validate_existing_local_path, validate_external_url, validate_local_write_target_path,
    validate_non_empty,
};

// ============================================================================
// JSON-RPC types
// ============================================================================

/// JSON-RPC 2.0 request structure.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 response structure.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 error structure.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

// ============================================================================
// HTTP endpoints
// ============================================================================

/// Health check endpoint.
pub async fn handle_health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

/// OpenAI-compatible served-model listing backed by Pumas serving status.
pub async fn handle_openai_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.api.get_serving_status().await {
        Ok(response) => {
            let mut served_models = response.snapshot.served_models;
            served_models.retain(|model| model.load_state == ServedModelLoadState::Loaded);
            served_models.sort_by(|left, right| {
                openai_model_id(left)
                    .cmp(openai_model_id(right))
                    .then_with(|| left.profile_id.as_str().cmp(right.profile_id.as_str()))
            });
            Json(json!({
                "object": "list",
                "data": served_models
                    .into_iter()
                    .map(openai_model_entry)
                    .collect::<Vec<_>>()
            }))
            .into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "message": error.to_string(),
                    "type": "pumas_error"
                }
            })),
        )
            .into_response(),
    }
}

/// OpenAI-compatible proxy for served models.
pub async fn handle_openai_proxy(
    State(state): State<Arc<AppState>>,
    path: axum::extract::OriginalUri,
    Json(mut body): Json<Value>,
) -> Response {
    let Some(requested_model) = body.get("model").and_then(Value::as_str) else {
        return openai_error_response(
            StatusCode::BAD_REQUEST,
            "request body must include a string model field",
        );
    };

    let served = match find_openai_served_model(&state, requested_model).await {
        Ok(OpenAiServedModelLookup::Found(model)) => model,
        Ok(OpenAiServedModelLookup::NotFound) => {
            return openai_error_response(
                StatusCode::NOT_FOUND,
                format!("model is not served: {requested_model}"),
            );
        }
        Ok(OpenAiServedModelLookup::Ambiguous { code, message }) => {
            return openai_error_response_with_code(StatusCode::CONFLICT, code, message);
        }
        Err(error) => {
            return openai_error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };

    let Some(endpoint) = served.endpoint_url.as_ref() else {
        return openai_error_response(
            StatusCode::BAD_GATEWAY,
            "served model does not have a provider endpoint",
        );
    };

    if let Some(alias) = served.model_alias.as_deref() {
        if let Some(object) = body.as_object_mut() {
            object.insert("model".to_string(), Value::String(alias.to_string()));
        }
    }

    let target_url = format!("{}{}", endpoint.as_str().trim_end_matches('/'), path.path());
    match reqwest::Client::new()
        .post(target_url)
        .json(&body)
        .send()
        .await
    {
        Ok(response) => proxy_response(response).await,
        Err(error) => openai_error_response(StatusCode::BAD_GATEWAY, error.to_string()),
    }
}

fn openai_model_entry(model: ServedModelStatus) -> Value {
    json!({
        "id": model.model_alias.unwrap_or(model.model_id),
        "object": "model",
        "created": 0,
        "owned_by": "pumas"
    })
}

fn openai_model_id(model: &ServedModelStatus) -> &str {
    model
        .model_alias
        .as_deref()
        .unwrap_or(model.model_id.as_str())
}

#[derive(Debug, Clone, PartialEq)]
enum OpenAiServedModelLookup {
    Found(ServedModelStatus),
    NotFound,
    Ambiguous {
        code: ModelServeErrorCode,
        message: String,
    },
}

async fn find_openai_served_model(
    state: &AppState,
    requested_model: &str,
) -> pumas_library::Result<OpenAiServedModelLookup> {
    let snapshot = state.api.get_serving_status().await?.snapshot;
    Ok(resolve_openai_served_model(snapshot, requested_model))
}

fn resolve_openai_served_model(
    snapshot: ServingStatusSnapshot,
    requested_model: &str,
) -> OpenAiServedModelLookup {
    let loaded: Vec<ServedModelStatus> = snapshot
        .served_models
        .into_iter()
        .filter(|model| model.load_state == ServedModelLoadState::Loaded)
        .collect();
    let alias_matches: Vec<ServedModelStatus> = loaded
        .iter()
        .filter(|model| model.model_alias.as_deref() == Some(requested_model))
        .cloned()
        .collect();

    if alias_matches.len() == 1 {
        return OpenAiServedModelLookup::Found(alias_matches.into_iter().next().unwrap());
    }
    if alias_matches.len() > 1 {
        return OpenAiServedModelLookup::Ambiguous {
            code: ModelServeErrorCode::DuplicateModelAlias,
            message: format!(
                "gateway model alias '{requested_model}' matches multiple served instances"
            ),
        };
    }

    let model_id_matches: Vec<ServedModelStatus> = loaded
        .into_iter()
        .filter(|model| model.model_id == requested_model)
        .collect();
    if model_id_matches.len() == 1 {
        return OpenAiServedModelLookup::Found(model_id_matches.into_iter().next().unwrap());
    }
    if model_id_matches.len() > 1 {
        return OpenAiServedModelLookup::Ambiguous {
            code: ModelServeErrorCode::AmbiguousModelRouting,
            message: format!(
                "base model id '{requested_model}' matches multiple served instances; request one of the listed gateway aliases instead"
            ),
        };
    }

    OpenAiServedModelLookup::NotFound
}

async fn proxy_response(response: reqwest::Response) -> Response {
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| HeaderValue::from_str(value).ok());
    match response.bytes().await {
        Ok(bytes) => response_with_bytes(status, content_type, bytes),
        Err(error) => openai_error_response(StatusCode::BAD_GATEWAY, error.to_string()),
    }
}

fn response_with_bytes(
    status: StatusCode,
    content_type: Option<HeaderValue>,
    bytes: Bytes,
) -> Response {
    let mut headers = HeaderMap::new();
    if let Some(content_type) = content_type {
        headers.insert(header::CONTENT_TYPE, content_type);
    }
    (status, headers, bytes).into_response()
}

fn openai_error_response(status: StatusCode, message: impl Into<String>) -> Response {
    openai_error_response_body(status, message, None)
}

fn openai_error_response_with_code(
    status: StatusCode,
    code: ModelServeErrorCode,
    message: impl Into<String>,
) -> Response {
    openai_error_response_body(status, message, Some(code))
}

fn openai_error_response_body(
    status: StatusCode,
    message: impl Into<String>,
    code: Option<ModelServeErrorCode>,
) -> Response {
    let mut error = Map::new();
    error.insert("message".to_string(), Value::String(message.into()));
    error.insert("type".to_string(), Value::String("pumas_error".to_string()));
    if let Some(code) = code {
        error.insert(
            "code".to_string(),
            serde_json::to_value(code).unwrap_or_else(|_| json!("unknown")),
        );
    }
    (
        status,
        Json(json!({
            "error": Value::Object(error)
        })),
    )
        .into_response()
}

#[cfg(test)]
mod openai_gateway_tests {
    use super::*;
    use pumas_library::models::{
        RuntimeDeviceMode, RuntimeProfileId, RuntimeProviderId, ServedModelLoadState,
        ServingEndpointStatus, ServingStatusSnapshot,
    };

    fn loaded_status(
        model_id: &str,
        profile_id: &str,
        model_alias: Option<&str>,
    ) -> ServedModelStatus {
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
}

/// Server-sent model-library update notification stream.
pub async fn handle_model_library_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelLibraryUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream: BoxStream<'static, Result<Event, Infallible>> =
        match build_model_library_update_stream_state(state, query.cursor).await {
            Ok(stream_state) => {
                stream::unfold(stream_state, next_model_library_update_event).boxed()
            }
            Err(error) => {
                warn!("model-library update stream startup failed: {}", error);
                stream::once(async move {
                    Ok(Event::default()
                        .event("model-library-error")
                        .data(json!({ "error": error.to_string() }).to_string()))
                })
                .boxed()
            }
        };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Debug, Default, Deserialize)]
pub struct ModelLibraryUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ModelDownloadUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RuntimeProfileUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ServingStatusUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct StatusTelemetryUpdateStreamQuery {
    pub cursor: Option<String>,
}

async fn build_model_library_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> pumas_library::Result<ModelLibraryUpdateStreamState> {
    let requested_cursor = match cursor {
        Some(cursor) if !cursor.trim().is_empty() => cursor,
        _ => {
            state
                .api
                .list_model_library_updates_since(None, MODEL_LIBRARY_UPDATE_STREAM_LIMIT)
                .await?
                .cursor
        }
    };
    let subscriber = state
        .api
        .subscribe_model_library_update_stream_since(&requested_cursor)
        .await?;
    let handshake = subscriber.handshake().clone();
    let cursor = handshake.cursor_after_recovery.clone();
    let pending_notification = if handshake.recovered_events.is_empty()
        && !handshake.stale_cursor
        && !handshake.snapshot_required
    {
        None
    } else {
        Some(ModelLibraryUpdateNotification {
            cursor: handshake.cursor_after_recovery,
            events: handshake.recovered_events,
            stale_cursor: handshake.stale_cursor,
            snapshot_required: handshake.snapshot_required,
        })
    };

    Ok(ModelLibraryUpdateStreamState {
        subscriber,
        cursor,
        pending_notification,
    })
}

/// Server-sent model download update notification stream.
pub async fn handle_model_download_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelDownloadUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream: BoxStream<'static, Result<Event, Infallible>> =
        match build_model_download_update_stream_state(state, query.cursor).await {
            Ok(stream_state) => {
                stream::unfold(stream_state, next_model_download_update_event).boxed()
            }
            Err(error) => {
                warn!("model download update stream startup failed: {}", error);
                stream::once(async move {
                    Ok(Event::default()
                        .event("model-download-error")
                        .data(json!({ "error": error.to_string() }).to_string()))
                })
                .boxed()
            }
        };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn build_model_download_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> pumas_library::Result<ModelDownloadUpdateStreamState> {
    let receiver = state.api.subscribe_hf_download_updates().ok_or_else(|| {
        pumas_library::PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        }
    })?;
    let pending_notification = Some(
        state
            .api
            .hf_download_notification_since(cursor.as_deref())
            .await,
    );

    Ok(ModelDownloadUpdateStreamState {
        state,
        receiver,
        pending_notification,
    })
}

/// Server-sent runtime-profile update notification stream.
pub async fn handle_runtime_profile_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RuntimeProfileUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream_state = build_runtime_profile_update_stream_state(state, query.cursor).await;
    let stream = stream::unfold(stream_state, next_runtime_profile_update_event).boxed();

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn build_runtime_profile_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> RuntimeProfileUpdateStreamState {
    let receiver = state.api.subscribe_runtime_profile_updates();
    let pending_feed = match state
        .api
        .list_runtime_profile_updates_since(cursor.as_deref())
        .await
    {
        Ok(response)
            if !response.feed.events.is_empty()
                || response.feed.stale_cursor
                || response.feed.snapshot_required =>
        {
            Some(response.feed)
        }
        Ok(_) => None,
        Err(error) => {
            warn!(
                "runtime-profile update stream startup recovery failed: {}",
                error
            );
            Some(RuntimeProfileUpdateFeed::snapshot_required(
                "runtime-profiles:0".to_string(),
            ))
        }
    };

    RuntimeProfileUpdateStreamState {
        receiver,
        pending_feed,
    }
}

/// Server-sent serving-status update notification stream.
pub async fn handle_serving_status_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ServingStatusUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream_state = build_serving_status_update_stream_state(state, query.cursor).await;
    let stream = stream::unfold(stream_state, next_serving_status_update_event).boxed();

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn build_serving_status_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> ServingStatusUpdateStreamState {
    let receiver = state.api.subscribe_serving_status_updates();
    let pending_feed = match state
        .api
        .list_serving_status_updates_since(cursor.as_deref())
        .await
    {
        Ok(response)
            if !response.feed.events.is_empty()
                || response.feed.stale_cursor
                || response.feed.snapshot_required =>
        {
            Some(response.feed)
        }
        Ok(_) => None,
        Err(error) => {
            warn!(
                "serving-status update stream startup recovery failed: {}",
                error
            );
            Some(ServingStatusUpdateFeed::snapshot_required(
                "serving:0".to_string(),
            ))
        }
    };

    ServingStatusUpdateStreamState {
        receiver,
        pending_feed,
    }
}

/// Server-sent status/resource telemetry update stream.
pub async fn handle_status_telemetry_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatusTelemetryUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream: BoxStream<'static, Result<Event, Infallible>> =
        match build_status_telemetry_update_stream_state(state, query.cursor).await {
            Ok(stream_state) => {
                stream::unfold(stream_state, next_status_telemetry_update_event).boxed()
            }
            Err(error) => {
                warn!("status telemetry update stream startup failed: {}", error);
                stream::once(async move {
                    Ok(Event::default()
                        .event("status-telemetry-error")
                        .data(json!({ "error": error.to_string() }).to_string()))
                })
                .boxed()
            }
        };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Main JSON-RPC handler.
pub async fn handle_rpc(
    State(state): State<Arc<AppState>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let method = &request.method;
    let params = request.params.unwrap_or(Value::Object(Default::default()));
    let id = request.id.clone();

    debug!("RPC call: {}({:?})", method, params);

    // Handle built-in methods
    if method == "health_check" {
        return (
            StatusCode::OK,
            Json(JsonRpcResponse::success(id, json!({"status": "ok"}))),
        );
    }

    if method == "shutdown" {
        // In production, this would trigger a graceful shutdown
        return (
            StatusCode::OK,
            Json(JsonRpcResponse::success(
                id,
                json!({"status": "shutting_down"}),
            )),
        );
    }

    // Dispatch to API methods
    let result = dispatch_method(&state, method, &params).await;

    match result {
        Ok(value) => {
            let wrapped = wrap_response(method, value);
            (StatusCode::OK, Json(JsonRpcResponse::success(id, wrapped)))
        }
        Err(e) => {
            error!("RPC error for {}: {}", method, e);
            let code = e.to_rpc_error_code();
            (
                StatusCode::OK,
                Json(JsonRpcResponse::error(id, code, e.to_string())),
            )
        }
    }
}

struct ModelLibraryUpdateStreamState {
    subscriber: pumas_library::model_library::ModelLibraryUpdateSubscriber,
    cursor: String,
    pending_notification: Option<ModelLibraryUpdateNotification>,
}

struct ModelDownloadUpdateStreamState {
    state: Arc<AppState>,
    receiver: broadcast::Receiver<ModelDownloadUpdateNotification>,
    pending_notification: Option<ModelDownloadUpdateNotification>,
}

async fn next_model_library_update_event(
    mut state: ModelLibraryUpdateStreamState,
) -> Option<(Result<Event, Infallible>, ModelLibraryUpdateStreamState)> {
    if let Some(notification) = state.pending_notification.take() {
        let event = model_library_update_sse_event(&notification);
        return Some((Ok(event), state));
    }

    match state.subscriber.next_event().await {
        Ok(update) => {
            state.cursor = update.cursor.clone();
            let notification = ModelLibraryUpdateNotification {
                cursor: update.cursor.clone(),
                events: vec![update],
                stale_cursor: false,
                snapshot_required: false,
            };
            let event = model_library_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(error) => {
            warn!(
                cursor = %state.cursor,
                "model-library update stream ended: {}",
                error
            );
            None
        }
    }
}

fn model_library_update_sse_event(notification: &ModelLibraryUpdateNotification) -> Event {
    match serde_json::to_string(notification) {
        Ok(payload) => Event::default().event("model-library-update").data(payload),
        Err(error) => Event::default()
            .event("model-library-error")
            .data(json!({ "error": error.to_string() }).to_string()),
    }
}

async fn next_model_download_update_event(
    mut state: ModelDownloadUpdateStreamState,
) -> Option<(Result<Event, Infallible>, ModelDownloadUpdateStreamState)> {
    if let Some(notification) = state.pending_notification.take() {
        let event = model_download_update_sse_event(&notification);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(notification) => {
            let event = model_download_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            let notification = state.state.api.hf_download_notification_since(None).await;
            let event = model_download_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

fn model_download_update_sse_event(notification: &ModelDownloadUpdateNotification) -> Event {
    match serde_json::to_string(notification) {
        Ok(payload) => Event::default()
            .event("model-download-update")
            .data(payload),
        Err(error) => Event::default()
            .event("model-download-error")
            .data(json!({ "error": error.to_string() }).to_string()),
    }
}

struct RuntimeProfileUpdateStreamState {
    receiver: broadcast::Receiver<RuntimeProfileUpdateFeed>,
    pending_feed: Option<RuntimeProfileUpdateFeed>,
}

struct ServingStatusUpdateStreamState {
    receiver: broadcast::Receiver<ServingStatusUpdateFeed>,
    pending_feed: Option<ServingStatusUpdateFeed>,
}

struct StatusTelemetryUpdateStreamState {
    state: Arc<AppState>,
    receiver: broadcast::Receiver<StatusTelemetryUpdateNotification>,
    pending_notification: Option<StatusTelemetryUpdateNotification>,
}

async fn build_status_telemetry_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> pumas_library::Result<StatusTelemetryUpdateStreamState> {
    let receiver = state.api.subscribe_status_telemetry_updates();
    let snapshot = state.api.get_status_telemetry_snapshot().await?;
    let pending_notification = state
        .api
        .status_telemetry_notification_since(cursor.as_deref(), snapshot);

    Ok(StatusTelemetryUpdateStreamState {
        state,
        receiver,
        pending_notification,
    })
}

async fn next_status_telemetry_update_event(
    mut state: StatusTelemetryUpdateStreamState,
) -> Option<(Result<Event, Infallible>, StatusTelemetryUpdateStreamState)> {
    if let Some(notification) = state.pending_notification.take() {
        let event = status_telemetry_update_sse_event(&notification);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(notification) => {
            let event = status_telemetry_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            match state.state.api.get_status_telemetry_snapshot().await {
                Ok(snapshot) => {
                    let notification = StatusTelemetryUpdateNotification {
                        cursor: snapshot.cursor.clone(),
                        snapshot,
                        stale_cursor: true,
                        snapshot_required: true,
                    };
                    let event = status_telemetry_update_sse_event(&notification);
                    Some((Ok(event), state))
                }
                Err(error) => {
                    warn!("status telemetry refresh after lag failed: {}", error);
                    None
                }
            }
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

fn status_telemetry_update_sse_event(notification: &StatusTelemetryUpdateNotification) -> Event {
    match serde_json::to_string(notification) {
        Ok(payload) => Event::default()
            .event("status-telemetry-update")
            .data(payload),
        Err(error) => Event::default()
            .event("status-telemetry-error")
            .data(json!({ "error": error.to_string() }).to_string()),
    }
}

async fn next_runtime_profile_update_event(
    mut state: RuntimeProfileUpdateStreamState,
) -> Option<(Result<Event, Infallible>, RuntimeProfileUpdateStreamState)> {
    if let Some(feed) = state.pending_feed.take() {
        let event = runtime_profile_update_sse_event(&feed);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(feed) => {
            let event = runtime_profile_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            let feed =
                RuntimeProfileUpdateFeed::snapshot_required("runtime-profiles:0".to_string());
            let event = runtime_profile_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

fn runtime_profile_update_sse_event(feed: &RuntimeProfileUpdateFeed) -> Event {
    match serde_json::to_string(feed) {
        Ok(payload) => Event::default()
            .event("runtime-profile-update")
            .data(payload),
        Err(error) => Event::default()
            .event("runtime-profile-error")
            .data(json!({ "error": error.to_string() }).to_string()),
    }
}

async fn next_serving_status_update_event(
    mut state: ServingStatusUpdateStreamState,
) -> Option<(Result<Event, Infallible>, ServingStatusUpdateStreamState)> {
    if let Some(feed) = state.pending_feed.take() {
        let event = serving_status_update_sse_event(&feed);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(feed) => {
            let event = serving_status_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            let feed = ServingStatusUpdateFeed::snapshot_required("serving:0".to_string());
            let event = serving_status_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

fn serving_status_update_sse_event(feed: &ServingStatusUpdateFeed) -> Event {
    match serde_json::to_string(feed) {
        Ok(payload) => Event::default()
            .event("serving-status-update")
            .data(payload),
        Err(error) => Event::default()
            .event("serving-status-error")
            .data(json!({ "error": error.to_string() }).to_string()),
    }
}

// ============================================================================
// Method dispatcher
// ============================================================================

/// Dispatch a method call to the appropriate domain handler.
async fn dispatch_method(
    state: &AppState,
    method: &str,
    params: &Value,
) -> pumas_library::Result<Value> {
    match method {
        // Status & System
        "get_status" => status::get_status(state, params).await,
        "get_disk_space" => status::get_disk_space(state, params).await,
        "get_system_resources" => status::get_system_resources(state, params).await,
        "get_status_telemetry_snapshot" => {
            status::get_status_telemetry_snapshot(state, params).await
        }
        "get_launcher_version" => status::get_launcher_version(state, params).await,
        "check_launcher_updates" => status::check_launcher_updates(state, params).await,
        "apply_launcher_update" => status::apply_launcher_update(state, params).await,
        "restart_launcher" => status::restart_launcher(state, params).await,
        "get_sandbox_info" => status::get_sandbox_info(state, params).await,
        "check_git" => status::check_git(state, params).await,
        "check_brave" => status::check_brave(state, params).await,
        "check_setproctitle" => status::check_setproctitle(state, params).await,
        "get_network_status" => status::get_network_status(state, params).await,
        "get_library_status" => status::get_library_status(state, params).await,
        "get_app_status" => status::get_app_status(state, params).await,

        // Local Runtime Profiles
        "get_runtime_profiles_snapshot" => {
            runtime_profiles::get_runtime_profiles_snapshot(state, params).await
        }
        "list_runtime_profile_updates_since" => {
            runtime_profiles::list_runtime_profile_updates_since(state, params).await
        }
        "upsert_runtime_profile" => runtime_profiles::upsert_runtime_profile(state, params).await,
        "delete_runtime_profile" => runtime_profiles::delete_runtime_profile(state, params).await,
        "set_model_runtime_route" => runtime_profiles::set_model_runtime_route(state, params).await,
        "clear_model_runtime_route" => {
            runtime_profiles::clear_model_runtime_route(state, params).await
        }
        "launch_runtime_profile" => runtime_profiles::launch_runtime_profile(state, params).await,
        "stop_runtime_profile" => runtime_profiles::stop_runtime_profile(state, params).await,

        // User-Directed Serving
        "get_serving_status" => serving::get_serving_status(state, params).await,
        "list_serving_status_updates_since" => {
            serving::list_serving_status_updates_since(state, params).await
        }
        "validate_model_serving_config" => {
            serving::validate_model_serving_config(state, params).await
        }
        "serve_model" => serving::serve_model(state, params).await,
        "unserve_model" => serving::unserve_model(state, params).await,

        // Version Management
        "get_available_versions" => versions::get_available_versions(state, params).await,
        "get_installed_versions" => versions::get_installed_versions(state, params).await,
        "get_active_version" => versions::get_active_version(state, params).await,
        "get_default_version" => versions::get_default_version(state, params).await,
        "set_default_version" => versions::set_default_version(state, params).await,
        "switch_version" => versions::switch_version(state, params).await,
        "install_version" => versions::install_version(state, params).await,
        "remove_version" => versions::remove_version(state, params).await,
        "cancel_installation" => versions::cancel_installation(state, params).await,
        "get_installation_progress" => versions::get_installation_progress(state, params).await,
        "validate_installations" => versions::validate_installations(state, params).await,
        "get_version_status" => versions::get_version_status(state, params).await,
        "get_version_info" => versions::get_version_info(state, params).await,
        "get_release_size_info" => versions::get_release_size_info(state, params).await,
        "get_release_size_breakdown" => versions::get_release_size_breakdown(state, params).await,
        "calculate_release_size" => versions::calculate_release_size(state, params).await,
        "calculate_all_release_sizes" => versions::calculate_all_release_sizes(state, params).await,
        "has_background_fetch_completed" => {
            versions::has_background_fetch_completed(state, params).await
        }
        "reset_background_fetch_flag" => versions::reset_background_fetch_flag(state, params).await,
        "get_github_cache_status" => versions::get_github_cache_status(state, params).await,
        "check_version_dependencies" => versions::check_version_dependencies(state, params).await,
        "install_version_dependencies" => {
            versions::install_version_dependencies(state, params).await
        }
        "get_release_dependencies" => versions::get_release_dependencies(state, params).await,
        "is_patched" => versions::is_patched(state, params).await,
        "toggle_patch" => versions::toggle_patch(state, params).await,

        // Model Library
        "get_models" => models::get_models(state, params).await,
        "refresh_model_index" => models::refresh_model_index(state, params).await,
        "refresh_model_mappings" => models::refresh_model_mappings(state, params).await,
        "import_model" => models::import_model(state, params).await,
        "download_model_from_hf" => models::download_model_from_hf(state, params).await,
        "start_model_download_from_hf" => models::start_model_download_from_hf(state, params).await,
        "get_model_download_status" => models::get_model_download_status(state, params).await,
        "cancel_model_download" => models::cancel_model_download(state, params).await,
        "pause_model_download" => models::pause_model_download(state, params).await,
        "resume_model_download" => models::resume_model_download(state, params).await,
        "list_model_downloads" => models::list_model_downloads(state, params).await,
        "list_interrupted_downloads" => models::list_interrupted_downloads(state, params).await,
        "recover_download" => models::recover_download(state, params).await,
        "resume_partial_download" => models::resume_partial_download(state, params).await,
        "search_hf_models" => models::search_hf_models(state, params).await,
        "get_hf_download_details" => models::get_hf_download_details(state, params).await,
        "get_related_models" => models::get_related_models(state, params).await,
        "search_models_fts" => models::search_models_fts(state, params).await,
        "import_batch" => models::import_batch(state, params).await,
        "import_external_diffusers_directory" => {
            models::import_external_diffusers_directory(state, params).await
        }
        "classify_model_import_paths" => models::classify_model_import_paths(state, params).await,
        "lookup_hf_metadata_for_file" => models::lookup_hf_metadata_for_file(state, params).await,
        "lookup_hf_metadata_for_bundle_directory" => {
            models::lookup_hf_metadata_for_bundle_directory(state, params).await
        }
        "detect_sharded_sets" => models::detect_sharded_sets(state, params).await,
        "validate_file_type" => models::validate_file_type(state, params).await,
        "get_embedded_metadata" => models::get_embedded_metadata(state, params).await,
        "get_library_model_metadata" => models::get_library_model_metadata(state, params).await,
        "resolve_model_execution_descriptor" => {
            models::resolve_model_execution_descriptor(state, params).await
        }
        "resolve_model_package_facts" => models::resolve_model_package_facts(state, params).await,
        "list_model_library_updates_since" => {
            models::list_model_library_updates_since(state, params).await
        }
        "resolve_model_package_facts_summary" => {
            models::resolve_model_package_facts_summary(state, params).await
        }
        "model_package_facts_summary_snapshot" => {
            models::model_package_facts_summary_snapshot(state, params).await
        }
        "refetch_model_metadata_from_hf" => {
            models::refetch_model_metadata_from_hf(state, params).await
        }
        "adopt_orphan_models" => models::adopt_orphan_models(state, params).await,
        "import_model_in_place" => models::import_model_in_place(state, params).await,
        "scan_shared_storage" => models::scan_shared_storage(state, params).await,

        // Inference Settings
        "get_inference_settings" => models::get_inference_settings(state, params).await,
        "update_inference_settings" => models::update_inference_settings(state, params).await,
        "update_model_notes" => models::update_model_notes(state, params).await,
        "resolve_model_dependency_requirements" => {
            models::resolve_model_dependency_requirements(state, params).await
        }
        "audit_dependency_pin_compliance" => {
            models::audit_dependency_pin_compliance(state, params).await
        }
        "list_models_needing_review" => models::list_models_needing_review(state, params).await,
        "submit_model_review" => models::submit_model_review(state, params).await,
        "reset_model_review" => models::reset_model_review(state, params).await,
        "generate_model_migration_dry_run_report" => {
            models::generate_model_migration_dry_run_report(state, params).await
        }
        "execute_model_migration" => models::execute_model_migration(state, params).await,
        "list_model_migration_reports" => models::list_model_migration_reports(state, params).await,
        "delete_model_migration_report" => {
            models::delete_model_migration_report(state, params).await
        }
        "prune_model_migration_reports" => {
            models::prune_model_migration_reports(state, params).await
        }

        // HuggingFace Authentication
        "set_hf_token" => models::set_hf_token(state, params).await,
        "clear_hf_token" => models::clear_hf_token(state, params).await,
        "get_hf_auth_status" => models::get_hf_auth_status(state, params).await,

        // Process Management
        "is_comfyui_running" => process::is_comfyui_running(state, params).await,
        "stop_comfyui" => process::stop_comfyui(state, params).await,
        "launch_comfyui" => process::launch_comfyui(state, params).await,
        "launch_ollama" => process::launch_ollama(state, params).await,
        "stop_ollama" => process::stop_ollama(state, params).await,
        "is_ollama_running" => process::is_ollama_running(state, params).await,
        "launch_torch" => process::launch_torch(state, params).await,
        "stop_torch" => process::stop_torch(state, params).await,
        "is_torch_running" => process::is_torch_running(state, params).await,
        "open_path" => process::open_path(state, params).await,
        "open_url" => process::open_url(state, params).await,
        "open_active_install" => process::open_active_install(state, params).await,

        // Ollama Model Management
        "ollama_list_models" => ollama::ollama_list_models(state, params).await,
        "ollama_list_models_for_profile" => {
            ollama::ollama_list_models_for_profile(state, params).await
        }
        "ollama_create_model" => ollama::ollama_create_model(state, params).await,
        "ollama_create_model_for_profile" => {
            ollama::ollama_create_model_for_profile(state, params).await
        }
        "ollama_delete_model" => ollama::ollama_delete_model(state, params).await,
        "ollama_delete_model_for_profile" => {
            ollama::ollama_delete_model_for_profile(state, params).await
        }
        "ollama_load_model" => ollama::ollama_load_model(state, params).await,
        "ollama_load_model_for_profile" => {
            ollama::ollama_load_model_for_profile(state, params).await
        }
        "ollama_unload_model" => ollama::ollama_unload_model(state, params).await,
        "ollama_unload_model_for_profile" => {
            ollama::ollama_unload_model_for_profile(state, params).await
        }
        "ollama_list_running" => ollama::ollama_list_running(state, params).await,

        // Torch Inference Server
        "torch_list_slots" => torch::torch_list_slots(state, params).await,
        "torch_load_model" => torch::torch_load_model(state, params).await,
        "torch_unload_model" => torch::torch_unload_model(state, params).await,
        "torch_get_status" => torch::torch_get_status(state, params).await,
        "torch_list_devices" => torch::torch_list_devices(state, params).await,
        "torch_configure" => torch::torch_configure(state, params).await,

        // Link Management
        "get_link_health" => links::get_link_health(state, params).await,
        "clean_broken_links" => links::clean_broken_links(state, params).await,
        "remove_orphaned_links" => links::remove_orphaned_links(state, params).await,
        "get_links_for_model" => links::get_links_for_model(state, params).await,
        "delete_model_with_cascade" => links::delete_model_with_cascade(state, params).await,
        "preview_model_mapping" => links::preview_model_mapping(state, params).await,
        "apply_model_mapping" => links::apply_model_mapping(state, params).await,
        "sync_models_incremental" => links::sync_models_incremental(state, params).await,
        "sync_with_resolutions" => links::sync_with_resolutions(state, params).await,
        "get_cross_filesystem_warning" => links::get_cross_filesystem_warning(state, params).await,
        "get_file_link_count" => links::get_file_link_count(state, params).await,
        "check_files_writable" => links::check_files_writable(state, params).await,
        "set_model_link_exclusion" => links::set_model_link_exclusion(state, params).await,
        "get_link_exclusions" => links::get_link_exclusions(state, params).await,

        // Shortcuts
        "get_version_shortcuts" => shortcuts::get_version_shortcuts(state, params).await,
        "get_all_shortcut_states" => shortcuts::get_all_shortcut_states(state, params).await,
        "toggle_menu" => shortcuts::toggle_menu(state, params).await,
        "toggle_desktop" => shortcuts::toggle_desktop(state, params).await,
        "menu_exists" => shortcuts::menu_exists(state, params).await,
        "desktop_exists" => shortcuts::desktop_exists(state, params).await,
        "install_icon" => shortcuts::install_icon(state, params).await,
        "create_menu_shortcut" => shortcuts::create_menu_shortcut(state, params).await,
        "create_desktop_shortcut" => shortcuts::create_desktop_shortcut(state, params).await,
        "remove_menu_shortcut" => shortcuts::remove_menu_shortcut(state, params).await,
        "remove_desktop_shortcut" => shortcuts::remove_desktop_shortcut(state, params).await,

        // Conversion
        "start_model_conversion" => conversion::start_model_conversion(state, params).await,
        "get_conversion_progress" => conversion::get_conversion_progress(state, params).await,
        "cancel_model_conversion" => conversion::cancel_model_conversion(state, params).await,
        "list_model_conversions" => conversion::list_model_conversions(state, params).await,
        "check_conversion_environment" => {
            conversion::check_conversion_environment(state, params).await
        }
        "setup_conversion_environment" => {
            conversion::setup_conversion_environment(state, params).await
        }
        "get_supported_quant_types" => conversion::get_supported_quant_types(state, params).await,
        "get_backend_status" => conversion::get_backend_status(state, params).await,
        "setup_quantization_backend" => conversion::setup_quantization_backend(state, params).await,

        // Plugins
        "get_plugins" => plugins::get_plugins(state, params).await,
        "get_plugin" => plugins::get_plugin(state, params).await,
        "call_plugin_endpoint" => plugins::call_plugin_endpoint(state, params).await,
        "check_plugin_health" => plugins::check_plugin_health(state, params).await,

        // Custom Nodes
        "get_custom_nodes" => custom_nodes::get_custom_nodes(state, params).await,
        "install_custom_node" => custom_nodes::install_custom_node(state, params).await,
        "update_custom_node" => custom_nodes::update_custom_node(state, params).await,
        "remove_custom_node" => custom_nodes::remove_custom_node(state, params).await,

        // Unknown method
        _ => {
            warn!("Method not found: {}", method);
            Err(pumas_library::PumasError::Other(format!(
                "Method not found: {}",
                method
            )))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_response_success() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!({"data": "test"}));
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let response = JsonRpcResponse::error(Some(json!(1)), -32600, "Test error".into());
        assert!(response.error.is_some());
        assert!(response.result.is_none());
        assert_eq!(response.error.unwrap().code, -32600);
    }

    #[tokio::test]
    async fn test_detect_sandbox() {
        let (is_sandboxed, sandbox_type, _) = detect_sandbox_environment().await;
        // In normal development, we're not sandboxed
        // This test verifies the function runs without error
        assert!(!is_sandboxed || ["flatpak", "snap", "docker", "appimage"].contains(&sandbox_type));
    }
}
