//! User-directed model serving RPC handlers.

use super::parse_params;
use crate::server::AppState;
use pumas_library::models::{
    ModelServeError, ModelServeErrorCode, RuntimeDeviceSettings, RuntimeProviderId,
    RuntimeProviderMode, ServeModelRequest, ServeModelResponse, ServedModelLoadState,
    ServedModelStatus, UnserveModelRequest, UnserveModelResponse,
};
use pumas_library::runtime_profiles::RuntimeProfileLaunchOverrides;
use serde::Deserialize;
use serde_json::Value;
use std::{path::PathBuf, time::Duration};
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
    Ok(serde_json::to_value(
        state
            .api
            .validate_model_serving_config(command.request)
            .await?,
    )?)
}

pub async fn serve_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let command: ServeModelParams = parse_params("serve_model", params)?;
    let validation = state
        .api
        .validate_model_serving_config(command.request.clone())
        .await?;
    if !validation.valid {
        let error = validation.errors.into_iter().next().unwrap_or_else(|| {
            serving_error(
                ModelServeErrorCode::InvalidRequest,
                "serving request is invalid",
                &command.request,
            )
        });
        return non_critical_failure_response(state, error).await;
    }

    match command.request.config.provider {
        RuntimeProviderId::Ollama => serve_ollama_model(state, command.request).await,
        RuntimeProviderId::LlamaCpp => serve_llama_cpp_model(state, command.request).await,
    }
}

pub async fn unserve_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let command: UnserveModelParams = parse_params("unserve_model", params)?;
    let served = state
        .api
        .find_served_model(
            &command.request.model_id,
            command.request.profile_id.as_ref(),
        )
        .await?;
    let profile_id = command
        .request
        .profile_id
        .clone()
        .or_else(|| served.as_ref().map(|status| status.profile_id.clone()));
    let Some(profile_id) = profile_id else {
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: None,
            unloaded: false,
            snapshot: Some(state.api.get_serving_status().await?.snapshot),
        })?);
    };
    let model_alias = command
        .request
        .model_alias
        .clone()
        .or_else(|| {
            served
                .as_ref()
                .and_then(|status| status.model_alias.clone())
        })
        .unwrap_or_else(|| derive_fallback_model_alias(&command.request.model_id));

    if served
        .as_ref()
        .is_some_and(|status| status.provider == RuntimeProviderId::LlamaCpp)
    {
        return unserve_llama_cpp_model(state, command.request, profile_id, model_alias).await;
    }

    let endpoint = match state
        .api
        .resolve_model_runtime_profile_endpoint_for_operation(
            RuntimeProviderId::Ollama,
            &command.request.model_id,
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
            &command.request.model_id,
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
        model_alias: request.config.model_alias.clone(),
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
    if profile_not_running || endpoint_unreachable {
        if let Some(error) = launch_llama_cpp_router_profile(state, &request).await? {
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

    let model_alias = request
        .config
        .model_alias
        .clone()
        .unwrap_or_else(|| request.model_id.clone());
    if let Err(message) = llama_cpp_router_load_model(endpoint.as_str(), &model_alias).await {
        warn!("llama.cpp router model load failed: {}", message);
        return non_critical_failure_response(
            state,
            serving_error(ModelServeErrorCode::ProviderLoadFailed, message, &request),
        )
        .await;
    }
    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(model_alias.clone()),
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

async fn launch_llama_cpp_router_profile(
    state: &AppState,
    request: &ServeModelRequest,
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
        .launch_runtime_profile(request.config.profile_id.clone(), &tag, &version_dir)
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

async fn unserve_llama_cpp_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: pumas_library::models::RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let is_dedicated = state
        .api
        .get_runtime_profiles_snapshot()
        .await?
        .snapshot
        .profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
        .is_some_and(|profile| {
            profile.provider_mode == pumas_library::models::RuntimeProviderMode::LlamaCppDedicated
        });

    if !is_dedicated {
        let snapshot = state
            .api
            .record_unserved_model(
                &request.model_id,
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
}
