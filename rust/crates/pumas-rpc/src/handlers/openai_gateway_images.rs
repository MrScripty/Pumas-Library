//! Image-specific bounds at the existing gateway transport boundary.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageRequest {
    model: String,
    prompt: String,
    #[serde(default = "one")]
    n: u8,
    #[serde(default = "default_size")]
    size: String,
    #[serde(default = "default_format")]
    response_format: String,
    #[serde(default)]
    seed: Option<u32>,
}
fn one() -> u8 {
    1
}
fn default_size() -> String {
    "1024x1024".into()
}
fn default_format() -> String {
    "b64_json".into()
}

pub(super) fn validate(body: &Value) -> Result<(), &'static str> {
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
    if ![
        "512x512",
        "512x768",
        "512x1024",
        "768x512",
        "768x768",
        "768x1024",
        "1024x512",
        "1024x768",
        "1024x1024",
    ]
    .contains(&request.size.as_str())
    {
        return Err("Width and height must each be 512, 768, or 1024");
    }
    let _ = request.seed;
    Ok(())
}

pub(super) fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error": {"type": "pumas_error", "code": code, "message": message}})),
    )
        .into_response()
}

pub(super) async fn response(mut upstream: reqwest::Response) -> Response {
    const MAX_RESPONSE_BYTES: usize = 12 * 1024 * 1024;
    let status = upstream.status();
    let mut bytes = Vec::new();
    loop {
        match upstream.chunk().await {
            Ok(Some(chunk)) if bytes.len().saturating_add(chunk.len()) <= MAX_RESPONSE_BYTES => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => {
                return error(
                    StatusCode::BAD_GATEWAY,
                    "invalid_backend_response",
                    "Image response exceeded its limit or could not be read",
                )
            }
        }
    }
    let body: Value = match serde_json::from_slice(&bytes) {
        Ok(body) => body,
        Err(_) => {
            return error(
                StatusCode::BAD_GATEWAY,
                "invalid_backend_response",
                "Image runtime returned invalid JSON",
            )
        }
    };
    if !status.is_success() {
        // Accept only known runtime codes; never forward a backend traceback,
        // filesystem path or arbitrary error string to clients.
        let (code, message) = match body.pointer("/detail/code").and_then(Value::as_str) {
            Some("runtime_busy") => ("runtime_busy", "Image runtime is busy"),
            Some("out_of_memory") => ("out_of_memory", "Insufficient GPU memory"),
            Some("model_unavailable") => {
                ("model_unavailable", "Load the image model in Pumas first")
            }
            Some("deadline_exceeded") => (
                "deadline_exceeded",
                "Image generation exceeded its deadline",
            ),
            Some("cancelled") => ("cancelled", "Image generation stopped"),
            Some("unsupported_model") => (
                "unsupported_model",
                "Selected model does not support image generation",
            ),
            _ => (
                "backend_failure",
                "Image runtime failed; inspect its Pumas log",
            ),
        };
        return error(status, code, message);
    }
    if body
        .get("data")
        .and_then(Value::as_array)
        .is_none_or(|items| {
            items.len() != 1 || items[0].get("b64_json").and_then(Value::as_str).is_none()
        })
    {
        return error(
            StatusCode::BAD_GATEWAY,
            "invalid_backend_response",
            "Image runtime returned an invalid image response",
        );
    }
    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_fields_batching_and_invalid_sizes_before_backend_admission() {
        for extra in [
            json!({"n": 2}),
            json!({"seed": -1}),
            json!({"seed": true}),
            json!({"size":"4096x4096"}),
            json!({"prompt":"  "}),
            json!({"steps":30}),
        ] {
            let mut body = json!({"model":"image", "prompt":"a watercolor bird"});
            body.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            assert!(validate(&body).is_err(), "{body}");
        }
        assert!(validate(
            &json!({"model":"image", "prompt":"a watercolor bird", "size":"768x1024", "seed":0})
        )
        .is_ok());
    }
}
