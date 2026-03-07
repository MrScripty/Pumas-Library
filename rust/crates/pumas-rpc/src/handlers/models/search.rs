//! Model search handlers.

use crate::handlers::{get_i64_param, get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn search_hf_models(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let query = require_str_param(params, "query", "query")?;
    let kind = get_str_param(params, "kind", "kind");
    let limit = get_i64_param(params, "limit", "limit").unwrap_or(25) as usize;
    let hydrate_limit = get_i64_param(params, "hydrate_limit", "hydrateLimit")
        .map(|value| value.max(0) as usize)
        .unwrap_or(limit);

    match state
        .api
        .search_hf_models_with_hydration(&query, kind, limit, hydrate_limit)
        .await
    {
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
    let hydrate_limit = limit.min(6);
    // Use the model's name to search for related models on HuggingFace
    let models = match state.api.get_model(&model_id).await {
        Ok(Some(model)) => state
            .api
            .search_hf_models_with_hydration(&model.official_name, None, limit, hydrate_limit)
            .await
            .unwrap_or_default(),
        _ => vec![],
    };
    Ok(json!({
        "success": true,
        "models": models
    }))
}

pub async fn get_hf_download_details(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let repo_id = require_str_param(params, "repo_id", "repoId")?;
    let quants: Vec<String> = params
        .get("quants")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();

    match state.api.get_hf_download_details(&repo_id, &quants).await {
        Ok(details) => Ok(json!({
            "success": true,
            "details": details
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
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
