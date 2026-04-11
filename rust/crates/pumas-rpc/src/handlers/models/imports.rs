//! Model import and metadata handlers.

use crate::handlers::{
    extract_safetensors_header, get_bool_param, get_str_param, require_str_param,
};
use crate::server::AppState;
use pumas_library::model_library::get_diffusers_component_manifest;
use serde_json::{json, Value};

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

pub async fn import_external_diffusers_directory(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let source_path = require_str_param(params, "source_path", "sourcePath")?;
    let family = require_str_param(params, "family", "family")?;
    let official_name = require_str_param(params, "official_name", "officialName")?;
    let repo_id = get_str_param(params, "repo_id", "repoId").map(String::from);

    let tags: Option<Vec<String>> =
        params
            .get("tags")
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(String::from))
                    .collect()
            });

    let spec = pumas_library::model_library::ExternalDiffusersImportSpec {
        source_path,
        family,
        official_name,
        repo_id,
        tags,
    };

    let result = state.api.import_external_diffusers_directory(&spec).await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn classify_model_import_paths(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let paths: Vec<String> = params
        .get("paths")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();

    let result = state.api.classify_model_import_paths(&paths);
    Ok(serde_json::to_value(result)?)
}

pub async fn lookup_hf_metadata_for_file(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let file_path = require_str_param(params, "file_path", "filePath")?;

    match state.api.lookup_hf_metadata_for_file(&file_path).await {
        Ok(Some(metadata)) => Ok(json!({
            "success": true,
            "found": true,
            "metadata": metadata
        })),
        Ok(None) => Ok(json!({
            "success": true,
            "found": false,
            "metadata": null
        })),
        Err(e) => Ok(json!({
            "success": false,
            "found": false,
            "error": e.to_string()
        })),
    }
}

pub async fn lookup_hf_metadata_for_bundle_directory(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let directory_path = require_str_param(params, "directory_path", "directoryPath")?;

    match state
        .api
        .lookup_hf_metadata_for_bundle_directory(&directory_path)
        .await
    {
        Ok(Some(metadata)) => Ok(json!({
            "success": true,
            "found": true,
            "metadata": metadata
        })),
        Ok(None) => Ok(json!({
            "success": true,
            "found": false,
            "metadata": null
        })),
        Err(e) => Ok(json!({
            "success": false,
            "found": false,
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

pub async fn validate_file_type(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let file_path = require_str_param(params, "file_path", "filePath")?;
    let response = state.api.validate_file_type(&file_path);
    Ok(serde_json::to_value(response)?)
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
        "gguf" => match pumas_library::model_library::extract_gguf_metadata(&file_path) {
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
        },
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
    let effective_metadata = state.api.get_effective_model_metadata(&model_id).await?;

    // Find primary model file and get embedded metadata
    let primary_file = library.get_primary_model_file(&model_id);
    let embedded_metadata: Option<pumas_library::EmbeddedMetadataResponse> =
        if let Some(ref file_path) = primary_file {
            let extension = file_path
                .extension()
                .and_then(|e: &std::ffi::OsStr| e.to_str())
                .map(|s: &str| s.to_lowercase())
                .unwrap_or_default();

            match extension.as_str() {
                "gguf" => match pumas_library::model_library::extract_gguf_metadata(file_path) {
                    Ok(metadata) => {
                        let metadata_value: serde_json::Map<String, Value> = metadata
                            .into_iter()
                            .map(|(k, v)| (k, Value::String(v)))
                            .collect();
                        Some(pumas_library::EmbeddedMetadataResponse {
                            file_type: "gguf".to_string(),
                            metadata: Value::Object(metadata_value),
                        })
                    }
                    Err(_) => None,
                },
                "safetensors" => match extract_safetensors_header(&file_path.to_string_lossy()) {
                    Ok(header) => Some(pumas_library::EmbeddedMetadataResponse {
                        file_type: "safetensors".to_string(),
                        metadata: header,
                    }),
                    Err(_) => None,
                },
                _ => None,
            }
        } else {
            None
        };

    let primary_file_str =
        primary_file.map(|p: std::path::PathBuf| p.to_string_lossy().to_string());

    let component_manifest = effective_metadata
        .as_ref()
        .filter(|metadata| {
            metadata.bundle_format == Some(pumas_library::BundleFormat::DiffusersDirectory)
        })
        .and_then(|metadata| {
            metadata
                .entry_path
                .as_deref()
                .or(primary_file_str.as_deref())
                .map(std::path::Path::new)
        })
        .and_then(get_diffusers_component_manifest);

    let response = pumas_library::LibraryModelMetadataResponse {
        success: true,
        model_id,
        stored_metadata: stored_metadata.map(serde_json::to_value).transpose()?,
        effective_metadata: effective_metadata.map(serde_json::to_value).transpose()?,
        embedded_metadata,
        primary_file: primary_file_str,
        component_manifest,
    };
    Ok(serde_json::to_value(response)?)
}

pub async fn resolve_model_execution_descriptor(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let descriptor = state
        .api
        .resolve_model_execution_descriptor(&model_id)
        .await?;
    Ok(serde_json::to_value(descriptor)?)
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
    let compute_hashes = get_bool_param(params, "compute_hashes", "computeHashes").unwrap_or(false);

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
    let release_date = get_str_param(params, "release_date", "releaseDate").map(String::from);
    let download_url = get_str_param(params, "download_url", "downloadUrl").map(String::from);
    let model_card_json =
        get_str_param(params, "model_card_json", "modelCardJson").map(String::from);
    let license_status = get_str_param(params, "license_status", "licenseStatus").map(String::from);
    let huggingface_evidence: Option<pumas_library::model_library::HuggingFaceEvidence> = params
        .get("huggingface_evidence")
        .or_else(|| params.get("huggingFaceEvidence"))
        .and_then(|value| serde_json::from_value(value.clone()).ok());

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
        huggingface_evidence,
        release_date,
        download_url,
        model_card_json,
        license_status,
    };

    let result = state.api.import_model_in_place(&spec).await?;
    Ok(serde_json::to_value(result)?)
}
