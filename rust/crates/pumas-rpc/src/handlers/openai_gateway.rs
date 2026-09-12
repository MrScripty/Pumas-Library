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
use pumas_library::models::{
    ModelServeErrorCode, RuntimeProviderId, ServedModelLoadState, ServedModelStatus,
    ServingStatusSnapshot,
};
use pumas_library::{OpenAiGatewayEndpoint, ProviderRegistry};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use std::time::Duration;

const OPENAI_CHAT_COMPLETIONS_BODY_BYTES: usize = 32 * 1024 * 1024;
const OPENAI_COMPLETIONS_BODY_BYTES: usize = 32 * 1024 * 1024;
const OPENAI_EMBEDDINGS_BODY_BYTES: usize = 32 * 1024 * 1024;
const OPENAI_GATEWAY_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// OpenAI-compatible served-model listing backed by Pumas serving status.
pub async fn handle_openai_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.api.get_serving_status().await {
        Ok(response) => openai_models_snapshot_response(response.snapshot),
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
    match state
        .gateway_http_client
        .post(target_url)
        .timeout(policy.request_timeout)
        .json(&body)
        .send()
        .await
    {
        Ok(response) => proxy_response(response).await,
        Err(_) => openai_public_error_response(StatusCode::BAD_GATEWAY, PublicError::unavailable()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenAiGatewayEndpointPolicy {
    endpoint: OpenAiGatewayEndpoint,
    max_request_body_bytes: usize,
    request_timeout: Duration,
}

fn openai_gateway_policy_for_path(path: &str) -> Option<OpenAiGatewayEndpointPolicy> {
    match path {
        "/v1/models" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::Models,
            max_request_body_bytes: 0,
            request_timeout: OPENAI_GATEWAY_REQUEST_TIMEOUT,
        }),
        "/v1/chat/completions" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::ChatCompletions,
            max_request_body_bytes: OPENAI_CHAT_COMPLETIONS_BODY_BYTES,
            request_timeout: OPENAI_GATEWAY_REQUEST_TIMEOUT,
        }),
        "/v1/completions" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::Completions,
            max_request_body_bytes: OPENAI_COMPLETIONS_BODY_BYTES,
            request_timeout: OPENAI_GATEWAY_REQUEST_TIMEOUT,
        }),
        "/v1/embeddings" => Some(OpenAiGatewayEndpointPolicy {
            endpoint: OpenAiGatewayEndpoint::Embeddings,
            max_request_body_bytes: OPENAI_EMBEDDINGS_BODY_BYTES,
            request_timeout: OPENAI_GATEWAY_REQUEST_TIMEOUT,
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
    json!({
        "id": model.model_alias.unwrap_or(model.model_id),
        "object": "model",
        "created": 0,
        "owned_by": "pumas"
    })
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
