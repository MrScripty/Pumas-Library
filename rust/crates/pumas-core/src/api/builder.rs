//! Builder for configuring PumasApi initialization.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::api::state::{ApiState, PrimaryState};
use crate::error::{PumasError, Result};
use crate::{config, conversion, model_library, network, process, registry, system};
use crate::{ApiInner, PumasApi};

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
    fn create_directory_structure(launcher_root: &PathBuf) -> Result<()> {
        use std::fs;

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
            if !dir.exists() {
                fs::create_dir_all(dir).map_err(|e| PumasError::Io {
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
            if !self.launcher_root.exists() {
                std::fs::create_dir_all(&self.launcher_root).map_err(|e| PumasError::Io {
                    message: format!("Failed to create launcher root: {}", self.launcher_root.display()),
                    path: Some(self.launcher_root.clone()),
                    source: Some(e),
                })?;
            }
            Self::create_directory_structure(&self.launcher_root)?;
        } else {
            // Ensure the launcher root exists
            if !self.launcher_root.exists() {
                return Err(PumasError::Config {
                    message: format!("Launcher root does not exist: {}", self.launcher_root.display()),
                });
            }
        }

        let state = Arc::new(RwLock::new(ApiState {
            background_fetch_completed: false,
        }));

        // Initialize network manager for connectivity checking
        let network_manager = Arc::new(
            network::NetworkManager::new().map_err(|e| PumasError::Config {
                message: format!("Failed to initialize network manager: {}", e),
            })?,
        );

        // Check initial connectivity (non-blocking, will update state)
        let nm_clone = network_manager.clone();
        tokio::spawn(async move {
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
        let model_library_dir = self.launcher_root
            .join("shared-resources")
            .join("models");
        let mapping_config_dir = self.launcher_root
            .join("launcher-data")
            .join("mapping-configs");

        // Initialize HuggingFace client (if enabled)
        let mut hf_client = if self.enable_hf_client {
            let cache_dir = self.launcher_root
                .join("launcher-data")
                .join(config::PathsConfig::CACHE_DIR_NAME);
            let hf_cache_dir = cache_dir.join("hf");

            // Initialize SQLite search cache at shared-resources/cache/search.sqlite
            let search_cache_dir = self.launcher_root.join("shared-resources").join("cache");
            let search_cache_db = search_cache_dir.join("search.sqlite");
            let search_cache = match model_library::HfSearchCache::new(&search_cache_db) {
                Ok(cache) => Some(std::sync::Arc::new(cache)),
                Err(e) => {
                    tracing::warn!("Failed to initialize HuggingFace search cache: {}", e);
                    None
                }
            };

            // Initialize download persistence
            let data_dir = self.launcher_root.join("launcher-data");
            let download_persistence = std::sync::Arc::new(
                model_library::DownloadPersistence::new(&data_dir)
            );

            match model_library::HuggingFaceClient::new(&hf_cache_dir) {
                Ok(mut client) => {
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
                Err(e) => {
                    tracing::warn!("Failed to initialize HuggingFace client: {}", e);
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
        let model_mapper = model_library::ModelMapper::new(model_library.clone(), &mapping_config_dir);
        let model_importer = model_library::ModelImporter::new(model_library.clone());

        // Wire aux-complete callback -> early metadata stub (model appears in index during download)
        if let Some(ref mut client) = hf_client {
            let lib = model_library.clone();
            client.set_aux_complete_callback(std::sync::Arc::new(move |info: model_library::AuxFilesCompleteInfo| {
                let lib = lib.clone();
                tokio::spawn(async move {
                    let req = &info.download_request;
                    let cleaned_name = model_library::normalize_name(&req.official_name);
                    let model_type = req.model_type.clone().unwrap_or_else(|| "unknown".to_string());
                    let model_id = format!("{}/{}/{}", model_type, req.family, cleaned_name);
                    let now = chrono::Utc::now().to_rfc3339();

                    let metadata = model_library::ModelMetadata {
                        model_id: Some(model_id),
                        family: Some(req.family.clone()),
                        model_type: Some(model_type),
                        official_name: Some(req.official_name.clone()),
                        cleaned_name: Some(cleaned_name),
                        repo_id: Some(req.repo_id.clone()),
                        expected_files: Some(info.filenames.clone()),
                        added_date: Some(now.clone()),
                        updated_date: Some(now),
                        size_bytes: info.total_bytes,
                        match_source: Some("download".to_string()),
                        pending_online_lookup: Some(true),
                        lookup_attempts: Some(0),
                        ..Default::default()
                    };

                    if let Err(e) = lib.save_metadata(&info.dest_dir, &metadata).await {
                        tracing::warn!("Failed to write early metadata stub: {}", e);
                        return;
                    }
                    if let Err(e) = lib.index_model_dir(&info.dest_dir).await {
                        tracing::warn!("Failed to index early metadata stub: {}", e);
                    }

                    tracing::info!(
                        "Early metadata stub created for {} (download in progress)",
                        req.official_name,
                    );
                });
            }));
        }

        // Wire download completion -> in-place import (metadata + indexing)
        if let Some(ref mut client) = hf_client {
            let lib = model_library.clone();
            client.set_completion_callback(std::sync::Arc::new(move |info: model_library::DownloadCompletionInfo| {
                let lib = lib.clone();
                tokio::spawn(async move {
                    // Remove stale metadata (including early stub) so import_in_place
                    // re-scans all files now present and creates the full metadata
                    let metadata_path = info.dest_dir.join("metadata.json");
                    if metadata_path.exists() {
                        tracing::info!("Removing stale metadata before re-import: {}", metadata_path.display());
                        let _ = tokio::fs::remove_file(&metadata_path).await;
                    }

                    let importer = model_library::ModelImporter::new(lib);
                    let spec = model_library::InPlaceImportSpec {
                        model_dir: info.dest_dir,
                        official_name: info.download_request.official_name,
                        family: info.download_request.family,
                        model_type: info.download_request.model_type,
                        repo_id: Some(info.download_request.repo_id.clone()),
                        known_sha256: info.known_sha256,
                        compute_hashes: false,
                        expected_files: Some(info.filenames.clone()),
                        pipeline_tag: info.download_request.pipeline_tag,
                    };
                    match importer.import_in_place(&spec).await {
                        Ok(r) if r.success => {
                            tracing::info!("Post-download import succeeded: {:?}", r.model_path);
                        }
                        Ok(r) => {
                            tracing::warn!("Post-download import failed: {:?}", r.error);
                        }
                        Err(e) => {
                            tracing::error!("Post-download import error: {}", e);
                        }
                    }
                });
            }));
        }

        // Initialize conversion manager
        let conversion_manager = Arc::new(conversion::ConversionManager::new(
            self.launcher_root.clone(),
            model_library.clone(),
            Arc::new(model_library::ModelImporter::new(model_library.clone())),
        ));

        // Best-effort registry connection (failures don't block initialization)
        let registry = match registry::LibraryRegistry::open() {
            Ok(reg) => Some(reg),
            Err(e) => {
                tracing::warn!("Failed to open global registry (non-fatal): {}", e);
                None
            }
        };

        // Spawn non-blocking orphan scan to adopt models missing metadata
        {
            let lib_clone = model_library.clone();
            tokio::spawn(async move {
                let importer = model_library::ModelImporter::new(lib_clone);
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
            server_handle: tokio::sync::Mutex::new(None),
            registry,
        });

        // Spawn one-time recovery for incomplete sharded models
        {
            let ps = primary_state.clone();
            tokio::spawn(async move {
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
                        quant: None,     // Download all files for this repo
                        filename: None,
                        pipeline_tag: None,
                    };
                    // start_download skips files already on disk, so it will
                    // only download the missing shards
                    match client.start_download(&request, &recovery.model_dir).await {
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

        // Spawn one-time recovery for interrupted downloads (.part files, no persistence)
        {
            let ps = primary_state.clone();
            let known_dirs = known_download_dirs;
            tokio::spawn(async move {
                let interrupted = ps.model_importer.find_interrupted_downloads(&known_dirs);
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
                    // Use repo_id from marker file, or fall back to inferred path
                    let repo_id = item.repo_id.unwrap_or_else(|| {
                        // Best-guess: {family}/{dir_name} using raw directory name
                        let dir_name = item.model_dir
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
                        pipeline_tag: None,
                    };
                    match client.start_download(&request, &item.model_dir).await {
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

        Ok(PumasApi {
            launcher_root: self.launcher_root,
            inner: ApiInner::Primary(primary_state),
        })
    }
}
