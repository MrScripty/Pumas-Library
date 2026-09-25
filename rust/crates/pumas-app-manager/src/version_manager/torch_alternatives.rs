//! Bounded discovery of official Torch wheels for installed Python interpreters.
//! A wheel match is only a lead; dependencies and adapters are not resolved.

use super::torch_preview::{valid_torch_channel, BUILDS, PYTHONS};
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io;
use tokio::process::Command;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(45);
const PYTHON_CHECK_TIMEOUT: Duration = Duration::from_secs(3);
// Leave room for interpreter checks, the bounded index scan, and process startup.
const RELEASE_OPTIONS_TIMEOUT: Duration = Duration::from_secs(60);

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolverReleaseOptions {
    tag: String,
    status: TorchReleaseOptionsStatus,
    complete_scan: bool,
    checked_channels: Vec<String>,
    combinations: Vec<TorchReleaseCombination>,
    issues: Vec<String>,
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
    version: Option<NvidiaDriverVersion>,
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

// Linux driver floors for CUDA minor compatibility. CUDA 13.x/12.x/11.x
// and exact 10.x floors: https://docs.nvidia.com/deploy/cuda-compatibility/minor-version-compatibility.html
// A passing floor is only a recommendation gate; feature/PTX and device/model
// behavior still require checks after installation.
fn cuda_linux_driver_floor(build: &str) -> Option<NvidiaDriverVersion> {
    let channel = build.strip_prefix("cu")?.parse::<u32>().ok()?;
    match channel {
        130..=139 => Some(NvidiaDriverVersion(580, 0, 0)),
        120..=129 => Some(NvidiaDriverVersion(525, 60, 13)),
        110..=119 => Some(NvidiaDriverVersion(450, 80, 2)),
        102 => Some(NvidiaDriverVersion(440, 33, 0)),
        101 => Some(NvidiaDriverVersion(418, 39, 0)),
        100 => Some(NvidiaDriverVersion(410, 48, 0)),
        _ => None,
    }
}

async fn nvidia_driver_probe() -> NvidiaDriverProbe {
    let mut command = Command::new("nvidia-smi");
    command
        .kill_on_drop(true)
        .args(["--query-gpu=driver_version", "--format=csv,noheader"]);
    match tokio::time::timeout(PYTHON_CHECK_TIMEOUT, command.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            let version = parse_nvidia_driver_output(&output.stdout);
            NvidiaDriverProbe {
                availability: if version.is_some() {
                    TorchReleaseDriverAvailability::Available
                } else {
                    TorchReleaseDriverAvailability::Unknown
                },
                version,
            }
        }
        Ok(Ok(_)) => NvidiaDriverProbe {
            availability: TorchReleaseDriverAvailability::Unavailable,
            version: None,
        },
        Ok(Err(_)) | Err(_) => NvidiaDriverProbe {
            availability: TorchReleaseDriverAvailability::Unknown,
            version: None,
        },
    }
}

fn amd_driver_available(devices_root: &Path) -> TorchReleaseDriverAvailability {
    let Ok(entries) = std::fs::read_dir(devices_root) else {
        return TorchReleaseDriverAvailability::Unknown;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return TorchReleaseDriverAvailability::Unknown;
        };
        let path = entry.path();
        let class = std::fs::read_to_string(path.join("class"))
            .ok()
            .and_then(|text| u32::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok());
        let vendor = std::fs::read_to_string(path.join("vendor"))
            .ok()
            .and_then(|text| u16::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok());
        let (Some(class), Some(vendor)) = (class, vendor) else {
            return TorchReleaseDriverAvailability::Unknown;
        };
        if class >> 16 != 0x03 || vendor != 0x1002 {
            continue;
        }
        match std::fs::read_link(path.join("driver")) {
            Ok(driver) if driver.file_name().is_some_and(|name| name == "amdgpu") => {
                return TorchReleaseDriverAvailability::Available;
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return TorchReleaseDriverAvailability::Unknown,
        }
    }
    TorchReleaseDriverAvailability::Unavailable
}

