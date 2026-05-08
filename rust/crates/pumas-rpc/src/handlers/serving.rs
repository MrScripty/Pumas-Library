//! User-directed model serving RPC handlers.

use super::parse_params;
use crate::server::AppState;
use pumas_library::models::ServeModelRequest;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct ValidateModelServingConfigParams {
    request: ServeModelRequest,
}

pub async fn get_serving_status(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    Ok(serde_json::to_value(state.api.get_serving_status().await?)?)
}

pub async fn validate_model_serving_config(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: ValidateModelServingConfigParams =
        parse_params("validate_model_serving_config", params)?;
    Ok(serde_json::to_value(
        state
            .api
            .validate_model_serving_config(command.request)
            .await?,
    )?)
}
