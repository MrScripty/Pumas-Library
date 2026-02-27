//! Hugging Face authentication handlers.

use crate::handlers::require_str_param;
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn set_hf_token(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let token = require_str_param(params, "token", "token")?;
    state.api.set_hf_token(&token).await?;
    Ok(json!({ "success": true }))
}

pub async fn clear_hf_token(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    state.api.clear_hf_token().await?;
    Ok(json!({ "success": true }))
}

pub async fn get_hf_auth_status(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let status = state.api.get_hf_auth_status().await?;
    Ok(json!({
        "success": true,
        "authenticated": status.authenticated,
        "username": status.username,
        "token_source": status.token_source
    }))
}
