//! Inference settings handlers.

use crate::handlers::require_str_param;
use crate::server::AppState;
use serde_json::Value;

pub async fn get_inference_settings(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<crate::contract::InferenceSettingsOutcome> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let settings = state.api.get_inference_settings(&model_id).await?;
    crate::contract::InferenceSettingsOutcome::new(model_id, settings)
}

pub async fn update_inference_settings(
    state: &AppState,
    model_id: &str,
    settings: Vec<pumas_library::models::InferenceParamSchema>,
) -> pumas_library::Result<crate::contract::UpdateInferenceSettingsOutcome> {
    state
        .api
        .update_inference_settings(model_id, settings)
        .await?;
    Ok(crate::contract::UpdateInferenceSettingsOutcome::new(
        model_id,
    ))
}

pub async fn update_model_notes(
    state: &AppState,
    model_id: &str,
    notes: Option<String>,
) -> pumas_library::Result<crate::contract::UpdateModelNotesOutcome> {
    let response = state.api.update_model_notes(model_id, notes).await?;
    crate::contract::UpdateModelNotesOutcome::new(model_id, response)
}
