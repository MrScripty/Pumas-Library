//! Native audio serving consumes the existing installed-runtime custody producer.
use super::serving::{current_serving_snapshot, non_critical_failure_response, serving_error};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeErrorCode, RuntimeProfileId, ServeModelRequest, UnserveModelResponse,
};
use pumas_library::serving::ServingLoadOperation;
use serde_json::Value;

pub(super) async fn serve_audio_model(
    state: &AppState,
    request: ServeModelRequest,
    operation: &ServingLoadOperation,
) -> pumas_library::Result<Value> {
    serve_audio(state, request, operation, false).await
}

pub(super) async fn serve_experimental_local_cohere(
    state: &AppState,
    request: ServeModelRequest,
    operation: &ServingLoadOperation,
) -> pumas_library::Result<Value> {
    serve_audio(state, request, operation, true).await
}

async fn serve_audio(
    state: &AppState,
    request: ServeModelRequest,
    operation: &ServingLoadOperation,
    experimental: bool,
) -> pumas_library::Result<Value> {
    let Some(manager) = super::get_version_manager(state, "torch").await else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::MissingRuntime,
                "Torch runtime manager is unavailable",
                &request,
            ),
        )
        .await;
    };
    let Some(tag) = manager.get_active_version().await? else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::MissingRuntime,
                "Install and explicitly select a Torch runtime first",
                &request,
            ),
        )
        .await;
    };
    // Polled only inside core's already-reserved profile startup. The manager
    // independently owns and joins all nested byte-preparation effects.
    let installed = async move { manager.prepare_active_torch_audio_runtime_bytes(&tag).await };
    let result = if experimental {
        state
            .api
            .serve_experimental_local_cohere_for_operation(request.clone(), operation, installed)
            .await
            .and_then(|response| Ok(serde_json::to_value(response)?))
    } else {
        state
            .api
            .serve_installed_audio_for_operation(request.clone(), operation, installed)
            .await
            .and_then(|response| Ok(serde_json::to_value(response)?))
    };
    match result {
        Ok(response) => Ok(response),
        Err(error) => {
            tracing::warn!(%error, experimental, "Installed audio owned load refused");
            // Only fixed, source-owned diagnostics are exposed, never arbitrary
            // native/path error text. The original error stays in local logs.
            let diagnostic = match &error {
                pumas_library::PumasError::InvalidParams { message } if message.starts_with("Installed Cohere audio requires") => message.as_str(),
                pumas_library::PumasError::Other(message) if message.starts_with("Installed Cohere") => message.as_str(),
                pumas_library::PumasError::Other(message) if message == "audio installed child refused" => "Installed Cohere child startup refused; verify host confinement and the pinned runtime/native closure.",
                pumas_library::PumasError::Other(message) if message == "audio private binding refused" => "Installed Cohere private hello/binding refused; no native load was admitted.",
                pumas_library::PumasError::Other(message) if message == "audio selected load refused" => "Installed Cohere native model load refused; selected bytes remain owned until exact child drainage. Detailed native stderr is intentionally not returned.",
                _ => "Installed Cohere load refused. Check the source-pinned CPU Torch runtime, valid indexed LibraryOwned Cohere artifact, and CPU-compatible managed profile. Local logs identify the failing phase, not full native stderr.",
            };
            non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    diagnostic,
                    &request,
                ),
            )
            .await
        }
    }
}

pub(super) async fn unserve_audio_model(
    state: &AppState,
    profile: &RuntimeProfileId,
    model: &str,
) -> pumas_library::Result<Value> {
    let unloaded = state.api.stop_installed_audio_model(profile, model).await?;
    Ok(serde_json::to_value(UnserveModelResponse {
        success: true,
        error: None,
        unloaded,
        snapshot: Some(current_serving_snapshot(state).await?),
    })?)
}
