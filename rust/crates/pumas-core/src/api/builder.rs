//! Builder for configuring PumasApi initialization.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::RwLock;

use crate::api::state::{ApiState, PrimaryState};
use crate::api::RuntimeTasks;
use crate::error::{PumasError, Result};
use crate::{config, conversion, model_library, network, process, registry, system};
use crate::{ApiInner, PumasApi};

use super::{
    start_model_library_watcher, ReconciliationCoordinator, WatcherWriteSuppressor,
    WATCHER_WRITE_SUPPRESSION_TTL,
};

/// Builder for configuring PumasApi initialization.
///
/// Use this for more control over API initialization options.
///
/// # Example
///
/// ```rust,ignore
/// use pumas_library::PumasApi;
///
/// let api = PumasApi::builder("./my-models")
///     .auto_create_dirs(true)
///     .with_hf_client(false)
///     .build()
///     .await?;
/// ```
pub struct PumasApiBuilder {
    launcher_root: PathBuf,
    auto_create_dirs: bool,
    enable_hf_client: bool,
    enable_process_manager: bool,
}

struct InstanceClaimGuard {
    registry: registry::LibraryRegistry,
    library_path: PathBuf,
    active: bool,
}

impl InstanceClaimGuard {
    fn new(registry: registry::LibraryRegistry, library_path: PathBuf) -> Self {
        Self {
            registry,
            library_path,
            active: true,
        }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for InstanceClaimGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = self.registry.unregister_instance(&self.library_path);
        }
    }
}

fn start_primary_background_work(
    primary_state: Arc<PrimaryState>,
    known_download_dirs: HashSet<PathBuf>,
    runtime_tasks: RuntimeTasks,
) -> Option<model_library::ModelLibraryWatcher> {
    let model_watcher = match start_model_library_watcher(primary_state.clone()) {
        Ok(watcher) => Some(watcher),
        Err(err) => {
            tracing::warn!("Failed to start model library watcher (non-fatal): {}", err);
            None
        }
    };

    {
        let ps = primary_state.clone();
        runtime_tasks.spawn(async move {
            let recoveries = ps.model_importer.recover_incomplete_shards();
            if recoveries.is_empty() {
                return;
            }
            tracing::info!(
                "Found {} incomplete sharded model(s) to recover",
                recoveries.len()
            );
            let Some(ref client) = ps.hf_client else {
                tracing::warn!("Cannot recover incomplete shards: HF client not available");
                return;
            };
            for recovery in recoveries {
                let request = model_library::DownloadRequest {
                    repo_id: recovery.repo_id.clone(),
                    family: recovery.family,
                    official_name: recovery.official_name,
                    model_type: recovery.model_type,
                    quant: None,
                    filename: None,
                    filenames: None,
                    pipeline_tag: None,
                    bundle_format: None,
                    pipeline_class: None,
                    release_date: None,
                    download_url: None,
                    model_card_json: None,
                    license_status: None,
                };
                match client
                    .start_download(&request, &recovery.model_dir, None)
                    .await
                {
                    Ok(id) => {
                        tracing::info!(
                            "Started shard recovery download {} for repo {}",
                            id,
                            recovery.repo_id,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to start shard recovery for {}: {}",
                            recovery.repo_id,
                            e,
                        );
                    }
                }
            }
        });
    }

    {
        let ps = primary_state;
        runtime_tasks.spawn(async move {
            let interrupted = ps
                .model_importer
                .find_interrupted_downloads(&known_download_dirs);
            if interrupted.is_empty() {
                return;
            }
            tracing::info!(
                "Found {} interrupted download(s) to recover",
                interrupted.len()
            );
            let Some(ref client) = ps.hf_client else {
                tracing::warn!("Cannot recover interrupted downloads: HF client not available");
                return;
            };
            for item in interrupted {
                let repo_id = item.repo_id.unwrap_or_else(|| {
                    let dir_name = item
                        .model_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&item.inferred_name);
                    format!("{}/{}", item.family, dir_name)
                });
                let request = model_library::DownloadRequest {
                    repo_id: repo_id.clone(),
                    family: item.family,
                    official_name: item.inferred_name,
                    model_type: item.model_type,
                    quant: None,
                    filename: None,
                    filenames: None,
                    pipeline_tag: None,
                    bundle_format: None,
                    pipeline_class: None,
                    release_date: None,
                    download_url: None,
                    model_card_json: None,
                    license_status: None,
                };
                match client.start_download(&request, &item.model_dir, None).await {
                    Ok(id) => {
                        tracing::info!(
                            "Started interrupted download recovery {} for repo {}",
                            id,
                            repo_id,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to recover interrupted download for {}: {}",
                            repo_id,
                            e,
                        );
                    }
                }
            }
        });
    }

    model_watcher
}