async fn installed_python_paths() -> Vec<(String, PathBuf)> {
    let mut interpreters = Vec::new();
    for python in PYTHONS {
        let mut command = Command::new(python);
        command.kill_on_drop(true).args([
            "-I", "-c",
            "import platform,sys; print(sys.executable if sys.platform == 'linux' and platform.machine() == 'x86_64' and sys.implementation.name == 'cpython' and f'{sys.version_info.major}.{sys.version_info.minor}' == sys.argv[1] else '')",
            python.strip_prefix("python").unwrap_or(""),
        ]);
        if let Ok(Ok(output)) = tokio::time::timeout(PYTHON_CHECK_TIMEOUT, command.output()).await {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                if let Ok(path) = std::fs::canonicalize(path.trim()) {
                    if path.is_absolute() {
                        interpreters.push(((*python).to_owned(), path));
                    }
                }
            }
        }
    }
    interpreters
}

fn valid_official_wheel(url: &str, build: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("download.pytorch.org" | "download-r2.pytorch.org")
        )
        && matches!(url.port(), None | Some(443))
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path().starts_with(&format!("/whl/{build}/"))
        && url
            .path()
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("torch-") && name.ends_with(".whl"))
}

fn python_rank(python: &str) -> u8 {
    match python {
        "python3.12" => 4,
        "python3.13" => 3,
        "python3.11" => 2,
        "python3.10" => 1,
        _ => 0,
    }
}

fn visible_combinations(
    combinations: Vec<TorchReleaseCombination>,
    vendors: &[String],
) -> Vec<TorchReleaseCombination> {
    combinations
        .into_iter()
        .filter(|item| visible_build(&item.build, vendors))
        .collect()
}

fn visible_build(build: &str, vendors: &[String]) -> bool {
    build == "cpu"
        || (build.starts_with("cu") && vendors.iter().any(|vendor| vendor == "nvidia"))
        || (build.starts_with("rocm") && vendors.iter().any(|vendor| vendor == "amd"))
}

fn best_combination<'a>(
    combinations: &'a [TorchReleaseCombination],
    family: &str,
) -> Option<&'a TorchReleaseCombination> {
    combinations
        .iter()
        .filter(|item| match family {
            "cpu" => item.build == "cpu",
            "cu" => item.build.starts_with("cu"),
            "rocm" => item.build.starts_with("rocm"),
            _ => false,
        })
        .max_by_key(|item| {
            let versions: Vec<u64> = item
                .build
                .strip_prefix(family)
                .unwrap_or("")
                .split('.')
                .filter_map(|part| part.parse().ok())
                .collect();
            (versions, python_rank(&item.python))
        })
}

