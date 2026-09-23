//! Managed upstream PyTorch installation; lifecycle and state remain in VersionManager.

use super::*;
use crate::torch_client::{SUPPORTED_TORCH_PROTOCOL, TORCH_IMAGE_GENERATION_CAPABILITY};
use serde::Deserialize;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

#[cfg(test)]
pub(crate) struct TorchPublicationPause {
    pub(crate) reached: tokio::sync::Notify,
    pub(crate) resume: tokio::sync::Semaphore,
}

#[cfg(test)]
impl TorchPublicationPause {
    pub(crate) fn new() -> Self {
        Self {
            reached: tokio::sync::Notify::new(),
            resume: tokio::sync::Semaphore::new(0),
        }
    }
}

#[cfg(test)]
#[path = "torch_upstream_contract_tests.rs"]
mod torch_upstream_contract_tests;

pub(crate) struct TorchRuntimeRecipe {
    pub(crate) release_tag: &'static str,
    pub(crate) recipe_id: &'static str,
    pub(crate) torch_version: &'static str,
    pub(crate) torch_wheel_url: &'static str,
    pub(crate) torch_wheel_sha256: &'static str,
    pub(crate) torchvision_wheel_url: &'static str,
    pub(crate) torchvision_wheel_sha256: &'static str,
}

const TORCH_291: TorchRuntimeRecipe = TorchRuntimeRecipe {
    release_tag: "v2.9.1",
    recipe_id: "torch-upstream-2.9.1-r1",
    torch_version: "2.9.1+cu130",
    torch_wheel_url: "https://download-r2.pytorch.org/whl/cu130/torch-2.9.1%2Bcu130-cp312-cp312-manylinux_2_28_x86_64.whl",
    torch_wheel_sha256: "e70e1b18881e6b3c1ce402d0a989da39f956a3a057526e03c354df23d704ce9b",
    torchvision_wheel_url: "https://download-r2.pytorch.org/whl/cu130/torchvision-0.24.1%2Bcu130-cp312-cp312-manylinux_2_28_x86_64.whl",
    torchvision_wheel_sha256: "6939dd403cc28ab0a46f53e6c86e2e852cf65771c1b0ddd09c44c541a1cdbad9",
};

pub(crate) fn torch_recipe_for_tag(tag: &str) -> Option<&'static TorchRuntimeRecipe> {
    match tag {
        "v2.9.1" => Some(&TORCH_291),
        _ => None,
    }
}

pub(crate) fn is_torch_runtime_release(release: &GitHubRelease) -> bool {
    !release.prerelease && stable_torch_tag(&release.tag_name).is_some()
}

fn stable_torch_tag(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix('v')?;
    let parts: Vec<_> = version.split('.').collect();
    (parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit())))
    .then_some(version)
}

