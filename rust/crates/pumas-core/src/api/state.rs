//! Primary instance state and IPC dispatch.

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{reconcile_on_demand, ReconcileScope, ReconciliationCoordinator};
use crate::conversion;
use crate::error::PumasError;
use crate::ipc;
use crate::model_library;
use crate::models;
use crate::network;
use crate::process;
use crate::registry;
use crate::system;
use std::path::{Path, PathBuf};

/// All state owned by a primary instance.
///
/// This is the full set of subsystems that were previously fields on `PumasApi`.
/// Wrapped in `Arc` so it can be shared with the IPC server dispatch.
pub(crate) struct PrimaryState {
    pub(crate) _state: Arc<RwLock<ApiState>>,
    pub(crate) network_manager: Arc<network::NetworkManager>,
    pub(crate) process_manager: Arc<RwLock<Option<process::ProcessManager>>>,
    pub(crate) system_utils: Arc<system::SystemUtils>,
    pub(crate) model_library: Arc<model_library::ModelLibrary>,
    pub(crate) model_mapper: model_library::ModelMapper,
    pub(crate) hf_client: Option<model_library::HuggingFaceClient>,
    pub(crate) model_importer: model_library::ModelImporter,
    pub(crate) conversion_manager: Arc<conversion::ConversionManager>,
    /// Internal scheduler for event-driven reconciliation.
    pub(crate) reconciliation: Arc<ReconciliationCoordinator>,
    /// IPC server handle (Primary only). Protected by async Mutex for shutdown.
    pub(crate) server_handle: tokio::sync::Mutex<Option<ipc::IpcServerHandle>>,
    /// Global registry connection (best-effort, None if unavailable).
    pub(crate) registry: Option<registry::LibraryRegistry>,
}

