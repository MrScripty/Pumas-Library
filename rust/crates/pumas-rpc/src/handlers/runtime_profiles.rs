//! Runtime profile RPC contract handlers.

use crate::server::AppState;
use pumas_library::models::{
    RuntimeProfileMutationResponse, RuntimeProfileUpdateFeed, RuntimeProfileUpdateFeedResponse,
    RuntimeProfilesSnapshotResponse,
};
use serde_json::{json, Value};

pub async fn get_runtime_profiles_snapshot(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    Ok(serde_json::to_value(
        RuntimeProfilesSnapshotResponse::empty_success(),
    )?)
}

pub async fn list_runtime_profile_updates_since(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let cursor = params.get("cursor").and_then(Value::as_str);
    Ok(serde_json::to_value(RuntimeProfileUpdateFeedResponse {
        success: true,
        error: None,
        feed: RuntimeProfileUpdateFeed::empty(cursor),
    })?)
}

pub async fn upsert_runtime_profile(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    Ok(json!(RuntimeProfileMutationResponse {
        success: false,
        error: Some("Runtime profile persistence is not implemented yet".to_string()),
        profile_id: None,
        snapshot_required: true,
    }))
}

pub async fn delete_runtime_profile(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    Ok(json!(RuntimeProfileMutationResponse {
        success: false,
        error: Some("Runtime profile persistence is not implemented yet".to_string()),
        profile_id: None,
        snapshot_required: true,
    }))
}

pub async fn set_model_runtime_route(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    Ok(json!(RuntimeProfileMutationResponse {
        success: false,
        error: Some("Runtime profile route persistence is not implemented yet".to_string()),
        profile_id: None,
        snapshot_required: true,
    }))
}

pub async fn clear_model_runtime_route(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    Ok(json!(RuntimeProfileMutationResponse {
        success: false,
        error: Some("Runtime profile route persistence is not implemented yet".to_string()),
        profile_id: None,
        snapshot_required: true,
    }))
}
