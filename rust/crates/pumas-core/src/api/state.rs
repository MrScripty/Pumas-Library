//! Primary instance state and IPC dispatch.

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::state_hf::{
    cancel_hf_download, clear_hf_token, get_hf_auth_status, get_hf_download_details,
    get_hf_download_progress, get_hf_repo_files, list_hf_downloads, list_interrupted_downloads,
    lookup_hf_metadata_for_bundle_directory, lookup_hf_metadata_for_file, pause_hf_download,
    recover_download, refetch_metadata_from_hf, resume_hf_download, resume_partial_download,
    search_hf_models, search_hf_models_with_hydration, set_hf_token, start_hf_download,
};
use super::state_process::{
    get_last_launch_error, get_last_launch_log, get_running_processes, is_comfyui_running,
    is_ollama_running, is_torch_running, launch_ollama, launch_torch, launch_version,
    set_process_version_paths, stop_comfyui, stop_ollama, stop_torch,
};
use super::state_runtime::{
    disk_space_response, network_status_response, status_response, system_resources_response,
};
use super::{ReconcileScope, ReconciliationCoordinator, reconcile_on_demand};
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
    /// Primary-local suppressor for Pumas-owned watcher feedback paths.
    pub(crate) watcher_write_suppressor: Arc<super::WatcherWriteSuppressor>,
    /// IPC server handle (Primary only). Protected by async Mutex for shutdown.
    pub(crate) server_handle: tokio::sync::Mutex<Option<ipc::IpcServerHandle>>,
    /// Global registry connection used for singleton claim ownership.
    pub(crate) registry: Option<registry::LibraryRegistry>,
    /// Pending startup claim that will be promoted to a ready instance row once IPC starts.
    pub(crate) instance_claim: tokio::sync::Mutex<Option<registry::PrimaryInstanceClaim>>,
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
                let model_count = self.model_library.model_count()?;
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
                let settings_value = if !params["settings"].is_null() {
                    params["settings"].clone()
                } else {
                    params["inference_settings"].clone()
                };
                let settings: Vec<models::InferenceParamSchema> =
                    serde_json::from_value(settings_value).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid inference settings: {e}"),
                        }
                    })?;
                store_inference_settings(self, model_id, settings).await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "update_model_notes" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let notes = params["notes"].as_str().map(ToOwned::to_owned);
                let response = store_model_notes(self, model_id, notes).await?;
                Ok(serde_json::to_value(response)?)
            }
            "get_library_status" => {
                let _ =
                    reconcile_on_demand(self, ReconcileScope::AllModels, "ipc-get-library-status")
                        .await?;
                let model_count = self.model_library.model_count()? as u32;
                let pending_lookups = self.model_library.get_pending_lookups().await?.len() as u32;
                Ok(serde_json::to_value(models::LibraryStatusResponse {
                    success: true,
                    error: None,
                    indexing: false,
                    deep_scan_in_progress: false,
                    model_count,
                    pending_lookups: Some(pending_lookups),
                    deep_scan_progress: None,
                })?)
            }
            "resolve_model_dependency_requirements" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let platform_context = params["platform_context"].as_str().ok_or_else(|| {
                    PumasError::InvalidParams {
                        message: "platform_context is required".to_string(),
                    }
                })?;
                let backend_key = params["backend_key"].as_str();
                let resolution = self
                    .model_library
                    .resolve_model_dependency_requirements(model_id, platform_context, backend_key)
                    .await?;
                Ok(serde_json::to_value(resolution)?)
            }
            "resolve_model_execution_descriptor" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let descriptor = self
                    .model_library
                    .resolve_model_execution_descriptor(model_id)
                    .await?;
                Ok(serde_json::to_value(descriptor)?)
            }
            "audit_dependency_pin_compliance" => {
                let report = self.model_library.audit_dependency_pin_compliance().await?;
                Ok(serde_json::to_value(report)?)
            }
            "list_models_needing_review" => {
                let filter: Option<model_library::ModelReviewFilter> =
                    serde_json::from_value(params["filter"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid review filter: {e}"),
                        }
                    })?;
                let items = self
                    .model_library
                    .list_models_needing_review(filter)
                    .await?;
                Ok(serde_json::to_value(items)?)
            }
            "submit_model_review" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let patch = params["patch"].clone();
                let reviewer =
                    params["reviewer"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "reviewer is required".to_string(),
                        })?;
                let reason = params["reason"].as_str();
                let result = self
                    .model_library
                    .submit_model_review(model_id, patch, reviewer, reason)
                    .await?;
                Ok(serde_json::to_value(result)?)
            }
            "reset_model_review" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let reviewer =
                    params["reviewer"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "reviewer is required".to_string(),
                        })?;
                let reason = params["reason"].as_str();
                let reset = self
                    .model_library
                    .reset_model_review(model_id, reviewer, reason)
                    .await?;
                Ok(serde_json::to_value(reset)?)
            }
            "get_effective_model_metadata" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let _ = reconcile_on_demand(
                    self,
                    ReconcileScope::Model(model_id.to_string()),
                    "ipc-get-effective-metadata",
                )
                .await?;
                let metadata = self.model_library.get_effective_metadata(model_id)?;
                Ok(serde_json::to_value(metadata)?)
            }
            "import_external_diffusers_directory" => {
                let spec: model_library::ExternalDiffusersImportSpec =
                    serde_json::from_value(params["spec"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid external diffusers import spec: {e}"),
                        }
                    })?;
                let result = self
                    .model_importer
                    .import_external_diffusers_directory(&spec)
                    .await?;
                Ok(serde_json::to_value(result)?)
            }
            "import_model_in_place" => {
                let spec: model_library::InPlaceImportSpec =
                    serde_json::from_value(params["spec"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid in-place import spec: {e}"),
                        }
                    })?;
                let result = self.model_importer.import_in_place(&spec).await?;
                Ok(serde_json::to_value(result)?)
            }
            "adopt_orphan_models" => {
                let result = self.model_importer.adopt_orphans(false).await;
                Ok(serde_json::to_value(result)?)
            }
            "get_link_health" => {
                let registry = self.model_library.link_registry().read().await;
                let all_links = registry.get_all().await;

                let mut healthy = 0;
                let mut broken: Vec<String> = Vec::new();

                for link in &all_links {
                    if link.target.is_symlink() {
                        if link.source.exists() {
                            healthy += 1;
                        } else {
                            broken.push(link.target.to_string_lossy().to_string());
                        }
                    } else if link.target.exists() {
                        healthy += 1;
                    } else {
                        broken.push(link.target.to_string_lossy().to_string());
                    }
                }

                Ok(serde_json::to_value(models::LinkHealthResponse {
                    success: true,
                    error: None,
                    status: if broken.is_empty() {
                        "healthy".to_string()
                    } else {
                        "degraded".to_string()
                    },
                    total_links: all_links.len(),
                    healthy_links: healthy,
                    broken_links: broken,
                    orphaned_links: vec![],
                    warnings: vec![],
                    errors: vec![],
                })?)
            }
            "clean_broken_links" => {
                let registry = self.model_library.link_registry().write().await;
                let broken = registry.cleanup_broken().await?;
                for entry in &broken {
                    if entry.target.exists() || entry.target.is_symlink() {
                        let _ = std::fs::remove_file(&entry.target);
                    }
                }
                Ok(serde_json::to_value(models::CleanBrokenLinksResponse {
                    success: true,
                    cleaned: broken.len(),
                })?)
            }
            "get_links_for_model" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let registry = self.model_library.link_registry().read().await;
                let links = registry.get_links_for_model(model_id).await;

                let link_info: Vec<models::LinkInfo> = links
                    .into_iter()
                    .map(|l| models::LinkInfo {
                        source: l.source.to_string_lossy().to_string(),
                        target: l.target.to_string_lossy().to_string(),
                        link_type: format!("{:?}", l.link_type).to_lowercase(),
                        app_id: l.app_id,
                        app_version: l.app_version,
                        created_at: l.created_at,
                    })
                    .collect();

                Ok(serde_json::to_value(models::LinksForModelResponse {
                    success: true,
                    links: link_info,
                })?)
            }
            "preview_model_mapping" => {
                let version_tag =
                    params["version_tag"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "version_tag is required".to_string(),
                        })?;
                let models_path =
                    params["models_path"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "models_path is required".to_string(),
                        })?;
                let models_path = PathBuf::from(models_path);
                let response =
                    preview_model_mapping_response(self, version_tag, models_path.as_path())
                        .await?;
                Ok(serde_json::to_value(response)?)
            }
            "apply_model_mapping" => {
                let version_tag =
                    params["version_tag"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "version_tag is required".to_string(),
                        })?;
                let models_path =
                    params["models_path"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "models_path is required".to_string(),
                        })?;
                let models_path = PathBuf::from(models_path);
                let response =
                    apply_model_mapping_response(self, version_tag, models_path.as_path()).await?;
                Ok(serde_json::to_value(response)?)
            }
            "sync_models_incremental" => {
                let version_tag =
                    params["version_tag"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "version_tag is required".to_string(),
                        })?;
                let models_path =
                    params["models_path"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "models_path is required".to_string(),
                        })?;
                let models_path = PathBuf::from(models_path);
                let apply =
                    apply_model_mapping_response(self, version_tag, models_path.as_path()).await?;
                let response = models::SyncModelsResponse {
                    success: apply.success,
                    error: apply.error,
                    synced: apply.links_created,
                    errors: vec![],
                };
                Ok(serde_json::to_value(response)?)
            }
            "sync_with_resolutions" => {
                let version_tag =
                    params["version_tag"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "version_tag is required".to_string(),
                        })?;
                let models_path =
                    params["models_path"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "models_path is required".to_string(),
                        })?;
                let resolutions: std::collections::HashMap<
                    String,
                    model_library::ConflictResolution,
                > = serde_json::from_value(params["resolutions"].clone()).map_err(|e| {
                    PumasError::InvalidParams {
                        message: format!("Invalid mapping resolutions: {e}"),
                    }
                })?;
                let response = sync_with_resolutions_response(
                    self,
                    version_tag,
                    Path::new(models_path),
                    resolutions,
                )
                .await?;
                Ok(serde_json::to_value(response)?)
            }
            "generate_model_migration_dry_run_report" => {
                let report = self
                    .model_library
                    .generate_migration_dry_run_report_with_artifacts()?;
                Ok(serde_json::to_value(report)?)
            }
            "execute_model_migration" => {
                let mut report = self
                    .model_library
                    .execute_migration_with_checkpoint()
                    .await?;
                let mutated =
                    super::migration::relocate_skipped_partial_downloads(self, &mut report).await?;
                if mutated {
                    super::migration::recompute_execution_report_counts(&mut report);
                    self.model_library
                        .rewrite_migration_execution_report(&report)?;
                }
                Ok(serde_json::to_value(report)?)
            }
            "list_model_migration_reports" => {
                let reports = self.model_library.list_migration_reports()?;
                Ok(serde_json::to_value(reports)?)
            }
            "delete_model_migration_report" => {
                let report_path =
                    params["report_path"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "report_path is required".to_string(),
                        })?;
                let deleted = self.model_library.delete_migration_report(report_path)?;
                Ok(serde_json::to_value(deleted)?)
            }
            "prune_model_migration_reports" => {
                let keep_latest =
                    params["keep_latest"]
                        .as_u64()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "keep_latest is required".to_string(),
                        })? as usize;
                let pruned = self.model_library.prune_migration_reports(keep_latest)?;
                Ok(serde_json::to_value(pruned)?)
            }
            "search_hf_models" => {
                let query = params["query"].as_str().unwrap_or("");
                let kind = params["kind"].as_str();
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;
                let models = search_hf_models(self, query, kind, limit).await?;
                Ok(serde_json::to_value(models)?)
            }
            "search_hf_models_with_hydration" => {
                let query = params["query"].as_str().unwrap_or("");
                let kind = params["kind"].as_str();
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;
                let hydrate_limit =
                    params["hydrate_limit"].as_u64().unwrap_or(limit as u64) as usize;
                let models =
                    search_hf_models_with_hydration(self, query, kind, limit, hydrate_limit)
                        .await?;
                Ok(serde_json::to_value(models)?)
            }
            "get_hf_download_details" => {
                let repo_id =
                    params["repo_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "repo_id is required".to_string(),
                        })?;
                let quants: Vec<String> = serde_json::from_value(params["quants"].clone())
                    .map_err(|e| PumasError::InvalidParams {
                        message: format!("Invalid quants: {e}"),
                    })?;
                let details = get_hf_download_details(self, repo_id, &quants).await?;
                Ok(serde_json::to_value(details)?)
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
            "pause_hf_download" => {
                let download_id =
                    params["download_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "download_id is required".to_string(),
                        })?;
                let paused = pause_hf_download(self, download_id).await?;
                Ok(serde_json::to_value(paused)?)
            }
            "resume_hf_download" => {
                let download_id =
                    params["download_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "download_id is required".to_string(),
                        })?;
                let resumed = resume_hf_download(self, download_id).await?;
                Ok(serde_json::to_value(resumed)?)
            }
            "list_hf_downloads" => {
                let downloads = list_hf_downloads(self).await;
                Ok(serde_json::to_value(downloads)?)
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
            "resume_partial_download" => {
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
                let action = resume_partial_download(self, repo_id, dest_dir).await?;
                Ok(serde_json::to_value(action)?)
            }
            "refetch_metadata_from_hf" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let metadata = refetch_metadata_from_hf(self, model_id).await?;
                Ok(serde_json::to_value(metadata)?)
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
            "lookup_hf_metadata_for_bundle_directory" => {
                let dir_path =
                    params["dir_path"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "dir_path is required".to_string(),
                        })?;
                let result = lookup_hf_metadata_for_bundle_directory(self, dir_path).await?;
                Ok(serde_json::to_value(result)?)
            }
            "set_hf_token" => {
                let token = params["token"]
                    .as_str()
                    .ok_or_else(|| PumasError::InvalidParams {
                        message: "token is required".to_string(),
                    })?;
                set_hf_token(self, token).await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "clear_hf_token" => {
                clear_hf_token(self).await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "get_hf_auth_status" => {
                let status = get_hf_auth_status(self).await?;
                Ok(serde_json::to_value(status)?)
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
            "connectivity_state" => Ok(serde_json::to_value(self.network_manager.connectivity())?),
            "check_connectivity" => {
                let state = self.network_manager.check_connectivity().await;
                Ok(serde_json::to_value(state)?)
            }
            "network_status" => {
                let status = self.network_manager.status().await;
                Ok(serde_json::to_value(status)?)
            }
            "get_network_status_response" => {
                let response = network_status_response(self).await;
                Ok(serde_json::to_value(response)?)
            }
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
            "is_comfyui_running" => Ok(serde_json::to_value(is_comfyui_running(self).await)?),
            "get_running_processes" => {
                let processes = get_running_processes(self).await;
                Ok(serde_json::to_value(processes)?)
            }
            "set_process_version_paths" => {
                let version_paths: std::collections::HashMap<String, PathBuf> =
                    serde_json::from_value(params["version_paths"].clone()).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid version_paths: {e}"),
                        }
                    })?;
                set_process_version_paths(self, version_paths).await;
                Ok(serde_json::json!({ "success": true }))
            }
            "stop_comfyui" => {
                let stopped = stop_comfyui(self).await?;
                Ok(serde_json::to_value(stopped)?)
            }
            "is_ollama_running" => Ok(serde_json::to_value(is_ollama_running(self).await)?),
            "stop_ollama" => {
                let stopped = stop_ollama(self).await?;
                Ok(serde_json::to_value(stopped)?)
            }
            "launch_ollama" => {
                let tag = params["tag"]
                    .as_str()
                    .ok_or_else(|| PumasError::InvalidParams {
                        message: "tag is required".to_string(),
                    })?;
                let version_dir =
                    params["version_dir"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "version_dir is required".to_string(),
                        })?;
                let response = launch_ollama(self, tag, Path::new(version_dir)).await?;
                Ok(serde_json::to_value(response)?)
            }
            "is_torch_running" => Ok(serde_json::to_value(is_torch_running(self).await)?),
            "stop_torch" => {
                let stopped = stop_torch(self).await?;
                Ok(serde_json::to_value(stopped)?)
            }
            "launch_torch" => {
                let tag = params["tag"]
                    .as_str()
                    .ok_or_else(|| PumasError::InvalidParams {
                        message: "tag is required".to_string(),
                    })?;
                let version_dir =
                    params["version_dir"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "version_dir is required".to_string(),
                        })?;
                let response = launch_torch(self, tag, Path::new(version_dir)).await?;
                Ok(serde_json::to_value(response)?)
            }
            "launch_version" => {
                let tag = params["tag"]
                    .as_str()
                    .ok_or_else(|| PumasError::InvalidParams {
                        message: "tag is required".to_string(),
                    })?;
                let version_dir =
                    params["version_dir"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "version_dir is required".to_string(),
                        })?;
                let response = launch_version(self, tag, Path::new(version_dir)).await?;
                Ok(serde_json::to_value(response)?)
            }
            "get_last_launch_log" => {
                let log = get_last_launch_log(self).await;
                Ok(serde_json::to_value(log)?)
            }
            "get_last_launch_error" => {
                let error = get_last_launch_error(self).await;
                Ok(serde_json::to_value(error)?)
            }
            "get_status" => Ok(serde_json::json!({
                "success": true,
                "version": env!("CARGO_PKG_VERSION"),
                "is_primary": true,
            })),
            "has_background_fetch_completed" => {
                let completed = self._state.read().await.background_fetch_completed;
                Ok(serde_json::to_value(completed)?)
            }
            "reset_background_fetch_flag" => {
                self._state.write().await.background_fetch_completed = false;
                Ok(serde_json::json!({ "success": true }))
            }
            "get_launcher_version" => {
                let updater =
                    crate::launcher::LauncherUpdater::new(&launcher_root_from_primary(self));
                Ok(updater.get_version_info())
            }
            "check_launcher_updates" => {
                let force_refresh = params["force_refresh"].as_bool().unwrap_or(false);
                let updater =
                    crate::launcher::LauncherUpdater::new(&launcher_root_from_primary(self));
                let result = updater.check_for_updates(force_refresh).await;
                Ok(serde_json::to_value(result)?)
            }
            "apply_launcher_update" => {
                let updater =
                    crate::launcher::LauncherUpdater::new(&launcher_root_from_primary(self));
                let result = updater.apply_update().await;
                Ok(serde_json::to_value(result)?)
            }
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