/// IPC dispatch implementation for the primary state.
///
/// Routes incoming JSON-RPC method calls to the appropriate PrimaryState methods.
/// Each method deserializes params, calls the real implementation, and serializes the result.
#[async_trait::async_trait]
impl ipc::server::IpcDispatch for PrimaryState {
    async fn dispatch(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, PumasError> {
        match method {
            "list_models" => {
                let _ =
                    reconcile_on_demand(self, ReconcileScope::AllModels, "ipc-list-models").await?;
                let models = self.model_library.list_models().await?;
                Ok(serde_json::to_value(models)?)
            }
            "search_models" => {
                let query = params["query"].as_str().unwrap_or("");
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;
                let offset = params["offset"].as_u64().unwrap_or(0) as usize;

                let result = if query.trim().is_empty() {
                    let _ = reconcile_on_demand(
                        self,
                        ReconcileScope::AllModels,
                        "ipc-search-empty-query",
                    )
                    .await?;
                    self.model_library
                        .search_models(query, limit, offset)
                        .await?
                } else {
                    let mut result = self
                        .model_library
                        .search_models(query, limit, offset)
                        .await?;
                    let mut model_ids = HashSet::new();
                    for model in &result.models {
                        model_ids.insert(model.id.clone());
                    }

                    let mut reconciled_any = false;
                    for model_id in model_ids {
                        if reconcile_on_demand(
                            self,
                            ReconcileScope::Model(model_id),
                            "ipc-search-model-hit",
                        )
                        .await?
                        {
                            reconciled_any = true;
                        }
                    }

                    if reconciled_any {
                        result = self
                            .model_library
                            .search_models(query, limit, offset)
                            .await?;
                    }

                    result
                };
                Ok(serde_json::to_value(result)?)
            }
            "get_model" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let _ = reconcile_on_demand(
                    self,
                    ReconcileScope::Model(model_id.to_string()),
                    "ipc-get-model",
                )
                .await?;
                let model = self.model_library.get_model(model_id).await?;
                Ok(serde_json::to_value(model)?)
            }
            "delete_model_with_cascade" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                self.model_library.delete_model(model_id, true).await?;
                Ok(serde_json::to_value(models::DeleteModelResponse {
                    success: true,
                    error: None,
                })?)
            }
            "import_model" => {
                let spec: model_library::ModelImportSpec =
                    serde_json::from_value(params["spec"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid import spec: {e}"),
                        }
                    })?;
                let result = self.model_importer.import(&spec).await?;
                Ok(serde_json::to_value(result)?)
            }
            "import_models_batch" => {
                let specs: Vec<model_library::ModelImportSpec> =
                    serde_json::from_value(params["specs"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid import specs: {e}"),
                        }
                    })?;
                let result = self.model_importer.batch_import(specs, None).await;
                Ok(serde_json::to_value(result)?)
            }
            "rebuild_model_index" => {
                self.reconciliation.mark_dirty_all().await;
                let _ = reconcile_on_demand(self, ReconcileScope::AllModels, "ipc-rebuild-index")
                    .await?;
                let model_count = self.model_library.list_models().await?.len();
                Ok(serde_json::to_value(model_count)?)
            }
            "reclassify_model" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let result = self.model_library.reclassify_model(model_id).await?;
                Ok(serde_json::to_value(result)?)
            }
            "reclassify_all_models" => {
                let result = self.model_library.reclassify_all_models().await?;
                Ok(serde_json::to_value(result)?)
            }
            "get_inference_settings" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let settings = load_inference_settings(self, model_id).await?;
                Ok(serde_json::to_value(settings)?)
            }
            "update_inference_settings" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let settings: Vec<models::InferenceParamSchema> =
                    serde_json::from_value(params["settings"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid inference settings: {e}"),
                        }
                    })?;
                store_inference_settings(self, model_id, settings).await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "search_hf_models" => {
                let query = params["query"].as_str().unwrap_or("");
                let kind = params["kind"].as_str();
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;
                let models = search_hf_models(self, query, kind, limit).await?;
                Ok(serde_json::to_value(models)?)
            }
            "start_hf_download" => {
                let request: model_library::DownloadRequest =
                    serde_json::from_value(params["request"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid download request: {e}"),
                        }
                    })?;
                let download_id = start_hf_download(self, &request).await?;
                Ok(serde_json::to_value(download_id)?)
            }
            "get_hf_download_progress" => {
                let download_id =
                    params["download_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "download_id is required".to_string(),
                        })?;
                let progress = get_hf_download_progress(self, download_id).await;
                Ok(serde_json::to_value(progress)?)
            }
            "cancel_hf_download" => {
                let download_id =
                    params["download_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "download_id is required".to_string(),
                        })?;
                let cancelled = cancel_hf_download(self, download_id).await?;
                Ok(serde_json::to_value(cancelled)?)
            }
            "list_interrupted_downloads" => {
                let downloads = list_interrupted_downloads(self);
                Ok(serde_json::to_value(downloads)?)
            }
            "recover_download" => {
                let repo_id =
                    params["repo_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "repo_id is required".to_string(),
                        })?;
                let dest_dir =
                    params["dest_dir"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "dest_dir is required".to_string(),
                        })?;
                let download_id = recover_download(self, repo_id, dest_dir).await?;
                Ok(serde_json::to_value(download_id)?)
            }
            "lookup_hf_metadata_for_file" => {
                let file_path =
                    params["file_path"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "file_path is required".to_string(),
                        })?;
                let result = lookup_hf_metadata_for_file(self, file_path).await?;
                Ok(serde_json::to_value(result)?)
            }
            "get_hf_repo_files" => {
                let repo_id =
                    params["repo_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "repo_id is required".to_string(),
                        })?;
                let tree = get_hf_repo_files(self, repo_id).await?;
                Ok(serde_json::to_value(tree)?)
            }
            "is_online" => Ok(serde_json::to_value(self.network_manager.is_online())?),
            "get_disk_space" => {
                let response = disk_space_response(self)?;
                Ok(serde_json::to_value(response)?)
            }
            "get_status_response" => {
                let response = status_response(self).await?;
                Ok(serde_json::to_value(response)?)
            }
            "get_system_resources" => {
                let response = system_resources_response(self).await?;
                Ok(serde_json::to_value(response)?)
            }
            "is_torch_running" => Ok(serde_json::to_value(is_torch_running(self).await)?),
            "stop_torch" => {
                let stopped = stop_torch(self).await?;
                Ok(serde_json::to_value(stopped)?)
            }
            "get_status" => Ok(serde_json::json!({
                "success": true,
                "version": env!("CARGO_PKG_VERSION"),
                "is_primary": true,
            })),
            "ping" => Ok(serde_json::json!("pong")),
            // Conversion methods
            "start_conversion" => {
                let request: conversion::ConversionRequest = serde_json::from_value(params)
                    .map_err(|e| PumasError::InvalidParams {
                        message: format!("Invalid conversion request: {e}"),
                    })?;
                let id = self.conversion_manager.start_conversion(request).await?;
                Ok(serde_json::json!({ "conversion_id": id }))
            }
            "get_conversion_progress" => {
                let id =
                    params["conversion_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "conversion_id is required".to_string(),
                        })?;
                let progress = self.conversion_manager.get_progress(id);
                Ok(serde_json::to_value(progress)?)
            }
            "cancel_conversion" => {
                let id =
                    params["conversion_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "conversion_id is required".to_string(),
                        })?;
                let cancelled = self.conversion_manager.cancel_conversion(id).await?;
                Ok(serde_json::json!({ "cancelled": cancelled }))
            }
            "list_conversions" => {
                let conversions = self.conversion_manager.list_conversions();
                Ok(serde_json::to_value(conversions)?)
            }
            "is_conversion_environment_ready" => {
                let ready = self.conversion_manager.is_environment_ready();
                Ok(serde_json::json!({ "ready": ready }))
            }
            "ensure_conversion_environment" => {
                self.conversion_manager.ensure_environment().await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "supported_quant_types" => {
                let types = self.conversion_manager.supported_quant_types();
                Ok(serde_json::to_value(types)?)
            }
            // Quantization backend methods
            "backend_status" => {
                let status = self.conversion_manager.backend_status();
                Ok(serde_json::to_value(status)?)
            }
            "ensure_backend_environment" => {
                let backend_str =
                    params["backend"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "backend is required".to_string(),
                        })?;
                let backend: conversion::QuantBackend =
                    serde_json::from_value(serde_json::json!(backend_str)).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid backend: {e}"),
                        }
                    })?;
                self.conversion_manager
                    .ensure_backend_environment(backend)
                    .await?;
                Ok(serde_json::json!({ "success": true }))
            }
            _ => Err(PumasError::InvalidParams {
                message: format!("Unknown IPC method: {}", method),
            }),
        }
    }
}

