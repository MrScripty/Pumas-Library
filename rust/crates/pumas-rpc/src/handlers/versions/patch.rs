//! Version patch toggle handlers.

use crate::handlers::get_str_param;
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn is_patched(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    let is_patched = state.api.is_patched(tag);
    Ok(json!(is_patched))
}

pub async fn toggle_patch(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    match state.api.toggle_patch(tag) {
        Ok(is_now_patched) => Ok(json!(is_now_patched)),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}
