//! User-directed model serving RPC handlers.

use super::parse_params;
use crate::server::AppState;
use pumas_library::models::{
    ModelServeError, ModelServeErrorCode, RuntimeDeviceMode, RuntimeDeviceSettings,
    RuntimeManagementMode, RuntimeProfileConfig, RuntimeProfileId, RuntimeProviderId,
    RuntimeProviderMode, ServeModelRequest, ServeModelResponse, ServedModelLoadState,
    ServedModelStatus, UnserveModelRequest, UnserveModelResponse,
};
use pumas_library::runtime_profiles::RuntimeProfileLaunchOverrides;
use pumas_library::{
    ProviderGatewayAliasPolicy, ProviderRegistry, ProviderServingAdapterKind,
    ProviderUnloadBehavior,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::warn;

#[derive(Debug, Deserialize)]
struct ValidateModelServingConfigParams {
    request: ServeModelRequest,
}

#[derive(Debug, Deserialize)]
struct ServeModelParams {
    request: ServeModelRequest,
}

#[derive(Debug, Deserialize)]
struct UnserveModelParams {
    request: UnserveModelRequest,
}

#[derive(Debug, Deserialize)]
struct ListServingStatusUpdatesSinceParams {
    cursor: Option<String>,
}

pub async fn get_serving_status(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    Ok(serde_json::to_value(state.api.get_serving_status().await?)?)
}

pub async fn list_serving_status_updates_since(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: ListServingStatusUpdatesSinceParams =
        parse_params("list_serving_status_updates_since", params)?;
    Ok(serde_json::to_value(
        state
            .api
            .list_serving_status_updates_since(command.cursor.as_deref())
            .await?,
    )?)
}

pub async fn validate_model_serving_config(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: ValidateModelServingConfigParams =
        parse_params("validate_model_serving_config", params)?;
    let request = request_with_effective_gateway_alias(state, command.request).await?;
    Ok(serde_json::to_value(
        state.api.validate_model_serving_config(request).await?,
    )?)
}

pub async fn serve_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let command: ServeModelParams = parse_params("serve_model", params)?;
    let request = request_with_effective_gateway_alias(state, command.request).await?;
    let validation = state
        .api
        .validate_model_serving_config(request.clone())
        .await?;
    if !validation.valid {
        let error = validation.errors.into_iter().next().unwrap_or_else(|| {
            serving_error(
                ModelServeErrorCode::InvalidRequest,
                "serving request is invalid",
                &request,
            )
        });
        return non_critical_failure_response(state, error).await;
    }

    let registry = ProviderRegistry::builtin();
    match registry
        .get(request.config.provider)
        .map(|behavior| behavior.serving_adapter_kind)
    {
        Some(ProviderServingAdapterKind::OllamaProviderApi) => {
            serve_ollama_model(state, request).await
        }
        Some(ProviderServingAdapterKind::LlamaCppRuntime) => {
            serve_llama_cpp_model(state, request).await
        }
        None => {
            let error = serving_error(
                ModelServeErrorCode::UnsupportedProvider,
                "selected serving provider is not registered",
                &request,
            );
            non_critical_failure_response(state, error).await
        }
    }
}

pub async fn unserve_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let command: UnserveModelParams = parse_params("unserve_model", params)?;
    let served = state
        .api
        .find_served_model(
            &command.request.model_id,
            command.request.provider,
            command.request.profile_id.as_ref(),
        )
        .await?;
    let Some(served_status) = served else {
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: None,
            unloaded: false,
            snapshot: Some(state.api.get_serving_status().await?.snapshot),
        })?);
    };
    let profile_id = command
        .request
        .profile_id
        .clone()
        .unwrap_or_else(|| served_status.profile_id.clone());
    let model_alias = command
        .request
        .model_alias
        .clone()
        .or_else(|| served_status.model_alias.clone())
        .unwrap_or_else(|| derive_fallback_model_alias(&command.request.model_id));

    let registry = ProviderRegistry::builtin();
    match registry
        .get(served_status.provider)
        .map(|behavior| behavior.unload_behavior)
    {
        Some(ProviderUnloadBehavior::ProviderApi) => {
            unserve_ollama_model(state, command.request, profile_id, model_alias).await
        }
        Some(ProviderUnloadBehavior::RouterPreset) => {
            unserve_llama_cpp_model(state, command.request, profile_id, model_alias).await
        }
        None => Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: Some("served model provider is not registered".to_string()),
            unloaded: false,
            snapshot: Some(state.api.get_serving_status().await?.snapshot),
        })?),
    }
}

