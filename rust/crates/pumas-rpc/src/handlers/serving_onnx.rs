//! ONNX Runtime serving adapter used by the serving RPC boundary.

use super::serving::{
    effective_gateway_alias_from_config, non_critical_failure_response, serving_error,
};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeErrorCode, RuntimeProfileId, RuntimeProviderId, ServeModelRequest,
    ServeModelResponse, ServedModelLoadState, ServedModelStatus, UnserveModelRequest,
    UnserveModelResponse,
};
use pumas_library::{
    ExecutableArtifactFormat, OnnxLoadOptions, OnnxLoadRequest, OnnxModelId, OnnxRuntimeError,
    ProviderRegistry,
};
use serde_json::Value;
use tracing::{debug, info, warn};

struct ValidatedOnnxServeRequest<'a> {
    request: &'a ServeModelRequest,
    load_request: OnnxLoadRequest,
    gateway_alias: String,
}

enum OnnxServeBoundaryError {
    NoExecutableArtifact,
    InvalidLoadRequest(OnnxRuntimeError),
}

struct ValidatedOnnxUnserveRequest {
    model_id: OnnxModelId,
    profile_id: RuntimeProfileId,
    model_alias: String,
}

pub(super) async fn serve_onnx_model(
    state: &AppState,
    request: ServeModelRequest,
) -> pumas_library::Result<Value> {
    let validated = match validate_onnx_serve_request(state, &request).await? {
        Ok(validated) => validated,
        Err(OnnxServeBoundaryError::NoExecutableArtifact) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                profile_id = %request.config.profile_id.as_str(),
                "ONNX serving request has no executable ONNX artifact"
            );
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ModelNotExecutable,
                    "model has no executable ONNX artifact",
                    &request,
                ),
            )
            .await;
        }
        Err(OnnxServeBoundaryError::InvalidLoadRequest(error)) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                provider_model_id = %onnx_provider_request_model_id(&request, &state.provider_registry),
                profile_id = %request.config.profile_id.as_str(),
                error_code = ?error.code,
                error_field = ?error.field,
                "ONNX serving load request validation failed"
            );
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::InvalidRequest,
                    "ONNX Runtime rejected the selected model load request",
                    &request,
                ),
            )
            .await;
        }
    };
    if let Some(response) = existing_loaded_onnx_response(state, &validated).await? {
        debug!(
            provider = "onnx_runtime",
            model_id = %request.model_id,
            provider_model_id = %validated.load_request.model_id.as_str(),
            profile_id = %request.config.profile_id.as_str(),
            "ONNX serving request reused existing loaded session"
        );
        return Ok(serde_json::to_value(response)?);
    }

    let onnx_model_id = validated.load_request.model_id.clone();
    let provider_model_id = onnx_model_id.as_str().to_string();
    let gateway_alias = validated.gateway_alias.clone();
    let session = match state
        .onnx_session_manager
        .load(validated.load_request)
        .await
    {
        Ok(session) => session,
        Err(error) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                provider_model_id = %provider_model_id,
                profile_id = %request.config.profile_id.as_str(),
                error_code = ?error.code,
                "ONNX session load failed"
            );
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "ONNX Runtime could not load the selected model",
                    &request,
                ),
            )
            .await;
        }
    };
    if let Err(error) = confirm_onnx_session_loaded(state, &onnx_model_id).await {
        warn!(
            provider = "onnx_runtime",
            model_id = %request.model_id,
            provider_model_id = %provider_model_id,
            profile_id = %request.config.profile_id.as_str(),
            error = %error,
            "ONNX session status confirmation failed"
        );
        compensate_onnx_load_failure(
            state,
            &request,
            &onnx_model_id,
            provider_model_id.as_str(),
            "status_confirmation_failed",
        )
        .await;
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ProviderLoadFailed,
                "ONNX Runtime loaded the selected model but did not report it as available",
                &request,
            ),
        )
        .await;
    }

    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(gateway_alias),
        provider: RuntimeProviderId::OnnxRuntime,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: request.config.gpu_layers,
        tensor_split: request.config.tensor_split.clone(),
        context_size: Some(session.embedding_dimensions as u32),
        keep_loaded: request.config.keep_loaded,
        endpoint_url: None,
        memory_bytes: None,
        loaded_at: None,
        last_error: None,
    };
    let snapshot = match state.api.record_served_model(status.clone()).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                provider_model_id = %provider_model_id,
                profile_id = %request.config.profile_id.as_str(),
                error = %error,
                "ONNX served-state record failed after session load"
            );
            compensate_onnx_load_failure(
                state,
                &request,
                &onnx_model_id,
                provider_model_id.as_str(),
                "served_state_record_failed",
            )
            .await;
            return Err(error);
        }
    };
    info!(
        provider = "onnx_runtime",
        model_id = %request.model_id,
        provider_model_id = %provider_model_id,
        profile_id = %request.config.profile_id.as_str(),
        gateway_alias = ?status.model_alias.as_deref(),
        embedding_dimensions = session.embedding_dimensions,
        "ONNX model loaded and recorded as served"
    );

    Ok(serde_json::to_value(ServeModelResponse {
        success: true,
        error: None,
        loaded: true,
        loaded_models_unchanged: false,
        status: Some(status),
        load_error: None,
        snapshot: Some(snapshot),
    })?)
}

