//! Runtime profile RPC contract handlers.

use super::parse_params;
use crate::server::AppState;
use pumas_library::models::{
    ModelRuntimeRoute, RuntimeProfileConfig, RuntimeProfileId, RuntimeProviderId,
};
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

#[derive(Debug, Deserialize)]
struct LaunchRuntimeProfileParams {
    profile_id: RuntimeProfileId,
    tag: Option<String>,
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StopRuntimeProfileParams {
    profile_id: RuntimeProfileId,
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

pub async fn launch_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: LaunchRuntimeProfileParams = parse_params("launch_runtime_profile", params)?;
    let snapshot = state.api.get_runtime_profiles_snapshot().await?;
    let Some(profile) = snapshot
        .snapshot
        .profiles
        .iter()
        .find(|profile| profile.profile_id == command.profile_id)
    else {
        return Ok(serde_json::json!({
            "success": false,
            "error": format!("Runtime profile not found: {}", command.profile_id.as_str()),
            "ready": false
        }));
    };

    let (tag, version_dir) = match profile.provider {
        RuntimeProviderId::Ollama => {
            let Some(version_manager) = super::get_version_manager(state, "ollama").await else {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": "Version manager not initialized for ollama",
                    "ready": false
                }));
            };
            let tag = match command.tag {
                Some(tag) => tag,
                None => match version_manager.get_active_version().await? {
                    Some(tag) => tag,
                    None => {
                        return Ok(serde_json::json!({
                            "success": false,
                            "error": "No active Ollama version set",
                            "ready": false
                        }));
                    }
                },
            };
            let version_dir = version_manager.version_path(&tag);
            (tag, version_dir)
        }
        RuntimeProviderId::LlamaCpp => {
            let Some(version_manager) = super::get_version_manager(state, "llama-cpp").await else {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": "Version manager not initialized for llama.cpp",
                    "ready": false
                }));
            };
            let tag = match command.tag {
                Some(tag) => tag,
                None => match version_manager.get_active_version().await? {
                    Some(tag) => tag,
                    None => {
                        return Ok(serde_json::json!({
                            "success": false,
                            "error": "No active llama.cpp version set. Open the llama.cpp app page, install a runtime version, and set it active.",
                            "ready": false
                        }));
                    }
                },
            };
            let version_dir = version_manager.version_path(&tag);
            (tag, version_dir)
        }
    };
    Ok(serde_json::to_value(
        state
            .api
            .launch_runtime_profile_for_model(
                command.profile_id,
                &tag,
                &version_dir,
                command.model_id.as_deref(),
            )
            .await?,
    )?)
}

pub async fn stop_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: StopRuntimeProfileParams = parse_params("stop_runtime_profile", params)?;
    let stopped = state.api.stop_runtime_profile(command.profile_id).await?;
    Ok(serde_json::json!({ "success": stopped }))
}
