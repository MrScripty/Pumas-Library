//! Additive selected-model capabilities and closed typed operations.
mod image_input;
mod modality;
mod projection;
mod stream;
#[cfg(test)]
mod tests;
pub(super) mod types;
pub(super) mod vision;
#[cfg(test)]
mod vision_tests;

use self::types::*;
use super::openai_gateway::{self, OpenAiServedModelLookup};
use crate::{http_transport::RequestDisconnect, server::AppState};
use axum::{
    body::{to_bytes, Bytes},
    extract::{OriginalUri, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use pumas_library::{
    models::{RuntimeProviderId, ServedModelStatus},
    OpenAiGatewayEndpoint,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

fn error(status: StatusCode, id: Option<String>, code: ErrorCode, admitted: bool) -> Response {
    (
        status,
        Json(ErrorResponse {
            contract_version: CONTRACT_VERSION,
            request_id: id,
            error: OperationError {
                code,
                outcome: if admitted {
                    Outcome::Unknown
                } else {
                    Outcome::NotAdmitted
                },
            },
        }),
    )
        .into_response()
}
async fn selected(
    state: &AppState,
    model: &str,
    profile: Option<&str>,
) -> Result<Box<ServedModelStatus>, ErrorCode> {
    match openai_gateway::find_selected_served_model(state, model, profile).await {
        Ok(OpenAiServedModelLookup::Found(model)) => Ok(model),
        Ok(OpenAiServedModelLookup::NotFound) => Err(ErrorCode::ModelNotFound),
        Ok(OpenAiServedModelLookup::Ambiguous { .. }) => Err(ErrorCode::AmbiguousModel),
        _ => Err(ErrorCode::CapabilityUnavailable),
    }
}
pub async fn handle_capabilities(
    State(state): State<Arc<AppState>>,
    query: Result<Query<CapabilityQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                None,
                ErrorCode::InvalidRequest,
                false,
            )
        }
    };
    if !projection::valid_model(&query.model)
        || query
            .profile
            .as_ref()
            .is_some_and(|p| pumas_library::models::RuntimeProfileId::parse(p).is_err())
    {
        return error(
            StatusCode::BAD_REQUEST,
            None,
            ErrorCode::InvalidRequest,
            false,
        );
    }
    let served = match selected(&state, &query.model, query.profile.as_deref()).await {
        Ok(model) => model,
        Err(code) => return error(status_for(code), None, code, false),
    };
    Json(CapabilitiesResponse {
        supported_contract_versions: vec![CONTRACT_VERSION],
        model: query.model,
        profile: served.profile_id.as_str().to_owned(),
        max_request_bytes: MAX_BYTES,
        max_response_bytes: MAX_BYTES,
        max_stream_event_bytes: MAX_EVENT_BYTES,
        capabilities: descriptors(&state, &served).await,
    })
    .into_response()
}
fn status_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::ModelNotFound => StatusCode::NOT_FOUND,
        ErrorCode::AmbiguousModel | ErrorCode::AmbiguousOperation => StatusCode::CONFLICT,
        ErrorCode::UnsupportedModality => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorCode::CapabilityUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::RequestLimit => StatusCode::PAYLOAD_TOO_LARGE,
        ErrorCode::TransportLost => StatusCode::BAD_GATEWAY,
        _ => StatusCode::BAD_REQUEST,
    }
}

