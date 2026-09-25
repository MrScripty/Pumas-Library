//! Server-owned Torch wheel resolution retained until installation or expiry.

use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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

/// Preserve catalog order while retaining only Python ABIs with an exact
/// official wheel for this release and build. An incomplete scan cannot prove
/// absence, so callers must not provision or try a lower minor on that basis.
fn auto_wheel_candidates(
    catalog_minors: &[String],
    build: &str,
    discovery: &TorchReleaseOptionsDiscovery,
) -> Option<Vec<String>> {
    if !discovery.complete_scan || discovery.status == TorchReleaseOptionsStatus::Inconclusive {
        return None;
    }
    Some(
        catalog_minors
            .iter()
            .filter(|minor| {
                let python = format!("python{minor}");
                discovery
                    .combinations
                    .iter()
                    .any(|wheel| wheel.build == build && wheel.python == python)
            })
            .cloned()
            .collect(),
    )
}

struct TorchPythonPreviewSearch {
    candidates: std::vec::IntoIter<String>,
    allow_fallback: bool,
    last_unsupported: Option<TorchPreviewOutcome>,
    finished: bool,
}

impl TorchPythonPreviewSearch {
    fn new(candidates: Vec<String>, allow_fallback: bool) -> Self {
        Self {
            candidates: candidates.into_iter(),
            allow_fallback,
            last_unsupported: None,
            finished: false,
        }
    }

    fn next_candidate(&mut self) -> Option<String> {
        if self.finished {
            None
        } else {
            self.candidates.next()
        }
    }

    /// None means the current rejection proves this candidate unsupported and
    /// the next compatible Python may be tried.
    fn record_attempt(&mut self, outcome: TorchPreviewOutcome) -> Option<TorchPreviewOutcome> {
        if self.allow_fallback
            && matches!(
                outcome,
                TorchPreviewOutcome::Rejected {
                    reason: TorchPreviewRejectionReason::Unsupported,
                    ..
                }
            )
        {
            self.last_unsupported = Some(outcome);
            None
        } else {
            self.finished = true;
            Some(outcome)
        }
    }

