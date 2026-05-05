//! Ollama model management handlers.

use super::{get_str_param, parse_params, require_str_param};
use crate::server::AppState;
use pumas_library::models::{RuntimeProfileId, RuntimeProviderId};
use serde::Deserialize;
use serde_json::{json, Value};

async fn get_primary_model_file(
    library: std::sync::Arc<pumas_library::ModelLibrary>,
    model_id: String,
) -> pumas_library::Result<Option<std::path::PathBuf>> {
    tokio::task::spawn_blocking(move || Ok(library.get_primary_model_file(&model_id)))
        .await
        .map_err(|err| {
            pumas_library::error::PumasError::Other(format!(
                "Failed to join primary model file lookup task: {}",
                err
            ))
        })?
}

pub async fn ollama_list_models(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");
    let client = pumas_app_manager::OllamaClient::new(connection_url);
    let models = client.list_models().await?;
    Ok(json!({
        "success": true,
        "models": models
    }))
}

#[derive(Debug, Deserialize)]
struct OllamaProfileParams {
    profile_id: Option<RuntimeProfileId>,
}

#[derive(Debug, Deserialize)]
struct OllamaProfileModelParams {
    model_name: String,
    model_id: Option<String>,
    profile_id: Option<RuntimeProfileId>,
}

#[derive(Debug, Deserialize)]
struct OllamaProfileCreateModelParams {
    model_id: String,
    model_name: Option<String>,
    profile_id: Option<RuntimeProfileId>,
}

async fn resolve_ollama_profile_endpoint(
    state: &AppState,
    profile_id: Option<RuntimeProfileId>,
) -> pumas_library::Result<pumas_library::models::RuntimeEndpointUrl> {
    state
        .api
        .resolve_runtime_profile_endpoint(RuntimeProviderId::Ollama, profile_id)
        .await
}

async fn resolve_ollama_operation_endpoint(
    state: &AppState,
    model_id: Option<&str>,
    profile_id: Option<RuntimeProfileId>,
) -> pumas_library::Result<pumas_library::models::RuntimeEndpointUrl> {
    match model_id {
        Some(model_id) if !model_id.trim().is_empty() => {
            state
                .api
                .resolve_model_runtime_profile_endpoint(
                    RuntimeProviderId::Ollama,
                    model_id,
                    profile_id,
                )
                .await
        }
        _ => resolve_ollama_profile_endpoint(state, profile_id).await,
    }
}

pub async fn ollama_list_models_for_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: OllamaProfileParams = parse_params("ollama_list_models_for_profile", params)?;
    let endpoint = resolve_ollama_profile_endpoint(state, command.profile_id).await?;
    let client = pumas_app_manager::OllamaClient::new(Some(endpoint.as_str()));
    let models = client.list_models().await?;
    Ok(json!({
        "success": true,
        "models": models
    }))
}

pub async fn ollama_load_model_for_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: OllamaProfileModelParams = parse_params("ollama_load_model_for_profile", params)?;
    let model_name = command.model_name.trim();
    if model_name.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "model_name is required"
        }));
    }

    let endpoint =
        resolve_ollama_operation_endpoint(state, command.model_id.as_deref(), command.profile_id)
            .await?;
    let client = pumas_app_manager::OllamaClient::new(Some(endpoint.as_str()));
    client.load_model(model_name).await?;

    Ok(json!({ "success": true }))
}

pub async fn ollama_unload_model_for_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: OllamaProfileModelParams =
        parse_params("ollama_unload_model_for_profile", params)?;
    let model_name = command.model_name.trim();
    if model_name.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "model_name is required"
        }));
    }

    let endpoint =
        resolve_ollama_operation_endpoint(state, command.model_id.as_deref(), command.profile_id)
            .await?;
    let client = pumas_app_manager::OllamaClient::new(Some(endpoint.as_str()));
    client.unload_model(model_name).await?;

    Ok(json!({ "success": true }))
}