fn recommend_release_option(
    combinations: &[TorchReleaseCombination],
    vendors: &[String],
    drivers: &TorchReleaseDriverStatus,
    nvidia_version: Option<NvidiaDriverVersion>,
    complete_scan: bool,
) -> (Option<TorchReleaseRecommendation>, String) {
    let nvidia = vendors.iter().any(|vendor| vendor == "nvidia");
    let amd = vendors.iter().any(|vendor| vendor == "amd");
    if !complete_scan {
        if let Some(cpu) = best_combination(combinations, "cpu") {
            return (
                Some(TorchReleaseRecommendation {
                    build: cpu.build.clone(),
                    python: cpu.python.clone(),
                }),
                "The official wheel scan was incomplete; CPU is a provisional recommendation. Exact GPU wheels remain advanced choices."
                    .to_owned(),
            );
        }
        return (
            None,
            "The official wheel scan was incomplete, and no exact CPU wheel was found; review GPU wheels as advanced choices."
                .to_owned(),
        );
    }
    if nvidia && drivers.nvidia == TorchReleaseDriverAvailability::Available {
        if let Some(version) = nvidia_version {
            let selected = combinations
                .iter()
                .filter(|item| {
                    cuda_linux_driver_floor(&item.build).is_some_and(|floor| version >= floor)
                })
                .max_by_key(|item| {
                    (
                        item.build
                            .strip_prefix("cu")
                            .and_then(|channel| channel.parse::<u32>().ok())
                            .unwrap_or(0),
                        python_rank(&item.python),
                    )
                });
            if let Some(selected) = selected {
                return (
                    Some(TorchReleaseRecommendation {
                        build: selected.build.clone(),
                        python: selected.python.clone(),
                    }),
                    "This is the highest exact CUDA wheel meeting NVIDIA's Linux minor-compatibility driver floor. Minor compatibility can limit features and PTX; device and model behavior are tested after installation."
                        .to_owned(),
                );
            }
        }
    }
    if let Some(cpu) = best_combination(combinations, "cpu") {
        let note = if nvidia && drivers.nvidia != TorchReleaseDriverAvailability::Available {
            "NVIDIA hardware was detected, but its driver could not be verified. Exact CUDA wheels remain advanced choices; CPU is recommended."
        } else if nvidia && best_combination(combinations, "cu").is_some() {
            "No exact CUDA wheel has a verified NVIDIA Linux minor-compatibility driver floor on this system. CUDA wheels remain advanced choices; CPU is recommended."
        } else if amd {
            "An AMD GPU was detected, but per-release ROCm device compatibility is not established. ROCm wheels remain advanced choices; CPU is recommended."
        } else if nvidia || amd {
            "No exact wheel for the available GPU driver was found for this release; CPU is recommended."
        } else {
            ""
        };
        return (
            Some(TorchReleaseRecommendation {
                build: cpu.build.clone(),
                python: cpu.python.clone(),
            }),
            note.to_owned(),
        );
    }
    let note = if combinations.is_empty() {
        "No exact official wheel was found for the detected devices and installed Python interpreters."
    } else if nvidia && best_combination(combinations, "cu").is_some() {
        "No exact CUDA wheel has a verified NVIDIA Linux minor-compatibility driver floor on this system, and no CPU wheel matched. CUDA wheels remain advanced choices."
    } else if amd {
        "Per-release ROCm device compatibility is not established, and no CPU wheel matched. ROCm wheels remain advanced choices."
    } else {
        "Exact GPU wheels were found, but a corresponding driver could not be verified; review them as advanced choices."
    };
    (None, note.to_owned())
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
) -> TorchAlternativeDiscovery {
    let unknown_host = detected.is_err();
    let vendors = detected.unwrap_or_default();
    result
        .checked_builds
        .retain(|build| visible_build(build, &vendors));
    result
        .matches
        .retain(|item| visible_build(&item.build, &vendors));
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

fn stable_release_version(tag: &str) -> Option<&str> {
    stable_version(tag.strip_prefix('v')?)
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
        Ok(filter_alternatives_for_host(
            result,
            pci_display_vendors(Path::new("/sys/bus/pci/devices"), |path| {
                std::fs::read_to_string(path)
            }),
        ))
    }
}