pub(super) fn launcher_root_from_primary(primary: &PrimaryState) -> PathBuf {
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

async fn store_model_notes(
    primary: &PrimaryState,
    model_id: &str,
    notes: Option<String>,
) -> std::result::Result<models::UpdateModelNotesResponse, PumasError> {
    let library = &primary.model_library;
    let model_dir = library.library_root().join(model_id);

    if !model_dir.exists() {
        return Ok(models::UpdateModelNotesResponse {
            success: false,
            error: Some(format!("Model not found: {}", model_id)),
            model_id: model_id.to_string(),
            notes: None,
        });
    }

    let mut metadata = library.load_metadata(&model_dir)?.unwrap_or_default();
    let normalized_notes = notes.and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    });
    metadata.notes = normalized_notes.clone();
    metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

    library.save_metadata(&model_dir, &metadata).await?;
    library.index_model_dir(&model_dir).await?;

    Ok(models::UpdateModelNotesResponse {
        success: true,
        error: None,
        model_id: model_id.to_string(),
        notes: normalized_notes,
    })
}

async fn preview_model_mapping_response(
    primary: &PrimaryState,
    version_tag: &str,
    models_path: &Path,
) -> std::result::Result<models::MappingPreviewResponse, PumasError> {
    if !models_path.exists() {
        return Ok(models::MappingPreviewResponse {
            success: false,
            error: Some(format!(
                "Version models directory not found: {}",
                models_path.display()
            )),
            to_create: vec![],
            to_skip_exists: vec![],
            conflicts: vec![],
            broken_to_remove: vec![],
            total_actions: 0,
            warnings: vec![],
            errors: vec![],
        });
    }

    primary
        .model_mapper
        .create_default_comfyui_config("*", models_path)?;

    let preview = primary
        .model_mapper
        .preview_mapping("comfyui", Some(version_tag), models_path)
        .await?;

    let to_action_info = |a: &crate::model_library::MappingAction| models::MappingActionInfo {
        model_id: a.model_id.clone(),
        model_name: a.model_name.clone(),
        source_path: a.source.display().to_string(),
        target_path: a.target.display().to_string(),
        reason: a.reason.clone().unwrap_or_default(),
    };

    let to_create: Vec<_> = preview.creates.iter().map(to_action_info).collect();
    let to_skip_exists: Vec<_> = preview.skips.iter().map(to_action_info).collect();
    let conflicts: Vec<_> = preview.conflicts.iter().map(to_action_info).collect();
    let broken_to_remove: Vec<_> = preview
        .broken
        .iter()
        .map(|a| models::BrokenLinkEntry {
            target_path: a.target.display().to_string(),
            existing_target: a.source.display().to_string(),
            reason: a.reason.clone().unwrap_or_default(),
        })
        .collect();
    let total_actions = to_create.len() + broken_to_remove.len();

    Ok(models::MappingPreviewResponse {
        success: true,
        error: None,
        to_create,
        to_skip_exists,
        conflicts,
        broken_to_remove,
        total_actions,
        warnings: vec![],
        errors: vec![],
    })
}

