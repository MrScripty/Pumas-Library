//! Managed runtime-profile launch spec derivation.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::models::{
    RuntimeDeviceMode, RuntimeDeviceSettings, RuntimeEndpointUrl, RuntimeManagementMode,
    RuntimePort, RuntimeProfileConfig, RuntimeProfileId, RuntimeProviderId, RuntimeProviderMode,
};
use crate::providers::{ProviderBehavior, ProviderRegistry};
use crate::{PumasError, Result};

use super::{RuntimeProfileLaunchSpec, RuntimeProfileLaunchStrategy, RuntimeProfilesConfigFile};

const IMPLICIT_RUNTIME_PORT_SPAN: u16 = 10_000;

pub(super) fn derive_managed_profile_launch_specs(
    launcher_root: &Path,
    config: &RuntimeProfilesConfigFile,
    provider_registry: &ProviderRegistry,
) -> Result<Vec<RuntimeProfileLaunchSpec>> {
    let mut used_ports: HashMap<u16, RuntimeProfileId> = HashMap::new();
    let mut profiles = config
        .profiles
        .iter()
        .filter(|profile| profile.management_mode == RuntimeManagementMode::Managed)
        .collect::<Vec<_>>();
    profiles.sort_by(|left, right| left.profile_id.as_str().cmp(right.profile_id.as_str()));

    let mut specs = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let behavior =
            provider_registry
                .get(profile.provider)
                .ok_or_else(|| PumasError::InvalidParams {
                    message: "runtime profile provider is not registered".to_string(),
                })?;
        let port = match profile.port {
            Some(port) => {
                if let Some(existing_profile_id) = used_ports.get(&port.value()) {
                    return Err(PumasError::InvalidParams {
                        message: format!(
                            "runtime profile port collision: {} is already used by {}; choose a unique managed profile process port or leave the port blank for automatic allocation",
                            port.value(),
                            existing_profile_id.as_str()
                        ),
                    });
                }
                used_ports.insert(port.value(), profile.profile_id.clone());
                port
            }
            None => match profile.endpoint_url.as_ref().and_then(endpoint_port) {
                Some(port) => {
                    if let Some(existing_profile_id) = used_ports.get(&port.value()) {
                        return Err(PumasError::InvalidParams {
                            message: format!(
                                "runtime profile endpoint port collision: {} is already used by {}; choose a unique managed profile endpoint or leave the endpoint blank for automatic allocation",
                                port.value(),
                                existing_profile_id.as_str()
                            ),
                        });
                    }
                    used_ports.insert(port.value(), profile.profile_id.clone());
                    port
                }
                None => allocate_implicit_runtime_port(profile, behavior, &mut used_ports)?,
            },
        };
        let endpoint_url = match &profile.endpoint_url {
            Some(endpoint_url) => endpoint_url.clone(),
            None => endpoint_url_for_port(port)?,
        };
        let runtime_dir = launcher_root
            .join("launcher-data")
            .join("runtime-profiles")
            .join(&behavior.managed_runtime_path_segment)
            .join(profile.profile_id.as_str());

        specs.push(RuntimeProfileLaunchSpec {
            profile_id: profile.profile_id.clone(),
            provider: profile.provider,
            provider_mode: profile.provider_mode,
            launch_strategy: RuntimeProfileLaunchStrategy::for_profile(profile, behavior)?,
            endpoint_url: endpoint_url.clone(),
            port,
            extra_args: profile_runtime_extra_args(launcher_root, profile, &endpoint_url, port)?,
            env_vars: profile_runtime_env_vars(profile, &endpoint_url, port)?,
            pid_file: runtime_dir.join("runtime.pid"),
            log_file: runtime_dir.join("runtime.log"),
            health_check_url: endpoint_url,
            runtime_dir,
        });
    }

    Ok(specs)
}

fn allocate_implicit_runtime_port(
    profile: &RuntimeProfileConfig,
    behavior: &ProviderBehavior,
    used_ports: &mut HashMap<u16, RuntimeProfileId>,
) -> Result<RuntimePort> {
    let base_port = behavior.managed_runtime_base_port;
    let start_offset = implicit_port_offset(profile.profile_id.as_str());
    for step in 0..IMPLICIT_RUNTIME_PORT_SPAN {
        let offset =
            ((start_offset as u32 + step as u32) % IMPLICIT_RUNTIME_PORT_SPAN as u32) as u16;
        let candidate = base_port as u32 + 1 + offset as u32;
        if candidate > u16::MAX as u32 {
            continue;
        }
        let candidate = candidate as u16;
        if let std::collections::hash_map::Entry::Vacant(entry) = used_ports.entry(candidate) {
            entry.insert(profile.profile_id.clone());
            return RuntimePort::parse(candidate).map_err(|message| PumasError::InvalidParams {
                message: format!("invalid implicit runtime port: {message}"),
            });
        }
    }

    Err(PumasError::InvalidParams {
        message: format!(
            "no available implicit runtime ports for profile {}",
            profile.profile_id.as_str()
        ),
    })
}

