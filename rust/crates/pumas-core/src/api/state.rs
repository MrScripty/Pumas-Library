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
            "get_library_status" => {
                let _ =
                    reconcile_on_demand(self, ReconcileScope::AllModels, "ipc-get-library-status")
                        .await?;
                let model_count = self.model_library.list_models().await?.len() as u32;
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
                    super::models::relocate_skipped_partial_downloads(self, &mut report).await?;
                if mutated {
                    super::models::recompute_execution_report_counts(&mut report);
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
    search_hf_models_with_hydration(primary, query, kind, limit, limit).await
}

async fn search_hf_models_with_hydration(
    primary: &PrimaryState,
    query: &str,
    kind: Option<&str>,
    limit: usize,
    hydrate_limit: usize,
) -> std::result::Result<Vec<models::HuggingFaceModel>, PumasError> {
    if let Some(ref client) = primary.hf_client {
        let params = model_library::HfSearchParams {
            query: query.to_string(),
            kind: kind.map(String::from),
            limit: Some(limit),
            hydrate_limit: Some(hydrate_limit.min(limit)),
            ..Default::default()
        };
        client.search(&params).await
    } else {
        Ok(vec![])
    }
}

async fn get_hf_download_details(
    primary: &PrimaryState,
    repo_id: &str,
    quants: &[String],
) -> std::result::Result<models::HfDownloadDetails, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.get_download_details(repo_id, quants).await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
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
    let mut huggingface_evidence = match client.get_model_evidence(&request.repo_id).await {
        Ok(evidence) => Some(evidence),
        Err(err) => {
            warn!(
                "Failed to capture HF evidence for {} before download: {}",
                request.repo_id, err
            );
            None
        }
    };
    if let Some(remote_pipeline_tag) = huggingface_evidence
        .as_ref()
        .and_then(|evidence| normalized_download_hint(evidence.pipeline_tag.as_deref()))
    {
        resolved_pipeline_tag = Some(remote_pipeline_tag.to_string());
    }
    let mut resolved_model_type = if let Some(ref evidence) = huggingface_evidence {
        let resolved = model_library::resolve_model_type_from_huggingface_evidence(
            primary.model_library.index(),
            Some(&resolved_request.official_name),
            resolved_pipeline_tag.as_deref(),
            request.model_type.as_deref(),
            Some(evidence),
        )?;
        (resolved.model_type != model_library::ModelType::Unknown)
            .then(|| resolved.model_type.as_str().to_string())
    } else {
        None
    };

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
    if let Some(ref mut evidence) = huggingface_evidence {
        evidence.requested_model_type = request.model_type.clone();
        evidence.requested_pipeline_tag = request.pipeline_tag.clone();
        evidence.requested_quant = request.quant.clone();
    }
    client
        .start_download(&resolved_request, &dest_dir, huggingface_evidence)
        .await
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

async fn pause_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.pause_download(download_id).await
    } else {
        Ok(false)
    }
}

async fn resume_hf_download(
    primary: &PrimaryState,
    download_id: &str,
) -> std::result::Result<bool, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.resume_download(download_id).await
    } else {
        Ok(false)
    }
}

