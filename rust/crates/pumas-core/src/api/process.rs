//! Process management methods on PumasApi.

use crate::error::Result;
use crate::models;
use crate::process;
use crate::PumasApi;

impl PumasApi {
    // ========================================
    // Process Management Methods
    // ========================================

    /// Check if ComfyUI is currently running.
    pub async fn is_comfyui_running(&self) -> bool {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_running()
        } else {
            false
        }
    }

    /// Get running processes with resource information.
    pub async fn get_running_processes(&self) -> Vec<process::ProcessInfo> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.get_processes_with_resources()
        } else {
            vec![]
        }
    }

    /// Update the version paths for process detection.
    ///
    /// This should be called by the RPC layer after obtaining version information
    /// from the VersionManager. Without this, PID file detection will only check
    /// the root-level PID file and may miss version-specific PID files.
    pub async fn set_process_version_paths(
        &self,
        version_paths: std::collections::HashMap<String, std::path::PathBuf>,
    ) {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.set_version_paths(version_paths);
        } else {
            tracing::warn!("PumasApi.set_process_version_paths: process manager not initialized");
        }
    }

    /// Stop all running ComfyUI processes.
    pub async fn stop_comfyui(&self) -> Result<bool> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.stop_all()
        } else {
            Ok(false)
        }
    }

    /// Check if Ollama is currently running.
    pub async fn is_ollama_running(&self) -> bool {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_ollama_running()
        } else {
            false
        }
    }

    /// Stop Ollama processes.
    pub async fn stop_ollama(&self) -> Result<bool> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.stop_ollama()
        } else {
            Ok(false)
        }
    }

    /// Launch an Ollama version from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_ollama(
        &self,
        tag: &str,
        version_dir: &std::path::Path,
    ) -> Result<models::LaunchResponse> {
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

        let proc_mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref pm) = *proc_mgr_lock {
            let log_dir = self.launcher_data_dir().join("logs");
            let result = pm.launch_ollama(tag, version_dir, Some(&log_dir));

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

    /// Check if the Torch inference server is currently running.
    pub async fn is_torch_running(&self) -> bool {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_torch_running()
        } else {
            false
        }
    }

    /// Stop the Torch inference server.
    pub async fn stop_torch(&self) -> Result<bool> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.stop_torch()
        } else {
            Ok(false)
        }
    }

    /// Launch the Torch inference server from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_torch(
        &self,
        tag: &str,
        version_dir: &std::path::Path,
    ) -> Result<models::LaunchResponse> {
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

        let proc_mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref pm) = *proc_mgr_lock {
            let log_dir = self.launcher_data_dir().join("logs");
            let result = pm.launch_torch(tag, version_dir, Some(&log_dir));

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

    /// Launch a specific version from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_version(
        &self,
        tag: &str,
        version_dir: &std::path::Path,
    ) -> Result<models::LaunchResponse> {
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

        let proc_mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref pm) = *proc_mgr_lock {
            let log_dir = self.launcher_data_dir().join("logs");
            let result = pm.launch_version(tag, version_dir, Some(&log_dir));

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
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.last_launch_log()
                .map(|p| p.to_string_lossy().to_string())
        } else {
            None
        }
    }

    /// Get the last launch error.
    pub async fn get_last_launch_error(&self) -> Option<String> {
        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.last_launch_error()
        } else {
            None
        }
    }
}