/// Materialize the qualified sidecar and lock from bytes embedded in the app binary.
pub(crate) fn write_embedded_torch_runtime(destination: &Path) -> Result<()> {
    let lock = include_str!("../../../../../../torch-server/runtime/requirements.lock");
    for (name, url, hash) in [
        (
            "torch",
            TORCH_291.torch_wheel_url,
            TORCH_291.torch_wheel_sha256,
        ),
        (
            "torchvision",
            TORCH_291.torchvision_wheel_url,
            TORCH_291.torchvision_wheel_sha256,
        ),
    ] {
        if !lock.contains(&format!("{name} @ {url}"))
            || !lock.contains(&format!("--hash=sha256:{hash}"))
        {
            return Err(failed(format!(
                "Embedded lock disagrees with the qualified {name} wheel"
            )));
        }
    }
    if !include_str!("../../../../../../torch-server/validate_runtime.py").contains(&format!(
        "torch.__version__ != \"{}\"",
        TORCH_291.torch_version
    )) {
        return Err(failed(
            "Embedded runtime validator disagrees with the qualified Torch version",
        ));
    }
    std::fs::create_dir_all(destination).map_err(PumasError::from)?;
    for (name, contents) in [
        ("LICENSE", include_str!("../../../../../../LICENSE")),
        (
            "serve.py",
            include_str!("../../../../../../torch-server/serve.py"),
        ),
        (
            "validate_runtime.py",
            include_str!("../../../../../../torch-server/validate_runtime.py"),
        ),
        (
            "resolve_runtime.py",
            include_str!("../../../../../../torch-server/resolve_runtime.py"),
        ),
        (
            "probe_runtime.py",
            include_str!("../../../../../../torch-server/probe_runtime.py"),
        ),
        (
            "nunchaku_compat.py",
            include_str!("../../../../../../torch-server/nunchaku_compat.py"),
        ),
        (
            "control_api.py",
            include_str!("../../../../../../torch-server/control_api.py"),
        ),
        (
            "device_manager.py",
            include_str!("../../../../../../torch-server/device_manager.py"),
        ),
        (
            "diffusion.py",
            include_str!("../../../../../../torch-server/diffusion.py"),
        ),
        (
            "flux2.py",
            include_str!("../../../../../../torch-server/flux2.py"),
        ),
        (
            "image_api.py",
            include_str!("../../../../../../torch-server/image_api.py"),
        ),
        (
            "model_manager.py",
            include_str!("../../../../../../torch-server/model_manager.py"),
        ),
        (
            "openai_api.py",
            include_str!("../../../../../../torch-server/openai_api.py"),
        ),
        (
            "validation.py",
            include_str!("../../../../../../torch-server/validation.py"),
        ),
        (
            "loaders/__init__.py",
            include_str!("../../../../../../torch-server/loaders/__init__.py"),
        ),
        (
            "loaders/dllm_loader.py",
            include_str!("../../../../../../torch-server/loaders/dllm_loader.py"),
        ),
        (
            "loaders/safetensors_loader.py",
            include_str!("../../../../../../torch-server/loaders/safetensors_loader.py"),
        ),
        (
            "loaders/sherry_loader.py",
            include_str!("../../../../../../torch-server/loaders/sherry_loader.py"),
        ),
        ("requirements.txt", lock),
    ] {
        let path = destination.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(PumasError::from)?;
        }
        std::fs::write(path, contents).map_err(PumasError::from)?;
    }
    let recipe = serde_json::json!({
        "recipe_id": TORCH_291.recipe_id,
        "protocol": SUPPORTED_TORCH_PROTOCOL,
        "capabilities": [TORCH_IMAGE_GENERATION_CAPABILITY],
        "python": "3.12",
        "platform": "linux-x86_64",
    });
    std::fs::write(
        destination.join("runtime.json"),
        serde_json::to_vec_pretty(&recipe)
            .map_err(|e| failed(format!("Cannot serialize Torch recipe: {e}")))?,
    )
    .map_err(PumasError::from)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeRecipe {
    recipe_id: String,
    protocol: u32,
    capabilities: Vec<String>,
    python: String,
    platform: String,
}

fn failed(message: impl Into<String>) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