pub async fn handle_model_operations(
    State(state): State<Arc<AppState>>,
    disconnect: Option<Extension<RequestDisconnect>>,
    bytes: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(rejection) => {
            let status = rejection.status();
            return error(
                status,
                None,
                if status == StatusCode::PAYLOAD_TOO_LARGE {
                    ErrorCode::RequestLimit
                } else {
                    ErrorCode::InvalidRequest
                },
                false,
            );
        }
    };
    if bytes.len() > MAX_BYTES {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            None,
            ErrorCode::RequestLimit,
            false,
        );
    }
    // A supplied capability selects the legacy parser even if malformed. This
    // prevents a rejected legacy request from falling back to another contract.
    // Deserialize the chosen DTO from the original bytes so duplicate fields
    // remain invalid rather than being collapsed by the routing-only Value.
    let capability_present = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(value) => value.get("capability").is_some(),
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                None,
                ErrorCode::InvalidRequest,
                false,
            )
        }
    };
    let (request, served, body, declarations) = if capability_present {
        let request: OperationRequest = match serde_json::from_slice(&bytes) {
            Ok(request) => request,
            Err(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    None,
                    ErrorCode::InvalidRequest,
                    false,
                )
            }
        };
        let id = Some(request.request_id.clone());
        let body = match prepare_provider_request(&request, &state, disconnect.as_ref()).await {
            Ok(body) => body,
            Err(code) => return error(status_for(code), id, code, false),
        };
        let served = match selected(&state, &request.model, request.profile.as_deref()).await {
            Ok(model) => model,
            Err(code) => return error(status_for(code), id, code, false),
        };
        (request, served, body, None)
    } else {
        let request: modality::ModalityRequest = match serde_json::from_slice(&bytes) {
            Ok(request) => request,
            Err(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    None,
                    ErrorCode::InvalidRequest,
                    false,
                )
            }
        };
        let id = Some(request.request_id.clone());
        if let Err(code) = request.validate() {
            return error(status_for(code), id, code, false);
        }
        // Decode image envelopes and check finite options before any backend
        // discovery. Preparing a provider body never authorizes execution.
        let image_body = match request.image_preflight() {
            Ok(Some(image_request)) => {
                match prepare_provider_request(&image_request, &state, disconnect.as_ref()).await {
                    Ok(body) => Some(body),
                    Err(code) => return error(status_for(code), id, code, false),
                }
            }
            Ok(None) => None,
            Err(code) => return error(status_for(code), id, code, false),
        };
        let served = match selected(&state, &request.model, request.profile.as_deref()).await {
            Ok(model) => model,
            Err(code) => return error(status_for(code), id, code, false),
        };
        let declarations = match operation_descriptors(&state, &served, disconnect.as_ref()).await {
            Ok(declarations) => declarations,
            Err(code) => return error(status_for(code), id, code, false),
        };
        let request = match request.resolve(&declarations) {
            Ok(request) => request,
            Err(code) => return error(status_for(code), id, code, false),
        };
        let body = match image_body {
            Some(body) if request.capability == Capability::ImageToText => body,
            _ => match projection::provider_request(&request) {
                Ok(body) => body,
                Err(code) => return error(status_for(code), id, code, false),
            },
        };
        (request, served, body, Some(declarations))
    };
    let id = Some(request.request_id.clone());
    let available = if request.capability == Capability::AudioTranscription {
        served.provider == RuntimeProviderId::Torch
            && state
                .api
                .owned_audio_endpoint(&served.profile_id, &served.model_id)
                .is_some()
    } else {
        // Modality resolution already observed these declarations. Reuse that
        // observation here; the gateway still performs its final live guard.
        match declarations {
            Some(declarations) => declarations,
            None => match operation_descriptors(&state, &served, disconnect.as_ref()).await {
                Ok(declarations) => declarations,
                Err(code) => return error(status_for(code), id, code, false),
            },
        }
        .into_iter()
        .any(|d| d.capability == request.capability && d.availability.available())
    };
    if !available {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            id,
            ErrorCode::CapabilityUnavailable,
            false,
        );
    }
    if request.capability == Capability::AudioTranscription {
        return dispatch_audio(state, disconnect, &request, &served, body).await;
    }
    let admitted = AtomicBool::new(false);
    let cancellation = Mutex::new(None);
    let admission = openai_gateway::OperationAdmission {
        marker: &admitted,
        expected: &served,
        cancellation: &cancellation,
        vision: request.capability == Capability::ImageToText,
    };
    let provider_bytes = serde_json::to_vec(&body).expect("closed request is serializable");
    let path = OriginalUri(
        request
            .capability
            .path()
            .expect("qualified adapter has a route")
            .parse()
            .expect("static route"),
    );
    let disconnect_signal = disconnect.as_ref().map(|Extension(signal)| signal.clone());
    // Native image/embedding adapters share downstream cancellation while text
    // transport retains its own exact-session stop and admission-permit custody.
    let dispatch = openai_gateway::dispatch_openai(
        state.clone(),
        path,
        disconnect,
        Bytes::from(provider_bytes),
        request.profile.as_deref(),
        Some(&admission),
    );
    let response = tokio::select! {
        biased;
        ()=state.shutdown_request.clone().requested()=>return error(StatusCode::BAD_GATEWAY,id,ErrorCode::TransportLost,admitted.load(Ordering::Acquire)),
        ()=async {match disconnect_signal {Some(signal)=>signal.disconnected().await,None=>std::future::pending().await}}=>return error(StatusCode::BAD_GATEWAY,id,ErrorCode::TransportLost,admitted.load(Ordering::Acquire)),
        response=dispatch=>response,
    };
    if !response.status().is_success() {
        let was_admitted = admitted.load(Ordering::Acquire);
        return error(
            if was_admitted {
                StatusCode::BAD_GATEWAY
            } else {
                response.status()
            },
            id,
            if was_admitted {
                ErrorCode::ProviderFailure
            } else {
                ErrorCode::CapabilityUnavailable
            },
            was_admitted,
        );
    }
    if request.stream {
        return stream::response(
            response,
            &request,
            served.profile_id.as_str(),
            cancellation
                .into_inner()
                .expect("request-local cancellation mutex"),
        );
    }
    let bytes = match to_bytes(response.into_body(), MAX_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return error(
                StatusCode::BAD_GATEWAY,
                id,
                ErrorCode::ResponseLimit,
                admitted.load(Ordering::Acquire),
            )
        }
    };
    let result = serde_json::from_slice(&bytes)
        .map_err(|_| ErrorCode::InvalidProviderResult)
        .and_then(|value: serde_json::Value| {
            if request.capability == Capability::ImageToText
                && value.get("model").and_then(serde_json::Value::as_str)
                    != Some(served.model_id.as_str())
            {
                return Err(ErrorCode::InvalidProviderResult);
            }
            projection::result(&request, value)
        });
    match result {
        Ok(result) => {
            let response = OperationResponse {
                contract_version: CONTRACT_VERSION,
                request_id: request.request_id,
                result,
            };
            let bytes = serde_json::to_vec(&response).expect("closed result is serializable");
            if bytes.len() > MAX_BYTES {
                return error(
                    StatusCode::BAD_GATEWAY,
                    id,
                    ErrorCode::ResponseLimit,
                    admitted.load(Ordering::Acquire),
                );
            }
            (
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                bytes,
            )
                .into_response()
        }
        Err(code) => error(
            StatusCode::BAD_GATEWAY,
            id,
            code,
            admitted.load(Ordering::Acquire),
        ),
    }
}

