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
            "error": crate::contract::PublicError::from(&e).message
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
    repo_id: &str,
    quants: &[String],
) -> pumas_library::Result<crate::contract::HfDownloadDetailsOutcome> {
    match state.api.get_hf_download_details(repo_id, quants).await {
        Ok(details) => crate::contract::HfDownloadDetailsOutcome::found(repo_id, details),
        Err(e) => Ok(crate::contract::HfDownloadDetailsOutcome::failed(&e)),
    }
}

pub async fn search_models_fts(
    state: &AppState,
    query: &str,
    limit: usize,
    offset: usize,
) -> pumas_library::Result<crate::contract::CatalogSearchOutcome> {
    let search = state.api.search_models(query, limit, offset).await?;
    let root = state.api.launcher_root().join("shared-resources/models");
    state.catalog_projection.search(search, root).await
}
