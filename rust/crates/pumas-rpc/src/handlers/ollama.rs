//! Ollama model management handlers.

use super::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn ollama_list_models(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");
    let client = pumas_app_manager::OllamaClient::new(connection_url);
    let models = client.list_models().await?;
    Ok(json!({
        "success": true,
        "models": models
    }))
}

pub async fn ollama_create_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let model_name = get_str_param(params, "model_name", "modelName");
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    // Resolve GGUF path from library
    let library = state.api.model_library();
    let primary_file = library.get_primary_model_file(&model_id);
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