impl VersionManager {
    /// Discover this release's exact official Torch wheels for installed Python.
    /// A wheel match is a choice to preview, not a resolved dependency plan.
    pub async fn discover_torch_release_options(
        &self,
        tag: &str,
    ) -> Result<TorchReleaseOptionsDiscovery> {
        if self.app_id != AppId::Torch {
            return Err(unsupported_input(
                "Torch release options require a Torch manager",
            ));
        }
        let version = stable_release_version(tag)
            .ok_or_else(|| unsupported_input("Select a stable upstream Torch tag"))?;
        self.resolve_installable_release(tag).await?;

        let detected = pci_display_vendors(Path::new("/sys/bus/pci/devices"), |path| {
            std::fs::read_to_string(path)
        });
        let (vendors, mut host_issues) = match detected {
            Ok(vendors) => (vendors, Vec::new()),
            Err(_) => (
                Vec::new(),
                vec!["GPU devices could not be inspected".to_owned()],
            ),
        };
        let unknown_host = !host_issues.is_empty();
        let nvidia_probe = if unknown_host {
            NvidiaDriverProbe {
                availability: TorchReleaseDriverAvailability::Unknown,
                version: None,
            }
        } else if vendors.iter().any(|vendor| vendor == "nvidia") {
            nvidia_driver_probe().await
        } else {
            NvidiaDriverProbe {
                availability: TorchReleaseDriverAvailability::NotPresent,
                version: None,
            }
        };
        let driver_status = TorchReleaseDriverStatus {
            nvidia: nvidia_probe.availability,
            amd: if unknown_host {
                TorchReleaseDriverAvailability::Unknown
            } else if vendors.iter().any(|vendor| vendor == "amd") {
                amd_driver_available(Path::new("/sys/bus/pci/devices"))
            } else {
                TorchReleaseDriverAvailability::NotPresent
            },
        };
        let interpreters = installed_python_paths().await;
        let empty = |issue: &str| ResolverReleaseOptions {
            tag: tag.to_owned(),
            status: TorchReleaseOptionsStatus::Inconclusive,
            complete_scan: false,
            checked_channels: Vec::new(),
            combinations: Vec::new(),
            issues: vec![issue.to_owned()],
        };
        let mut result = if interpreters.is_empty() {
            empty("No installed CPython 3.10–3.13 interpreter on Linux x86_64 could inspect official wheels")
        } else {
            let workspace = tempfile::tempdir().map_err(PumasError::from)?;
            let resolver = workspace.path().join("resolve_runtime.py");
            std::fs::write(
                &resolver,
                include_str!("../../../../../torch-server/resolve_runtime.py"),
            )
            .map_err(PumasError::from)?;
            let mut command = Command::new(&interpreters[0].1);
            command.kill_on_drop(true).arg("-I").arg(&resolver).args([
                "--release-options",
                "--version",
                version,
            ]);
            for (_, path) in &interpreters {
                command.arg("--interpreter").arg(path);
            }
            match tokio::time::timeout(RELEASE_OPTIONS_TIMEOUT, command.output()).await {
                Ok(Ok(output)) if output.status.success() => {
                    serde_json::from_slice::<ResolverReleaseOptions>(&output.stdout)
                        .map_err(|_| unsupported_input("Invalid Torch release-options result"))?
                }
                Ok(Ok(_)) => empty("Official Torch wheel scan did not complete conclusively"),
                Ok(Err(_)) => empty("Official Torch wheel scan could not start"),
                Err(_) => empty("Official Torch wheel scan timed out"),
            }
        };
        let available_pythons: BTreeSet<_> =
            interpreters.iter().map(|(name, _)| name.as_str()).collect();
        let checked: BTreeSet<_> = result.checked_channels.iter().map(String::as_str).collect();
        if result.tag != tag
            || result.checked_channels.len() > 64
            || result.combinations.len() > 1024
            || checked.len() != result.checked_channels.len()
            || result
                .checked_channels
                .iter()
                .any(|channel| !valid_torch_channel(channel))
            || result
                .issues
                .iter()
                .any(|issue| issue.len() > 300 || issue.chars().any(char::is_control))
            || result.combinations.iter().any(|item| {
                !checked.contains(item.build.as_str())
                    || !available_pythons.contains(item.python.as_str())
                    || !valid_official_wheel(&item.wheel_url, &item.build)
                    || item.sha256.as_ref().is_some_and(|hash| {
                        hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
            })
            || match result.status {
                TorchReleaseOptionsStatus::Matches => {
                    !result.complete_scan || result.combinations.is_empty()
                }
                TorchReleaseOptionsStatus::None => {
                    !result.complete_scan || !result.combinations.is_empty()
                }
                TorchReleaseOptionsStatus::Inconclusive => result.complete_scan,
            }
        {
            return Err(unsupported_input(
                "Torch release-options result violated its exact-wheel contract",
            ));
        }
        result.combinations = visible_combinations(result.combinations, &vendors);
        let status = if !result.complete_scan || (unknown_host && result.combinations.is_empty()) {
            TorchReleaseOptionsStatus::Inconclusive
        } else if result.combinations.is_empty() {
            TorchReleaseOptionsStatus::None
        } else {
            TorchReleaseOptionsStatus::Matches
        };
        let (recommended, mut recommendation_note) = recommend_release_option(
            &result.combinations,
            &vendors,
            &driver_status,
            nvidia_probe.version,
            result.complete_scan,
        );
        if unknown_host {
            recommendation_note = if recommended.is_some() {
                "GPU devices could not be inspected; CPU is the only recommended choice until hardware is known."
            } else {
                "GPU devices could not be inspected, and no CPU wheel matched; no default can be recommended."
            }
            .to_owned();
        }
        host_issues.append(&mut result.issues);
        Ok(TorchReleaseOptionsDiscovery {
            tag: result.tag,
            status,
            complete_scan: result.complete_scan,
            checked_channels: result.checked_channels,
            combinations: result.combinations,
            issues: host_issues,
            detected_gpu_vendors: vendors,
            driver_status,
            recommended,
            recommendation_note,
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
            "cu136"
        ));
        assert!(value["combinations"][0].get("sha256").is_none());
        assert_eq!(value["driverStatus"]["nvidia"], "available");
        assert_eq!(value["driverStatus"]["amd"], "not_present");
        assert!(valid_official_wheel(
            "https://download.pytorch.org/whl/cu136/torch-2.14.0%2Bcu136.whl",
            "cu136"
        ));
        assert!(!valid_official_wheel(
            "https://example.com/whl/cu136/torch-2.14.0%2Bcu136.whl",
            "cu136"
        ));
    }

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
    fn amd_driver_status_requires_binding_to_display_device() {
        let root = tempfile::tempdir().unwrap();
        let device = root.path().join("0000:02:00.0");
        std::fs::create_dir(&device).unwrap();
        std::fs::write(device.join("class"), "0x030000").unwrap();
        std::fs::write(device.join("vendor"), "0x1002").unwrap();
        assert_eq!(
            amd_driver_available(root.path()),
            TorchReleaseDriverAvailability::Unavailable
        );
        std::os::unix::fs::symlink(root.path().join("amdgpu"), device.join("driver")).unwrap();
        assert_eq!(
            amd_driver_available(root.path()),
            TorchReleaseDriverAvailability::Available
        );
    }

    #[test]
    fn nvidia_and_intel_show_cuda_cpu_and_recommend_highest_exact_cuda() {
        let vendors = vec!["intel".into(), "nvidia".into()];
        let combinations = visible_combinations(
            vec![
                wheel("cpu", "python3.12"),
                wheel("cu130", "python3.13"),
                wheel("cu136", "python3.12"),
                wheel("rocm7.14", "python3.12"),
            ],
            &vendors,
        );
        assert_eq!(
            combinations
                .iter()
                .map(|item| item.build.as_str())
                .collect::<Vec<_>>(),
            ["cpu", "cu130", "cu136"]
        );
        let drivers = TorchReleaseDriverStatus {
            nvidia: TorchReleaseDriverAvailability::Available,
            amd: TorchReleaseDriverAvailability::NotPresent,
        };
        let (selected, note) = recommend_release_option(
            &combinations,
            &vendors,
            &drivers,
            Some(NvidiaDriverVersion(580, 65, 6)),
            true,
        );
        let selected = selected.unwrap();
        assert_eq!(
            (selected.build.as_str(), selected.python.as_str()),
            ("cu136", "python3.12")
        );
        assert!(note.contains("NVIDIA's Linux minor-compatibility driver floor"));
        assert!(note.contains("device and model behavior are tested after installation"));
    }

    #[test]
    fn missing_gpu_driver_keeps_exact_advanced_wheels_but_recommends_cpu() {
        let vendors = vec!["nvidia".into()];
        let combinations = visible_combinations(
            vec![wheel("cu136", "python3.12"), wheel("cpu", "python3.13")],
            &vendors,
        );
        let drivers = TorchReleaseDriverStatus {
            nvidia: TorchReleaseDriverAvailability::Unavailable,
            amd: TorchReleaseDriverAvailability::NotPresent,
        };
        let (selected, note) =
            recommend_release_option(&combinations, &vendors, &drivers, None, true);
        assert_eq!(combinations.len(), 2);
        assert_eq!(selected.unwrap().build, "cpu");
        assert!(note.contains("CUDA wheels remain advanced choices"));
    }

    #[test]
    fn amd_driver_keeps_rocm_advanced_and_cpu_only_host_sees_cpu() {
        let vendors = vec!["amd".into()];
        let combinations = visible_combinations(
            vec![
                wheel("rocm7.2", "python3.12"),
                wheel("rocm7.14", "python3.13"),
                wheel("cpu", "python3.12"),
                wheel("cu136", "python3.12"),
            ],
            &vendors,
        );
        let drivers = TorchReleaseDriverStatus {
            nvidia: TorchReleaseDriverAvailability::NotPresent,
            amd: TorchReleaseDriverAvailability::Available,
        };
        let (selected, note) =
            recommend_release_option(&combinations, &vendors, &drivers, None, true);
        assert_eq!(selected.unwrap().build, "cpu");
        assert!(note.contains("ROCm wheels remain advanced choices"));
        let cpu = visible_combinations(combinations, &[]);
        assert_eq!(cpu.len(), 1);
        assert_eq!(
            recommend_release_option(&cpu, &[], &drivers, None, true)
                .0
                .unwrap()
                .build,
            "cpu"
        );
    }

    #[test]
    fn cuda_minor_compatibility_floors_are_numeric_and_unknown_families_stay_advanced() {
        assert_eq!(
            parse_nvidia_driver_output(b"590.10.00\n580.65.06\n"),
            Some(NvidiaDriverVersion(580, 65, 6))
        );
        assert_eq!(parse_nvidia_driver_output(b"not a version\n"), None);
        assert_eq!(
            cuda_linux_driver_floor("cu132"),
            Some(NvidiaDriverVersion(580, 0, 0))
        );
        assert_eq!(
            cuda_linux_driver_floor("cu121"),
            Some(NvidiaDriverVersion(525, 60, 13))
        );
        assert_eq!(
            cuda_linux_driver_floor("cu118"),
            Some(NvidiaDriverVersion(450, 80, 2))
        );
        assert_eq!(
            cuda_linux_driver_floor("cu102"),
            Some(NvidiaDriverVersion(440, 33, 0))
        );
        assert_eq!(
            cuda_linux_driver_floor("cu101"),
            Some(NvidiaDriverVersion(418, 39, 0))
        );
        assert_eq!(
            cuda_linux_driver_floor("cu100"),
            Some(NvidiaDriverVersion(410, 48, 0))
        );
        assert_eq!(cuda_linux_driver_floor("cu90"), None);
        assert_eq!(cuda_linux_driver_floor("cu140"), None);
        assert!(NvidiaDriverVersion(525, 60, 12) < cuda_linux_driver_floor("cu121").unwrap());
        assert!(NvidiaDriverVersion(525, 60, 13) >= cuda_linux_driver_floor("cu121").unwrap());
    }

    #[test]
    fn v214_cu132_requires_nvidia_580_floor_and_unknown_channel_is_not_default() {
        let vendors = vec!["nvidia".into()];
        let drivers = TorchReleaseDriverStatus {
            nvidia: TorchReleaseDriverAvailability::Available,
            amd: TorchReleaseDriverAvailability::NotPresent,
        };
        let combinations = vec![wheel("cpu", "python3.12"), wheel("cu132", "python3.12")];
        let (too_old, old_note) = recommend_release_option(
            &combinations,
            &vendors,
            &drivers,
            Some(NvidiaDriverVersion(579, 99, 99)),
            true,
        );
        assert_eq!(too_old.unwrap().build, "cpu");
        assert!(old_note.contains("driver floor"));
        let (sufficient, note) = recommend_release_option(
            &combinations,
            &vendors,
            &drivers,
            Some(NvidiaDriverVersion(580, 0, 0)),
            true,
        );
        assert_eq!(sufficient.unwrap().build, "cu132");
        assert!(note.contains("minor-compatibility"));
        let (unknown, note) = recommend_release_option(
            &[wheel("cpu", "python3.12"), wheel("cu140", "python3.12")],
            &vendors,
            &drivers,
            Some(NvidiaDriverVersion(700, 0, 0)),
            true,
        );
        assert_eq!(unknown.unwrap().build, "cpu");
        assert!(note.contains("driver floor"));
    }

    #[test]
    fn incomplete_scan_never_recommends_gpu() {
        let vendors = vec!["nvidia".into()];
        let drivers = TorchReleaseDriverStatus {
            nvidia: TorchReleaseDriverAvailability::Available,
            amd: TorchReleaseDriverAvailability::NotPresent,
        };
        let combinations = vec![wheel("cpu", "python3.12"), wheel("cu132", "python3.12")];
        let (selected, note) = recommend_release_option(
            &combinations,
            &vendors,
            &drivers,
            Some(NvidiaDriverVersion(600, 0, 0)),
            false,
        );
        assert_eq!(selected.unwrap().build, "cpu");
        assert!(note.contains("provisional"));
        let (selected, note) = recommend_release_option(
            &[wheel("cu132", "python3.12")],
            &vendors,
            &drivers,
            Some(NvidiaDriverVersion(600, 0, 0)),
            false,
        );
        assert!(selected.is_none());
        assert!(note.contains("incomplete"));
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
        let cpu = filter_alternatives_for_host(legacy_alternatives(), Ok(Vec::new()));
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
        let result = filter_alternatives_for_host(result, Ok(Vec::new()));
        assert!(result.matches.is_empty());
        assert_eq!(result.status, "none");
    }
}
