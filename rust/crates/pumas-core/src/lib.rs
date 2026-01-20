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
pub mod error;
pub mod index;
pub mod metadata;
pub mod models;
pub mod network;
pub mod version_manager;

// Re-export commonly used types
pub use config::AppId;
pub use error::{PumasError, Result};
pub use index::{ModelIndex, ModelRecord, SearchResult};
pub use metadata::MetadataManager;
pub use version_manager::VersionManager;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use version_manager::VersionManager as VersionMgr;

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
    version_manager: Arc<RwLock<Option<VersionMgr>>>,
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
        let version_manager = match VersionMgr::new(&launcher_root, AppId::ComfyUI).await {
            Ok(mgr) => Arc::new(RwLock::new(Some(mgr))),
            Err(e) => {
                tracing::warn!("Failed to initialize version manager: {}", e);
                Arc::new(RwLock::new(None))
            }
        };

        Ok(Self {
            launcher_root,
            _state: state,
            version_manager,
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
        // TODO: Implement actual status gathering
        Ok(models::StatusResponse {
            success: true,
            error: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            deps_ready: true,
            patched: false,
            menu_shortcut: false,
            desktop_shortcut: false,
            shortcut_version: None,
            message: "Rust backend running".to_string(),
            comfyui_running: false,
            last_launch_error: None,
            last_launch_log: None,
            app_resources: None,
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
    pub async fn get_version_manager(&self, _app_id: Option<AppId>) -> Result<Arc<RwLock<Option<VersionMgr>>>> {
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
