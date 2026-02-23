//! Status & system check handlers.

use super::{
    detect_sandbox_environment, get_bool_param, require_str_param,
    sync_version_paths_to_process_manager,
};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_status(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    // Ensure process manager has current version paths for accurate running detection
    sync_version_paths_to_process_manager(state).await;
    let mut response = state.api.get_status().await?;

    // Enrich with version-specific data from ComfyUI version manager
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        // Get active version
        let active_version = vm.get_active_version().await.ok().flatten();

        // Check dependencies for active version
        if let Some(ref tag) = active_version {
            if let Ok(deps) = vm.check_dependencies(tag).await {
                response.deps_ready = deps.missing.is_empty();
            }
        }

        // Check if active version is patched
        if let Some(ref tag) = active_version {
            response.patched = state.api.is_patched(Some(tag));
        }

        // Get shortcut states for active version
        if let Some(ref tag) = active_version {
            response.shortcut_version = Some(tag.clone());
            let sm_lock = state.shortcut_manager.read().await;
            if let Some(ref sm) = *sm_lock {
                let shortcut_state = sm.get_version_shortcut_state(tag);
                response.menu_shortcut = shortcut_state.menu;
                response.desktop_shortcut = shortcut_state.desktop;
            }
        }
    }
    drop(managers);

    Ok(serde_json::to_value(response)?)
}

pub async fn get_disk_space(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let response = state.api.get_disk_space().await?;
    Ok(serde_json::to_value(response)?)
}

pub async fn get_system_resources(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let response = state.api.get_system_resources().await?;
    Ok(serde_json::to_value(response)?)
}

pub async fn get_launcher_version(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let version_info = state.api.get_launcher_version();
    Ok(version_info)
}

pub async fn check_launcher_updates(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let force_refresh = get_bool_param(params, "force_refresh", "forceRefresh").unwrap_or(false);
    let result = state.api.check_launcher_updates(force_refresh).await;
    Ok(serde_json::to_value(result)?)
}

pub async fn apply_launcher_update(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let result = state.api.apply_launcher_update().await;
    Ok(serde_json::to_value(result)?)
}

pub async fn restart_launcher(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    match state.api.restart_launcher() {
        Ok(success) => Ok(json!({
            "success": success
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn get_sandbox_info(_state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let (is_sandboxed, sandbox_type, limitations) = detect_sandbox_environment();
    Ok(json!({
        "success": true,
        "is_sandboxed": is_sandboxed,
        "sandbox_type": sandbox_type,
        "limitations": limitations
    }))
}

pub async fn check_git(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let result = state.api.check_git();
    Ok(serde_json::to_value(result)?)
}

pub async fn check_brave(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let result = state.api.check_brave();
    Ok(serde_json::to_value(result)?)
}

pub async fn check_setproctitle(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let result = state.api.check_setproctitle();
    Ok(serde_json::to_value(result)?)
}

pub async fn get_network_status(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // TODO: Implement network status from network layer
    Ok(json!({
        "success": true,
        "total_requests": 0,
        "successful_requests": 0,
        "failed_requests": 0,
        "circuit_breaker_rejections": 0,
        "retries": 0,
        "success_rate": 1.0,
        "circuit_states": {},
        "is_offline": false
    }))
}

pub async fn get_library_status(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // TODO: Implement library status
    Ok(json!({
        "success": true,
        "indexing": false,
        "deep_scan_in_progress": false,
        "model_count": 0,
        "pending_lookups": null,
        "deep_scan_progress": null
    }))
}

pub async fn get_app_status(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id = require_str_param(params, "app_id", "appId")?;
    let running = match app_id.as_str() {
        "comfyui" => state.api.is_comfyui_running().await,
        "ollama" => state.api.is_ollama_running().await,
        "torch" => state.api.is_torch_running().await,
        _ => false,
    };
    Ok(json!({
        "success": true,
        "running": running
    }))
}