impl VersionInstaller {
    async fn stage_resolved_torch_runtime(
        &self,
        tag: &str,
        staging: &Path,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        let version = stable_torch_tag(tag).ok_or_else(|| failed("Invalid stable Torch tag"))?;
        let build = std::env::var("PUMAS_TORCH_BUILD").unwrap_or_else(|_| "cpu".to_string());
        if !matches!(
            build.as_str(),
            "cpu"
                | "cu118"
                | "cu121"
                | "cu124"
                | "cu126"
                | "cu128"
                | "cu130"
                | "rocm6.1"
                | "rocm6.2"
                | "rocm6.3"
                | "rocm6.4"
                | "rocm7.0"
                | "rocm7.1"
        ) {
            return Err(failed(
                "Unknown PUMAS_TORCH_BUILD; use an official CPU, CUDA, or ROCm build",
            ));
        }
        let runtime = staging.join("runtime");
        let runtime_for_write = runtime.clone();
        tokio::task::spawn_blocking(move || write_embedded_torch_runtime(&runtime_for_write))
            .await
            .map_err(|e| failed(format!("Runtime staging task failed: {e}")))??;
        fs::remove_file(runtime.join("validate_runtime.py"))
            .await
            .map_err(PumasError::from)?;
        let mut failures = Vec::new();
        let mut resolved = false;
        for interpreter in ["python3.13", "python3.12", "python3.11", "python3.10"] {
            self.check_cancelled()?;
            if Command::new(interpreter)
                .arg("--version")
                .output()
                .await
                .is_err()
            {
                continue;
            }
            let _ = fs::remove_dir_all(runtime.join("venv")).await;
            let mut venv = Command::new(interpreter);
            venv.args(["-I", "-m", "venv"]).arg(runtime.join("venv"));
            if let Err(error) = self
                .run_runtime_command(
                    venv,
                    log_path,
                    &format!("Creating Python environment with {interpreter}"),
                    progress_tx,
                )
                .await
            {
                self.check_cancelled()?;
                failures.push(format!("{interpreter}: {error}"));
                continue;
            }
            let python = runtime.join("venv/bin/python");
            let mut resolve = Command::new(&python);
            resolve
                .arg(runtime.join("resolve_runtime.py"))
                .args(["--version", version, "--build", &build])
                .arg("--output")
                .arg(&runtime);
            match self
                .run_runtime_command(
                    resolve,
                    log_path,
                    &format!(
                        "Resolving official Torch {version} {build} wheels with {interpreter}"
                    ),
                    progress_tx,
                )
                .await
            {
                Ok(()) => {
                    resolved = true;
                    break;
                }
                Err(error) => {
                    self.check_cancelled()?;
                    if error.to_string().contains("exit status: 75") {
                        return Err(failed("Official wheel indexes are unreachable; artifact availability is inconclusive. Retry with network access. See installation log."));
                    }
                    failures.push(format!("{interpreter}: {error}"));
                }
            }
        }
        if !resolved {
            return Err(failed(format!("No official {build} binary combination for Torch {version} with installed Python interpreters. Interpreter provisioning and source builds are not supported. Try another build or install a compatible Python interpreter. Attempts: {}", failures.join("; "))));
        }
        let recipe = serde_json::json!({
            "recipe_id": format!("upstream-auto-{tag}-{build}"),
            "protocol": SUPPORTED_TORCH_PROTOCOL,
            "capabilities": [TORCH_IMAGE_GENERATION_CAPABILITY],
            "qualification": "not verified by Pumas",
            "build": build,
        });
        fs::write(
            runtime.join("runtime.json"),
            serde_json::to_vec_pretty(&recipe).map_err(|e| failed(e.to_string()))?,
        )
        .await
        .map_err(PumasError::from)?;
        let python = runtime.join("venv/bin/python");
        let mut install = Command::new(&python);
        install
            .args([
                "-I",
                "-m",
                "pip",
                "--isolated",
                "install",
                "--no-deps",
                "--require-hashes",
                "--only-binary=:all:",
                "--disable-pip-version-check",
                "-r",
            ])
            .arg(runtime.join("requirements.txt"))
            .arg("--cache-dir")
            .arg(self.launcher_root.join("launcher-data/cache/pip"));
        self.run_runtime_command(
            install,
            log_path,
            "Installing resolved wheel artifacts",
            progress_tx,
        )
        .await?;
        let mut probe = Command::new(&python);
        probe.arg(runtime.join("probe_runtime.py"));
        self.run_runtime_command(
            probe,
            log_path,
            "Checking installed Torch identity and CPU operation",
            progress_tx,
        )
        .await?;
        Ok(runtime)
    }

    pub(super) async fn install_torch_runtime(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        let _attempt = self
            .torch_attempt_lock
            .try_lock()
            .map_err(|_| failed("Torch installation already active"))?;
        self.torch_control.start();
        let result = self
            .install_torch_runtime_inner(tag, release, progress_tx)
            .await;
        self.torch_control.finish();
        result
    }

