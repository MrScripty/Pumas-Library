//! Process management handlers.

use super::{get_str_param, require_str_param, sync_version_paths_to_process_manager};
use crate::server::AppState;
use serde_json::{json, Value};
use tracing::{info, warn};

pub async fn is_comfyui_running(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    // Ensure process manager has current version paths for accurate detection
    sync_version_paths_to_process_manager(state).await;
    let running = state.api.is_comfyui_running().await;
    Ok(serde_json::to_value(running)?)
}

pub async fn stop_comfyui(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    // Ensure process manager has current version paths for proper PID file cleanup
    sync_version_paths_to_process_manager(state).await;
    let result = state.api.stop_comfyui().await?;
    Ok(json!({ "success": result }))
}

pub async fn launch_comfyui(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    // Ensure process manager has current version paths
    sync_version_paths_to_process_manager(state).await;
    // Get the active version from comfyui version_manager and launch it
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        let active = vm.get_active_version().await?;
        if let Some(tag) = active {
            let version_dir = vm.version_path(&tag);
            drop(managers); // Release lock before calling api
            let response = state.api.launch_version(&tag, &version_dir).await?;
            Ok(serde_json::to_value(response)?)
        } else {
            Ok(json!({
                "success": false,
                "error": "No active version set"
            }))
        }
    } else {
        Ok(json!({
            "success": false,
            "error": "Version manager not initialized for comfyui"
        }))
    }
}

pub async fn launch_ollama(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    // Get the active version from ollama version_manager and launch it
    let managers = state.version_managers.read().await;
    info!("launch_ollama: checking for ollama version manager");
    if let Some(vm) = managers.get("ollama") {
        let installed = vm.get_installed_versions().await?;
        info!("launch_ollama: installed versions: {:?}", installed);
        let active = vm.get_active_version().await?;
        info!("launch_ollama: active version: {:?}", active);
        if let Some(tag) = active {
            let version_dir = vm.version_path(&tag);
            info!(
                "launch_ollama: launching tag={} from {:?}",
                tag, version_dir
            );
            drop(managers); // Release lock before calling api
            let response = state.api.launch_ollama(&tag, &version_dir).await?;
            info!("launch_ollama: result success={}", response.success);
            Ok(serde_json::to_value(response)?)
        } else {
            warn!("launch_ollama: no active version set");
            Ok(json!({
                "success": false,
                "error": "No active Ollama version set"
            }))
        }
    } else {
        warn!("launch_ollama: version manager not initialized");
        Ok(json!({
            "success": false,
            "error": "Version manager not initialized for ollama"
        }))
    }
}

pub async fn stop_ollama(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let result = state.api.stop_ollama().await?;
    Ok(json!({ "success": result }))
}

pub async fn is_ollama_running(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let running = state.api.is_ollama_running().await;
    Ok(serde_json::to_value(running)?)
}

pub async fn launch_torch(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let managers = state.version_managers.read().await;
    info!("launch_torch: checking for torch version manager");
    if let Some(vm) = managers.get("torch") {
        let active = vm.get_active_version().await?;
        info!("launch_torch: active version: {:?}", active);
        if let Some(tag) = active {
            let version_dir = vm.version_path(&tag);
            info!("launch_torch: launching tag={} from {:?}", tag, version_dir);
            drop(managers);
            let response = state.api.launch_torch(&tag, &version_dir).await?;
            info!("launch_torch: result success={}", response.success);
            Ok(serde_json::to_value(response)?)
        } else {
            warn!("launch_torch: no active version set");
            Ok(json!({
                "success": false,
                "error": "No active Torch version set"
            }))
        }
    } else {
        warn!("launch_torch: version manager not initialized");
        Ok(json!({
            "success": false,
            "error": "Version manager not initialized for torch"
        }))
    }
}

pub async fn stop_torch(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let result = state.api.stop_torch().await?;
    Ok(json!({ "success": result }))
}

pub async fn is_torch_running(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let running = state.api.is_torch_running().await;
    Ok(serde_json::to_value(running)?)
}

pub async fn open_path(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let path = require_str_param(params, "path", "path")?;
    match state.api.open_path(&path) {
        Ok(()) => Ok(json!({"success": true})),
        Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
    }
}

pub async fn open_url(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let url = require_str_param(params, "url", "url")?;
    match state.api.open_url(&url) {
        Ok(()) => Ok(json!({"success": true})),
        Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
    }
}

pub async fn open_active_install(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    // Get the active version from version_manager and open its directory
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        if let Some(tag) = vm.get_active_version().await? {
            let version_dir = vm.version_path(&tag);
            drop(managers);
            if version_dir.exists() {
                match state.api.open_directory(&version_dir) {
                    Ok(()) => Ok(json!({"success": true})),
                    Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
                }
            } else {
                Ok(json!({"success": false, "error": "Version directory not found"}))
            }
        } else {
            Ok(json!({"success": false, "error": "No active version set"}))
        }
    } else {
        Ok(
            json!({"success": false, "error": format!("Version manager not initialized for app: {}", app_id_str)}),
        )
    }
}
