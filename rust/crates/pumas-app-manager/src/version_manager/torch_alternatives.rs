//! Bounded discovery of official Torch wheels for managed Python candidates.
//! A wheel match is only a lead; dependencies and adapters are not resolved.

use super::managed_python::{
    ensure_managed_torch_interpreter as provision_managed_torch_interpreter,
    list_managed_torch_candidates, ManagedPythonFailure, ManagedPythonIdentity,
};
use super::torch_preview::valid_torch_channel;
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io;
use tokio::process::Command;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(45);
const PYTHON_CHECK_TIMEOUT: Duration = Duration::from_secs(3);
// Leave room for interpreter checks, the bounded index scan, and process startup.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TorchHostTarget {
    LinuxX8664,
    WindowsX8664,
    MacosArm64,
}

impl TorchHostTarget {
    fn current() -> Option<Self> {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::LinuxX8664)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Self::WindowsX8664)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Self::MacosArm64)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TorchReleaseOptionsStatus {
    Matches,
    None,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TorchReleaseDriverAvailability {
    Available,
    Unavailable,
    NotPresent,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchReleaseDriverStatus {
    pub nvidia: TorchReleaseDriverAvailability,
    pub amd: TorchReleaseDriverAvailability,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchReleaseCombination {
    pub build: String,
    pub python: String,
    pub wheel_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchReleaseRecommendation {
    pub build: String,
    pub python: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchReleaseOptionsDiscovery {
    pub tag: String,
    pub status: TorchReleaseOptionsStatus,
    pub complete_scan: bool,
    pub checked_channels: Vec<String>,
    pub combinations: Vec<TorchReleaseCombination>,
    pub issues: Vec<String>,
    pub detected_gpu_vendors: Vec<String>,
    pub driver_status: TorchReleaseDriverStatus,
    pub recommended: Option<TorchReleaseRecommendation>,
    pub recommendation_note: String,
}

fn pci_display_vendors(
    devices_root: &Path,
    read: impl Fn(&Path) -> io::Result<String>,
) -> io::Result<Vec<String>> {
    let mut vendors = BTreeSet::new();
    for entry in std::fs::read_dir(devices_root)? {
        let path = entry?.path();
        let class = read(&path.join("class"))?;
        let class = u32::from_str_radix(class.trim().trim_start_matches("0x"), 16)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid PCI class"))?;
        if class >> 16 != 0x03 {
            continue;
        }
        let vendor = read(&path.join("vendor"))?;
        let vendor = u16::from_str_radix(vendor.trim().trim_start_matches("0x"), 16)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid PCI vendor"))?;
        match vendor {
            0x10de => {
                vendors.insert("nvidia".to_owned());
            }
            0x1002 => {
                vendors.insert("amd".to_owned());
            }
            0x8086 => {
                vendors.insert("intel".to_owned());
            }
            _ => {}
        }
    }
    Ok(vendors.into_iter().collect())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NvidiaDriverVersion(u32, u32, u32);

#[derive(Clone, Copy, Debug)]
struct NvidiaDriverProbe {
    availability: TorchReleaseDriverAvailability,
}

fn parse_nvidia_driver_version(text: &str) -> Option<NvidiaDriverVersion> {
    let parts = text.trim().split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let mut numbers = [0_u32; 3];
    for (index, part) in parts.iter().enumerate() {
        numbers[index] = part.parse().ok()?;
    }
    Some(NvidiaDriverVersion(numbers[0], numbers[1], numbers[2]))
}

fn parse_nvidia_driver_output(stdout: &[u8]) -> Option<NvidiaDriverVersion> {
    let output = std::str::from_utf8(stdout).ok()?;
    let mut versions = output.lines().map(parse_nvidia_driver_version);
    let mut lowest = versions.next()??;
    for version in versions {
        lowest = lowest.min(version?);
    }
    Some(lowest)
}

async fn nvidia_driver_probe() -> NvidiaDriverProbe {
    let mut command = Command::new("nvidia-smi");
    command
        .kill_on_drop(true)
        .args(["--query-gpu=driver_version", "--format=csv,noheader"]);
    match tokio::time::timeout(PYTHON_CHECK_TIMEOUT, command.output()).await {
        Ok(Ok(output)) if output.status.success() => NvidiaDriverProbe {
            availability: if parse_nvidia_driver_output(&output.stdout).is_some() {
                TorchReleaseDriverAvailability::Available
            } else {
                TorchReleaseDriverAvailability::Unknown
            },
        },
        Ok(Ok(_)) => NvidiaDriverProbe {
            availability: TorchReleaseDriverAvailability::Unavailable,
        },
        Ok(Err(_)) | Err(_) => NvidiaDriverProbe {
            availability: TorchReleaseDriverAvailability::Unknown,
        },
    }
}

fn managed_python_error(error: ManagedPythonFailure) -> PumasError {
    unsupported_input(error.message)
}

fn catalog_python_minor(python: &str, candidates: &[String]) -> Option<String> {
    let minor = python.strip_prefix("python")?;
    candidates
        .iter()
        .any(|candidate| candidate == minor)
        .then(|| minor.to_owned())
}

fn sort_catalog_minors(mut candidates: Vec<String>) -> Vec<String> {
    candidates.sort_by_key(|minor| {
        let (major, minor) = minor.split_once('.').unwrap_or(("0", "0"));
        (
            major.parse::<u32>().unwrap_or(0),
            minor.parse::<u32>().unwrap_or(0),
        )
    });
    candidates.dedup();
    candidates.reverse();
    candidates
}

pub(super) async fn managed_torch_candidate_minors(
    root: &Path,
    cleanup: &Arc<super::installer::TorchCleanupTasks>,
) -> Result<Vec<String>> {
    let candidates = list_managed_torch_candidates(root, cleanup)
        .await
        .map_err(managed_python_error)?;
    Ok(sort_catalog_minors(
        candidates
            .into_iter()
            .map(|candidate| candidate.minor)
            .collect(),
    ))
}

pub(super) async fn ensure_managed_torch_interpreter(
    root: &Path,
    cleanup: &Arc<super::installer::TorchCleanupTasks>,
    minor: &str,
) -> Result<ManagedPythonIdentity> {
    let candidates = managed_torch_candidate_minors(root, cleanup).await?;
    if !candidates.iter().any(|candidate| candidate == minor) {
        return Err(unsupported_input(
            "Select a Python version from the managed CPython catalog",
        ));
    }
    provision_managed_torch_interpreter(root, cleanup, minor)
        .await
        .map_err(managed_python_error)
}

fn valid_wheel_platform(platform: &str, target: TorchHostTarget) -> bool {
    match target {
        TorchHostTarget::LinuxX8664 => {
            platform == "linux_x86_64"
                || ((platform.starts_with("manylinux") || platform.starts_with("musllinux"))
                    && platform.ends_with("_x86_64"))
        }
        TorchHostTarget::WindowsX8664 => platform == "win_amd64",
        TorchHostTarget::MacosArm64 => {
            let Some(numbers) = platform.strip_prefix("macosx_").and_then(|value| {
                value
                    .strip_suffix("_arm64")
                    .or_else(|| value.strip_suffix("_universal2"))
            }) else {
                return false;
            };
            let Some((major, minor)) = numbers.split_once('_') else {
                return false;
            };
            major.parse::<u32>().is_ok_and(|value| value >= 11) && minor.parse::<u32>().is_ok()
        }
    }
}

fn valid_official_wheel(
    url: &str,
    build: &str,
    version: &str,
    python: &str,
    target: TorchHostTarget,
) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(filename) = url.path().rsplit('/').next() else {
        return false;
    };
    let Some(stem) = filename.strip_suffix(".whl") else {
        return false;
    };
    let mut fields = stem.rsplitn(4, '-');
    let (Some(platform), Some(abi), Some(python_tag), Some(distribution)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return false;
    };
    let Some(remainder) = distribution.strip_prefix(&format!("torch-{version}")) else {
        return false;
    };
    let expected_prefix = format!("/whl/{build}/");
    let artifact_path = url
        .path()
        .strip_prefix(&expected_prefix)
        .is_some_and(|path| path == filename || path.strip_prefix("torch/") == Some(filename));
    let expected_python = python.replace("python", "cp").replace('.', "");
    let exact_version = remainder.is_empty()
        || remainder.eq_ignore_ascii_case(&format!("%2B{build}"))
        || remainder.eq_ignore_ascii_case(&format!("+{build}"));
    let target_build = match target {
        TorchHostTarget::MacosArm64 => build == "cpu",
        TorchHostTarget::WindowsX8664 => build == "cpu" || build.starts_with("cu"),
        TorchHostTarget::LinuxX8664 => valid_torch_channel(build),
    };
    target_build
        && exact_version
        && python_tag.split('.').any(|tag| tag == expected_python)
        && !abi.is_empty()
        && platform
            .split('.')
            .any(|tag| valid_wheel_platform(tag, target))
        && url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("download.pytorch.org" | "download-r2.pytorch.org")
        )
        && matches!(url.port(), None | Some(443))
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && artifact_path
}

fn visible_build_for_target(build: &str, vendors: &[String], target: TorchHostTarget) -> bool {
    match target {
        TorchHostTarget::LinuxX8664 => visible_build(build, vendors),
        TorchHostTarget::WindowsX8664 => {
            build == "cpu"
                || (build.starts_with("cu") && vendors.iter().any(|vendor| vendor == "nvidia"))
        }
        TorchHostTarget::MacosArm64 => build == "cpu",
    }
}

fn visible_build(build: &str, vendors: &[String]) -> bool {
    build == "cpu"
        || (build.starts_with("cu") && vendors.iter().any(|vendor| vendor == "nvidia"))
        || (build.starts_with("rocm") && vendors.iter().any(|vendor| vendor == "amd"))
}

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

fn filter_alternatives_for_host(
    mut result: TorchAlternativeDiscovery,
    detected: io::Result<Vec<String>>,
    target: TorchHostTarget,
) -> TorchAlternativeDiscovery {
    let unknown_host = detected.is_err();
    let vendors = detected.unwrap_or_default();
    result
        .checked_builds
        .retain(|build| visible_build_for_target(build, &vendors, target));
    result
        .matches
        .retain(|item| visible_build_for_target(&item.build, &vendors, target));
    if unknown_host {
        result.status = "inconclusive".into();
        result
            .issues
            .push("GPU devices could not be inspected; GPU wheel leads were withheld".into());
    } else if result.matches.is_empty() && result.status == "matches" {
        result.status = "none".into();
    }
    result
}

fn validate_legacy_wheel_leads(
    mut result: TorchAlternativeDiscovery,
    interpreters: &[(String, PathBuf)],
    version: &str,
    target: TorchHostTarget,
) -> TorchAlternativeDiscovery {
    let before = result.matches.len();
    let checked_before = result.checked_builds.len();
    result
        .checked_builds
        .retain(|build| valid_torch_channel(build));
    result.matches.retain(|candidate| {
        candidate.tag == result.selected_tag
            && interpreters
                .iter()
                .any(|(python, _)| python == &candidate.python)
            && valid_torch_channel(&candidate.build)
            && candidate.wheel_url.as_ref().is_some_and(|url| {
                valid_official_wheel(url, &candidate.build, version, &candidate.python, target)
            })
            && candidate.sha256.as_ref().is_none_or(|hash| {
                hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    });
    if result.matches.len() != before || result.checked_builds.len() != checked_before {
        result.status = "inconclusive".into();
        result
            .issues
            .push("Some wheel leads could not be verified for this host and were withheld".into());
    }
    result
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

pub(super) fn stable_release_version(tag: &str) -> Option<&str> {
    stable_version(tag.strip_prefix('v')?)
}

fn unsupported_input(message: &str) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

fn inconclusive_alternatives(
    tag: &str,
    build: &str,
    python: &str,
    issue: &str,
) -> TorchAlternativeDiscovery {
    TorchAlternativeDiscovery {
        selected_tag: tag.into(),
        selected_build: build.into(),
        selected_python: python.into(),
        status: "inconclusive".into(),
        incomplete: true,
        dependencies_not_checked: true,
        checked_builds: Vec::new(),
        matches: Vec::new(),
        issues: vec![issue.into()],
    }
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
        let target = TorchHostTarget::current().ok_or_else(|| {
            unsupported_input(
                "Torch alternatives require Linux x86_64, Windows x86_64, or macOS arm64",
            )
        })?;
        if !valid_torch_channel(selected_build) {
            return Err(unsupported_input(
                "Select a supported Torch build and Python version",
            ));
        }
        let managed_root = self.launcher_root.join("launcher-data/managed-python");
        let candidates =
            match managed_torch_candidate_minors(&managed_root, &self.torch_cleanup).await {
                Ok(candidates) if !candidates.is_empty() => candidates,
                Ok(_) | Err(_) => {
                    return Ok(inconclusive_alternatives(
                        selected_tag,
                        selected_build,
                        selected_python,
                        "Managed CPython catalog could not be verified",
                    ));
                }
            };
        let Some(minor) = catalog_python_minor(selected_python, &candidates) else {
            return Err(unsupported_input(
                "Select a Python version from the managed CPython catalog",
            ));
        };
        let interpreter = match ensure_managed_torch_interpreter(
            &managed_root,
            &self.torch_cleanup,
            &minor,
        )
        .await
        {
            Ok(interpreter) => interpreter,
            Err(_) => {
                return Ok(inconclusive_alternatives(
                    selected_tag,
                    selected_build,
                    selected_python,
                    "Managed CPython interpreter could not be provisioned",
                ));
            }
        };
        let workspace = super::torch_workspace::create_launcher_workspace(&self.launcher_root)?;
        let resolver = workspace.path().join("resolve_runtime.py");
        std::fs::write(
            &resolver,
            include_str!("../../../../../torch-server/resolve_runtime.py"),
        )
        .map_err(PumasError::from)?;
        let mut command = Command::new(&interpreter.executable);
        command.args(["-I", "-B"]).arg(&resolver).args([
            "--discover",
            "--version",
            version,
            "--build",
            selected_build,
            "--selected-python",
            selected_python,
        ]);
        command.arg("--interpreter").arg(&interpreter.executable);
        let result = match super::torch_preview::run_preview_resolver(
            command,
            &workspace,
            DISCOVERY_TIMEOUT,
            &self.torch_cleanup,
        )
        .await
        {
            Ok(super::torch_preview::PreviewResolverRun::Exited(status)) if status.success() => {
                let output_path = workspace.path().join("resolver.stdout");
                let output = match std::fs::metadata(&output_path) {
                    Ok(metadata) if metadata.len() <= 4 * 1024 * 1024 => {
                        std::fs::read(output_path).map_err(PumasError::from)?
                    }
                    _ => {
                        return Ok(inconclusive_alternatives(
                            selected_tag,
                            selected_build,
                            selected_python,
                            "Official Torch wheel discovery returned an invalid result",
                        ));
                    }
                };
                serde_json::from_slice::<TorchAlternativeDiscovery>(&output)
            }
            Ok(super::torch_preview::PreviewResolverRun::Exited(_))
            | Ok(super::torch_preview::PreviewResolverRun::TimedOut)
            | Err(_) => {
                return Ok(inconclusive_alternatives(
                    selected_tag,
                    selected_build,
                    selected_python,
                    "Official Torch wheel discovery did not complete conclusively",
                ));
            }
        };
        let Ok(result) = result else {
            return Ok(inconclusive_alternatives(
                selected_tag,
                selected_build,
                selected_python,
                "Official Torch wheel discovery returned an invalid result",
            ));
        };
        if result.matches.len() > 3
            || !result.incomplete
            || !result.dependencies_not_checked
            || result.selected_tag != format!("v{version}")
            || result.selected_build != selected_build
            || result.selected_python != selected_python
            || !matches!(result.status.as_str(), "matches" | "none" | "inconclusive")
            || result.checked_builds.len() > 5
        {
            return Err(unsupported_input(
                "Torch discovery violated its bounded wheel-only contract",
            ));
        }
        let result = validate_legacy_wheel_leads(
            result,
            &[(interpreter.python, interpreter.executable)],
            version,
            target,
        );
        let detected = match target {
            TorchHostTarget::LinuxX8664 => {
                pci_display_vendors(Path::new("/sys/bus/pci/devices"), |path| {
                    std::fs::read_to_string(path)
                })
            }
            TorchHostTarget::WindowsX8664 => {
                if nvidia_driver_probe().await.availability
                    == TorchReleaseDriverAvailability::Available
                {
                    Ok(vec!["nvidia".into()])
                } else {
                    Err(io::Error::from(io::ErrorKind::NotFound))
                }
            }
            TorchHostTarget::MacosArm64 => Ok(Vec::new()),
        };
        Ok(filter_alternatives_for_host(result, detected, target))
    }
}

impl VersionManager {
    /// Return an immediate advisory result. This method performs no network,
    /// Python provisioning, or subprocess work; an unknown result never blocks
    /// the install request.
    pub async fn discover_torch_release_options(
        &self,
        tag: &str,
    ) -> Result<TorchReleaseOptionsDiscovery> {
        if self.app_id != AppId::Torch {
            return Err(unsupported_input(
                "Torch release options require a Torch manager",
            ));
        }
        stable_release_version(tag)
            .ok_or_else(|| unsupported_input("Select a stable upstream Torch tag"))?;
        if TorchHostTarget::current().is_none() {
            return Err(unsupported_input(
                "Torch release options require Linux x86_64, Windows x86_64, or macOS arm64",
            ));
        }
        Ok(TorchReleaseOptionsDiscovery {
            tag: tag.to_owned(),
            status: TorchReleaseOptionsStatus::Inconclusive,
            complete_scan: false,
            checked_channels: Vec::new(),
            combinations: Vec::new(),
            issues: vec![
                "Compatibility was not checked. Start installation to resolve packages.".to_owned(),
            ],
            detected_gpu_vendors: Vec::new(),
            driver_status: TorchReleaseDriverStatus {
                nvidia: TorchReleaseDriverAvailability::Unknown,
                amd: TorchReleaseDriverAvailability::Unknown,
            },
            recommended: None,
            recommendation_note: "Availability is resolved during installation.".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(build: &str, python: &str) -> TorchReleaseCombination {
        let abi = python.strip_prefix("python").unwrap().replace('.', "");
        TorchReleaseCombination {
            build: build.into(),
            python: python.into(),
            wheel_url: format!("https://download.pytorch.org/whl/{build}/torch/torch-2.14.0%2B{build}-cp{abi}-cp{abi}-manylinux_2_17_x86_64.whl"),
            sha256: None,
        }
    }

    #[test]
    fn release_options_wire_shape_and_official_wheel_validation() {
        let discovery = TorchReleaseOptionsDiscovery {
            tag: "v2.14.0".into(),
            status: TorchReleaseOptionsStatus::Matches,
            complete_scan: true,
            checked_channels: vec!["cpu".into(), "cu136".into()],
            combinations: vec![wheel("cu136", "python3.12")],
            issues: Vec::new(),
            detected_gpu_vendors: vec!["nvidia".into()],
            driver_status: TorchReleaseDriverStatus {
                nvidia: TorchReleaseDriverAvailability::Available,
                amd: TorchReleaseDriverAvailability::NotPresent,
            },
            recommended: Some(TorchReleaseRecommendation {
                build: "cu136".into(),
                python: "python3.12".into(),
            }),
            recommendation_note: String::new(),
        };
        let value = serde_json::to_value(discovery).unwrap();
        assert_eq!(value["status"], "matches");
        assert_eq!(value["completeScan"], true);
        assert!(value["combinations"][0]["wheelUrl"]
            .as_str()
            .unwrap()
            .contains("torch-2.14.0%2Bcu136"));
        assert!(valid_official_wheel(
            value["combinations"][0]["wheelUrl"].as_str().unwrap(),
            "cu136",
            "2.14.0",
            "python3.12",
            TorchHostTarget::LinuxX8664,
        ));
        assert!(value["combinations"][0].get("sha256").is_none());
        assert_eq!(value["driverStatus"]["nvidia"], "available");
        assert_eq!(value["driverStatus"]["amd"], "not_present");
        assert!(valid_official_wheel(
            "https://download.pytorch.org/whl/cu136/torch/torch-2.14.0%2Bcu136-cp312-cp312-manylinux_2_17_x86_64.whl",
            "cu136",
            "2.14.0",
            "python3.12",
            TorchHostTarget::LinuxX8664,
        ));
        assert!(!valid_official_wheel(
            "https://example.com/whl/cu136/torch/torch-2.14.0%2Bcu136-cp312-cp312-manylinux_2_17_x86_64.whl",
            "cu136",
            "2.14.0",
            "python3.12",
            TorchHostTarget::LinuxX8664,
        ));
        assert!(valid_official_wheel(
            "https://download-r2.pytorch.org/whl/cu130/torch-2.14.0%2Bcu130-cp310-cp310-manylinux_2_28_x86_64.whl",
            "cu130",
            "2.14.0",
            "python3.10",
            TorchHostTarget::LinuxX8664,
        ));
        assert!(valid_official_wheel(
            "https://download-r2.pytorch.org/whl/cpu/torch-2.14.0%2Bcpu-cp310-cp310-manylinux_2_28_x86_64.whl",
            "cpu",
            "2.14.0",
            "python3.10",
            TorchHostTarget::LinuxX8664,
        ));
        assert!(!valid_official_wheel(
            "https://download-r2.pytorch.org/whl/cu130/untrusted/torch-2.14.0%2Bcu130-cp310-cp310-manylinux_2_28_x86_64.whl",
            "cu130",
            "2.14.0",
            "python3.10",
            TorchHostTarget::LinuxX8664,
        ));
    }

    #[test]
    fn native_wheel_urls_are_release_and_target_bounded() {
        let win_cpu =
            "https://download.pytorch.org/whl/cpu/torch/torch-2.14.0-cp312-cp312-win_amd64.whl";
        let win_cuda = "https://download.pytorch.org/whl/cu132/torch/torch-2.14.0%2Bcu132-cp312-cp312-win_amd64.whl";
        let mac_cpu = "https://download.pytorch.org/whl/cpu/torch/torch-2.14.0-cp312-cp312-macosx_11_0_arm64.whl";
        assert!(valid_official_wheel(
            win_cpu,
            "cpu",
            "2.14.0",
            "python3.12",
            TorchHostTarget::WindowsX8664
        ));
        assert!(valid_official_wheel(
            win_cuda,
            "cu132",
            "2.14.0",
            "python3.12",
            TorchHostTarget::WindowsX8664
        ));
        assert!(valid_official_wheel(
            mac_cpu,
            "cpu",
            "2.14.0",
            "python3.12",
            TorchHostTarget::MacosArm64
        ));
        assert!(valid_official_wheel(
            "https://download.pytorch.org/whl/cpu/torch/torch-2.14.0-cp312-cp312-macosx_11_0_universal2.whl",
            "cpu",
            "2.14.0",
            "python3.12",
            TorchHostTarget::MacosArm64
        ));
        for (url, build, target) in [
            (win_cpu, "cpu", TorchHostTarget::LinuxX8664),
            (win_cuda, "cu132", TorchHostTarget::MacosArm64),
            (mac_cpu, "cpu", TorchHostTarget::WindowsX8664),
            ("https://evil.example/whl/cpu/torch/torch-2.14.0-cp312-cp312-win_amd64.whl", "cpu", TorchHostTarget::WindowsX8664),
            ("https://download.pytorch.org/whl/cu132/torch/torch-2.14.0%2Bcu132-cp312-cp312-win_arm64.whl", "cu132", TorchHostTarget::WindowsX8664),
            ("https://download.pytorch.org/whl/cpu/torch/torch-2.14.0-cp312-cp312-macosx_11_0_x86_64.whl", "cpu", TorchHostTarget::MacosArm64),
        ] {
            assert!(!valid_official_wheel(url, build, "2.14.0", "python3.12", target), "{url}");
        }
        assert!(!valid_official_wheel(
            win_cpu,
            "cpu",
            "2.13.0",
            "python3.12",
            TorchHostTarget::WindowsX8664
        ));
        assert!(!valid_official_wheel(
            win_cpu,
            "cpu",
            "2.14.0",
            "python3.13",
            TorchHostTarget::WindowsX8664
        ));
    }

    #[test]
    fn catalog_candidates_sort_semantically_and_reject_unknown_minor() {
        let candidates = sort_catalog_minors(vec![
            "3.9".into(),
            "3.14".into(),
            "3.10".into(),
            "3.12".into(),
            "3.14".into(),
        ]);
        assert_eq!(candidates, ["3.14", "3.12", "3.10", "3.9"]);
        assert_eq!(
            catalog_python_minor("python3.14", &candidates),
            Some("3.14".into())
        );
        assert_eq!(catalog_python_minor("python3.15", &candidates), None);
        assert_eq!(catalog_python_minor("3.14", &candidates), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pci_reader_identifies_only_display_gpu_vendors() {
        let root = tempfile::tempdir().unwrap();
        let mut files = std::collections::HashMap::new();
        for (slot, class, vendor) in [
            ("0000:01:00.0", "0x030000", "0x10de"),
            ("0000:02:00.0", "0x030200", "0x1002"),
            ("0000:00:02.0", "0x030000", "0x8086"),
            ("0000:00:00.0", "0x060000", "0x1002"),
        ] {
            let path = root.path().join(slot);
            std::fs::create_dir(&path).unwrap();
            files.insert(path.join("class"), class.to_owned());
            files.insert(path.join("vendor"), vendor.to_owned());
        }
        let vendors = pci_display_vendors(root.path(), |path| {
            files
                .get(path)
                .cloned()
                .ok_or(io::Error::from(io::ErrorKind::NotFound))
        })
        .unwrap();
        assert_eq!(vendors, ["amd", "intel", "nvidia"]);
    }

    #[test]
    fn only_stable_three_part_versions_are_accepted() {
        assert_eq!(stable_version("v2.10.0"), Some("2.10.0"));
        assert_eq!(stable_version("2.10.0"), Some("2.10.0"));
        assert_eq!(stable_release_version("v2.10.0"), Some("2.10.0"));
        assert_eq!(stable_release_version("2.10.0"), None);
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

    fn legacy_alternatives() -> TorchAlternativeDiscovery {
        TorchAlternativeDiscovery {
            selected_tag: "v2.10.0".into(),
            selected_build: "cu130".into(),
            selected_python: "python3.12".into(),
            status: "matches".into(),
            incomplete: true,
            dependencies_not_checked: true,
            checked_builds: vec!["cpu".into(), "cu130".into(), "rocm7.2".into()],
            matches: ["cpu", "cu130", "rocm7.2"]
                .into_iter()
                .map(|build| TorchAlternativeMatch {
                    tag: "v2.10.0".into(),
                    build: build.into(),
                    python: "python3.12".into(),
                    wheel_url: None,
                    sha256: None,
                })
                .collect(),
            issues: Vec::new(),
        }
    }

    #[test]
    fn legacy_alternatives_filter_cpu_only_and_nvidia_intel_hosts() {
        let cpu = filter_alternatives_for_host(
            legacy_alternatives(),
            Ok(Vec::new()),
            TorchHostTarget::LinuxX8664,
        );
        assert_eq!(cpu.checked_builds, ["cpu"]);
        assert_eq!(
            cpu.matches
                .iter()
                .map(|item| item.build.as_str())
                .collect::<Vec<_>>(),
            ["cpu"]
        );
        assert_eq!(cpu.status, "matches");

        let nvidia = filter_alternatives_for_host(
            legacy_alternatives(),
            Ok(vec!["intel".into(), "nvidia".into()]),
            TorchHostTarget::LinuxX8664,
        );
        assert_eq!(nvidia.checked_builds, ["cpu", "cu130"]);
        assert_eq!(
            nvidia
                .matches
                .iter()
                .map(|item| item.build.as_str())
                .collect::<Vec<_>>(),
            ["cpu", "cu130"]
        );
        assert!(nvidia.incomplete && nvidia.dependencies_not_checked);
    }

    #[test]
    fn legacy_alternatives_unknown_host_withholds_gpu_and_is_inconclusive() {
        let result = filter_alternatives_for_host(
            legacy_alternatives(),
            Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            TorchHostTarget::LinuxX8664,
        );
        assert_eq!(result.checked_builds, ["cpu"]);
        assert_eq!(
            result
                .matches
                .iter()
                .map(|item| item.build.as_str())
                .collect::<Vec<_>>(),
            ["cpu"]
        );
        assert_eq!(result.status, "inconclusive");
        assert!(result
            .issues
            .iter()
            .any(|issue| issue.contains("GPU devices could not be inspected")));
    }

    #[test]
    fn legacy_alternatives_removed_gpu_only_match_does_not_claim_match() {
        let mut result = legacy_alternatives();
        result.matches.retain(|item| item.build == "cu130");
        let result =
            filter_alternatives_for_host(result, Ok(Vec::new()), TorchHostTarget::LinuxX8664);
        assert!(result.matches.is_empty());
        assert_eq!(result.status, "none");
    }

    #[test]
    fn legacy_native_wheel_leads_require_exact_official_target_wheels() {
        let interpreters = vec![("python3.12".into(), PathBuf::from("/unused/python"))];
        let mut windows = legacy_alternatives();
        for item in &mut windows.matches {
            item.wheel_url = Some(match item.build.as_str() {
                "cpu" => "https://download.pytorch.org/whl/cpu/torch/torch-2.10.0-cp312-cp312-win_amd64.whl",
                "cu130" => "https://download.pytorch.org/whl/cu130/torch/torch-2.10.0%2Bcu130-cp312-cp312-win_amd64.whl",
                _ => "https://download.pytorch.org/whl/rocm7.2/torch/torch-2.10.0%2Brocm7.2-cp312-cp312-manylinux_2_17_x86_64.whl",
            }.into());
        }
        let windows = validate_legacy_wheel_leads(
            windows,
            &interpreters,
            "2.10.0",
            TorchHostTarget::WindowsX8664,
        );
        assert_eq!(windows.status, "inconclusive");
        assert_eq!(windows.matches.len(), 2);
        let windows = filter_alternatives_for_host(
            windows,
            Ok(vec!["nvidia".into()]),
            TorchHostTarget::WindowsX8664,
        );
        assert_eq!(
            windows
                .matches
                .iter()
                .map(|item| item.build.as_str())
                .collect::<Vec<_>>(),
            ["cpu", "cu130"]
        );
        assert_eq!(windows.checked_builds, ["cpu", "cu130"]);

        let mut mac = legacy_alternatives();
        mac.matches[0].wheel_url = Some("https://download.pytorch.org/whl/cpu/torch/torch-2.10.0-cp312-cp312-macosx_11_0_arm64.whl".into());
        mac.matches[1].wheel_url = Some("https://download.pytorch.org/whl/cu130/torch/torch-2.10.0%2Bcu130-cp312-cp312-win_amd64.whl".into());
        let mac =
            validate_legacy_wheel_leads(mac, &interpreters, "2.10.0", TorchHostTarget::MacosArm64);
        let mac = filter_alternatives_for_host(mac, Ok(Vec::new()), TorchHostTarget::MacosArm64);
        assert_eq!(
            mac.matches
                .iter()
                .map(|item| item.build.as_str())
                .collect::<Vec<_>>(),
            ["cpu"]
        );
        assert_eq!(mac.checked_builds, ["cpu"]);
        assert_eq!(mac.status, "inconclusive");
    }
}