async fn prepare_provider_request(
    request: &OperationRequest,
    state: &AppState,
    disconnect: Option<&Extension<RequestDisconnect>>,
) -> Result<serde_json::Value, ErrorCode> {
    if request.capability != Capability::ImageToText {
        return projection::provider_request(request);
    }
    use std::sync::OnceLock;
    static DECODERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let permit = DECODERS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| ErrorCode::CapabilityUnavailable)?;
    let request = request.clone();
    let decode = tokio::task::spawn_blocking(move || {
        // A running codec cannot be asynchronously interrupted. It retains its
        // memory and capacity share through its bounded, effect-free decode.
        let _permit = permit;
        projection::provider_request(&request)
    });
    tokio::select! {
        biased;
        () = state.shutdown_request.clone().requested() => Err(ErrorCode::TransportLost),
        () = async { match disconnect { Some(Extension(signal)) => signal.clone().disconnected().await, None => std::future::pending().await } } => Err(ErrorCode::TransportLost),
        result = decode => result.map_err(|_| ErrorCode::InvalidRequest)?,
    }
}

async fn operation_descriptors(
    state: &AppState,
    served: &ServedModelStatus,
    disconnect: Option<&Extension<RequestDisconnect>>,
) -> Result<Vec<CapabilityDescriptor>, ErrorCode> {
    tokio::select! {
        biased;
        () = state.shutdown_request.clone().requested() => Err(ErrorCode::TransportLost),
        () = async { match disconnect { Some(Extension(signal)) => signal.clone().disconnected().await, None => std::future::pending().await } } => Err(ErrorCode::TransportLost),
        declarations = descriptors(state, served) => Ok(declarations),
    }
}

