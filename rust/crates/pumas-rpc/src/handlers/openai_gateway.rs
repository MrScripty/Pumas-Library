//! OpenAI-compatible gateway handlers backed by Pumas serving state.

use super::openai_gateway_onnx::handle_onnx_embedding;
use crate::contract::PublicError;
use crate::server::AppState;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use pumas_app_manager::{
    SlotState, TorchClient, TorchCompatibilityError, TorchHandshakeFailure, TorchImageError,
    GENERATION_CONNECT_TIMEOUT,
};
use pumas_library::models::{
    ModelServeErrorCode, RuntimeEndpointUrl, RuntimeLifecycleState, RuntimeProviderId,
    ServedModelLoadState, ServedModelStatus, ServingStatusSnapshot,
};
use pumas_library::runtime_profiles::OwnedRuntimeProfileObservation;
use pumas_library::{OpenAiGatewayEndpoint, ProviderRegistry};
use serde_json::{json, Map, Value};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const OPENAI_CHAT_COMPLETIONS_BODY_BYTES: usize = 32 * 1024 * 1024;
const OPENAI_COMPLETIONS_BODY_BYTES: usize = 32 * 1024 * 1024;
const OPENAI_EMBEDDINGS_BODY_BYTES: usize = 32 * 1024 * 1024;
/// Total timeout for non-generation gateway routes (models, embeddings).
/// Generation routes never use this budget.
const OPENAI_GATEWAY_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// The one shared generation transport seam for current and future generation
/// routes (`/v1/chat/completions`, `/v1/completions`, and image generation
/// through `TorchClient`).
///
/// Connection establishment is bounded by [`GENERATION_CONNECT_TIMEOUT`]; no
/// total, response-read, idle, or elapsed deadline is set, so an admitted
/// generation may remain connected and silent until its terminal result (see
/// `docs/contracts/generation-lifetime.md`). Non-generation routes
/// (models, embeddings, health, startup, loading, installation, shutdown)
/// keep their independently bounded policies and must not use this client.
fn generation_http_client() -> &'static reqwest::Client {
    static GENERATION_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    GENERATION_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(GENERATION_CONNECT_TIMEOUT)
            .build()
            .expect("failed to build generation HTTP client")
    })
}

#[derive(Debug)]
enum TorchLiveCheckError {
    ProcessIdentity,
    Handshake(TorchHandshakeFailure),
    Slots,
    ModelNotReady,
}

fn torch_process_is_current(
    observation: Option<&OwnedRuntimeProfileObservation>,
    endpoint: &RuntimeEndpointUrl,
) -> bool {
    match observation {
        Some(observation) => {
            observation.state == RuntimeLifecycleState::Running
                && &observation.endpoint_url == endpoint
        }
        // Gateway unit fixtures use an in-process TCP stub rather than a
        // managed runtime. Production Torch serving is always profile-owned.
        #[cfg(test)]
        None => true,
        #[cfg(not(test))]
        None => false,
    }
}

fn torch_process_is_stable(
    before: Option<&OwnedRuntimeProfileObservation>,
    after: Option<&OwnedRuntimeProfileObservation>,
    endpoint: &RuntimeEndpointUrl,
) -> bool {
    before == after
        && torch_process_is_current(before, endpoint)
        && torch_process_is_current(after, endpoint)
}

fn torch_slot_supports_images(
    model: &ServedModelStatus,
    slot: &pumas_app_manager::ModelSlot,
) -> bool {
    slot.model_name == model.model_id
        && slot.state == SlotState::Ready
        && matches!(
            slot.model_type.as_deref(),
            Some("nunchaku-z-image-turbo" | "flux2-klein-9b-kv-fp8")
        )
}

