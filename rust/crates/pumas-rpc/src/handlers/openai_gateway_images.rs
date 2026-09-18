//! Public image-generation contract authority and Torch provider projection.
//!
//! This module owns the public `/v1/images/generations` contract: [`parse`]
//! decodes and validates untrusted gateway JSON into a
//! [`PublicImageGenerationRequest`], and the [`success`]/[`provider_error`]
//! projections render the typed Torch adapter result back into the public
//! response shape. No other module may admit image fields.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use pumas_app_manager::{TorchImageError, TorchImageResult};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageRequest {
    model: String,
    prompt: String,
    #[serde(default = "one")]
    n: u8,
    width: u32,
    height: u32,
    #[serde(default = "default_format")]
    response_format: String,
    #[serde(default)]
    seed: Option<u32>,
}
fn one() -> u8 {
    1
}
fn default_format() -> String {
    "b64_json".into()
}

/// Validated public image-generation request: the single contract authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicImageGenerationRequest {
    pub model: String,
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    pub seed: Option<u32>,
    pub n: u8,
    pub response_format: String,
}

/// Decode and validate untrusted gateway JSON into the public request.
pub(super) fn parse(body: &Value) -> Result<PublicImageGenerationRequest, &'static str> {
    let request: ImageRequest = serde_json::from_value(body.clone())
        .map_err(|_| "Unsupported or invalid image request fields")?;
    if request.model.trim().is_empty() || request.model.chars().count() > 256 {
        return Err("model must contain 1 to 256 characters");
    }
    if request.prompt.trim().is_empty() || request.prompt.chars().count() > 4000 {
        return Err("prompt must contain 1 to 4000 characters");
    }
    if request.n != 1 || request.response_format != "b64_json" {
        return Err("Only one base64 PNG per request is supported");
    }
    if request.width == 0 || request.height == 0 {
        return Err("width and height must be positive integers");
    }
    Ok(PublicImageGenerationRequest {
        model: request.model,
        prompt: request.prompt,
        width: request.width,
        height: request.height,
        seed: request.seed,
        n: request.n,
        response_format: request.response_format,
    })
}

pub(super) fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error": {"type": "pumas_error", "code": code, "message": message}})),
    )
        .into_response()
}

/// Project the typed Torch result into the public response shape.
pub(super) fn success(result: &TorchImageResult) -> Response {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    (
        StatusCode::OK,
        Json(json!({
            "created": created,
            "data": [{"b64_json": result.png_base64}],
            "metadata": {
                "seed": result.seed,
                "steps": result.steps,
                "guidance": result.guidance,
                "memory_policy": result.memory_policy,
                "duration_seconds": result.duration_seconds
            }
        })),
    )
        .into_response()
}

/// Map decoded Torch provider errors to the existing public messages.
pub(super) fn provider_error(failure: &TorchImageError) -> Response {
    match failure {
        TorchImageError::RuntimeBusy => error(
            StatusCode::CONFLICT,
            "runtime_busy",
            "Image runtime is busy",
        ),
        TorchImageError::ModelUnavailable => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "model_unavailable",
            "Load the image model in Pumas first",
        ),
        TorchImageError::UnsupportedModel => error(
            StatusCode::BAD_REQUEST,
            "unsupported_model",
            "Selected model does not support image generation",
        ),
        TorchImageError::OutOfMemory => error(
            StatusCode::INSUFFICIENT_STORAGE,
            "out_of_memory",
            "Insufficient GPU memory",
        ),
        TorchImageError::DeadlineExceeded => error(
            StatusCode::GATEWAY_TIMEOUT,
            "deadline_exceeded",
            "Image generation exceeded its deadline",
        ),
        TorchImageError::Cancelled => {
            error(cancelled_status(), "cancelled", "Image generation stopped")
        }
        TorchImageError::BackendFailure | TorchImageError::Transport => error(
            StatusCode::BAD_GATEWAY,
            "backend_failure",
            "Image runtime failed; inspect its Pumas log",
        ),
        TorchImageError::InvalidBackendResult => error(
            StatusCode::BAD_GATEWAY,
            "invalid_backend_response",
            "Image runtime returned an invalid image response",
        ),
    }
}

