//! Model download handlers.

use crate::handlers::{
    parse_params, require_str_param, validate_existing_local_directory_path, validate_non_empty,
};
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
        Ok(download_id) => Ok(download_start_response(state, download_id).await),
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
        Ok(download_id) => Ok(download_start_response(state, download_id).await),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

async fn download_start_response(state: &AppState, download_id: String) -> Value {
    let selected_artifact_id = state
        .api
        .get_hf_download_progress(&download_id)
        .await
        .and_then(|progress| progress.selected_artifact_id);

    format_download_start_response(download_id, selected_artifact_id)
}

fn format_download_start_response(
    download_id: String,
    selected_artifact_id: Option<String>,
) -> Value {
    json!({
        "success": true,
        "download_id": download_id,
        "selectedArtifactId": selected_artifact_id.clone(),
        "artifactId": selected_artifact_id,
    })
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
    let interrupted = state.api.list_interrupted_downloads().await;
    Ok(json!({
        "success": true,
        "interrupted": interrupted
    }))
}

pub async fn recover_download(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let repo_id = require_str_param(params, "repo_id", "repoId")?;
    let dest_dir = validate_existing_local_directory_path(
        require_str_param(params, "dest_dir", "destDir")?,
        "dest_dir",
    )
    .await?;

    match state
        .api
        .recover_download(&repo_id, &dest_dir.to_string_lossy())
        .await
    {
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
    let dest_dir = validate_existing_local_directory_path(
        require_str_param(params, "dest_dir", "destDir")?,
        "dest_dir",
    )
    .await?;

    let action = state
        .api
        .resume_partial_download(&repo_id, &dest_dir.to_string_lossy())
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_recover_download_validates_dest_dir() {
        let temp = TempDir::new().unwrap();
        let dest_dir = temp.path().join("downloads");
        tokio::fs::create_dir_all(&dest_dir).await.unwrap();

        let validated = validate_existing_local_directory_path(
            dest_dir.to_string_lossy().to_string(),
            "dest_dir",
        )
        .await
        .unwrap();

        assert_eq!(validated, tokio::fs::canonicalize(dest_dir).await.unwrap());
    }

    #[tokio::test]
    async fn test_resume_partial_download_rejects_file_dest_dir() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("not-a-directory");
        tokio::fs::write(&file_path, b"x").await.unwrap();

        let error = validate_existing_local_directory_path(
            file_path.to_string_lossy().to_string(),
            "dest_dir",
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            pumas_library::PumasError::InvalidParams { .. }
        ));
    }

    #[test]
    fn test_download_start_response_includes_selected_artifact_aliases() {
        let response = format_download_start_response(
            "dl-1".to_string(),
            Some("owner--model__q4_k_m".to_string()),
        );

        assert_eq!(response["success"], true);
        assert_eq!(response["download_id"], "dl-1");
        assert_eq!(response["selectedArtifactId"], "owner--model__q4_k_m");
        assert_eq!(response["artifactId"], "owner--model__q4_k_m");
    }
}