pub async fn ollama_delete_model_for_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: OllamaProfileModelParams =
        parse_params("ollama_delete_model_for_profile", params)?;
    let model_name = command.model_name.trim();
    if model_name.is_empty() {
        return Ok(json!({
            "success": false,
            "error": "model_name is required"
        }));
    }

    let endpoint = resolve_ollama_profile_endpoint(state, command.profile_id).await?;
    let client = pumas_app_manager::OllamaClient::new(Some(endpoint.as_str()));
    client.delete_model(model_name).await?;

    Ok(json!({ "success": true }))
}

pub async fn ollama_create_model_for_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: OllamaProfileCreateModelParams =
        parse_params("ollama_create_model_for_profile", params)?;
    let endpoint = state
        .api
        .resolve_model_runtime_profile_endpoint(
            RuntimeProviderId::Ollama,
            &command.model_id,
            command.profile_id,
        )
        .await?;
    create_ollama_model(
        state,
        command.model_id,
        command.model_name.as_deref(),
        Some(endpoint.as_str()),
    )
    .await
}

pub async fn ollama_create_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let model_name = get_str_param(params, "model_name", "modelName");
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    create_ollama_model(state, model_id, model_name, connection_url).await
}

async fn create_ollama_model(
    state: &AppState,
    model_id: String,
    model_name: Option<&str>,
    connection_url: Option<&str>,
) -> pumas_library::Result<Value> {
    // Resolve GGUF path from library
    let library = state.api.model_library().clone();
    let primary_file = get_primary_model_file(library.clone(), model_id.clone()).await?;
    let gguf_path = match primary_file {
        Some(path) => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            if ext != "gguf" {
                return Ok(json!({
                    "success": false,
                    "error": format!("Model file is not GGUF format (found .{})", ext)
                }));
            }
            path
        }
        None => {
            return Ok(json!({
                "success": false,
                "error": format!("No model file found for '{}'", model_id)
            }));
        }
    };

    // Look up model record for name derivation and cached SHA256
    let model_record = library.get_model(&model_id).await?;

    let ollama_name = match model_name {
        Some(name) => name.to_string(),
        None => {
            let display = model_record
                .as_ref()
                .map(|r| r.cleaned_name.clone())
                .unwrap_or_else(|| {
                    model_id
                        .split('/')
                        .next_back()
                        .unwrap_or(&model_id)
                        .to_string()
                });
            pumas_app_manager::derive_ollama_name(&display)
        }
    };

    // Use cached SHA256 from library metadata if available
    let known_sha256 = model_record
        .as_ref()
        .and_then(|r| r.hashes.get("sha256"))
        .cloned();

    let client = pumas_app_manager::OllamaClient::new(connection_url);
    client
        .create_model(&ollama_name, &gguf_path, known_sha256.as_deref())
        .await?;

    // Auto-load the model into VRAM/RAM so it's ready for inference.
    client.load_model(&ollama_name).await?;

    Ok(json!({
        "success": true,
        "model_name": ollama_name
    }))
}

pub async fn ollama_delete_model(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_name = require_str_param(params, "model_name", "modelName")?;
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = pumas_app_manager::OllamaClient::new(connection_url);
    client.delete_model(&model_name).await?;

    Ok(json!({ "success": true }))
}

pub async fn ollama_load_model(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_name = require_str_param(params, "model_name", "modelName")?;
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = pumas_app_manager::OllamaClient::new(connection_url);
    client.load_model(&model_name).await?;

    Ok(json!({ "success": true }))
}

pub async fn ollama_unload_model(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_name = require_str_param(params, "model_name", "modelName")?;
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = pumas_app_manager::OllamaClient::new(connection_url);
    client.unload_model(&model_name).await?;

    Ok(json!({ "success": true }))
}

pub async fn ollama_list_running(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = pumas_app_manager::OllamaClient::new(connection_url);
    let models = client.list_running_models().await?;

    Ok(json!({ "success": true, "models": models }))
}
