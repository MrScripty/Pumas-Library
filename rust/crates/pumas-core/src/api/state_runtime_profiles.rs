//! Runtime profile lifecycle helpers used by primary-state IPC dispatch.

use super::state::PrimaryState;
use crate::error::PumasError;
use crate::models::{
    LaunchResponse, RuntimeLifecycleState, RuntimeProfileId, RuntimeProfileStatus,
    RuntimeProviderId,
};
use crate::process::{BinaryLaunchConfig, ProcessLauncher};
use std::path::Path;

pub(super) async fn launch_runtime_profile(
    primary: &PrimaryState,
    profile_id: RuntimeProfileId,
    tag: &str,
    version_dir: &Path,
) -> std::result::Result<LaunchResponse, PumasError> {
    let _operation_guard = primary
        .runtime_profile_service
        .begin_profile_operation(profile_id.clone())?;
    let spec = primary
        .runtime_profile_service
        .managed_profile_launch_spec(profile_id.clone())
        .await?;

    if spec.provider != RuntimeProviderId::Ollama {
        return Ok(LaunchResponse {
            success: false,
            error: Some("runtime profile launch currently supports Ollama profiles".to_string()),
            log_path: None,
            ready: Some(false),
        });
    }

    primary
        .runtime_profile_service
        .record_profile_lifecycle_status(RuntimeProfileStatus {
            profile_id: profile_id.clone(),
            state: RuntimeLifecycleState::Starting,
            endpoint_url: Some(spec.endpoint_url.clone()),
            pid: None,
            log_path: Some(spec.log_file.to_string_lossy().to_string()),
            last_error: None,
        })?;

    let tag = tag.to_string();
    let version_dir = version_dir.to_path_buf();
    let launch_spec = spec.clone();
    let launch_result = tokio::task::spawn_blocking(move || {
        let config = BinaryLaunchConfig::ollama(&tag, &version_dir)
            .with_pid_file(&launch_spec.pid_file)
            .with_log_file(&launch_spec.log_file)
            .with_health_check_url(launch_spec.health_check_url.as_str())
            .with_env_vars(launch_spec.env_vars);
        ProcessLauncher::launch_binary(&config)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!("Failed to join runtime profile launch task: {err}"))
    })??;

    let pid = launch_result.process.as_ref().map(std::process::Child::id);
    let error = launch_result.error.clone();
    primary
        .runtime_profile_service
        .record_profile_lifecycle_status(RuntimeProfileStatus {
            profile_id,
            state: if launch_result.success {
                RuntimeLifecycleState::Running
            } else {
                RuntimeLifecycleState::Failed
            },
            endpoint_url: Some(spec.endpoint_url),
            pid,
            log_path: launch_result
                .log_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            last_error: error.clone(),
        })?;

    Ok(LaunchResponse {
        success: launch_result.success,
        error,
        log_path: launch_result
            .log_path
            .map(|path| path.to_string_lossy().to_string()),
        ready: Some(launch_result.ready),
    })
}
