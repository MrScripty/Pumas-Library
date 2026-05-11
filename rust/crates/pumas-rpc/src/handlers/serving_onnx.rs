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
use pumas_library::{ExecutableArtifactFormat, OnnxLoadOptions, OnnxLoadRequest, OnnxModelId};
use serde_json::Value;
use tracing::warn;

pub(super) async fn serve_onnx_model(
    state: &AppState,
    request: ServeModelRequest,
) -> pumas_library::Result<Value> {
    let Some(onnx_path) = resolve_onnx_model_path(state, &request).await? else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ModelNotExecutable,
                "model has no executable ONNX artifact",
                &request,
            ),
        )
        .await;
    };
    let library_root = state.api.model_library().library_root().to_path_buf();
    let load_request = match OnnxLoadRequest::parse(
        library_root,
        &onnx_path,
        &request.model_id,
        OnnxLoadOptions::default(),
    ) {
        Ok(load_request) => load_request,
        Err(error) => {
            warn!("ONNX serving load request validation failed: {}", error);
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

    let session = match state.onnx_session_manager.load(load_request).await {
        Ok(session) => session,
        Err(error) => {
            warn!("ONNX fake session load failed: {}", error);
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

    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(effective_gateway_alias_from_config(&request)),
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
    let snapshot = state.api.record_served_model(status.clone()).await?;

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

pub(super) async fn unserve_onnx_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let model_id = match OnnxModelId::parse(&request.model_id) {
        Ok(model_id) => model_id,
        Err(error) => {
            warn!("ONNX serving unload request validation failed: {}", error);
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("ONNX Runtime rejected the selected model unload request".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?);
        }
    };

    match state.onnx_session_manager.unload(&model_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("ONNX Runtime model was not loaded".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?);
        }
        Err(error) => {
            warn!("ONNX fake session unload failed: {}", error);
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
            Some(&profile_id),
            Some(model_alias.as_str()),
        )
        .await?;
    Ok(serde_json::to_value(UnserveModelResponse {
        success: true,
        error: None,
        unloaded: true,
        snapshot: Some(snapshot),
    })?)
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