async fn dispatch_audio(
    state: Arc<AppState>,
    disconnect: Option<Extension<RequestDisconnect>>,
    request: &OperationRequest,
    served: &ServedModelStatus,
    mut body: serde_json::Value,
) -> Response {
    use pumas_library::runtime_profiles::OwnedAudioEndpointError;
    let id = Some(request.request_id.clone());
    let Some(endpoint) = state
        .api
        .owned_audio_endpoint(&served.profile_id, &served.model_id)
    else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            id,
            ErrorCode::CapabilityUnavailable,
            false,
        );
    };
    // Canonical selection is fixed by the original owning load, not the alias.
    body["model"] = serde_json::json!(served.model_id);
    body["profile"] = serde_json::json!(served.profile_id.as_str());
    let admitted = Arc::new(AtomicBool::new(false));
    let result = tokio::select! {
        biased;
        () = state.shutdown_request.clone().requested() => {
            return error(StatusCode::BAD_GATEWAY, id, ErrorCode::TransportLost, admitted.load(Ordering::Acquire));
        }
        () = async { match disconnect { Some(Extension(signal)) => signal.disconnected().await, None => std::future::pending().await } } => {
            return error(StatusCode::BAD_GATEWAY, id, ErrorCode::TransportLost, admitted.load(Ordering::Acquire));
        }
        result = endpoint.execute(body, admitted.clone()) => result,
    };
    match result {
        Ok(result) => Json(OperationResponse {
            contract_version: CONTRACT_VERSION,
            request_id: request.request_id.clone(),
            result: OperationResult::Text {
                text: result.text,
                finish_reason: if result.length_limited {
                    FinishReason::Length
                } else {
                    FinishReason::Stop
                },
            },
        })
        .into_response(),
        Err(OwnedAudioEndpointError::NotAdmitted) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            id,
            ErrorCode::CapabilityUnavailable,
            false,
        ),
        Err(OwnedAudioEndpointError::InvalidRequest) => error(
            StatusCode::BAD_REQUEST,
            id,
            ErrorCode::InvalidRequest,
            false,
        ),
        Err(OwnedAudioEndpointError::UnsupportedContract) => error(
            StatusCode::BAD_REQUEST,
            id,
            ErrorCode::UnsupportedContract,
            false,
        ),
        Err(OwnedAudioEndpointError::ProviderFailure) => error(
            StatusCode::BAD_GATEWAY,
            id,
            ErrorCode::ProviderFailure,
            true,
        ),
        Err(OwnedAudioEndpointError::TransportLost) => error(
            StatusCode::BAD_GATEWAY,
            id,
            ErrorCode::TransportLost,
            admitted.load(Ordering::Acquire),
        ),
    }
}

