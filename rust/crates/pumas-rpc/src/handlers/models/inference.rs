//! Inference settings handlers.

use crate::handlers::require_str_param;
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_inference_settings(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let settings = state.api.get_inference_settings(&model_id).await?;
    Ok(json!({
        "success": true,
        "model_id": model_id,
        "inference_settings": settings
    }))
}

pub async fn update_inference_settings(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;

    let settings: Vec<pumas_library::models::InferenceParamSchema> = params
        .get("settings")
        .or_else(|| params.get("inference_settings"))
        .or_else(|| params.get("inferenceSettings"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    state
        .api
        .update_inference_settings(&model_id, settings)
        .await?;
    Ok(json!({
        "success": true,
        "model_id": model_id
    }))
}

pub async fn update_model_notes(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let notes = params
        .get("notes")
        .or_else(|| params.get("model_notes"))
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);

    let response = state.api.update_model_notes(&model_id, notes).await?;
    Ok(serde_json::to_value(response)?)
}
