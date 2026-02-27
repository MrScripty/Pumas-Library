//! Version lifecycle handlers.

use crate::handlers::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};
use tracing::warn;

pub async fn get_installed_versions(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let versions = vm.get_installed_versions().await?;
        // Return raw array - wrapper.rs will add {success, versions} wrapper
        Ok(serde_json::to_value(versions)?)
    } else {
        Ok(json!([]))
    }
}

pub async fn get_active_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let version = vm.get_active_version().await?;
        // Return raw value - wrapper.rs will add {success, version} wrapper
        Ok(serde_json::to_value(version)?)
    } else {
        Ok(Value::Null)
    }
}

pub async fn get_default_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let version = vm.get_default_version().await?;
        // Return raw value - wrapper.rs will add {success, version} wrapper
        Ok(serde_json::to_value(version)?)
    } else {
        Ok(Value::Null)
    }
}

pub async fn set_default_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let result = vm.set_default_version(tag).await?;
        Ok(serde_json::to_value(result)?)
    } else {
        Err(pumas_library::PumasError::Config {
            message: format!("Version manager not initialized for app: {}", app_id_str),
        })
    }
}

pub async fn switch_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let result = vm.set_active_version(&tag).await?;
        Ok(serde_json::to_value(result)?)
    } else {
        Err(pumas_library::PumasError::Config {
            message: format!("Version manager not initialized for app: {}", app_id_str),
        })
    }
}

pub async fn install_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");

    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        // Start the installation (returns a progress receiver)
        match vm.install_version(&tag).await {
            Ok(_rx) => {
                // Installation started successfully
                // Progress can be monitored via get_installation_progress
                Ok(json!({
                    "success": true,
                    "message": format!("Installation of {} started", tag)
                }))
            }
            Err(e) => {
                warn!("Failed to start installation of {}: {}", tag, e);
                Ok(json!({
                    "success": false,
                    "error": e.to_string()
                }))
            }
        }
    } else {
        Ok(json!({
            "success": false,
            "error": format!("Version manager not initialized for app: {}", app_id_str)
        }))
    }
}

pub async fn remove_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let result = vm.remove_version(&tag).await?;
        Ok(serde_json::to_value(result)?)
    } else {
        Err(pumas_library::PumasError::Config {
            message: format!("Version manager not initialized for app: {}", app_id_str),
        })
    }
}

pub async fn cancel_installation(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let result = vm.cancel_installation().await?;
        Ok(serde_json::to_value(result)?)
    } else {
        Ok(serde_json::to_value(false)?)
    }
}

pub async fn get_installation_progress(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let progress = vm.get_installation_progress().await;
        Ok(serde_json::to_value(progress)?)
    } else {
        Ok(serde_json::to_value::<
            Option<pumas_library::models::InstallationProgress>,
        >(None)?)
    }
}

pub async fn validate_installations(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let result = vm.validate_installations().await?;
        Ok(serde_json::to_value(result)?)
    } else {
        Ok(json!({
            "removed_tags": [],
            "orphaned_dirs": [],
            "valid_count": 0
        }))
    }
}
