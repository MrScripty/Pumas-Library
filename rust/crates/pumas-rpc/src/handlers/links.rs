//! Link management handlers.

use super::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_link_health(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let version_tag = get_str_param(params, "version_tag", "versionTag");
    let response = state.api.get_link_health(version_tag).await?;
    Ok(serde_json::to_value(response)?)
}

pub async fn clean_broken_links(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let response = state.api.clean_broken_links().await?;
    Ok(serde_json::to_value(response)?)
}

pub async fn remove_orphaned_links(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let _version_tag = require_str_param(params, "version_tag", "versionTag")?;
    // Orphaned links are handled as part of clean_broken_links
    let response = state.api.clean_broken_links().await?;
    Ok(json!({
        "success": response.success,
        "removed": response.cleaned
    }))
}

pub async fn get_links_for_model(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let response = state.api.get_links_for_model(&model_id).await?;
    Ok(serde_json::to_value(response)?)
}

pub async fn delete_model_with_cascade(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let response = state.api.delete_model_with_cascade(&model_id).await?;
    Ok(serde_json::to_value(response)?)
}

pub async fn preview_model_mapping(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let version_tag = require_str_param(params, "version_tag", "versionTag")?;
    // Get the models path from comfyui version_manager
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        let version_path = vm.version_path(&version_tag);
        let models_path = version_path.join("models");
        drop(managers);
        let response = state
            .api
            .preview_model_mapping(&version_tag, &models_path)
            .await?;
        Ok(serde_json::to_value(response)?)
    } else {
        Ok(json!({
            "success": false,
            "error": "Version manager not initialized for comfyui"
        }))
    }
}

pub async fn apply_model_mapping(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let version_tag = require_str_param(params, "version_tag", "versionTag")?;
    // Get the models path from comfyui version_manager
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        let version_path = vm.version_path(&version_tag);
        let models_path = version_path.join("models");
        drop(managers);
        let response = state
            .api
            .apply_model_mapping(&version_tag, &models_path)
            .await?;
        Ok(serde_json::to_value(response)?)
    } else {
        Ok(json!({
            "success": false,
            "error": "Version manager not initialized for comfyui"
        }))
    }
}

pub async fn sync_models_incremental(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let version_tag = require_str_param(params, "version_tag", "versionTag")?;
    // Get the models path from comfyui version_manager
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        let version_path = vm.version_path(&version_tag);
        let models_path = version_path.join("models");
        drop(managers);
        let response = state
            .api
            .sync_models_incremental(&version_tag, &models_path)
            .await?;
        Ok(serde_json::to_value(response)?)
    } else {
        Ok(json!({
            "success": false,
            "error": "Version manager not initialized for comfyui"
        }))
    }
}

pub async fn sync_with_resolutions(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // TODO: Implement sync with resolutions
    Ok(json!({
        "success": true,
        "synced": 0
    }))
}

pub async fn get_cross_filesystem_warning(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let _version_tag = require_str_param(params, "version_tag", "versionTag")?;
    // TODO: Check for cross-filesystem issues
    Ok(json!({
        "success": true,
        "warning": null,
        "affected_models": []
    }))
}

pub async fn get_file_link_count(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let file_path = require_str_param(params, "file_path", "filePath")?;
    // Count hard links to a file
    let path = std::path::Path::new(&file_path);
    if path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                return Ok(json!({
                    "success": true,
                    "count": metadata.nlink()
                }));
            }
        }
    }
    Ok(json!({
        "success": true,
        "count": 1
    }))
}

pub async fn check_files_writable(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    // Check if files can be written/modified
    let file_paths: Vec<String> = params
        .get("file_paths")
        .or_else(|| params.get("filePaths"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let results: Vec<_> = file_paths
        .iter()
        .map(|p| {
            let path = std::path::Path::new(p);
            let writable = if path.exists() {
                std::fs::metadata(path)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false)
            } else {
                // Check if parent directory is writable
                path.parent()
                    .map(|parent| {
                        std::fs::metadata(parent)
                            .map(|m| !m.permissions().readonly())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            };
            json!({
                "path": p,
                "writable": writable
            })
        })
        .collect();

    Ok(json!({
        "success": true,
        "results": results
    }))
}

pub async fn set_model_link_exclusion(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let app_id = require_str_param(params, "app_id", "appId")?;
    let excluded = params
        .get("excluded")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let response = state.api.set_model_link_exclusion(&model_id, &app_id, excluded)?;
    Ok(serde_json::to_value(response)?)
}

pub async fn get_link_exclusions(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id = require_str_param(params, "app_id", "appId")?;
    let response = state.api.get_link_exclusions(&app_id)?;
    Ok(serde_json::to_value(response)?)
}