async fn compensate_onnx_load_failure(
    state: &AppState,
    request: &ServeModelRequest,
    model_id: &OnnxModelId,
    provider_model_id: &str,
    reason: &'static str,
) {
    match state.onnx_session_manager.unload(model_id).await {
        Ok(Some(_)) => {
            info!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                provider_model_id,
                profile_id = %request.config.profile_id.as_str(),
                reason,
                "ONNX session unloaded after load workflow failure"
            );
        }
        Ok(None) => {
            debug!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                provider_model_id,
                profile_id = %request.config.profile_id.as_str(),
                reason,
                "ONNX load workflow compensation found no loaded session"
            );
        }
        Err(error) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                provider_model_id,
                profile_id = %request.config.profile_id.as_str(),
                reason,
                error_code = ?error.code,
                "ONNX load workflow compensation failed"
            );
        }
    }
}

pub(super) async fn unserve_onnx_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let validated = match validate_onnx_unserve_request(&request, &profile_id, model_alias) {
        Ok(validated) => validated,
        Err(error) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                profile_id = %profile_id.as_str(),
                error_code = ?error.code,
                error_field = ?error.field,
                "ONNX serving unload request validation failed"
            );
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("ONNX Runtime rejected the selected model unload request".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?);
        }
    };

    match state.onnx_session_manager.unload(&validated.model_id).await {
        Ok(Some(_)) => {
            info!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                profile_id = %validated.profile_id.as_str(),
                gateway_alias = %validated.model_alias,
                "ONNX session unloaded"
            );
        }
        Ok(None) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                profile_id = %validated.profile_id.as_str(),
                gateway_alias = %validated.model_alias,
                "ONNX session was already absent; removing stale served status"
            );
            let snapshot = state
                .api
                .record_unserved_model(
                    &request.model_id,
                    Some(RuntimeProviderId::OnnxRuntime),
                    Some(&validated.profile_id),
                    Some(validated.model_alias.as_str()),
                )
                .await?;
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: None,
                unloaded: true,
                snapshot: Some(snapshot),
            })?);
        }
        Err(error) => {
            warn!(
                provider = "onnx_runtime",
                model_id = %request.model_id,
                profile_id = %validated.profile_id.as_str(),
                gateway_alias = %validated.model_alias,
                error_code = ?error.code,
                "ONNX session unload failed"
            );
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("ONNX Runtime could not unload the selected model".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?);
        }
    }

    let snapshot = state
        .api
        .record_unserved_model(
            &request.model_id,
            Some(RuntimeProviderId::OnnxRuntime),
            Some(&validated.profile_id),
            Some(validated.model_alias.as_str()),
        )
        .await?;
    Ok(serde_json::to_value(UnserveModelResponse {
        success: true,
        error: None,
        unloaded: true,
        snapshot: Some(snapshot),
    })?)
}

