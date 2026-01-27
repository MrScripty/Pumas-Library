//! Pumas Core - Headless library for AI model and version management.
//!
//! This crate provides the core functionality for managing AI application versions
//! (ComfyUI, Ollama, etc.) and AI models. It can be used programmatically without
//! any HTTP/RPC layer.
//!
//! # Example
//!
//! ```rust,no_run
//! use pumas_core::{PumasApi, config::AppId};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let api = PumasApi::new("/path/to/pumas").await?;
//!
//!     // Get available versions
//!     let versions = api.get_available_versions(false, None).await?;
//!     println!("Found {} versions", versions.len());
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod custom_nodes;
pub mod error;
pub mod index;
pub mod launcher;
pub mod metadata;
pub mod model_library;
pub mod models;
pub mod network;
pub mod process;
pub mod shortcut;
pub mod system;
pub mod version_manager;

// Re-export commonly used types
pub use config::AppId;
pub use custom_nodes::CustomNodesManager;
pub use error::{PumasError, Result};
pub use index::{ModelIndex, ModelRecord, SearchResult};
pub use metadata::MetadataManager;
pub use model_library::sharding::{self, ShardValidation};
pub use model_library::{
    HuggingFaceClient, ModelImporter, ModelLibrary, ModelMapper,
    HfSearchParams, DownloadRequest, BatchImportProgress,
};
pub use process::{ProcessManager, ProcessInfo};
pub use shortcut::{ShortcutManager, ShortcutState, ShortcutResult};
pub use system::{GpuInfo, GpuMonitor, ProcessResources, ResourceTracker, SystemResourceSnapshot, SystemUtils, SystemCheckResult, check_git, check_brave, check_setproctitle};
pub use launcher::{LauncherUpdater, PatchManager, UpdateCheckResult, UpdateApplyResult, CommitInfo};
pub use version_manager::{VersionManager, SizeCalculator, ReleaseSize, SizeBreakdown};

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main API struct for Pumas operations.
///
/// This is the primary entry point for programmatic access to Pumas functionality.
/// It manages versions, models, and system integration.
pub struct PumasApi {
    /// Root directory for launcher data
    launcher_root: PathBuf,
    /// Shared state (will be expanded as we implement more features)
    _state: Arc<RwLock<ApiState>>,
    /// Version manager for ComfyUI
    version_manager: Arc<RwLock<Option<version_manager::VersionManager>>>,
    /// Process manager for managing running processes
    process_manager: Arc<RwLock<Option<process::ProcessManager>>>,
    /// Shortcut manager for desktop/menu shortcuts
    shortcut_manager: Arc<RwLock<Option<shortcut::ShortcutManager>>>,
    /// System utilities
    system_utils: Arc<system::SystemUtils>,
    /// Custom nodes manager for managing custom nodes in versions
    custom_nodes_manager: Arc<custom_nodes::CustomNodesManager>,
    /// Size calculator for estimating release sizes
    size_calculator: Arc<RwLock<version_manager::SizeCalculator>>,
    /// Model library for managing AI models
    model_library: Arc<RwLock<Option<Arc<model_library::ModelLibrary>>>>,
    /// Model mapper for linking models to application directories
    model_mapper: Arc<RwLock<Option<model_library::ModelMapper>>>,
    /// HuggingFace client for model search and download
    hf_client: Arc<RwLock<Option<model_library::HuggingFaceClient>>>,
    /// Model importer for importing local models
    model_importer: Arc<RwLock<Option<model_library::ModelImporter>>>,
}

/// Internal state for the API.
struct ApiState {
    /// Whether background fetch has completed
    background_fetch_completed: bool,
}

