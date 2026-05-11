//! User-directed model serving RPC handlers.

use super::parse_params;
use super::serving_llama_cpp::{serve_llama_cpp_model, unserve_llama_cpp_model};
use super::serving_ollama::{serve_ollama_model, unserve_ollama_model};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeError, ModelServeErrorCode, ServeModelRequest, ServeModelResponse,
    UnserveModelRequest, UnserveModelResponse,
};
use pumas_library::{
    ProviderGatewayAliasPolicy, ProviderServingAdapterKind, ProviderUnloadBehavior,
};
use serde::Deserialize;
use serde_json::Value;

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

    match state
        .provider_registry
        .get(request.config.provider)
        .map(|behavior| behavior.serving_adapter_kind)
    {
        Some(ProviderServingAdapterKind::OllamaProviderApi) => {
            serve_ollama_model(state, request).await
        }
        Some(ProviderServingAdapterKind::LlamaCppRuntime) => {
            serve_llama_cpp_model(state, request).await
        }
        Some(ProviderServingAdapterKind::OnnxRuntime) => {
            let error = serving_error(
                ModelServeErrorCode::UnsupportedProvider,
                "ONNX Runtime serving is not wired in this slice",
                &request,
            );
            non_critical_failure_response(state, error).await
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

    match state
        .provider_registry
        .get(served_status.provider)
        .map(|behavior| behavior.unload_behavior)
    {
        Some(ProviderUnloadBehavior::ProviderApi) => {
            unserve_ollama_model(state, command.request, profile_id, model_alias).await
        }
        Some(ProviderUnloadBehavior::RouterPreset) => {
            unserve_llama_cpp_model(state, command.request, profile_id, model_alias).await
        }
        Some(ProviderUnloadBehavior::SessionManager) => {
            Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("ONNX Runtime unload is not wired in this slice".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?)
        }
        None => Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: Some("served model provider is not registered".to_string()),
            unloaded: false,
            snapshot: Some(state.api.get_serving_status().await?.snapshot),
        })?),
    }
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

    let model_alias = match state
        .provider_registry
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

pub(super) fn effective_gateway_alias_from_config(request: &ServeModelRequest) -> String {
    request
        .config
        .model_alias
        .as_deref()
        .map(str::trim)
        .filter(|model_alias| !model_alias.is_empty())
        .unwrap_or_else(|| request.model_id.trim())
        .to_string()
}

pub(super) async fn non_critical_failure_response(
    state: &AppState,
    error: ModelServeError,
) -> pumas_library::Result<Value> {
    let snapshot = state.api.record_serving_load_error(error.clone()).await?;
    let mut response = ServeModelResponse::non_critical_failure(error);
    response.snapshot = Some(snapshot);
    Ok(serde_json::to_value(response)?)
}

pub(super) fn serving_error(
    code: ModelServeErrorCode,
    message: impl Into<String>,
    request: &ServeModelRequest,
) -> ModelServeError {
    ModelServeError::non_critical(code, message)
        .for_model(request.model_id.clone())
        .for_profile(request.config.profile_id.clone())
        .for_provider(request.config.provider)
}

pub(super) fn derive_fallback_model_alias(model_id: &str) -> String {
    pumas_app_manager::derive_ollama_name(model_id.split('/').next_back().unwrap_or(model_id))
}
