//! ONNX Runtime OpenAI-compatible gateway adapter.

use super::openai_gateway::openai_error_response_with_code;
use crate::server::AppState;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use pumas_library::models::{ModelServeErrorCode, ServedModelStatus};
use pumas_library::{
    OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxRuntimeError, OnnxRuntimeErrorCode,
    OpenAiGatewayEndpoint,
};
use serde_json::{json, Value};
use tracing::{debug, warn};

pub(crate) async fn handle_onnx_embedding(
    state: &AppState,
    served: &ServedModelStatus,
    requested_model: &str,
    endpoint: OpenAiGatewayEndpoint,
    body: Value,
) -> Response {
    if endpoint != OpenAiGatewayEndpoint::Embeddings {
        return openai_error_response_with_code(
            StatusCode::BAD_REQUEST,
            ModelServeErrorCode::EndpointUnavailable,
            "ONNX Runtime only supports /v1/embeddings through the gateway",
        );
    }

    let request = match parse_openai_embedding_request(served, &body) {
        Ok(request) => request,
        Err(error) => return error.into_response(),
    };
    debug!(
        provider = "onnx_runtime",
        model_id = %served.model_id,
        gateway_model = %requested_model,
        profile_id = %served.profile_id.as_str(),
        input_count = request.input.len(),
        dimensions = ?request.dimensions,
        "routing ONNX embedding request through in-process session manager"
    );

    match state.onnx_session_manager.embed(request).await {
        Ok(response) => Json(openai_embedding_response(response, requested_model)).into_response(),
        Err(error) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %served.model_id,
                gateway_model = %requested_model,
                profile_id = %served.profile_id.as_str(),
                error_code = ?error.code,
                "ONNX embedding gateway request failed"
            );
            onnx_runtime_error_response(error)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GatewayRequestError {
    status: StatusCode,
    code: ModelServeErrorCode,
    message: String,
}

impl GatewayRequestError {
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: ModelServeErrorCode::InvalidRequest,
            message: message.into(),
        }
    }

    fn into_response(self) -> Response {
        openai_error_response_with_code(self.status, self.code, self.message)
    }
}

fn parse_openai_embedding_request(
    served: &ServedModelStatus,
    body: &Value,
) -> Result<OnnxEmbeddingRequest, GatewayRequestError> {
    let input = parse_openai_embedding_input(body.get("input"))?;
    let dimensions = parse_openai_embedding_dimensions(body.get("dimensions"))?;
    validate_openai_embedding_encoding(body.get("encoding_format"))?;
    OnnxEmbeddingRequest::parse(served.model_id.as_str(), input, dimensions)
        .map_err(|error| GatewayRequestError::invalid_request(error.to_string()))
}

fn parse_openai_embedding_input(input: Option<&Value>) -> Result<Vec<String>, GatewayRequestError> {
    match input {
        Some(Value::String(value)) => Ok(vec![value.clone()]),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(str::to_string).ok_or_else(|| {
                    GatewayRequestError::invalid_request(
                        "embedding input arrays must contain strings",
                    )
                })
            })
            .collect(),
        Some(_) => Err(GatewayRequestError::invalid_request(
            "embedding input must be a string or an array of strings",
        )),
        None => Err(GatewayRequestError::invalid_request(
            "request body must include an input field",
        )),
    }
}

fn parse_openai_embedding_dimensions(
    dimensions: Option<&Value>,
) -> Result<Option<usize>, GatewayRequestError> {
    match dimensions {
        Some(Value::Null) | None => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| {
                GatewayRequestError::invalid_request(
                    "dimensions must be a positive integer when provided",
                )
            }),
        Some(_) => Err(GatewayRequestError::invalid_request(
            "dimensions must be a positive integer when provided",
        )),
    }
}

fn validate_openai_embedding_encoding(
    encoding_format: Option<&Value>,
) -> Result<(), GatewayRequestError> {
    match encoding_format {
        Some(Value::String(value)) if value == "float" => Ok(()),
        Some(Value::Null) | None => Ok(()),
        Some(_) => Err(GatewayRequestError::invalid_request(
            "ONNX Runtime embeddings currently support only float encoding",
        )),
    }
}

