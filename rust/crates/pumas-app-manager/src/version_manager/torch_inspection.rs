//! Local Torch install choices and the saved install-time probe report.
//!
//! Neither reads the network. The RPC layer projects both into its closed
//! desktop wire types.

use serde::Deserialize;
use std::collections::BTreeMap;

/// Choices the install dialog offers, plus the configuration of each
/// installed runtime as recorded in its `runtime.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchRuntimeOptions {
    pub builds: Vec<String>,
    pub default_build: String,
    pub pythons: Vec<TorchPythonChoice>,
    /// Accepted for older callers: `none` plus the offered extras. Every
    /// install carries the general modalities, so no other adapter is offered.
    pub adapters: Vec<String>,
    pub default_adapter: String,
    /// General modalities every install resolves together.
    pub modalities: Vec<String>,
    /// Optional extras offered on this platform. An extra with a matching
    /// `adapter_requirements` entry installs only on that selection.
    pub extras: Vec<String>,
    /// Combinations verified on this platform. Evidence only: choosing one
    /// still resolves current package versions.
    pub verified: Vec<super::torch_verified::TorchVerifiedRuntime>,
    /// Offered adapters that install only on one selection, because the
    /// adapter's own binary wheel was built for it.
    pub adapter_requirements: Vec<TorchAdapterRequirement>,
    pub installed: Vec<TorchInstalledRuntime>,
}

/// The one Torch release, build and managed Python an adapter installs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchAdapterRequirement {
    pub adapter: String,
    pub tag: String,
    pub build: String,
    pub python: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchPythonChoice {
    pub id: String,
    pub label: String,
}

/// An installed runtime. Fields are `None` when its recipe does not record them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchInstalledRuntime {
    pub tag: String,
    pub build: Option<String>,
    pub python: Option<String>,
    pub adapter: Option<String>,
    /// Optional extras the runtime installed; `None` when it records neither
    /// extras nor a former adapter.
    pub extras: Option<Vec<String>>,
    /// What the install-time probe found importable, per capability.
    pub capabilities: Vec<TorchRuntimeCapability>,
    /// True when the runtime's recorded artifacts are a version match for a
    /// verified combination: every recorded package at its recorded version.
    /// A runtime from the former fixed v2.9.1 preset qualifies only through
    /// its recorded artifacts, never by its recipe ID.
    pub qualified: bool,
}

/// One capability of an installed runtime, from what its install-time probe
/// could import. `Unknown` marks a runtime probed before every install
/// carried the general modalities; reinstalling it probes it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchRuntimeCapability {
    /// `text`, `image` or `audio` for a general modality, or a model family
    /// such as `flux2_klein`, `cohere_asr` or `nunchaku_z_image`.
    pub name: String,
    pub status: TorchCapabilityStatus,
    /// Why the capability is unavailable or unknown.
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TorchCapabilityStatus {
    Available,
    Unavailable,
    Unknown,
}

/// Capabilities from a saved `probe-results.json` value. General modalities
/// a probe from before unified installs did not record are `Unknown`, as are
/// model families it did not probe; the nunchaku extra is listed only when
/// the runtime installed it.
pub(crate) fn runtime_capabilities(
    probe: &serde_json::Value,
    extras: &[String],
) -> Vec<TorchRuntimeCapability> {
    const LEGACY: &str = "Probed before every Torch install carried text, image and audio \
         packages; reinstall it to add and check them";
    let mut names: Vec<(&str, &str)> = vec![
        ("text", "text_modality"),
        ("image", "image_modality"),
        ("audio", "audio_modality"),
        ("flux2_klein", "flux2_klein"),
        ("cohere_asr", "cohere_asr"),
    ];
    if extras.iter().any(|extra| extra == "nunchaku") {
        names.push(("nunchaku_z_image", "nunchaku_z_image"));
    }
    names
        .into_iter()
        .map(|(name, key)| {
            let check = &probe["capabilities"][key];
            let text = |field: &str| check[field].as_str().map(str::to_owned);
            let (status, reason) = match check["status"].as_str() {
                Some("passed" | "inconclusive") => (TorchCapabilityStatus::Available, None),
                Some("unavailable" | "failed") => (
                    TorchCapabilityStatus::Unavailable,
                    text("reason")
                        .or_else(|| text("error"))
                        .or_else(|| Some("Its packages did not import".to_owned())),
                ),
                _ => (TorchCapabilityStatus::Unknown, Some(LEGACY.to_owned())),
            };
            TorchRuntimeCapability {
                name: name.to_owned(),
                status,
                reason,
            }
        })
        .collect()
}

