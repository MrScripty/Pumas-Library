//! Closed desktop projections of local Torch runtime inspection: install
//! choices, the saved install-time probe, startup trials and alternative wheels.
//!
//! Each type ships with its renderer consumer, so objects are closed. The
//! projections validate the domain invariants the renderer relies on.

use serde::Serialize;
use std::collections::BTreeMap;

#[cfg(feature = "inference-plugins")]
use super::{invalid_domain_outcome, PumasError};
#[cfg(feature = "inference-plugins")]
use pumas_app_manager::version_manager as domain;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchRuntimeOptionsOutcome {
    builds: Vec<String>,
    default_build: String,
    pythons: Vec<TorchPythonChoice>,
    adapters: Vec<String>,
    /// One of `adapters`.
    default_adapter: String,
    /// Combinations verified on this platform. Evidence, not install pins:
    /// choosing one still resolves current package versions.
    verified: Vec<TorchVerifiedRuntime>,
    /// Offered adapters that install only on one selection.
    adapter_requirements: Vec<TorchAdapterRequirement>,
    installed: Vec<TorchInstalledRuntime>,
}

/// The one release, build and managed Python an adapter's own wheel allows.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchAdapterRequirement {
    adapter: String,
    tag: String,
    build: String,
    python: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchPythonChoice {
    id: String,
    label: String,
}

/// A Torch release, build, Python and adapters that worked together, with
/// the package versions present and when it was verified.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchVerifiedRuntime {
    tag: String,
    build: String,
    python: String,
    adapters: Vec<String>,
    /// ISO date of the verification.
    verified_on: String,
    /// What the verification exercised.
    scope: String,
    source_commit: String,
    packages: Vec<TorchVerifiedPackage>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchVerifiedPackage {
    name: String,
    version: String,
}

/// An installed runtime; a field is null when its recipe does not record it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchInstalledRuntime {
    tag: String,
    build: Option<String>,
    python: Option<String>,
    adapter: Option<String>,
    qualification: super::TorchRuntimePreviewQualification,
}

#[cfg(feature = "inference-plugins")]
impl TryFrom<domain::TorchRuntimeOptions> for TorchRuntimeOptionsOutcome {
    type Error = PumasError;

    fn try_from(options: domain::TorchRuntimeOptions) -> Result<Self, Self::Error> {
        if !options.adapters.contains(&options.default_adapter) {
            return Err(invalid_domain_outcome("Torch default adapter"));
        }
        if options
            .adapter_requirements
            .iter()
            .any(|requirement| !options.adapters.contains(&requirement.adapter))
        {
            return Err(invalid_domain_outcome("Torch adapter requirement"));
        }
        Ok(Self {
            builds: options.builds,
            default_build: options.default_build,
            pythons: options
                .pythons
                .into_iter()
                .map(|p| TorchPythonChoice {
                    id: p.id,
                    label: p.label,
                })
                .collect(),
            adapters: options.adapters,
            default_adapter: options.default_adapter,
            verified: options
                .verified
                .into_iter()
                .map(|record| TorchVerifiedRuntime {
                    tag: record.tag,
                    build: record.build,
                    python: record.python,
                    adapters: record.adapters,
                    verified_on: record.verified_on,
                    scope: record.scope,
                    source_commit: record.source_commit,
                    packages: record
                        .packages
                        .into_iter()
                        .map(|package| TorchVerifiedPackage {
                            name: package.name,
                            version: package.version,
                        })
                        .collect(),
                })
                .collect(),
            adapter_requirements: options
                .adapter_requirements
                .into_iter()
                .map(|requirement| TorchAdapterRequirement {
                    adapter: requirement.adapter,
                    tag: requirement.tag,
                    build: requirement.build,
                    python: requirement.python,
                })
                .collect(),
            installed: options
                .installed
                .into_iter()
                .map(|runtime| TorchInstalledRuntime {
                    tag: runtime.tag,
                    build: runtime.build,
                    python: runtime.python,
                    adapter: runtime.adapter,
                    qualification: if runtime.qualified {
                        super::TorchRuntimePreviewQualification::Qualified
                    } else {
                        super::TorchRuntimePreviewQualification::Unverified
                    },
                })
                .collect(),
        })
    }
}

/// The saved install-time probe and whether it still describes the runtime.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchRuntimeProbeOutcome {
    /// RFC 3339 time the probe ran; null for reports older than this field.
    recorded_at: Option<String>,
    status: TorchProbeStatus,
    core_status: TorchProbeCoreStatus,
    adapter_status: TorchProbeAdapterStatus,
    capabilities: BTreeMap<String, TorchCapabilityProbe>,
    stale: bool,
    #[serde(rename = "staleReasons")]
    stale_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum TorchProbeStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum TorchProbeCoreStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum TorchProbeAdapterStatus {
    #[serde(rename = "not selected")]
    NotSelected,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "inconclusive")]
    Inconclusive,
}

