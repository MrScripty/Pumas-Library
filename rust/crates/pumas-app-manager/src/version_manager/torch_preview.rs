//! Torch install selections retained until installation or expiry, and the
//! local install choices and identity checks around them.

use super::torch_inspection::{
    TorchInstalledProbeReport, TorchInstalledRuntime, TorchPythonChoice, TorchRuntimeOptions,
};
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use tokio::process::Command;

pub(super) const BUILDS: &[&str] = &[
    "cpu",
    "cu75",
    "cu80",
    "cu90",
    "cu91",
    "cu92",
    "cu100",
    "cu101",
    "cu102",
    "cu110",
    "cu111",
    "cu113",
    "cu115",
    "cu116",
    "cu117",
    "cu118",
    "cu121",
    "cu124",
    "cu126",
    "cu128",
    "cu129",
    "cu130",
    "cu132",
    "cu134",
    "rocm3.7",
    "rocm3.8",
    "rocm3.10",
    "rocm4.0.1",
    "rocm4.1",
    "rocm4.2",
    "rocm4.3.1",
    "rocm4.5.2",
    "rocm5.0",
    "rocm5.1.1",
    "rocm5.2",
    "rocm5.3",
    "rocm5.4.2",
    "rocm5.5",
    "rocm5.6",
    "rocm5.7",
    "rocm6.0",
    "rocm6.1",
    "rocm6.2",
    "rocm6.2.4",
    "rocm6.3",
    "rocm6.4",
    "rocm7.0",
    "rocm7.1",
    "rocm7.2",
    "rocm7.14",
];
const ADAPTERS: &[&str] = &["none", "flux2", "nunchaku", "cohere-asr"];

/// An adapter whose own binary wheel was built for one selection.
struct AdapterRequirement {
    adapter: &'static str,
    tag: &'static str,
    build: &'static str,
    python: &'static str,
    /// Why another selection is refused.
    refusal: &'static str,
}

/// Nunchaku 1.2.0 publishes one wheel, for Torch 2.9 on CPython 3.12 Linux
/// x86_64; the resolver installs it on the 2.9.1 cu130 build. A test checks
/// this against `torch-server/resolve_runtime.py`, which owns the wheel.
const ADAPTER_REQUIREMENTS: &[AdapterRequirement] = &[AdapterRequirement {
    adapter: "nunchaku",
    tag: "v2.9.1",
    build: "cu130",
    python: "python3.12",
    refusal: "The Nunchaku adapter's wheel installs only on Torch v2.9.1 with the cu130 build and Python 3.12.",
}];

fn adapter_requirement(adapter: &str) -> Option<&'static AdapterRequirement> {
    ADAPTER_REQUIREMENTS
        .iter()
        .find(|requirement| requirement.adapter == adapter)
}
pub(super) const PREVIEW_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_RETAINED_TORCH_SELECTIONS: usize = 32;

pub(super) fn valid_torch_channel(build: &str) -> bool {
    if build == "cpu" {
        return true;
    }
    if let Some(cuda) = build.strip_prefix("cu") {
        return !cuda.is_empty() && cuda.bytes().all(|byte| byte.is_ascii_digit());
    }
    if let Some(rocm) = build.strip_prefix("rocm") {
        let parts: Vec<_> = rocm.split('.').collect();
        return parts.len() >= 2
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
    }
    false
}

fn supported_torch_build(build: &str) -> bool {
    match std::env::consts::OS {
        "linux" => cfg!(target_arch = "x86_64") && valid_torch_channel(build),
        "windows" => {
            cfg!(all(target_arch = "x86_64", target_env = "msvc"))
                && (build == "cpu" || build.starts_with("cu") && valid_torch_channel(build))
        }
        "macos" => cfg!(target_arch = "aarch64") && build == "cpu",
        _ => false,
    }
}