/// `probe-results.json`, written by `torch-server/probe_runtime.py` at install
/// time. Saved reports outlive the probe that wrote them, so fields this
/// reader does not use are ignored.
#[derive(Clone, Debug, Deserialize)]
pub struct TorchInstalledProbeReport {
    #[serde(default)]
    pub recorded_at: Option<String>,
    pub status: TorchProbeStatus,
    pub core_status: TorchProbeCoreStatus,
    pub adapter_status: TorchProbeAdapterStatus,
    pub capabilities: BTreeMap<String, TorchCapabilityProbe>,
    /// Why the saved checks no longer describe this runtime; empty when current.
    #[serde(skip)]
    pub stale_reasons: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorchProbeStatus {
    Passed,
    Partial,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorchProbeCoreStatus {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum TorchProbeAdapterStatus {
    #[serde(rename = "not selected")]
    NotSelected,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "inconclusive")]
    Inconclusive,
}

/// One saved capability check. `status` stays open text: the probe adds
/// checks and outcomes (for example `not tested`) that are only displayed.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct TorchCapabilityProbe {
    pub status: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    /// Version a check measured, such as the imported Torch build.
    #[serde(default)]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_reports_ignore_unused_fields_and_require_known_statuses() {
        let saved = serde_json::json!({
            "status": "partial", "core_status": "passed", "adapter_status": "not selected",
            "environment": {"adapter": "none"}, "context": {"driver_version": null},
            "capabilities": {
                "torch_import": {"status": "passed", "version": "2.9.1"},
                "cuda_tensor": {"status": "unavailable", "scope": "CUDA tensor operation",
                    "reason": "CUDA is not available"},
            },
        });
        let report: TorchInstalledProbeReport = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(report.recorded_at, None);
        assert_eq!(report.adapter_status, TorchProbeAdapterStatus::NotSelected);
        assert_eq!(report.capabilities["torch_import"].scope, None);
        assert_eq!(
            report.capabilities["torch_import"].version.as_deref(),
            Some("2.9.1")
        );
        assert_eq!(
            report.capabilities["cuda_tensor"].reason.as_deref(),
            Some("CUDA is not available")
        );

        let mut unknown = saved;
        unknown["core_status"] = "partial".into();
        let error = serde_json::from_value::<TorchInstalledProbeReport>(unknown).unwrap_err();
        assert!(error.to_string().contains("partial"), "{error}");
    }

    #[test]
    fn capabilities_come_from_what_the_probe_imported() {
        let status_of = |capabilities: &[TorchRuntimeCapability], name: &str| {
            capabilities
                .iter()
                .find(|capability| capability.name == name)
                .map(|capability| (capability.status, capability.reason.clone()))
        };
        let current = serde_json::json!({"capabilities": {
            "text_modality": {"status": "inconclusive"},
            "image_modality": {"status": "unavailable", "reason": "No compatible binary wheel for torchvision"},
            "audio_modality": {"status": "inconclusive"},
            "flux2_klein": {"status": "unavailable", "error": "Flux2KleinPipeline is unavailable"},
            "cohere_asr": {"status": "inconclusive"},
            "nunchaku_z_image": {"status": "not selected"},
        }});
        let capabilities = runtime_capabilities(&current, &[]);
        assert_eq!(
            status_of(&capabilities, "text"),
            Some((TorchCapabilityStatus::Available, None))
        );
        assert_eq!(
            status_of(&capabilities, "image"),
            Some((
                TorchCapabilityStatus::Unavailable,
                Some("No compatible binary wheel for torchvision".into())
            ))
        );
        assert_eq!(
            status_of(&capabilities, "flux2_klein")
                .unwrap()
                .1
                .as_deref(),
            Some("Flux2KleinPipeline is unavailable")
        );
        // An extra is listed only when the runtime installed it.
        assert!(status_of(&capabilities, "nunchaku_z_image").is_none());
        let with_extra = runtime_capabilities(&current, &["nunchaku".to_owned()]);
        assert_eq!(
            status_of(&with_extra, "nunchaku_z_image").unwrap().0,
            TorchCapabilityStatus::Unknown
        );

        // A probe from before unified installs recorded no modalities.
        let legacy = serde_json::json!({"capabilities": {
            "flux2_klein": {"status": "inconclusive"},
            "cohere_asr": {"status": "not selected"},
        }});
        let legacy = runtime_capabilities(&legacy, &[]);
        let (status, reason) = status_of(&legacy, "text").unwrap();
        assert_eq!(status, TorchCapabilityStatus::Unknown);
        assert!(reason.unwrap().contains("reinstall it"));
        assert_eq!(
            status_of(&legacy, "flux2_klein").unwrap().0,
            TorchCapabilityStatus::Available
        );
        assert_eq!(
            status_of(&legacy, "cohere_asr").unwrap().0,
            TorchCapabilityStatus::Unknown
        );
        // No saved probe at all.
        assert!(runtime_capabilities(&serde_json::Value::Null, &[])
            .iter()
            .all(|capability| capability.status == TorchCapabilityStatus::Unknown));
    }
}