/// One saved check. `status` is display text (for example `not tested`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchCapabilityProbe {
    status: String,
    scope: Option<String>,
    reason: Option<String>,
    error: Option<String>,
    /// Version the check measured, such as the imported Torch build.
    version: Option<String>,
}

#[cfg(feature = "inference-plugins")]
impl From<domain::TorchInstalledProbeReport> for TorchRuntimeProbeOutcome {
    fn from(report: domain::TorchInstalledProbeReport) -> Self {
        use domain::{TorchProbeAdapterStatus as Adapter, TorchProbeCoreStatus as Core};
        Self {
            recorded_at: report.recorded_at,
            status: match report.status {
                domain::TorchProbeStatus::Passed => TorchProbeStatus::Passed,
                domain::TorchProbeStatus::Partial => TorchProbeStatus::Partial,
                domain::TorchProbeStatus::Failed => TorchProbeStatus::Failed,
            },
            core_status: match report.core_status {
                Core::Passed => TorchProbeCoreStatus::Passed,
                Core::Failed => TorchProbeCoreStatus::Failed,
            },
            adapter_status: match report.adapter_status {
                Adapter::NotSelected => TorchProbeAdapterStatus::NotSelected,
                Adapter::Unavailable => TorchProbeAdapterStatus::Unavailable,
                Adapter::Inconclusive => TorchProbeAdapterStatus::Inconclusive,
            },
            capabilities: report
                .capabilities
                .into_iter()
                .map(|(name, check)| {
                    (
                        name,
                        TorchCapabilityProbe {
                            status: check.status,
                            scope: check.scope,
                            reason: check.reason,
                            error: check.error,
                            version: check.version,
                        },
                    )
                })
                .collect(),
            stale: !report.stale_reasons.is_empty(),
            stale_reasons: report.stale_reasons,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum TorchTrialStartupStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum TorchTrialHealthStatus {
    Passed,
    Failed,
    NotChecked,
}

/// What happened to a process the trial started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum TorchTrialCleanup {
    NotNeeded,
    StoppedOwnedGeneration,
    ManualStopRequired,
}

/// Exact wheels for the selected Torch release on other builds or Pythons.
/// Dependencies are never resolved here, so the result is always incomplete.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchAlternativesOutcome {
    selected_tag: String,
    selected_build: String,
    selected_python: String,
    status: TorchAlternativesStatus,
    /// Always true.
    incomplete: bool,
    /// Always true.
    dependencies_not_checked: bool,
    checked_builds: Vec<String>,
    matches: Vec<TorchAlternativeMatch>,
    issues: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum TorchAlternativesStatus {
    Matches,
    None,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct TorchAlternativeMatch {
    tag: String,
    build: String,
    python: String,
    wheel_url: Option<String>,
    sha256: Option<String>,
}

#[cfg(feature = "inference-plugins")]
impl TryFrom<domain::TorchAlternativeDiscovery> for TorchAlternativesOutcome {
    type Error = PumasError;

    fn try_from(discovery: domain::TorchAlternativeDiscovery) -> Result<Self, Self::Error> {
        let status = match discovery.status.as_str() {
            "matches" => TorchAlternativesStatus::Matches,
            "none" => TorchAlternativesStatus::None,
            "inconclusive" => TorchAlternativesStatus::Inconclusive,
            _ => return Err(invalid_domain_outcome("Torch alternatives status")),
        };
        if !discovery.incomplete || !discovery.dependencies_not_checked {
            return Err(invalid_domain_outcome("Torch alternatives completeness"));
        }
        Ok(Self {
            selected_tag: discovery.selected_tag,
            selected_build: discovery.selected_build,
            selected_python: discovery.selected_python,
            status,
            incomplete: true,
            dependencies_not_checked: true,
            checked_builds: discovery.checked_builds,
            matches: discovery
                .matches
                .into_iter()
                .map(|m| TorchAlternativeMatch {
                    tag: m.tag,
                    build: m.build,
                    python: m.python,
                    wheel_url: m.wheel_url,
                    sha256: m.sha256,
                })
                .collect(),
            issues: discovery.issues,
        })
    }
}

/// Producer-made samples for the desktop conformance tests.
#[cfg(feature = "export-contract")]
pub(crate) fn torch_runtime_fixtures() -> serde_json::Result<serde_json::Value> {
    let options = TorchRuntimeOptionsOutcome {
        builds: vec!["cu130".into(), "cpu".into()],
        default_build: "auto".into(),
        pythons: vec![TorchPythonChoice {
            id: "auto".into(),
            label: "Select automatically".into(),
        }],
        adapters: vec!["none".into(), "flux2".into(), "nunchaku".into()],
        default_adapter: "none".into(),
        verified: vec![TorchVerifiedRuntime {
            tag: "v2.9.1".into(),
            build: "cu130".into(),
            python: "python3.12".into(),
            adapters: vec!["flux2".into(), "nunchaku".into()],
            verified_on: "2026-09-23".into(),
            scope: "CUDA execution and adapter imports; no model loaded".into(),
            source_commit: "5d373bf8791c43f1b995f4ba51a17da1610a6686".into(),
            packages: vec![TorchVerifiedPackage {
                name: "diffusers".into(),
                version: "0.37.0".into(),
            }],
        }],
        adapter_requirements: vec![TorchAdapterRequirement {
            adapter: "nunchaku".into(),
            tag: "v2.9.1".into(),
            build: "cu130".into(),
            python: "python3.12".into(),
        }],
        installed: vec![
            TorchInstalledRuntime {
                tag: "v2.9.1".into(),
                build: Some("cu130".into()),
                python: Some("python3.12".into()),
                adapter: Some("bundled".into()),
                qualification: super::TorchRuntimePreviewQualification::Qualified,
            },
            TorchInstalledRuntime {
                tag: "v2.10.0".into(),
                build: None,
                python: None,
                adapter: None,
                qualification: super::TorchRuntimePreviewQualification::Unverified,
            },
        ],
    };
    let probe = TorchRuntimeProbeOutcome {
        recorded_at: Some("2026-10-01T12:00:00+00:00".into()),
        status: TorchProbeStatus::Partial,
        core_status: TorchProbeCoreStatus::Passed,
        adapter_status: TorchProbeAdapterStatus::NotSelected,
        capabilities: BTreeMap::from([
            (
                "cuda_tensor".into(),
                TorchCapabilityProbe {
                    status: "unavailable".into(),
                    scope: Some("CUDA tensor operation".into()),
                    reason: Some("CUDA is not available".into()),
                    error: None,
                    version: None,
                },
            ),
            (
                "sidecar_startup".into(),
                TorchCapabilityProbe {
                    status: "not tested".into(),
                    scope: Some("socket startup requires an explicit trial".into()),
                    reason: None,
                    error: None,
                    version: None,
                },
            ),
            (
                "torch_import".into(),
                TorchCapabilityProbe {
                    status: "passed".into(),
                    scope: None,
                    reason: None,
                    error: None,
                    version: Some("2.9.1+cu130".into()),
                },
            ),
        ]),
        stale: true,
        stale_reasons: vec!["Installed distributions changed".into()],
    };
    let alternatives = TorchAlternativesOutcome {
        selected_tag: "v2.10.0".into(),
        selected_build: "cu130".into(),
        selected_python: "python3.12".into(),
        status: TorchAlternativesStatus::Matches,
        incomplete: true,
        dependencies_not_checked: true,
        checked_builds: vec!["cu128".into(), "cu130".into()],
        matches: vec![
            TorchAlternativeMatch {
                tag: "v2.10.0".into(),
                build: "cu128".into(),
                python: "python3.12".into(),
                wheel_url: Some("https://download.pytorch.org/whl/cu128/torch-fixture.whl".into()),
                sha256: Some("b".repeat(64)),
            },
            TorchAlternativeMatch {
                tag: "v2.10.0".into(),
                build: "cu130".into(),
                python: "python3.11".into(),
                wheel_url: None,
                sha256: None,
            },
        ],
        issues: vec!["cu126: no wheel for python3.12".into()],
    };
    let trial = super::TrialTorchRuntimeOutcome {
        success: false,
        tag: "v2.10.0".into(),
        profile_id: "torch.profile".into(),
        startup_status: TorchTrialStartupStatus::Passed,
        health_status: TorchTrialHealthStatus::Failed,
        protocol: Some(3),
        capabilities: vec!["image-generation".into()],
        generation: Some("42".into()),
        started_by_trial: true,
        error: Some("Managed Torch profile did not become healthy within 60 seconds".into()),
        cleanup: TorchTrialCleanup::StoppedOwnedGeneration,
    };
    Ok(serde_json::json!({
        "torch_runtime_options": options,
        "torch_runtime_probe": probe,
        "torch_alternatives": alternatives,
        "torch_trial_failed": trial,
    }))
}

#[cfg(all(test, feature = "inference-plugins"))]
mod tests {
    use super::*;

    #[test]
    fn options_project_qualification_and_reject_an_unoffered_default() {
        let options = domain::TorchRuntimeOptions {
            builds: vec!["cpu".into()],
            default_build: "auto".into(),
            pythons: Vec::new(),
            adapters: vec!["none".into()],
            default_adapter: "none".into(),
            verified: vec![domain::TorchVerifiedRuntime {
                tag: "v2.9.1".into(),
                build: "cu130".into(),
                python: "python3.12".into(),
                adapters: vec!["nunchaku".into()],
                verified_on: "2026-09-23".into(),
                scope: "adapter imports".into(),
                source_commit: "5d373bf8".into(),
                packages: vec![domain::TorchVerifiedPackage {
                    name: "pillow".into(),
                    version: "12.1.0".into(),
                }],
            }],
            adapter_requirements: Vec::new(),
            installed: vec![domain::TorchInstalledRuntime {
                tag: "v2.10.0".into(),
                build: None,
                python: None,
                adapter: None,
                qualified: false,
            }],
        };
        let wire =
            serde_json::to_value(TorchRuntimeOptionsOutcome::try_from(options.clone()).unwrap())
                .unwrap();
        assert_eq!(wire["defaultAdapter"], "none");
        assert_eq!(wire["installed"][0]["qualification"], "unverified");
        assert!(wire["installed"][0]["build"].is_null());
        assert_eq!(wire["verified"][0]["verifiedOn"], "2026-09-23");
        assert_eq!(wire["verified"][0]["sourceCommit"], "5d373bf8");
        assert_eq!(wire["verified"][0]["packages"][0]["version"], "12.1.0");

        let unoffered = domain::TorchRuntimeOptions {
            default_adapter: "flux2".into(),
            ..options.clone()
        };
        assert!(TorchRuntimeOptionsOutcome::try_from(unoffered).is_err());
        let unoffered_requirement = domain::TorchRuntimeOptions {
            adapter_requirements: vec![domain::TorchAdapterRequirement {
                adapter: "nunchaku".into(),
                tag: "v2.9.1".into(),
                build: "cu130".into(),
                python: "python3.12".into(),
            }],
            ..options
        };
        assert!(TorchRuntimeOptionsOutcome::try_from(unoffered_requirement).is_err());
    }

    #[test]
    fn probe_projects_only_displayed_fields_and_derives_staleness() {
        let mut report: domain::TorchInstalledProbeReport = serde_json::from_value(serde_json::json!({
            "status": "failed", "core_status": "failed", "adapter_status": "unavailable",
            "context": {"installed_distributions": {"torch": "2.9.1"}},
            "capabilities": {"torch_import": {"status": "failed", "error": "No module named torch", "version": null}},
        }))
        .unwrap();
        let current = serde_json::to_value(TorchRuntimeProbeOutcome::from(report.clone())).unwrap();
        assert_eq!(current["stale"], false);
        assert_eq!(current["staleReasons"], serde_json::json!([]));
        assert!(current.get("context").is_none());
        assert_eq!(
            current["capabilities"]["torch_import"],
            serde_json::json!({"status": "failed", "scope": null, "reason": null, "error": "No module named torch", "version": null})
        );

        report.stale_reasons = vec!["Hardware changed".into()];
        let stale = serde_json::to_value(TorchRuntimeProbeOutcome::from(report)).unwrap();
        assert_eq!(stale["stale"], true);
        assert_eq!(stale["adapter_status"], "unavailable");
    }

    #[test]
    fn alternatives_require_a_known_status_and_incomplete_scope() {
        let discovery = domain::TorchAlternativeDiscovery {
            selected_tag: "v2.10.0".into(),
            selected_build: "cu130".into(),
            selected_python: "python3.12".into(),
            status: "none".into(),
            incomplete: true,
            dependencies_not_checked: true,
            checked_builds: vec!["cu130".into()],
            matches: Vec::new(),
            issues: Vec::new(),
        };
        let wire =
            serde_json::to_value(TorchAlternativesOutcome::try_from(discovery.clone()).unwrap())
                .unwrap();
        assert_eq!(wire["status"], "none");
        assert_eq!(wire["dependenciesNotChecked"], true);
        for invalid in [
            domain::TorchAlternativeDiscovery {
                status: "complete".into(),
                ..discovery.clone()
            },
            domain::TorchAlternativeDiscovery {
                incomplete: false,
                ..discovery
            },
        ] {
            assert!(TorchAlternativesOutcome::try_from(invalid).is_err());
        }
    }
}
