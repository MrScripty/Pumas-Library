//! Runtime profile lifecycle helpers used by primary-state IPC dispatch.

use super::state::PrimaryState;
use crate::error::PumasError;
use crate::models::{
    LaunchResponse, RuntimeDeviceMode, RuntimeDeviceSettings, RuntimeLifecycleState,
    RuntimeProfileId, RuntimeProfileStatus, RuntimeProviderId, RuntimeProviderMode,
};
use crate::process::{BinaryLaunchConfig, ProcessLauncher};
use crate::runtime_profiles::{
    generate_llama_cpp_router_catalog, RuntimeProfileLaunchOverrides, RuntimeProfileLaunchSpec,
};
use std::fs;
use std::path::Path;
use tokio::fs as async_fs;

pub(super) async fn launch_runtime_profile(
    primary: &PrimaryState,
    profile_id: RuntimeProfileId,
    tag: &str,
    version_dir: &Path,
    model_id: Option<String>,
    overrides: Option<RuntimeProfileLaunchOverrides>,
) -> std::result::Result<LaunchResponse, PumasError> {
    let _operation_guard = primary
        .runtime_profile_service
        .begin_profile_operation(profile_id.clone())?;
    let spec = primary
        .runtime_profile_service
        .managed_profile_launch_spec(profile_id.clone())
        .await?;

    let spec =
        prepare_runtime_profile_launch_spec(primary, spec, model_id.as_deref(), overrides.as_ref())
            .await?;

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
            RuntimeProviderMode::LlamaCppDedicated => BinaryLaunchConfig::llama_cpp_dedicated(
                tag,
                version_dir,
                "127.0.0.1",
                launch_spec.port.value(),
                version_dir,
            ),
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
    model_id: Option<&str>,
    overrides: Option<&RuntimeProfileLaunchOverrides>,
) -> std::result::Result<RuntimeProfileLaunchSpec, PumasError> {
    if launch_spec.provider != RuntimeProviderId::LlamaCpp {
        return Ok(launch_spec);
    }

    match launch_spec.provider_mode {
        RuntimeProviderMode::LlamaCppRouter => {
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
            if let Some(overrides) = overrides {
                apply_llama_cpp_launch_overrides(&mut launch_spec, overrides);
            }
        }
        RuntimeProviderMode::LlamaCppDedicated => {
            let Some(model_id) = model_id else {
                return Err(PumasError::InvalidParams {
                    message: "model_id is required to launch a dedicated llama.cpp profile"
                        .to_string(),
                });
            };
            let model_path = primary
                .model_library
                .get_primary_model_file(model_id)
                .ok_or_else(|| PumasError::ModelNotFound {
                    model_id: model_id.to_string(),
                })?;
            if model_path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| !extension.eq_ignore_ascii_case("gguf"))
                .unwrap_or(true)
            {
                return Err(PumasError::InvalidParams {
                    message: format!(
                        "dedicated llama.cpp profiles require a GGUF model file: {}",
                        model_path.display()
                    ),
                });
            }
            launch_spec.extra_args =
                append_llama_cpp_model_arg(&launch_spec.extra_args, &model_path);
            if let Some(overrides) = overrides {
                apply_llama_cpp_launch_overrides(&mut launch_spec, overrides);
            }
        }
        RuntimeProviderMode::OllamaServe => {}
    }

    Ok(launch_spec)
}

fn apply_llama_cpp_launch_overrides(
    launch_spec: &mut RuntimeProfileLaunchSpec,
    overrides: &RuntimeProfileLaunchOverrides,
) {
    if let Some(device) = &overrides.device {
        launch_spec.extra_args = remove_llama_cpp_device_args(&launch_spec.extra_args);
        apply_llama_cpp_device_override_args(&mut launch_spec.extra_args, device);
        apply_device_visibility_override_env(&mut launch_spec.env_vars, device);
    }

    if let Some(context_size) = overrides.context_size {
        launch_spec.extra_args = remove_arg_with_value(&launch_spec.extra_args, "--ctx-size");
        launch_spec
            .extra_args
            .extend(["--ctx-size".to_string(), context_size.to_string()]);
    }
}

fn remove_llama_cpp_device_args(args: &[String]) -> Vec<String> {
    remove_args_with_values(args, &["--n-gpu-layers", "--tensor-split"])
}

fn remove_arg_with_value(args: &[String], flag: &str) -> Vec<String> {
    remove_args_with_values(args, &[flag])
}

fn remove_args_with_values(args: &[String], flags: &[&str]) -> Vec<String> {
    let mut output = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        if flags.contains(&args[index].as_str()) {
            index += 2;
            continue;
        }
        output.push(args[index].clone());
        index += 1;
    }
    output
}

fn apply_llama_cpp_device_override_args(args: &mut Vec<String>, device: &RuntimeDeviceSettings) {
    if let Some(gpu_layers) = llama_cpp_gpu_layers_arg(device) {
        args.extend(["--n-gpu-layers".to_string(), gpu_layers.to_string()]);
    }

    if let Some(tensor_split) = &device.tensor_split {
        if !tensor_split.is_empty() {
            args.extend([
                "--tensor-split".to_string(),
                tensor_split
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ]);
        }
    }
}

fn llama_cpp_gpu_layers_arg(device: &RuntimeDeviceSettings) -> Option<i32> {
    match device.mode {
        RuntimeDeviceMode::Cpu => Some(0),
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::SpecificDevice => {
            Some(device.gpu_layers.unwrap_or(-1))
        }
        RuntimeDeviceMode::Auto | RuntimeDeviceMode::Hybrid => device.gpu_layers,
    }
}

