//! llama.cpp router serving adapter helpers.

use super::serving::{
    effective_gateway_alias_from_config, non_critical_failure_response, serving_error,
};
use super::serving_llama_cpp_shared::{
    active_llama_cpp_runtime, llama_cpp_runtime_support_error, provider_request_model_id,
};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeError, ModelServeErrorCode, RuntimeDeviceMode, RuntimeManagementMode,
    RuntimeProfileConfig, RuntimeProviderId, RuntimeProviderMode, ServeModelRequest,
    ServeModelResponse, ServedModelLoadState, ServedModelStatus,
};
use pumas_library::runtime_profiles::RuntimeProfileLaunchOverrides;
use serde_json::Value;
use tracing::warn;

pub(super) async fn serve_llama_cpp_router_model(
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
                "selected llama.cpp router profile was not found",
                &request,
            ),
        )
        .await;
    };
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
            warn!(
                "failed to resolve llama.cpp router serving endpoint: {}",
                err
            );
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::EndpointUnavailable,
                    "selected llama.cpp router profile is not available",
                    &request,
                ),
            )
            .await;
        }
    };

    let Some((_tag, version_dir)) = active_llama_cpp_runtime(state, &request).await? else {
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

    let profile_not_running = state
        .api
        .resolve_model_runtime_profile_endpoint_for_operation(
            RuntimeProviderId::LlamaCpp,
            &request.model_id,
            Some(request.config.profile_id.clone()),
        )
        .await
        .is_err();
    let endpoint_unreachable = !state
        .llama_cpp_router_client
        .endpoint_ready(endpoint.as_str())
        .await;
    let should_restart_for_profile_settings = !profile_not_running
        && !endpoint_unreachable
        && llama_cpp_router_should_restart_for_launch_settings(state, &request, &profile).await?;
    if should_restart_for_profile_settings {
        if let Err(err) = state
            .api
            .stop_runtime_profile(request.config.profile_id.clone())
            .await
        {
            warn!(
                "failed to restart llama.cpp router profile before applying device settings: {}",
                err
            );
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "llama.cpp router profile could not be restarted to apply device settings",
                    &request,
                ),
            )
            .await;
        }
    }
    if profile_not_running || endpoint_unreachable || should_restart_for_profile_settings {
        if let Some(error) = launch_llama_cpp_router_profile(state, &request, &profile).await? {
            return non_critical_failure_response(state, error).await;
        }
        if !state
            .llama_cpp_router_client
            .endpoint_ready(endpoint.as_str())
            .await
        {
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "llama.cpp router profile started but its model endpoint is not reachable",
                    &request,
                ),
            )
            .await;
        }
    }

    let gateway_alias = effective_gateway_alias_from_config(&request);
    let router_model_id = provider_request_model_id(&request, &state.provider_registry);
    if let Err(message) = state
        .llama_cpp_router_client
        .load_model(endpoint.as_str(), &router_model_id)
        .await
    {
        warn!("llama.cpp router model load failed: {}", message);
        return non_critical_failure_response(
            state,
            serving_error(ModelServeErrorCode::ProviderLoadFailed, message, &request),
        )
        .await;
    }
    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(gateway_alias.clone()),
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

pub(super) async fn unserve_llama_cpp_router_model(
    state: &AppState,
    request_model_id: &str,
    profile_id: &pumas_library::models::RuntimeProfileId,
    model_alias: &str,
) -> pumas_library::Result<Option<Value>> {
    let endpoint = match state
        .api
        .resolve_model_runtime_profile_endpoint(
            RuntimeProviderId::LlamaCpp,
            request_model_id,
            Some(profile_id.clone()),
        )
        .await
    {
        Ok(endpoint) => endpoint,
        Err(err) => {
            warn!(
                "failed to resolve llama.cpp router unload endpoint: {}",
                err
            );
            return Ok(Some(serde_json::to_value(
                pumas_library::models::UnserveModelResponse {
                    success: true,
                    error: Some("llama.cpp router endpoint is not available".to_string()),
                    unloaded: false,
                    snapshot: Some(state.api.get_serving_status().await?.snapshot),
                },
            )?));
        }
    };
    let router_model_id = request_model_id.trim();
    if let Err(message) = state
        .llama_cpp_router_client
        .unload_model(endpoint.as_str(), router_model_id)
        .await
    {
        warn!("llama.cpp router model unload failed: {}", message);
        return Ok(Some(serde_json::to_value(
            pumas_library::models::UnserveModelResponse {
                success: true,
                error: Some(message),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            },
        )?));
    }
    let snapshot = state
        .api
        .record_unserved_model(
            request_model_id,
            Some(RuntimeProviderId::LlamaCpp),
            Some(profile_id),
            Some(model_alias),
        )
        .await?;
    Ok(Some(serde_json::to_value(
        pumas_library::models::UnserveModelResponse {
            success: true,
            error: None,
            unloaded: true,
            snapshot: Some(snapshot),
        },
    )?))
}