    async fn install_torch_runtime_inner(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(failed("Managed Torch requires Linux x86_64"));
        }
        if tag != release.tag_name || !is_torch_runtime_release(release) {
            return Err(failed(
                "Unsupported upstream PyTorch release or mismatched tag",
            ));
        }
        let recipe = torch_recipe_for_tag(tag);
        if !tag
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._".contains(&c))
        {
            return Err(failed("Invalid Torch runtime version tag"));
        }
        let destination = self.versions_dir().join(tag);
        if path_exists(&destination).await? {
            return Err(failed(
                "Runtime directory already exists; refusing to replace existing files",
            ));
        }
        fs::create_dir_all(self.versions_dir())
            .await
            .map_err(PumasError::from)?;
        // Staging shares the publication filesystem; failed attempts never enter
        // installed-version state. TempDir removes this attempt on every exit.
        let staging = tempfile::Builder::new()
            .prefix(".torch-install-")
            .tempdir_in(self.versions_dir())
            .map_err(PumasError::from)?;
        let logs = self.logs_dir();
        fs::create_dir_all(&logs).await.map_err(PumasError::from)?;
        let log_path = logs.join(format!(
            "install-{}-{}.log",
            tag,
            Utc::now().timestamp_millis()
        ));
        self.progress_tracker.write().await.start_installation(
            tag,
            None,
            None,
            Some(log_path.to_string_lossy().as_ref()),
        );
        let result = self
            .stage_torch_runtime(tag, recipe, staging.path(), &log_path, &progress_tx)
            .await;
        #[cfg(test)]
        if result.is_ok() && self.torch_stage_override.is_some() {
            if let Some(pause) = &self.torch_stage_pause {
                pause.reached.notify_one();
                pause
                    .resume
                    .acquire()
                    .await
                    .map_err(|e| failed(format!("Staging pause failed: {e}")))?
                    .forget();
            }
        }
        let result = async {
            match result {
                Ok(runtime) => {
                    self.check_cancelled()?;
                    if !self.torch_control.try_begin_publication() {
                        return Err(failed(
                            "Torch installation was cancelled before publication",
                        ));
                    }
                    #[cfg(test)]
                    if let Some(pause) = &self.torch_publication_pause {
                        pause.reached.notify_one();
                        pause
                            .resume
                            .acquire()
                            .await
                            .map_err(|e| failed(format!("Publication pause failed: {e}")))?
                            .forget();
                    }
                    let publish_to = destination.clone();
                    tokio::task::spawn_blocking(move || {
                        pumas_library::platform::filesystem::rename_directory_noreplace(
                            &runtime,
                            &publish_to,
                        )
                    })
                    .await
                    .map_err(|error| failed(format!("Runtime publication task failed: {error}")))?
                    .map_err(PumasError::from)?;
                    let result = self
                        .finalize_installation(tag, release, &destination, &progress_tx)
                        .await;
                    if result.is_err() {
                        // Only our newly published directory is compensated.
                        fs::remove_dir_all(&destination)
                            .await
                            .map_err(PumasError::from)?;
                    }
                    result
                }
                Err(error) => Err(error),
            }
        }
        .await;
        let mut tracker = self.progress_tracker.write().await;
        if let Err(error) = &result {
            tracker.set_error(&error.to_string());
        }
        tracker.complete_installation(result.is_ok());
        result
    }

    async fn stage_torch_runtime(
        &self,
        tag: &str,
        recipe_spec: Option<&TorchRuntimeRecipe>,
        staging: &Path,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        #[cfg(test)]
        if let Some(stage) = &self.torch_stage_override {
            return stage(staging);
        }
        if recipe_spec.is_none() {
            return self
                .stage_resolved_torch_runtime(tag, staging, log_path, progress_tx)
                .await;
        }
        let recipe_spec = recipe_spec.expect("checked above");
        let runtime = staging.join("runtime");
        let runtime_for_write = runtime.clone();
        tokio::task::spawn_blocking(move || write_embedded_torch_runtime(&runtime_for_write))
            .await
            .map_err(|e| failed(format!("Runtime staging task failed: {e}")))??;
        self.check_cancelled()?;
        let recipe: RuntimeRecipe = serde_json::from_slice(
            &fs::read(runtime.join("runtime.json"))
                .await
                .map_err(PumasError::from)?,
        )
        .map_err(|e| failed(format!("Invalid runtime recipe: {e}")))?;
        if recipe.recipe_id != recipe_spec.recipe_id {
            return Err(failed("Embedded Torch recipe identity mismatch"));
        }
        if recipe.protocol != SUPPORTED_TORCH_PROTOCOL {
            return Err(failed(format!(
                "Runtime recipe protocol {} does not match required protocol {SUPPORTED_TORCH_PROTOCOL}",
                recipe.protocol
            )));
        }
        if !recipe
            .capabilities
            .iter()
            .any(|capability| capability == TORCH_IMAGE_GENERATION_CAPABILITY)
        {
            return Err(failed(format!(
                "Runtime recipe is missing required capability {TORCH_IMAGE_GENERATION_CAPABILITY}"
            )));
        }
        if recipe.python != "3.12" || recipe.platform != "linux-x86_64" {
            return Err(failed(
                "Runtime recipe does not match Python 3.12 on linux-x86_64",
            ));
        }
        let mut python_check = Command::new("python3.12");
        python_check.args([
            "-I",
            "-c",
            "import sys; assert sys.version_info[:2] == (3,12)",
        ]);
        self.run_runtime_command(python_check, log_path, "Checking Python 3.12", progress_tx)
            .await?;
        let mut venv = Command::new("python3.12");
        venv.args(["-I", "-m", "venv"]).arg(runtime.join("venv"));
        self.run_runtime_command(venv, log_path, "Creating managed environment", progress_tx)
            .await?;
        let python = runtime.join("venv/bin/python");
        let mut install = Command::new(&python);
        install
            .args([
                "-I",
                "-m",
                "pip",
                "--isolated",
                "install",
                "--require-hashes",
                "--only-binary=:all:",
                "--disable-pip-version-check",
                "-r",
            ])
            .arg(runtime.join("requirements.txt"))
            .arg("--cache-dir")
            .arg(self.launcher_root.join("launcher-data/cache/pip"));
        self.run_runtime_command(
            install,
            log_path,
            "Installing locked runtime dependencies",
            progress_tx,
        )
        .await?;
        let mut validate = Command::new(&python);
        validate
            .arg(runtime.join("validate_runtime.py"))
            .current_dir(&runtime)
            .env("HF_HUB_OFFLINE", "1")
            .env("PYTHONNOUSERSITE", "1");
        self.run_runtime_command(
            validate,
            log_path,
            "Validating GPU and sidecar protocol",
            progress_tx,
        )
        .await?;
        Ok(runtime)
    }

    pub(super) async fn run_runtime_command(
        &self,
        mut command: Command,
        log_path: &Path,
        stage: &str,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        self.check_cancelled()?;
        self.progress_tracker.write().await.update_stage(
            InstallationStage::Setup,
            0.0,
            Some(stage),
        );
        let _ = progress_tx.try_send(ProgressUpdate::Setup {
            message: stage.to_string(),
        });
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(PumasError::from)?;
        command
            .stdout(Stdio::from(log.try_clone().map_err(PumasError::from)?))
            .stderr(Stdio::from(log))
            .kill_on_drop(true);
        #[cfg(not(target_os = "linux"))]
        return Err(failed("Managed Torch process supervision requires Linux"));
        #[cfg(target_os = "linux")]
        {
            use pumas_library::platform::linux_group;
            linux_group::ensure_supported().map_err(PumasError::from)?;
            command.process_group(0);
            let mut child = command
                .spawn()
                .map_err(|error| failed(format!("{stage}: {error}")))?;
            let pid = child
                .id()
                .ok_or_else(|| failed("Installer process has no owned PID"))?;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(3600);
            loop {
                // Observe without reaping: the leader pins the numeric PGID until
                // every cooperating child (including a GPU health probe) is dead.
                let observed = linux_group::observe_exit(pid);
                let cancelled = self.cancel_flag.load(Ordering::SeqCst);
                let timed_out = tokio::time::Instant::now() >= deadline;
                if cancelled || timed_out || !matches!(&observed, Ok(None)) {
                    finish_process_group(&mut child, pid).await?;
                    self.check_cancelled()?;
                    if timed_out {
                        return Err(failed(format!(
                            "{stage} exceeded the installation deadline"
                        )));
                    }
                    let status = observed
                        .map_err(PumasError::from)?
                        .ok_or_else(|| failed("Missing terminal installer status"))?;
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(failed(format!(
                            "{stage} failed ({status}); see installation log"
                        )))
                    };
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

#[cfg(target_os = "linux")]
async fn finish_process_group(child: &mut tokio::process::Child, pid: u32) -> Result<()> {
    use pumas_library::platform::linux_group;
    let group = i32::try_from(pid).map_err(|_| failed("Invalid installer process ID"))?;
    let mut first_failure = None;
    loop {
        let observation = match linux_group::signal_group(pid) {
            Ok(()) => {
                tokio::task::spawn_blocking(move || linux_group::group_has_live_members(group))
                    .await
                    .map_err(|error| failed(format!("Process-group observation failed: {error}")))
                    .and_then(|result| result.map_err(PumasError::from))
            }
            Err(error) => Err(PumasError::from(error)),
        };
        match observation {
            Ok(false) => break,
            Ok(true) => {}
            Err(error) => {
                if first_failure.is_none() {
                    warn!("Holding runtime staging until process cleanup completes: {error}");
                }
                first_failure.get_or_insert(error);
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // Reaping is deliberately last; no signal may use this PID afterwards.
    child.wait().await.map_err(PumasError::from)?;
    match first_failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
