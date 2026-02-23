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

        // Wire download completion -> in-place import (metadata + indexing)
        if let Some(ref mut client) = hf_client {
            let lib = model_library.clone();
            client.set_completion_callback(std::sync::Arc::new(move |info: model_library::DownloadCompletionInfo| {
                let lib = lib.clone();
                tokio::spawn(async move {
                    // Remove stale metadata from any previous partial download
                    // so import_in_place re-scans all files now present
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

        Ok(PumasApi {
            launcher_root: self.launcher_root,
            inner: ApiInner::Primary(primary_state),
        })
    }
}