/// Observe one compatible, ready Torch runtime without crossing a process
/// replacement. The process observation, protocol/capability handshake and
/// ready-slot check remain distinct facts, but all must describe one current
/// owned profile before admission or listing.
async fn check_live_torch_image_runtime(
    state: &AppState,
    model: &ServedModelStatus,
    endpoint: &RuntimeEndpointUrl,
) -> Result<TorchClient, TorchLiveCheckError> {
    let before = state
        .api
        .observe_owned_runtime_profile(&model.profile_id)
        .map_err(|_| TorchLiveCheckError::ProcessIdentity)?;
    if !torch_process_is_current(before.as_ref(), endpoint) {
        return Err(TorchLiveCheckError::ProcessIdentity);
    }

    let client = TorchClient::new(Some(endpoint.as_str()));
    client
        .check_image_runtime()
        .await
        .map_err(TorchLiveCheckError::Handshake)?;
    let slots = client
        .list_slots()
        .await
        .map_err(|_| TorchLiveCheckError::Slots)?;
    if !slots
        .iter()
        .any(|slot| torch_slot_supports_images(model, slot))
    {
        return Err(TorchLiveCheckError::ModelNotReady);
    }

    let after = state
        .api
        .observe_owned_runtime_profile(&model.profile_id)
        .map_err(|_| TorchLiveCheckError::ProcessIdentity)?;
    if !torch_process_is_stable(before.as_ref(), after.as_ref(), endpoint) {
        return Err(TorchLiveCheckError::ProcessIdentity);
    }
    Ok(client)
}

/// OpenAI-compatible served-model listing backed by Pumas serving status.
pub async fn handle_openai_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.api.get_serving_status().await {
        Ok(mut response) => {
            let mut unavailable = Vec::new();
            for model in response
                .snapshot
                .served_models
                .iter()
                .filter(|model| model.provider == RuntimeProviderId::Torch)
            {
                let ready = match model.endpoint_url.as_ref() {
                    Some(endpoint) => check_live_torch_image_runtime(&state, model, endpoint)
                        .await
                        .is_ok(),
                    None => false,
                };
                if !ready {
                    unavailable.push((model.model_id.clone(), model.profile_id.clone()));
                }
            }
            response.snapshot.served_models.retain(|model| {
                !unavailable.contains(&(model.model_id.clone(), model.profile_id.clone()))
            });
            openai_models_snapshot_response(response.snapshot)
        }
        Err(error) => openai_public_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            PublicError::from(&error),
        ),
    }
}

fn openai_models_snapshot_response(snapshot: ServingStatusSnapshot) -> Response {
    if snapshot
        .router_profiles
        .iter()
        .any(|profile| !profile.is_observation_current())
    {
        return openai_error_response_with_code(
            StatusCode::SERVICE_UNAVAILABLE,
            ModelServeErrorCode::EndpointUnavailable,
            "Managed router model discovery is unavailable; retry after observation reconnects",
        );
    }
    let mut served_models = snapshot.served_models;
    served_models.retain(|model| {
        model.load_state == ServedModelLoadState::Loaded
            && router_profile_current(&snapshot.router_profiles, &model.profile_id)
    });
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

/// OpenAI-compatible proxy for served models.
pub async fn handle_openai_proxy(
    State(state): State<Arc<AppState>>,
    path: axum::extract::OriginalUri,
    body_bytes: Bytes,
) -> Response {
    let request_path = path.path();
    let Some(policy) = openai_gateway_policy_for_path(request_path) else {
        return openai_error_response(
            StatusCode::NOT_FOUND,
            format!("unsupported OpenAI-compatible endpoint: {request_path}"),
        );
    };

    if body_bytes.len() > policy.max_request_body_bytes {
        return openai_error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "{request_path} request body exceeds {} bytes",
                policy.max_request_body_bytes
            ),
        );
    }

    let mut body: Value = match serde_json::from_slice(&body_bytes) {
        Ok(body) => body,
        Err(error) => {
            return openai_error_response(
                StatusCode::BAD_REQUEST,
                format!("request body must be valid JSON: {error}"),
            );
        }
    };

    if policy.endpoint == OpenAiGatewayEndpoint::ImagesGenerations {
        let request = match super::openai_gateway_images::parse(&body) {
            Ok(request) => request,
            Err(message) => {
                return super::openai_gateway_images::error(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    message,
                );
            }
        };
        return handle_image_generation(&state, request_path, request).await;
    }

    let Some(requested_model) = body
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return openai_error_response(
            StatusCode::BAD_REQUEST,
            "request body must include a string model field",
        );
    };

    let served = match find_openai_served_model(&state, requested_model.as_str()).await {
        Ok(OpenAiServedModelLookup::Found(model)) => model,
        Ok(OpenAiServedModelLookup::NotFound) => {
            return openai_error_response(
                StatusCode::NOT_FOUND,
                format!("model is not served: {}", requested_model.as_str()),
            );
        }
        Ok(OpenAiServedModelLookup::Unavailable) => {
            return openai_error_response_with_code(
                StatusCode::SERVICE_UNAVAILABLE,
                ModelServeErrorCode::EndpointUnavailable,
                "Selected router model observation is unavailable",
            );
        }
        Ok(OpenAiServedModelLookup::Ambiguous { code, message }) => {
            return openai_error_response_with_code(StatusCode::CONFLICT, code, message);
        }
        Err(error) => {
            return openai_public_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                PublicError::from(&error),
            );
        }
    };

    if !provider_supports_openai_gateway_endpoint(
        served.provider,
        policy.endpoint,
        &state.provider_registry,
    ) {
        return openai_error_response_with_code(
            StatusCode::BAD_REQUEST,
            ModelServeErrorCode::EndpointUnavailable,
            format!(
                "provider {:?} does not support {request_path}",
                served.provider
            ),
        );
    }

    if served.provider == RuntimeProviderId::OnnxRuntime {
        return handle_onnx_embedding(
            &state,
            &served,
            requested_model.as_str(),
            policy.endpoint,
            body,
        )
        .await;
    }

    let Some(endpoint) = served.endpoint_url.as_ref() else {
        return openai_error_response(
            StatusCode::BAD_GATEWAY,
            "served model does not have a provider endpoint",
        );
    };

    if let Some(object) = body.as_object_mut() {
        object.insert(
            "model".to_string(),
            Value::String(provider_request_model_id(&served, &state.provider_registry)),
        );
    }

    let target_url = format!("{}{}", endpoint.as_str().trim_end_matches('/'), path.path());
    // Generation routes use the shared duration-unbounded transport without
    // any per-request total/read/idle deadline; non-generation routes keep
    // their independently bounded budget on the gateway client.
    let send = if policy.generation {
        generation_http_client().post(target_url).json(&body).send()
    } else {
        let Some(bounded) = policy.request_timeout else {
            return openai_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "non-generation gateway route is missing its bounded policy",
            );
        };
        state
            .gateway_http_client
            .post(target_url)
            .timeout(bounded)
            .json(&body)
            .send()
    };
    match send.await {
        Ok(response) => proxy_response(response).await,
        Err(_) => openai_public_error_response(StatusCode::BAD_GATEWAY, PublicError::unavailable()),
    }
}