fn is_python_selection(python: &str) -> bool {
    let Some(version) = python.strip_prefix("python3.") else {
        return false;
    };
    !version.is_empty()
        && version.bytes().all(|byte| byte.is_ascii_digit())
        && version.parse::<u32>().is_ok_and(|minor| minor >= 10)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchArtifact {
    pub name: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn builds_cover_the_upstream_cuda_and_rocm_range() {
        assert!(BUILDS.contains(&"cu75"));
        assert!(BUILDS.contains(&"cu134"));
        assert!(BUILDS.contains(&"rocm3.7"));
        assert!(BUILDS.contains(&"rocm7.14"));
        assert!(!BUILDS.contains(&"xpu"));
        assert!(
            BUILDS.iter().position(|build| *build == "rocm7.2")
                < BUILDS.iter().position(|build| *build == "rocm7.14")
        );
    }

    #[tokio::test]
    async fn nunchaku_is_refused_before_provisioning_outside_its_wheel_selection() {
        let root = tempfile::tempdir().unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        for (tag, build, python) in [
            ("v2.14.0", "cu130", "python3.12"),
            ("v2.9.1", "auto", "python3.12"),
            ("v2.9.1", "cpu", "python3.12"),
            ("v2.9.1", "cu130", "auto"),
            ("v2.9.1", "cu130", "python3.13"),
        ] {
            let outcome = manager
                .preview_torch_runtime(tag, build, python, "nunchaku")
                .await
                .unwrap();
            let TorchPreviewOutcome::Rejected { reason, message } = outcome else {
                panic!("{tag} {build} {python} was not refused");
            };
            assert_eq!(reason, TorchPreviewRejectionReason::Unsupported);
            assert!(message.contains("v2.9.1"), "{message}");
        }
        assert!(manager.torch_install_selections.lock().await.is_empty());
        assert!(!root.path().join("launcher-data/managed-python").exists());
        let ready = manager
            .preview_torch_runtime("v2.9.1", "cu130", "python3.12", "nunchaku")
            .await
            .unwrap();
        assert!(matches!(ready, TorchPreviewOutcome::Ready { .. }));
        let options = manager.torch_runtime_options().await.unwrap();
        assert_eq!(options.adapter_requirements.len(), 1);
        assert_eq!(options.adapter_requirements[0].tag, "v2.9.1");
    }

    #[test]
    fn nunchaku_requirement_matches_the_resolver_that_owns_its_wheel() {
        let resolver = include_str!("../../../../../torch-server/resolve_runtime.py");
        let requirement = adapter_requirement("nunchaku").unwrap();
        let version = requirement.tag.trim_start_matches('v');
        let minor = requirement.python.trim_start_matches("python");
        assert!(resolver.contains(&format!("version == \"{version}\"")));
        assert!(resolver.contains(&format!("build == \"{}\"", requirement.build)));
        assert!(resolver.contains(&format!(
            "sys.version_info[:2] == ({})",
            minor.replace('.', ", ")
        )));
        let torch_minor = version.rsplit_once('.').unwrap().0;
        let abi = minor.replace('.', "");
        assert!(resolver.contains(&format!(
            "%2Btorch{torch_minor}-cp{abi}-cp{abi}-linux_x86_64.whl"
        )));
    }

    #[tokio::test]
    async fn the_former_bundled_preset_is_not_an_adapter() {
        let root = tempfile::tempdir().unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        for python in ["python3.12", "auto"] {
            assert!(manager
                .preview_torch_runtime("v2.9.1", "cu130", python, "bundled")
                .await
                .is_err());
        }
        assert!(manager.torch_install_selections.lock().await.is_empty());
    }

    #[tokio::test]
    async fn cohere_asr_public_selection_is_exact_unverified_and_one_use() {
        let root = tempfile::tempdir().unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert!(manager
            .preview_torch_runtime("v2.3.1", "cpu", "python3.12", "cohere-asr")
            .await
            .is_err());
        assert!(manager.torch_install_selections.lock().await.is_empty());
        for (tag, build) in [("v2.10.0", "cpu"), ("v2.14.0", "cu134")] {
            let TorchPreviewOutcome::Ready { preview } = manager
                .preview_torch_runtime(tag, build, "python3.12", "cohere-asr")
                .await
                .unwrap()
            else {
                panic!("selection refused")
            };
            assert_eq!(preview.qualification, "unverified");
            assert!(preview.artifacts.is_empty());
            let selected = manager
                .consume_torch_install_selection(&preview.preview_id, tag)
                .await
                .unwrap();
            assert_eq!(selected.tag, tag);
            assert_eq!(selected.build, build);
            assert_eq!(selected.adapter, "cohere-asr");
            assert!(manager
                .consume_torch_install_selection(&preview.preview_id, tag)
                .await
                .is_err());
        }
        assert!(!root.path().join("launcher-data/managed-python").exists());
    }

    #[test]
    fn preview_accepts_canonical_future_channels_without_xpu_or_paths() {
        for build in ["cpu", "cu136", "cu999", "rocm8.0", "rocm8.0.1"] {
            assert!(valid_torch_channel(build), "{build}");
        }
        for build in ["cu", "rocm8", "rocm8..1", "cu13/6", "xpu", "cu13+foo"] {
            assert!(!valid_torch_channel(build), "{build}");
        }
        assert!(!BUILDS.contains(&"cu136"));
    }

    #[test]
    fn probe_context_comparison_identifies_each_changed_identity() {
        let saved = serde_json::json!({
            "hardware_fingerprint":"gpu-a",
            "installed_distributions":{"torch":"2.10.0"},
            "all_installed_distributions":{"torch":"2.10.0"},
            "distributions_sha256":"dist-a",
            "interpreter_sha256":"python-a",
            "driver_version":["590.0"],
        });
        let mut reasons = Vec::new();
        append_runtime_context_changes(&saved, &saved, &mut reasons);
        assert!(reasons.is_empty());
        let current = serde_json::json!({
            "hardware_fingerprint":"gpu-b",
            "installed_distributions":{"torch":"2.10.1"},
            "all_installed_distributions":{"torch":"2.10.1"},
            "distributions_sha256":"dist-b",
            "interpreter_sha256":"python-b",
            "driver_version":["591.0"],
        });
        append_runtime_context_changes(&saved, &current, &mut reasons);
        assert_eq!(reasons.len(), 5);
    }

    #[test]
    fn historical_file_checksums_do_not_make_probe_context_stale() {
        let saved = serde_json::json!({"interpreter_sha256":"old", "runtime_files_sha256":{"serve.py":"old"}});
        let current = serde_json::json!({"interpreter_sha256":"new", "runtime_files_sha256":{"serve.py":"new"}});
        let mut reasons = Vec::new();
        append_runtime_context_changes(&saved, &current, &mut reasons);
        assert!(reasons.is_empty());
    }

    #[tokio::test]
    async fn preview_resolver_deadline_kills_process_group() {
        let workspace = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let cleanup = super::installer::TorchCleanupTasks::default();
        let outcome =
            run_preview_resolver(command, &workspace, Duration::from_millis(25), &cleanup)
                .await
                .unwrap();
        assert!(matches!(&outcome, PreviewResolverRun::TimedOut));
    }

    #[tokio::test]
    async fn preview_resolver_cleans_child_after_leader_exits() {
        let workspace = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let mut command = Command::new("sh");
        command.args(["-c", "echo $$; sleep 30 &"]);
        let cleanup = super::installer::TorchCleanupTasks::default();
        let outcome = run_preview_resolver(command, &workspace, Duration::from_secs(2), &cleanup)
            .await
            .unwrap();
        let PreviewResolverRun::Exited(status) = outcome else {
            panic!("Resolver completed before the deadline");
        };
        assert!(status.success());
        let pid: i32 = std::fs::read_to_string(workspace.path().join("resolver.stdout"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(!pumas_library::platform::linux_group::group_has_live_members(pid).unwrap());
    }

    #[tokio::test]
    async fn cancelled_resolver_waiter_keeps_workspace_until_owned_drain() {
        let launcher_root = tempfile::tempdir().unwrap();
        let workspace =
            super::torch_workspace::create_launcher_workspace(launcher_root.path()).unwrap();
        let path = workspace.path().to_path_buf();
        let ready = path.join("ready");
        let cleanup = std::sync::Arc::new(super::installer::TorchCleanupTasks::default());
        let run_workspace = workspace.clone();
        let run_cleanup = cleanup.clone();
        let task = tokio::spawn(async move {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg("printf '%s\\n' \"$TMPDIR\" \"$TMP\" \"$TEMP\" > \"$1.tmp\"; mv \"$1.tmp\" \"$1\"; sleep 30")
                .arg("sh")
                .arg(&ready);
            run_preview_resolver(
                command,
                &run_workspace,
                Duration::from_secs(30),
                &run_cleanup,
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !path.join("ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let environment = std::fs::read_to_string(path.join("ready")).unwrap();
        let expected_tmp = path.join("tmp");
        let canonical_launcher_root = std::fs::canonicalize(launcher_root.path()).unwrap();
        assert!(path.starts_with(canonical_launcher_root.join("launcher-data/tmp/torch-preview")));
        assert_eq!(
            environment.lines().collect::<Vec<_>>(),
            vec![expected_tmp.to_str().unwrap(); 3]
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        drop(workspace);
        assert!(
            path.exists(),
            "workspace lease released before descendant drain"
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            while path.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("registered cleanup supervisor did not drain cancelled resolver");
        cleanup.drain().await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn healthy_long_resolver_does_not_block_another_preview_or_preflight() {
        let cleanup = std::sync::Arc::new(super::installer::TorchCleanupTasks::default());
        let long_workspace = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let ready = long_workspace.path().join("ready");
        let long_cleanup = cleanup.clone();
        let long_path = long_workspace.clone();
        let long_task = tokio::spawn(async move {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg("touch \"$1\"; sleep 30")
                .arg("sh")
                .arg(&ready);
            run_preview_resolver(command, &long_path, Duration::from_secs(30), &long_cleanup).await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !long_workspace.path().join("ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let short_workspace = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let mut short = Command::new("sh");
        short.args(["-c", "exit 0"]);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            run_preview_resolver(short, &short_workspace, Duration::from_secs(2), &cleanup),
        )
        .await
        .expect("healthy resolver blocked another preview")
        .unwrap();
        assert!(matches!(result, PreviewResolverRun::Exited(status) if status.success()));
        tokio::time::timeout(Duration::from_secs(1), cleanup.drain_residual_child_slots())
            .await
            .expect("healthy resolver blocked preflight")
            .unwrap();
        long_task.abort();
        assert!(long_task.await.unwrap_err().is_cancelled());
        cleanup.drain().await.unwrap();
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchPreview {
    pub preview_id: String,
    pub tag: String,
    pub build: String,
    pub python: String,
    pub adapter: String,
    pub artifacts: Vec<TorchArtifact>,
    pub qualification: String,
    pub expires_in_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TorchPreviewRejectionReason {
    Unsupported,
    ValidationFailed,
    NetworkInconclusive,
    Inconclusive,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TorchPreviewOutcome {
    Ready {
        preview: TorchPreview,
    },
    Rejected {
        reason: TorchPreviewRejectionReason,
        message: &'static str,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct TorchInstallSelection {
    pub tag: String,
    pub build: String,
    pub python: String,
    pub adapter: String,
    /// pip constraints (`name==version` lines) from a verified combination's
    /// record, present only when the user opted into its versions.
    pub verified_constraints: Option<String>,
    pub created: Instant,
}

pub(crate) type TorchInstallSelections = Arc<Mutex<HashMap<String, TorchInstallSelection>>>;

fn failed(message: impl Into<String>) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

fn preview_token() -> Result<String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes)
        .map_err(|error| failed(format!("Preview ID randomness unavailable: {error}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn append_runtime_context_changes(
    saved: &serde_json::Value,
    current: &serde_json::Value,
    reasons: &mut Vec<String>,
) {
    for (field, reason) in [
        ("hardware_fingerprint", "Hardware changed since the probe"),
        (
            "installed_distributions",
            "Installed distributions changed since the probe",
        ),
        (
            "all_installed_distributions",
            "Python environment changed since the probe",
        ),
        (
            "distributions_sha256",
            "Python distribution fingerprint changed since the probe",
        ),
        ("driver_version", "Driver context changed since the probe"),
    ] {
        if saved[field] != current[field] {
            reasons.push(reason.into());
        }
    }
}

fn is_known_legacy_torch_tag(tag: &str) -> bool {
    let version = tag
        .strip_prefix("torch-runtime-")
        .unwrap_or(tag)
        .trim_start_matches('v');
    matches!(version, "0.1.1" | "0.1.2" | "0.1.3" | "0.1.4")
}

fn safe_torch_tag(tag: &str) -> bool {
    tag != "."
        && tag != ".."
        && !tag.is_empty()
        && tag
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._".contains(&c))
}

#[derive(Debug)]
pub(super) enum PreviewResolverRun {
    Exited(std::process::ExitStatus),
    TimedOut,
}

pub(super) async fn run_preview_resolver(
    mut command: Command,
    workspace: &std::sync::Arc<tempfile::TempDir>,
    deadline: Duration,
    cleanup: &super::installer::TorchCleanupTasks,
) -> Result<PreviewResolverRun> {
    super::torch_workspace::configure_resolver_command(&mut command, workspace.path())?;
    let stdout = std::fs::File::create(workspace.path().join("resolver.stdout"))
        .map_err(PumasError::from)?;
    let stderr = std::fs::File::create(workspace.path().join("resolver.stderr"))
        .map_err(PumasError::from)?;
    command.stdout(stdout).stderr(stderr);
    let custody = cleanup.new_child_slot()?;
    let mut child = pumas_library::platform::managed_child::ManagedChild::spawn(
        command.as_std_mut(),
        custody.clone(),
    )
    .map_err(|e| failed(format!("Torch resolver could not start: {e}")))?;
    child.attach_cleanup_lease(workspace.clone());
    let until = tokio::time::Instant::now() + deadline;
    let (status, timed_out) = loop {
        let observed = child.observe_exit().map_err(PumasError::from)?;
        let timed_out = tokio::time::Instant::now() >= until;
        if observed.is_some() || timed_out {
            break (observed, timed_out);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    drop(child);
    cleanup.drain_child_slot(&custody).await.map_err(|_| {
        failed("Torch resolver cleanup incomplete; owned process cleanup remains pending")
    })?;
    if timed_out {
        return Ok(PreviewResolverRun::TimedOut);
    }
    status
        .map(PreviewResolverRun::Exited)
        .ok_or_else(|| failed("Torch preview resolver exited without status"))
}

impl VersionManager {
    /// Check that an installed Torch tag is safe to address and its interpreter
    /// still matches the recorded wheel identity before selection or launch.
    pub async fn verify_torch_identity(&self, tag: &str) -> Result<()> {
        self.verify_torch_manifest(tag).await?;
        self.check_torch_identity(tag).await
    }

    pub(crate) async fn verify_torch_manifest(&self, tag: &str) -> Result<()> {
        if self.app_id != AppId::Torch || !safe_torch_tag(tag) {
            return Err(PumasError::Config {
                message: "Invalid Torch version tag".into(),
            });
        }
        if !self.state.read().await.is_installed(tag) {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }
        self.expected_torch_version(tag).await.map(|_| ())
    }

    async fn expected_torch_version(&self, tag: &str) -> Result<String> {
        let runtime = self.versions_dir().join(tag);
        let legacy_version = tag
            .strip_prefix("torch-runtime-")
            .unwrap_or(tag)
            .trim_start_matches('v');
        let legacy_recipe = if matches!(legacy_version, "0.1.5" | "0.1.6") {
            fs::read(runtime.join("runtime.json"))
                .await
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_some_and(|recipe| {
                    recipe["recipe_id"] == format!("torch-runtime-{legacy_version}")
                })
        } else {
            false
        };
        if (is_known_legacy_torch_tag(tag) || legacy_recipe)
            && fs::try_exists(runtime.join("serve.py"))
                .await
                .map_err(PumasError::from)?
            && fs::try_exists(pumas_library::platform::paths::venv_python(&runtime))
                .await
                .map_err(PumasError::from)?
        {
            return Ok("legacy".into());
        }
        if !fs::try_exists(runtime.join("runtime.json"))
            .await
            .map_err(PumasError::from)?
        {
            #[cfg(test)]
            if self.torch_stage_override.is_some() {
                return Ok("test-fixture".into());
            }
            return Err(failed("Installed Torch identity manifest is missing"));
        }
        match fs::read(runtime.join("resolution.json")).await {
            Ok(bytes) => {
                let manifest: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|e| failed(format!("Installed Torch resolution is invalid: {e}")))?;
                Ok(manifest["torch"]
                    .as_str()
                    .ok_or_else(|| failed("Installed Torch identity is missing"))?
                    .to_string())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && tag == "v2.9.1" => {
                let recipe: serde_json::Value = serde_json::from_slice(
                    &fs::read(runtime.join("runtime.json"))
                        .await
                        .map_err(PumasError::from)?,
                )
                .map_err(|e| failed(format!("Installed Torch recipe is invalid: {e}")))?;
                if recipe["recipe_id"] != "torch-upstream-2.9.1-r1" {
                    return Err(failed("Installed Torch has no trusted identity manifest"));
                }
                Ok("2.9.1+cu130".to_string())
            }
            Err(error) => Err(failed(format!(
                "Installed Torch resolution is unavailable: {error}"
            ))),
        }
    }

    pub(crate) async fn check_torch_identity(&self, tag: &str) -> Result<()> {
        let runtime = self.versions_dir().join(tag);
        let expected = self.expected_torch_version(tag).await?;
        let mut command = Command::new(pumas_library::platform::paths::venv_python(&runtime));
        command.kill_on_drop(true).args([
            "-I",
            "-B",
            "-c",
            "import torch; print(torch.__version__)",
        ]);
        let output = tokio::time::timeout(Duration::from_secs(20), command.output())
            .await
            .map_err(|_| failed("Installed Torch identity check timed out"))?
            .map_err(|e| failed(format!("Installed Torch cannot start: {e}")))?;
        let observed = String::from_utf8_lossy(&output.stdout);
        if !output.status.success()
            || (expected == "legacy" && observed.trim().is_empty())
            || (expected != "legacy" && observed.trim() != expected)
        {
            return Err(failed(
                "Installed Torch identity does not match its resolved wheel",
            ));
        }
        Ok(())
    }

    pub async fn torch_runtime_options(&self) -> Result<TorchRuntimeOptions> {
        /// Artifacts the installer recorded in `resolution.json`; empty when
        /// that record is absent or unreadable.
        async fn installed_artifacts(version_dir: &Path) -> Vec<TorchArtifact> {
            fs::read(version_dir.join("resolution.json"))
                .await
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|resolution| serde_json::from_value(resolution["artifacts"].clone()).ok())
                .unwrap_or_default()
        }
        if self.app_id != AppId::Torch {
            return Err(failed("Torch manager required"));
        }
        let linux_x64 = cfg!(all(target_os = "linux", target_arch = "x86_64"));
        let adapters: &[&str] = if linux_x64 { ADAPTERS } else { &["none"] };
        let mut installed = Vec::new();
        for tag in self.get_installed_versions().await? {
            let recipe_path = self.versions_dir().join(&tag).join("runtime.json");
            let recipe = fs::read(&recipe_path)
                .await
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .unwrap_or(serde_json::Value::Null);
            let recorded = |key: &str| recipe[key].as_str().map(str::to_owned);
            // Installed by the former fixed v2.9.1 preset. The recipe records
            // no adapter; it carried both image adapters, so the Nunchaku record
            // (a superset of the Flux 2 one) is the one its packages can match.
            // The recipe ID alone qualifies nothing: only recorded artifacts can.
            installed.push(if recipe["recipe_id"] == "torch-upstream-2.9.1-r1" {
                let mut artifacts = installed_artifacts(&self.versions_dir().join(&tag)).await;
                if artifacts.is_empty() {
                    artifacts =
                        serde_json::from_value(recipe["artifacts"].clone()).unwrap_or_default();
                }
                let qualified = super::torch_verified::is_verified(
                    &tag,
                    "cu130",
                    "python3.12",
                    "nunchaku",
                    &artifacts,
                );
                TorchInstalledRuntime {
                    tag,
                    build: Some("cu130".into()),
                    python: Some("python3.12".into()),
                    adapter: Some("bundled".into()),
                    qualified,
                }
            } else {
                let (build, python, adapter) =
                    (recorded("build"), recorded("python"), recorded("adapter"));
                let qualified = match (&build, &python, &adapter) {
                    (Some(build), Some(python), Some(adapter)) => {
                        let artifacts = installed_artifacts(&self.versions_dir().join(&tag)).await;
                        super::torch_verified::is_verified(&tag, build, python, adapter, &artifacts)
                    }
                    _ => false,
                };
                TorchInstalledRuntime {
                    tag,
                    build,
                    python,
                    adapter,
                    qualified,
                }
            });
        }
        Ok(TorchRuntimeOptions {
            builds: BUILDS
                .iter()
                .copied()
                .filter(|build| supported_torch_build(build))
                .map(str::to_owned)
                .collect(),
            default_build: "auto".into(),
            pythons: vec![TorchPythonChoice {
                id: "auto".into(),
                label: "Select automatically".into(),
            }],
            adapters: adapters.iter().copied().map(str::to_owned).collect(),
            default_adapter: "none".into(),
            verified: super::torch_verified::verified_torch_runtimes(),
            adapter_requirements: ADAPTER_REQUIREMENTS
                .iter()
                .filter(|requirement| adapters.contains(&requirement.adapter))
                .map(
                    |requirement| super::torch_inspection::TorchAdapterRequirement {
                        adapter: requirement.adapter.into(),
                        tag: requirement.tag.into(),
                        build: requirement.build.into(),
                        python: requirement.python.into(),
                    },
                )
                .collect(),
            installed,
        })
    }

    pub async fn preview_torch_runtime(
        &self,
        tag: &str,
        build: &str,
        python: &str,
        adapter: &str,
    ) -> Result<TorchPreviewOutcome> {
        self.preview_torch_runtime_with_verified_versions(tag, build, python, adapter, false)
            .await
    }

    /// Preview a Torch selection, optionally constraining its packages to the
    /// versions a verified combination recorded.
    ///
    /// By default an install resolves current package versions: verified
    /// combinations are evidence, not pins. With `verified_versions`, the
    /// selection must match a verified combination exactly (no `auto` build or
    /// Python), and its resolution is constrained to that record's versions,
    /// so a successful install is again a version match for the record.
    pub async fn preview_torch_runtime_with_verified_versions(
        &self,
        tag: &str,
        build: &str,
        python: &str,
        adapter: &str,
        verified_versions: bool,
    ) -> Result<TorchPreviewOutcome> {
        if self.app_id != AppId::Torch
            || !(build == "auto" || supported_torch_build(build))
            || !(python == "auto" || is_python_selection(python))
            || !ADAPTERS.contains(&adapter)
            || !safe_torch_tag(tag)
            || super::torch_alternatives::stable_release_version(tag).is_none()
            || !cfg!(any(
                all(target_os = "linux", target_arch = "x86_64"),
                all(
                    target_os = "windows",
                    target_arch = "x86_64",
                    target_env = "msvc"
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))
        {
            return Err(failed("Invalid Torch installation selection"));
        }
        if adapter == "cohere-asr" {
            super::cohere_asr_profile::validate_selection(tag.trim_start_matches('v'))?;
            if !cfg!(target_os = "linux") {
                return Err(failed(
                    "ASR dependency profile is unavailable on this platform",
                ));
            }
        }
        // Refuse before any Python is provisioned, not after every attempt.
        if let Some(requirement) = adapter_requirement(adapter) {
            if !cfg!(all(target_os = "linux", target_arch = "x86_64"))
                || tag != requirement.tag
                || build != requirement.build
                || python != requirement.python
            {
                return Ok(TorchPreviewOutcome::Rejected {
                    reason: TorchPreviewRejectionReason::Unsupported,
                    message: requirement.refusal,
                });
            }
        }
        let verified_constraints = if verified_versions {
            match super::torch_verified::verified_constraints(tag, build, python, adapter) {
                Some(constraints) => Some(constraints),
                None => {
                    return Ok(TorchPreviewOutcome::Rejected {
                        reason: TorchPreviewRejectionReason::Unsupported,
                        message: "No verified combination matches this release, build, Python and adapter, so its recorded versions cannot be applied",
                    })
                }
            }
        } else {
            None
        };
        let id = preview_token()?;
        let selection = TorchInstallSelection {
            verified_constraints,
            tag: tag.to_owned(),
            build: build.to_owned(),
            python: python.to_owned(),
            adapter: adapter.to_owned(),
            created: Instant::now(),
        };
        {
            let mut selections = self.torch_install_selections.lock().await;
            selections.retain(|_, retained| retained.created.elapsed() < PREVIEW_TTL);
            while selections.len() >= MAX_RETAINED_TORCH_SELECTIONS {
                if let Some(oldest) = selections
                    .iter()
                    .min_by_key(|(_, retained)| retained.created)
                    .map(|(id, _)| id.clone())
                {
                    selections.remove(&oldest);
                } else {
                    break;
                }
            }
            selections.insert(id.clone(), selection);
        }

        Ok(TorchPreviewOutcome::Ready {
            preview: TorchPreview {
                preview_id: id,
                tag: tag.to_owned(),
                build: build.to_owned(),
                python: python.to_owned(),
                adapter: adapter.to_owned(),
                artifacts: Vec::new(),
                qualification: "unverified".to_owned(),
                expires_in_seconds: PREVIEW_TTL.as_secs(),
            },
        })
    }

    pub(crate) async fn consume_torch_install_selection(
        &self,
        preview_id: &str,
        tag: &str,
    ) -> Result<TorchInstallSelection> {
        let Some(selection) = self
            .torch_install_selections
            .lock()
            .await
            .remove(preview_id)
        else {
            return Err(failed(
                "Torch installation selection expired or was already used",
            ));
        };
        if selection.created.elapsed() >= PREVIEW_TTL || selection.tag != tag {
            return Err(failed(
                "Torch installation selection does not match the requested tag or has expired",
            ));
        }
        Ok(selection)
    }

    pub async fn torch_installed_probe_report(
        &self,
        tag: &str,
    ) -> Result<TorchInstalledProbeReport> {
        if self.app_id != AppId::Torch
            || !safe_torch_tag(tag)
            || !self.state.read().await.is_installed(tag)
        {
            return Err(failed("Torch runtime is not installed"));
        }
        let runtime = self.versions_dir().join(tag);
        let report_path = runtime.join("probe-results.json");
        let bytes = fs::read(&report_path)
            .await
            .map_err(|e| failed(format!("Installed Torch probe report is unavailable: {e}")))?;
        let report: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| failed(format!("Invalid installed Torch probe report: {e}")))?;
        let mut stale_reasons = Vec::new();
        // Retain historical checksums in the report without automatically
        // comparing installed files. Staleness reflects actual package and
        // hardware context, not post-download content integrity.
        let distribution_names: Vec<&str> = report["context"]["installed_distributions"]
            .as_object()
            .map(|values| values.keys().map(String::as_str).collect())
            .unwrap_or_default();
        let distribution_names =
            serde_json::to_string(&distribution_names).map_err(|e| failed(e.to_string()))?;
        let mut hardware_command =
            Command::new(pumas_library::platform::paths::venv_python(&runtime));
        hardware_command.kill_on_drop(true).args(["-I", "-B"]).arg("-c").arg(r#"import hashlib,importlib.metadata,json,subprocess,sys
import torch
devices=[]
if torch.cuda.is_available():
    for index in range(torch.cuda.device_count()):
        devices.append({"name":torch.cuda.get_device_name(index),"capability":list(torch.cuda.get_device_capability(index))})
identity={"cuda":torch.version.cuda,"hip":torch.version.hip,"device_count":len(devices),"devices":devices}
mps_available=bool(getattr(getattr(torch.backends,"mps",None),"is_available",lambda:False)())
identity["mps_available"]=mps_available
versions={}
for name in json.loads(sys.argv[1]):
    try: versions[name]=importlib.metadata.version(name)
    except importlib.metadata.PackageNotFoundError: versions[name]=None
all_versions={}
for distribution in importlib.metadata.distributions():
    name=distribution.metadata.get("Name")
    if name: all_versions[name.lower().replace("_","-")]=distribution.version
all_versions=dict(sorted(all_versions.items()))
distributions_sha256=hashlib.sha256(json.dumps(all_versions,sort_keys=True,separators=(",",":")).encode()).hexdigest()
try:
    driver=subprocess.run(["nvidia-smi","--query-gpu=driver_version","--format=csv,noheader"],capture_output=True,text=True,timeout=5)
    driver_version=(sorted(set(line.strip() for line in driver.stdout.splitlines() if line.strip())) or None) if driver.returncode == 0 else None
except (OSError,subprocess.TimeoutExpired): driver_version=None
print(json.dumps({"hardware_fingerprint":hashlib.sha256(json.dumps(identity,sort_keys=True,separators=(",",":")).encode()).hexdigest(),"installed_distributions":versions,"all_installed_distributions":all_versions,"distributions_sha256":distributions_sha256,"driver_version":driver_version}))"#).arg(distribution_names);
        let hardware_check =
            tokio::time::timeout(Duration::from_secs(15), hardware_command.output()).await;
        match hardware_check {
            Ok(Ok(output)) if output.status.success() => {
                if let Ok(current) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    append_runtime_context_changes(
                        &report["context"],
                        &current,
                        &mut stale_reasons,
                    );
                } else {
                    stale_reasons.push("Current runtime context could not be decoded".into());
                }
            }
            _ => stale_reasons.push("Current runtime context could not be checked".into()),
        }
        let mut report: TorchInstalledProbeReport = serde_json::from_value(report)
            .map_err(|e| failed(format!("Invalid installed Torch probe report: {e}")))?;
        report.stale_reasons = stale_reasons;
        Ok(report)
    }
}
