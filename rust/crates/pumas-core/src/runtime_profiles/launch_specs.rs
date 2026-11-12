//! Managed runtime-profile launch spec derivation.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rand::RngExt;

use crate::model_library::ModelLibrary;
use crate::models::{
    RuntimeDeviceMode, RuntimeDeviceSettings, RuntimeEndpointUrl, RuntimeManagementMode,
    RuntimePort, RuntimeProfileConfig, RuntimeProfileId,
};
use crate::providers::{ProviderBehavior, ProviderRegistry};
use crate::{PumasError, Result};

use super::{
    RuntimeProfileBinaryLaunchKind, RuntimeProfileLaunchSpec, RuntimeProfileLaunchStrategy,
    RuntimeProfilesConfigFile,
};

const IMPLICIT_RUNTIME_PORT_SPAN: u16 = 10_000;

/// Environment variable carrying the Torch sidecar's API token.
pub const TORCH_API_TOKEN_ENV: &str = "PUMAS_TORCH_API_TOKEN";
/// Header every Torch sidecar request except `/health` must carry.
pub const TORCH_API_TOKEN_HEADER: &str = "x-pumas-torch-token";
/// Environment variable listing the roots Torch may load model files from.
/// Set at launch from the model library (see [`torch_model_roots`]).
pub const TORCH_MODEL_ROOTS_ENV: &str = "PUMAS_TORCH_MODEL_ROOTS";
/// Environment variable naming the launcher process the sidecar watches.
pub const TORCH_PARENT_PID_ENV: &str = "PUMAS_TORCH_PARENT_PID";

/// Random API token for Torch sidecars launched by this backend process.
///
/// It is generated once per backend process, never persisted, and reaches the
/// sidecar only through its environment (never argv). Every Torch sidecar this
/// process launches requires it, so a sidecar left by an earlier backend, or
/// any other local process, cannot drive the sidecar without it.
pub fn torch_api_token() -> &'static str {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN.get_or_init(|| hex::encode(rand::rng().random::<[u8; 32]>()))
}

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
        let launch_strategy = RuntimeProfileLaunchStrategy::for_profile(profile, behavior)?;

        specs.push(RuntimeProfileLaunchSpec {
            profile_id: profile.profile_id.clone(),
            provider: profile.provider,
            provider_mode: profile.provider_mode,
            launch_strategy,
            endpoint_url: endpoint_url.clone(),
            port,
            extra_args: profile_runtime_extra_args(
                launcher_root,
                profile,
                &endpoint_url,
                port,
                launch_strategy,
            )?,
            env_vars: profile_runtime_env_vars(profile, &endpoint_url, port, launch_strategy)?,
            pid_file: runtime_dir.join("runtime.pid"),
            log_file: runtime_dir.join("runtime.log"),
            health_check_url: if profile.provider == crate::models::RuntimeProviderId::Torch {
                RuntimeEndpointUrl::parse(format!(
                    "{}/health",
                    endpoint_url.as_str().trim_end_matches('/')
                ))
                .map_err(|message| PumasError::InvalidParams { message })?
            } else {
                endpoint_url
            },
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
    launch_strategy: RuntimeProfileLaunchStrategy,
) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();
    env_vars.insert(
        "PUMAS_RUNTIME_PROFILE_ID".to_string(),
        profile.profile_id.as_str().to_string(),
    );
    match launch_strategy {
        RuntimeProfileLaunchStrategy::BinaryProcess(RuntimeProfileBinaryLaunchKind::TorchServe) => {
            env_vars.insert("HF_HUB_OFFLINE".into(), "1".into());
            env_vars.insert("TRANSFORMERS_OFFLINE".into(), "1".into());
            env_vars.insert("PYTHONNOUSERSITE".into(), "1".into());
            env_vars.insert(TORCH_API_TOKEN_ENV.into(), torch_api_token().into());
            env_vars.insert(TORCH_PARENT_PID_ENV.into(), std::process::id().to_string());
        }
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::OllamaServe,
        ) => {
            env_vars.insert(
                "OLLAMA_HOST".to_string(),
                runtime_host_port(endpoint_url, port)?,
            );
        }
        RuntimeProfileLaunchStrategy::BinaryProcess(RuntimeProfileBinaryLaunchKind::OnnxHost) => {
            #[cfg(feature = "onnx-runtime")]
            {
                use crate::onnx_runtime::host_protocol;
                env_vars.insert(
                    host_protocol::TOKEN_ENV.into(),
                    host_protocol::host_api_token().into(),
                );
                env_vars.insert(
                    host_protocol::PARENT_PID_ENV.into(),
                    std::process::id().to_string(),
                );
            }
        }
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppRouter
            | RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
        )
        | RuntimeProfileLaunchStrategy::InProcessRuntime(_)
        | RuntimeProfileLaunchStrategy::ExternalOnly => {}
    }
    apply_device_visibility_env(&mut env_vars, profile);
    Ok(env_vars)
}