async fn validate_onnx_serve_request<'a>(
    state: &AppState,
    request: &'a ServeModelRequest,
) -> pumas_library::Result<Result<ValidatedOnnxServeRequest<'a>, OnnxServeBoundaryError>> {
    let Some(onnx_path) = resolve_onnx_model_path(state, request).await? else {
        return Ok(Err(OnnxServeBoundaryError::NoExecutableArtifact));
    };
    let library_root = state.api.model_library().library_root().to_path_buf();
    let provider_model_id = onnx_provider_request_model_id(request, &state.provider_registry);
    let load_request = match OnnxLoadRequest::parse(
        library_root,
        &onnx_path,
        provider_model_id.as_str(),
        OnnxLoadOptions::default(),
    ) {
        Ok(load_request) => load_request,
        Err(error) => return Ok(Err(OnnxServeBoundaryError::InvalidLoadRequest(error))),
    };
    Ok(Ok(ValidatedOnnxServeRequest {
        request,
        load_request,
        gateway_alias: effective_gateway_alias_from_config(request),
    }))
}

fn validate_onnx_unserve_request(
    request: &UnserveModelRequest,
    profile_id: &RuntimeProfileId,
    model_alias: String,
) -> Result<ValidatedOnnxUnserveRequest, OnnxRuntimeError> {
    Ok(ValidatedOnnxUnserveRequest {
        model_id: OnnxModelId::parse(&request.model_id)?,
        profile_id: profile_id.clone(),
        model_alias,
    })
}

async fn resolve_onnx_model_path(
    state: &AppState,
    request: &ServeModelRequest,
) -> pumas_library::Result<Option<std::path::PathBuf>> {
    let library = state.api.model_library().clone();
    let model_id = request.model_id.clone();
    let primary_file =
        tokio::task::spawn_blocking(move || library.get_primary_model_file(&model_id))
            .await
            .map_err(|err| {
                pumas_library::PumasError::Other(format!(
                    "Failed to join primary ONNX model lookup task: {}",
                    err
                ))
            })?;
    let Some(onnx_path) = primary_file else {
        return Ok(None);
    };
    if ExecutableArtifactFormat::from_path(&onnx_path) != Some(ExecutableArtifactFormat::Onnx) {
        return Ok(None);
    }
    Ok(Some(onnx_path))
}

async fn confirm_onnx_session_loaded(
    state: &AppState,
    model_id: &OnnxModelId,
) -> Result<(), String> {
    let sessions = state
        .onnx_session_manager
        .list()
        .await
        .map_err(|error| error.to_string())?;
    if sessions
        .iter()
        .any(|session| session.model_id.as_str() == model_id.as_str())
    {
        return Ok(());
    }
    Err(format!(
        "ONNX model '{}' was absent from session list after load",
        model_id.as_str()
    ))
}

async fn existing_loaded_onnx_response(
    state: &AppState,
    validated: &ValidatedOnnxServeRequest<'_>,
) -> pumas_library::Result<Option<ServeModelResponse>> {
    let Some(status) = state
        .api
        .find_served_model(
            &validated.request.model_id,
            Some(RuntimeProviderId::OnnxRuntime),
            Some(&validated.request.config.profile_id),
        )
        .await?
    else {
        return Ok(None);
    };
    if status.load_state != ServedModelLoadState::Loaded
        || status.model_alias.as_deref() != Some(validated.gateway_alias.as_str())
        || confirm_onnx_session_loaded(state, &validated.load_request.model_id)
            .await
            .is_err()
    {
        return Ok(None);
    }
    Ok(Some(ServeModelResponse {
        success: true,
        error: None,
        loaded: true,
        loaded_models_unchanged: true,
        status: Some(status),
        load_error: None,
        snapshot: Some(state.api.get_serving_status().await?.snapshot),
    }))
}

fn onnx_provider_request_model_id(
    request: &ServeModelRequest,
    registry: &ProviderRegistry,
) -> String {
    let library_model_id = request.model_id.trim();
    registry
        .get(RuntimeProviderId::OnnxRuntime)
        .map(|behavior| {
            behavior
                .provider_request_model_id(library_model_id, request.config.model_alias.as_deref())
        })
        .unwrap_or_else(|| library_model_id.to_string())
}

#[cfg(test)]
#[path = "serving_onnx_tests.rs"]
mod tests;
