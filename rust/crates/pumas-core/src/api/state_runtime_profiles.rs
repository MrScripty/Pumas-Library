//! Runtime profile lifecycle helpers used by primary-state IPC dispatch.

use super::runtime_profiles::ManagedRuntimeShutdownSummary;
use super::state::PrimaryState;
use crate::error::PumasError;
use crate::models::{
    LaunchResponse, RuntimeDeviceMode, RuntimeDeviceSettings, RuntimeLifecycleState,
    RuntimeProfileId, RuntimeProfileStatus,
};
use crate::process::BinaryLaunchConfig;
use crate::providers::ExecutableArtifactFormat;
use crate::runtime_profiles::{
    generate_llama_cpp_router_catalog, RuntimeProfileBinaryLaunchKind,
    RuntimeProfileLaunchOverrides, RuntimeProfileLaunchSpec, RuntimeProfileLaunchStrategy,
};
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
    Ok(launch_runtime_profile_with_receipt(
        primary,
        profile_id,
        tag,
        version_dir,
        model_id,
        overrides,
    )
    .await?
    .response)
}

pub(super) async fn launch_runtime_profile_with_receipt(
    primary: &PrimaryState,
    profile_id: RuntimeProfileId,
    tag: &str,
    version_dir: &Path,
    model_id: Option<String>,
    overrides: Option<RuntimeProfileLaunchOverrides>,
) -> std::result::Result<crate::runtime_profiles::OwnedRuntimeProfileLaunchReceipt, PumasError> {
    let operation_guard = primary
        .runtime_profile_service
        .begin_profile_operation(profile_id.clone())?;
    let spec = primary
        .runtime_profile_service
        .managed_profile_launch_spec(profile_id.clone())
        .await?;

    primary
        .runtime_profile_service
        .process_owner
        .ensure_inactive(&profile_id)?;
    let (spec, model_path) =
        prepare_runtime_profile_launch_spec(primary, spec, model_id.as_deref(), overrides.as_ref())
            .await?;

    if matches!(
        spec.launch_strategy,
        RuntimeProfileLaunchStrategy::InProcessRuntime(_)
    ) {
        return Ok(crate::runtime_profiles::OwnedRuntimeProfileLaunchReceipt {
            response: launch_in_process_runtime_profile(primary, profile_id, spec).await?,
            observation: None,
        });
    }

    let config = runtime_profile_binary_launch_config(tag, version_dir, &spec)?;
    primary
        .runtime_profile_service
        .process_owner
        .launch(config, spec, model_path, operation_guard)
        .await
}

async fn launch_in_process_runtime_profile(
    primary: &PrimaryState,
    profile_id: RuntimeProfileId,
    spec: RuntimeProfileLaunchSpec,
) -> std::result::Result<LaunchResponse, PumasError> {
    primary
        .runtime_profile_service
        .record_profile_lifecycle_status(RuntimeProfileStatus {
            profile_id,
            state: RuntimeLifecycleState::Running,
            endpoint_url: Some(spec.endpoint_url),
            pid: None,
            log_path: None,
            last_error: None,
        })?;

    Ok(LaunchResponse {
        success: true,
        error: None,
        log_path: None,
        ready: Some(true),
    })
}

fn runtime_profile_binary_launch_config(
    tag: &str,
    version_dir: &Path,
    launch_spec: &RuntimeProfileLaunchSpec,
) -> std::result::Result<BinaryLaunchConfig, PumasError> {
    let config = match launch_spec.launch_strategy {
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::OllamaServe,
        ) => BinaryLaunchConfig::ollama(tag, version_dir),
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppRouter,
        ) => BinaryLaunchConfig::llama_cpp_router(
            tag,
            version_dir,
            "127.0.0.1",
            launch_spec.port.value(),
            version_dir,
        ),
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
        ) => BinaryLaunchConfig::llama_cpp_dedicated(
            tag,
            version_dir,
            "127.0.0.1",
            launch_spec.port.value(),
            version_dir,
        ),
        RuntimeProfileLaunchStrategy::InProcessRuntime(_) => {
            return Err(PumasError::InvalidParams {
                message: "in-process runtime profiles are not launched as binary processes"
                    .to_string(),
            });
        }
        RuntimeProfileLaunchStrategy::ExternalOnly => {
            return Err(PumasError::InvalidParams {
                message: "external-only runtime profiles cannot be launched as managed processes"
                    .to_string(),
            });
        }
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
) -> std::result::Result<(RuntimeProfileLaunchSpec, Option<std::path::PathBuf>), PumasError> {
    let mut selected_model_path = None;
    match launch_spec.launch_strategy {
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppRouter,
        ) => {
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
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
        ) => {
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
            if ExecutableArtifactFormat::from_path(&model_path)
                != Some(ExecutableArtifactFormat::Gguf)
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
            selected_model_path = Some(model_path);
            if let Some(overrides) = overrides {
                apply_llama_cpp_launch_overrides(&mut launch_spec, overrides);
            }
        }
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::OllamaServe,
        )
        | RuntimeProfileLaunchStrategy::InProcessRuntime(_)
        | RuntimeProfileLaunchStrategy::ExternalOnly => {}
    }

    Ok((launch_spec, selected_model_path))
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