async fn list_hf_downloads(primary: &PrimaryState) -> Vec<models::ModelDownloadProgress> {
    if let Some(ref client) = primary.hf_client {
        client.list_downloads().await
    } else {
        vec![]
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

    client.start_download(&request, dest, None).await
}

async fn resume_partial_download(
    primary: &PrimaryState,
    repo_id: &str,
    dest_dir: &str,
) -> std::result::Result<models::PartialDownloadAction, PumasError> {
    let dest = Path::new(dest_dir);
    if !dest.is_dir() {
        return Ok(models::PartialDownloadAction {
            action: "none".to_string(),
            download_id: None,
            status: None,
            reason_code: Some("dest_dir_missing".to_string()),
            message: Some(format!("directory not found: {}", dest_dir)),
        });
    }

    let client = match primary.hf_client.as_ref() {
        Some(client) => client,
        None => {
            return Ok(models::PartialDownloadAction {
                action: "none".to_string(),
                download_id: None,
                status: None,
                reason_code: Some("hf_client_unavailable".to_string()),
                message: Some("HuggingFace client not initialized".to_string()),
            });
        }
    };

    if let Some(download_id) = client.find_download_id_by_dest_dir(dest).await {
        let status = client.get_download_status(&download_id).await;
        if let Some(status) = status {
            match status {
                models::DownloadStatus::Paused | models::DownloadStatus::Error => {
                    match client.resume_download(&download_id).await {
                        Ok(true) => {
                            return Ok(models::PartialDownloadAction {
                                action: "resume".to_string(),
                                download_id: Some(download_id),
                                status: Some(models::DownloadStatus::Queued),
                                reason_code: None,
                                message: None,
                            });
                        }
                        Ok(false) => {
                            return Ok(models::PartialDownloadAction {
                                action: "none".to_string(),
                                download_id: Some(download_id),
                                status: Some(status),
                                reason_code: Some("resume_rejected".to_string()),
                                message: Some(format!(
                                    "tracked download cannot be resumed from status {:?}",
                                    status
                                )),
                            });
                        }
                        Err(err) => {
                            return Ok(models::PartialDownloadAction {
                                action: "none".to_string(),
                                download_id: Some(download_id),
                                status: Some(status),
                                reason_code: Some(
                                    crate::api::hf::partial_download_reason_code(&err).to_string(),
                                ),
                                message: Some(err.to_string()),
                            });
                        }
                    }
                }
                models::DownloadStatus::Queued
                | models::DownloadStatus::Downloading
                | models::DownloadStatus::Pausing
                | models::DownloadStatus::Cancelling => {
                    return Ok(models::PartialDownloadAction {
                        action: "attach".to_string(),
                        download_id: Some(download_id),
                        status: Some(status),
                        reason_code: None,
                        message: None,
                    });
                }
                models::DownloadStatus::Completed => {
                    return Ok(models::PartialDownloadAction {
                        action: "none".to_string(),
                        download_id: Some(download_id),
                        status: Some(status),
                        reason_code: Some("already_completed".to_string()),
                        message: Some("tracked download is already completed".to_string()),
                    });
                }
                models::DownloadStatus::Cancelled => {
                    return Ok(models::PartialDownloadAction {
                        action: "none".to_string(),
                        download_id: Some(download_id),
                        status: Some(status),
                        reason_code: Some("already_cancelled".to_string()),
                        message: Some(
                            "tracked download was cancelled; start a new download".to_string(),
                        ),
                    });
                }
            }
        }
    }

    match recover_download(primary, repo_id, dest_dir).await {
        Ok(download_id) => Ok(models::PartialDownloadAction {
            action: "recover".to_string(),
            download_id: Some(download_id),
            status: Some(models::DownloadStatus::Queued),
            reason_code: None,
            message: None,
        }),
        Err(err) => Ok(models::PartialDownloadAction {
            action: "none".to_string(),
            download_id: None,
            status: None,
            reason_code: Some(crate::api::hf::partial_download_reason_code(&err).to_string()),
            message: Some(err.to_string()),
        }),
    }
}

async fn refetch_metadata_from_hf(
    primary: &PrimaryState,
    model_id: &str,
) -> std::result::Result<models::ModelMetadata, PumasError> {
    let hf_client = primary
        .hf_client
        .as_ref()
        .ok_or_else(|| PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })?;
    let library = &primary.model_library;

    if let Some(repo_id) = model_id.strip_prefix("download:") {
        let model = hf_client.get_model_info(repo_id).await?;
        let model_type = crate::api::hf::resolve_model_type_from_hints(
            library.index(),
            [Some(model.kind.as_str()), None, None],
        )?;
        return Ok(models::ModelMetadata {
            repo_id: Some(model.repo_id),
            official_name: Some(model.name),
            model_type,
            download_url: Some(model.url),
            match_source: Some("hf".to_string()),
            match_method: Some("repo_id".to_string()),
            match_confidence: Some(1.0),
            ..Default::default()
        });
    }

    let model_dir = library.library_root().join(model_id);
    let current = library.load_metadata(&model_dir)?;

    let repo_id = current
        .as_ref()
        .and_then(|m| m.repo_id.clone())
        .or_else(|| {
            let parts: Vec<&str> = model_id.splitn(3, '/').collect();
            if parts.len() == 3 {
                Some(format!("{}/{}", parts[1], parts[2]))
            } else {
                None
            }
        });

    let hf_result = if let Some(ref repo_id) = repo_id {
        let model = hf_client.get_model_info(repo_id).await?;
        let translated_model_type = crate::api::hf::resolve_model_type_from_hints(
            library.index(),
            [Some(model.kind.as_str()), None, None],
        )?;
        model_library::HfMetadataResult {
            repo_id: model.repo_id,
            official_name: Some(model.name),
            family: None,
            model_type: translated_model_type,
            subtype: None,
            variant: None,
            precision: None,
            tags: vec![],
            base_model: None,
            download_url: Some(model.url),
            description: None,
            match_confidence: 1.0,
            match_method: "repo_id".to_string(),
            requires_confirmation: false,
            hash_mismatch: false,
            matched_filename: None,
            pending_full_verification: false,
            fast_hash: None,
            expected_sha256: None,
        }
    } else {
        let primary_file = library.get_primary_model_file(model_id);
        let file_path = primary_file.ok_or_else(|| PumasError::NotFound {
            resource: format!("primary model file for: {}", model_id),
        })?;
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        hf_client
            .lookup_metadata(filename, Some(&file_path), None)
            .await?
            .ok_or_else(|| PumasError::NotFound {
                resource: format!("HuggingFace metadata for: {}", model_id),
            })?
    };

    library
        .update_metadata_from_hf(model_id, &hf_result, true)
        .await?;

    let updated = library.load_metadata(&model_dir)?.unwrap_or_default();
    Ok(updated)
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

async fn lookup_hf_metadata_for_bundle_directory(
    primary: &PrimaryState,
    dir_path: &str,
) -> std::result::Result<Option<model_library::HfMetadataResult>, PumasError> {
    let Some(client) = primary.hf_client.as_ref() else {
        return Ok(None);
    };

    let bundle_root = Path::new(dir_path);
    let Some(hints) = model_library::get_diffusers_bundle_lookup_hints(bundle_root) else {
        return Ok(None);
    };

    let search_results =
        crate::api::hf::collect_bundle_lookup_candidates(client, &hints.bundle_name).await?;

    for candidate in crate::api::hf::rank_bundle_lookup_candidates(
        &hints.bundle_name,
        hints.name_or_path.as_deref(),
        &search_results,
    ) {
        if client
            .classify_repo_bundle(&candidate.repo_id)
            .await?
            .is_none()
        {
            continue;
        }

        let candidate_repo_id = candidate.repo_id.clone();
        let match_confidence = if crate::api::hf::is_exact_bundle_lookup_match(
            &hints.bundle_name,
            &candidate_repo_id,
            &candidate.name,
        ) {
            0.95
        } else {
            0.72
        };

        return Ok(Some(crate::api::hf::build_lookup_metadata_from_model(
            primary.model_library.index(),
            candidate,
            if match_confidence >= 0.9 {
                "filename_exact"
            } else {
                "filename_fuzzy"
            },
            match_confidence,
            hints
                .name_or_path
                .as_ref()
                .filter(|repo_id| *repo_id != &candidate_repo_id)
                .cloned(),
        )?));
    }

    if let Some((candidate, match_method, match_confidence)) =
        crate::api::hf::fallback_bundle_lookup_candidate(
            &hints.bundle_name,
            hints.name_or_path.as_deref(),
            &search_results,
        )
    {
        let candidate_repo_id = candidate.repo_id.clone();
        return Ok(Some(crate::api::hf::build_lookup_metadata_from_model(
            primary.model_library.index(),
            candidate,
            match_method,
            match_confidence,
            hints
                .name_or_path
                .as_ref()
                .filter(|repo_id| *repo_id != &candidate_repo_id)
                .cloned(),
        )?));
    }

    let Some(base_repo_id) = hints.name_or_path.as_deref() else {
        return Ok(None);
    };
    if !crate::api::hf::looks_like_repo_id(base_repo_id) {
        return Ok(None);
    }

    match client.get_model_info(base_repo_id).await {
        Ok(model) => Ok(Some(crate::api::hf::build_lookup_metadata_from_model(
            primary.model_library.index(),
            model,
            "filename_fuzzy",
            0.55,
            None,
        )?)),
        Err(err) => {
            tracing::warn!(
                "Failed to resolve diffusers bundle base model {} for {}: {}",
                base_repo_id,
                dir_path,
                err
            );
            Ok(None)
        }
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

async fn set_hf_token(primary: &PrimaryState, token: &str) -> std::result::Result<(), PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.set_auth_token(token).await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
    }
}

async fn clear_hf_token(primary: &PrimaryState) -> std::result::Result<(), PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.clear_auth_token().await
    } else {
        Err(PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        })
    }
}

