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
    match state
        .api
        .serve_installed_audio_for_operation(request.clone(), operation, installed)
        .await
    {
        Ok(response) => Ok(serde_json::to_value(response)?),
        Err(error) => {
            tracing::warn!(%error, "Installed audio qualification or owned load refused");
            non_critical_failure_response(state, serving_error(ModelServeErrorCode::ProviderLoadFailed, "Installed audio is unavailable until its source-owned runtime and model qualification succeeds", &request)).await
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
