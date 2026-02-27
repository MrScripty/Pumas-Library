//! Model search handlers.

use crate::handlers::{get_i64_param, get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn search_hf_models(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let query = require_str_param(params, "query", "query")?;
    let kind = get_str_param(params, "kind", "kind");
    let limit = get_i64_param(params, "limit", "limit").unwrap_or(25) as usize;

    match state.api.search_hf_models(&query, kind, limit).await {
        Ok(models) => Ok(json!({
            "success": true,
            "models": models
        })),
        Err(e) => Ok(json!({
            "success": false,
            "models": [],
            "error": e.to_string()
        })),
    }
}

pub async fn get_related_models(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let limit = get_i64_param(params, "limit", "limit").unwrap_or(25) as usize;
    // Use the model's name to search for related models on HuggingFace
    let models = match state.api.get_model(&model_id).await {
        Ok(Some(model)) => state
            .api
            .search_hf_models(&model.official_name, None, limit)
            .await
            .unwrap_or_default(),
        _ => vec![],
    };
    Ok(json!({
        "success": true,
        "models": models
    }))
}

pub async fn search_models_fts(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let query = require_str_param(params, "query", "query")?;
    let limit = get_i64_param(params, "limit", "limit").unwrap_or(100) as usize;
    let offset = get_i64_param(params, "offset", "offset").unwrap_or(0) as usize;

    match state.api.search_models(&query, limit, offset).await {
        Ok(result) => Ok(json!({
            "success": true,
            "models": result.models,
            "total_count": result.total_count,
            "query_time_ms": result.query_time_ms,
            "query": result.query
        })),
        Err(e) => Ok(json!({
            "success": false,
            "models": [],
            "total_count": 0,
            "query_time_ms": 0,
            "query": query,
            "error": e.to_string()
        })),
    }
}
