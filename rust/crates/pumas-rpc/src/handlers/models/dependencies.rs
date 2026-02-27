//! Model dependency and review handlers.

use crate::handlers::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_model_dependency_profiles(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let platform_context = get_str_param(params, "platform_context", "platformContext")
        .unwrap_or("unknown")
        .to_string();
    let backend_key = get_str_param(params, "backend_key", "backendKey");

    let profiles = state
        .api
        .get_model_dependency_profiles(&model_id, &platform_context, backend_key)
        .await?;
    Ok(json!({
        "success": true,
        "model_id": model_id,
        "platform_context": platform_context,
        "backend_key": backend_key,
        "profiles": profiles
    }))
}

pub async fn resolve_model_dependency_plan(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let platform_context = get_str_param(params, "platform_context", "platformContext")
        .unwrap_or("unknown")
        .to_string();
    let backend_key = get_str_param(params, "backend_key", "backendKey");

    let plan = state
        .api
        .resolve_model_dependency_plan(&model_id, &platform_context, backend_key)
        .await?;
    Ok(json!({
        "success": true,
        "plan": plan
    }))
}

pub async fn check_model_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let platform_context = get_str_param(params, "platform_context", "platformContext")
        .unwrap_or("unknown")
        .to_string();
    let backend_key = get_str_param(params, "backend_key", "backendKey");
    let selected_binding_ids: Option<Vec<String>> = params
        .get("selected_binding_ids")
        .or_else(|| params.get("selectedBindingIds"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let check = state
        .api
        .check_model_dependencies(
            &model_id,
            &platform_context,
            backend_key,
            selected_binding_ids,
        )
        .await?;
    Ok(json!({
        "success": true,
        "check": check
    }))
}

pub async fn install_model_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let platform_context = get_str_param(params, "platform_context", "platformContext")
        .unwrap_or("unknown")
        .to_string();
    let backend_key = get_str_param(params, "backend_key", "backendKey");
    let selected_binding_ids: Option<Vec<String>> = params
        .get("selected_binding_ids")
        .or_else(|| params.get("selectedBindingIds"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let install = state
        .api
        .install_model_dependencies(
            &model_id,
            &platform_context,
            backend_key,
            selected_binding_ids,
        )
        .await?;
    Ok(json!({
        "success": true,
        "install": install
    }))
}

pub async fn audit_dependency_pin_compliance(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let report = state.api.audit_dependency_pin_compliance().await?;
    Ok(json!({
        "success": true,
        "report": report
    }))
}

pub async fn list_models_needing_review(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let filter: Option<pumas_library::model_library::ModelReviewFilter> = params
        .get("filter")
        .or_else(|| params.get("review_filter"))
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?;

    let models = state.api.list_models_needing_review(filter).await?;
    Ok(json!({
        "success": true,
        "models": models
    }))
}

pub async fn submit_model_review(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let reviewer = require_str_param(params, "reviewer", "reviewer")?;
    let reason = get_str_param(params, "reason", "reason");
    let patch = params
        .get("patch")
        .or_else(|| params.get("metadata_patch"))
        .cloned()
        .unwrap_or_else(|| json!({}));

    let result = state
        .api
        .submit_model_review(&model_id, patch, &reviewer, reason)
        .await?;
    Ok(json!({
        "success": true,
        "result": result
    }))
}

pub async fn reset_model_review(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let reviewer = require_str_param(params, "reviewer", "reviewer")?;
    let reason = get_str_param(params, "reason", "reason");

    let reset = state
        .api
        .reset_model_review(&model_id, &reviewer, reason)
        .await?;
    Ok(json!({
        "success": true,
        "model_id": model_id,
        "reset": reset
    }))
}
