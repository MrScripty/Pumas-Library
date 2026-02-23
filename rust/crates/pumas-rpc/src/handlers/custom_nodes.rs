//! Custom nodes management handlers.

use super::require_str_param;
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_custom_nodes(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let version_tag = require_str_param(params, "version_tag", "versionTag")?;
    let nodes = state
        .custom_nodes_manager
        .list_custom_nodes(&version_tag)?;
    Ok(serde_json::to_value(nodes)?)
}

pub async fn install_custom_node(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let repo_url = require_str_param(params, "repo_url", "repoUrl")?;
    let version_tag = require_str_param(params, "version_tag", "versionTag")?;
    let result = state
        .custom_nodes_manager
        .install_from_git(&repo_url, &version_tag)
        .await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn update_custom_node(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let node_name = require_str_param(params, "node_name", "nodeName")?;
    let version_tag = require_str_param(params, "version_tag", "versionTag")?;
    let result = state
        .custom_nodes_manager
        .update(&node_name, &version_tag)
        .await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn remove_custom_node(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let node_name = require_str_param(params, "node_name", "nodeName")?;
    let version_tag = require_str_param(params, "version_tag", "versionTag")?;
    let result = state
        .custom_nodes_manager
        .remove(&node_name, &version_tag)?;
    Ok(json!({"success": result}))
}
