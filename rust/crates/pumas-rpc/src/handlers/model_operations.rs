//! Additive selected-model capabilities and closed typed operations.
mod decision;
mod modality;
mod parts;
mod projection;
mod runtime_modalities;
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

fn error(
    status: StatusCode,
    id: Option<String>,
    rejection: impl Into<Rejection>,
    admitted: bool,
) -> Response {
    (
        status,
        Json(ErrorResponse {
            contract_version: CONTRACT_VERSION,
            request_id: id,
            error: OperationError::new(rejection.into(), admitted),
        }),
    )
        .into_response()
}
/// Reject before admission with the status that matches the code.
fn reject(id: Option<String>, rejection: impl Into<Rejection>) -> Response {
    let rejection = rejection.into();
    error(status_for(rejection.code), id, rejection, false)
}
async fn selected(
    state: &AppState,
    model: &str,
    profile: Option<&str>,
) -> Result<Box<ServedModelStatus>, Rejection> {
    let target = match profile {
        Some(profile) => format!("model \"{model}\" on profile \"{profile}\""),
        None => format!("model \"{model}\""),
    };
    match openai_gateway::find_selected_served_model(state, model, profile).await {
        Ok(OpenAiServedModelLookup::Found(model)) => Ok(model),
        Ok(OpenAiServedModelLookup::NotFound) => Err(Rejection::new(
            ErrorCode::ModelNotFound,
            format!("no served {target}; list served models with GET /v1/served-models"),
        )),
        Ok(OpenAiServedModelLookup::Ambiguous { message, .. }) => {
            Err(Rejection::new(ErrorCode::AmbiguousModel, message))
        }
        Ok(OpenAiServedModelLookup::Unavailable) => Err(Rejection::new(
            ErrorCode::CapabilityUnavailable,
            format!("serving status for {target} is unavailable"),
        )),
        Err(_) => Err(Rejection::new(
            ErrorCode::CapabilityUnavailable,
            format!("serving status for {target} could not be read"),
        )),
    }
}
pub async fn handle_capabilities(
    State(state): State<Arc<AppState>>,
    query: Result<Query<CapabilityQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(rejection) => return reject(None, Rejection::invalid(rejection.body_text())),
    };
    if !projection::valid_model(&query.model) {
        return reject(
            None,
            Rejection::invalid("model is not a valid served alias"),
        );
    }
    if let Some(Err(rejection)) = query.profile.as_deref().map(projection::valid_profile) {
        return reject(None, rejection);
    }
    let served = match selected(&state, &query.model, query.profile.as_deref()).await {
        Ok(model) => model,
        Err(rejection) => return reject(None, rejection),
    };
    let (modalities, capabilities) = describe(&state, &served).await;
    Json(CapabilitiesResponse {
        supported_contract_versions: vec![CONTRACT_VERSION],
        model: query.model,
        profile: served.profile_id.as_str().to_owned(),
        max_request_bytes: MAX_BYTES,
        max_response_bytes: MAX_BYTES,
        max_stream_event_bytes: MAX_EVENT_BYTES,
        context_tokens: served.context_size,
        modalities,
        capabilities,
    })
    .into_response()
}