async fn get_hf_auth_status(
    primary: &PrimaryState,
) -> std::result::Result<model_library::HfAuthStatus, PumasError> {
    if let Some(ref client) = primary.hf_client {
        client.get_auth_status().await
    } else {
        Ok(model_library::HfAuthStatus {
            authenticated: false,
            username: None,
            token_source: None,
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

async fn network_status_response(primary: &PrimaryState) -> models::NetworkStatusResponse {
    let status = primary.network_manager.status().await;

    let mut total_successful_requests: u64 = 0;
    let mut total_failed_requests: u64 = 0;
    let mut circuit_states = std::collections::HashMap::new();
    let mut any_open_circuit = false;

    for breaker in &status.circuit_breakers {
        total_successful_requests += breaker.total_successes;
        total_failed_requests += breaker.total_failures;
        let state = breaker.state.to_string();
        if state == "OPEN" {
            any_open_circuit = true;
        }
        circuit_states.insert(breaker.domain.clone(), state);
    }

    let total_requests = total_successful_requests + total_failed_requests;
    let success_rate = if total_requests > 0 {
        total_successful_requests as f64 / total_requests as f64
    } else {
        1.0
    };

    models::NetworkStatusResponse {
        success: true,
        error: None,
        total_requests,
        successful_requests: total_successful_requests,
        failed_requests: total_failed_requests,
        circuit_breaker_rejections: 0,
        retries: 0,
        success_rate,
        circuit_states,
        is_offline: status.connectivity == network::ConnectivityState::Offline || any_open_circuit,
    }
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

async fn is_comfyui_running(primary: &PrimaryState) -> bool {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.is_running()
    } else {
        false
    }
}

async fn get_running_processes(primary: &PrimaryState) -> Vec<process::ProcessInfo> {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.get_processes_with_resources()
    } else {
        vec![]
    }
}

async fn set_process_version_paths(
    primary: &PrimaryState,
    version_paths: std::collections::HashMap<String, PathBuf>,
) {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.set_version_paths(version_paths);
    } else {
        tracing::warn!("PumasApi.set_process_version_paths: process manager not initialized");
    }
}

async fn stop_comfyui(primary: &PrimaryState) -> std::result::Result<bool, PumasError> {
    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(mgr) = process_manager {
        tokio::task::spawn_blocking(move || mgr.stop_all())
            .await
            .map_err(|e| PumasError::Other(format!("Failed to join stop_comfyui task: {}", e)))?
    } else {
        Ok(false)
    }
}

async fn is_ollama_running(primary: &PrimaryState) -> bool {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.is_ollama_running()
    } else {
        false
    }
}

async fn stop_ollama(primary: &PrimaryState) -> std::result::Result<bool, PumasError> {
    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(mgr) = process_manager {
        tokio::task::spawn_blocking(move || mgr.stop_ollama())
            .await
            .map_err(|e| PumasError::Other(format!("Failed to join stop_ollama task: {}", e)))?
    } else {
        Ok(false)
    }
}

async fn launch_ollama(
    primary: &PrimaryState,
    tag: &str,
    version_dir: &Path,
) -> std::result::Result<models::LaunchResponse, PumasError> {
    if !version_dir.exists() {
        return Ok(models::LaunchResponse {
            success: false,
            error: Some(format!(
                "Version directory does not exist: {}",
                version_dir.display()
            )),
            log_path: None,
            ready: None,
        });
    }

    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(pm) = process_manager {
        let log_dir = launcher_root_from_primary(primary)
            .join("launcher-data")
            .join("logs");
        let tag = tag.to_string();
        let version_dir = version_dir.to_path_buf();
        let result = tokio::task::spawn_blocking(move || {
            pm.launch_ollama(&tag, &version_dir, Some(&log_dir))
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join launch_ollama task: {}", e)))?;

        Ok(models::LaunchResponse {
            success: result.success,
            error: result.error,
            log_path: result.log_path.map(|p| p.to_string_lossy().to_string()),
            ready: Some(result.ready),
        })
    } else {
        Ok(models::LaunchResponse {
            success: false,
            error: Some("Process manager not initialized".to_string()),
            log_path: None,
            ready: None,
        })
    }
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

async fn launch_torch(
    primary: &PrimaryState,
    tag: &str,
    version_dir: &Path,
) -> std::result::Result<models::LaunchResponse, PumasError> {
    if !version_dir.exists() {
        return Ok(models::LaunchResponse {
            success: false,
            error: Some(format!(
                "Version directory does not exist: {}",
                version_dir.display()
            )),
            log_path: None,
            ready: None,
        });
    }

    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(pm) = process_manager {
        let log_dir = launcher_root_from_primary(primary)
            .join("launcher-data")
            .join("logs");
        let tag = tag.to_string();
        let version_dir = version_dir.to_path_buf();
        let result = tokio::task::spawn_blocking(move || {
            pm.launch_torch(&tag, &version_dir, Some(&log_dir))
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join launch_torch task: {}", e)))?;

        Ok(models::LaunchResponse {
            success: result.success,
            error: result.error,
            log_path: result.log_path.map(|p| p.to_string_lossy().to_string()),
            ready: Some(result.ready),
        })
    } else {
        Ok(models::LaunchResponse {
            success: false,
            error: Some("Process manager not initialized".to_string()),
            log_path: None,
            ready: None,
        })
    }
}

async fn launch_version(
    primary: &PrimaryState,
    tag: &str,
    version_dir: &Path,
) -> std::result::Result<models::LaunchResponse, PumasError> {
    if !version_dir.exists() {
        return Ok(models::LaunchResponse {
            success: false,
            error: Some(format!(
                "Version directory does not exist: {}",
                version_dir.display()
            )),
            log_path: None,
            ready: None,
        });
    }

    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(pm) = process_manager {
        let log_dir = launcher_root_from_primary(primary)
            .join("launcher-data")
            .join("logs");
        let tag = tag.to_string();
        let version_dir = version_dir.to_path_buf();
        let result = tokio::task::spawn_blocking(move || {
            pm.launch_version(&tag, &version_dir, Some(&log_dir))
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join launch_version task: {}", e)))?;

        Ok(models::LaunchResponse {
            success: result.success,
            error: result.error,
            log_path: result.log_path.map(|p| p.to_string_lossy().to_string()),
            ready: Some(result.ready),
        })
    } else {
        Ok(models::LaunchResponse {
            success: false,
            error: Some("Process manager not initialized".to_string()),
            log_path: None,
            ready: None,
        })
    }
}

async fn get_last_launch_log(primary: &PrimaryState) -> Option<String> {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.last_launch_log()
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    }
}

async fn get_last_launch_error(primary: &PrimaryState) -> Option<String> {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.last_launch_error()
    } else {
        None
    }
}