async fn descriptors(state: &AppState, served: &ServedModelStatus) -> Vec<CapabilityDescriptor> {
    // Package evidence supplies semantics; an installed loader or matching modalities does not.
    let record = state
        .api
        .model_library()
        .get_model(&served.model_id)
        .await
        .ok()
        .flatten();
    let task = record
        .as_ref()
        .and_then(|r| semantic_task_evidence(&r.metadata));
    let model_type = record.as_ref().map(|r| r.model_type.as_str());
    let runtime_ready = match served.provider {
        RuntimeProviderId::OnnxRuntime => {
            state.onnx_session_manager.list().await.is_ok_and(|models| {
                models
                    .iter()
                    .any(|model| model.model_id.as_str() == served.model_id)
            })
        }
        RuntimeProviderId::Torch => match served.endpoint_url.as_ref() {
            Some(endpoint) => {
                openai_gateway::check_live_torch_image_runtime(state, served, endpoint)
                    .await
                    .is_ok()
            }
            None => false,
        },
        _ => openai_gateway::generation_session_stop(state, served)
            .await
            .is_ok(),
    };
    let image_adapter = state
        .provider_registry
        .get(served.provider)
        .is_some_and(|provider| {
            provider.supports_serving_task(pumas_library::ServingTask::ImageToText)
        })
        && vision::supported_profile(state, served).await;
    let vision_ready = image_adapter
        && runtime_ready
        && semantic_match(Capability::ImageToText, task, model_type, served.provider) == Some(true)
        && vision::ready(state, served).await;
    [
        Capability::ChatGeneration,
        Capability::TextGeneration,
        Capability::TextEmbedding,
        Capability::ImageGeneration,
        Capability::AudioTranscription,
        Capability::AudioClassification,
        Capability::ImageToText,
    ]
    .into_iter()
    .map(|capability| {
        let (semantic_task, input_formats, output_formats, endpoint, option_bounds) =
            match capability {
                Capability::ChatGeneration => (
                    SemanticTask::ChatGeneration,
                    vec![InputFormat::MessagesText],
                    vec![OutputFormat::Text],
                    Some(OpenAiGatewayEndpoint::ChatCompletions),
                    text_bounds(),
                ),
                Capability::TextGeneration => (
                    SemanticTask::TextGeneration,
                    vec![InputFormat::Text],
                    vec![OutputFormat::Text],
                    Some(OpenAiGatewayEndpoint::Completions),
                    text_bounds(),
                ),
                Capability::TextEmbedding => (
                    SemanticTask::TextEmbedding,
                    vec![InputFormat::Text, InputFormat::TextBatch],
                    vec![OutputFormat::EmbeddingsFloat32],
                    Some(OpenAiGatewayEndpoint::Embeddings),
                    vec![
                        bound(OptionName::Dimensions, 1., 8192.),
                        bound(OptionName::InputCount, 1., 128.),
                        bound(OptionName::InputCharacters, 0., 65536.),
                    ],
                ),
                Capability::ImageGeneration => (
                    SemanticTask::TextToImage,
                    vec![InputFormat::Text],
                    vec![OutputFormat::PngBase64],
                    Some(OpenAiGatewayEndpoint::ImagesGenerations),
                    vec![
                        bound(OptionName::Width, 1., u32::MAX as f64),
                        bound(OptionName::Height, 1., u32::MAX as f64),
                        bound(OptionName::Seed, 0., u32::MAX as f64),
                        bound(OptionName::InputCharacters, 1., 4000.),
                    ],
                ),
                Capability::AudioTranscription => (
                    SemanticTask::SpeechToText,
                    vec![InputFormat::PcmS16le, InputFormat::PcmF32le],
                    vec![OutputFormat::Text],
                    None,
                    vec![bound(OptionName::MaxOutputTokens, 512., 512.)],
                ),
                Capability::AudioClassification => (
                    SemanticTask::AudioClassification,
                    vec![InputFormat::PcmS16le, InputFormat::PcmF32le],
                    vec![OutputFormat::Labels],
                    None,
                    vec![],
                ),
                Capability::ImageToText => (
                    SemanticTask::ImageToText,
                    vec![
                        InputFormat::PngBase64,
                        InputFormat::JpegBase64,
                        InputFormat::MessagesImage,
                    ],
                    vec![OutputFormat::Text],
                    Some(OpenAiGatewayEndpoint::ChatCompletions),
                    vec![
                        bound(OptionName::MaxTokens, 1., 2048.),
                        bound(OptionName::Temperature, 0., 2.),
                        bound(OptionName::TopP, 0., 1.),
                        bound(OptionName::ImageBytes, 1., (8 * 1024 * 1024) as f64),
                        bound(OptionName::ImagePixels, 1., 4_194_304.),
                        bound(OptionName::ImageCount, 1., 4.),
                        bound(OptionName::Width, 1., 4096.),
                        bound(OptionName::Height, 1., 4096.),
                    ],
                ),
            };
        let adapter = endpoint.is_some_and(|endpoint| {
            state
                .provider_registry
                .get(served.provider)
                .is_some_and(|provider| provider.supports_openai_endpoint(endpoint))
        });
        let semantics = semantic_match(capability, task, model_type, served.provider);
        let reason = if capability == Capability::AudioClassification {
            Some(AvailabilityReason::UnsupportedAdapter)
        } else if capability == Capability::AudioTranscription
            && (served.provider != RuntimeProviderId::Torch
                || !state
                    .api
                    .owned_audio_endpoint(&served.profile_id, &served.model_id)
                    .is_some_and(|endpoint| endpoint.available()))
        {
            Some(AvailabilityReason::UnqualifiedAudioRuntime)
        } else if capability == Capability::AudioTranscription {
            None
        } else if !adapter || (capability == Capability::ImageToText && !image_adapter) {
            Some(AvailabilityReason::UnsupportedAdapter)
        } else if semantics == Some(false) {
            Some(AvailabilityReason::ModelTaskMismatch)
        } else if semantics.is_none() {
            Some(AvailabilityReason::UnknownModelTask)
        } else if !runtime_ready || (capability == Capability::ImageToText && !vision_ready) {
            Some(AvailabilityReason::RuntimeUnavailable)
        } else {
            None
        };
        CapabilityDescriptor {
            capability,
            semantic_task,
            input_formats,
            output_formats,
            streaming: capability.text_generation() && adapter && reason.is_none(),
            availability: reason.map_or(Availability::Available, |reason| {
                Availability::Unavailable { reason }
            }),
            option_bounds,
        }
    })
    .collect()
}
fn semantic_task_evidence(metadata: &serde_json::Value) -> Option<&str> {
    // A canonical modality signature loses distinctions such as classification.
    // Prefer the explicit source task before using its normalized signature.
    ["pipeline_tag", "task_type_primary"]
        .into_iter()
        .find_map(|key| {
            metadata
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|task| !matches!(*task, "" | "unknown" | "unknown->unknown"))
        })
}