impl PumasApi {
    /// Create a new PumasApi instance.
    ///
    /// # Arguments
    ///
    /// * `launcher_root` - Path to the launcher root directory (containing launcher-data, etc.)
    pub async fn new(launcher_root: impl Into<PathBuf>) -> Result<Self> {
        let launcher_root = launcher_root.into();

        // Ensure the launcher root exists
        if !launcher_root.exists() {
            return Err(PumasError::Config {
                message: format!("Launcher root does not exist: {}", launcher_root.display()),
            });
        }

        let state = Arc::new(RwLock::new(ApiState {
            background_fetch_completed: false,
        }));

        // Initialize version manager
        let version_manager = match version_manager::VersionManager::new(&launcher_root, AppId::ComfyUI).await {
            Ok(mgr) => Arc::new(RwLock::new(Some(mgr))),
            Err(e) => {
                tracing::warn!("Failed to initialize version manager: {}", e);
                Arc::new(RwLock::new(None))
            }
        };

        // Initialize process manager
        let process_manager = match process::ProcessManager::new(&launcher_root, None) {
            Ok(mgr) => Arc::new(RwLock::new(Some(mgr))),
            Err(e) => {
                tracing::warn!("Failed to initialize process manager: {}", e);
                Arc::new(RwLock::new(None))
            }
        };

        // Initialize shortcut manager
        let shortcut_manager = match shortcut::ShortcutManager::new(&launcher_root) {
            Ok(mgr) => Arc::new(RwLock::new(Some(mgr))),
            Err(e) => {
                tracing::warn!("Failed to initialize shortcut manager: {}", e);
                Arc::new(RwLock::new(None))
            }
        };

        // Initialize system utilities
        let system_utils = Arc::new(system::SystemUtils::new(&launcher_root));

        // Initialize custom nodes manager (for ComfyUI versions)
        let versions_dir = launcher_root.join(AppId::ComfyUI.versions_dir_name());
        let custom_nodes_manager = Arc::new(custom_nodes::CustomNodesManager::new(versions_dir));

        // Initialize size calculator
        let cache_dir = launcher_root
            .join("launcher-data")
            .join(config::PathsConfig::CACHE_DIR_NAME);
        let size_calculator = Arc::new(RwLock::new(version_manager::SizeCalculator::new(cache_dir.clone())));

        // Initialize model library for AI model management
        let model_library_dir = launcher_root
            .join("launcher-data")
            .join("model-library");
        let mapping_config_dir = launcher_root
            .join("launcher-data")
            .join("mapping-configs");

        // Initialize HuggingFace client for model search/download
        let hf_cache_dir = cache_dir.join("hf");
        let hf_client = match model_library::HuggingFaceClient::new(&hf_cache_dir) {
            Ok(client) => Arc::new(RwLock::new(Some(client))),
            Err(e) => {
                tracing::warn!("Failed to initialize HuggingFace client: {}", e);
                Arc::new(RwLock::new(None))
            }
        };

        let (model_library, model_mapper, model_importer) = match model_library::ModelLibrary::new(&model_library_dir).await {
            Ok(library) => {
                let lib_arc = Arc::new(library);
                let mapper = model_library::ModelMapper::new(lib_arc.clone(), &mapping_config_dir);
                let importer = model_library::ModelImporter::new(lib_arc.clone());
                (
                    Arc::new(RwLock::new(Some(lib_arc))),
                    Arc::new(RwLock::new(Some(mapper))),
                    Arc::new(RwLock::new(Some(importer))),
                )
            }
            Err(e) => {
                tracing::warn!("Failed to initialize model library: {}", e);
                (
                    Arc::new(RwLock::new(None)),
                    Arc::new(RwLock::new(None)),
                    Arc::new(RwLock::new(None)),
                )
            }
        };

        Ok(Self {
            launcher_root,
            _state: state,
            version_manager,
            process_manager,
            shortcut_manager,
            system_utils,
            custom_nodes_manager,
            size_calculator,
            model_library,
            model_mapper,
            hf_client,
            model_importer,
        })
    }

    /// Get the launcher root directory.
    pub fn launcher_root(&self) -> &PathBuf {
        &self.launcher_root
    }

    /// Get the launcher-data directory path.
    pub fn launcher_data_dir(&self) -> PathBuf {
        self.launcher_root.join("launcher-data")
    }

    /// Get the metadata directory path.
    pub fn metadata_dir(&self) -> PathBuf {
        self.launcher_data_dir().join(config::PathsConfig::METADATA_DIR_NAME)
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> PathBuf {
        self.launcher_data_dir().join(config::PathsConfig::CACHE_DIR_NAME)
    }

    /// Get the shared resources directory path.
    pub fn shared_resources_dir(&self) -> PathBuf {
        self.launcher_root.join(config::PathsConfig::SHARED_RESOURCES_DIR_NAME)
    }

    /// Get the versions directory for a specific app.
    pub fn versions_dir(&self, app_id: AppId) -> PathBuf {
        self.launcher_root.join(app_id.versions_dir_name())
    }

    // ========================================
    // Status & System Methods (stubs for now)
    // ========================================

    /// Get overall system status.
    pub async fn get_status(&self) -> Result<models::StatusResponse> {
        // Get actual running status
        let comfyui_running = self.is_comfyui_running().await;
        let last_launch_error = self.get_last_launch_error().await;
        let last_launch_log = self.get_last_launch_log().await;

        // Get shortcut state for active version (if any)
        let (menu_shortcut, desktop_shortcut, shortcut_version) = {
            let vm_lock = self.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                match vm.get_active_version().await {
                    Ok(Some(active)) => {
                        let state = self.get_version_shortcut_state(&active).await;
                        (state.menu, state.desktop, Some(active))
                    }
                    _ => (false, false, None),
                }
            } else {
                (false, false, None)
            }
        };

        Ok(models::StatusResponse {
            success: true,
            error: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            deps_ready: true,  // TODO: implement real check
            patched: false,    // TODO: implement real check
            menu_shortcut,
            desktop_shortcut,
            shortcut_version,
            message: if comfyui_running {
                "ComfyUI running".to_string()
            } else {
                "Ready".to_string()
            },
            comfyui_running,
            last_launch_error,
            last_launch_log,
            app_resources: None, // TODO: implement resource gathering
        })
    }

