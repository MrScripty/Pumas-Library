//! Torch text embeddings and rerank through the gateway.
//!
//! The Torch sidecar requires its owner token, so these endpoints go through
//! the authenticated [`TorchClient`] instead of the plain proxy. Responses take
//! llama.cpp's `/v1/embeddings` and `/v1/rerank` shapes, which the typed model
//! operations already project and check.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use pumas_app_manager::torch_client::TorchOperationError;
use pumas_library::models::{ModelServeErrorCode, ServedModelStatus};
use pumas_library::OpenAiGatewayEndpoint;
use serde_json::Value;

use super::openai_gateway::{check_live_torch_runtime, openai_error_response_with_code, TorchWork};
use super::openai_gateway_onnx::{
    parse_openai_embedding_dimensions, parse_openai_embedding_input, parse_rerank_fields,
    sorted_rerank_response, validate_openai_embedding_encoding, GatewayRequestError,
};
use crate::server::AppState;

pub(super) async fn handle_torch_text(
    state: &AppState,
    served: &ServedModelStatus,
    requested_model: &str,
    endpoint: OpenAiGatewayEndpoint,
    body: Value,
) -> Response {
    let work = match endpoint {
        OpenAiGatewayEndpoint::Embeddings => TorchWork::TextEmbedding,
        OpenAiGatewayEndpoint::Rerank => TorchWork::TextRerank,
        _ => {
            return openai_error_response_with_code(
                StatusCode::BAD_REQUEST,
                ModelServeErrorCode::EndpointUnavailable,
                "this Torch text model serves /v1/embeddings or /v1/rerank",
            )
        }
    };
    let Some(endpoint_url) = served.endpoint_url.as_ref() else {
        return unavailable("the Torch runtime for this model has no endpoint");
    };
    let client = match check_live_torch_runtime(state, served, endpoint_url, work).await {
        Ok(client) => client,
        Err(_) => {
            return unavailable(
                "the Torch runtime for this model is not ready for this operation; serve the model again",
            )
        }
    };
    match work {
        TorchWork::TextEmbedding => {
            let parsed = (|| {
                validate_openai_embedding_encoding(body.get("encoding_format"))?;
                let input = parse_openai_embedding_input(body.get("input"))?;
                let dimensions = parse_openai_embedding_dimensions(body.get("dimensions"))?
                    .map(|dimensions| {
                        u32::try_from(dimensions).map_err(|_| {
                            GatewayRequestError::invalid_request("dimensions is too large")
                        })
                    })
                    .transpose()?;
                Ok::<_, GatewayRequestError>((input, dimensions))
            })();
            let (input, dimensions) = match parsed {
                Ok(parsed) => parsed,
                Err(error) => return error.into_response(),
            };
            match client
                .embed_text(&served.model_id, &input, dimensions)
                .await
            {
                Ok(mut result) => {
                    result["model"] = Value::String(requested_model.to_string());
                    Json(result).into_response()
                }
                Err(error) => torch_error_response(error),
            }
        }
        _ => {
            let (query, documents, top_n) = match parse_rerank_fields(&body, "Torch") {
                Ok(parsed) => parsed,
                Err(error) => return error.into_response(),
            };
            match client
                .rerank_text(&served.model_id, &query, &documents)
                .await
            {
                Ok(result) => match scored(&result, documents.len()) {
                    Some(scores) => Json(sorted_rerank_response(
                        requested_model,
                        scores,
                        result["usage"]["prompt_tokens"].clone(),
                        top_n,
                    ))
                    .into_response(),
                    None => torch_error_response(TorchOperationError::InvalidBackendResult),
                },
                Err(error) => torch_error_response(error),
            }
        }
    }
}

/// One finite score per document, by index, or `None` when the sidecar's
/// result is incoherent.
fn scored(result: &Value, documents: usize) -> Option<Vec<(usize, f64)>> {
    let rows = result.get("results")?.as_array()?;
    if rows.len() != documents {
        return None;
    }
    let mut seen = vec![false; documents];
    rows.iter()
        .map(|row| {
            let index = usize::try_from(row.get("index")?.as_u64()?).ok()?;
            let score = row.get("relevance_score")?.as_f64()?;
            let slot = seen.get_mut(index)?;
            if *slot || !score.is_finite() {
                return None;
            }
            *slot = true;
            Some((index, score))
        })
        .collect()
}

fn unavailable(message: &str) -> Response {
    openai_error_response_with_code(
        StatusCode::SERVICE_UNAVAILABLE,
        ModelServeErrorCode::EndpointUnavailable,
        message,
    )
}

fn torch_error_response(error: TorchOperationError) -> Response {
    match error {
        TorchOperationError::InvalidRequest(reason) => openai_error_response_with_code(
            StatusCode::BAD_REQUEST,
            ModelServeErrorCode::InvalidRequest,
            reason,
        ),
        TorchOperationError::UnsupportedModel => openai_error_response_with_code(
            StatusCode::BAD_REQUEST,
            ModelServeErrorCode::EndpointUnavailable,
            "the served Torch model does not perform this operation",
        ),
        TorchOperationError::ModelUnavailable => {
            unavailable("the model is not loaded in its Torch runtime")
        }
        TorchOperationError::RuntimeBusy => {
            unavailable("the Torch runtime is busy with another operation on this device; retry")
        }
        failure => openai_error_response_with_code(
            StatusCode::BAD_GATEWAY,
            ModelServeErrorCode::ProviderLoadFailed,
            format!(
                "the Torch runtime could not complete the request ({}); inspect its runtime log",
                failure.code()
            ),
        ),
    }
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sidecar_scores_must_cover_every_document_once_with_finite_values() {
        let result = json!({"results": [{"index": 1, "relevance_score": 0.2}, {"index": 0, "relevance_score": 0.9}]});
        assert_eq!(scored(&result, 2), Some(vec![(1, 0.2), (0, 0.9)]));
        assert_eq!(scored(&result, 3), None);
        let repeated = json!({"results": [{"index": 0, "relevance_score": 0.2}, {"index": 0, "relevance_score": 0.9}]});
        assert_eq!(scored(&repeated, 2), None);
        let outside = json!({"results": [{"index": 2, "relevance_score": 0.2}]});
        assert_eq!(scored(&outside, 1), None);
        let missing = json!({"results": [{"index": 0}]});
        assert_eq!(scored(&missing, 1), None);
    }

    #[test]
    fn caller_input_refused_by_the_sidecar_is_a_bad_request_with_its_reason() {
        let response = torch_error_response(TorchOperationError::InvalidRequest(
            "documents: List should have at least 1 item after validation, not 0".into(),
        ));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            torch_error_response(TorchOperationError::BackendFailure).status(),
            StatusCode::BAD_GATEWAY
        );
    }
}