    fn exhausted(self) -> TorchPreviewOutcome {
        self.last_unsupported
            .unwrap_or(TorchPreviewOutcome::Rejected {
                reason: TorchPreviewRejectionReason::Inconclusive,
                message: TorchPreviewRejectionReason::Inconclusive.message(),
            })
    }
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
            managed_python: super::managed_python::ManagedPythonIdentity {
                python: "python3.12".into(),
                version: "3.12.0".into(),
                catalog_key: "cpython-3.12.0-test".into(),
                source_url: "https://github.com/astral-sh/python-build-standalone/releases/download/test/python.tar.zst".into(),
                target_triple: "x86_64-unknown-linux-gnu".into(),
                uv_version: "0.12.19".into(),
                uv_archive_sha256: "a".repeat(64),
                executable: PathBuf::from("/usr/bin/python3.12"),
            },
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
    fn retained_managed_identity_hashes_canonical_path_without_launching_python() {
        let root = tempfile::tempdir().unwrap();
        let inert = root.path().join("python-that-cannot-run");
        std::fs::write(&inert, b"provider-verified executable bytes").unwrap();
        let mut managed = retained_preview("test", Instant::now()).managed_python;
        managed.executable = std::fs::canonicalize(&inert).unwrap();
        let (path, hash) = retained_managed_interpreter(&managed).unwrap();
        assert_eq!(path, managed.executable);
        assert_eq!(
            hash,
            format!(
                "{:x}",
                Sha256::digest(b"provider-verified executable bytes")
            )
        );
        std::fs::create_dir(root.path().join("subdir")).unwrap();
        managed.executable = root
            .path()
            .join("subdir")
            .join("..")
            .join("python-that-cannot-run");
        assert!(retained_managed_interpreter(&managed).is_err());
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
    fn auto_skips_newer_python_without_exact_wheel_and_tries_newest_match_first() {
        let catalog = vec!["3.15".into(), "3.14".into(), "3.13".into(), "3.12".into()];
        let wheel = |build: &str, python: &str| TorchReleaseCombination {
            build: build.into(),
            python: python.into(),
            wheel_url: String::new(),
            sha256: None,
        };
        let mut discovery = TorchReleaseOptionsDiscovery {
            tag: "v2.14.0".into(),
            status: TorchReleaseOptionsStatus::Matches,
            complete_scan: true,
            checked_channels: vec!["cpu".into(), "cu136".into()],
            combinations: vec![
                wheel("cu136", "python3.14"),
                wheel("cpu", "python3.15"),
                wheel("cu136", "python3.12"),
            ],
            issues: Vec::new(),
            detected_gpu_vendors: Vec::new(),
            driver_status: TorchReleaseDriverStatus {
                nvidia: TorchReleaseDriverAvailability::NotPresent,
                amd: TorchReleaseDriverAvailability::NotPresent,
            },
            recommended: None,
            recommendation_note: String::new(),
        };
        assert_eq!(
            auto_wheel_candidates(&catalog, "cu136", &discovery),
            Some(vec!["3.14".into(), "3.12".into()])
        );
        discovery.complete_scan = false;
        discovery.status = TorchReleaseOptionsStatus::Inconclusive;
        assert_eq!(auto_wheel_candidates(&catalog, "cu136", &discovery), None);
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
        let workspace = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let cleanup = super::installer::TorchCleanupTasks::default();
        let outcome =
            run_preview_resolver(command, &workspace, Duration::from_millis(25), &cleanup)
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
        let workspace = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let path = workspace.path().to_path_buf();
        let ready = path.join("ready");
        let cleanup = std::sync::Arc::new(super::installer::TorchCleanupTasks::default());
        let run_workspace = workspace.clone();
        let run_cleanup = cleanup.clone();
        let task = tokio::spawn(async move {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg("touch \"$1\"; sleep 30")
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

#[cfg(test)]
mod platform_neutral_tests {
    use super::*;

    #[test]
    fn auto_python_fallback_requires_a_definite_unsupported_attempt() {
        let catalog = vec!["3.15".into(), "3.14".into(), "3.13".into(), "3.12".into()];
        let discovery = TorchReleaseOptionsDiscovery {
            tag: "v2.14.0".into(),
            status: TorchReleaseOptionsStatus::Matches,
            complete_scan: true,
            checked_channels: vec!["cu136".into()],
            combinations: ["3.12", "3.14", "3.13"]
                .into_iter()
                .map(|minor| TorchReleaseCombination {
                    build: "cu136".into(),
                    python: format!("python{minor}"),
                    wheel_url: String::new(),
                    sha256: None,
                })
                .collect(),
            issues: Vec::new(),
            detected_gpu_vendors: Vec::new(),
            driver_status: TorchReleaseDriverStatus {
                nvidia: TorchReleaseDriverAvailability::NotPresent,
                amd: TorchReleaseDriverAvailability::NotPresent,
            },
            recommended: None,
            recommendation_note: String::new(),
        };
        let candidates = auto_wheel_candidates(&catalog, "cu136", &discovery).unwrap();
        assert_eq!(candidates, ["3.14", "3.13", "3.12"]);

        let rejected = |reason| TorchPreviewOutcome::Rejected {
            reason,
            message: reason.message(),
        };
        let mut search = TorchPythonPreviewSearch::new(candidates.clone(), true);
        assert_eq!(search.next_candidate().as_deref(), Some("3.14"));
        let resolved = TorchPreviewOutcome::Resolved {
            preview: TorchPreview {
                preview_id: "newest-python".into(),
                tag: "v2.14.0".into(),
                build: "cu136".into(),
                python: "python3.14".into(),
                adapter: "none".into(),
                artifacts: Vec::new(),
                qualification: "unverified".into(),
                expires_in_seconds: PREVIEW_TTL.as_secs(),
            },
        };
        let outcome = search.record_attempt(resolved).unwrap();
        assert!(matches!(
            outcome,
            TorchPreviewOutcome::Resolved { preview } if preview.preview_id == "newest-python"
        ));
        assert_eq!(search.next_candidate(), None);

        let mut search = TorchPythonPreviewSearch::new(candidates.clone(), true);
        assert_eq!(search.next_candidate().as_deref(), Some("3.14"));
        assert!(search
            .record_attempt(rejected(TorchPreviewRejectionReason::Unsupported))
            .is_none());
        assert_eq!(search.next_candidate().as_deref(), Some("3.13"));
        let outcome = search
            .record_attempt(rejected(TorchPreviewRejectionReason::Inconclusive))
            .unwrap();
        assert!(matches!(
            outcome,
            TorchPreviewOutcome::Rejected {
                reason: TorchPreviewRejectionReason::Inconclusive,
                ..
            }
        ));
        assert_eq!(search.next_candidate(), None);

        let mut search = TorchPythonPreviewSearch::new(candidates.clone(), false);
        assert_eq!(search.next_candidate().as_deref(), Some("3.14"));
        assert!(matches!(
            search.record_attempt(rejected(TorchPreviewRejectionReason::Unsupported)),
            Some(TorchPreviewOutcome::Rejected {
                reason: TorchPreviewRejectionReason::Unsupported,
                ..
            })
        ));
        assert_eq!(search.next_candidate(), None);

        // Interpreter provisioning has the same inconclusive outcome as a
        // resolver failure, so it must also leave lower candidates untouched.
        let mut search = TorchPythonPreviewSearch::new(candidates.clone(), true);
        assert_eq!(search.next_candidate().as_deref(), Some("3.14"));
        assert!(search
            .record_attempt(TorchPreviewOutcome::Rejected {
                reason: TorchPreviewRejectionReason::Inconclusive,
                message: "The managed Python interpreter could not be provisioned.",
            })
            .is_some());
        assert_eq!(search.next_candidate(), None);

        let mut search = TorchPythonPreviewSearch::new(candidates, true);
        for minor in ["3.14", "3.13", "3.12"] {
            assert_eq!(search.next_candidate().as_deref(), Some(minor));
            assert!(search
                .record_attempt(rejected(TorchPreviewRejectionReason::Unsupported))
                .is_none());
        }
        assert_eq!(search.next_candidate(), None);
        assert!(matches!(
            search.exhausted(),
            TorchPreviewOutcome::Rejected {
                reason: TorchPreviewRejectionReason::Unsupported,
                ..
            }
        ));
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
    pub managed_python: super::managed_python::ManagedPythonIdentity,
    pub created: Instant,
}

pub(crate) type TorchPreviews = Arc<Mutex<HashMap<String, RetainedTorchPreview>>>;

#[derive(Deserialize)]
struct Resolution {
    release: String,
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
    getrandom::fill(&mut bytes)
        .map_err(|error| failed(format!("Preview ID randomness unavailable: {error}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn retained_managed_interpreter(
    managed: &super::managed_python::ManagedPythonIdentity,
) -> Result<(PathBuf, String)> {
    let path = std::fs::canonicalize(&managed.executable).map_err(PumasError::from)?;
    if path != managed.executable || !path.is_absolute() {
        return Err(failed(
            "Managed Python executable is not the retained canonical path",
        ));
    }
    let hash = format!(
        "{:x}",
        Sha256::digest(std::fs::read(&path).map_err(PumasError::from)?)
    );
    Ok((path, hash))
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

#[derive(Debug)]
pub(super) enum PreviewResolverRun {
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

pub(super) async fn run_preview_resolver(
    mut command: Command,
    workspace: &std::sync::Arc<tempfile::TempDir>,
    deadline: Duration,
    cleanup: &super::installer::TorchCleanupTasks,
) -> Result<PreviewResolverRun> {
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
        let python_candidates = super::torch_alternatives::managed_torch_candidate_minors(
            &self.launcher_root.join("launcher-data/managed-python"),
            &self.torch_cleanup,
        )
        .await
        .unwrap_or_default();
        let pythons: Vec<_> = python_candidates
            .iter()
            .map(|minor| {
                let python = format!("python{minor}");
                serde_json::json!({"id": python, "label": format!("Pumas-managed CPython {minor}")})
            })
            .collect();
        let linux_x64 = cfg!(all(target_os = "linux", target_arch = "x86_64"));
        let bundled_preset_available =
            linux_x64 && python_candidates.iter().any(|minor| minor == "3.12");
        let adapters: &[&str] = if linux_x64 { ADAPTERS } else { &["none"] };
        let builds: Vec<&str> = BUILDS
            .iter()
            .copied()
            .filter(|build| supported_torch_build(build))
            .collect();
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
            serde_json::json!({"builds": builds, "pythons": pythons, "adapters": adapters,
            "bundledPresetAvailable": bundled_preset_available,
            "defaultAdapter": "none",
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
            || !supported_torch_build(build)
            || (!ADAPTERS.contains(&adapter) && adapter != "bundled")
        {
            return Err(failed("Invalid Torch preview selection"));
        }
        if !cfg!(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(
                target_os = "windows",
                target_arch = "x86_64",
                target_env = "msvc"
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )) {
            return Err(failed("Managed Torch is unsupported on this platform"));
        }
        if !cfg!(target_os = "linux") && adapter != "none" {
            return Err(failed(
                "Image dependency profiles are unavailable on this platform",
            ));
        }
        let python_root = self.launcher_root.join("launcher-data/managed-python");
        let candidates = match super::torch_alternatives::managed_torch_candidate_minors(
            &python_root,
            &self.torch_cleanup,
        )
        .await
        {
            Ok(candidates) if candidates.len() <= 16 => candidates,
            Ok(_) => {
                return Ok(TorchPreviewOutcome::Rejected {
                    reason: TorchPreviewRejectionReason::Inconclusive,
                    message: "The managed Python catalog exceeded the bounded candidate scan.",
                });
            }
            Err(_) => {
                return Ok(TorchPreviewOutcome::Rejected {
                    reason: TorchPreviewRejectionReason::Inconclusive,
                    message: "The managed Python catalog could not be verified.",
                });
            }
        };
        let mut release_verified = false;
        let requested_minors = if python == "auto" {
            if adapter == "bundled" {
                if !is_bundled_preset(tag, build, "python3.12", adapter) {
                    return Err(failed(
                        "Bundled adapters require the v2.9.1 CUDA 13.0/Python 3.12 preset",
                    ));
                }
                vec!["3.12".to_owned()]
            } else {
                let discovery = self.discover_torch_release_options(tag).await?;
                release_verified = true;
                let Some(exact_minors) = auto_wheel_candidates(&candidates, build, &discovery)
                else {
                    return Ok(TorchPreviewOutcome::Rejected {
                        reason: TorchPreviewRejectionReason::Inconclusive,
                        message: "Official wheel discovery did not complete conclusively.",
                    });
                };
                if exact_minors.is_empty() {
                    return Ok(TorchPreviewOutcome::Rejected {
                        reason: TorchPreviewRejectionReason::Unsupported,
                        message: "No compatible official Torch wheel was found for this version, build, and the managed Python catalog.",
                    });
                }
                exact_minors
            }
        } else {
            let minor = python
                .strip_prefix("python")
                .ok_or_else(|| failed("Select automatic Python or a managed CPython choice"))?;
            if !candidates.iter().any(|candidate| candidate == minor) {
                return Err(failed(
                    "Selected Python is not in the managed CPython catalog",
                ));
            }
            vec![minor.to_owned()]
        };
        if requested_minors.is_empty() {
            return Ok(TorchPreviewOutcome::Rejected {
                reason: TorchPreviewRejectionReason::Inconclusive,
                message:
                    "No stable native CPython candidate is available from the managed provider.",
            });
        }
        if !release_verified {
            self.resolve_installable_release(tag).await?;
        }
        let mut search = TorchPythonPreviewSearch::new(
            requested_minors,
            python == "auto" && adapter != "bundled",
        );
        while let Some(minor) = search.next_candidate() {
            // A provisioning failure is inconclusive and must stop the search; it
            // cannot be used as evidence that a lower Python is a better match.
            let managed = match super::torch_alternatives::ensure_managed_torch_interpreter(
                &python_root,
                &self.torch_cleanup,
                &minor,
            )
            .await
            {
                Ok(managed) => managed,
                Err(_) => {
                    let outcome = TorchPreviewOutcome::Rejected {
                        reason: TorchPreviewRejectionReason::Inconclusive,
                        message: "The managed Python interpreter could not be provisioned.",
                    };
                    return Ok(search
                        .record_attempt(outcome)
                        .expect("provisioning must stop"));
                }
            };
            let outcome = self
                .preview_torch_runtime_with_interpreter(tag, build, adapter, &managed)
                .await?;
            if let Some(outcome) = search.record_attempt(outcome) {
                return Ok(outcome);
            }
        }
        Ok(search.exhausted())
    }

    async fn preview_torch_runtime_with_interpreter(
        &self,
        tag: &str,
        build: &str,
        adapter: &str,
        managed_python: &super::managed_python::ManagedPythonIdentity,
    ) -> Result<TorchPreviewOutcome> {
        let python = managed_python.python.as_str();
        let interpreter = managed_python.executable.as_path();
        if self.app_id != AppId::Torch
            || !supported_torch_build(build)
            || !python.starts_with("python3.")
            || (!ADAPTERS.contains(&adapter) && adapter != "bundled")
        {
            return Err(failed("Invalid Torch preview selection"));
        }
        if !cfg!(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(
                target_os = "windows",
                target_arch = "x86_64",
                target_env = "msvc"
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )) {
            return Err(failed("Managed Torch is unsupported on this platform"));
        }
        if !cfg!(target_os = "linux") && adapter != "none" {
            return Err(failed(
                "Image dependency profiles are unavailable on this platform",
            ));
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
        let expected_python = python.strip_prefix("python").unwrap();
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
            let (interpreter_path, interpreter_hash) =
                retained_managed_interpreter(managed_python)?;
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
                    managed_python: managed_python.clone(),
                    created: Instant::now(),
                },
            );
            return Ok(TorchPreviewOutcome::Resolved { preview });
        }
        // This temporary directory is only a resolver workspace. Retained preview
        // data is held in memory, so restart invalidates every outstanding ID.
        let workspace = std::sync::Arc::new(tempfile::tempdir().map_err(PumasError::from)?);
        let resolver = workspace.path().join("resolve_runtime.py");
        fs::write(
            &resolver,
            include_str!("../../../../../torch-server/resolve_runtime.py"),
        )
        .await
        .map_err(PumasError::from)?;
        let mut command = Command::new(interpreter);
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
        self.torch_cleanup.drain_residual_child_slots().await?;
        let run = run_preview_resolver(
            command,
            &workspace,
            Duration::from_secs(180),
            &self.torch_cleanup,
        )
        .await?;
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
        if parsed.release != version
            || parsed.build != build
            || parsed.python != expected_python
            || parsed.adapter != adapter
            || parsed.artifacts.is_empty()
            || parsed.implementation != "cpython"
            || !match std::env::consts::OS {
                "linux" => parsed.machine == "x86_64" && parsed.platform.starts_with("Linux-"),
                "windows" => parsed.machine == "AMD64" && parsed.platform.starts_with("Windows-"),
                "macos" => parsed.machine == "arm64" && parsed.platform.starts_with("macOS-"),
                _ => false,
            }
            || !parsed
                .artifacts
                .iter()
                .any(|artifact| artifact.name == "torch" && artifact.version == parsed.torch)
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
        if !Path::new(&parsed.interpreter).is_absolute()
            || interpreter_path != managed_python.executable
        {
            return Err(failed(
                "Resolver did not identify the selected absolute Python executable",
            ));
        }
        let (interpreter_path, interpreter_hash) = retained_managed_interpreter(managed_python)?;
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
                managed_python: managed_python.clone(),
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
        let mut hardware_command =
            Command::new(pumas_library::platform::paths::venv_python(&runtime));
        hardware_command.kill_on_drop(true).arg("-I").arg("-c").arg(r#"import hashlib,importlib.metadata,json,subprocess,sys
from pathlib import Path
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