pub(super) async fn stop_runtime_profile(
    primary: &PrimaryState,
    profile_id: RuntimeProfileId,
) -> std::result::Result<bool, PumasError> {
    // Owned sessions are stopped from their immutable launch identity, even if
    // a caller cancels startup or configuration is subsequently unavailable.
    if let Some((receipt, result)) = primary
        .runtime_profile_service
        .process_owner
        .stop_with_receipt(&profile_id)
        .await?
    {
        primary
            .serving_service
            .record_profile_unavailable_for_owned_generation(
                &profile_id,
                receipt.generation,
                &primary.runtime_profile_service.process_owner,
            )
            .await?;
        return result;
    }
    let _guard = primary
        .runtime_profile_service
        .begin_profile_operation(profile_id.clone())?;
    let spec = primary
        .runtime_profile_service
        .managed_profile_launch_spec(profile_id.clone())
        .await?;
    if !matches!(
        spec.launch_strategy,
        RuntimeProfileLaunchStrategy::InProcessRuntime(_)
    ) {
        if async_fs::try_exists(&spec.pid_file)
            .await
            .map_err(|e| PumasError::io_with_path(e, &spec.pid_file))?
        {
            return Err(PumasError::Other(
                "Runtime PID metadata is unowned; refusing to signal or remove it".to_string(),
            ));
        }
        return Ok(false);
    }
    primary
        .runtime_profile_service
        .record_profile_lifecycle_status(RuntimeProfileStatus {
            profile_id: profile_id.clone(),
            state: RuntimeLifecycleState::Stopped,
            endpoint_url: Some(spec.endpoint_url),
            pid: None,
            log_path: None,
            last_error: None,
        })?;
    primary
        .serving_service
        .record_profile_unavailable(&profile_id)
        .await;
    Ok(false)
}

pub(super) async fn stop_all_managed_runtime_profiles(
    primary: &PrimaryState,
) -> std::result::Result<ManagedRuntimeShutdownSummary, PumasError> {
    let results = primary
        .runtime_profile_service
        .process_owner
        .close_and_drain()
        .await?;
    let mut summary = ManagedRuntimeShutdownSummary {
        profiles_processed: results.len(),
        processes_stopped: 0,
        errors: Vec::new(),
    };
    for (profile_id, result) in results {
        primary
            .serving_service
            .record_profile_unavailable(&profile_id)
            .await;
        match result {
            Ok(true) => summary.processes_stopped += 1,
            Ok(false) => {}
            Err(error) => summary
                .errors
                .push(format!("{}: {error}", profile_id.as_str())),
        }
    }
    for spec in primary
        .runtime_profile_service
        .list_managed_profile_launch_specs()
        .await?
    {
        if matches!(
            spec.launch_strategy,
            RuntimeProfileLaunchStrategy::InProcessRuntime(_)
        ) {
            summary.profiles_processed += 1;
            if let Err(error) = stop_runtime_profile(primary, spec.profile_id.clone()).await {
                summary
                    .errors
                    .push(format!("{}: {error}", spec.profile_id.as_str()));
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RuntimeEndpointUrl, RuntimePort, RuntimeProviderId, RuntimeProviderMode};
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn llama_cpp_launch_overrides_replace_profile_device_args() {
        let endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:39191").unwrap();
        let mut launch_spec = RuntimeProfileLaunchSpec {
            profile_id: RuntimeProfileId::parse("llama-dedicated").unwrap(),
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppDedicated,
            launch_strategy: RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
            ),
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
            launch_strategy: RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
            ),
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
            launch_strategy: RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppRouter,
            ),
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
