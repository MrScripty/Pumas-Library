//! System utility methods on PumasApi.

use crate::error::{PumasError, Result};
use crate::launcher;
use crate::models;
use crate::system;
use crate::PumasApi;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs;

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| crate::error::PumasError::io_with_path(err, path))
}

async fn validate_existing_local_open_path(path: &str) -> Result<std::path::PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(PumasError::InvalidParams {
            message: "path is required".to_string(),
        });
    }

    let candidate = std::path::PathBuf::from(trimmed);
    fs::canonicalize(&candidate)
        .await
        .map_err(|source| match source.kind() {
            ErrorKind::NotFound => PumasError::NotFound {
                resource: format!("Path: {}", candidate.display()),
            },
            _ => PumasError::io_with_path(source, &candidate),
        })
}

impl PumasApi {
    // ========================================
    // Status & System Methods
    // ========================================

    /// Get overall system status.
    ///
    /// Runtime version state is obtained through pumas-app-manager in the RPC layer.
    pub async fn get_status(&self) -> Result<models::StatusResponse> {
        self.try_primary()?;
        // Get actual running status
        let ollama_running = self.is_ollama_running().await;
        let torch_running = self.is_torch_running().await;
        let last_launch_error = self.get_last_launch_error().await;
        let last_launch_log = self.get_last_launch_log().await;

        // Get app resources for running apps
        let process_manager = {
            let mgr_lock = self.primary().process_manager.read().await;
            mgr_lock.clone()
        };
        let app_resources = if let Some(mgr) = process_manager {
            tokio::task::spawn_blocking(move || {
                let ollama_resources = if ollama_running {
                    mgr.aggregate_ollama_resources()
                        .map(|r| models::AppResourceUsage {
                            gpu_memory: Some((r.gpu_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                            ram_memory: Some((r.ram_memory * 1024.0 * 1024.0 * 1024.0) as u64),
                        })
                } else {
                    None
                };

                if ollama_resources.is_some() {
                    Some(models::AppResources {
                        ollama: ollama_resources,
                    })
                } else {
                    None
                }
            })
            .await
            .map_err(|e| PumasError::Other(format!("Failed to join get_status task: {}", e)))?
        } else {
            None
        };

        // Debug: log app_resources before returning
        if let Some(ref res) = app_resources {
            tracing::debug!(
                "get_status: app_resources = ollama={:?}",
                res.ollama.as_ref().map(|r| (r.ram_memory, r.gpu_memory))
            );
        } else {
            tracing::debug!("get_status: app_resources = None");
        }

        Ok(models::StatusResponse {
            success: true,
            error: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            message: if ollama_running {
                "Ollama running".to_string()
            } else if torch_running {
                "Torch running".to_string()
            } else {
                "Ready".to_string()
            },
            ollama_running,
            torch_running,
            last_launch_error,
            last_launch_log,
            app_resources,
        })
    }

    /// Get disk space information.
    pub async fn get_disk_space(&self) -> Result<models::DiskSpaceResponse> {
        self.try_primary()?;
        let launcher_root = self.launcher_root.clone();
        tokio::task::spawn_blocking(move || {
            use sysinfo::Disks;

            let disks = Disks::new_with_refreshed_list();

            // Find the disk containing the launcher root
            let launcher_root_str = launcher_root.to_string_lossy();

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
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join get_disk_space task: {}", e)))?
    }

    /// Get system resources (CPU, GPU, RAM, disk).
    pub async fn get_system_resources(&self) -> Result<models::SystemResourcesResponse> {
        self.try_primary()?;
        let tracker = self.primary().resource_tracker.clone();
        let snapshot = tokio::task::spawn_blocking(move || tracker.get_system_resources())
            .await
            .map_err(|e| {
                PumasError::Other(format!("Failed to join get_system_resources task: {}", e))
            })??;

        Ok(super::resource_responses::system_resources_response_from_snapshot(snapshot))
    }

    // ========================================
    // System Utility Methods
    // ========================================

    /// Open a path in the file manager.
    pub async fn open_path(&self, path: &str) -> Result<()> {
        self.try_primary()?;
        let system_utils = self.primary().system_utils.clone();
        let path = validate_existing_local_open_path(path).await?;
        let path = path.to_string_lossy().to_string();
        tokio::task::spawn_blocking(move || system_utils.open_path(&path))
            .await
            .map_err(|e| PumasError::Other(format!("Failed to join open_path task: {}", e)))?
    }

    /// Open a URL in the default browser.
    pub async fn open_url(&self, url: &str) -> Result<()> {
        self.try_primary()?;
        let system_utils = self.primary().system_utils.clone();
        let url = url.to_string();
        tokio::task::spawn_blocking(move || system_utils.open_url(&url))
            .await
            .map_err(|e| PumasError::Other(format!("Failed to join open_url task: {}", e)))?
    }

    /// Open a directory in the file manager.
    ///
    /// The caller (RPC layer) can use this with a version directory path
    /// obtained from pumas-app-manager's VersionManager.
    pub async fn open_directory(&self, dir: &std::path::Path) -> Result<()> {
        self.try_primary()?;
        if !path_exists(dir).await? {
            return Err(PumasError::NotFound {
                resource: format!("Directory: {}", dir.display()),
            });
        }

        let system_utils = self.primary().system_utils.clone();
        let dir = dir.to_path_buf();
        tokio::task::spawn_blocking(move || system_utils.open_path(&dir.to_string_lossy()))
            .await
            .map_err(|e| PumasError::Other(format!("Failed to join open_directory task: {}", e)))?
    }

    // ========================================
    // Background fetch tracking
    // ========================================

    /// Check if background fetch has completed.
    pub async fn has_background_fetch_completed(&self) -> bool {
        let _ = self.primary();
        self.primary()
            ._state
            .read()
            .await
            .background_fetch_completed
    }

    /// Reset the background fetch flag.
    pub async fn reset_background_fetch_flag(&self) {
        let _ = self.primary();
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
    pub async fn get_launcher_version(&self) -> serde_json::Value {
        let _ = self.primary();
        let launcher_root = self.launcher_root.clone();
        match tokio::task::spawn_blocking(move || {
            let updater = launcher::LauncherUpdater::new(&launcher_root);
            updater.get_version_info()
        })
        .await
        {
            Ok(value) => value,
            Err(error) => serde_json::json!({
                "success": false,
                "error": format!("Failed to join get_launcher_version task: {}", error),
            }),
        }
    }

    /// Check for launcher updates via GitHub.
    pub async fn check_launcher_updates(&self, force_refresh: bool) -> launcher::UpdateCheckResult {
        let _ = self.primary();
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.check_for_updates(force_refresh).await
    }

    /// Apply launcher update by pulling latest changes and rebuilding.
    pub async fn apply_launcher_update(&self) -> launcher::UpdateApplyResult {
        let _ = self.primary();
        let updater = launcher::LauncherUpdater::new(&self.launcher_root);
        updater.apply_update().await
    }

    /// Restart the launcher by spawning a new process.
    pub async fn restart_launcher(&self) -> Result<bool> {
        self.try_primary()?;
        let launcher_root = self.launcher_root.clone();
        tokio::task::spawn_blocking(move || {
            let updater = launcher::LauncherUpdater::new(&launcher_root);
            updater.restart_launcher()
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join restart_launcher task: {}", e)))?
    }

    // ========================================
    // System Check Methods
    // ========================================

    /// Check if git is available on the system.
    pub async fn check_git(&self) -> system::SystemCheckResult {
        let _ = self.primary();
        tokio::task::spawn_blocking(system::check_git)
            .await
            .unwrap_or_else(|_| system::SystemCheckResult {
                available: false,
                path: None,
                info: Some("Failed to join check_git task".to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::validate_existing_local_open_path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn validate_existing_local_open_path_canonicalizes_existing_directory() {
        let temp_dir = TempDir::new().unwrap();

        let validated =
            validate_existing_local_open_path(temp_dir.path().to_string_lossy().as_ref())
                .await
                .unwrap();

        assert_eq!(validated, temp_dir.path().canonicalize().unwrap());
    }
}
