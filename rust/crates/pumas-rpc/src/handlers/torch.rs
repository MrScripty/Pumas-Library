//! Torch inference server handlers.

use super::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn torch_list_slots(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");
    let client = pumas_app_manager::TorchClient::new(connection_url);
    let slots = client.list_slots().await?;
    Ok(json!({
        "success": true,
        "slots": slots
    }))
}

pub async fn torch_load_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let device = get_str_param(params, "device", "device").unwrap_or("auto");
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let descriptor = match state
        .api
        .resolve_model_execution_descriptor(&model_id)
        .await
    {
        Ok(descriptor) => descriptor,
        Err(err) => {
            return Ok(json!({
                "success": false,
                "error": err.to_string()
            }));
        }
    };

    let library = state.api.model_library();
    let model_record = library.get_model(&model_id).await?;
    let model_name = model_record
        .as_ref()
        .map(|r| r.cleaned_name.clone())
        .unwrap_or_else(|| {
            model_id
                .split('/')
                .next_back()
                .unwrap_or(&model_id)
                .to_string()
        });

    let compute_device = pumas_app_manager::ComputeDevice::from_server_string(device);
    let client = pumas_app_manager::TorchClient::new(connection_url);
    let slot = client
        .load_model(&descriptor.entry_path, &model_name, &compute_device, None)
        .await?;

    Ok(json!({
        "success": true,
        "slot": slot
    }))
}

pub async fn torch_unload_model(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let slot_id = require_str_param(params, "slot_id", "slotId")?;
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = pumas_app_manager::TorchClient::new(connection_url);
    client.unload_model(&slot_id).await?;

    Ok(json!({ "success": true }))
}

pub async fn torch_get_status(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = pumas_app_manager::TorchClient::new(connection_url);
    let status = client.get_status().await?;

    Ok(json!({
        "success": true,
        "status": status
    }))
}

pub async fn torch_list_devices(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = pumas_app_manager::TorchClient::new(connection_url);
    let devices = client.list_devices().await?;

    Ok(json!({
        "success": true,
        "devices": devices
    }))
}

pub async fn torch_configure(_state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");
    let config: pumas_app_manager::TorchServerConfig =
        serde_json::from_value(params.get("config").cloned().unwrap_or_default()).map_err(|e| {
            pumas_library::PumasError::InvalidParams {
                message: format!("Invalid torch config: {}", e),
            }
        })?;

    let client = pumas_app_manager::TorchClient::new(connection_url);
    client.configure(&config).await?;

    Ok(json!({ "success": true }))
}
