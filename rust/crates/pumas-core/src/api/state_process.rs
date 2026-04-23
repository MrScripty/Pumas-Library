//! Process lifecycle helpers used by primary-state IPC dispatch.

use super::state::{launcher_root_from_primary, PrimaryState};
use crate::error::PumasError;
use crate::{models, process};
use std::path::{Path, PathBuf};
use tokio::fs;

async fn path_exists(path: &Path) -> std::result::Result<bool, PumasError> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

pub(super) async fn is_comfyui_running(primary: &PrimaryState) -> bool {
    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(mgr) = process_manager {
        tokio::task::spawn_blocking(move || mgr.is_running())
            .await
            .unwrap_or(false)
    } else {
        false
    }
}

pub(super) async fn get_running_processes(primary: &PrimaryState) -> Vec<process::ProcessInfo> {
    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(mgr) = process_manager {
        tokio::task::spawn_blocking(move || mgr.get_processes_with_resources())
            .await
            .unwrap_or_default()
    } else {
        vec![]
    }
}

pub(super) async fn set_process_version_paths(
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

pub(super) async fn stop_comfyui(primary: &PrimaryState) -> std::result::Result<bool, PumasError> {
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

pub(super) async fn is_ollama_running(primary: &PrimaryState) -> bool {
    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(mgr) = process_manager {
        tokio::task::spawn_blocking(move || mgr.is_ollama_running())
            .await
            .unwrap_or(false)
    } else {
        false
    }
}

pub(super) async fn stop_ollama(primary: &PrimaryState) -> std::result::Result<bool, PumasError> {
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

pub(super) async fn launch_ollama(
    primary: &PrimaryState,
    tag: &str,
    version_dir: &Path,
) -> std::result::Result<models::LaunchResponse, PumasError> {
    if !path_exists(version_dir).await? {
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

pub(super) async fn is_torch_running(primary: &PrimaryState) -> bool {
    let process_manager = {
        let mgr_lock = primary.process_manager.read().await;
        mgr_lock.clone()
    };

    if let Some(mgr) = process_manager {
        tokio::task::spawn_blocking(move || mgr.is_torch_running())
            .await
            .unwrap_or(false)
    } else {
        false
    }
}

pub(super) async fn stop_torch(primary: &PrimaryState) -> std::result::Result<bool, PumasError> {
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

pub(super) async fn launch_torch(
    primary: &PrimaryState,
    tag: &str,
    version_dir: &Path,
) -> std::result::Result<models::LaunchResponse, PumasError> {
    if !path_exists(version_dir).await? {
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

pub(super) async fn launch_version(
    primary: &PrimaryState,
    tag: &str,
    version_dir: &Path,
) -> std::result::Result<models::LaunchResponse, PumasError> {
    if !path_exists(version_dir).await? {
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

pub(super) async fn get_last_launch_log(primary: &PrimaryState) -> Option<String> {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.last_launch_log()
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    }
}

pub(super) async fn get_last_launch_error(primary: &PrimaryState) -> Option<String> {
    let mgr_lock = primary.process_manager.read().await;
    if let Some(ref mgr) = *mgr_lock {
        mgr.last_launch_error()
    } else {
        None
    }
}