async fn unserve_ollama_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let endpoint = match state
        .api
        .resolve_model_runtime_profile_endpoint_for_operation(
            RuntimeProviderId::Ollama,
            &request.model_id,
            Some(profile_id.clone()),
        )
        .await
    {
        Ok(endpoint) => endpoint,
        Err(err) => {
            warn!("failed to resolve serving unload endpoint: {}", err);
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("selected runtime profile is not available".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?);
        }
    };

    let client = pumas_app_manager::OllamaClient::new(Some(endpoint.as_str()));
    if let Err(err) = client.unload_model(&model_alias).await {
        warn!("Ollama serving unload failed: {}", err);
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: Some("Ollama could not unload the selected model".to_string()),
            unloaded: false,
            snapshot: Some(state.api.get_serving_status().await?.snapshot),
        })?);
    }

    let snapshot = state
        .api
        .record_unserved_model(
            &request.model_id,
            Some(RuntimeProviderId::Ollama),
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

async fn serve_llama_cpp_model(
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

async fn serve_llama_cpp_router_model(
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
    let endpoint_unreachable = !llama_cpp_router_endpoint_ready(endpoint.as_str()).await;
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
        if !llama_cpp_router_endpoint_ready(endpoint.as_str()).await {
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
    let router_model_id = provider_request_model_id(&request);
    if let Err(message) = llama_cpp_router_load_model(endpoint.as_str(), &router_model_id).await {
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

async fn llama_cpp_router_endpoint_ready(endpoint: &str) -> bool {
    let url = llama_cpp_router_models_url(endpoint);
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    client
        .get(url)
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

fn llama_cpp_router_models_url(endpoint: &str) -> String {
    format!("{}/v1/models", endpoint.trim_end_matches('/'))
}

async fn llama_cpp_router_load_model(endpoint: &str, model_alias: &str) -> Result<(), String> {
    let url = llama_cpp_router_model_load_url(endpoint);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|err| format!("failed to build llama.cpp router client: {err}"))?;
    let response = client
        .post(url)
        .json(&serde_json::json!({ "model": model_alias }))
        .send()
        .await
        .map_err(|err| format!("failed to load model through llama.cpp router: {err}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status.is_success() || body.contains("model is already running") {
        return Ok(());
    }
    Err(if body.trim().is_empty() {
        format!("llama.cpp router failed to load model with HTTP status {status}")
    } else {
        format!("llama.cpp router failed to load model with HTTP status {status}: {body}")
    })
}

fn llama_cpp_router_model_load_url(endpoint: &str) -> String {
    format!("{}/models/load", endpoint.trim_end_matches('/'))
}

async fn llama_cpp_router_unload_model(endpoint: &str, model_alias: &str) -> Result<(), String> {
    let url = llama_cpp_router_model_unload_url(endpoint);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| format!("failed to build llama.cpp router client: {err}"))?;
    let response = client
        .post(url)
        .json(&serde_json::json!({ "model": model_alias }))
        .send()
        .await
        .map_err(|err| format!("failed to unload model through llama.cpp router: {err}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status.is_success() || body.contains("model is not running") {
        return Ok(());
    }
    Err(if body.trim().is_empty() {
        format!("llama.cpp router failed to unload model with HTTP status {status}")
    } else {
        format!("llama.cpp router failed to unload model with HTTP status {status}: {body}")
    })
}

fn llama_cpp_router_model_unload_url(endpoint: &str) -> String {
    format!("{}/models/unload", endpoint.trim_end_matches('/'))
}

fn provider_request_model_id(request: &ServeModelRequest) -> String {
    let library_model_id = request.model_id.trim();
    ProviderRegistry::builtin()
        .get(request.config.provider)
        .map(|behavior| {
            behavior
                .provider_request_model_id(library_model_id, request.config.model_alias.as_deref())
        })
        .unwrap_or_else(|| library_model_id.to_string())
}

async fn active_llama_cpp_runtime(
    state: &AppState,
    request: &ServeModelRequest,
) -> pumas_library::Result<Option<(String, PathBuf)>> {
    let Some(version_manager) = super::get_version_manager(state, "llama-cpp").await else {
        return Ok(None);
    };
    let Some(tag) = version_manager.get_active_version().await? else {
        warn!(
            model_id = %request.model_id,
            profile_id = %request.config.profile_id.as_str(),
            "No active llama.cpp runtime version is set"
        );
        return Ok(None);
    };
    let version_dir = version_manager.version_path(&tag);
    Ok(Some((tag, version_dir)))
}

fn llama_cpp_launch_overrides(request: &ServeModelRequest) -> RuntimeProfileLaunchOverrides {
    RuntimeProfileLaunchOverrides {
        device: Some(RuntimeDeviceSettings {
            mode: request.config.device_mode,
            device_id: request.config.device_id.clone(),
            gpu_layers: request.config.gpu_layers,
            tensor_split: request.config.tensor_split.clone(),
        }),
        context_size: request.config.context_size,
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

fn llama_cpp_runtime_support_error(
    version_dir: &Path,
    request: &ServeModelRequest,
) -> Option<ModelServeError> {
    if !llama_cpp_request_needs_gpu_runtime(request)
        || llama_cpp_runtime_has_gpu_backend(version_dir)
    {
        return None;
    }
    Some(serving_error(
        ModelServeErrorCode::DeviceUnavailable,
        "selected llama.cpp profile requires GPU offload, but the active llama.cpp runtime is CPU-only; install a GPU-capable llama.cpp runtime build such as the Vulkan or ROCm archive, or choose a CPU profile",
        request,
    ))
}

fn llama_cpp_request_needs_gpu_runtime(request: &ServeModelRequest) -> bool {
    matches!(
        request.config.device_mode,
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::Hybrid | RuntimeDeviceMode::SpecificDevice
    ) || request.config.gpu_layers.is_some_and(|layers| layers != 0)
        || request
            .config
            .tensor_split
            .as_ref()
            .is_some_and(|split| !split.is_empty())
}

fn llama_cpp_runtime_has_gpu_backend(version_dir: &Path) -> bool {
    let mut stack = vec![version_dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.filter_map(std::result::Result::ok) {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if llama_cpp_backend_filename_is_gpu(entry_path.file_name()) {
                return true;
            }
        }
    }
    false
}

fn llama_cpp_backend_filename_is_gpu(file_name: Option<&OsStr>) -> bool {
    let Some(file_name) = file_name else {
        return false;
    };
    let file_name = file_name.to_string_lossy().to_ascii_lowercase();
    [
        "ggml-vulkan",
        "ggml-cuda",
        "ggml-hip",
        "ggml-rocm",
        "ggml-sycl",
        "ggml-metal",
        "ggml-kompute",
        "ggml-opencl",
    ]
    .iter()
    .any(|backend| file_name.contains(backend))
}

async fn unserve_llama_cpp_model(
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

    if provider_mode == Some(pumas_library::models::RuntimeProviderMode::LlamaCppRouter) {
        let endpoint = match state
            .api
            .resolve_model_runtime_profile_endpoint(
                RuntimeProviderId::LlamaCpp,
                &request.model_id,
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
                return Ok(serde_json::to_value(UnserveModelResponse {
                    success: true,
                    error: Some("llama.cpp router endpoint is not available".to_string()),
                    unloaded: false,
                    snapshot: Some(state.api.get_serving_status().await?.snapshot),
                })?);
            }
        };
        let router_model_id = request.model_id.trim();
        if let Err(message) =
            llama_cpp_router_unload_model(endpoint.as_str(), router_model_id).await
        {
            warn!("llama.cpp router model unload failed: {}", message);
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some(message),
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
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: None,
            unloaded: true,
            snapshot: Some(snapshot),
        })?);
    }

    if provider_mode != Some(pumas_library::models::RuntimeProviderMode::LlamaCppDedicated) {
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

async fn serve_ollama_model(
    state: &AppState,
    request: ServeModelRequest,
) -> pumas_library::Result<Value> {
    let endpoint = match state
        .api
        .resolve_model_runtime_profile_endpoint_for_operation(
            RuntimeProviderId::Ollama,
            &request.model_id,
            Some(request.config.profile_id.clone()),
        )
        .await
    {
        Ok(endpoint) => endpoint,
        Err(err) => {
            warn!("failed to resolve Ollama serving endpoint: {}", err);
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::EndpointUnavailable,
                    "selected Ollama runtime profile is not available",
                    &request,
                ),
            )
            .await;
        }
    };

    let Some((gguf_path, model_alias, known_sha256)) =
        resolve_ollama_model_inputs(state, &request).await?
    else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ModelNotExecutable,
                "model has no executable GGUF artifact",
                &request,
            ),
        )
        .await;
    };

    let client = pumas_app_manager::OllamaClient::new(Some(endpoint.as_str()));
    let registered = match client.list_models().await {
        Ok(models) => models.iter().any(|model| model.name == model_alias),
        Err(err) => {
            warn!("Ollama model inventory failed before serving: {}", err);
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::EndpointUnavailable,
                    "Ollama endpoint is not available",
                    &request,
                ),
            )
            .await;
        }
    };

    if !registered {
        if let Err(err) = client
            .create_model(&model_alias, &gguf_path, known_sha256.as_deref())
            .await
        {
            warn!("Ollama model registration failed before serving: {}", err);
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "Ollama could not register the selected model",
                    &request,
                ),
            )
            .await;
        }
    }

    if let Err(err) = client
        .load_model_with_keep_alive(&model_alias, request.config.keep_loaded)
        .await
    {
        warn!("Ollama model load failed: {}", err);
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ProviderLoadFailed,
                "Ollama could not load the selected model with the requested configuration",
                &request,
            ),
        )
        .await;
    }

    let memory_bytes = match client.list_running_models().await {
        Ok(running) => running
            .iter()
            .find(|model| model.name == model_alias)
            .map(|model| model.size),
        Err(err) => {
            warn!(
                "Ollama running inventory failed after serving load: {}",
                err
            );
            None
        }
    };

    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(model_alias),
        provider: RuntimeProviderId::Ollama,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: request.config.gpu_layers,
        tensor_split: request.config.tensor_split.clone(),
        context_size: request.config.context_size,
        keep_loaded: request.config.keep_loaded,
        endpoint_url: Some(endpoint),
        memory_bytes,
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