async fn llama_cpp_router_should_restart_for_launch_settings(
    state: &AppState,
    request: &ServeModelRequest,
    profile: &RuntimeProfileConfig,
) -> pumas_library::Result<bool> {
    if profile.management_mode != RuntimeManagementMode::Managed
        || profile.provider_mode != RuntimeProviderMode::LlamaCppRouter
    {
        return Ok(false);
    }
    let has_launch_overrides = llama_cpp_router_profile_has_explicit_device_settings(profile)
        || request.config.context_size.is_some();
    if !has_launch_overrides {
        return Ok(false);
    }
    let snapshot = state.api.get_serving_status().await?.snapshot;
    Ok(!snapshot.served_models.iter().any(|status| {
        status.provider == RuntimeProviderId::LlamaCpp
            && status.profile_id == profile.profile_id
            && status.load_state == ServedModelLoadState::Loaded
    }))
}

fn llama_cpp_router_profile_has_explicit_device_settings(profile: &RuntimeProfileConfig) -> bool {
    profile.device.mode != RuntimeDeviceMode::Auto
        || profile.device.device_id.is_some()
        || profile.device.gpu_layers.is_some()
        || profile
            .device
            .tensor_split
            .as_ref()
            .is_some_and(|tensor_split| !tensor_split.is_empty())
}

async fn launch_llama_cpp_router_profile(
    state: &AppState,
    request: &ServeModelRequest,
    profile: &RuntimeProfileConfig,
) -> pumas_library::Result<Option<ModelServeError>> {
    let Some((tag, version_dir)) = active_llama_cpp_runtime(state, request).await? else {
        return Ok(Some(serving_error(
            ModelServeErrorCode::MissingRuntime,
            "llama.cpp runtime versions are not available",
            request,
        )));
    };
    let launch_response = state
        .api
        .launch_runtime_profile_for_model_with_overrides(
            request.config.profile_id.clone(),
            &tag,
            &version_dir,
            Some(&request.model_id),
            Some(llama_cpp_router_launch_overrides(request, profile)),
        )
        .await;
    match launch_response {
        Ok(response) if response.success => Ok(None),
        Ok(response) => {
            let message = response.error.unwrap_or_else(|| {
                "llama.cpp router profile did not start for the selected model".to_string()
            });
            warn!("llama.cpp router serve launch failed: {}", message);
            Ok(Some(serving_error(
                ModelServeErrorCode::ProviderLoadFailed,
                message,
                request,
            )))
        }
        Err(err) => {
            warn!("llama.cpp router serve launch failed: {}", err);
            Ok(Some(serving_error(
                ModelServeErrorCode::MissingRuntime,
                "llama.cpp router runtime could not be launched",
                request,
            )))
        }
    }
}

fn llama_cpp_router_launch_overrides(
    request: &ServeModelRequest,
    profile: &RuntimeProfileConfig,
) -> RuntimeProfileLaunchOverrides {
    RuntimeProfileLaunchOverrides {
        device: Some(profile.device.clone()),
        context_size: request.config.context_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::models::{ModelServingConfig, RuntimeProfileId};

    #[test]
    fn llama_cpp_router_profile_restart_detects_explicit_device_settings() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider = RuntimeProviderId::LlamaCpp;
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;
        profile.management_mode = RuntimeManagementMode::Managed;
        profile.device.mode = RuntimeDeviceMode::Auto;

        assert!(!llama_cpp_router_profile_has_explicit_device_settings(
            &profile
        ));

        profile.device.mode = RuntimeDeviceMode::Gpu;

        assert!(llama_cpp_router_profile_has_explicit_device_settings(
            &profile
        ));
    }

    #[test]
    fn llama_cpp_router_launch_overrides_use_profile_device_and_request_context() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider = RuntimeProviderId::LlamaCpp;
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;
        profile.device.mode = RuntimeDeviceMode::Gpu;
        profile.device.gpu_layers = Some(20);
        profile.device.tensor_split = Some(vec![1.0, 1.0]);
        let request = ServeModelRequest {
            model_id: "models/example.gguf".to_string(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
                device_mode: RuntimeDeviceMode::Gpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: Some(8192),
                keep_loaded: true,
                model_alias: None,
            },
        };

        let overrides = llama_cpp_router_launch_overrides(&request, &profile);

        let device = overrides.device.expect("router device override");
        assert_eq!(device.mode, RuntimeDeviceMode::Gpu);
        assert_eq!(device.gpu_layers, Some(20));
        assert_eq!(device.tensor_split, Some(vec![1.0, 1.0]));
        assert_eq!(overrides.context_size, Some(8192));
    }
}