fn profile_runtime_extra_args(
    launcher_root: &Path,
    profile: &RuntimeProfileConfig,
    endpoint_url: &RuntimeEndpointUrl,
    port: RuntimePort,
    launch_strategy: RuntimeProfileLaunchStrategy,
) -> Result<Vec<String>> {
    match launch_strategy {
        RuntimeProfileLaunchStrategy::BinaryProcess(RuntimeProfileBinaryLaunchKind::TorchServe) => {
            // Profile arguments replace the default Torch launch arguments.
            // The sidecar comes from this Pumas, never from the runtime, and
            // `-B` keeps every instance from writing next to the shared scripts.
            let entry = super::torch_sidecar::torch_sidecar_entry(launcher_root);
            let mut args = vec!["-B".to_string(), entry.to_string_lossy().into_owned()];
            args.extend(runtime_listener_args(endpoint_url, port)?);
            Ok(args)
        }

        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::OllamaServe,
        ) => Ok(Vec::new()),
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppRouter,
        ) => {
            let mut args = runtime_listener_args(endpoint_url, port)?;
            args.extend([
                "--models-dir".to_string(),
                llama_cpp_router_models_dir(launcher_root)
                    .to_string_lossy()
                    .to_string(),
            ]);
            apply_llama_cpp_device_args(&mut args, profile);
            Ok(args)
        }
        RuntimeProfileLaunchStrategy::BinaryProcess(
            RuntimeProfileBinaryLaunchKind::LlamaCppDedicated,
        ) => {
            let mut args = runtime_listener_args(endpoint_url, port)?;
            apply_llama_cpp_device_args(&mut args, profile);
            Ok(args)
        }
        RuntimeProfileLaunchStrategy::BinaryProcess(RuntimeProfileBinaryLaunchKind::OnnxHost) => {
            // The host derives its execution provider from the installed
            // build it loads and this device mode.
            let mut args = runtime_listener_args(endpoint_url, port)?;
            args.extend([
                "--onnx-device-mode".to_string(),
                onnx_device_mode_arg(profile.device.mode).to_string(),
            ]);
            Ok(args)
        }
        RuntimeProfileLaunchStrategy::InProcessRuntime(_) => Ok(Vec::new()),
        RuntimeProfileLaunchStrategy::ExternalOnly => Ok(Vec::new()),
    }
}

/// The `--onnx-device-mode` spelling of a profile's device mode.
pub fn onnx_device_mode_arg(mode: RuntimeDeviceMode) -> &'static str {
    match mode {
        RuntimeDeviceMode::Auto => "auto",
        RuntimeDeviceMode::Cpu => "cpu",
        RuntimeDeviceMode::Gpu => "gpu",
        RuntimeDeviceMode::Hybrid => "hybrid",
        RuntimeDeviceMode::SpecificDevice => "specific_device",
    }
}

/// Parse an `--onnx-device-mode` value.
pub fn parse_onnx_device_mode(value: &str) -> Option<RuntimeDeviceMode> {
    [
        RuntimeDeviceMode::Auto,
        RuntimeDeviceMode::Cpu,
        RuntimeDeviceMode::Gpu,
        RuntimeDeviceMode::Hybrid,
        RuntimeDeviceMode::SpecificDevice,
    ]
    .into_iter()
    .find(|mode| onnx_device_mode_arg(*mode) == value)
}

static ONNX_HOST_EXECUTABLE: OnceLock<PathBuf> = OnceLock::new();

/// Register the executable that runs ONNX hosts (`pumas-rpc --onnx-host`).
///
/// The RPC server registers its own executable at startup. An embedding
/// application that registers none cannot launch ONNX Runtime profiles, and
/// such a launch is refused with that reason.
pub fn register_onnx_host_executable(executable: PathBuf) -> Result<()> {
    let registered = ONNX_HOST_EXECUTABLE.get_or_init(|| executable.clone());
    if registered != &executable {
        return Err(PumasError::InvalidParams {
            message: format!(
                "an ONNX host executable is already registered: {}",
                registered.display()
            ),
        });
    }
    Ok(())
}

