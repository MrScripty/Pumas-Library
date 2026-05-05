//! Runtime profile RPC contract handlers.

use super::parse_params;
use crate::server::AppState;
use pumas_library::models::{ModelRuntimeRoute, RuntimeProfileConfig, RuntimeProfileId};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct RuntimeProfileUpdateParams {
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertRuntimeProfileParams {
    profile: RuntimeProfileConfig,
}

#[derive(Debug, Deserialize)]
struct DeleteRuntimeProfileParams {
    profile_id: RuntimeProfileId,
}

#[derive(Debug, Deserialize)]
struct SetModelRuntimeRouteParams {
    route: ModelRuntimeRoute,
}

#[derive(Debug, Deserialize)]
struct ClearModelRuntimeRouteParams {
    model_id: String,
}

pub async fn get_runtime_profiles_snapshot(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    Ok(serde_json::to_value(
        state.api.get_runtime_profiles_snapshot().await?,
    )?)
}

pub async fn list_runtime_profile_updates_since(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: RuntimeProfileUpdateParams =
        parse_params("list_runtime_profile_updates_since", params)?;
    Ok(serde_json::to_value(
        state
            .api
            .list_runtime_profile_updates_since(command.cursor.as_deref())
            .await?,
    )?)
}

pub async fn upsert_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: UpsertRuntimeProfileParams = parse_params("upsert_runtime_profile", params)?;
    Ok(serde_json::to_value(
        state.api.upsert_runtime_profile(command.profile).await?,
    )?)
}

pub async fn delete_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: DeleteRuntimeProfileParams = parse_params("delete_runtime_profile", params)?;
    Ok(serde_json::to_value(
        state.api.delete_runtime_profile(command.profile_id).await?,
    )?)
}

pub async fn set_model_runtime_route(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: SetModelRuntimeRouteParams = parse_params("set_model_runtime_route", params)?;
    Ok(serde_json::to_value(
        state.api.set_model_runtime_route(command.route).await?,
    )?)
}

pub async fn clear_model_runtime_route(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: ClearModelRuntimeRouteParams = parse_params("clear_model_runtime_route", params)?;
    Ok(serde_json::to_value(
        state
            .api
            .clear_model_runtime_route(command.model_id)
            .await?,
    )?)
}
