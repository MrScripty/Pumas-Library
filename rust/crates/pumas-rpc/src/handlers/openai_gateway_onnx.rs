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
    OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxRerankRequest, OnnxRerankResponse,
    OnnxRuntimeError, OnnxRuntimeErrorCode, OpenAiGatewayEndpoint,
};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use tracing::{debug, warn};

// Request-scoped fixture sequencing only. It neither grants admission nor
// replaces the real session-manager queue or backend lookup.
#[cfg(test)]
tokio::task_local! {
    pub(super) static TEST_BEFORE_ONNX_EMBED: std::sync::Arc<
        dyn Fn() -> futures::future::BoxFuture<'static, ()> + Send + Sync
    >;
}

/// Route a gateway request to the in-process ONNX session for its endpoint.
pub(crate) async fn handle_onnx_request(
    state: &AppState,
    served: &ServedModelStatus,
    requested_model: &str,
    endpoint: OpenAiGatewayEndpoint,
    body: Value,
    admission: Option<&AtomicBool>,
) -> Response {
    match endpoint {
        OpenAiGatewayEndpoint::Rerank => {
            handle_onnx_rerank(state, served, requested_model, body, admission).await
        }
        _ => handle_onnx_embedding(state, served, requested_model, endpoint, body, admission).await,
    }
}

/// Score a Jina/Cohere-style rerank request on a cross-encoder session. The
/// response matches llama.cpp's `/v1/rerank`: results most relevant first
/// (ties keep document order), cut to `top_n`.
async fn handle_onnx_rerank(
    state: &AppState,
    served: &ServedModelStatus,
    requested_model: &str,
    body: Value,
    admission: Option<&AtomicBool>,
) -> Response {
    let (request, top_n) = match parse_rerank_request(served, &body) {
        Ok(parsed) => parsed,
        Err(error) => return error.into_response(),
    };
    debug!(
        provider = "onnx_runtime",
        model_id = %served.model_id,
        gateway_model = %requested_model,
        documents = request.documents.len(),
        "routing ONNX rerank request through in-process session manager"
    );
    match state
        .onnx_session_manager
        .rerank_with_admission(request, admission)
        .await
    {
        Ok(response) => Json(rerank_response(response, requested_model, top_n)).into_response(),
        Err(error) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %served.model_id,
                gateway_model = %requested_model,
                error_code = ?error.code,
                "ONNX rerank gateway request failed"
            );
            onnx_runtime_error_response(error)
        }
    }
}

const RERANK_FIELDS: &[&str] = &["model", "query", "documents", "top_n"];

fn parse_rerank_request(
    served: &ServedModelStatus,
    body: &Value,
) -> Result<(OnnxRerankRequest, Option<usize>), GatewayRequestError> {
    let object = body
        .as_object()
        .ok_or_else(|| GatewayRequestError::invalid_request("rerank request must be an object"))?;
    if let Some(unknown) = object
        .keys()
        .find(|key| !RERANK_FIELDS.contains(&key.as_str()))
    {
        return Err(GatewayRequestError::invalid_request(format!(
            "rerank request field `{unknown}` is not supported by the ONNX adapter"
        )));
    }
    let query = object
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| GatewayRequestError::invalid_request("`query` must be a string"))?
        .to_string();
    let documents = object
        .get("documents")
        .and_then(Value::as_array)
        .ok_or_else(|| GatewayRequestError::invalid_request("`documents` must be an array"))?
        .iter()
        .map(|document| {
            document.as_str().map(str::to_string).ok_or_else(|| {
                GatewayRequestError::invalid_request(
                    "the ONNX adapter reranks text documents; each document must be a string",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let top_n = match object.get("top_n") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .filter(|top_n| *top_n > 0)
                .and_then(|top_n| usize::try_from(top_n).ok())
                .ok_or_else(|| {
                    GatewayRequestError::invalid_request("`top_n` must be a positive integer")
                })?,
        ),
    };
    let request = OnnxRerankRequest::parse(served.model_id.as_str(), query, documents)
        .map_err(|error| GatewayRequestError::invalid_request(error.message))?;
    Ok((request, top_n))
}

fn rerank_response(
    response: OnnxRerankResponse,
    requested_model: &str,
    top_n: Option<usize>,
) -> Value {
    let mut results = response.results;
    results.sort_by(|left, right| right.relevance_score.total_cmp(&left.relevance_score));
    if let Some(top_n) = top_n {
        results.truncate(top_n);
    }
    json!({
        "model": requested_model,
        "object": "list",
        "results": results
            .into_iter()
            .map(|result| json!({"index": result.index, "relevance_score": result.relevance_score}))
            .collect::<Vec<_>>(),
        "usage": {
            "prompt_tokens": response.usage.prompt_tokens,
            "total_tokens": response.usage.total_tokens,
        },
    })
}

async fn handle_onnx_embedding(
    state: &AppState,
    served: &ServedModelStatus,
    requested_model: &str,
    endpoint: OpenAiGatewayEndpoint,
    body: Value,
    admission: Option<&AtomicBool>,
) -> Response {
    if endpoint != OpenAiGatewayEndpoint::Embeddings {
        return openai_error_response_with_code(
            StatusCode::BAD_REQUEST,
            ModelServeErrorCode::EndpointUnavailable,
            "ONNX Runtime supports /v1/embeddings and /v1/rerank through the gateway",
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

    #[cfg(test)]
    if let Ok(before) = TEST_BEFORE_ONNX_EMBED.try_with(|before| before()) {
        before.await;
    }
    match state
        .onnx_session_manager
        .embed_with_admission(request, admission)
        .await
    {
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
    let (status, code, message) = match error.code {
        // Validation messages are Pumas-authored and bounded; they carry the
        // reason (for example a token limit or the session's task).
        OnnxRuntimeErrorCode::Validation => (
            StatusCode::BAD_REQUEST,
            ModelServeErrorCode::InvalidRequest,
            error.message,
        ),
        OnnxRuntimeErrorCode::NotLoaded => (
            StatusCode::NOT_FOUND,
            ModelServeErrorCode::ModelNotFound,
            "The requested model is not loaded.".to_string(),
        ),
        OnnxRuntimeErrorCode::Backend => (
            StatusCode::BAD_GATEWAY,
            ModelServeErrorCode::ProviderLoadFailed,
            "The ONNX provider could not complete the request.".to_string(),
        ),
    };
    openai_error_response_with_code(status, code, message)
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
