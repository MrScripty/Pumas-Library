//! Bounded discovery of official Torch wheels for installed Python interpreters.
//! A wheel match is only a lead; dependencies and adapters are not resolved.

use super::*;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

const BUILDS: &[&str] = &[
    "cpu", "cu118", "cu121", "cu124", "cu126", "cu128", "cu130", "rocm6.1", "rocm6.2", "rocm6.3",
    "rocm6.4", "rocm7.0", "rocm7.1",
];
const PYTHONS: &[&str] = &["python3.10", "python3.11", "python3.12", "python3.13"];
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(45);
const PYTHON_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchAlternativeMatch {
    pub tag: String,
    pub build: String,
    pub python: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wheel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchAlternativeDiscovery {
    pub selected_tag: String,
    pub selected_build: String,
    pub selected_python: String,
    pub status: String,
    pub incomplete: bool,
    pub dependencies_not_checked: bool,
    pub checked_builds: Vec<String>,
    pub matches: Vec<TorchAlternativeMatch>,
    pub issues: Vec<String>,
}

fn stable_version(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        Some(version)
    } else {
        None
    }
}

fn unsupported_input(message: &str) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

async fn installed_pythons() -> Vec<&'static str> {
    let mut available = Vec::new();
    for python in PYTHONS {
        let mut command = Command::new(python);
        command.kill_on_drop(true).args([
            "-I",
            "-c",
            "import platform,sys; print(f'{sys.version_info.major}.{sys.version_info.minor}' if sys.platform == 'linux' and platform.machine() == 'x86_64' and sys.implementation.name == 'cpython' else '')",
        ]);
        if let Ok(Ok(output)) = tokio::time::timeout(PYTHON_CHECK_TIMEOUT, command.output()).await {
            let expected = python.strip_prefix("python").unwrap_or("");
            if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == expected
            {
                available.push(*python);
            }
        }
    }
    available
}

impl VersionManager {
    /// Find a few official binary Torch wheel leads for the selected stable tag.
    /// This never claims a dependency-compatible install plan.
    pub async fn discover_torch_alternatives(
        &self,
        selected_tag: &str,
        selected_build: &str,
        selected_python: &str,
    ) -> Result<TorchAlternativeDiscovery> {
        if self.app_id != AppId::Torch {
            return Err(unsupported_input(
                "Torch alternatives require a Torch version manager",
            ));
        }
        let version = stable_version(selected_tag)
            .ok_or_else(|| unsupported_input("Select a stable upstream Torch tag"))?;
        if !BUILDS.contains(&selected_build) || !PYTHONS.contains(&selected_python) {
            return Err(unsupported_input(
                "Select a supported Torch build and Python version",
            ));
        }
        let interpreters = installed_pythons().await;
        if interpreters.is_empty() {
            return Ok(TorchAlternativeDiscovery {
                selected_tag: selected_tag.into(),
                selected_build: selected_build.into(),
                selected_python: selected_python.into(),
                status: "inconclusive".into(),
                incomplete: true,
                dependencies_not_checked: true,
                checked_builds: Vec::new(),
                matches: Vec::new(),
                issues: vec![
                    "No installed CPython interpreter on Linux x86_64 can inspect official wheels"
                        .into(),
                ],
            });
        }
        let workspace = tempfile::tempdir().map_err(PumasError::from)?;
        let resolver = workspace.path().join("resolve_runtime.py");
        std::fs::write(
            &resolver,
            include_str!("../../../../../torch-server/resolve_runtime.py"),
        )
        .map_err(PumasError::from)?;
        let mut command = Command::new(interpreters[0]);
        command.kill_on_drop(true).arg("-I").arg(&resolver).args([
            "--discover",
            "--version",
            version,
            "--build",
            selected_build,
            "--selected-python",
            selected_python,
        ]);
        for interpreter in &interpreters {
            command.arg("--interpreter").arg(interpreter);
        }
        let output = tokio::time::timeout(DISCOVERY_TIMEOUT, command.output())
            .await
            .map_err(|_| unsupported_input("Official Torch wheel discovery timed out"))?
            .map_err(PumasError::from)?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(unsupported_input(&format!(
                "Official Torch wheel discovery failed: {}",
                detail.chars().take(500).collect::<String>()
            )));
        }
        let result: TorchAlternativeDiscovery =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                unsupported_input(&format!("Invalid Torch discovery result: {error}"))
            })?;
        if result.matches.len() > 3
            || !result.incomplete
            || !result.dependencies_not_checked
            || result.selected_tag != format!("v{version}")
            || result.selected_build != selected_build
            || result.selected_python != selected_python
            || result.matches.iter().any(|candidate| {
                candidate.tag != result.selected_tag
                    || !BUILDS.contains(&candidate.build.as_str())
                    || !interpreters.contains(&candidate.python.as_str())
            })
        {
            return Err(unsupported_input(
                "Torch discovery violated its bounded wheel-only contract",
            ));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_stable_three_part_versions_are_accepted() {
        assert_eq!(stable_version("v2.10.0"), Some("2.10.0"));
        assert_eq!(stable_version("2.10.0"), Some("2.10.0"));
        for value in ["2.10", "2.10.0rc1", "../2.10.0", "v2.10.0+cu130"] {
            assert_eq!(stable_version(value), None);
        }
    }

    #[test]
    fn wheel_only_contract_deserializes_without_claiming_dependency_resolution() {
        let result: TorchAlternativeDiscovery = serde_json::from_value(serde_json::json!({
            "selectedTag":"v2.10.0", "selectedBuild":"cu130", "selectedPython":"python3.13",
            "status":"matches", "incomplete":true, "dependenciesNotChecked":true,
            "checkedBuilds":["cu130"], "matches":[{
                "tag":"v2.10.0", "build":"cu130", "python":"python3.12",
                "wheelUrl":"https://download.pytorch.org/whl/cu130/torch.whl",
                "sha256":null
            }], "issues":["Dependencies and adapters were not resolved"]
        }))
        .unwrap();
        assert_eq!(result.matches.len(), 1);
        assert!(result.incomplete && result.dependencies_not_checked);
    }
}