fn apply_device_visibility_override_env(
    env_vars: &mut std::collections::HashMap<String, String>,
    device: &RuntimeDeviceSettings,
) {
    for key in [
        "CUDA_VISIBLE_DEVICES",
        "HIP_VISIBLE_DEVICES",
        "ROCR_VISIBLE_DEVICES",
    ] {
        env_vars.remove(key);
    }

    match device.mode {
        RuntimeDeviceMode::Cpu => {
            env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), String::new());
            env_vars.insert("HIP_VISIBLE_DEVICES".to_string(), String::new());
            env_vars.insert("ROCR_VISIBLE_DEVICES".to_string(), String::new());
        }
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::SpecificDevice => {
            if let Some(device_id) = device.device_id.as_deref() {
                env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), device_id.to_string());
                env_vars.insert("HIP_VISIBLE_DEVICES".to_string(), device_id.to_string());
                env_vars.insert("ROCR_VISIBLE_DEVICES".to_string(), device_id.to_string());
            }
        }
        RuntimeDeviceMode::Auto | RuntimeDeviceMode::Hybrid => {}
    }
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

fn append_llama_cpp_model_arg(args: &[String], model_path: &Path) -> Vec<String> {
    let mut output = args.to_vec();
    output.push("--model".to_string());
    output.push(model_path.to_string_lossy().to_string());
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RuntimeEndpointUrl, RuntimePort};
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn llama_cpp_launch_overrides_replace_profile_device_args() {
        let endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:39191").unwrap();
        let mut launch_spec = RuntimeProfileLaunchSpec {
            profile_id: RuntimeProfileId::parse("llama-dedicated").unwrap(),
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppDedicated,
            endpoint_url: endpoint_url.clone(),
            port: RuntimePort::parse(39191).unwrap(),
            extra_args: vec![
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                "39191".to_string(),
                "--n-gpu-layers".to_string(),
                "16".to_string(),
                "--tensor-split".to_string(),
                "1,1".to_string(),
                "--model".to_string(),
                "/models/base.gguf".to_string(),
            ],
            env_vars: HashMap::from([("CUDA_VISIBLE_DEVICES".to_string(), "0".to_string())]),
            runtime_dir: PathBuf::from("/tmp/runtime"),
            pid_file: PathBuf::from("/tmp/runtime/runtime.pid"),
            log_file: PathBuf::from("/tmp/runtime/runtime.log"),
            health_check_url: endpoint_url,
        };

        apply_llama_cpp_launch_overrides(
            &mut launch_spec,
            &RuntimeProfileLaunchOverrides {
                device: Some(RuntimeDeviceSettings {
                    mode: RuntimeDeviceMode::SpecificDevice,
                    device_id: Some("1".to_string()),
                    gpu_layers: Some(32),
                    tensor_split: Some(vec![3.0, 1.0]),
                }),
                context_size: Some(8192),
            },
        );

        assert!(launch_spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--n-gpu-layers", "32"]));
        assert!(launch_spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--tensor-split", "3,1"]));
        assert!(launch_spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--ctx-size", "8192"]));
        assert_eq!(
            launch_spec
                .env_vars
                .get("CUDA_VISIBLE_DEVICES")
                .map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn llama_cpp_launch_gpu_override_defaults_to_full_offload() {
        let endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:39192").unwrap();
        let mut launch_spec = RuntimeProfileLaunchSpec {
            profile_id: RuntimeProfileId::parse("llama-dedicated").unwrap(),
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppDedicated,
            endpoint_url: endpoint_url.clone(),
            port: RuntimePort::parse(39192).unwrap(),
            extra_args: vec![
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                "39192".to_string(),
                "--model".to_string(),
                "/models/base.gguf".to_string(),
            ],
            env_vars: HashMap::new(),
            runtime_dir: PathBuf::from("/tmp/runtime"),
            pid_file: PathBuf::from("/tmp/runtime/runtime.pid"),
            log_file: PathBuf::from("/tmp/runtime/runtime.log"),
            health_check_url: endpoint_url,
        };

        apply_llama_cpp_launch_overrides(
            &mut launch_spec,
            &RuntimeProfileLaunchOverrides {
                device: Some(RuntimeDeviceSettings {
                    mode: RuntimeDeviceMode::Gpu,
                    device_id: None,
                    gpu_layers: None,
                    tensor_split: None,
                }),
                context_size: None,
            },
        );

        assert!(launch_spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--n-gpu-layers", "-1"]));
    }

    #[test]
    fn llama_cpp_router_override_replaces_stale_cpu_layers() {
        let endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:39193").unwrap();
        let mut launch_spec = RuntimeProfileLaunchSpec {
            profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            endpoint_url: endpoint_url.clone(),
            port: RuntimePort::parse(39193).unwrap(),
            extra_args: vec![
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                "39193".to_string(),
                "--models-dir".to_string(),
                "/models".to_string(),
                "--n-gpu-layers".to_string(),
                "0".to_string(),
            ],
            env_vars: HashMap::new(),
            runtime_dir: PathBuf::from("/tmp/runtime"),
            pid_file: PathBuf::from("/tmp/runtime/runtime.pid"),
            log_file: PathBuf::from("/tmp/runtime/runtime.log"),
            health_check_url: endpoint_url,
        };

        apply_llama_cpp_launch_overrides(
            &mut launch_spec,
            &RuntimeProfileLaunchOverrides {
                device: Some(RuntimeDeviceSettings {
                    mode: RuntimeDeviceMode::Gpu,
                    device_id: None,
                    gpu_layers: None,
                    tensor_split: None,
                }),
                context_size: Some(4096),
            },
        );

        assert!(launch_spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--n-gpu-layers", "-1"]));
        assert!(launch_spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--ctx-size", "4096"]));
        assert!(!launch_spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--n-gpu-layers", "0"]));
    }
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
