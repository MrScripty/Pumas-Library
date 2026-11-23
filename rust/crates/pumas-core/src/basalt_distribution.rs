//! Basalt release selection for Pumas-managed Linux x64 installations.

use crate::network::GitHubAsset;

pub const ENTRY: &str = "bin/basalt";

pub const fn available() -> bool {
    cfg!(all(
        feature = "basalt",
        target_os = "linux",
        target_arch = "x86_64"
    ))
}

pub fn select_archive<'a>(tag: &str, assets: &'a [GitHubAsset]) -> Option<&'a GitHubAsset> {
    if !available() {
        return None;
    }
    let version = semver::Version::parse(tag.strip_prefix('v')?).ok()?;
    // This provider uses the 0.5 health and launch interfaces as qualified on 0.5.6.
    if version.major != 0 || version.minor != 5 || version.patch < 6 || !version.pre.is_empty() {
        return None;
    }
    let name = format!("basalt-{tag}-linux-x64.tar.gz");
    assets.iter().find(|asset| asset.name == name)
}

pub(crate) async fn prepare_launch(
    config: &mut crate::process::BinaryLaunchConfig,
) -> crate::Result<()> {
    use std::{path::PathBuf, process::Stdio, time::Duration};

    fn unavailable(message: impl Into<String>) -> crate::PumasError {
        crate::PumasError::InvalidParams {
            message: message.into(),
        }
    }

    if !available() {
        return Err(unavailable("This Pumas build cannot run Basalt"));
    }
    let engine = config.version_dir.join(ENTRY);
    let engine_arg = engine
        .to_str()
        .ok_or_else(|| unavailable("Basalt executable path must be Unicode"))?;

    if let Some(path) = std::env::var_os("PUMAS_BASALT_CUDA_LIB_DIR") {
        let directory = PathBuf::from(path);
        if !directory.is_absolute() {
            return Err(unavailable(
                "PUMAS_BASALT_CUDA_LIB_DIR must be an absolute directory",
            ));
        }
        let directory = tokio::fs::canonicalize(directory)
            .await
            .map_err(|_| unavailable("PUMAS_BASALT_CUDA_LIB_DIR cannot be resolved"))?;
        for library in ["libcudart.so.13", "libcublas.so.13", "libcublasLt.so.13"] {
            if !tokio::fs::metadata(directory.join(library))
                .await
                .is_ok_and(|metadata| metadata.is_file())
            {
                return Err(unavailable(format!(
                    "Basalt CUDA library {library} is missing from PUMAS_BASALT_CUDA_LIB_DIR"
                )));
            }
        }
        let directory = directory
            .to_str()
            .filter(|path| !path.contains(':'))
            .ok_or_else(|| {
                unavailable("Basalt CUDA library directory cannot be used as a single loader path")
            })?;
        config
            .env_vars
            .insert("LD_LIBRARY_PATH".into(), directory.into());
    }
    if !tokio::fs::metadata(&config.binary_path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
    {
        return Err(unavailable(
            "Basalt server Python is missing from the installed runtime",
        ));
    }

    // Use the engine's authoritative GPU selection under exactly the same
    // device visibility and loader environment as the subsequent server.
    let mut command = std::process::Command::new(&engine);
    crate::platform::child_environment::apply_owned_child_environment(&mut command);
    command
        .envs(&config.env_vars)
        .current_dir(&config.version_dir)
        .args(["--gpu-plan", "--gpu", "auto"])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    #[derive(serde::Deserialize)]
    struct GpuPlan {
        devices: String,
        error: String,
    }
    let output = tokio::time::timeout(Duration::from_secs(10), command.output())
        .await
        .map_err(|_| device_unavailable("Basalt CUDA compatibility check timed out"))?
        .map_err(|_| {
            device_unavailable("Could not run the installed Basalt GPU compatibility check")
        })?;
    let plan: GpuPlan = serde_json::from_slice(&output.stdout)
        .map_err(|_| device_unavailable("Basalt requires a working CUDA 13 loader and a visible NVIDIA GPU with compute capability 12.0"))?;
    if !output.status.success() || !plan.error.is_empty() || plan.devices.is_empty() {
        return Err(device_unavailable("Basalt requires a visible NVIDIA GPU with compute capability 12.0 and a compatible driver"));
    }

    config
        .extra_args
        .extend(["--engine-bin".into(), engine_arg.into()]);
    config
        .env_vars
        .insert("PYTHONNOUSERSITE".into(), "1".into());
    config
        .env_vars
        .insert("PYTHONDONTWRITEBYTECODE".into(), "1".into());
    Ok(())
}

fn device_unavailable(device: &str) -> crate::PumasError {
    crate::PumasError::DeviceNotAvailable {
        device: device.into(),
    }
}
