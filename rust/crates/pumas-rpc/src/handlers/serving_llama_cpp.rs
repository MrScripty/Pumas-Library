//! llama.cpp serving adapter used by the serving RPC boundary.

use super::serving::{
    effective_gateway_alias_from_config, non_critical_failure_response, serving_error,
};
use super::serving_llama_cpp_router::{
    serve_llama_cpp_router_model, unserve_llama_cpp_router_model,
};
use super::serving_llama_cpp_shared::{
    active_llama_cpp_runtime, llama_cpp_launch_overrides, llama_cpp_runtime_support_error,
};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeErrorCode, RuntimeProfileId, RuntimeProviderId, RuntimeProviderMode,
    ServeModelRequest, ServeModelResponse, ServedModelLoadState, ServedModelStatus,
    UnserveModelRequest, UnserveModelResponse,
};
use serde_json::Value;
use tracing::warn;

pub(super) async fn serve_llama_cpp_model(
    state: &AppState,
    request: ServeModelRequest,
) -> pumas_library::Result<Value> {
    let profile = state
        .api
        .get_runtime_profiles_snapshot()
        .await?
        .snapshot
        .profiles
        .into_iter()
        .find(|profile| profile.profile_id == request.config.profile_id);

    let Some(profile) = profile else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ProfileNotFound,
                "selected llama.cpp runtime profile was not found",
                &request,
            ),
        )
        .await;
    };

    if profile.provider_mode == RuntimeProviderMode::LlamaCppRouter {
        return serve_llama_cpp_router_model(state, request).await;
    }

    if profile.provider_mode != RuntimeProviderMode::LlamaCppDedicated {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::UnsupportedProvider,
                "selected llama.cpp runtime profile mode is not supported for serving",
                &request,
            ),
        )
        .await;
    }

    let endpoint = match state
        .api
        .resolve_model_runtime_profile_endpoint(
            RuntimeProviderId::LlamaCpp,
            &request.model_id,
            Some(request.config.profile_id.clone()),
        )
        .await
    {
        Ok(endpoint) => endpoint,
        Err(err) => {
            warn!("failed to resolve llama.cpp serving endpoint: {}", err);
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::EndpointUnavailable,
                    "selected llama.cpp runtime profile has no serving endpoint",
                    &request,
                ),
            )
            .await;
        }
    };

    let Some((tag, version_dir)) = active_llama_cpp_runtime(state, &request).await? else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::MissingRuntime,
                "llama.cpp runtime versions are not available",
                &request,
            ),
        )
        .await;
    };
    if let Some(error) = llama_cpp_runtime_support_error(&version_dir, &request) {
        return non_critical_failure_response(state, error).await;
    }
    let launch_response = state
        .api
        .launch_runtime_profile_for_model_with_overrides(
            request.config.profile_id.clone(),
            &tag,
            &version_dir,
            Some(&request.model_id),
            Some(llama_cpp_launch_overrides(&request)),
        )
        .await;
    match launch_response {
        Ok(response) if response.success => {}
        Ok(response) => {
            let message = response.error.unwrap_or_else(|| {
                "llama.cpp runtime profile did not start for the selected model".to_string()
            });
            warn!("llama.cpp model serve launch failed: {}", message);
            return non_critical_failure_response(
                state,
                serving_error(ModelServeErrorCode::ProviderLoadFailed, message, &request),
            )
            .await;
        }
        Err(err) => {
            warn!("llama.cpp model serve launch failed: {}", err);
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::MissingRuntime,
                    "llama.cpp runtime could not be launched for the selected model",
                    &request,
                ),
            )
            .await;
        }
    }

    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(effective_gateway_alias_from_config(&request)),
        provider: RuntimeProviderId::LlamaCpp,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: request.config.gpu_layers,
        tensor_split: request.config.tensor_split.clone(),
        context_size: request.config.context_size,
        keep_loaded: request.config.keep_loaded,
        endpoint_url: Some(endpoint),
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

pub(super) async fn unserve_llama_cpp_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let provider_mode = state
        .api
        .get_runtime_profiles_snapshot()
        .await?
        .snapshot
        .profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
        .map(|profile| profile.provider_mode);

    if provider_mode == Some(RuntimeProviderMode::LlamaCppRouter) {
        if let Some(response) =
            unserve_llama_cpp_router_model(state, &request.model_id, &profile_id, &model_alias)
                .await?
        {
            return Ok(response);
        }
    }

    if provider_mode != Some(RuntimeProviderMode::LlamaCppDedicated) {
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: Some("selected llama.cpp runtime profile mode is not supported".to_string()),
            unloaded: false,
            snapshot: Some(state.api.get_serving_status().await?.snapshot),
        })?);
    }

    if let Err(err) = state.api.stop_runtime_profile(profile_id.clone()).await {
        warn!("llama.cpp serving unload failed: {}", err);
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: Some("llama.cpp runtime profile could not be stopped".to_string()),
            unloaded: false,
            snapshot: Some(state.api.get_serving_status().await?.snapshot),
        })?);
    }

    let snapshot = state
        .api
        .record_unserved_model(
            &request.model_id,
            Some(RuntimeProviderId::LlamaCpp),
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