/// Internal state for the API.
pub(crate) struct ApiState {
    /// Whether background fetch has completed
    pub(crate) background_fetch_completed: bool,
}

fn launcher_root_from_primary(primary: &PrimaryState) -> PathBuf {
    primary
        .model_library
        .library_root()
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| primary.model_library.library_root())
        .to_path_buf()
}

async fn load_inference_settings(
    primary: &PrimaryState,
    model_id: &str,
) -> std::result::Result<Vec<models::InferenceParamSchema>, PumasError> {
    let library = &primary.model_library;
    let model_dir = library.library_root().join(model_id);

    if !model_dir.exists() {
        return Err(PumasError::Other(format!("Model not found: {}", model_id)));
    }

    let metadata = library.load_metadata(&model_dir)?.unwrap_or_default();
    if let Some(settings) = metadata.inference_settings {
        return Ok(settings);
    }

    let file_format = library
        .get_primary_model_file(model_id)
        .and_then(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
        })
        .unwrap_or_default();

    Ok(models::resolve_inference_settings(&metadata, &file_format).unwrap_or_default())
}

async fn store_inference_settings(
    primary: &PrimaryState,
    model_id: &str,
    settings: Vec<models::InferenceParamSchema>,
) -> std::result::Result<(), PumasError> {
    let library = &primary.model_library;
    let model_dir = library.library_root().join(model_id);

    if !model_dir.exists() {
        return Err(PumasError::Other(format!("Model not found: {}", model_id)));
    }

    let mut metadata = library.load_metadata(&model_dir)?.unwrap_or_default();
    metadata.inference_settings = if settings.is_empty() {
        None
    } else {
        Some(settings)
    };
    metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

    library.save_metadata(&model_dir, &metadata).await?;
    library.index_model_dir(&model_dir).await?;
    Ok(())
}