fn cancelled_status() -> StatusCode {
    StatusCode::from_u16(499).unwrap_or(StatusCode::BAD_GATEWAY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn rejects_unknown_fields_batching_and_invalid_dimensions_before_backend_admission() {
        for extra in [
            json!({"n": 2}),
            json!({"seed": -1}),
            json!({"seed": true}),
            json!({"size": "512x512"}),
            json!({"width": 0}),
            json!({"height": -1}),
            json!({"width": "512"}),
            json!({"width": true}),
            json!({"prompt":"  "}),
            json!({"steps":30}),
        ] {
            let mut body =
                json!({"model":"image", "prompt":"a watercolor bird", "width":512, "height":512});
            body.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            assert!(parse(&body).is_err(), "{body}");
        }
        for missing in [json!({"height": 720}), json!({"width": 1280})] {
            let mut body = json!({"model":"image", "prompt":"a watercolor bird"});
            body.as_object_mut()
                .unwrap()
                .extend(missing.as_object().unwrap().clone());
            assert!(parse(&body).is_err(), "{body}");
        }
        let request = parse(
            &json!({"model":"image", "prompt":"a watercolor bird", "width":1280, "height":720, "seed":0}),
        )
        .unwrap();
        assert_eq!(request.model, "image");
        assert_eq!(request.prompt, "a watercolor bird");
        assert_eq!(request.width, 1280);
        assert_eq!(request.height, 720);
        assert_eq!(request.seed, Some(0));
        assert_eq!(request.n, 1);
        assert_eq!(request.response_format, "b64_json");
    }

    #[tokio::test]
    async fn success_projects_typed_result_into_public_shape() {
        let response = success(&TorchImageResult {
            png_base64: "aGVsbG8=".to_string(),
            seed: 7,
            steps: 8,
            guidance: 0.0,
            memory_policy: "sequential_cpu_offload".to_string(),
            duration_seconds: 1.5,
        });
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(body["data"][0]["b64_json"], "aGVsbG8=");
        assert_eq!(body["data"].as_array().unwrap().len(), 1);
        assert_eq!(body["metadata"]["seed"], 7);
        assert_eq!(body["metadata"]["steps"], 8);
        assert_eq!(body["metadata"]["memory_policy"], "sequential_cpu_offload");
        assert!(body["created"].as_u64().is_some());
    }

    #[tokio::test]
    async fn provider_errors_keep_existing_messages() {
        for (error, code, message) in [
            (
                TorchImageError::RuntimeBusy,
                "runtime_busy",
                "Image runtime is busy",
            ),
            (
                TorchImageError::ModelUnavailable,
                "model_unavailable",
                "Load the image model in Pumas first",
            ),
            (
                TorchImageError::UnsupportedModel,
                "unsupported_model",
                "Selected model does not support image generation",
            ),
            (
                TorchImageError::OutOfMemory,
                "out_of_memory",
                "Insufficient GPU memory",
            ),
            (
                TorchImageError::DeadlineExceeded,
                "deadline_exceeded",
                "Image generation exceeded its deadline",
            ),
            (
                TorchImageError::Cancelled,
                "cancelled",
                "Image generation stopped",
            ),
            (
                TorchImageError::BackendFailure,
                "backend_failure",
                "Image runtime failed; inspect its Pumas log",
            ),
        ] {
            let response = provider_error(&error);
            assert_eq!(response.status(), provider_expected_status(&error));
            let status = response.status();
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                    .unwrap();
            assert_eq!(status, provider_expected_status(&error));
            assert_eq!(body["error"]["code"], code, "{error:?}");
            assert_eq!(body["error"]["message"], message, "{error:?}");
        }
    }

    fn provider_expected_status(error: &TorchImageError) -> StatusCode {
        match error {
            TorchImageError::RuntimeBusy => StatusCode::CONFLICT,
            TorchImageError::ModelUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            TorchImageError::UnsupportedModel => StatusCode::BAD_REQUEST,
            TorchImageError::OutOfMemory => StatusCode::INSUFFICIENT_STORAGE,
            TorchImageError::DeadlineExceeded => StatusCode::GATEWAY_TIMEOUT,
            TorchImageError::Cancelled => cancelled_status(),
            TorchImageError::BackendFailure => StatusCode::BAD_GATEWAY,
            _ => StatusCode::BAD_GATEWAY,
        }
    }
}
