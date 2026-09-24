//! Managed upstream PyTorch installation; lifecycle and state remain in VersionManager.

use super::*;
use crate::torch_client::{SUPPORTED_TORCH_PROTOCOL, TORCH_IMAGE_GENERATION_CAPABILITY};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const MAX_TORCH_ORPHAN_QUARANTINES: usize = 2;
pub(super) const TORCH_PUBLISHING_MARKER: &[u8] = b"metadata pending";

struct TorchOrphanQuarantine {
    tag: String,
    timestamp_ms: u64,
    path: PathBuf,
}

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
    #[cfg(test)]
    pub(crate) release_tag: &'static str,
    pub(crate) recipe_id: &'static str,
    pub(crate) torch_version: &'static str,
    pub(crate) torch_wheel_url: &'static str,
    pub(crate) torch_wheel_sha256: &'static str,
    pub(crate) torchvision_wheel_url: &'static str,
    pub(crate) torchvision_wheel_sha256: &'static str,
}

const TORCH_291: TorchRuntimeRecipe = TorchRuntimeRecipe {
    #[cfg(test)]
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

fn list_owned_torch_orphan_quarantines(
    versions_dir: &Path,
) -> std::io::Result<Vec<TorchOrphanQuarantine>> {
    let entries = match std::fs::read_dir(versions_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut quarantines = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(suffix) = name
            .to_str()
            .and_then(|name| name.strip_prefix(".torch-orphan-"))
        else {
            continue;
        };
        let Some((tag, timestamp)) = suffix.rsplit_once('-') else {
            continue;
        };
        let (Some(_), Ok(timestamp_ms)) = (stable_torch_tag(tag), timestamp.parse::<u64>()) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if !matches!(
            std::fs::read(path.join(".pumas-publishing")),
            Ok(contents) if contents.as_slice() == TORCH_PUBLISHING_MARKER
        ) {
            continue;
        }
        quarantines.push(TorchOrphanQuarantine {
            tag: tag.to_owned(),
            timestamp_ms,
            path,
        });
    }
    Ok(quarantines)
}

pub(super) fn prune_torch_orphan_quarantines(
    versions_dir: &Path,
    max_keep: usize,
    only_tag: Option<&str>,
) -> std::io::Result<()> {
    let mut quarantines = list_owned_torch_orphan_quarantines(versions_dir)?;
    quarantines.retain(|quarantine| only_tag.is_none_or(|tag| quarantine.tag == tag));
    quarantines.sort_by(|left, right| right.timestamp_ms.cmp(&left.timestamp_ms));
    for quarantine in quarantines.into_iter().skip(max_keep) {
        if let Err(error) = std::fs::remove_dir_all(quarantine.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error);
            }
        }
    }
    Ok(())
}