async fn search_hf_models(
    primary: &PrimaryState,
    query: &str,
    kind: Option<&str>,
    limit: usize,
) -> std::result::Result<Vec<models::HuggingFaceModel>, PumasError> {
    if let Some(ref client) = primary.hf_client {
        let params = model_library::HfSearchParams {
            query: query.to_string(),
            kind: kind.map(String::from),
            limit: Some(limit),
            hydrate_limit: Some(limit),
            ..Default::default()
        };
        client.search(&params).await
    } else {
        Ok(vec![])
    }
}

async fn start_hf_download(
    primary: &PrimaryState,
    request: &model_library::DownloadRequest,
) -> std::result::Result<String, PumasError> {
    use crate::api::hf::{normalized_download_hint, resolve_model_type_from_hints};
    use tracing::{info, warn};

    let client = primary
        .hf_client
        .as_ref()
        .ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;

    let mut resolved_request = request.clone();
    let mut resolved_pipeline_tag =
        normalized_download_hint(resolved_request.pipeline_tag.as_deref()).map(ToOwned::to_owned);

    let mut resolved_model_type = resolve_model_type_from_hints(
        primary.model_library.index(),
        [
            normalized_download_hint(request.model_type.as_deref()),
            resolved_pipeline_tag.as_deref(),
        ],
    )?;

    if resolved_model_type.is_none() || resolved_pipeline_tag.is_none() {
        let model_info = client.get_model_info(&request.repo_id).await?;
        if resolved_pipeline_tag.is_none() {
            resolved_pipeline_tag =
                normalized_download_hint(Some(model_info.kind.as_str())).map(ToOwned::to_owned);
        }
        if resolved_model_type.is_none() {
            resolved_model_type = resolve_model_type_from_hints(
                primary.model_library.index(),
                [
                    normalized_download_hint(request.model_type.as_deref()),
                    resolved_pipeline_tag.as_deref(),
                    normalized_download_hint(Some(model_info.kind.as_str())),
                ],
            )?;
        }
    }

    let should_check_bundle = resolved_model_type
        .as_deref()
        .is_none_or(|model_type| model_type == "diffusion")
        || resolved_pipeline_tag.as_deref() == Some("text-to-image");
    if should_check_bundle {
        match client.classify_repo_bundle(&request.repo_id).await {
            Ok(Some(bundle)) => {
                if resolved_request.filename.is_some()
                    || resolved_request.filenames.is_some()
                    || resolved_request.quant.is_some()
                {
                    info!(
                        "HF repo {} classified as {:?}; forcing full bundle download",
                        request.repo_id, bundle.bundle_format
                    );
                }
                resolved_request.filename = None;
                resolved_request.filenames = None;
                resolved_request.quant = None;
                resolved_request.bundle_format = Some(bundle.bundle_format);
                resolved_request.pipeline_class = Some(bundle.pipeline_class);
                if resolved_pipeline_tag.is_none() {
                    resolved_pipeline_tag = Some("text-to-image".to_string());
                }
                if resolved_model_type.is_none() {
                    resolved_model_type = Some("diffusion".to_string());
                }
            }
            Ok(None) => {}
            Err(err) => {
                warn!(
                    "Failed to classify HF repo {} as a bundle: {}",
                    request.repo_id, err
                );
            }
        }
    }

    resolved_request.pipeline_tag = resolved_pipeline_tag;
    let model_type = resolved_model_type.unwrap_or_else(|| "unknown".to_string());
    resolved_request.model_type = Some(model_type.clone());
    let dest_dir = primary.model_library.build_model_path(
        &model_type,
        &resolved_request.family,
        &model_library::normalize_name(&resolved_request.official_name),
    );
    if model_type == "unknown" {
        warn!(
            "Download {} is starting with unknown model_type after HF metadata lookup; destination={}",
            request.repo_id,
            dest_dir.display()
        );
    }
    client.start_download(&resolved_request, &dest_dir).await
}