    /// Get disk space information.
    pub async fn get_disk_space(&self) -> Result<models::DiskSpaceResponse> {
        use sysinfo::Disks;

        let disks = Disks::new_with_refreshed_list();

        // Find the disk containing the launcher root
        let launcher_root_str = self.launcher_root.to_string_lossy();

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

        // Fallback: use first disk
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

        Err(PumasError::Other("Could not determine disk space".into()))
    }

    /// Get system resources (CPU, GPU, RAM, disk).
    pub async fn get_system_resources(&self) -> Result<models::SystemResourcesResponse> {
        use sysinfo::{System, Disks};

        let mut sys = System::new_all();
        sys.refresh_all();

        // CPU
        let cpu_usage = sys.global_cpu_usage();

        // RAM
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let ram_usage = if total_memory > 0 {
            (used_memory as f32 / total_memory as f32) * 100.0
        } else {
            0.0
        };

        // Disk
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

        // GPU (placeholder - would need nvml-wrapper for real GPU stats)
        let gpu = models::GpuResources {
            usage: 0.0,
            memory: 0,
            memory_total: 0,
            temp: None,
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

    // ========================================
    // Version Management Methods
    // ========================================

    /// Get the version manager for an app.
    pub async fn get_version_manager(&self, _app_id: Option<AppId>) -> Result<Arc<RwLock<Option<version_manager::VersionManager>>>> {
        // For now, return the ComfyUI version manager
        // In the future, we could have separate managers per app
        Ok(self.version_manager.clone())
    }

    /// Get available versions from GitHub.
    pub async fn get_available_versions(
        &self,
        force_refresh: bool,
        _app_id: Option<AppId>,
    ) -> Result<Vec<models::VersionReleaseInfo>> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            let releases = mgr.get_available_releases(force_refresh).await?;
            Ok(releases.into_iter().map(models::VersionReleaseInfo::from).collect())
        } else {
            Ok(vec![])
        }
    }