/// Route an image generation through the Torch provider adapter.
///
/// Keeps gateway routing, model lookup, body limits and disconnect
/// propagation; the adapter owns the private sidecar boundary and this
/// projects its typed result into the public response shape.
async fn handle_image_generation(
    state: &Arc<AppState>,
    request_path: &str,
    request: super::openai_gateway_images::PublicImageGenerationRequest,
) -> Response {
    let served = match find_openai_served_model(state, request.model.as_str()).await {
        Ok(OpenAiServedModelLookup::Found(model)) => model,
        Ok(OpenAiServedModelLookup::NotFound) => {
            return openai_error_response(
                StatusCode::NOT_FOUND,
                format!("model is not served: {}", request.model.as_str()),
            );
        }
        Ok(OpenAiServedModelLookup::Unavailable) => {
            return openai_error_response_with_code(
                StatusCode::SERVICE_UNAVAILABLE,
                ModelServeErrorCode::EndpointUnavailable,
                "Selected router model observation is unavailable",
            );
        }
        Ok(OpenAiServedModelLookup::Ambiguous { code, message }) => {
            return openai_error_response_with_code(StatusCode::CONFLICT, code, message);
        }
        Err(error) => {
            return openai_public_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                PublicError::from(&error),
            );
        }
    };

    if !provider_supports_openai_gateway_endpoint(
        served.provider,
        OpenAiGatewayEndpoint::ImagesGenerations,
        &state.provider_registry,
    ) || served.provider != RuntimeProviderId::Torch
    {
        return openai_error_response_with_code(
            StatusCode::BAD_REQUEST,
            ModelServeErrorCode::EndpointUnavailable,
            format!(
                "provider {:?} does not support {request_path}",
                served.provider
            ),
        );
    }

    let Some(endpoint) = served.endpoint_url.as_ref() else {
        return openai_error_response(
            StatusCode::BAD_GATEWAY,
            "served model does not have a provider endpoint",
        );
    };
    debug_assert_eq!(request.n, 1);
    debug_assert_eq!(request.response_format, "b64_json");

    // Recheck process identity, protocol/capability and the ready image slot
    // immediately before admission. Endpoint equality alone is insufficient:
    // the same URL may belong to a replacement process.
    let torch_client = match check_live_torch_image_runtime(state, &served, endpoint).await {
        Ok(client) => client,
        Err(TorchLiveCheckError::ProcessIdentity) => {
            return openai_error_response_with_code(
                StatusCode::SERVICE_UNAVAILABLE,
                ModelServeErrorCode::EndpointUnavailable,
                "Torch process ownership changed before admission; retry the request",
            );
        }
        Err(TorchLiveCheckError::Handshake(TorchHandshakeFailure::Unavailable(_))) => {
            return super::openai_gateway_images::provider_error(&TorchImageError::Transport);
        }
        Err(TorchLiveCheckError::Handshake(TorchHandshakeFailure::Malformed(_)))
        | Err(TorchLiveCheckError::Slots) => {
            return super::openai_gateway_images::provider_error(
                &TorchImageError::InvalidBackendResult,
            );
        }
        Err(TorchLiveCheckError::Handshake(TorchHandshakeFailure::Incompatible(
            TorchCompatibilityError::MissingCapability { .. },
        ))) => {
            return super::openai_gateway_images::provider_error(
                &TorchImageError::UnsupportedModel,
            );
        }
        Err(TorchLiveCheckError::Handshake(TorchHandshakeFailure::Incompatible(_))) => {
            return super::openai_gateway_images::provider_error(&TorchImageError::BackendFailure);
        }
        Err(TorchLiveCheckError::ModelNotReady) => {
            return super::openai_gateway_images::provider_error(
                &TorchImageError::ModelUnavailable,
            );
        }
    };
    // Bind admission to the current process/profile identity: when serving
    // state moved to another endpoint while we handshook, the observation is
    // stale and the request is not admitted against the old process.
    match find_openai_served_model(state, request.model.as_str()).await {
        Ok(OpenAiServedModelLookup::Found(fresh))
            if fresh.endpoint_url.as_ref() == Some(endpoint) => {}
        Ok(_) => {
            return openai_error_response_with_code(
                StatusCode::SERVICE_UNAVAILABLE,
                ModelServeErrorCode::EndpointUnavailable,
                "Torch process ownership changed before admission; retry the request",
            );
        }
        Err(error) => {
            return openai_public_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                PublicError::from(&error),
            );
        }
    }

    // Awaiting inline keeps disconnect propagation: dropping this future
    // drops the sidecar request at a denoising checkpoint, which requests
    // cancellation without proving that sidecar work stopped. A lost
    // transport before a terminal result is an unknown outcome and is never
    // replayed here.
    match torch_client
        .generate_image(
            provider_request_model_id(&served, &state.provider_registry).as_str(),
            request.prompt.as_str(),
            request.width,
            request.height,
            request.seed,
        )
        .await
    {
        Ok(result) => super::openai_gateway_images::success(&result),
        Err(error) => super::openai_gateway_images::provider_error(&error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenAiGatewayEndpointPolicy {
    endpoint: OpenAiGatewayEndpoint,
    max_request_body_bytes: usize,
    /// When `generation` is true this is always `None`: the route uses the
    /// shared duration-unbounded generation transport with no total, read,
    /// idle, or elapsed deadline. Non-generation routes carry `Some` budget.
    generation: bool,
    request_timeout: Option<Duration>,
}

fn openai_gateway_policy_for_path(path: &str) -> Option<OpenAiGatewayEndpointPolicy> {
    match path {
        "/v1/images/generations" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::ImagesGenerations,
            max_request_body_bytes: 32 * 1024,
            generation: true,
            request_timeout: None,
        }),
        "/v1/models" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::Models,
            max_request_body_bytes: 0,
            generation: false,
            request_timeout: Some(OPENAI_GATEWAY_REQUEST_TIMEOUT),
        }),
        "/v1/chat/completions" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::ChatCompletions,
            max_request_body_bytes: OPENAI_CHAT_COMPLETIONS_BODY_BYTES,
            generation: true,
            request_timeout: None,
        }),
        "/v1/completions" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::Completions,
            max_request_body_bytes: OPENAI_COMPLETIONS_BODY_BYTES,
            generation: true,
            request_timeout: None,
        }),
        "/v1/embeddings" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::Embeddings,
            max_request_body_bytes: OPENAI_EMBEDDINGS_BODY_BYTES,
            generation: false,
            request_timeout: Some(OPENAI_GATEWAY_REQUEST_TIMEOUT),
        }),
        _ => None,
    }
}

