//! Version lifecycle handlers.

use crate::handlers::{
    get_str_param, get_version_manager, require_str_param, require_version_manager,
};
use crate::server::AppState;
use serde_json::{json, Value};
use tracing::warn;

pub async fn get_installed_versions(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<crate::contract::InstalledVersionsOutcome> {
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    if let Some(vm) = get_version_manager(state, &app_id_str).await {
        let versions = vm.get_installed_versions().await?;
        Ok(crate::contract::InstalledVersionsOutcome::new(versions))
    } else {
        Ok(crate::contract::InstalledVersionsOutcome::new(vec![]))
    }
}

pub async fn get_active_version(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<crate::contract::SelectedVersionOutcome> {
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    if let Some(vm) = get_version_manager(state, app_id_str).await {
        let version = vm.get_active_version().await?;
        Ok(crate::contract::SelectedVersionOutcome::new(version))
    } else {
        Ok(crate::contract::SelectedVersionOutcome::new(None))
    }
}

pub async fn get_default_version(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<crate::contract::SelectedVersionOutcome> {
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    if let Some(vm) = get_version_manager(state, app_id_str).await {
        let version = vm.get_default_version().await?;
        Ok(crate::contract::SelectedVersionOutcome::new(version))
    } else {
        Ok(crate::contract::SelectedVersionOutcome::new(None))
    }
}

pub async fn set_default_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    let vm = require_version_manager(state, app_id_str).await?;
    let result = vm.set_default_version(tag).await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn switch_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    let vm = require_version_manager(state, app_id_str).await?;
    let result = vm.set_active_version(&tag).await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn install_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = require_str_param(params, "app_id", "appId")?;

    if let Some(vm) = get_version_manager(state, &app_id_str).await {
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
                warn!("Failed to start requested version installation");
                Ok(json!({
                    "success": false,
                    "error": crate::contract::PublicError::from(&e).message
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
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    let vm = require_version_manager(state, app_id_str).await?;
    let result = vm.remove_version(&tag).await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn cancel_installation(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    if let Some(vm) = get_version_manager(state, app_id_str).await {
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
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    if let Some(vm) = get_version_manager(state, app_id_str).await {
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
) -> pumas_library::Result<crate::contract::ValidateInstallationsOutcome> {
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    if let Some(vm) = get_version_manager(state, app_id_str).await {
        let result = vm.validate_installations().await?;
        crate::contract::ValidateInstallationsOutcome::new(
            result.removed_tags,
            result.orphaned_dirs,
            result.valid_count,
        )
    } else {
        crate::contract::ValidateInstallationsOutcome::new(vec![], vec![], 0)
    }
}