fn semantic_match(
    capability: Capability,
    task: Option<&str>,
    model_type: Option<&str>,
    provider: RuntimeProviderId,
) -> Option<bool> {
    if let Some(task) = task.filter(|task| !matches!(*task, "unknown" | "")) {
        return Some(match capability {
            Capability::ChatGeneration | Capability::TextGeneration => {
                matches!(
                    task,
                    "text-generation"
                        | "text_generation"
                        | "text-to-text"
                        | "text2text-generation"
                        | "conversational"
                ) || (task == "text->text"
                    && matches!(model_type, Some("llm" | "language" | "text")))
            }
            Capability::TextEmbedding => matches!(
                task,
                "feature-extraction"
                    | "feature_extraction"
                    | "text-embedding"
                    | "text_embedding"
                    | "sentence-similarity"
                    | "text->embedding"
            ),
            Capability::ImageGeneration => {
                matches!(task, "text-to-image" | "text_to_image" | "text->image")
            }
            Capability::ImageToText => {
                matches!(task, "image-to-text" | "image_to_text" | "image->text")
            }
            _ => false,
        });
    }
    if provider == RuntimeProviderId::OnnxRuntime && capability == Capability::TextEmbedding {
        return Some(true);
    }
    if provider == RuntimeProviderId::Torch && capability == Capability::ImageGeneration {
        return Some(true);
    } // qualified image-only handshake and ready slot above
    match model_type {
        Some("llm" | "language" | "text") => Some(capability.text_generation()),
        Some("embedding" | "embeddings") => Some(capability == Capability::TextEmbedding),
        Some(_) => Some(false),
        None => None,
    }
}
fn bound(option: OptionName, minimum: f64, maximum: f64) -> OptionBound {
    OptionBound {
        option,
        minimum,
        maximum,
    }
}
fn text_bounds() -> Vec<OptionBound> {
    vec![
        bound(OptionName::MaxTokens, 1., u32::MAX as f64),
        bound(OptionName::Temperature, 0., 2.),
        bound(OptionName::TopP, 0., 1.),
    ]
}