fn provider_supports_openai_gateway_endpoint(
    provider: pumas_library::models::RuntimeProviderId,
    endpoint: OpenAiGatewayEndpoint,
    registry: &ProviderRegistry,
) -> bool {
    registry
        .get(provider)
        .is_some_and(|behavior| behavior.supports_openai_endpoint(endpoint))
}

fn openai_model_entry(model: ServedModelStatus) -> Value {
    let image_generation = model.provider == RuntimeProviderId::Torch;
    let mut entry = json!({
        "id": model.model_alias.unwrap_or(model.model_id),
        "object": "model",
        "created": 0,
        "owned_by": "pumas"
    });
    if image_generation {
        entry["capabilities"] = json!(["image_generation"]);
    }
    entry
}

fn provider_request_model_id(model: &ServedModelStatus, registry: &ProviderRegistry) -> String {
    registry
        .get(model.provider)
        .map(|behavior| {
            behavior
                .provider_request_model_id(model.model_id.as_str(), model.model_alias.as_deref())
        })
        .unwrap_or_else(|| model.model_id.clone())
}

fn openai_model_id(model: &ServedModelStatus) -> &str {
    model
        .model_alias
        .as_deref()
        .unwrap_or(model.model_id.as_str())
}

#[derive(Debug, Clone, PartialEq)]
enum OpenAiServedModelLookup {
    Found(Box<ServedModelStatus>),
    NotFound,
    Unavailable,
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
    let unavailable_target = snapshot.served_models.iter().any(|model| {
        (model.model_id == requested_model || model.model_alias.as_deref() == Some(requested_model))
            && !router_profile_current(&snapshot.router_profiles, &model.profile_id)
    });
    let loaded: Vec<ServedModelStatus> = snapshot
        .served_models
        .into_iter()
        .filter(|model| {
            model.load_state == ServedModelLoadState::Loaded
                && router_profile_current(&snapshot.router_profiles, &model.profile_id)
        })
        .collect();
    let alias_matches: Vec<ServedModelStatus> = loaded
        .iter()
        .filter(|model| model.model_alias.as_deref() == Some(requested_model))
        .cloned()
        .collect();