async fn resolve_ollama_model_inputs(
    state: &AppState,
    request: &ServeModelRequest,
) -> pumas_library::Result<Option<(std::path::PathBuf, String, Option<String>)>> {
    let library = state.api.model_library().clone();
    let model_id = request.model_id.clone();
    let primary_file = tokio::task::spawn_blocking({
        let library = library.clone();
        let model_id = model_id.clone();
        move || library.get_primary_model_file(&model_id)
    })
    .await
    .map_err(|err| {
        pumas_library::PumasError::Other(format!(
            "Failed to join primary model file lookup task: {}",
            err
        ))
    })?;
    let Some(gguf_path) = primary_file else {
        return Ok(None);
    };
    let is_gguf = gguf_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"));
    if !is_gguf {
        return Ok(None);
    }

    let model_record = library.get_model(&model_id).await?;
    let model_alias = request.config.model_alias.clone().unwrap_or_else(|| {
        let display = model_record
            .as_ref()
            .map(|record| record.cleaned_name.clone())
            .unwrap_or_else(|| derive_fallback_model_alias(&model_id));
        pumas_app_manager::derive_ollama_name(&display)
    });
    let known_sha256 = model_record
        .as_ref()
        .and_then(|record| record.hashes.get("sha256"))
        .cloned();

    Ok(Some((gguf_path, model_alias, known_sha256)))
}