impl PumasApiBuilder {
    /// Create a new builder with the launcher root directory.
    pub fn new(launcher_root: impl Into<PathBuf>) -> Self {
        Self {
            launcher_root: launcher_root.into(),
            auto_create_dirs: false,
            enable_hf_client: true,
            enable_process_manager: true,
        }
    }

    /// Auto-create required directories if they don't exist.
    ///
    /// When enabled, the builder will create the following directories:
    /// - `launcher-data/`
    /// - `launcher-data/metadata/`
    /// - `launcher-data/cache/`
    /// - `launcher-data/mapping-configs/`
    /// - `shared-resources/models/`
    ///
    /// Default: `false` (directories must exist)
    pub fn auto_create_dirs(mut self, enable: bool) -> Self {
        self.auto_create_dirs = enable;
        self
    }

    /// Enable or disable HuggingFace client initialization.
    ///
    /// When disabled, HuggingFace search and download features will not be available.
    ///
    /// Default: `true`
    pub fn with_hf_client(mut self, enable: bool) -> Self {
        self.enable_hf_client = enable;
        self
    }

    /// Enable or disable process manager initialization.
    ///
    /// When disabled, ComfyUI process management features will not be available.
    ///
    /// Default: `true`
    pub fn with_process_manager(mut self, enable: bool) -> Self {
        self.enable_process_manager = enable;
        self
    }

    /// Create the required directory structure.
    async fn create_directory_structure(launcher_root: &Path) -> Result<()> {
        let dirs = [
            launcher_root.join("launcher-data"),
            launcher_root.join("launcher-data").join("metadata"),
            launcher_root.join("launcher-data").join("cache"),
            launcher_root.join("launcher-data").join("cache").join("hf"),
            launcher_root.join("launcher-data").join("mapping-configs"),
            launcher_root.join("launcher-data").join("logs"),
            launcher_root.join("shared-resources"),
            launcher_root.join("shared-resources").join("models"),
        ];

        for dir in &dirs {
            if !fs::try_exists(dir)
                .await
                .map_err(|e| PumasError::io_with_path(e, dir))?
            {
                fs::create_dir_all(dir).await.map_err(|e| PumasError::Io {
                    message: format!("Failed to create directory: {}", dir.display()),
                    path: Some(dir.clone()),
                    source: Some(e),
                })?;
            }
        }

        Ok(())
    }

