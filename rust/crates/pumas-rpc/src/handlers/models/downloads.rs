//! Model download handlers.

use crate::handlers::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn download_model_from_hf(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let repo_id = require_str_param(params, "repo_id", "repoId")?;
    let family = require_str_param(params, "family", "family")?;
    let official_name = require_str_param(params, "official_name", "officialName")?;
    let model_type = get_str_param(params, "model_type", "modelType").map(String::from);
    let quant = get_str_param(params, "quant", "quant").map(String::from);
    let filename = get_str_param(params, "filename", "filename").map(String::from);
    let filenames: Option<Vec<String>> = params
        .get("filenames")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let pipeline_tag = get_str_param(params, "pipeline_tag", "pipelineTag").map(String::from);

    let request = pumas_library::DownloadRequest {
        repo_id,
        family,
        official_name,
        model_type,
        quant,
        filename,
        filenames,
        pipeline_tag,
    };

    match state.api.start_hf_download(&request).await {
        Ok(download_id) => Ok(json!({
            "success": true,
            "download_id": download_id
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn start_model_download_from_hf(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let repo_id = require_str_param(params, "repo_id", "repoId")?;
    let family = require_str_param(params, "family", "family")?;
    let official_name = require_str_param(params, "official_name", "officialName")?;
    let model_type = get_str_param(params, "model_type", "modelType").map(String::from);
    let quant = get_str_param(params, "quant", "quant").map(String::from);
    let filename = get_str_param(params, "filename", "filename").map(String::from);
    let filenames: Option<Vec<String>> = params
        .get("filenames")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let pipeline_tag = get_str_param(params, "pipeline_tag", "pipelineTag").map(String::from);

    let request = pumas_library::DownloadRequest {
        repo_id,
        family,
        official_name,
        model_type,
        quant,
        filename,
        filenames,
        pipeline_tag,
    };

    match state.api.start_hf_download(&request).await {
        Ok(download_id) => Ok(json!({
            "success": true,
            "download_id": download_id
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn get_model_download_status(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let download_id = require_str_param(params, "download_id", "downloadId")?;
    match state.api.get_hf_download_progress(&download_id).await {
        Some(progress) => {
            let mut response = serde_json::to_value(progress)?;
            if let Some(obj) = response.as_object_mut() {
                obj.insert("success".to_string(), json!(true));
            }
            Ok(response)
        }
        None => Ok(json!({
            "success": false,
            "error": "Download not found"
        })),
    }
}

pub async fn cancel_model_download(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let download_id = require_str_param(params, "download_id", "downloadId")?;
    match state.api.cancel_hf_download(&download_id).await {
        Ok(cancelled) => Ok(json!({
            "success": cancelled
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn pause_model_download(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let download_id = require_str_param(params, "download_id", "downloadId")?;
    match state.api.pause_hf_download(&download_id).await {
        Ok(paused) => Ok(json!({
            "success": paused
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn resume_model_download(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let download_id = require_str_param(params, "download_id", "downloadId")?;
    match state.api.resume_hf_download(&download_id).await {
        Ok(resumed) => Ok(json!({
            "success": resumed
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn list_model_downloads(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let downloads = state.api.list_hf_downloads().await;
    Ok(json!({
        "success": true,
        "downloads": downloads
    }))
}

pub async fn list_interrupted_downloads(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let interrupted = state.api.list_interrupted_downloads();
    Ok(json!({
        "success": true,
        "interrupted": interrupted
    }))
}

pub async fn recover_download(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let repo_id = require_str_param(params, "repo_id", "repoId")?;
    let dest_dir = require_str_param(params, "dest_dir", "destDir")?;

    match state.api.recover_download(&repo_id, &dest_dir).await {
        Ok(download_id) => Ok(json!({
            "success": true,
            "download_id": download_id
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}