/// List every served (model, profile) pair with its current capabilities.
pub async fn handle_served_models(State(state): State<Arc<AppState>>) -> Response {
    let snapshot = match state.api.get_serving_status().await {
        Ok(response) => response.snapshot,
        Err(_) => {
            return reject(
                None,
                Rejection::new(
                    ErrorCode::CapabilityUnavailable,
                    "serving status could not be read",
                ),
            )
        }
    };
    let mut served = snapshot.served_models;
    served.sort_by(|left, right| {
        served_alias(left)
            .cmp(served_alias(right))
            .then_with(|| left.profile_id.as_str().cmp(right.profile_id.as_str()))
    });
    served.dedup_by(|right, left| {
        served_alias(left) == served_alias(right) && left.profile_id == right.profile_id
    });
    let mut alias_count = std::collections::HashMap::<String, usize>::new();
    for model in &served {
        *alias_count
            .entry(served_alias(model).to_owned())
            .or_default() += 1;
    }
    let described =
        futures::future::join_all(served.iter().map(|model| describe(&state, model))).await;
    let models = served
        .iter()
        .zip(described)
        .map(|(model, (modalities, capabilities))| ServedModelEntry {
            model: served_alias(model).to_owned(),
            profile: model.profile_id.as_str().to_owned(),
            model_id: model.model_id.clone(),
            provider: enum_name(&model.provider),
            load_state: enum_name(&model.load_state),
            context_tokens: model.context_size,
            profile_required: alias_count[served_alias(model)] > 1,
            modalities,
            capabilities,
        })
        .collect();
    Json(ServedModelsResponse {
        supported_contract_versions: vec![CONTRACT_VERSION],
        max_request_bytes: MAX_BYTES,
        max_response_bytes: MAX_BYTES,
        max_stream_event_bytes: MAX_EVENT_BYTES,
        models,
    })
    .into_response()
}
fn served_alias(model: &ServedModelStatus) -> &str {
    model.model_alias.as_deref().unwrap_or(&model.model_id)
}
fn enum_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
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
/// The caller's request ID, when the body is JSON and carries a valid one.
fn readable_request_id(value: &serde_json::Value) -> Option<String> {
    value
        .get("request_id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| projection::valid_identifier(id, 128))
        .map(str::to_owned)
}
/// Bounded reason from a gateway or provider error body (`{"error":{"message":..}}`).
async fn error_body_message(response: Response) -> Option<String> {
    const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;
    let bytes = to_bytes(response.into_body(), MAX_ERROR_BODY_BYTES)
        .await
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let message = value
        .pointer("/error/message")
        .or_else(|| value.get("error"))
        .or_else(|| value.get("message"))?
        .as_str()?
        .trim();
    (!message.is_empty()).then(|| message.to_owned())
}
/// Classify a gateway refusal that happened before provider admission. The
/// model was already selected, so a later lookup miss means the runtime lost it.
fn not_admitted_code(status: StatusCode) -> ErrorCode {
    match status {
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => ErrorCode::InvalidRequest,
        StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::RequestLimit,
        _ => ErrorCode::CapabilityUnavailable,
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
            let code = if status == StatusCode::PAYLOAD_TOO_LARGE {
                ErrorCode::RequestLimit
            } else {
                ErrorCode::InvalidRequest
            };
            return error(
                status,
                None,
                Rejection::new(code, rejection.body_text()),
                false,
            );
        }
    };
    if bytes.len() > MAX_BYTES {
        return reject(
            None,
            Rejection::new(
                ErrorCode::RequestLimit,
                format!(
                    "request body is {} bytes; the maximum is {MAX_BYTES}",
                    bytes.len()
                ),
            ),
        );
    }
    let value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(value) => value,
        Err(parse) => {
            return reject(
                None,
                Rejection::invalid(format!("request body is not valid JSON: {parse}")),
            )
        }
    };
    let id = readable_request_id(&value);
    // Deserialize from the original bytes so duplicate fields remain invalid
    // rather than being collapsed by the inspection Value above.
    let request: modality::ModalityRequest = match serde_json::from_slice(&bytes) {
        Ok(request) => request,
        Err(schema) => {
            return reject(
                id,
                Rejection::invalid(format!("request does not match the contract: {schema}")),
            )
        }
    };
    let id = Some(request.request_id.clone());
    if let Err(rejection) = request.validate() {
        return reject(id, rejection);
    }
    // Decode image envelopes and check finite options before any backend
    // discovery. Preparing a provider body never authorizes execution.
    let image_body = match request.image_preflight() {
        Ok(Some(image_request)) => {
            match prepare_provider_request(&image_request, &state, disconnect.as_ref()).await {
                Ok(body) => Some(body),
                Err(rejection) => return reject(id, rejection),
            }
        }
        Ok(None) => None,
        Err(rejection) => return reject(id, rejection),
    };
    let served = match selected(&state, &request.model, request.profile.as_deref()).await {
        Ok(model) => model,
        Err(rejection) => return reject(id, rejection),
    };
    let declarations = match operation_descriptors(&state, &served, disconnect.as_ref()).await {
        Ok(declarations) => declarations,
        Err(code) => return reject(id, code),
    };
    let request = match request.resolve(&declarations) {
        Ok(request) => request,
        Err(rejection) => return reject(id, rejection),
    };
    let body = match image_body {
        Some(body)
            if matches!(
                request.capability,
                Capability::ImageToText | Capability::MultimodalEmbedding
            ) || media_chat(&request) =>
        {
            body
        }
        // Decision images are decoded off the async runtime.
        _ if request.capability == Capability::Decision => {
            match prepare_provider_request(&request, &state, disconnect.as_ref()).await {
                Ok(body) => body,
                Err(rejection) => return reject(id, rejection),
            }
        }
        _ => match projection::provider_request(&request) {
            Ok(body) => body,
            Err(rejection) => return reject(id, rejection),
        },
    };
    if request.capability == Capability::AudioTranscription {
        return dispatch_audio(state, disconnect, &request, &served, body).await;
    }
    let admitted = AtomicBool::new(false);
    let cancellation = Mutex::new(None);
    let admission = openai_gateway::OperationAdmission {
        marker: &admitted,
        expected: &served,
        cancellation: &cancellation,
        // Non-text parts must be live in the session that receives them.
        media: modality::input_modalities(&request.input)
            .into_iter()
            .filter(|modality| *modality != Modality::Text)
            .collect(),
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
        let status = response.status();
        let reported = error_body_message(response).await;
        if was_admitted {
            let message = match reported {
                Some(reason) => format!("provider returned HTTP {}: {reason}", status.as_u16()),
                None => format!("provider returned HTTP {}", status.as_u16()),
            };
            return error(
                StatusCode::BAD_GATEWAY,
                id,
                Rejection::new(ErrorCode::ProviderFailure, message),
                true,
            );
        }
        let code = not_admitted_code(status);
        return reject(
            id,
            match reported {
                Some(reason) => Rejection::new(code, reason),
                None => code.into(),
            },
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
) -> Result<serde_json::Value, Rejection> {
    if !matches!(
        request.capability,
        Capability::ImageToText | Capability::MultimodalEmbedding | Capability::Decision
    ) && !media_chat(request)
    {
        return projection::provider_request(request);
    }
    use std::sync::OnceLock;
    static DECODERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let permit = DECODERS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            Rejection::new(
                ErrorCode::CapabilityUnavailable,
                "image decoding is at capacity; retry when an in-flight image request completes",
            )
        })?;
    let request = request.clone();
    let decode = tokio::task::spawn_blocking(move || {
        // A running codec cannot be asynchronously interrupted. It retains its
        // memory and capacity share through its bounded, effect-free decode.
        let _permit = permit;
        projection::provider_request(&request)
    });
    tokio::select! {
        biased;
        () = state.shutdown_request.clone().requested() => Err(ErrorCode::TransportLost.into()),
        () = async { match disconnect { Some(Extension(signal)) => signal.clone().disconnected().await, None => std::future::pending().await } } => Err(ErrorCode::TransportLost.into()),
        result = decode => result.map_err(|_| Rejection::invalid("image decoding failed"))?,
    }
}

