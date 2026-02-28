//! Model dependency and review handlers.

use crate::handlers::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn resolve_model_dependency_requirements(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let platform_context = get_str_param(params, "platform_context", "platformContext")
        .unwrap_or("unknown")
        .to_string();
    let backend_key = get_str_param(params, "backend_key", "backendKey");
    let expected_contract_version = params
        .get("expected_dependency_contract_version")
        .or_else(|| params.get("expectedDependencyContractVersion"))
        .and_then(Value::as_u64);
    if let Some(expected) = expected_contract_version {
        let actual = pumas_library::model_library::DEPENDENCY_CONTRACT_VERSION as u64;
        if expected != actual {
            return Err(pumas_library::PumasError::InvalidParams {
                message: format!(
                    "dependency contract version mismatch: expected {}, actual {}",
                    expected, actual
                ),
            });
        }
    }

    let requirements = state
        .api
        .resolve_model_dependency_requirements(&model_id, &platform_context, backend_key)
        .await?;
    Ok(json!({
        "success": true,
        "requirements": requirements
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