async fn get_hf_download_progress(
    primary: &PrimaryState,
    download_id: &str,
) -> Option<models::ModelDownloadProgress> {
    if let Some(ref client) = primary.hf_client {
        client.get_download_progress(download_id).await
    } else {
        None
    }
}

async fn cancel_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.cancel_download(download_id).await
    } else {
        Ok(false)
    }
}

fn list_interrupted_downloads(primary: &PrimaryState) -> Vec<model_library::InterruptedDownload> {
    let known_dirs: HashSet<std::path::PathBuf> = if let Some(ref client) = primary.hf_client {
        if let Some(persistence) = client.persistence() {
            persistence
                .load_all()
                .into_iter()
                .map(|entry| entry.dest_dir)
                .collect()
        } else {
            HashSet::new()
        }
    } else {
        HashSet::new()
    };

    primary
        .model_importer
        .find_interrupted_downloads(&known_dirs)
}

async fn recover_download(
    primary: &PrimaryState,
    repo_id: &str,
    dest_dir: &str,
) -> std::result::Result<String, PumasError> {
    let dest = Path::new(dest_dir);
    if !dest.is_dir() {
        return Err(PumasError::NotFound {
            resource: format!("directory: {}", dest_dir),
        });
    }

    let client = primary
        .hf_client
        .as_ref()
        .ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;

    let parts: Vec<&str> = repo_id.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(PumasError::Config {
            message: format!(
                "Invalid repo_id format (expected 'owner/name'): {}",
                repo_id
            ),
        });
    }

    let model_type = dest
        .strip_prefix(primary.model_library.library_root())
        .ok()
        .and_then(|rel| rel.components().next())
        .and_then(|c| c.as_os_str().to_str())
        .map(String::from);

    let request = model_library::DownloadRequest {
        repo_id: repo_id.to_string(),
        family: parts[0].to_string(),
        official_name: parts[1].to_string(),
        model_type,
        quant: None,
        filename: None,
        filenames: None,
        pipeline_tag: None,
        bundle_format: None,
        pipeline_class: None,
    };

    client.start_download(&request, dest).await
}

async fn lookup_hf_metadata_for_file(
    primary: &PrimaryState,
    file_path: &str,
) -> std::result::Result<Option<model_library::HfMetadataResult>, PumasError> {
    if let Some(ref client) = primary.hf_client {
        let path = Path::new(file_path);
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);
        client.lookup_metadata(filename, Some(path), None).await
    } else {
        Ok(None)
    }
}

async fn get_hf_repo_files(
    primary: &PrimaryState,
    repo_id: &str,
) -> std::result::Result<model_library::RepoFileTree, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.get_repo_files(repo_id).await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
    }
}