#[cfg(feature = "onnx-runtime")]
pub(crate) fn onnx_host_executable() -> Option<&'static Path> {
    ONNX_HOST_EXECUTABLE.get().map(PathBuf::as_path)
}

fn runtime_listener_args(
    endpoint_url: &RuntimeEndpointUrl,
    port: RuntimePort,
) -> Result<Vec<String>> {
    Ok(vec![
        "--host".to_string(),
        runtime_host(endpoint_url)?.to_string(),
        "--port".to_string(),
        port.value().to_string(),
    ])
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

/// The roots the Torch sidecar may load model files from, joined for
/// `PUMAS_TORCH_MODEL_ROOTS`: the library's own root plus the directory of
/// every external-reference model the library knows about at launch.
///
/// The set is read once per launch. A model imported by external reference
/// while Torch runs is refused (with that reason) until the Torch profile is
/// restarted, which reads the set again.
pub(crate) async fn torch_model_roots(library: &ModelLibrary) -> Result<String> {
    let external = serde_json::to_value(crate::models::StorageKind::ExternalReference)
        .map_err(|err| PumasError::Other(format!("storage kind serialization failed: {err}")))?;
    let mut roots = vec![library.library_root().to_path_buf()];
    for record in library.list_models().await? {
        if record.metadata.get("storage_kind") != Some(&external) {
            continue;
        }
        let Some(entry) = record
            .metadata
            .get("entry_path")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let entry = PathBuf::from(entry);
        let is_file = tokio::fs::metadata(&entry)
            .await
            .is_ok_and(|metadata| metadata.is_file());
        roots.push(match (is_file, entry.parent()) {
            (true, Some(parent)) => parent.to_path_buf(),
            _ => entry,
        });
    }
    join_model_roots(roots)
}

/// Join roots for the sidecar, dropping duplicates and roots nested in another.
///
/// A root the platform cannot place in a path list (it contains the list
/// separator) or that is not Unicode is skipped with a warning; Torch then
/// refuses models under it with a reason instead of the launch failing.
fn join_model_roots(mut roots: Vec<PathBuf>) -> Result<String> {
    roots.sort();
    roots.dedup();
    let covered = |root: &PathBuf, all: &[PathBuf]| {
        all.iter()
            .any(|other| other != root && root.starts_with(other))
    };
    let kept = roots
        .iter()
        .filter(|root| !covered(root, &roots))
        .filter(|root| {
            let usable = root.to_str().is_some() && std::env::join_paths([root]).is_ok();
            if !usable {
                tracing::warn!(
                    root = %root.display(),
                    "model root cannot be passed to Torch; models under it will be refused"
                );
            }
            usable
        })
        .collect::<Vec<_>>();
    let joined = std::env::join_paths(kept).map_err(|err| PumasError::InvalidParams {
        message: format!("library model roots cannot be passed to Torch: {err}"),
    })?;
    joined.into_string().map_err(|_| PumasError::InvalidParams {
        message: "library model roots are not valid Unicode".to_string(),
    })
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

#[cfg(test)]
mod torch_tests {
    use super::*;
    use crate::models::{RuntimeProviderId, RuntimeProviderMode};

    #[test]
    fn torch_profile_uses_managed_python_entry_health_and_offline_assets() {
        let mut config = RuntimeProfilesConfigFile::default_seed();
        let profile = &mut config.profiles[0];
        profile.provider = RuntimeProviderId::Torch;
        profile.provider_mode = RuntimeProviderMode::TorchServe;
        profile.device.mode = RuntimeDeviceMode::Hybrid;
        profile.endpoint_url = None;
        profile.port = None;
        let specs = derive_managed_profile_launch_specs(
            Path::new("/fixture"),
            &config,
            &ProviderRegistry::builtin(),
        )
        .unwrap();
        let [spec] = specs.as_slice() else {
            panic!("expected one managed profile")
        };
        // The entry is this Pumas's sidecar, shared by every runtime version.
        let entry = super::super::torch_sidecar::torch_sidecar_entry(Path::new("/fixture"));
        assert!(entry.starts_with("/fixture/launcher-data/torch-sidecar"));
        assert_eq!(
            &spec.extra_args[..2],
            &["-B".to_string(), entry.to_string_lossy().into_owned()]
        );
        assert!(spec.extra_args.contains(&"--port".into()));
        assert!(spec.health_check_url.as_str().ends_with("/health"));
        assert_eq!(
            spec.env_vars.get("HF_HUB_OFFLINE").map(String::as_str),
            Some("1")
        );
        assert!(spec
            .pid_file
            .starts_with("/fixture/launcher-data/runtime-profiles/torch"));
        // The sidecar's token, model confinement and parent watchdog arrive
        // through the environment; the token never appears in argv.
        let token = spec.env_vars.get(TORCH_API_TOKEN_ENV).expect("API token");
        assert_eq!(token, torch_api_token());
        assert_eq!(token.len(), 64);
        assert!(!spec
            .extra_args
            .iter()
            .any(|arg| arg.contains(token.as_str())));
        // Model roots come from the library at launch, not from the spec.
        assert!(!spec.env_vars.contains_key(TORCH_MODEL_ROOTS_ENV));
        assert_eq!(
            spec.env_vars.get(TORCH_PARENT_PID_ENV),
            Some(&std::process::id().to_string())
        );

        // Exercise both production argument builders against real Python imports.
        // The fixture substitutes the sidecar and package code, not the launch
        // arguments: the entry runs from the shared sidecar directory while the
        // working directory is the runtime's own.
        let mut changed_routes = Vec::new();
        let launcher = tempfile::tempdir().unwrap();
        let profile_spec = derive_managed_profile_launch_specs(
            launcher.path(),
            &config,
            &ProviderRegistry::builtin(),
        )
        .unwrap()
        .remove(0);
        let entry = super::super::torch_sidecar::torch_sidecar_entry(launcher.path());
        let sidecar = entry.parent().unwrap();
        std::fs::create_dir_all(sidecar).unwrap();
        std::fs::write(
            sidecar.join("sidecar_dependency.py"),
            "import launch_dependency\nVALUE = launch_dependency.VALUE\n",
        )
        .unwrap();
        std::fs::write(
            &entry,
            "import sidecar_dependency\nassert sidecar_dependency.VALUE == 42\n",
        )
        .unwrap();
        for (route, args) in [
            (
                "version",
                crate::process::BinaryLaunchConfig::torch("fixture", "/fixture", &entry).extra_args,
            ),
            ("profile", profile_spec.extra_args.clone()),
        ] {
            let runtime = tempfile::tempdir().unwrap();
            let packages = runtime.path().join("site-packages");
            std::fs::create_dir(&packages).unwrap();
            std::fs::write(packages.join("launch_dependency.py"), "VALUE = 42\n").unwrap();
            let status =
                std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
                    .args(args)
                    .envs(&spec.env_vars)
                    .env("PYTHONPATH", &packages)
                    .env_remove("PYTHONDONTWRITEBYTECODE")
                    .env_remove("PYTHONPYCACHEPREFIX")
                    .current_dir(runtime.path())
                    .status()
                    .expect("Torch launch regression requires Python");
            assert!(status.success(), "{route} launch failed");
            if packages.join("__pycache__").exists() || sidecar.join("__pycache__").exists() {
                changed_routes.push(route);
            }
        }
        assert!(
            changed_routes.is_empty(),
            "Torch launches wrote bytecode into packages or the shared sidecar: {changed_routes:?}"
        );
    }

    #[tokio::test]
    async fn torch_model_roots_admit_library_and_external_models_only() {
        use crate::models::{BundleFormat, ModelMetadata, StorageKind};

        let temp = tempfile::tempdir().unwrap();
        let library_root = temp.path().join("library");
        let library = ModelLibrary::new(&library_root).await.unwrap();
        let external = temp.path().join("external").join("bundle");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::write(external.join("model_index.json"), "{}").unwrap();
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let model_dir = library.build_model_path("diffusion", "test", "external-bundle");
        std::fs::create_dir_all(&model_dir).unwrap();
        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some("diffusion/test/external-bundle".to_string()),
            family: Some("test".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("external-bundle".to_string()),
            cleaned_name: Some("external-bundle".to_string()),
            source_path: Some(external.display().to_string()),
            entry_path: Some(external.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(BundleFormat::DiffusersDirectory),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library
            .upsert_index_from_metadata(&model_dir, &metadata)
            .unwrap();

        let roots = torch_model_roots(&library).await.unwrap();
        let listed = std::env::split_paths(&roots).collect::<Vec<_>>();
        assert!(listed.contains(&library.library_root().to_path_buf()));
        assert!(listed.contains(&external));

        // The sidecar's own validator accepts the library and external model
        // and refuses a path outside both.
        let torch_server = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../torch-server");
        let check = "import sys, validation\n\
for path in sys.argv[1:3]: validation.validate_model_path(path)\n\
try:\n    validation.validate_model_path(sys.argv[3])\n\
except ValueError as error:\n    assert 'approved model root' in str(error), error\n\
else:\n    raise SystemExit('outside path was accepted')\n";
        let status = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
            .args(["-B", "-c", check])
            .arg(&model_dir)
            .arg(external.join("model_index.json"))
            .arg(&outside)
            .env(TORCH_MODEL_ROOTS_ENV, &roots)
            .current_dir(&torch_server)
            .status()
            .expect("model root regression requires Python");
        assert!(status.success());
    }

    #[test]
    fn joined_model_roots_drop_duplicates_and_nested_roots() {
        let joined = join_model_roots(vec![
            PathBuf::from("/library"),
            PathBuf::from("/library/nested"),
            PathBuf::from("/external/a"),
            PathBuf::from("/external/a"),
        ])
        .unwrap();
        assert_eq!(
            std::env::split_paths(&joined).collect::<Vec<_>>(),
            vec![PathBuf::from("/external/a"), PathBuf::from("/library")]
        );
    }
}

#[cfg(all(test, feature = "onnx-runtime"))]
mod onnx_host_tests {
    use super::*;
    use crate::models::{RuntimeProviderId, RuntimeProviderMode};
    use crate::onnx_runtime::host_protocol;

    #[test]
    fn onnx_profile_launches_its_host_with_device_mode_token_and_parent_watch() {
        let mut config = RuntimeProfilesConfigFile::default_seed();
        let profile = &mut config.profiles[0];
        profile.provider = RuntimeProviderId::OnnxRuntime;
        profile.provider_mode = RuntimeProviderMode::OnnxServe;
        profile.device.mode = RuntimeDeviceMode::Gpu;
        profile.endpoint_url = None;
        profile.port = None;
        let specs = derive_managed_profile_launch_specs(
            Path::new("/fixture"),
            &config,
            &ProviderRegistry::builtin(),
        )
        .unwrap();
        let [spec] = specs.as_slice() else {
            panic!("expected one managed profile")
        };
        assert_eq!(
            spec.launch_strategy,
            RuntimeProfileLaunchStrategy::BinaryProcess(RuntimeProfileBinaryLaunchKind::OnnxHost)
        );
        let args = &spec.extra_args;
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--onnx-device-mode", "gpu"]));
        assert!(args.contains(&"--port".into()));
        assert!(spec
            .pid_file
            .starts_with("/fixture/launcher-data/runtime-profiles/onnx-runtime"));
        let token = spec.env_vars.get(host_protocol::TOKEN_ENV).expect("token");
        assert_eq!(token, host_protocol::host_api_token());
        assert!(!args.iter().any(|arg| arg.contains(token.as_str())));
        assert_eq!(
            spec.env_vars.get(host_protocol::PARENT_PID_ENV),
            Some(&std::process::id().to_string())
        );

        // The launch config adds the installed version the host loads.
        let config = crate::process::BinaryLaunchConfig::onnx_host(
            "v1.31.1+cuda12",
            "/fixture/onnx-runtime-versions/v1.31.1+cuda12",
            "/fixture/bin/pumas-rpc",
        );
        assert_eq!(config.command.as_deref(), Some(host_protocol::HOST_FLAG));
        assert_eq!(
            config
                .env_vars
                .get(host_protocol::RUNTIME_VERSION_ENV)
                .map(String::as_str),
            Some("v1.31.1+cuda12")
        );
        assert!(config.env_vars.contains_key(host_protocol::RUNTIME_DIR_ENV));
        assert_eq!(
            config
                .env_vars
                .get(host_protocol::ORT_DISABLE_TELEMETRY_ENV)
                .map(String::as_str),
            Some("1")
        );
        for mode in [
            RuntimeDeviceMode::Auto,
            RuntimeDeviceMode::Cpu,
            RuntimeDeviceMode::Gpu,
            RuntimeDeviceMode::Hybrid,
            RuntimeDeviceMode::SpecificDevice,
        ] {
            assert_eq!(
                parse_onnx_device_mode(onnx_device_mode_arg(mode)),
                Some(mode)
            );
        }
    }
}
