//! Managed Torch bundle installation; lifecycle and state remain in VersionManager.

use super::*;
use crate::torch_client::SUPPORTED_TORCH_PROTOCOL;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const ARCHIVE: &str = "pumas-torch-runtime-linux-x86_64.tar.gz";
const CHECKSUM: &str = "pumas-torch-runtime-linux-x86_64.tar.gz.sha256";

pub(crate) fn is_torch_runtime_release(release: &GitHubRelease) -> bool {
    release.tag_name.starts_with("torch-runtime-")
        && release.assets.iter().any(|asset| asset.name == ARCHIVE)
        && release.assets.iter().any(|asset| asset.name == CHECKSUM)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeRecipe {
    recipe_id: String,
    protocol: u32,
    python: String,
    platform: String,
}

fn failed(message: impl Into<String>) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

impl VersionInstaller {
    pub(super) async fn install_torch_runtime(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(failed("Managed Torch requires Linux x86_64"));
        }
        if !is_torch_runtime_release(release) || tag != release.tag_name {
            return Err(failed("Not a Pumas Torch runtime release; legacy PyTorch source installs cannot serve models"));
        }
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
            release.total_size,
            None,
            Some(log_path.to_string_lossy().as_ref()),
        );
        let result = self
            .stage_torch_runtime(tag, release, staging.path(), &log_path, &progress_tx)
            .await;
        let result = async {
            match result {
                Ok(runtime) => {
                    self.check_cancelled()?;
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
        release: &GitHubRelease,
        staging: &Path,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        let archive = release
            .assets
            .iter()
            .find(|a| a.name == ARCHIVE)
            .ok_or_else(|| failed("Missing runtime archive"))?;
        let checksum = release
            .assets
            .iter()
            .find(|a| a.name == CHECKSUM)
            .ok_or_else(|| failed("Missing runtime checksum"))?;
        let archive_path = staging.join(ARCHIVE);
        let checksum_path = staging.join(CHECKSUM);
        self.download_torch_asset(&checksum.download_url, &checksum_path, progress_tx)
            .await?;
        let checksum = fs::read_to_string(checksum_path)
            .await
            .map_err(PumasError::from)?;
        let expected = checksum
            .split_whitespace()
            .next()
            .filter(|value| value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| failed("Malformed runtime SHA-256 checksum"))?
            .to_ascii_lowercase();
        self.download_torch_asset(&archive.download_url, &archive_path, progress_tx)
            .await?;
        self.check_cancelled()?;
        let archive_for_hash = archive_path.clone();
        let actual = tokio::task::spawn_blocking(move || -> Result<String> {
            let mut file = File::open(archive_for_hash).map_err(PumasError::from)?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut hasher).map_err(PumasError::from)?;
            Ok(format!("{:x}", hasher.finalize()))
        })
        .await
        .map_err(|e| failed(format!("Runtime checksum task failed: {e}")))??;
        if actual != expected {
            return Err(failed("Runtime archive checksum mismatch"));
        }
        let runtime = staging.join("runtime");
        fs::create_dir_all(&runtime)
            .await
            .map_err(PumasError::from)?;
        // Do not accept symlinks, special files or path traversal in a runtime
        // bundle, even when its transport checksum matches.
        let extract_to = runtime.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let file = File::open(archive_path).map_err(PumasError::from)?;
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
            for entry in archive.entries().map_err(PumasError::from)? {
                let mut entry = entry.map_err(PumasError::from)?;
                let kind = entry.header().entry_type();
                if !kind.is_file() && !kind.is_dir() {
                    return Err(failed("Runtime bundle contains a non-regular entry"));
                }
                if !entry.unpack_in(&extract_to).map_err(PumasError::from)? {
                    return Err(failed("Runtime bundle path escapes installation directory"));
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| failed(format!("Runtime extraction task failed: {e}")))??;
        let recipe: RuntimeRecipe = serde_json::from_slice(
            &fs::read(runtime.join("runtime.json"))
                .await
                .map_err(PumasError::from)?,
        )
        .map_err(|e| failed(format!("Invalid runtime recipe: {e}")))?;
        // Each qualification dimension fails with its owning reason so artifact
        // identity, recipe identity, recipe protocol, and environment are
        // verified separately. Recipe-to-sidecar agreement (bundled
        // handshake protocol/capabilities) is checked by the bundled
        // validation below; client-to-live-sidecar agreement is owned by
        // `TorchClient` handshake verification, not by installation.
        if recipe.recipe_id != tag {
            return Err(failed(format!(
                "Runtime recipe identity '{}' does not match release tag '{tag}'",
                recipe.recipe_id
            )));
        }
        if recipe.protocol != SUPPORTED_TORCH_PROTOCOL {
            return Err(failed(format!(
                "Runtime recipe protocol {} does not match required protocol {SUPPORTED_TORCH_PROTOCOL}",
                recipe.protocol
            )));
        }
        if recipe.python != "3.12" || recipe.platform != "linux-x86_64" {
            return Err(failed(
                "Runtime recipe does not match Python 3.12 on linux-x86_64",
            ));
        }
        for required in ["serve.py", "validate_runtime.py", "requirements.txt"] {
            if !path_exists(&runtime.join(required)).await? {
                return Err(failed(format!("Runtime bundle missing {required}")));
            }
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

    async fn download_torch_asset(
        &self,
        url: &str,
        path: &Path,
        progress: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        tokio::select! {
            result = self.download_archive(url, path, progress) => result,
            _ = async {
                while !self.cancel_flag.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            } => self.check_cancelled(),
            _ = tokio::time::sleep(Duration::from_secs(3600)) => Err(failed("Runtime artifact download deadline exceeded")),
        }
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