fn disk_space_response(
    primary: &PrimaryState,
) -> std::result::Result<models::DiskSpaceResponse, PumasError> {
    use sysinfo::Disks;

    let launcher_root = launcher_root_from_primary(primary);
    let launcher_root_str = launcher_root.to_string_lossy();
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.list() {
        let mount_point = disk.mount_point().to_string_lossy();
        if launcher_root_str.starts_with(mount_point.as_ref()) {
            let total = disk.total_space();
            let free = disk.available_space();
            let used = total.saturating_sub(free);
            let percent = if total > 0 {
                (used as f32 / total as f32) * 100.0
            } else {
                0.0
            };

            return Ok(models::DiskSpaceResponse {
                success: true,
                error: None,
                total,
                used,
                free,
                percent,
            });
        }
    }

    if let Some(disk) = disks.list().first() {
        let total = disk.total_space();
        let free = disk.available_space();
        let used = total.saturating_sub(free);
        let percent = if total > 0 {
            (used as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        return Ok(models::DiskSpaceResponse {
            success: true,
            error: None,
            total,
            used,
            free,
            percent,
        });
    }

    Err(PumasError::Other(
        "Could not determine disk space".to_string(),
    ))
}

async fn status_response(
    primary: &PrimaryState,
) -> std::result::Result<models::StatusResponse, PumasError> {
    let mgr_lock = primary.process_manager.read().await;
    let comfyui_running = mgr_lock.as_ref().is_some_and(|mgr| mgr.is_running());
    let ollama_running = mgr_lock.as_ref().is_some_and(|mgr| mgr.is_ollama_running());
    let torch_running = mgr_lock.as_ref().is_some_and(|mgr| mgr.is_torch_running());
    let last_launch_error = mgr_lock.as_ref().and_then(|mgr| mgr.last_launch_error());
    let last_launch_log = mgr_lock.as_ref().and_then(|mgr| {
        mgr.last_launch_log()
            .map(|p| p.to_string_lossy().to_string())
    });

    let app_resources = if let Some(ref mgr) = *mgr_lock {
        let comfyui_resources = if comfyui_running {
            mgr.aggregate_app_resources()
                .map(|r| models::AppResourceUsage {
                    gpu_memory: Some((r.gpu_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                    ram_memory: Some((r.ram_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                })
        } else {
            None
        };

        let ollama_resources = if ollama_running {
            mgr.aggregate_ollama_resources()
                .map(|r| models::AppResourceUsage {
                    gpu_memory: Some((r.gpu_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                    ram_memory: Some((r.ram_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                })
        } else {
            None
        };

        if comfyui_resources.is_some() || ollama_resources.is_some() {
            Some(models::AppResources {
                comfyui: comfyui_resources,
                ollama: ollama_resources,
            })
        } else {
            None
        }
    } else {
        None
    };
    drop(mgr_lock);

    Ok(models::StatusResponse {
        success: true,
        error: None,
        version: env!("CARGO_PKG_VERSION").to_string(),
        deps_ready: true,
        patched: false,
        menu_shortcut: false,
        desktop_shortcut: false,
        shortcut_version: None,
        message: if comfyui_running {
            "ComfyUI running".to_string()
        } else if ollama_running {
            "Ollama running".to_string()
        } else if torch_running {
            "Torch running".to_string()
        } else {
            "Ready".to_string()
        },
        comfyui_running,
        ollama_running,
        torch_running,
        last_launch_error,
        last_launch_log,
        app_resources,
    })
}

async fn system_resources_response(
    primary: &PrimaryState,
) -> std::result::Result<models::SystemResourcesResponse, PumasError> {
    use sysinfo::{Disks, System};

    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_usage();
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let ram_usage = if total_memory > 0 {
        (used_memory as f32 / total_memory as f32) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let (disk_total, disk_free) = if let Some(disk) = disks.list().first() {
        (disk.total_space(), disk.available_space())
    } else {
        (0, 0)
    };
    let disk_usage = if disk_total > 0 {
        ((disk_total - disk_free) as f32 / disk_total as f32) * 100.0
    } else {
        0.0
    };

    let gpu = if let Some(ref mgr) = *primary.process_manager.read().await {
        let tracker = mgr.resource_tracker();
        match tracker.get_system_resources() {
            Ok(snapshot) => models::GpuResources {
                usage: snapshot.gpu_usage,
                memory: snapshot.gpu_memory_used,
                memory_total: snapshot.gpu_memory_total,
                temp: snapshot.gpu_temp,
            },
            Err(_) => models::GpuResources {
                usage: 0.0,
                memory: 0,
                memory_total: 0,
                temp: None,
            },
        }
    } else {
        models::GpuResources {
            usage: 0.0,
            memory: 0,
            memory_total: 0,
            temp: None,
        }
    };

    Ok(models::SystemResourcesResponse {
        success: true,
        error: None,
        resources: models::SystemResources {
            cpu: models::CpuResources {
                usage: cpu_usage,
                temp: None,
            },
            gpu,
            ram: models::RamResources {
                usage: ram_usage,
                total: total_memory,
            },
            disk: models::DiskResources {
                usage: disk_usage,
                total: disk_total,
                free: disk_free,
            },
        },
    })
}

async fn is_torch_running(primary: &PrimaryState) -> bool {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.is_torch_running()
    } else {
        false
    }
}

async fn stop_torch(primary: &PrimaryState) -> std::result::Result<bool, PumasError> {
    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(mgr) = process_manager {
        tokio::task::spawn_blocking(move || mgr.stop_torch())
            .await
            .map_err(|e| PumasError::Other(format!("Failed to join stop_torch task: {}", e)))?
    } else {
        Ok(false)
    }
}