pub(super) fn schedule_torch_orphan_prune(
    cleanup: &TorchCleanupTasks,
    versions_dir: &Path,
    max_keep: usize,
    only_tag: Option<String>,
    phase: &'static str,
) {
    let versions_dir = versions_dir.to_owned();
    cleanup.schedule(move || {
        if let Err(error) =
            prune_torch_orphan_quarantines(&versions_dir, max_keep, only_tag.as_deref())
        {
            warn!(%error, phase, "Torch orphan cleanup failed");
        }
    });
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
        plan: &TorchInstallPlan,
        staging: &Path,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        let runtime = staging.join("runtime");
        let runtime_for_write = runtime.clone();
        tokio::task::spawn_blocking(move || write_embedded_torch_runtime(&runtime_for_write))
            .await
            .map_err(|e| failed(format!("Runtime staging task failed: {e}")))??;
        fs::remove_file(runtime.join("validate_runtime.py"))
            .await
            .map_err(PumasError::from)?;
        let observed_hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read(&plan.interpreter_path).map_err(PumasError::from)?)
        );
        if observed_hash != plan.interpreter_hash {
            return Err(failed("Selected Python executable changed after preview"));
        }
        let interpreter = &plan.interpreter_path;
        let mut venv = Command::new(interpreter);
        venv.args(["-I", "-m", "venv"]).arg(runtime.join("venv"));
        self.run_runtime_command(
            venv,
            log_path,
            &format!("Creating Python environment with {}", interpreter.display()),
            progress_tx,
        )
        .await?;
        fs::write(runtime.join("requirements.txt"), &plan.requirements)
            .await
            .map_err(PumasError::from)?;
        fs::write(runtime.join("resolution.json"), &plan.resolution)
            .await
            .map_err(PumasError::from)?;
        fs::write(runtime.join("pip-resolution.json"), &plan.report)
            .await
            .map_err(PumasError::from)?;
        let recipe = serde_json::json!({
            "recipe_id": format!("upstream-preview-{}-{}-{}-{}", plan.preview.tag, plan.preview.build, plan.preview.python, plan.preview.adapter),
            "protocol": SUPPORTED_TORCH_PROTOCOL,
            "capabilities": [TORCH_IMAGE_GENERATION_CAPABILITY],
            "qualification": "not verified by Pumas",
            "build": plan.preview.build,
            "python": plan.preview.python,
            "adapter": plan.preview.adapter,
            "artifacts": plan.preview.artifacts,
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
        plan: Option<TorchInstallPlan>,
    ) -> Result<()> {
        let _attempt = self
            .torch_attempt_lock
            .try_lock()
            .map_err(|_| failed("Torch installation already active"))?;
        self.torch_control.start();
        let result = self
            .install_torch_runtime_inner(tag, release, progress_tx, plan)
            .await;
        self.torch_control.finish();
        result
    }

    async fn install_torch_runtime_inner(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
        plan: Option<TorchInstallPlan>,
    ) -> Result<()> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(failed("Managed Torch requires Linux x86_64"));
        }
        if tag != release.tag_name || !is_torch_runtime_release(release) {
            return Err(failed(
                "Unsupported upstream PyTorch release or mismatched tag",
            ));
        }
        let recipe = if plan
            .as_ref()
            .is_none_or(|p| p.preview.qualification == "qualified")
        {
            torch_recipe_for_tag(tag)
        } else {
            None
        };
        if plan.as_ref().is_some_and(|p| p.preview.tag != tag) {
            return Err(failed("Torch preview tag mismatch"));
        }
        if !tag
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._".contains(&c))
        {
            return Err(failed("Invalid Torch runtime version tag"));
        }
        let versions_dir = self.versions_dir();
        fs::create_dir_all(&versions_dir)
            .await
            .map_err(PumasError::from)?;
        schedule_torch_orphan_prune(
            &self.torch_cleanup,
            &versions_dir,
            MAX_TORCH_ORPHAN_QUARANTINES,
            None,
            "before_install",
        );
        let destination = versions_dir.join(tag);
        if path_exists(&destination).await? {
            if path_exists(&destination.join(".pumas-publishing")).await?
                && self
                    .metadata_manager
                    .get_installed_version(tag, Some(AppId::Torch))?
                    .is_none()
            {
                let quarantine = versions_dir.join(format!(
                    ".torch-orphan-{tag}-{}",
                    Utc::now().timestamp_millis()
                ));
                let from = destination.clone();
                tokio::task::spawn_blocking(move || {
                    pumas_library::platform::filesystem::rename_directory_noreplace(
                        &from,
                        &quarantine,
                    )
                })
                .await
                .map_err(|e| failed(format!("Orphan recovery task failed: {e}")))?
                .map_err(PumasError::from)?;
                schedule_torch_orphan_prune(
                    &self.torch_cleanup,
                    &versions_dir,
                    MAX_TORCH_ORPHAN_QUARANTINES,
                    None,
                    "after_interrupted_publish_recovery",
                );
            } else {
                return Err(failed(
                    "Runtime directory already exists; refusing to replace existing files",
                ));
            }
        }
        if recipe.is_none() && plan.is_none() {
            #[cfg(test)]
            if self.torch_stage_override.is_none() {
                return Err(failed(
                    "A retained preview is required for this Torch runtime",
                ));
            }
            #[cfg(not(test))]
            return Err(failed(
                "A retained preview is required for this Torch runtime",
            ));
        }
        // Staging shares the publication filesystem; failed attempts never enter
        // installed-version state. TempDir removes this attempt on every exit.
        let staging = tempfile::Builder::new()
            .prefix(".torch-install-")
            .tempdir_in(&versions_dir)
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
            .stage_torch_runtime(
                tag,
                recipe,
                plan.as_ref(),
                staging.path(),
                &log_path,
                &progress_tx,
            )
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
                    fs::write(runtime.join(".pumas-publishing"), b"metadata pending")
                        .await
                        .map_err(PumasError::from)?;
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
                    if result.is_ok() {
                        if let Err(error) =
                            fs::remove_file(destination.join(".pumas-publishing")).await
                        {
                            warn!(
                                "Installed Torch publication marker could not be removed: {error}"
                            );
                        }
                        schedule_torch_orphan_prune(
                            &self.torch_cleanup,
                            &versions_dir,
                            0,
                            Some(tag.to_owned()),
                            "after_successful_install",
                        );
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
        _tag: &str,
        recipe_spec: Option<&TorchRuntimeRecipe>,
        plan: Option<&TorchInstallPlan>,
        staging: &Path,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        #[cfg(test)]
        if let Some(stage) = &self.torch_stage_override {
            return stage(staging);
        }
        if let Some(plan) = plan.filter(|p| p.preview.qualification != "qualified") {
            return self
                .stage_resolved_torch_runtime(plan, staging, log_path, progress_tx)
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
        if let Some(plan) = plan {
            let observed_hash = format!(
                "{:x}",
                Sha256::digest(std::fs::read(&plan.interpreter_path).map_err(PumasError::from)?)
            );
            if observed_hash != plan.interpreter_hash {
                return Err(failed("Selected Python executable changed after preview"));
            }
        }
        let interpreter = plan
            .map(|p| p.interpreter_path.as_path())
            .unwrap_or_else(|| Path::new("python3.12"));
        let mut python_check = Command::new(interpreter);
        python_check.args([
            "-I",
            "-c",
            "import sys; assert sys.version_info[:2] == (3,12)",
        ]);
        self.run_runtime_command(python_check, log_path, "Checking Python 3.12", progress_tx)
            .await?;
        let mut venv = Command::new(interpreter);
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
        let resolution = serde_json::json!({
            "torch": recipe_spec.torch_version,
            "build": "cu130",
            "python": "3.12",
            "adapter": "bundled",
            "artifacts": plan.map(|p| p.preview.artifacts.clone()).unwrap_or_default(),
        });
        fs::write(
            runtime.join("resolution.json"),
            serde_json::to_vec_pretty(&resolution).map_err(|e| failed(e.to_string()))?,
        )
        .await
        .map_err(PumasError::from)?;
        let mut probe = Command::new(&python);
        probe.arg(runtime.join("probe_runtime.py"));
        self.run_runtime_command(
            probe,
            log_path,
            "Recording core and adapter probe evidence",
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
