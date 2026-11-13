//! Model catalog and mapping handlers.

use crate::contract::{ModelIndexRefreshOutcome, ModelLookupOutcome, ModelsOutcome};
use crate::handlers::require_str_param;
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_models(state: &AppState) -> pumas_library::Result<ModelsOutcome> {
    let library_root = state
        .api
        .launcher_root()
        .join("shared-resources")
        .join("models");
    state
        .catalog_projection
        .models(state.api.list_models().await?, library_root)
        .await
}

pub async fn refresh_model_index(
    state: &AppState,
) -> pumas_library::Result<ModelIndexRefreshOutcome> {
    let count = state.api.rebuild_model_index().await?;
    count.try_into()
}

pub async fn scan_shared_storage(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // Rebuild the model index from metadata files on disk
    let count = state.api.rebuild_model_index().await?;
    Ok(json!({
        "modelsFound": count,
        "scanned": count,
        "indexed": count
    }))
}

pub async fn refetch_model_metadata_from_hf(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;

    let updated = state.api.refetch_metadata_from_hf(&model_id).await?;
    Ok(json!({
        "success": true,
        "model_id": model_id,
        "metadata": serde_json::to_value(&updated)?
    }))
}

/// The model card, license documents and code stored with a library model.
pub async fn get_model_documents(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    Ok(serde_json::to_value(
        state.api.get_model_documents(&model_id).await?,
    )?)
}

/// `dry_run` defaults to true: fetching and writing must be asked for.
fn companion_backfill_dry_run(params: &Value) -> bool {
    crate::handlers::get_bool_param(params, "dry_run", "dryRun").unwrap_or(true)
}

/// Fetch the companion files one library model is missing.
pub async fn backfill_model_companion_files(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    Ok(serde_json::to_value(
        state
            .api
            .backfill_model_companion_files(&model_id, companion_backfill_dry_run(params))
            .await?,
    )?)
}

/// Fetch the companion files every library model is missing.
pub async fn backfill_library_companion_files(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    Ok(serde_json::to_value(
        state
            .api
            .backfill_library_companion_files(companion_backfill_dry_run(params))
            .await?,
    )?)
}

/// The caller refreshes selection explicitly after observing a replacement ID.
pub async fn lookup_model(
    state: &AppState,
    model_id: &str,
) -> pumas_library::Result<ModelLookupOutcome> {
    state.api.lookup_model(model_id).await.map(Into::into)
}
