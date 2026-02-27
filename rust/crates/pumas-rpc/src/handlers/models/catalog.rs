//! Model catalog and mapping handlers.

use crate::handlers::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_models(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let models = state.api.list_models().await?;
    // Convert to a format with model_id as keys for frontend compatibility
    let mut result = serde_json::Map::new();
    for model in models {
        result.insert(model.id.clone(), serde_json::to_value(&model)?);
    }
    Ok(json!(result))
}

pub async fn refresh_model_index(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let count = state.api.rebuild_model_index().await?;
    Ok(json!({
        "success": true,
        "indexed_count": count
    }))
}

pub async fn refresh_model_mappings(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    if app_id != "comfyui" {
        return Ok(json!({
            "success": false,
            "error": format!("Model mapping refresh currently supports only comfyui, got: {}", app_id),
        }));
    }

    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        let active = vm.get_active_version().await?;
        if let Some(version_tag) = active {
            let version_path = vm.version_path(&version_tag);
            let models_path = version_path.join("models");
            drop(managers);

            let response = state
                .api
                .apply_model_mapping(&version_tag, &models_path)
                .await?;

            Ok(json!({
                "success": response.success,
                "error": response.error,
                "app_id": "comfyui",
                "version_tag": version_tag,
                "links_created": response.links_created,
                "links_removed": response.links_removed,
                "total_links": response.total_links,
            }))
        } else {
            Ok(json!({
                "success": false,
                "error": "No active version set for comfyui",
            }))
        }
    } else {
        Ok(json!({
            "success": false,
            "error": "Version manager not initialized for comfyui",
        }))
    }
}

pub async fn scan_shared_storage(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // Rebuild the model index from metadata files on disk
    let count = state.api.rebuild_model_index().await?;
    Ok(json!({
        "modelsFound": count,
        "scanned": count,
        "indexed": count
    }))
}

pub async fn refetch_model_metadata_from_hf(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;

    let updated = state.api.refetch_metadata_from_hf(&model_id).await?;
    Ok(json!({
        "success": true,
        "model_id": model_id,
        "metadata": serde_json::to_value(&updated)?
    }))
}