/// Chat over messages with image or audio parts.
fn media_chat(request: &OperationRequest) -> bool {
    request.capability == Capability::ChatGeneration
        && matches!(request.input, OperationInput::ImageMessages { .. })
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

/// Modalities a capability's protocol can carry and those it produces. A
/// descriptor accepts the intersection with what the served model declares.
fn protocol_modalities(capability: Capability) -> (&'static [Modality], &'static [Modality]) {
    use Modality::*;
    match capability {
        Capability::ChatGeneration => (&[Text, Image, Audio], &[Text]),
        Capability::TextGeneration => (&[Text], &[Text]),
        Capability::ImageToText => (&[Text, Image], &[Text]),
        Capability::TextEmbedding => (&[Text], &[Vector]),
        Capability::MultimodalEmbedding => (&[Text, Image, Audio], &[Vector]),
        Capability::ImageGeneration => (&[Text], &[Image]),
        Capability::AudioTranscription => (&[Audio], &[Text]),
        Capability::AudioClassification => (&[Audio], &[Json]),
        Capability::Decision => (&[Text, Image, Audio], &[Json]),
        Capability::Rerank => (&[Text], &[Json]),
    }
}

async fn descriptors(state: &AppState, served: &ServedModelStatus) -> Vec<CapabilityDescriptor> {
    describe(state, served).await.1
}

/// The served model's runtime-declared modalities and its capabilities.
async fn describe(
    state: &AppState,
    served: &ServedModelStatus,
) -> (ModalitySet, Vec<CapabilityDescriptor>) {
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
    // llama.cpp derives non-text input from a projector beside the weights,
    // so a text-only package is never probed.
    let projector = served.provider == RuntimeProviderId::LlamaCpp && {
        let library = state.api.model_library().clone();
        let model_id = served.model_id.clone();
        tokio::task::spawn_blocking(move || {
            library
                .get_primary_model_file(&model_id)
                .is_some_and(|path| {
                    pumas_library::runtime_profiles::llama_cpp_model_has_projector(&path)
                })
        })
        .await
        .unwrap_or(false)
    };
    let hints = runtime_modalities::Hints {
        model_type,
        projector,
        media_task: semantic_match(Capability::ImageToText, task, model_type, served.provider)
            == Some(true)
            || matches!(task, Some("audio-text-to-text" | "any-to-any")),
        decision: decision::candidate_task(task) || model_type == Some("decision"),
    };
    let onnx_session =
        match served.provider {
            RuntimeProviderId::OnnxRuntime => state
                .onnx_session_manager
                .list()
                .await
                .ok()
                .and_then(|models| {
                    models
                        .into_iter()
                        .find(|model| model.model_id.as_str() == served.model_id)
                }),
            _ => None,
        };
    let runtime_ready = match served.provider {
        RuntimeProviderId::OnnxRuntime => onnx_session.is_some(),
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
    // A llama.cpp runtime is asked only through a current, enabled profile.
    let observation = if served.provider == RuntimeProviderId::LlamaCpp
        && !(runtime_ready && vision::supported_profile(state, served).await)
    {
        runtime_modalities::Observation::default()
    } else {
        runtime_modalities::observe(state, served, hints).await
    };
    // Without a recorded task, the runtime's own declaration of a generation
    // model decides what it can do (ADR 0005); it only ever adds a match.
    let task_known = task.is_some_and(|task| !matches!(task, "unknown" | ""));
    let declared_text_out = observation.declared.output.contains(&Modality::Text);
    let semantics_of = |capability: Capability| {
        let by_runtime = !task_known
            && observation.runtime_declared
            && !observation.decisions
            && declared_text_out
            && match capability {
                Capability::ChatGeneration | Capability::TextGeneration => true,
                Capability::ImageToText => observation.declares_input(Modality::Image),
                _ => false,
            };
        if by_runtime {
            Some(true)
        } else {
            semantic_match(capability, task, model_type, served.provider)
        }
    };
    let vision_ready = image_adapter
        && runtime_ready
        && semantics_of(Capability::ImageToText) == Some(true)
        && observation.live(Modality::Image);
    // Native decision models are a runtime fact; they answer only `decision`.
    let decision_model = runtime_ready && observation.decisions;
    // Non-text embeddings need a llama.cpp session with its projector loaded.
    let embedding_media_ready = runtime_ready
        && image_adapter
        && semantic_match(
            Capability::MultimodalEmbedding,
            task,
            model_type,
            served.provider,
        ) == Some(true)
        && (observation.live(Modality::Image) || observation.live(Modality::Audio));
    let declared_audio = observation.declares_input(Modality::Audio);
    let capabilities = [
        Capability::ChatGeneration,
        Capability::TextGeneration,
        Capability::TextEmbedding,
        Capability::ImageGeneration,
        Capability::AudioTranscription,
        Capability::AudioClassification,
        Capability::ImageToText,
        Capability::MultimodalEmbedding,
        Capability::Decision,
        Capability::Rerank,
    ]
    .into_iter()
    .map(|capability| {
        let (semantic_task, input_formats, output_formats, endpoint, option_bounds) =
            match capability {
                Capability::ChatGeneration => (
                    SemanticTask::ChatGeneration,
                    vec![InputFormat::MessagesText, InputFormat::MessagesMultimodal],
                    vec![OutputFormat::Text],
                    Some(OpenAiGatewayEndpoint::ChatCompletions),
                    media_bounds(text_bounds(served.context_size), declared_audio),
                ),
                Capability::TextGeneration => (
                    SemanticTask::TextGeneration,
                    vec![InputFormat::Text],
                    vec![OutputFormat::Text],
                    Some(OpenAiGatewayEndpoint::Completions),
                    text_bounds(served.context_size),
                ),
                Capability::TextEmbedding => (
                    SemanticTask::TextEmbedding,
                    vec![InputFormat::Text, InputFormat::TextBatch],
                    vec![OutputFormat::EmbeddingsFloat32],
                    Some(OpenAiGatewayEndpoint::Embeddings),
                    embedding_bounds(
                        served.provider,
                        onnx_session
                            .as_ref()
                            .and_then(|session| session.max_input_tokens),
                    ),
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
                    // The qualified transcription adapter generates up to its
                    // fixed native bound; projection refuses any other value.
                    vec![bound(
                        OptionName::MaxOutputTokens,
                        f64::from(AUDIO_MAX_OUTPUT_TOKENS),
                        f64::from(AUDIO_MAX_OUTPUT_TOKENS),
                    )],
                ),
                Capability::AudioClassification => (
                    SemanticTask::AudioClassification,
                    vec![InputFormat::PcmS16le, InputFormat::PcmF32le],
                    vec![OutputFormat::Labels],
                    None,
                    vec![],
                ),
                Capability::Decision => (
                    SemanticTask::Decision,
                    vec![InputFormat::Decision],
                    vec![OutputFormat::Decisions],
                    Some(OpenAiGatewayEndpoint::SystemOne),
                    media_bounds(parts::image_bounds(0, false), declared_audio),
                ),
                // Document count and pair length are the runtime's limits; a
                // pair longer than its micro-batch is refused with its reason.
                Capability::Rerank => (
                    SemanticTask::Rerank,
                    vec![InputFormat::Rerank],
                    vec![OutputFormat::RelevanceScores],
                    Some(OpenAiGatewayEndpoint::Rerank),
                    vec![bound(OptionName::TopN, 1., u32::MAX as f64)],
                ),
                Capability::MultimodalEmbedding => (
                    SemanticTask::MultimodalEmbedding,
                    vec![
                        InputFormat::PngBase64,
                        InputFormat::JpegBase64,
                        InputFormat::ContentBatch,
                    ],
                    vec![OutputFormat::EmbeddingsFloat32],
                    Some(OpenAiGatewayEndpoint::Embeddings),
                    media_bounds(
                        std::iter::once(bound(OptionName::Dimensions, 1., u32::MAX as f64))
                            .chain(parts::image_bounds(1, false))
                            .collect(),
                        declared_audio,
                    ),
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
                        bound(
                            OptionName::MaxTokens,
                            1.,
                            f64::from(served.context_size.unwrap_or(u32::MAX)),
                        ),
                        bound(OptionName::Temperature, 0., 2.),
                        bound(OptionName::TopP, 0., 1.),
                    ]
                    .into_iter()
                    .chain(parts::image_bounds(1, true))
                    .collect(),
                ),
            };
        let adapter = endpoint.is_some_and(|endpoint| {
            state
                .provider_registry
                .get(served.provider)
                .is_some_and(|provider| provider.supports_openai_endpoint(endpoint))
        });
        let semantics = match capability {
            // Unprobed while the runtime is down, so report readiness, not task.
            Capability::Decision if decision::candidate_task(task) && !runtime_ready => Some(true),
            Capability::Decision => Some(decision_model),
            _ if decision_model => Some(false),
            _ => semantics_of(capability),
        };
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
        } else if !runtime_ready
            || (capability == Capability::ImageToText && !vision_ready)
            || (capability == Capability::MultimodalEmbedding && !embedding_media_ready)
        {
            Some(AvailabilityReason::RuntimeUnavailable)
        } else {
            None
        };
        let (accepts, produces) = protocol_modalities(capability);
        let input_modalities = if observation.observed {
            accepts
                .iter()
                .copied()
                .filter(|modality| observation.declares_input(*modality))
                .collect()
        } else {
            // Undeclared runtime: the capability is unavailable for that
            // reason, and the protocol's modalities are reported unnarrowed.
            accepts.to_vec()
        };
        CapabilityDescriptor {
            capability,
            semantic_task,
            input_formats,
            output_formats,
            input_modalities,
            output_modalities: produces.to_vec(),
            streaming: capability.text_generation() && adapter && reason.is_none(),
            availability: reason.map_or(Availability::Available, |reason| {
                Availability::Unavailable { reason }
            }),
            option_bounds,
        }
    })
    .collect();
    (observation.declared, capabilities)
}
/// Audio bounds join a descriptor's bounds when the model declares audio.
fn media_bounds(mut bounds: Vec<OptionBound>, audio: bool) -> Vec<OptionBound> {
    if audio {
        bounds.extend(parts::audio_bounds());
    }
    bounds
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

/// Hugging Face and normalized task names for rerankers.
fn text_ranking_task(task: &str) -> bool {
    matches!(task, "text-ranking" | "text_ranking" | "text->ranking")
}
fn text_embedding_task(task: &str) -> bool {
    matches!(
        task,
        "feature-extraction"
            | "feature_extraction"
            | "text-embedding"
            | "text_embedding"
            | "sentence-similarity"
            | "text->embedding"
    )
}
/// Hugging Face and normalized task names for models that take text and
/// images and produce text.
fn vision_language_task(task: &str) -> bool {
    matches!(
        task,
        "image-text-to-text" | "image_text_to_text" | "text+image->text" | "any-to-any"
    )
}
fn semantic_match(
    capability: Capability,
    task: Option<&str>,
    model_type: Option<&str>,
    provider: RuntimeProviderId,
) -> Option<bool> {
    if let Some(task) = task.filter(|task| !matches!(*task, "unknown" | "")) {
        return Some(match capability {
            // Vision-language models (`image-text-to-text`) are chat models
            // that also accept images; text-only requests are valid for them.
            Capability::ChatGeneration | Capability::TextGeneration => {
                matches!(
                    task,
                    "text-generation"
                        | "text_generation"
                        | "text-to-text"
                        | "text2text-generation"
                        | "conversational"
                ) || vision_language_task(task)
                    || (task == "text->text"
                        && matches!(model_type, Some("llm" | "language" | "text")))
            }
            Capability::TextEmbedding => text_embedding_task(task),
            // Multimodal embedders (EmbeddingGemma 2) are published as
            // feature-extraction models; the loaded projector decides images.
            Capability::MultimodalEmbedding => {
                text_embedding_task(task)
                    || matches!(task, "image-feature-extraction" | "image->embedding")
            }
            Capability::ImageGeneration => {
                matches!(task, "text-to-image" | "text_to_image" | "text->image")
            }
            Capability::ImageToText => {
                matches!(task, "image-to-text" | "image_to_text" | "image->text")
                    || vision_language_task(task)
            }
            Capability::Rerank => text_ranking_task(task),
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
        Some("reranker") => Some(capability == Capability::Rerank),
        Some(_) => Some(false),
        None => None,
    }
}
/// The in-process ONNX runtime has fixed batch and tokenizer limits, plus the
/// loaded model's own per-input token limit when its config declares one.
/// Proxied providers enforce their own (context and batch size) and report
/// violations.
fn embedding_bounds(
    provider: RuntimeProviderId,
    max_input_tokens: Option<usize>,
) -> Vec<OptionBound> {
    if provider == RuntimeProviderId::OnnxRuntime {
        let limits = pumas_library::OnnxEmbeddingRequest::LIMITS;
        let mut bounds = vec![
            bound(OptionName::Dimensions, 1., limits.max_dimensions as f64),
            bound(OptionName::InputCount, 1., limits.max_inputs as f64),
            bound(
                OptionName::InputCharacters,
                0.,
                limits.max_input_chars as f64,
            ),
        ];
        if let Some(max_input_tokens) = max_input_tokens {
            bounds.push(bound(OptionName::InputTokens, 1., max_input_tokens as f64));
        }
        bounds
    } else {
        vec![bound(OptionName::Dimensions, 1., u32::MAX as f64)]
    }
}
/// The qualified transcription adapter's fixed native generation bound
/// (`torch-server/owned_model_operations.py`); it accepts no other value.
const AUDIO_MAX_OUTPUT_TOKENS: u32 = 512;
fn bound(option: OptionName, minimum: f64, maximum: f64) -> OptionBound {
    OptionBound {
        option,
        minimum,
        maximum,
    }
}
/// Generation length is bounded by the served context window when the runtime
/// reports one; Pumas adds no fixed cap of its own.
fn text_bounds(context_tokens: Option<u32>) -> Vec<OptionBound> {
    vec![
        bound(
            OptionName::MaxTokens,
            1.,
            f64::from(context_tokens.unwrap_or(u32::MAX)),
        ),
        bound(OptionName::Temperature, 0., 2.),
        bound(OptionName::TopP, 0., 1.),
    ]
}
