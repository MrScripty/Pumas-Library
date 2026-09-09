//! Process management handlers.

#[cfg(feature = "inference-plugins")]
use super::{get_version_manager, path_exists, require_str_param};
use super::{validate_existing_local_path, validate_external_url};
use crate::contract::OperationStatusOutcome;
#[cfg(feature = "inference-plugins")]
use crate::contract::RuntimeLaunchOutcome;
use crate::server::AppState;
#[cfg(feature = "inference-plugins")]
use serde_json::{json, Value};
#[cfg(feature = "inference-plugins")]
use tracing::{info, warn};

#[cfg(feature = "inference-plugins")]
pub async fn launch_ollama(state: &AppState) -> pumas_library::Result<RuntimeLaunchOutcome> {
    // Get the active version from ollama version_manager and launch it
    info!("launch_ollama: checking for ollama version manager");
    if let Some(vm) = get_version_manager(state, "ollama").await {
        let installed = vm.get_installed_versions().await?;
        info!(
            installed_version_count = installed.len(),
            "launch_ollama: installed versions resolved"
        );
        let active = vm.get_active_version().await?;
        info!(
            active_version_present = active.is_some(),
            "launch_ollama: active version resolved"
        );
        if let Some(tag) = active {
            let version_dir = vm.version_path(&tag);
            info!("launch_ollama: launching active version");
            let response = state.api.launch_ollama(&tag, &version_dir).await?;
            info!("launch_ollama: result success={}", response.success);
            Ok(response.into())
        } else {
            warn!("launch_ollama: no active version set");
            Ok(RuntimeLaunchOutcome::failure(
                "No active Ollama version set",
            ))
        }
    } else {
        warn!("launch_ollama: version manager not initialized");
        Ok(RuntimeLaunchOutcome::failure(
            "Version manager not initialized for ollama",
        ))
    }
}

#[cfg(feature = "inference-plugins")]
pub async fn stop_ollama(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let result = state.api.stop_ollama().await?;
    Ok(json!({ "success": result }))
}

#[cfg(feature = "inference-plugins")]
pub async fn is_ollama_running(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let running = state.api.is_ollama_running().await;
    Ok(serde_json::to_value(running)?)
}

#[cfg(feature = "inference-plugins")]
pub async fn launch_torch(state: &AppState) -> pumas_library::Result<RuntimeLaunchOutcome> {
    info!("launch_torch: checking for torch version manager");
    if let Some(vm) = get_version_manager(state, "torch").await {
        let active = vm.get_active_version().await?;
        info!(
            active_version_present = active.is_some(),
            "launch_torch: active version resolved"
        );
        if let Some(tag) = active {
            let version_dir = vm.version_path(&tag);
            info!("launch_torch: launching active version");
            let response = state.api.launch_torch(&tag, &version_dir).await?;
            info!("launch_torch: result success={}", response.success);
            Ok(response.into())
        } else {
            warn!("launch_torch: no active version set");
            Ok(RuntimeLaunchOutcome::failure("No active Torch version set"))
        }
    } else {
        warn!("launch_torch: version manager not initialized");
        Ok(RuntimeLaunchOutcome::failure(
            "Version manager not initialized for torch",
        ))
    }
}

#[cfg(feature = "inference-plugins")]
pub async fn stop_torch(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let result = state.api.stop_torch().await?;
    Ok(json!({ "success": result }))
}

#[cfg(feature = "inference-plugins")]
pub async fn is_torch_running(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let running = state.api.is_torch_running().await;
    Ok(serde_json::to_value(running)?)
}

pub async fn open_path(
    state: &AppState,
    path: String,
) -> pumas_library::Result<OperationStatusOutcome> {
    let path = validate_existing_local_path(path, "path").await?;
    let path = path.to_string_lossy().to_string();
    match state.api.open_path(&path).await {
        Ok(()) => Ok(OperationStatusOutcome::success()),
        Err(_) => Ok(OperationStatusOutcome::failed()),
    }
}

pub async fn open_url(
    state: &AppState,
    url: String,
) -> pumas_library::Result<OperationStatusOutcome> {
    let url = validate_external_url(url)?;
    match state.api.open_url(&url).await {
        Ok(()) => Ok(OperationStatusOutcome::success()),
        Err(_) => Ok(OperationStatusOutcome::failed()),
    }
}

#[cfg(feature = "inference-plugins")]
pub async fn open_active_install(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    // Get the active version from version_manager and open its directory
    if let Some(vm) = get_version_manager(state, &app_id_str).await {
        if let Some(tag) = vm.get_active_version().await? {
            let version_dir = vm.version_path(&tag);
            if path_exists(&version_dir).await? {
                match state.api.open_directory(&version_dir).await {
                    Ok(()) => Ok(json!({"success": true})),
                    Err(e) => Ok(json!({
                        "success": false,
                        "error": crate::contract::PublicError::from(&e).message
                    })),
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
