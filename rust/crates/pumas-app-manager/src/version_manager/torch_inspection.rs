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
    pub adapters: Vec<String>,
    pub bundled_preset_available: bool,
    pub default_adapter: String,
    pub preset: TorchRuntimeSelection,
    pub installed: Vec<TorchInstalledRuntime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchPythonChoice {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchRuntimeSelection {
    pub tag: String,
    pub build: String,
    pub python: String,
    pub adapter: String,
}

/// An installed runtime. Fields are `None` when its recipe does not record them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorchInstalledRuntime {
    pub tag: String,
    pub build: Option<String>,
    pub python: Option<String>,
    pub adapter: Option<String>,
    /// True only for the qualified bundled preset.
    pub qualified: bool,
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
}
