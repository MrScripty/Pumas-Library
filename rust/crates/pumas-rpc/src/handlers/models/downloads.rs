//! Model download handlers.

use crate::handlers::{parse_params, require_str_param, validate_non_empty};
use crate::server::AppState;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct DownloadModelFromHfParams {
    #[serde(alias = "repoId")]
    repo_id: String,
    family: String,
    #[serde(alias = "officialName")]
    official_name: String,
    #[serde(default, alias = "modelType")]
    model_type: Option<String>,
    #[serde(default)]
    quant: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    filenames: Option<Vec<String>>,
    #[serde(default, alias = "pipelineTag")]
    pipeline_tag: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default, alias = "releaseDate")]
    release_date: Option<String>,
    #[serde(default, alias = "downloadUrl")]
    download_url: Option<String>,
    #[serde(default, alias = "modelCardJson")]
    model_card_json: Option<String>,
    #[serde(default, alias = "licenseStatus")]
    license_status: Option<String>,
}

impl DownloadModelFromHfParams {
    fn into_download_request(self) -> pumas_library::Result<pumas_library::DownloadRequest> {
        Ok(pumas_library::DownloadRequest {
            repo_id: validate_non_empty(self.repo_id, "repo_id")?,
            family: validate_non_empty(self.family, "family")?,
            official_name: validate_non_empty(self.official_name, "official_name")?,
            model_type: self.model_type,
            quant: self.quant,
            filename: self.filename,
            filenames: self.filenames,
            pipeline_tag: self.pipeline_tag.or(self.subtype),
            bundle_format: None,
            pipeline_class: None,
            release_date: self.release_date,
            download_url: self.download_url,
            model_card_json: self.model_card_json,
            license_status: self.license_status,
        })
    }
}

fn parse_download_request(
    method: &str,
    params: &Value,
) -> pumas_library::Result<pumas_library::DownloadRequest> {
    let command: DownloadModelFromHfParams = parse_params(method, params)?;
    command.into_download_request()
}

pub async fn download_model_from_hf(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let request = parse_download_request("download_model_from_hf", params)?;

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
    let request = parse_download_request("start_model_download_from_hf", params)?;

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

pub async fn resume_partial_download(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let repo_id = require_str_param(params, "repo_id", "repoId")?;
    let dest_dir = require_str_param(params, "dest_dir", "destDir")?;

    let action = state
        .api
        .resume_partial_download(&repo_id, &dest_dir)
        .await?;
    let success = action.action != "none";
    Ok(json!({
        "success": success,
        "action": action.action,
        "download_id": action.download_id,
        "status": action.status,
        "reason_code": action.reason_code,
        "error": if success { Value::Null } else { serde_json::to_value(action.message)? }
    }))
}
