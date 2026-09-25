//! Server-owned Torch wheel resolution retained until installation or expiry.

use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Read;
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
pub(super) const PYTHONS: &[&str] = &["python3.10", "python3.11", "python3.12", "python3.13"];
const ADAPTERS: &[&str] = &["none", "flux2"];
const PREVIEW_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_RETAINED_TORCH_PREVIEWS: usize = 32;

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

fn insert_retained_torch_preview(
    previews: &mut HashMap<String, RetainedTorchPreview>,
    preview_id: String,
    preview: RetainedTorchPreview,
) {
    previews.retain(|_, retained| retained.created.elapsed() < PREVIEW_TTL);
    while previews.len() >= MAX_RETAINED_TORCH_PREVIEWS {
        let oldest = previews
            .iter()
            .min_by_key(|(_, retained)| retained.created)
            .map(|(id, _)| id.clone());
        if let Some(oldest) = oldest {
            previews.remove(&oldest);
        } else {
            break;
        }
    }
    previews.insert(preview_id, preview);
}

fn is_bundled_preset(tag: &str, build: &str, python: &str, adapter: &str) -> bool {
    tag == "v2.9.1" && build == "cu130" && python == "python3.12" && adapter == "bundled"
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
    use std::os::unix::process::ExitStatusExt;

    fn retained_preview(preview_id: &str, created: Instant) -> RetainedTorchPreview {
        RetainedTorchPreview {
            preview: TorchPreview {
                preview_id: preview_id.into(),
                tag: "v2.9.0".into(),
                build: "cpu".into(),
                python: "python3.12".into(),
                adapter: "none".into(),
                artifacts: Vec::new(),
                qualification: "unverified".into(),
                expires_in_seconds: PREVIEW_TTL.as_secs(),
            },
            requirements: String::new(),
            resolution: String::new(),
            report: String::new(),
            interpreter_path: PathBuf::from("/usr/bin/python3.12"),
            interpreter_hash: String::new(),
            created,
        }
    }

    #[test]
    fn retained_previews_expire_and_remain_bounded() {
        let mut previews = HashMap::new();
        let now = Instant::now();
        previews.insert(
            "expired".into(),
            retained_preview("expired", now - PREVIEW_TTL - Duration::from_secs(1)),
        );

        for index in 0..=MAX_RETAINED_TORCH_PREVIEWS {
            let preview_id = format!("preview-{index}");
            insert_retained_torch_preview(
                &mut previews,
                preview_id.clone(),
                retained_preview(&preview_id, now + Duration::from_secs(index as u64)),
            );
        }

        assert_eq!(previews.len(), MAX_RETAINED_TORCH_PREVIEWS);
        assert!(!previews.contains_key("expired"));
        assert!(!previews.contains_key("preview-0"));
        assert!(previews.contains_key(&format!("preview-{MAX_RETAINED_TORCH_PREVIEWS}")));
    }

    #[test]
    fn bundled_preset_is_one_choice_in_the_upstream_range() {
        assert!(is_bundled_preset(
            "v2.9.1",
            "cu130",
            "python3.12",
            "bundled"
        ));
        for (tag, build, python, adapter) in [
            ("v2.9.0", "cu130", "python3.12", "bundled"),
            ("v2.9.1", "cpu", "python3.12", "none"),
            ("v2.9.1", "cu130", "python3.13", "none"),
            ("v2.9.1", "cu130", "python3.12", "flux2"),
        ] {
            assert!(!is_bundled_preset(tag, build, python, adapter));
        }
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
    fn resolver_exit_codes_serialize_distinct_safe_rejections() {
        for (code, expected_reason, expected_message) in [
            (
                4,
                "unsupported",
                "No compatible official Torch wheel was found for this version, build, and Python selection.",
            ),
            (
                2,
                "unsupported",
                "A required dependency is unavailable or incompatible for this selection.",
            ),
            (3, "validation_failed", "The resolved wheel report failed validation."),
            (
                75,
                "network_inconclusive",
                "Network access prevented a conclusive wheel resolution.",
            ),
            (1, "inconclusive", "Wheel resolution did not complete conclusively."),
        ] {
            let outcome = resolver_rejection(PreviewResolverRun::Exited(
                std::process::ExitStatus::from_raw(code << 8),
            ))
            .unwrap();
            let value = serde_json::to_value(outcome).unwrap();
            assert_eq!(value["status"], "rejected");
            assert_eq!(value["reason"], expected_reason);
            assert_eq!(value["message"], expected_message);
            assert_eq!(value.as_object().unwrap().len(), 3);
        }
        assert!(resolver_rejection(PreviewResolverRun::Exited(
            std::process::ExitStatus::from_raw(0)
        ))
        .is_none());
        assert_eq!(
            TorchPreviewRejectionReason::from_exit_code(None),
            TorchPreviewRejectionReason::Inconclusive
        );
        let timeout =
            serde_json::to_value(resolver_rejection(PreviewResolverRun::TimedOut).unwrap())
                .unwrap();
        assert_eq!(timeout["status"], "rejected");
        assert_eq!(timeout["reason"], "inconclusive");
        assert_eq!(
            timeout["message"],
            TorchPreviewRejectionReason::Inconclusive.message()
        );
    }

    #[test]
    fn resolved_preview_serializes_under_preview_key() {
        let outcome = TorchPreviewOutcome::Resolved {
            preview: TorchPreview {
                preview_id: "retained-id".into(),
                tag: "v2.9.1".into(),
                build: "cu130".into(),
                python: "python3.12".into(),
                adapter: "bundled".into(),
                artifacts: Vec::new(),
                qualification: "qualified".into(),
                expires_in_seconds: 1800,
            },
        };
        let value = serde_json::to_value(outcome).unwrap();
        assert_eq!(value["status"], "resolved");
        assert_eq!(value["preview"]["previewId"], "retained-id");
        assert_eq!(value["preview"]["expiresInSeconds"], 1800);
        assert_eq!(value.as_object().unwrap().len(), 2);
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
        assert_eq!(reasons.len(), 6);
    }

    #[test]
    fn runtime_source_hashes_track_sidecar_files_without_virtualenv_or_links() {
        let root = tempfile::tempdir().unwrap();
        for (name, contents) in [
            ("serve.py", "first"),
            ("loaders/image.py", "loader"),
            ("venv/lib/site.py", "venv"),
            (".venv/site.py", "other venv"),
            ("__pycache__/cached.py", "cache"),
        ] {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        std::os::unix::fs::symlink(root.path().join("serve.py"), root.path().join("alias.py"))
            .unwrap();
        let original = runtime_source_hashes(root.path()).unwrap();
        assert_eq!(original.len(), 2);
        assert!(original.contains_key("serve.py"));
        assert!(original.contains_key("loaders/image.py"));
        std::fs::write(root.path().join("serve.py"), "changed").unwrap();
        assert_ne!(original, runtime_source_hashes(root.path()).unwrap());
    }

    #[tokio::test]
    async fn preview_resolver_deadline_kills_process_group() {
        let workspace = tempfile::tempdir().unwrap();
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let outcome = run_preview_resolver(command, workspace.path(), Duration::from_millis(25))
            .await
            .unwrap();
        assert!(matches!(&outcome, PreviewResolverRun::TimedOut));
        let rejected = resolver_rejection(outcome).unwrap();
        let value = serde_json::to_value(rejected).unwrap();
        assert_eq!(value["status"], "rejected");
        assert_eq!(value["reason"], "inconclusive");
    }

    #[tokio::test]
    async fn preview_resolver_cleans_child_after_leader_exits() {
        let workspace = tempfile::tempdir().unwrap();
        let mut command = Command::new("sh");
        command.args(["-c", "echo $$; sleep 30 &"]);
        let outcome = run_preview_resolver(command, workspace.path(), Duration::from_secs(2))
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

impl TorchPreviewRejectionReason {
    fn from_exit_code(code: Option<i32>) -> Self {
        match code {
            Some(2 | 4) => Self::Unsupported,
            Some(3) => Self::ValidationFailed,
            Some(75) => Self::NetworkInconclusive,
            _ => Self::Inconclusive,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Unsupported => {
                "A required dependency is unavailable or incompatible for this selection."
            }
            Self::ValidationFailed => "The resolved wheel report failed validation.",
            Self::NetworkInconclusive => "Network access prevented a conclusive wheel resolution.",
            Self::Inconclusive => "Wheel resolution did not complete conclusively.",
        }
    }

    fn message_for_exit_code(self, code: Option<i32>) -> &'static str {
        if code == Some(4) {
            "No compatible official Torch wheel was found for this version, build, and Python selection."
        } else {
            self.message()
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TorchPreviewOutcome {
    Resolved {
        preview: TorchPreview,
    },
    Rejected {
        reason: TorchPreviewRejectionReason,
        message: &'static str,
    },
}

pub(crate) struct RetainedTorchPreview {
    pub preview: TorchPreview,
    pub requirements: String,
    pub resolution: String,
    pub report: String,
    pub interpreter_path: PathBuf,
    pub interpreter_hash: String,
    pub created: Instant,
}

pub(crate) type TorchPreviews = Arc<Mutex<HashMap<String, RetainedTorchPreview>>>;

#[derive(Deserialize)]
struct Resolution {
    torch: String,
    build: String,
    python: String,
    interpreter: String,
    implementation: String,
    platform: String,
    machine: String,
    adapter: String,
    artifacts: Vec<TorchArtifact>,
}

fn failed(message: impl Into<String>) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

fn preview_token() -> Result<String> {
    let mut bytes = [0u8; 24];
    std::fs::File::open("/dev/urandom")
        .map_err(PumasError::from)?
        .read_exact(&mut bytes)
        .map_err(PumasError::from)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn runtime_source_hashes(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for directory in [root.to_path_buf(), root.join("loaders")] {
        if !directory.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&directory).map_err(PumasError::from)? {
            let entry = entry.map_err(PumasError::from)?;
            let kind = entry.file_type().map_err(PumasError::from)?;
            if !kind.is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("py")
            {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| failed(format!("Invalid runtime source path: {e}")))?
                .to_str()
                .ok_or_else(|| failed("Runtime source path is not UTF-8"))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            let bytes = std::fs::read(entry.path()).map_err(PumasError::from)?;
            hashes.insert(relative, format!("{:x}", Sha256::digest(bytes)));
        }
    }
    Ok(hashes)
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
        (
            "interpreter_sha256",
            "Python executable changed since the probe",
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

#[cfg(target_os = "linux")]
struct PreviewGroupGuard(Option<u32>);

#[derive(Debug)]
enum PreviewResolverRun {
    Exited(std::process::ExitStatus),
    TimedOut,
}

fn resolver_rejection(run: PreviewResolverRun) -> Option<TorchPreviewOutcome> {
    let (reason, message) = match run {
        PreviewResolverRun::Exited(status) if status.success() => return None,
        PreviewResolverRun::Exited(status) => {
            let reason = TorchPreviewRejectionReason::from_exit_code(status.code());
            (reason, reason.message_for_exit_code(status.code()))
        }
        PreviewResolverRun::TimedOut => (
            TorchPreviewRejectionReason::Inconclusive,
            TorchPreviewRejectionReason::Inconclusive.message(),
        ),
    };
    Some(TorchPreviewOutcome::Rejected { reason, message })
}

#[cfg(target_os = "linux")]
impl Drop for PreviewGroupGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0.take() {
            let _ = pumas_library::platform::linux_group::signal_group(pid);
        }
    }
}

#[cfg(target_os = "linux")]
async fn run_preview_resolver(
    mut command: Command,
    workspace: &Path,
    deadline: Duration,
) -> Result<PreviewResolverRun> {
    use pumas_library::platform::linux_group;
    linux_group::ensure_supported().map_err(PumasError::from)?;
    let stdout =
        std::fs::File::create(workspace.join("resolver.stdout")).map_err(PumasError::from)?;
    let stderr =
        std::fs::File::create(workspace.join("resolver.stderr")).map_err(PumasError::from)?;
    command
        .stdout(stdout)
        .stderr(stderr)
        .kill_on_drop(true)
        .process_group(0);
    let mut child = command
        .spawn()
        .map_err(|e| failed(format!("Torch resolver could not start: {e}")))?;
    let pid = child
        .id()
        .ok_or_else(|| failed("Torch resolver process has no PID"))?;
    let mut group = PreviewGroupGuard(Some(pid));
    let until = tokio::time::Instant::now() + deadline;
    let (status, timed_out) = loop {
        let observed = linux_group::observe_exit(pid).map_err(PumasError::from)?;
        let timed_out = tokio::time::Instant::now() >= until;
        if observed.is_some() || timed_out {
            break (observed, timed_out);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let group_id = i32::try_from(pid).map_err(|_| failed("Invalid Torch resolver PID"))?;
    loop {
        linux_group::signal_group(pid).map_err(PumasError::from)?;
        let alive =
            tokio::task::spawn_blocking(move || linux_group::group_has_live_members(group_id))
                .await
                .map_err(|e| failed(format!("Preview process observation failed: {e}")))?
                .map_err(PumasError::from)?;
        if !alive {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    child.wait().await.map_err(PumasError::from)?;
    group.0 = None;
    if timed_out {
        return Ok(PreviewResolverRun::TimedOut);
    }
    status
        .map(PreviewResolverRun::Exited)
        .ok_or_else(|| failed("Torch preview resolver exited without status"))
}

#[cfg(not(target_os = "linux"))]
async fn run_preview_resolver(
    _command: Command,
    _workspace: &Path,
    _deadline: Duration,
) -> Result<PreviewResolverRun> {
    Err(failed("Managed Torch previews require Linux x86_64"))
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
            && fs::try_exists(runtime.join("venv/bin/python"))
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
        let mut command = Command::new(runtime.join("venv/bin/python"));
        command
            .kill_on_drop(true)
            .args(["-I", "-c", "import torch; print(torch.__version__)"]);
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

    pub async fn torch_runtime_options(&self) -> Result<serde_json::Value> {
        if self.app_id != AppId::Torch {
            return Err(failed("Torch manager required"));
        }
        let mut pythons = Vec::new();
        for name in PYTHONS {
            if let Ok(output) = Command::new(name).args(["-I", "--version"]).output().await {
                if output.status.success() {
                    pythons.push(serde_json::json!({"id": name, "label": String::from_utf8_lossy(&output.stdout).trim()}));
                }
            }
        }
        let mut installed = Vec::new();
        for tag in self.state.read().await.get_installed_tags() {
            let recipe_path = self.versions_dir().join(&tag).join("runtime.json");
            let recipe = fs::read(&recipe_path)
                .await
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .unwrap_or(serde_json::Value::Null);
            let preset = recipe["recipe_id"] == "torch-upstream-2.9.1-r1";
            installed.push(serde_json::json!({"tag":tag,
                "build": if preset { Some("cu130") } else { recipe["build"].as_str() },
                "python": if preset { Some("python3.12") } else { recipe["python"].as_str() },
                "adapter": if preset { Some("bundled") } else { recipe["adapter"].as_str() },
                "qualification": if preset { "qualified" } else { "unverified" } }));
        }
        Ok(
            serde_json::json!({"builds": BUILDS, "pythons": pythons, "adapters": ADAPTERS,
            "preset": {"tag":"v2.9.1", "build":"cu130", "python":"python3.12", "adapter":"bundled"},
            "installed": installed}),
        )
    }

    pub async fn preview_torch_runtime(
        &self,
        tag: &str,
        build: &str,
        python: &str,
        adapter: &str,
    ) -> Result<TorchPreviewOutcome> {
        if self.app_id != AppId::Torch
            || !valid_torch_channel(build)
            || !PYTHONS.contains(&python)
            || (!ADAPTERS.contains(&adapter) && adapter != "bundled")
        {
            return Err(failed("Invalid Torch preview selection"));
        }
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(failed("Managed Torch requires Linux x86_64"));
        }
        let version = tag
            .strip_prefix('v')
            .filter(|v| {
                let parts: Vec<_> = v.split('.').collect();
                parts.len() == 3
                    && parts
                        .iter()
                        .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
            })
            .ok_or_else(|| failed("Invalid stable Torch tag"))?;
        if adapter == "bundled" && !is_bundled_preset(tag, build, python, adapter) {
            return Err(failed(
                "Bundled adapters require the v2.9.1 CUDA 13.0/Python 3.12 preset",
            ));
        }
        self.resolve_installable_release(tag).await?;
        let output = Command::new(python)
            .args([
                "-I",
                "-c",
                "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",
            ])
            .output()
            .await
            .map_err(|e| failed(format!("Selected Python is unavailable: {e}")))?;
        let expected_python = python.strip_prefix("python").unwrap();
        if !output.status.success()
            || String::from_utf8_lossy(&output.stdout).trim() != expected_python
        {
            return Err(failed("Selected Python interpreter has the wrong version"));
        }
        if is_bundled_preset(tag, build, python, adapter) {
            let lock = include_str!("../../../../../torch-server/runtime/requirements.lock");
            let mut artifacts = Vec::new();
            let mut lines = lock.lines();
            while let Some(line) = lines.next() {
                let Some((name, rest)) = line.split_once(" @ ") else {
                    continue;
                };
                let Some(url) = rest.split_whitespace().next() else {
                    continue;
                };
                if !["torch", "torchvision", "nunchaku"].contains(&name) {
                    continue;
                }
                let hash_line = lines.next().unwrap_or_default();
                let hash = hash_line
                    .split("--hash=sha256:")
                    .nth(1)
                    .unwrap_or_default()
                    .split_whitespace()
                    .next()
                    .unwrap_or_default();
                artifacts.push(TorchArtifact {
                    name: name.into(),
                    version: if name == "torch" {
                        "2.9.1+cu130"
                    } else if name == "torchvision" {
                        "0.24.1+cu130"
                    } else {
                        "1.2.0+torch2.9"
                    }
                    .into(),
                    url: url.into(),
                    sha256: hash.into(),
                });
            }
            if artifacts.len() != 3 || artifacts.iter().any(|a| a.sha256.len() != 64) {
                return Err(failed("Embedded preset lock is incomplete"));
            }
            let preview_id = preview_token()?;
            let preview = TorchPreview {
                preview_id: preview_id.clone(),
                tag: tag.into(),
                build: build.into(),
                python: python.into(),
                adapter: adapter.into(),
                artifacts,
                qualification: "qualified".into(),
                expires_in_seconds: PREVIEW_TTL.as_secs(),
            };
            let path_output = Command::new(python)
                .args(["-I", "-c", "import sys; print(sys.executable)"])
                .output()
                .await
                .map_err(PumasError::from)?;
            let interpreter_path =
                std::fs::canonicalize(String::from_utf8_lossy(&path_output.stdout).trim())
                    .map_err(PumasError::from)?;
            let interpreter_hash = format!(
                "{:x}",
                Sha256::digest(std::fs::read(&interpreter_path).map_err(PumasError::from)?)
            );
            let mut previews = self.torch_previews.lock().await;
            insert_retained_torch_preview(
                &mut previews,
                preview_id,
                RetainedTorchPreview {
                    preview: preview.clone(),
                    requirements: lock.into(),
                    resolution: String::new(),
                    report: serde_json::json!({"qualification":"qualified preset", "requirementsLock":lock,
                        "directArtifacts":preview.artifacts, "scope":"hash-pinned lock; transitive wheel URLs are selected during install"}).to_string(),
                    interpreter_path,
                    interpreter_hash,
                    created: Instant::now(),
                },
            );
            return Ok(TorchPreviewOutcome::Resolved { preview });
        }
        // This temporary directory is only a resolver workspace. Retained preview
        // data is held in memory, so restart invalidates every outstanding ID.
        let workspace = tempfile::tempdir().map_err(PumasError::from)?;
        let resolver = workspace.path().join("resolve_runtime.py");
        fs::write(
            &resolver,
            include_str!("../../../../../torch-server/resolve_runtime.py"),
        )
        .await
        .map_err(PumasError::from)?;
        let mut command = Command::new(python);
        command
            .arg(&resolver)
            .args([
                "--version",
                version,
                "--build",
                build,
                "--adapter",
                adapter,
                "--output",
            ])
            .arg(workspace.path());
        let run = run_preview_resolver(command, workspace.path(), Duration::from_secs(180)).await?;
        if let Some(rejection) = resolver_rejection(run) {
            return Ok(rejection);
        }
        let resolution = fs::read_to_string(workspace.path().join("resolution.json"))
            .await
            .map_err(PumasError::from)?;
        let requirements = fs::read_to_string(workspace.path().join("requirements.txt"))
            .await
            .map_err(PumasError::from)?;
        let report = fs::read_to_string(workspace.path().join("pip-resolution.json"))
            .await
            .map_err(PumasError::from)?;
        let parsed: Resolution = serde_json::from_str(&resolution)
            .map_err(|e| failed(format!("Invalid resolver manifest: {e}")))?;
        if parsed.torch != format!("{version}+{build}")
            || parsed.build != build
            || parsed.python != expected_python
            || parsed.adapter != adapter
            || parsed.artifacts.is_empty()
            || parsed.implementation != "cpython"
            || parsed.machine != "x86_64"
            || !parsed.platform.starts_with("Linux-")
        {
            return Err(failed(
                "Resolver manifest does not match requested Torch selection",
            ));
        }
        for artifact in &parsed.artifacts {
            if artifact.sha256.len() != 64
                || !artifact.sha256.bytes().all(|c| c.is_ascii_hexdigit())
                || !artifact.url.starts_with("https://")
                || !requirements.contains(&format!(
                    "{} @ {} --hash=sha256:{}",
                    artifact.name, artifact.url, artifact.sha256
                ))
            {
                return Err(failed(
                    "Resolver artifact does not match exact hashed requirements",
                ));
            }
        }
        let interpreter_path = std::fs::canonicalize(&parsed.interpreter)
            .map_err(|e| failed(format!("Cannot identify selected Python executable: {e}")))?;
        if !interpreter_path.is_absolute() || !parsed.interpreter.starts_with('/') {
            return Err(failed(
                "Resolver did not identify an absolute Python executable",
            ));
        }
        let interpreter_hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read(&interpreter_path).map_err(PumasError::from)?)
        );
        let preview_id = preview_token()?;
        let preview = TorchPreview {
            preview_id: preview_id.clone(),
            tag: tag.into(),
            build: build.into(),
            python: python.into(),
            adapter: adapter.into(),
            artifacts: parsed.artifacts,
            qualification: "unverified".into(),
            expires_in_seconds: PREVIEW_TTL.as_secs(),
        };
        let mut previews = self.torch_previews.lock().await;
        insert_retained_torch_preview(
            &mut previews,
            preview_id,
            RetainedTorchPreview {
                preview: preview.clone(),
                requirements,
                resolution,
                report,
                interpreter_path,
                interpreter_hash,
                created: Instant::now(),
            },
        );
        Ok(TorchPreviewOutcome::Resolved { preview })
    }

    pub async fn torch_preview_report(&self, preview_id: &str) -> Result<String> {
        let mut previews = self.torch_previews.lock().await;
        let retained = previews
            .get(preview_id)
            .filter(|value| value.created.elapsed() < PREVIEW_TTL)
            .map(|value| value.report.clone());
        match retained {
            Some(report) => Ok(report),
            None => {
                previews.remove(preview_id);
                Err(failed("Torch preview expired or unknown"))
            }
        }
    }

    pub async fn torch_installed_probe_report(&self, tag: &str) -> Result<serde_json::Value> {
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
        let mut report: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| failed(format!("Invalid installed Torch probe report: {e}")))?;
        let mut stale_reasons = Vec::new();
        for (name, key) in [
            ("resolution.json", "resolution_sha256"),
            ("serve.py", "sidecar_sha256"),
        ] {
            let observed = fs::read(runtime.join(name))
                .await
                .map_err(PumasError::from)?;
            let hash = format!("{:x}", Sha256::digest(&observed));
            if report["context"][key].as_str() != Some(hash.as_str()) {
                stale_reasons.push(format!("{name} changed since the probe"));
            }
        }
        let sources = runtime_source_hashes(&runtime)?;
        if report["context"]["runtime_files_sha256"] != serde_json::json!(sources) {
            stale_reasons.push("Runtime Python sources changed since the probe".into());
        }
        let distribution_names: Vec<&str> = report["context"]["installed_distributions"]
            .as_object()
            .map(|values| values.keys().map(String::as_str).collect())
            .unwrap_or_default();
        let distribution_names =
            serde_json::to_string(&distribution_names).map_err(|e| failed(e.to_string()))?;
        let mut hardware_command = Command::new(runtime.join("venv/bin/python"));
        hardware_command.kill_on_drop(true).arg("-I").arg("-c").arg(r#"import hashlib,importlib.metadata,json,subprocess,sys
from pathlib import Path
import torch
devices=[]
if torch.cuda.is_available():
    for index in range(torch.cuda.device_count()):
        devices.append({"name":torch.cuda.get_device_name(index),"capability":list(torch.cuda.get_device_capability(index))})
identity={"cuda":torch.version.cuda,"hip":torch.version.hip,"device_count":len(devices),"devices":devices}
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
print(json.dumps({"hardware_fingerprint":hashlib.sha256(json.dumps(identity,sort_keys=True,separators=(",",":")).encode()).hexdigest(),"installed_distributions":versions,"all_installed_distributions":all_versions,"distributions_sha256":distributions_sha256,"interpreter_sha256":hashlib.sha256(Path(sys.executable).resolve().read_bytes()).hexdigest(),"driver_version":driver_version}))"#).arg(distribution_names);
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
        if let Some(object) = report.as_object_mut() {
            object.insert("stale".into(), (!stale_reasons.is_empty()).into());
            object.insert("staleReasons".into(), serde_json::json!(stale_reasons));
        }
        Ok(report)
    }
}
