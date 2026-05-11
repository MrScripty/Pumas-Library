//! Ollama serving adapter used by the serving RPC boundary.

use super::serving::{derive_fallback_model_alias, non_critical_failure_response, serving_error};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeErrorCode, RuntimeProfileId, RuntimeProviderId, ServeModelRequest,
    ServeModelResponse, ServedModelLoadState, ServedModelStatus, UnserveModelRequest,
    UnserveModelResponse,
};
use pumas_library::ExecutableArtifactFormat;
use serde_json::Value;
use tracing::warn;

pub(super) async fn serve_ollama_model(
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

pub(super) async fn unserve_ollama_model(
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
    if ExecutableArtifactFormat::from_path(&gguf_path) != Some(ExecutableArtifactFormat::Gguf) {
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