async fn apply_model_mapping_response(
    primary: &PrimaryState,
    version_tag: &str,
    models_path: &Path,
) -> std::result::Result<models::MappingApplyResponse, PumasError> {
    if !models_path.exists() {
        std::fs::create_dir_all(models_path)?;
    }

    primary
        .model_mapper
        .create_default_comfyui_config("*", models_path)?;

    let result = primary
        .model_mapper
        .apply_mapping("comfyui", Some(version_tag), models_path)
        .await?;

    Ok(models::MappingApplyResponse {
        success: true,
        error: None,
        links_created: result.created,
        links_removed: result.broken_removed,
        total_links: result.created + result.skipped,
    })
}

async fn sync_with_resolutions_response(
    primary: &PrimaryState,
    version_tag: &str,
    models_path: &Path,
    resolutions: std::collections::HashMap<String, model_library::ConflictResolution>,
) -> std::result::Result<models::SyncWithResolutionsResponse, PumasError> {
    if !models_path.exists() {
        std::fs::create_dir_all(models_path)?;
    }

    primary
        .model_mapper
        .create_default_comfyui_config("*", models_path)?;

    let resolution_count = |kind: model_library::ConflictResolution| {
        resolutions.values().filter(|value| **value == kind).count()
    };
    let overwrite_count = resolution_count(model_library::ConflictResolution::Overwrite);
    let rename_count = resolution_count(model_library::ConflictResolution::Rename);

    let typed_resolutions: std::collections::HashMap<PathBuf, model_library::ConflictResolution> =
        resolutions
            .into_iter()
            .map(|(target, resolution)| (PathBuf::from(target), resolution))
            .collect();

    let result = primary
        .model_mapper
        .apply_mapping_with_resolutions(
            "comfyui",
            Some(version_tag),
            models_path,
            &typed_resolutions,
        )
        .await?;

    let errors: Vec<String> = result
        .errors
        .iter()
        .map(|(path, err)| format!("{}: {}", path.display(), err))
        .collect();
    let success = errors.is_empty();
    let error = if success {
        None
    } else {
        Some(format!("{} mapping operation(s) failed", errors.len()))
    };

    Ok(models::SyncWithResolutionsResponse {
        success,
        error,
        links_created: result.created,
        links_skipped: result.skipped + result.conflicts,
        links_renamed: rename_count,
        overwrites: overwrite_count,
        errors,
    })
}