    if alias_matches.len() == 1 {
        return OpenAiServedModelLookup::Found(Box::new(alias_matches.into_iter().next().unwrap()));
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
        return OpenAiServedModelLookup::Found(Box::new(
            model_id_matches.into_iter().next().unwrap(),
        ));
    }
    if model_id_matches.len() > 1 {
        return OpenAiServedModelLookup::Ambiguous {
            code: ModelServeErrorCode::AmbiguousModelRouting,
            message: format!(
                "base model id '{requested_model}' matches multiple served instances; request one of the listed gateway aliases instead"
            ),
        };
    }

    if unavailable_target {
        OpenAiServedModelLookup::Unavailable
    } else {
        OpenAiServedModelLookup::NotFound
    }
}

/// Buffer one provider generation response into the public shape.
///
/// Dropping this future drops the provider request, which requests
/// cancellation through the closed transport without proving that provider
/// work stopped. A lost transport before a terminal result is an unknown
/// outcome: this proxy never claims provider cleanup and never replays.
/// Streaming bytes are progress information, not lease renewal; a connected
/// generation may legitimately remain silent until completion.
async fn proxy_response(response: reqwest::Response) -> Response {
    let status = response.status();
    if !status.is_success() {
        return openai_public_error_response(status, PublicError::unavailable());
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| HeaderValue::from_str(value).ok());
    match response.bytes().await {
        Ok(bytes) => response_with_bytes(status, content_type, bytes),
        Err(_) => openai_public_error_response(StatusCode::BAD_GATEWAY, PublicError::unavailable()),
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

pub(crate) fn openai_error_response(status: StatusCode, message: impl Into<String>) -> Response {
    openai_error_response_body(status, message, None)
}

pub(crate) fn openai_error_response_with_code(
    status: StatusCode,
    code: ModelServeErrorCode,
    message: impl Into<String>,
) -> Response {
    openai_error_response_body(status, message, Some(code))
}

fn openai_public_error_response(status: StatusCode, error: PublicError) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "message": error.message,
                "type": "pumas_error",
                "code": error.code,
                "class": error.class,
            }
        })),
    )
        .into_response()
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
#[path = "openai_gateway_tests.rs"]
mod tests;

fn router_profile_current(
    profiles: &[pumas_library::models::RouterProfileSyncStatus],
    id: &pumas_library::models::RuntimeProfileId,
) -> bool {
    profiles
        .iter()
        .find(|profile| &profile.profile_id == id)
        .is_none_or(|profile| profile.is_observation_current())
}