    /// Get installed versions.
    pub async fn get_installed_versions(
        &self,
        _app_id: Option<AppId>,
    ) -> Result<Vec<String>> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.get_installed_versions().await
        } else {
            Ok(vec![])
        }
    }

    /// Get the active (currently selected) version.
    pub async fn get_active_version(
        &self,
        _app_id: Option<AppId>,
    ) -> Result<Option<String>> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.get_active_version().await
        } else {
            Ok(None)
        }
    }

    /// Get the default version.
    pub async fn get_default_version(
        &self,
        _app_id: Option<AppId>,
    ) -> Result<Option<String>> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.get_default_version().await
        } else {
            Ok(None)
        }
    }

    /// Set the active version.
    pub async fn set_active_version(
        &self,
        tag: &str,
        _app_id: Option<AppId>,
    ) -> Result<bool> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.set_active_version(tag).await
        } else {
            Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            })
        }
    }

    /// Set the default version.
    pub async fn set_default_version(
        &self,
        tag: Option<&str>,
        _app_id: Option<AppId>,
    ) -> Result<bool> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.set_default_version(tag).await
        } else {
            Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            })
        }
    }

    /// Get installation progress.
    pub async fn get_installation_progress(
        &self,
        _app_id: Option<AppId>,
    ) -> Option<models::InstallationProgress> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.get_installation_progress().await
        } else {
            None
        }
    }

    /// Cancel the current installation.
    pub async fn cancel_installation(
        &self,
        _app_id: Option<AppId>,
    ) -> Result<bool> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.cancel_installation().await
        } else {
            Ok(false)
        }
    }

    /// Remove an installed version.
    pub async fn remove_version(
        &self,
        tag: &str,
        _app_id: Option<AppId>,
    ) -> Result<bool> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.remove_version(tag).await
        } else {
            Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            })
        }
    }

    /// Validate all installations.
    pub async fn validate_installations(
        &self,
        _app_id: Option<AppId>,
    ) -> Result<version_manager::ValidationResult> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.validate_installations().await
        } else {
            Ok(version_manager::ValidationResult {
                removed_tags: vec![],
                orphaned_dirs: vec![],
                valid_count: 0,
            })
        }
    }

    /// Install a version from GitHub.
    ///
    /// Returns a receiver for progress updates.
    pub async fn install_version(
        &self,
        tag: &str,
        _app_id: Option<AppId>,
    ) -> Result<tokio::sync::mpsc::Receiver<version_manager::ProgressUpdate>> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.install_version(tag).await
        } else {
            Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            })
        }
    }

    /// Check dependencies for a version.
    pub async fn check_version_dependencies(
        &self,
        tag: &str,
        _app_id: Option<AppId>,
    ) -> Result<models::DependencyStatus> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.check_dependencies(tag).await
        } else {
            Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            })
        }
    }

    /// Install dependencies for a version.
    pub async fn install_version_dependencies(
        &self,
        tag: &str,
        _app_id: Option<AppId>,
    ) -> Result<bool> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.install_dependencies(tag, None).await
        } else {
            Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            })
        }
    }

    /// Get release dependencies (requirements.txt packages) for a version.
    pub async fn get_release_dependencies(
        &self,
        tag: &str,
        _app_id: Option<AppId>,
    ) -> Result<Vec<String>> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            let version_path = mgr.version_path(tag);
            let requirements_path = version_path.join("requirements.txt");

            if !requirements_path.exists() {
                return Ok(vec![]);
            }

            let content = std::fs::read_to_string(&requirements_path).map_err(|e| {
                PumasError::Io {
                    message: format!("Failed to read requirements.txt: {}", e),
                    path: Some(requirements_path),
                    source: Some(e),
                }
            })?;

            // Parse requirements (simple extraction of package names)
            let packages: Vec<String> = content
                .lines()
                .filter(|line| {
                    let line = line.trim();
                    !line.is_empty() && !line.starts_with('#') && !line.starts_with('-')
                })
                .filter_map(|line| {
                    let name = line
                        .split(|c| c == '=' || c == '>' || c == '<' || c == '[' || c == ';')
                        .next()?
                        .trim();
                    if !name.is_empty() {
                        Some(name.to_string())
                    } else {
                        None
                    }
                })
                .collect();

            Ok(packages)
        } else {
            Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            })
        }
    }

    // ========================================
    // Process Management Methods
    // ========================================

    /// Check if ComfyUI is currently running.
    pub async fn is_comfyui_running(&self) -> bool {
        let mgr_lock = self.process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_running()
        } else {
            false
        }
    }

    /// Get running processes with resource information.
    pub async fn get_running_processes(&self) -> Vec<process::ProcessInfo> {
        let mgr_lock = self.process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.get_processes_with_resources()
        } else {
            vec![]
        }
    }

    /// Stop all running ComfyUI processes.
    pub async fn stop_comfyui(&self) -> Result<bool> {
        let mgr_lock = self.process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.stop_all()
        } else {
            Ok(false)
        }
    }

    /// Launch a specific version.
    pub async fn launch_version(&self, tag: &str, _app_id: Option<AppId>) -> Result<models::LaunchResponse> {
        let version_mgr_lock = self.version_manager.read().await;
        let proc_mgr_lock = self.process_manager.read().await;

        let version_dir = if let Some(ref vm) = *version_mgr_lock {
            let path = vm.version_path(tag);
            if !path.exists() {
                return Ok(models::LaunchResponse {
                    success: false,
                    error: Some(format!("Version {} not installed", tag)),
                    log_path: None,
                    ready: None,
                });
            }
            path
        } else {
            return Ok(models::LaunchResponse {
                success: false,
                error: Some("Version manager not initialized".to_string()),
                log_path: None,
                ready: None,
            });
        };

        if let Some(ref pm) = *proc_mgr_lock {
            let log_dir = self.launcher_data_dir().join("logs");
            let result = pm.launch_version(tag, &version_dir, Some(&log_dir));

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

    /// Get the last launch log path.
    pub async fn get_last_launch_log(&self) -> Option<String> {
        let mgr_lock = self.process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.last_launch_log().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        }
    }

    /// Get the last launch error.
    pub async fn get_last_launch_error(&self) -> Option<String> {
        let mgr_lock = self.process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.last_launch_error()
        } else {
            None
        }
    }

    // ========================================
    // Shortcut Management Methods
    // ========================================

    /// Get shortcut state for a version.
    pub async fn get_version_shortcut_state(&self, tag: &str) -> models::ShortcutState {
        let mgr_lock = self.shortcut_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            let state = mgr.get_version_shortcut_state(tag);
            models::ShortcutState {
                tag: state.tag,
                menu: state.menu,
                desktop: state.desktop,
            }
        } else {
            models::ShortcutState {
                tag: tag.to_string(),
                menu: false,
                desktop: false,
            }
        }
    }

    /// Create shortcuts for a version.
    pub async fn create_version_shortcuts(
        &self,
        tag: &str,
        create_menu: bool,
        create_desktop: bool,
    ) -> Result<models::ShortcutState> {
        let version_mgr_lock = self.version_manager.read().await;
        let shortcut_mgr_lock = self.shortcut_manager.read().await;

        let version_dir = if let Some(ref vm) = *version_mgr_lock {
            let path = vm.version_path(tag);
            if !path.exists() {
                return Err(PumasError::NotFound {
                    resource: format!("Version: {}", tag),
                });
            }
            path
        } else {
            return Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            });
        };

        if let Some(ref sm) = *shortcut_mgr_lock {
            let result = sm.create_version_shortcuts(tag, &version_dir, create_menu, create_desktop)?;
            Ok(models::ShortcutState {
                tag: result.state.tag,
                menu: result.state.menu,
                desktop: result.state.desktop,
            })
        } else {
            Err(PumasError::Config {
                message: "Shortcut manager not initialized".to_string(),
            })
        }
    }

    /// Remove shortcuts for a version.
    pub async fn remove_version_shortcuts(
        &self,
        tag: &str,
        remove_menu: bool,
        remove_desktop: bool,
    ) -> Result<models::ShortcutState> {
        let mgr_lock = self.shortcut_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            let result = mgr.remove_version_shortcuts(tag, remove_menu, remove_desktop)?;
            Ok(models::ShortcutState {
                tag: result.state.tag,
                menu: result.state.menu,
                desktop: result.state.desktop,
            })
        } else {
            Err(PumasError::Config {
                message: "Shortcut manager not initialized".to_string(),
            })
        }
    }

    /// Toggle menu shortcut for a version.
    pub async fn toggle_menu_shortcut(&self, tag: &str) -> Result<bool> {
        let version_mgr_lock = self.version_manager.read().await;
        let shortcut_mgr_lock = self.shortcut_manager.read().await;

        let version_dir = if let Some(ref vm) = *version_mgr_lock {
            let path = vm.version_path(tag);
            if !path.exists() {
                return Err(PumasError::NotFound {
                    resource: format!("Version: {}", tag),
                });
            }
            path
        } else {
            return Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            });
        };

        if let Some(ref sm) = *shortcut_mgr_lock {
            let result = sm.toggle_menu_shortcut(tag, &version_dir)?;
            Ok(result.success)
        } else {
            Err(PumasError::Config {
                message: "Shortcut manager not initialized".to_string(),
            })
        }
    }

    /// Toggle desktop shortcut for a version.
    pub async fn toggle_desktop_shortcut(&self, tag: &str) -> Result<bool> {
        let version_mgr_lock = self.version_manager.read().await;
        let shortcut_mgr_lock = self.shortcut_manager.read().await;

        let version_dir = if let Some(ref vm) = *version_mgr_lock {
            let path = vm.version_path(tag);
            if !path.exists() {
                return Err(PumasError::NotFound {
                    resource: format!("Version: {}", tag),
                });
            }
            path
        } else {
            return Err(PumasError::Config {
                message: "Version manager not initialized".to_string(),
            });
        };

        if let Some(ref sm) = *shortcut_mgr_lock {
            let result = sm.toggle_desktop_shortcut(tag, &version_dir)?;
            Ok(result.success)
        } else {
            Err(PumasError::Config {
                message: "Shortcut manager not initialized".to_string(),
            })
        }
    }

    // ========================================
    // System Utility Methods
    // ========================================

    /// Open a path in the file manager.
    pub fn open_path(&self, path: &str) -> Result<()> {
        self.system_utils.open_path(path)
    }

    /// Open a URL in the default browser.
    pub fn open_url(&self, url: &str) -> Result<()> {
        self.system_utils.open_url(url)
    }

    /// Open the active version's installation directory.
    pub async fn open_active_install(&self) -> Result<()> {
        let mgr_lock = self.version_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            if let Some(tag) = mgr.get_active_version().await? {
                let path = mgr.version_path(&tag);
                if path.exists() {
                    return self.system_utils.open_path(&path.to_string_lossy());
                }
            }
        }
        Err(PumasError::NotFound {
            resource: "Active version".to_string(),
        })
    }

    // ========================================
    // Background fetch tracking
    // ========================================

    /// Check if background fetch has completed.
    pub async fn has_background_fetch_completed(&self) -> bool {
        self._state.read().await.background_fetch_completed
    }

    /// Reset the background fetch flag.
    pub async fn reset_background_fetch_flag(&self) {
        self._state.write().await.background_fetch_completed = false;
    }

    // ========================================
    // Custom Nodes Management
    // ========================================

    /// List all custom nodes for a specific version.
    pub fn list_custom_nodes(&self, tag: &str) -> Result<Vec<custom_nodes::types::InstalledCustomNode>> {
        self.custom_nodes_manager.list_custom_nodes(tag)
    }

    /// Install a custom node from a git URL.
    pub async fn install_custom_node(&self, git_url: &str, tag: &str) -> Result<custom_nodes::types::InstallResult> {
        self.custom_nodes_manager.install_from_git(git_url, tag).await
    }

    /// Update a custom node to the latest version.
    pub async fn update_custom_node(&self, node_name: &str, tag: &str) -> Result<custom_nodes::types::UpdateResult> {
        self.custom_nodes_manager.update(node_name, tag).await
    }

    /// Remove a custom node from a specific version.
    pub fn remove_custom_node(&self, node_name: &str, tag: &str) -> Result<bool> {
        self.custom_nodes_manager.remove(node_name, tag)
    }

    // ========================================
    // Size Calculator
    // ========================================

    /// Calculate size for a release.
    pub async fn calculate_release_size(
        &self,
        tag: &str,
        archive_size: u64,
        requirements: Option<&[String]>,
    ) -> Result<version_manager::ReleaseSize> {
        let mut calc = self.size_calculator.write().await;
        calc.calculate_release_size(tag, archive_size, requirements).await
    }

    /// Get cached size for a release.
    pub async fn get_cached_release_size(&self, tag: &str) -> Option<version_manager::ReleaseSize> {
        let calc = self.size_calculator.read().await;
        calc.get_cached_size(tag).cloned()
    }

    /// Get detailed size breakdown for a release.
    pub async fn get_release_size_breakdown(&self, tag: &str) -> Option<version_manager::SizeBreakdown> {
        let calc = self.size_calculator.read().await;
        calc.get_size_breakdown(tag)
    }

    // ========================================
    // Model Library Methods
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<ModelRecord>> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref lib) = *lib_lock {
            lib.list_models().await
        } else {
            Ok(vec![])
        }
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResult> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref lib) = *lib_lock {
            lib.search_models(query, limit, offset).await
        } else {
            Ok(SearchResult {
                models: vec![],
                total_count: 0,
                query_time_ms: 0.0,
                query: query.to_string(),
            })
        }
    }

    /// Rebuild the model index from metadata files.
    pub async fn rebuild_model_index(&self) -> Result<usize> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref lib) = *lib_lock {
            lib.rebuild_index().await
        } else {
            Ok(0)
        }
    }

    /// Get a single model by ID.
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref lib) = *lib_lock {
            lib.get_model(model_id).await
        } else {
            Ok(None)
        }
    }

    /// Mark a model's metadata as manually set (protected from auto-updates).
    pub async fn mark_model_metadata_as_manual(&self, model_id: &str) -> Result<()> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref lib) = *lib_lock {
            lib.mark_metadata_as_manual(model_id).await
        } else {
            Err(PumasError::Config {
                message: "Model library not initialized".to_string(),
            })
        }
    }

    /// Import a model from a local path.
    pub async fn import_model(
        &self,
        spec: &model_library::ModelImportSpec,
    ) -> Result<model_library::ModelImportResult> {
        let importer_lock = self.model_importer.read().await;
        if let Some(ref importer) = *importer_lock {
            importer.import(spec).await
        } else {
            Ok(model_library::ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_path: None,
                error: Some("Model importer not initialized".to_string()),
                security_tier: None,
            })
        }
    }

    /// Import multiple models in batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<model_library::ModelImportSpec>,
    ) -> Vec<model_library::ModelImportResult> {
        let importer_lock = self.model_importer.read().await;
        if let Some(ref importer) = *importer_lock {
            importer.batch_import(specs, None).await
        } else {
            specs
                .into_iter()
                .map(|spec| model_library::ModelImportResult {
                    path: spec.path.clone(),
                    success: false,
                    model_path: None,
                    error: Some("Model importer not initialized".to_string()),
                    security_tier: None,
                })
                .collect()
        }
    }

    /// Search for models on HuggingFace.
    pub async fn search_hf_models(
        &self,
        query: &str,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<models::HuggingFaceModel>> {
        let hf_lock = self.hf_client.read().await;
        if let Some(ref client) = *hf_lock {
            let params = model_library::HfSearchParams {
                query: query.to_string(),
                kind: kind.map(String::from),
                limit: Some(limit),
                ..Default::default()
            };
            client.search(&params).await
        } else {
            Ok(vec![])
        }
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(
        &self,
        request: &model_library::DownloadRequest,
    ) -> Result<String> {
        let hf_lock = self.hf_client.read().await;
        let lib_lock = self.model_library.read().await;

        if let (Some(ref client), Some(ref lib)) = (&*hf_lock, &*lib_lock) {
            // Determine destination directory
            let model_type = request.model_type.as_deref().unwrap_or("unknown");
            let dest_dir = lib.build_model_path(
                model_type,
                &request.family,
                &model_library::normalize_name(&request.official_name),
            );
            client.start_download(request, &dest_dir).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client or model library not initialized".to_string(),
            })
        }
    }

    /// Get download progress for a HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: &str,
    ) -> Option<models::ModelDownloadProgress> {
        let hf_lock = self.hf_client.read().await;
        if let Some(ref client) = *hf_lock {
            client.get_download_progress(download_id).await
        } else {
            None
        }
    }

    /// Cancel a HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: &str) -> Result<bool> {
        let hf_lock = self.hf_client.read().await;
        if let Some(ref client) = *hf_lock {
            client.cancel_download(download_id).await
        } else {
            Ok(false)
        }
    }

    /// Look up HuggingFace metadata for a local file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: &str,
    ) -> Result<Option<model_library::HfMetadataResult>> {
        let hf_lock = self.hf_client.read().await;
        if let Some(ref client) = *hf_lock {
            let path = std::path::Path::new(file_path);
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path);
            client.lookup_metadata(filename, Some(path), None).await
        } else {
            Ok(None)
        }
    }

    /// Get repository file tree from HuggingFace.
    pub async fn get_hf_repo_files(
        &self,
        repo_id: &str,
    ) -> Result<model_library::RepoFileTree> {
        let hf_lock = self.hf_client.read().await;
        if let Some(ref client) = *hf_lock {
            client.get_repo_files(repo_id).await
        } else {
            Err(PumasError::Config {
                message: "HuggingFace client not initialized".to_string(),
            })
        }
    }

    // ========================================
    // Link Management
    // ========================================

    /// Get the health status of model links for a version.
    ///
    /// Returns information about total links, healthy links, broken links, etc.
    pub async fn get_link_health(&self, _version_tag: Option<&str>) -> Result<models::LinkHealthResponse> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref library) = *lib_lock {
            let registry = library.link_registry().read().await;
            let all_links = registry.get_all().await;

            let mut healthy = 0;
            let mut broken: Vec<String> = Vec::new();

            for link in &all_links {
                // Check if symlink target exists
                if link.target.is_symlink() {
                    if link.source.exists() {
                        healthy += 1;
                    } else {
                        broken.push(link.target.to_string_lossy().to_string());
                    }
                } else if link.target.exists() {
                    // Hardlink or copy - just check if target exists
                    healthy += 1;
                } else {
                    broken.push(link.target.to_string_lossy().to_string());
                }
            }

            Ok(models::LinkHealthResponse {
                success: true,
                error: None,
                status: if broken.is_empty() { "healthy".to_string() } else { "degraded".to_string() },
                total_links: all_links.len(),
                healthy_links: healthy,
                broken_links: broken,
                orphaned_links: vec![],
                warnings: vec![],
                errors: vec![],
            })
        } else {
            Ok(models::LinkHealthResponse {
                success: true,
                error: None,
                status: "unknown".to_string(),
                total_links: 0,
                healthy_links: 0,
                broken_links: vec![],
                orphaned_links: vec![],
                warnings: vec!["Model library not initialized".to_string()],
                errors: vec![],
            })
        }
    }

    /// Clean up broken model links.
    ///
    /// Returns the number of broken links that were removed.
    pub async fn clean_broken_links(&self) -> Result<models::CleanBrokenLinksResponse> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref library) = *lib_lock {
            let registry = library.link_registry().write().await;
            let broken = registry.cleanup_broken().await?;

            // Also remove the actual broken symlinks from the filesystem
            for entry in &broken {
                if entry.target.exists() || entry.target.is_symlink() {
                    let _ = std::fs::remove_file(&entry.target);
                }
            }

            Ok(models::CleanBrokenLinksResponse {
                success: true,
                cleaned: broken.len(),
            })
        } else {
            Ok(models::CleanBrokenLinksResponse {
                success: false,
                cleaned: 0,
            })
        }
    }

    /// Get all links for a specific model.
    pub async fn get_links_for_model(&self, model_id: &str) -> Result<models::LinksForModelResponse> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref library) = *lib_lock {
            let registry = library.link_registry().read().await;
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

            Ok(models::LinksForModelResponse {
                success: true,
                links: link_info,
            })
        } else {
            Ok(models::LinksForModelResponse {
                success: false,
                links: vec![],
            })
        }
    }

    /// Delete a model and cascade delete all its links.
    pub async fn delete_model_with_cascade(&self, model_id: &str) -> Result<models::DeleteModelResponse> {
        let lib_lock = self.model_library.read().await;
        if let Some(ref library) = *lib_lock {
            library.delete_model(model_id, true).await?;
            Ok(models::DeleteModelResponse {
                success: true,
                error: None,
            })
        } else {
            Ok(models::DeleteModelResponse {
                success: false,
                error: Some("Model library not initialized".to_string()),
            })
        }
    }

    /// Preview model mapping for a version without applying it.
    pub async fn preview_model_mapping(
        &self,
        version_tag: &str,
    ) -> Result<models::MappingPreviewResponse> {
        let mapper_lock = self.model_mapper.read().await;
        let vm_lock = self.version_manager.read().await;

        if let (Some(ref mapper), Some(ref vm)) = (&*mapper_lock, &*vm_lock) {
            let version_path = vm.version_path(version_tag);
            let models_path = version_path.join("models");

            if !models_path.exists() {
                return Ok(models::MappingPreviewResponse {
                    success: false,
                    error: Some(format!("Version models directory not found: {}", models_path.display())),
                    preview: None,
                });
            }

            let preview = mapper.preview_mapping("comfyui", Some(version_tag), &models_path).await?;

            Ok(models::MappingPreviewResponse {
                success: true,
                error: None,
                preview: Some(models::MappingPreviewData {
                    creates: preview.creates.len(),
                    skips: preview.skips.len(),
                    conflicts: preview.conflicts.len(),
                    broken: preview.broken.len(),
                }),
            })
        } else {
            Ok(models::MappingPreviewResponse {
                success: false,
                error: Some("Model mapper or version manager not initialized".to_string()),
                preview: None,
            })
        }
    }

    /// Apply model mapping for a version.
    pub async fn apply_model_mapping(
        &self,
        version_tag: &str,
    ) -> Result<models::MappingApplyResponse> {
        let mapper_lock = self.model_mapper.read().await;
        let vm_lock = self.version_manager.read().await;

        if let (Some(ref mapper), Some(ref vm)) = (&*mapper_lock, &*vm_lock) {
            let version_path = vm.version_path(version_tag);
            let models_path = version_path.join("models");

            if !models_path.exists() {
                std::fs::create_dir_all(&models_path)?;
            }

            let result = mapper.apply_mapping("comfyui", Some(version_tag), &models_path).await?;

            Ok(models::MappingApplyResponse {
                success: true,
                error: None,
                created: result.created,
                updated: 0,
                errors: result.errors.iter().map(|(p, e)| format!("{}: {}", p.display(), e)).collect(),
            })
        } else {
            Ok(models::MappingApplyResponse {
                success: false,
                error: Some("Model mapper or version manager not initialized".to_string()),
                created: 0,
                updated: 0,
                errors: vec![],
            })
        }
    }

    /// Perform incremental sync of models for a version.
    pub async fn sync_models_incremental(
        &self,
        version_tag: &str,
    ) -> Result<models::SyncModelsResponse> {
        // Incremental sync is essentially the same as apply_mapping
        // but we could add additional logic here for detecting changes
        let result = self.apply_model_mapping(version_tag).await?;

        Ok(models::SyncModelsResponse {
            success: result.success,
            error: result.error,
            synced: result.created,
            errors: result.errors,
        })
    }

    // ========================================
    // Launcher Updater Methods
    // ========================================

    /// Get launcher version information.
    pub fn get_launcher_version(&self) -> serde_json::Value {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.get_version_info()
    }

    /// Check for launcher updates via GitHub.
    pub async fn check_launcher_updates(&self, force_refresh: bool) -> launcher::UpdateCheckResult {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.check_for_updates(force_refresh).await
    }

    /// Apply launcher update by pulling latest changes and rebuilding.
    pub async fn apply_launcher_update(&self) -> launcher::UpdateApplyResult {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.apply_update().await
    }

    /// Restart the launcher by spawning a new process.
    pub fn restart_launcher(&self) -> Result<bool> {
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.restart_launcher()
    }

    // ========================================
    // Patch Manager Methods
    // ========================================

    /// Check if ComfyUI main.py is patched with setproctitle.
    pub fn is_patched(&self, tag: Option<&str>) -> bool {
        let comfyui_dir = self.launcher_root.join("ComfyUI");
        let main_py = comfyui_dir.join("main.py");
        let versions_dir = Some(self.versions_dir(AppId::ComfyUI));

        let patch_mgr = launcher::PatchManager::new(&comfyui_dir, &main_py, versions_dir);
        patch_mgr.is_patched(tag)
    }

    /// Toggle the setproctitle patch for a ComfyUI version.
    ///
    /// Returns `true` if now patched, `false` if now unpatched.
    pub fn toggle_patch(&self, tag: Option<&str>) -> Result<bool> {
        let comfyui_dir = self.launcher_root.join("ComfyUI");
        let main_py = comfyui_dir.join("main.py");
        let versions_dir = Some(self.versions_dir(AppId::ComfyUI));

        let patch_mgr = launcher::PatchManager::new(&comfyui_dir, &main_py, versions_dir);
        patch_mgr.toggle_patch(tag)
    }

    // ========================================
    // System Check Methods
    // ========================================

    /// Check if git is available on the system.
    pub fn check_git(&self) -> system::SystemCheckResult {
        system::check_git()
    }

    /// Check if Brave browser is available on the system.
    pub fn check_brave(&self) -> system::SystemCheckResult {
        system::check_brave()
    }

    /// Check if setproctitle Python package is available.
    pub fn check_setproctitle(&self) -> system::SystemCheckResult {
        system::check_setproctitle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_api_creation() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        assert_eq!(api.launcher_root(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_api_paths() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        assert!(api.launcher_data_dir().ends_with("launcher-data"));
        assert!(api.metadata_dir().ends_with("metadata"));
        assert!(api.versions_dir(AppId::ComfyUI).ends_with("comfyui-versions"));
    }

    #[tokio::test]
    async fn test_get_status() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let status = api.get_status().await.unwrap();
        assert!(status.success);
    }

    #[tokio::test]
    async fn test_get_disk_space() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let disk = api.get_disk_space().await.unwrap();
        assert!(disk.success);
        assert!(disk.total > 0);
    }
}