fn implicit_port_offset(profile_id: &str) -> u16 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    profile_id.hash(&mut hasher);
    (hasher.finish() % IMPLICIT_RUNTIME_PORT_SPAN as u64) as u16
}

fn endpoint_url_for_port(port: RuntimePort) -> Result<RuntimeEndpointUrl> {
    RuntimeEndpointUrl::parse(format!("http://127.0.0.1:{}", port.value())).map_err(|message| {
        PumasError::InvalidParams {
            message: format!("invalid runtime endpoint URL: {message}"),
        }
    })
}

pub(super) fn endpoint_port(endpoint_url: &RuntimeEndpointUrl) -> Option<RuntimePort> {
    url::Url::parse(endpoint_url.as_str())
        .ok()
        .and_then(|url| url.port())
        .and_then(|port| RuntimePort::parse(port).ok())
}

fn profile_runtime_env_vars(
    profile: &RuntimeProfileConfig,
    endpoint_url: &RuntimeEndpointUrl,
    port: RuntimePort,
) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();
    env_vars.insert(
        "PUMAS_RUNTIME_PROFILE_ID".to_string(),
        profile.profile_id.as_str().to_string(),
    );
    match profile.provider {
        RuntimeProviderId::Ollama => {
            env_vars.insert(
                "OLLAMA_HOST".to_string(),
                runtime_host_port(endpoint_url, port)?,
            );
        }
        RuntimeProviderId::LlamaCpp => {}
    }
    apply_device_visibility_env(&mut env_vars, profile);
    Ok(env_vars)
}

fn profile_runtime_extra_args(
    launcher_root: &Path,
    profile: &RuntimeProfileConfig,
    endpoint_url: &RuntimeEndpointUrl,
    port: RuntimePort,
) -> Result<Vec<String>> {
    match profile.provider {
        RuntimeProviderId::Ollama => Ok(Vec::new()),
        RuntimeProviderId::LlamaCpp => match profile.provider_mode {
            RuntimeProviderMode::LlamaCppRouter | RuntimeProviderMode::LlamaCppDedicated => {
                let mut args = vec![
                    "--host".to_string(),
                    runtime_host(endpoint_url)?.to_string(),
                    "--port".to_string(),
                    port.value().to_string(),
                ];
                if profile.provider_mode == RuntimeProviderMode::LlamaCppRouter {
                    args.extend([
                        "--models-dir".to_string(),
                        llama_cpp_router_models_dir(launcher_root)
                            .to_string_lossy()
                            .to_string(),
                    ]);
                }
                apply_llama_cpp_device_args(&mut args, profile);
                Ok(args)
            }
            RuntimeProviderMode::OllamaServe => Err(PumasError::InvalidParams {
                message: "llama.cpp runtime profile cannot use ollama_serve mode".to_string(),
            }),
        },
    }
}

fn apply_llama_cpp_device_args(args: &mut Vec<String>, profile: &RuntimeProfileConfig) {
    if let Some(gpu_layers) = llama_cpp_gpu_layers_arg(&profile.device) {
        args.extend(["--n-gpu-layers".to_string(), gpu_layers.to_string()]);
    }

    if let Some(tensor_split) = &profile.device.tensor_split {
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

fn llama_cpp_router_models_dir(launcher_root: &Path) -> PathBuf {
    launcher_root.join("shared-resources").join("models")
}

fn runtime_host(endpoint_url: &RuntimeEndpointUrl) -> Result<String> {
    let parsed =
        url::Url::parse(endpoint_url.as_str()).map_err(|err| PumasError::InvalidParams {
            message: format!("invalid runtime endpoint URL: {err}"),
        })?;
    parsed
        .host_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| PumasError::InvalidParams {
            message: "runtime endpoint URL must include a host".to_string(),
        })
}

fn runtime_host_port(endpoint_url: &RuntimeEndpointUrl, port: RuntimePort) -> Result<String> {
    let host = runtime_host(endpoint_url)?;
    Ok(format!("{host}:{}", port.value()))
}

fn apply_device_visibility_env(
    env_vars: &mut HashMap<String, String>,
    profile: &RuntimeProfileConfig,
) {
    match profile.device.mode {
        RuntimeDeviceMode::Cpu => {
            env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), String::new());
            env_vars.insert("HIP_VISIBLE_DEVICES".to_string(), String::new());
            env_vars.insert("ROCR_VISIBLE_DEVICES".to_string(), String::new());
        }
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::SpecificDevice => {
            if let Some(device_id) = profile.device.device_id.as_deref() {
                env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), device_id.to_string());
                env_vars.insert("HIP_VISIBLE_DEVICES".to_string(), device_id.to_string());
                env_vars.insert("ROCR_VISIBLE_DEVICES".to_string(), device_id.to_string());
            }
        }
        RuntimeDeviceMode::Auto | RuntimeDeviceMode::Hybrid => {}
    }
}
