//! Model library handlers.

use super::{
    extract_safetensors_header, get_bool_param, get_i64_param, get_str_param, require_str_param,
};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_models(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let models = state.api.list_models().await?;
    // Convert to a format with model_id as keys for frontend compatibility
    let mut result = serde_json::Map::new();
    for model in models {
        result.insert(model.id.clone(), serde_json::to_value(&model)?);
    }
    Ok(json!(result))
}

pub async fn refresh_model_index(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let count = state.api.rebuild_model_index().await?;
    Ok(json!({
        "success": true,
        "indexed_count": count
    }))
}

pub async fn refresh_model_mappings(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let _app_id = get_str_param(params, "app_id", "appId");
    // TODO: Implement model mapping refresh
    Ok(json!({}))
}

pub async fn import_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let local_path = require_str_param(params, "local_path", "localPath")?;
    let family = require_str_param(params, "family", "family")?;
    let official_name = require_str_param(params, "official_name", "officialName")?;
    let repo_id = get_str_param(params, "repo_id", "repoId").map(String::from);
    let model_type = get_str_param(params, "model_type", "modelType").map(String::from);
    let subtype = get_str_param(params, "subtype", "subtype").map(String::from);
    let security_acknowledged =
        get_bool_param(params, "security_acknowledged", "securityAcknowledged");

    let spec = pumas_library::model_library::ModelImportSpec {
        path: local_path,
        family,
        official_name,
        repo_id,
        model_type,
        subtype,
        tags: None,
        security_acknowledged,
    };

    let result = state.api.import_model(&spec).await?;
    Ok(serde_json::to_value(result)?)
}

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
    let pipeline_tag = get_str_param(params, "pipeline_tag", "pipelineTag").map(String::from);

    let request = pumas_library::DownloadRequest {
        repo_id,
        family,
        official_name,
        model_type,
        quant,
        filename,
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
    let pipeline_tag = get_str_param(params, "pipeline_tag", "pipelineTag").map(String::from);

    let request = pumas_library::DownloadRequest {
        repo_id,
        family,
        official_name,
        model_type,
        quant,
        filename,
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

pub async fn recover_download(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
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

pub async fn get_related_models(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
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

pub async fn import_batch(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    // Parse the imports array from params
    let imports: Vec<pumas_library::model_library::ModelImportSpec> = params
        .get("imports")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let results = state.api.import_models_batch(imports).await;
    let imported = results.iter().filter(|r| r.success).count();
    let failed = results.iter().filter(|r| !r.success).count();

    Ok(json!({
        "success": true,
        "imported": imported,
        "failed": failed,
        "results": results
    }))
}

pub async fn lookup_hf_metadata_for_file(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let file_path = require_str_param(params, "file_path", "filePath")?;

    match state.api.lookup_hf_metadata_for_file(&file_path).await {
        Ok(Some(metadata)) => Ok(json!({
            "success": true,
            "metadata": metadata
        })),
        Ok(None) => Ok(json!({
            "success": false,
            "error": "No metadata found"
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn detect_sharded_sets(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    // Get files array from params
    let files: Vec<String> = params
        .get("files")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Convert to PathBuf
    let paths: Vec<std::path::PathBuf> = files.iter().map(std::path::PathBuf::from).collect();

    // Detect sharded sets
    let sets = pumas_library::sharding::detect_sharded_sets(&paths);

    // Convert to serializable format
    let result: std::collections::HashMap<String, Vec<String>> = sets
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                v.into_iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
            )
        })
        .collect();

    Ok(json!({
        "success": true,
        "sets": result
    }))
}

pub async fn validate_file_type(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let _file_path = require_str_param(params, "file_path", "filePath")?;
    // TODO: Implement file type validation
    Ok(json!({
        "success": true,
        "valid": true,
        "detected_type": null
    }))
}

pub async fn mark_metadata_as_manual(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    match state.api.mark_model_metadata_as_manual(&model_id).await {
        Ok(()) => Ok(json!({
            "success": true
        })),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn get_embedded_metadata(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let file_path = require_str_param(params, "file_path", "filePath")?;
    let path = std::path::Path::new(&file_path);

    // Detect file type from extension
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "gguf" => {
            match pumas_library::model_library::extract_gguf_metadata(&file_path) {
                Ok(metadata) => {
                    // Convert HashMap<String, String> to Value
                    let metadata_value: serde_json::Map<String, Value> = metadata
                        .into_iter()
                        .map(|(k, v)| (k, Value::String(v)))
                        .collect();
                    Ok(json!({
                        "success": true,
                        "file_type": "gguf",
                        "metadata": metadata_value
                    }))
                }
                Err(e) => Ok(json!({
                    "success": false,
                    "file_type": "gguf",
                    "error": e.to_string(),
                    "metadata": null
                })),
            }
        }
        "safetensors" => {
            // Read safetensors JSON header
            match extract_safetensors_header(&file_path) {
                Ok(header) => Ok(json!({
                    "success": true,
                    "file_type": "safetensors",
                    "metadata": header
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "file_type": "safetensors",
                    "error": e,
                    "metadata": null
                })),
            }
        }
        _ => Ok(json!({
            "success": false,
            "file_type": "unsupported",
            "error": format!("Unsupported file type: {}", extension),
            "metadata": null
        })),
    }
}

pub async fn get_library_model_metadata(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;

    // Get the library
    let library = state.api.model_library();

    // Get stored metadata from metadata.json
    let model_dir = library.library_root().join(&model_id);
    let stored_metadata = library.load_metadata(&model_dir)?;

    // Find primary model file and get embedded metadata
    let primary_file = library.get_primary_model_file(&model_id);
    let embedded_metadata: Option<Value> = if let Some(ref file_path) = primary_file {
        let extension = file_path
            .extension()
            .and_then(|e: &std::ffi::OsStr| e.to_str())
            .map(|s: &str| s.to_lowercase())
            .unwrap_or_default();

        match extension.as_str() {
            "gguf" => {
                match pumas_library::model_library::extract_gguf_metadata(file_path) {
                    Ok(metadata) => {
                        let metadata_value: serde_json::Map<String, Value> = metadata
                            .into_iter()
                            .map(|(k, v)| (k, Value::String(v)))
                            .collect();
                        Some(json!({
                            "file_type": "gguf",
                            "metadata": metadata_value
                        }))
                    }
                    Err(_) => None,
                }
            }
            "safetensors" => {
                match extract_safetensors_header(&file_path.to_string_lossy()) {
                    Ok(header) => Some(json!({
                        "file_type": "safetensors",
                        "metadata": header
                    })),
                    Err(_) => None,
                }
            }
            _ => None,
        }
    } else {
        None
    };

    let primary_file_str =
        primary_file.map(|p: std::path::PathBuf| p.to_string_lossy().to_string());

    Ok(json!({
        "success": true,
        "model_id": model_id,
        "stored_metadata": stored_metadata,
        "embedded_metadata": embedded_metadata,
        "primary_file": primary_file_str
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

pub async fn get_model_overrides(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let _rel_path = require_str_param(params, "rel_path", "relPath")?;
    // TODO: Implement model overrides
    Ok(json!({}))
}

pub async fn update_model_overrides(
    _state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let _rel_path = require_str_param(params, "rel_path", "relPath")?;
    // TODO: Implement model overrides update
    Ok(json!(false))
}

pub async fn adopt_orphan_models(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let result = state.api.adopt_orphan_models().await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn import_model_in_place(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_dir = require_str_param(params, "model_dir", "modelDir")?;
    let official_name = require_str_param(params, "official_name", "officialName")?;
    let family = require_str_param(params, "family", "family")?;
    let model_type = get_str_param(params, "model_type", "modelType").map(String::from);
    let repo_id = get_str_param(params, "repo_id", "repoId").map(String::from);
    let known_sha256 = get_str_param(params, "known_sha256", "knownSha256").map(String::from);
    let compute_hashes =
        get_bool_param(params, "compute_hashes", "computeHashes").unwrap_or(false);

    let expected_files: Option<Vec<String>> = params
        .get("expected_files")
        .or_else(|| params.get("expectedFiles"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let pipeline_tag: Option<String> = params
        .get("pipeline_tag")
        .or_else(|| params.get("pipelineTag"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let spec = pumas_library::model_library::InPlaceImportSpec {
        model_dir: std::path::PathBuf::from(model_dir),
        official_name,
        family,
        model_type,
        repo_id,
        known_sha256,
        compute_hashes,
        expected_files,
        pipeline_tag,
    };

    let result = state.api.import_model_in_place(&spec).await?;
    Ok(serde_json::to_value(result)?)
}

// ========================================
// HuggingFace Authentication
// ========================================

pub async fn set_hf_token(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let token = require_str_param(params, "token", "token")?;
    state.api.set_hf_token(&token).await?;
    Ok(json!({ "success": true }))
}

pub async fn clear_hf_token(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    state.api.clear_hf_token().await?;
    Ok(json!({ "success": true }))
}

pub async fn get_hf_auth_status(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let status = state.api.get_hf_auth_status().await?;
    Ok(json!({
        "success": true,
        "authenticated": status.authenticated,
        "username": status.username,
        "token_source": status.token_source
    }))
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

// ========================================
// Inference Settings
// ========================================

pub async fn get_inference_settings(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let settings = state.api.get_inference_settings(&model_id).await?;
    Ok(json!({
        "success": true,
        "model_id": model_id,
        "inference_settings": settings
    }))
}

pub async fn update_inference_settings(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;

    let settings: Vec<pumas_library::models::InferenceParamSchema> = params
        .get("inference_settings")
        .or_else(|| params.get("inferenceSettings"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    state
        .api
        .update_inference_settings(&model_id, settings)
        .await?;
    Ok(json!({
        "success": true,
        "model_id": model_id
    }))
}