    /// Build the PumasApi instance.
    pub async fn build(self) -> Result<PumasApi> {
        // Auto-create directories if requested
        if self.auto_create_dirs {
            // Create launcher_root if it doesn't exist
            if !fs::try_exists(&self.launcher_root)
                .await
                .map_err(|e| PumasError::io_with_path(e, &self.launcher_root))?
            {
                fs::create_dir_all(&self.launcher_root)
                    .await
                    .map_err(|e| PumasError::Io {
                        message: format!(
                            "Failed to create launcher root: {}",
                            self.launcher_root.display()
                        ),
                        path: Some(self.launcher_root.clone()),
                        source: Some(e),
                    })?;
            }
            Self::create_directory_structure(&self.launcher_root).await?;
        } else {
            // Ensure the launcher root exists
            if !fs::try_exists(&self.launcher_root)
                .await
                .map_err(|e| PumasError::io_with_path(e, &self.launcher_root))?
            {
                return Err(PumasError::Config {
                    message: format!(
                        "Launcher root does not exist: {}",
                        self.launcher_root.display()
                    ),
                });
            }
        }

        let registry = registry::LibraryRegistry::open()?;
        let library_name = self
            .launcher_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pumas-library");
        let _ = registry.register(&self.launcher_root, library_name)?;
        let claim = loop {
            match registry.try_claim_instance(&self.launcher_root, std::process::id())? {
                registry::InstanceClaimResult::Claimed(claim) => break claim,
                registry::InstanceClaimResult::Occupied(_) => {
                    if let Some(client) =
                        PumasApi::connect_or_wait_for_existing_instance(&self.launcher_root).await?
                    {
                        return Ok(client);
                    }
                }
            }
        };
        let mut claim_guard = InstanceClaimGuard::new(registry.clone(), claim.library_path.clone());

        let state = Arc::new(RwLock::new(ApiState {
            background_fetch_completed: false,
        }));
        let runtime_tasks = RuntimeTasks::default();

        // Initialize network manager for connectivity checking
        let network_manager =
            Arc::new(
                network::NetworkManager::new().map_err(|e| PumasError::Config {
                    message: format!("Failed to initialize network manager: {}", e),
                })?,
            );

        // Check initial connectivity (non-blocking, will update state)
        let nm_clone = network_manager.clone();
        runtime_tasks.spawn(async move {
            nm_clone.check_connectivity().await;
        });

        // Initialize process manager (if enabled)
        let process_manager = if self.enable_process_manager {
            match process::ProcessManager::new(&self.launcher_root, None) {
                Ok(mgr) => Arc::new(RwLock::new(Some(mgr))),
                Err(e) => {
                    tracing::warn!("Failed to initialize process manager: {}", e);
                    Arc::new(RwLock::new(None))
                }
            }
        } else {
            Arc::new(RwLock::new(None))
        };

        // Initialize system utilities
        let system_utils = Arc::new(system::SystemUtils::new(&self.launcher_root));

        // Initialize model library for AI model management
        let model_library_dir = self.launcher_root.join("shared-resources").join("models");
        let mapping_config_dir = self
            .launcher_root
            .join("launcher-data")
            .join("mapping-configs");

        // Initialize HuggingFace client (if enabled)
        let mut hf_client = if self.enable_hf_client {
            let cache_dir = self
                .launcher_root
                .join("launcher-data")
                .join(config::PathsConfig::CACHE_DIR_NAME);
            let hf_cache_dir = cache_dir.join("hf");

            // Initialize SQLite search cache at shared-resources/cache/search.sqlite
            let search_cache_dir = self.launcher_root.join("shared-resources").join("cache");
            let search_cache_db = search_cache_dir.join("search.sqlite");
            let search_cache_db_for_task = search_cache_db.clone();
            let search_cache = match tokio::task::spawn_blocking(move || {
                model_library::HfSearchCache::new(&search_cache_db_for_task)
                    .map(std::sync::Arc::new)
            })
            .await
            {
                Ok(Ok(cache)) => Some(cache),
                Ok(Err(e)) => {
                    tracing::warn!("Failed to initialize HuggingFace search cache: {}", e);
                    None
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to join HuggingFace search cache initialization task: {}",
                        e
                    );
                    None
                }
            };

            // Initialize download persistence
            let data_dir = self.launcher_root.join("launcher-data");
            let download_persistence =
                std::sync::Arc::new(model_library::DownloadPersistence::new(&data_dir));

            let hf_cache_dir_for_task = hf_cache_dir.clone();
            match tokio::task::spawn_blocking(move || {
                model_library::HuggingFaceClient::new(&hf_cache_dir_for_task)
            })
            .await
            {
                Ok(Ok(mut client)) => {
                    // Attach search cache if available
                    if let Some(cache) = search_cache {
                        client.set_search_cache(cache);
                    }
                    // Attach download persistence
                    client.set_persistence(download_persistence);
                    // Restore persisted downloads from previous session
                    client.restore_persisted_downloads().await;
                    Some(client)
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to initialize HuggingFace client: {}", e);
                    None
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to join HuggingFace client initialization task: {}",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Initialize model library (required - core functionality)
        let model_library = model_library::ModelLibrary::new(&model_library_dir)
            .await
            .map_err(|e| PumasError::Config {
                message: format!("Model library initialization failed: {}", e),
            })?;
        let model_library = Arc::new(model_library);
        let watcher_write_suppressor =
            Arc::new(WatcherWriteSuppressor::new(WATCHER_WRITE_SUPPRESSION_TTL));
        model_library.set_metadata_write_notifier(Some(Arc::new({
            let suppressor = watcher_write_suppressor.clone();
            move |path| suppressor.record(path)
        })));
        let model_mapper =
            model_library::ModelMapper::new(model_library.clone(), &mapping_config_dir);
        let model_importer = model_library::ModelImporter::new(model_library.clone());

        // Wire download completion -> in-place import (metadata + indexing)
        if let Some(ref mut client) = hf_client {
            let lib = model_library.clone();
            let tasks = runtime_tasks.clone();
            client.set_aux_complete_callback(std::sync::Arc::new(
                move |info: model_library::AuxFilesCompleteInfo| {
                    let lib = lib.clone();
                    tasks.spawn(async move {
                        let importer = model_library::ModelImporter::new(lib);
                        if let Err(err) = importer.upsert_download_metadata_stub(&info).await {
                            tracing::warn!(
                                "Failed to persist partial download metadata for {}: {}",
                                info.download_id,
                                err
                            );
                        }
                    });
                },
            ));

            let lib = model_library.clone();
            let tasks = runtime_tasks.clone();
            client.set_completion_callback(std::sync::Arc::new(
                move |info: model_library::DownloadCompletionInfo| {
                    let lib = lib.clone();
                    tasks.spawn(async move {
                        let importer = model_library::ModelImporter::new(lib);
                        match importer.finalize_downloaded_directory(&info).await {
                            Ok(r) if r.success => {
                                tracing::info!("Post-download import succeeded: {:?}", r.model_id);
                            }
                            Ok(r) => {
                                tracing::warn!("Post-download import failed: {:?}", r.error);
                            }
                            Err(e) => {
                                tracing::error!("Post-download import error: {}", e);
                            }
                        }
                    });
                },
            ));
        }

        // Initialize conversion manager
        let conversion_manager = Arc::new(conversion::ConversionManager::new(
            self.launcher_root.clone(),
            model_library.clone(),
            Arc::new(model_library::ModelImporter::new(model_library.clone())),
        ));

        // Spawn non-blocking orphan scan to adopt models missing metadata
        {
            let lib_clone = model_library.clone();
            let importer = model_library::ModelImporter::new(lib_clone);
            if importer.has_orphan_candidates() {
                runtime_tasks.spawn(async move {
                    let result = importer.adopt_orphans(false).await;
                    if result.orphans_found > 0 {
                        tracing::info!(
                            "Startup orphan scan: found={}, adopted={}, errors={}",
                            result.orphans_found,
                            result.adopted,
                            result.errors.len()
                        );
                    }
                });
            }
        }

        // Collect known dest_dirs for interrupted download detection
        // (must happen before hf_client is moved into PrimaryState)
        let known_download_dirs: std::collections::HashSet<std::path::PathBuf> =
            if let Some(ref client) = hf_client {
                if let Some(persistence) = client.persistence() {
                    persistence
                        .load_all()
                        .into_iter()
                        .map(|e| e.dest_dir)
                        .collect()
                } else {
                    std::collections::HashSet::new()
                }
            } else {
                std::collections::HashSet::new()
            };

        let primary_state = Arc::new(PrimaryState {
            _state: state,
            network_manager,
            process_manager,
            system_utils,
            model_library,
            model_mapper,
            hf_client,
            model_importer,
            conversion_manager,
            runtime_tasks: runtime_tasks.clone(),
            reconciliation: Arc::new(ReconciliationCoordinator::new(
                Duration::from_secs(5),
                Duration::from_secs(5),
            )),
            watcher_write_suppressor,
            server_handle: tokio::sync::Mutex::new(None),
            registry: Some(registry),
            instance_claim: tokio::sync::Mutex::new(Some(claim)),
        });
        primary_state.reconciliation.mark_dirty_all().await;

        let mut api = PumasApi {
            launcher_root: self.launcher_root,
            inner: ApiInner::Primary(primary_state),
            model_watcher: None,
            runtime_tasks: runtime_tasks.clone(),
        };
        api.start_ipc_server().await?;
        api.model_watcher = start_primary_background_work(
            api.primary().clone(),
            known_download_dirs,
            runtime_tasks,
        );
        claim_guard.disarm();

        Ok(api)
    }
}