async fn request_with_effective_gateway_alias(
    state: &AppState,
    mut request: ServeModelRequest,
) -> pumas_library::Result<ServeModelRequest> {
    if let Some(model_alias) = request.config.model_alias.as_deref() {
        let trimmed = model_alias.trim();
        if trimmed.is_empty() {
            return Ok(request);
        }
        request.config.model_alias = Some(trimmed.to_string());
        return Ok(request);
    }

    let model_id = request.model_id.trim();
    if model_id.is_empty() {
        return Ok(request);
    }

    let registry = ProviderRegistry::builtin();
    let model_alias = match registry
        .get(request.config.provider)
        .map(|behavior| behavior.gateway_alias_policy)
    {
        Some(ProviderGatewayAliasPolicy::OllamaModelName) => {
            let model = state.api.get_model(model_id).await?;
            let display = model
                .as_ref()
                .map(|record| record.cleaned_name.clone())
                .unwrap_or_else(|| derive_fallback_model_alias(model_id));
            pumas_app_manager::derive_ollama_name(&display)
        }
        Some(ProviderGatewayAliasPolicy::LibraryModelId) | None => model_id.to_string(),
    };
    request.config.model_alias = Some(model_alias);
    Ok(request)
}

fn effective_gateway_alias_from_config(request: &ServeModelRequest) -> String {
    request
        .config
        .model_alias
        .as_deref()
        .map(str::trim)
        .filter(|model_alias| !model_alias.is_empty())
        .unwrap_or_else(|| request.model_id.trim())
        .to_string()
}

async fn non_critical_failure_response(
    state: &AppState,
    error: ModelServeError,
) -> pumas_library::Result<Value> {
    let snapshot = state.api.record_serving_load_error(error.clone()).await?;
    let mut response = ServeModelResponse::non_critical_failure(error);
    response.snapshot = Some(snapshot);
    Ok(serde_json::to_value(response)?)
}

fn serving_error(
    code: ModelServeErrorCode,
    message: impl Into<String>,
    request: &ServeModelRequest,
) -> ModelServeError {
    ModelServeError::non_critical(code, message)
        .for_model(request.model_id.clone())
        .for_profile(request.config.profile_id.clone())
        .for_provider(request.config.provider)
}