fn openai_embedding_response(response: OnnxEmbeddingResponse, requested_model: &str) -> Value {
    json!({
        "object": "list",
        "data": response
            .data
            .into_iter()
            .map(|embedding| {
                json!({
                    "object": "embedding",
                    "embedding": embedding.embedding,
                    "index": embedding.index
                })
            })
            .collect::<Vec<_>>(),
        "model": requested_model,
        "usage": {
            "prompt_tokens": response.usage.prompt_tokens,
            "total_tokens": response.usage.total_tokens
        }
    })
}

fn onnx_runtime_error_response(error: OnnxRuntimeError) -> Response {
    let (status, code) = match error.code {
        OnnxRuntimeErrorCode::Validation => {
            (StatusCode::BAD_REQUEST, ModelServeErrorCode::InvalidRequest)
        }
        OnnxRuntimeErrorCode::NotLoaded => {
            (StatusCode::NOT_FOUND, ModelServeErrorCode::ModelNotFound)
        }
        OnnxRuntimeErrorCode::Backend => (
            StatusCode::BAD_GATEWAY,
            ModelServeErrorCode::ProviderLoadFailed,
        ),
    };
    openai_error_response_with_code(status, code, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::models::{
        RuntimeDeviceMode, RuntimeProfileId, RuntimeProviderId, ServedModelLoadState,
    };
    use pumas_library::{OnnxEmbedding, OnnxEmbeddingUsage};

    fn onnx_status() -> ServedModelStatus {
        ServedModelStatus {
            model_id: "embeddings/nomic".to_string(),
            model_alias: Some("nomic".to_string()),
            provider: RuntimeProviderId::OnnxRuntime,
            profile_id: RuntimeProfileId::parse("onnx-cpu").unwrap(),
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: Some(8),
            keep_loaded: true,
            endpoint_url: None,
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        }
    }

    #[test]
    fn parse_openai_embedding_request_accepts_string_input() {
        let request = parse_openai_embedding_request(
            &onnx_status(),
            &json!({
                "model": "nomic",
                "input": "search_document: hello",
                "dimensions": 4,
                "encoding_format": "float"
            }),
        )
        .unwrap();

        assert_eq!(request.model_id.as_str(), "embeddings/nomic");
        assert_eq!(request.input, vec!["search_document: hello"]);
        assert_eq!(request.dimensions, Some(4));
    }

    #[test]
    fn parse_openai_embedding_request_rejects_token_arrays() {
        let error = parse_openai_embedding_request(
            &onnx_status(),
            &json!({
                "model": "nomic",
                "input": [[1, 2, 3]]
            }),
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code, ModelServeErrorCode::InvalidRequest);
        assert!(error.message.contains("arrays must contain strings"));
    }

    #[test]
    fn parse_openai_embedding_request_rejects_base64_encoding() {
        let error = parse_openai_embedding_request(
            &onnx_status(),
            &json!({
                "model": "nomic",
                "input": ["hello"],
                "encoding_format": "base64"
            }),
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("float encoding"));
    }

    #[test]
    fn openai_embedding_response_uses_gateway_model_and_shape() {
        let response = openai_embedding_response(
            OnnxEmbeddingResponse {
                model: "embeddings/nomic".to_string(),
                data: vec![OnnxEmbedding {
                    index: 0,
                    embedding: vec![0.25, 0.5],
                }],
                usage: OnnxEmbeddingUsage {
                    prompt_tokens: 2,
                    total_tokens: 2,
                },
            },
            "nomic",
        );

        assert_eq!(response["object"], "list");
        assert_eq!(response["model"], "nomic");
        assert_eq!(response["data"][0]["object"], "embedding");
        assert_eq!(response["data"][0]["embedding"], json!([0.25, 0.5]));
        assert_eq!(response["usage"]["total_tokens"], 2);
    }
}
