//! Process management methods on PumasApi.

use crate::PumasApi;
use crate::error::{PumasError, Result};
use crate::models;
use crate::process;

impl PumasApi {
    // ========================================
    // Process Management Methods
    // ========================================

    /// Check if ComfyUI is currently running.
    pub async fn is_comfyui_running(&self) -> bool {
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default("is_comfyui_running", serde_json::json!({}))
                .await;
        }

        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_running()
        } else {
            false
        }
    }

    /// Get running processes with resource information.
    pub async fn get_running_processes(&self) -> Vec<process::ProcessInfo> {
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default("get_running_processes", serde_json::json!({}))
                .await;
        }

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
        if self.try_client().is_some() {
            let result: Result<serde_json::Value> = self
                .call_client_method(
                    "set_process_version_paths",
                    serde_json::json!({ "version_paths": version_paths }),
                )
                .await;
            if let Err(err) = result {
                tracing::warn!(
                    "Failed to proxy set_process_version_paths over IPC: {}",
                    err
                );
            }
            return;
        }

        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.set_version_paths(version_paths);
        } else {
            tracing::warn!("PumasApi.set_process_version_paths: process manager not initialized");
        }
    }

    /// Stop all running ComfyUI processes.
    pub async fn stop_comfyui(&self) -> Result<bool> {
        if self.try_client().is_some() {
            return self
                .call_client_method("stop_comfyui", serde_json::json!({}))
                .await;
        }

        let process_manager = {
            let mgr_lock = self.primary().process_manager.read().await;
            mgr_lock.clone()
        };

        if let Some(mgr) = process_manager {
            tokio::task::spawn_blocking(move || mgr.stop_all())
                .await
                .map_err(|e| {
                    PumasError::Other(format!("Failed to join stop_comfyui task: {}", e))
                })?
        } else {
            Ok(false)
        }
    }

    /// Check if Ollama is currently running.
    pub async fn is_ollama_running(&self) -> bool {
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default("is_ollama_running", serde_json::json!({}))
                .await;
        }

        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_ollama_running()
        } else {
            false
        }
    }

    /// Stop Ollama processes.
    pub async fn stop_ollama(&self) -> Result<bool> {
        if self.try_client().is_some() {
            return self
                .call_client_method("stop_ollama", serde_json::json!({}))
                .await;
        }

        let process_manager = {
            let mgr_lock = self.primary().process_manager.read().await;
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

    /// Launch an Ollama version from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_ollama(
        &self,
        tag: &str,
        version_dir: &std::path::Path,
    ) -> Result<models::LaunchResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "launch_ollama",
                    serde_json::json!({
                        "tag": tag,
                        "version_dir": version_dir,
                    }),
                )
                .await;
        }

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
            let mgr_lock = self.primary().process_manager.read().await;
            mgr_lock.clone()
        };

        if let Some(pm) = process_manager {
            let log_dir = self.launcher_data_dir().join("logs");
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

    /// Check if the Torch inference server is currently running.
    pub async fn is_torch_running(&self) -> bool {
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default("is_torch_running", serde_json::json!({}))
                .await;
        }

        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.is_torch_running()
        } else {
            false
        }
    }

    /// Stop the Torch inference server.
    pub async fn stop_torch(&self) -> Result<bool> {
        if self.try_client().is_some() {
            return self
                .call_client_method("stop_torch", serde_json::json!({}))
                .await;
        }

        let process_manager = {
            let mgr_lock = self.primary().process_manager.read().await;
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

    /// Launch the Torch inference server from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_torch(
        &self,
        tag: &str,
        version_dir: &std::path::Path,
    ) -> Result<models::LaunchResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "launch_torch",
                    serde_json::json!({
                        "tag": tag,
                        "version_dir": version_dir,
                    }),
                )
                .await;
        }

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
            let mgr_lock = self.primary().process_manager.read().await;
            mgr_lock.clone()
        };

        if let Some(pm) = process_manager {
            let log_dir = self.launcher_data_dir().join("logs");
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

    /// Launch a specific version from a given directory.
    ///
    /// The caller (RPC layer) is responsible for resolving the version tag to a directory
    /// using pumas-app-manager's VersionManager.
    pub async fn launch_version(
        &self,
        tag: &str,
        version_dir: &std::path::Path,
    ) -> Result<models::LaunchResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "launch_version",
                    serde_json::json!({
                        "tag": tag,
                        "version_dir": version_dir,
                    }),
                )
                .await;
        }

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
            let mgr_lock = self.primary().process_manager.read().await;
            mgr_lock.clone()
        };

        if let Some(pm) = process_manager {
            let log_dir = self.launcher_data_dir().join("logs");
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

    /// Get the last launch log path.
    pub async fn get_last_launch_log(&self) -> Option<String> {
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default("get_last_launch_log", serde_json::json!({}))
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method_or_default("get_last_launch_error", serde_json::json!({}))
                .await;
        }

        let mgr_lock = self.primary().process_manager.read().await;
        if let Some(ref mgr) = *mgr_lock {
            mgr.last_launch_error()
        } else {
            None
        }
    }
}
