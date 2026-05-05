//! Runtime profile lifecycle helpers used by primary-state IPC dispatch.

use super::state::PrimaryState;
use crate::error::PumasError;
use crate::models::{
    LaunchResponse, RuntimeLifecycleState, RuntimeProfileId, RuntimeProfileStatus,
    RuntimeProviderId, RuntimeProviderMode,
};
use crate::process::{BinaryLaunchConfig, ProcessLauncher};
use crate::runtime_profiles::{generate_llama_cpp_router_catalog, RuntimeProfileLaunchSpec};
use std::fs;
use std::path::Path;
use tokio::fs as async_fs;

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

    if spec.provider == RuntimeProviderId::LlamaCpp
        && spec.provider_mode != RuntimeProviderMode::LlamaCppRouter
    {
        return Ok(LaunchResponse {
            success: false,
            error: Some(
                "runtime profile launch currently supports Ollama and llama.cpp router profiles"
                    .to_string(),
            ),
            log_path: None,
            ready: Some(false),
        });
    }

    let spec = prepare_runtime_profile_launch_spec(primary, spec).await?;

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
        let config = runtime_profile_binary_launch_config(&tag, &version_dir, &launch_spec)?;
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

fn runtime_profile_binary_launch_config(
    tag: &str,
    version_dir: &Path,
    launch_spec: &RuntimeProfileLaunchSpec,
) -> std::result::Result<BinaryLaunchConfig, PumasError> {
    let config = match launch_spec.provider {
        RuntimeProviderId::Ollama => BinaryLaunchConfig::ollama(tag, version_dir),
        RuntimeProviderId::LlamaCpp => match launch_spec.provider_mode {
            RuntimeProviderMode::LlamaCppRouter => BinaryLaunchConfig::llama_cpp_router(
                tag,
                version_dir,
                "127.0.0.1",
                launch_spec.port.value(),
                version_dir,
            ),
            RuntimeProviderMode::LlamaCppDedicated => {
                return Err(PumasError::InvalidParams {
                    message: "managed llama.cpp dedicated launch is not implemented yet"
                        .to_string(),
                });
            }
            RuntimeProviderMode::OllamaServe => {
                return Err(PumasError::InvalidParams {
                    message: "llama.cpp runtime profile cannot use ollama_serve mode".to_string(),
                });
            }
        },
    };

    Ok(config
        .with_extra_args(launch_spec.extra_args.clone())
        .with_pid_file(&launch_spec.pid_file)
        .with_log_file(&launch_spec.log_file)
        .with_health_check_url(launch_spec.health_check_url.as_str())
        .with_env_vars(launch_spec.env_vars.clone()))
}

async fn prepare_runtime_profile_launch_spec(
    primary: &PrimaryState,
    mut launch_spec: RuntimeProfileLaunchSpec,
) -> std::result::Result<RuntimeProfileLaunchSpec, PumasError> {
    if launch_spec.provider != RuntimeProviderId::LlamaCpp
        || launch_spec.provider_mode != RuntimeProviderMode::LlamaCppRouter
    {
        return Ok(launch_spec);
    }

    let catalog = generate_llama_cpp_router_catalog(primary.model_library.clone()).await?;
    async_fs::create_dir_all(&launch_spec.runtime_dir)
        .await
        .map_err(|err| PumasError::io_with_path(err, &launch_spec.runtime_dir))?;
    let preset_path = launch_spec.runtime_dir.join("models-preset.ini");
    async_fs::write(&preset_path, catalog.preset_ini)
        .await
        .map_err(|err| PumasError::io_with_path(err, &preset_path))?;
    launch_spec.extra_args =
        replace_llama_cpp_models_dir_with_preset(&launch_spec.extra_args, &preset_path);

    Ok(launch_spec)
}

fn replace_llama_cpp_models_dir_with_preset(args: &[String], preset_path: &Path) -> Vec<String> {
    let preset_path = preset_path.to_string_lossy().to_string();
    let mut output = Vec::with_capacity(args.len() + 2);
    let mut inserted = false;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--models-dir" {
            output.push("--models-preset".to_string());
            output.push(preset_path.clone());
            inserted = true;
            index += 2;
        } else {
            output.push(args[index].clone());
            index += 1;
        }
    }
    if !inserted {
        output.push("--models-preset".to_string());
        output.push(preset_path);
    }
    output
}

pub(super) async fn stop_runtime_profile(
    primary: &PrimaryState,
    profile_id: RuntimeProfileId,
) -> std::result::Result<bool, PumasError> {
    let _operation_guard = primary
        .runtime_profile_service
        .begin_profile_operation(profile_id.clone())?;
    let spec = primary
        .runtime_profile_service
        .managed_profile_launch_spec(profile_id.clone())
        .await?;

    primary
        .runtime_profile_service
        .record_profile_lifecycle_status(RuntimeProfileStatus {
            profile_id: profile_id.clone(),
            state: RuntimeLifecycleState::Stopping,
            endpoint_url: Some(spec.endpoint_url.clone()),
            pid: None,
            log_path: Some(spec.log_file.to_string_lossy().to_string()),
            last_error: None,
        })?;

    let pid_file = spec.pid_file.clone();
    let stop_result = tokio::task::spawn_blocking(move || stop_profile_pid_file(&pid_file))
        .await
        .map_err(|err| {
            PumasError::Other(format!("Failed to join runtime profile stop task: {err}"))
        })?;

    match stop_result {
        Ok(stopped) => {
            primary
                .runtime_profile_service
                .record_profile_lifecycle_status(RuntimeProfileStatus {
                    profile_id,
                    state: RuntimeLifecycleState::Stopped,
                    endpoint_url: Some(spec.endpoint_url),
                    pid: None,
                    log_path: Some(spec.log_file.to_string_lossy().to_string()),
                    last_error: None,
                })?;
            Ok(stopped)
        }
        Err(error) => {
            let error_message = error.to_string();
            primary
                .runtime_profile_service
                .record_profile_lifecycle_status(RuntimeProfileStatus {
                    profile_id,
                    state: RuntimeLifecycleState::Failed,
                    endpoint_url: Some(spec.endpoint_url),
                    pid: None,
                    log_path: Some(spec.log_file.to_string_lossy().to_string()),
                    last_error: Some(error_message),
                })?;
            Err(error)
        }
    }
}

fn stop_profile_pid_file(pid_file: &Path) -> std::result::Result<bool, PumasError> {
    if !pid_file.exists() {
        return Ok(false);
    }

    let pid = fs::read_to_string(pid_file)
        .map_err(|err| PumasError::io_with_path(err, pid_file))?
        .trim()
        .parse::<u32>()
        .map_err(|err| PumasError::InvalidParams {
            message: format!(
                "invalid runtime profile PID file {}: {err}",
                pid_file.display()
            ),
        })?;
    let stopped = ProcessLauncher::stop_process(pid, 5_000)?;
    ProcessLauncher::remove_pid_file(pid_file)?;
    Ok(stopped)
}