fn derive_fallback_model_alias(model_id: &str) -> String {
    pumas_app_manager::derive_ollama_name(model_id.split('/').next_back().unwrap_or(model_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llama_cpp_router_models_url_normalizes_trailing_slash() {
        assert_eq!(
            llama_cpp_router_models_url("http://127.0.0.1:20617/"),
            "http://127.0.0.1:20617/v1/models"
        );
        assert_eq!(
            llama_cpp_router_models_url("http://127.0.0.1:20617"),
            "http://127.0.0.1:20617/v1/models"
        );
    }

    #[test]
    fn llama_cpp_router_model_load_url_normalizes_trailing_slash() {
        assert_eq!(
            llama_cpp_router_model_load_url("http://127.0.0.1:20617/"),
            "http://127.0.0.1:20617/models/load"
        );
        assert_eq!(
            llama_cpp_router_model_load_url("http://127.0.0.1:20617"),
            "http://127.0.0.1:20617/models/load"
        );
    }

    #[test]
    fn llama_cpp_router_model_unload_url_normalizes_trailing_slash() {
        assert_eq!(
            llama_cpp_router_model_unload_url("http://127.0.0.1:20617/"),
            "http://127.0.0.1:20617/models/unload"
        );
        assert_eq!(
            llama_cpp_router_model_unload_url("http://127.0.0.1:20617"),
            "http://127.0.0.1:20617/models/unload"
        );
    }

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
            config: pumas_library::models::ModelServingConfig {
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

    #[test]
    fn provider_request_model_id_uses_provider_behavior_policy() {
        let request = ServeModelRequest {
            model_id: "llm/qwen/model-gguf".to_string(),
            config: pumas_library::models::ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
                device_mode: RuntimeDeviceMode::Gpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: Some(8192),
                keep_loaded: true,
                model_alias: Some("qwen-gpu".to_string()),
            },
        };

        assert_eq!(provider_request_model_id(&request), "llm/qwen/model-gguf");

        let ollama_request = ServeModelRequest {
            model_id: "llm/qwen/model-gguf".to_string(),
            config: pumas_library::models::ModelServingConfig {
                provider: RuntimeProviderId::Ollama,
                profile_id: RuntimeProfileId::parse("ollama-default").unwrap(),
                device_mode: RuntimeDeviceMode::Gpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: Some(8192),
                keep_loaded: true,
                model_alias: Some("qwen-gpu".to_string()),
            },
        };

        assert_eq!(provider_request_model_id(&ollama_request), "qwen-gpu");
    }

    #[test]
    fn llama_cpp_runtime_gpu_backend_detection_requires_gpu_library() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("libggml-cpu-alderlake.so"), b"cpu").unwrap();

        assert!(!llama_cpp_runtime_has_gpu_backend(temp_dir.path()));

        std::fs::write(temp_dir.path().join("libggml-vulkan.so"), b"vulkan").unwrap();

        assert!(llama_cpp_runtime_has_gpu_backend(temp_dir.path()));
    }

    #[test]
    fn llama_cpp_runtime_support_error_rejects_gpu_profile_on_cpu_only_runtime() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("libggml-cpu-alderlake.so"), b"cpu").unwrap();
        let request = ServeModelRequest {
            model_id: "models/example.gguf".to_string(),
            config: pumas_library::models::ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("llama-gpu").unwrap(),
                device_mode: RuntimeDeviceMode::Gpu,
                device_id: None,
                gpu_layers: Some(-1),
                tensor_split: None,
                context_size: None,
                keep_loaded: true,
                model_alias: None,
            },
        };

        let error = llama_cpp_runtime_support_error(temp_dir.path(), &request).unwrap();

        assert_eq!(error.code, ModelServeErrorCode::DeviceUnavailable);
        assert!(error.message.contains("CPU-only"));
    }

    #[test]
    fn llama_cpp_runtime_support_error_allows_cpu_profile_on_cpu_runtime() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("libggml-cpu-alderlake.so"), b"cpu").unwrap();
        let request = ServeModelRequest {
            model_id: "models/example.gguf".to_string(),
            config: pumas_library::models::ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("llama-cpu").unwrap(),
                device_mode: RuntimeDeviceMode::Cpu,
                device_id: None,
                gpu_layers: Some(0),
                tensor_split: None,
                context_size: None,
                keep_loaded: true,
                model_alias: None,
            },
        };

        assert!(llama_cpp_runtime_support_error(temp_dir.path(), &request).is_none());
    }
}
