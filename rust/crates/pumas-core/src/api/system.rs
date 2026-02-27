//! System utility methods on PumasApi.

use crate::config::AppId;
use crate::error::{PumasError, Result};
use crate::launcher;
use crate::models;
use crate::system;
use crate::PumasApi;

impl PumasApi {
    // ========================================
    // Status & System Methods
    // ========================================

    /// Get overall system status.
    ///
    /// Note: This returns basic status. Version-specific status (shortcuts, active version)
    /// should be obtained through pumas-app-manager in the RPC layer.
    pub async fn get_status(&self) -> Result<models::StatusResponse> {
        // Get actual running status
        let comfyui_running = self.is_comfyui_running().await;
        let ollama_running = self.is_ollama_running().await;
        let torch_running = self.is_torch_running().await;
        let last_launch_error = self.get_last_launch_error().await;
        let last_launch_log = self.get_last_launch_log().await;

        // Get app resources for running apps
        let app_resources = {
            let mgr_lock = self.primary().process_manager.read().await;
            if let Some(ref mgr) = *mgr_lock {
                let comfyui_resources = if comfyui_running {
                    mgr.aggregate_app_resources()
                        .map(|r| models::AppResourceUsage {
                            // Convert from GB (f32) to bytes (u64) for frontend
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
            }
        };

        // Debug: log app_resources before returning
        if let Some(ref res) = app_resources {
            tracing::debug!(
                "get_status: app_resources = comfyui={:?}, ollama={:?}",
                res.comfyui.as_ref().map(|r| (r.ram_memory, r.gpu_memory)),
                res.ollama.as_ref().map(|r| (r.ram_memory, r.gpu_memory))
            );
        } else {
            tracing::debug!("get_status: app_resources = None");
        }

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
        use sysinfo::{Disks, System};

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

        // GPU - use ResourceTracker's NvidiaSmiMonitor for real GPU stats
        let gpu = if let Some(ref mgr) = *self.primary().process_manager.read().await {
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

    // ========================================
    // System Utility Methods
    // ========================================

    /// Open a path in the file manager.
    pub fn open_path(&self, path: &str) -> Result<()> {
        self.primary().system_utils.open_path(path)
    }

    /// Open a URL in the default browser.
    pub fn open_url(&self, url: &str) -> Result<()> {
        self.primary().system_utils.open_url(url)
    }

    /// Open a directory in the file manager.
    ///
    /// The caller (RPC layer) can use this with a version directory path
    /// obtained from pumas-app-manager's VersionManager.
    pub fn open_directory(&self, dir: &std::path::Path) -> Result<()> {
        if !dir.exists() {
            return Err(PumasError::NotFound {
                resource: format!("Directory: {}", dir.display()),
            });
        }
        self.primary()
            .system_utils
            .open_path(&dir.to_string_lossy())
    }

    // ========================================
    // Background fetch tracking
    // ========================================

    /// Check if background fetch has completed.
    pub async fn has_background_fetch_completed(&self) -> bool {
        self.primary()
            ._state
            .read()
            .await
            .background_fetch_completed
    }

    /// Reset the background fetch flag.
    pub async fn reset_background_fetch_flag(&self) {
        self.primary()
            ._state
            .write()
            .await
            .background_fetch_completed = false;
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
