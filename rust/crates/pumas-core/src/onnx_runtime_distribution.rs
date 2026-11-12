//! Upstream ONNX Runtime distributions that Pumas downloads and manages.
//!
//! ONNX Runtime is an external native runtime. The version manager downloads
//! official `microsoft/onnxruntime` release archives; each managed runtime
//! profile runs an ONNX host process that loads the library of the version the
//! profile pins. This module owns the facts both sides share: which releases are
//! compatible with the C API that the `ort` binding uses, which build variants
//! (CPU, CUDA 12, CUDA 13) serve the current platform, which execution provider
//! a profile's device mode selects, and where the library sits inside an
//! installed version.
//!
//! A CUDA build's NVIDIA CUDA runtime and cuDNN libraries are installed with
//! it under [`CUDA_LIBRARY_DIR`]; the NVIDIA driver stays the host system's.
//!
//! Compatibility is derived, not pinned: any stable `1.x` release with
//! `x >= C_API_VERSION` exports the C API Pumas requests, because ONNX Runtime
//! keeps older API versions available in newer releases. A release that was
//! verified end to end is evidence of a working combination, not a requirement.

use crate::models::{GitHubAsset, RuntimeDeviceMode};

/// The ONNX Runtime C API version requested by the `ort` binding.
///
/// ONNX Runtime `1.N` provides C API versions up to `N`, so a release is
/// compatible when its minor version is at least this value.
pub const C_API_VERSION: u32 = 24;

#[cfg(feature = "onnx-runtime")]
const _: () = assert!(
    C_API_VERSION == ort::sys::ORT_API_VERSION,
    "C_API_VERSION must match the ort binding's ONNX Runtime C API version"
);

/// Variant suffix for the CPU build in installed version tags (`v1.24.2+cpu`).
pub const CPU_VARIANT: &str = "cpu";

/// Display label of the CPU build.
pub const CPU_VARIANT_LABEL: &str = "CPU";

/// One upstream build of a release. The variant is the suffix of an installed
/// tag (`v1.31.1+cuda12`) and decides which execution providers it offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnnxRuntimeVariant {
    /// The CPU build. On macOS it also contains the CoreML execution provider.
    Cpu,
    /// The GPU build linked against CUDA 12 (named `gpu` before 1.27).
    Cuda12,
    /// The GPU build linked against CUDA 13.
    Cuda13,
}

impl OnnxRuntimeVariant {
    /// Every variant, in listing order.
    pub const ALL: [Self; 3] = [Self::Cpu, Self::Cuda12, Self::Cuda13];

    /// The tag suffix that names this variant.
    pub fn id(self) -> &'static str {
        match self {
            Self::Cpu => CPU_VARIANT,
            Self::Cuda12 => "cuda12",
            Self::Cuda13 => "cuda13",
        }
    }

    /// The display label of this variant.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => CPU_VARIANT_LABEL,
            Self::Cuda12 => "CUDA 12",
            Self::Cuda13 => "CUDA 13",
        }
    }

    /// The variant named by an installed tag's suffix.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let (_, suffix) = tag.split_once('+')?;
        Self::ALL.into_iter().find(|variant| variant.id() == suffix)
    }

    /// The CUDA major version this build links against, if it is a CUDA build.
    pub fn cuda_major(self) -> Option<u32> {
        match self {
            Self::Cpu => None,
            Self::Cuda12 => Some(12),
            Self::Cuda13 => Some(13),
        }
    }

    /// The GPU execution provider this build offers on the running platform.
    pub fn gpu_execution_provider(self) -> Option<OnnxRuntimeExecutionProvider> {
        match self {
            Self::Cpu if cfg!(target_os = "macos") => Some(OnnxRuntimeExecutionProvider::CoreMl),
            Self::Cpu => None,
            Self::Cuda12 | Self::Cuda13 => Some(OnnxRuntimeExecutionProvider::Cuda),
        }
    }

    /// Upstream archive platform names for this variant, most specific first.
    fn platform_names(self, os: &str, arch: &str) -> &'static [&'static str] {
        match (self, os, arch) {
            (Self::Cpu, "linux", "x86_64") => &["linux-x64"],
            (Self::Cpu, "linux", "aarch64") => &["linux-aarch64"],
            (Self::Cpu, "macos", "aarch64") => &["osx-arm64", "osx-universal2"],
            (Self::Cpu, "macos", "x86_64") => &["osx-x86_64", "osx-universal2"],
            (Self::Cpu, "windows", "x86_64") => &["win-x64"],
            (Self::Cpu, "windows", "aarch64") => &["win-arm64"],
            (Self::Cuda12, "linux", "x86_64") => &["linux-x64-gpu_cuda12", "linux-x64-gpu"],
            (Self::Cuda12, "windows", "x86_64") => &["win-x64-gpu_cuda12", "win-x64-gpu"],
            (Self::Cuda13, "linux", "x86_64") => &["linux-x64-gpu_cuda13"],
            (Self::Cuda13, "windows", "x86_64") => &["win-x64-gpu_cuda13"],
            _ => &[],
        }
    }
}

/// The execution provider an ONNX host registers for its sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnnxRuntimeExecutionProvider {
    Cpu,
    Cuda,
    CoreMl,
}

impl OnnxRuntimeExecutionProvider {
    /// The stable command-line and status spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::CoreMl => "coreml",
        }
    }

    /// Parse the command-line spelling.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cpu" => Some(Self::Cpu),
            "cuda" => Some(Self::Cuda),
            "coreml" => Some(Self::CoreMl),
            _ => None,
        }
    }
}

/// The execution provider a profile's device mode selects for an installed
/// version, or the reason it cannot run that way.
///
/// `Auto` uses the build's GPU provider when it has one, otherwise the CPU. An
/// explicit GPU request on a build without a GPU provider is refused rather
/// than silently running on the CPU.
pub fn execution_provider_for(
    tag: &str,
    device: RuntimeDeviceMode,
) -> Result<OnnxRuntimeExecutionProvider, String> {
    let variant = OnnxRuntimeVariant::from_tag(tag).ok_or_else(|| {
        format!(
            "ONNX Runtime version {tag} does not name a build variant; reinstall it from the \
             ONNX Runtime versions"
        )
    })?;
    match device {
        RuntimeDeviceMode::Cpu => Ok(OnnxRuntimeExecutionProvider::Cpu),
        RuntimeDeviceMode::Auto => Ok(variant
            .gpu_execution_provider()
            .unwrap_or(OnnxRuntimeExecutionProvider::Cpu)),
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::SpecificDevice => {
            variant.gpu_execution_provider().ok_or_else(|| {
                format!(
                    "ONNX Runtime {tag} is a {} build without a GPU execution provider on this \
                     platform; install a CUDA variant from the ONNX Runtime versions or choose \
                     the CPU device",
                    variant.label()
                )
            })
        }
        RuntimeDeviceMode::Hybrid => Err(
            "ONNX Runtime runs a model on one device; choose CPU, GPU or Auto for this profile"
                .to_string(),
        ),
    }
}

/// The library file, relative to an installed version directory, that the
/// ONNX host loads. On Linux and macOS upstream ships this name as a link to
/// the versioned library file; installation makes it the regular file.
pub const fn library_relative_path() -> &'static str {
    if cfg!(target_os = "windows") {
        "lib/onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "lib/libonnxruntime.dylib"
    } else {
        "lib/libonnxruntime.so"
    }
}

/// The directory, relative to an installed CUDA version, that holds the
/// NVIDIA CUDA runtime and cuDNN shared libraries Pumas installs with it. The
/// ONNX host's library search path points here.
pub const CUDA_LIBRARY_DIR: &str = "cuda/lib";

/// The record, relative to an installed CUDA version, of the NVIDIA packages
/// and library digests that filled [`CUDA_LIBRARY_DIR`].
pub const CUDA_LIBRARIES_RECORD: &str = "cuda/libraries.json";

/// `relative` (one of the `/`-separated paths above) under `root`, joined
/// component by component so it uses the platform's separator.
pub fn join_relative(root: &std::path::Path, relative: &str) -> std::path::PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

/// The CUDA library directory Pumas installed with the version at
/// `version_dir`, if it has one.
pub fn installed_cuda_library_dir(version_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let directory = join_relative(version_dir, CUDA_LIBRARY_DIR);
    let installed = std::fs::symlink_metadata(&directory)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        && join_relative(version_dir, CUDA_LIBRARIES_RECORD).is_file();
    installed.then_some(directory)
}

/// The environment variable the platform's dynamic loader searches for the
/// libraries a loaded library depends on, and the separator of its entries.
pub const fn library_search_path_variable() -> Option<(&'static str, char)> {
    if cfg!(target_os = "windows") {
        Some(("PATH", ';'))
    } else if cfg!(target_os = "linux") {
        Some(("LD_LIBRARY_PATH", ':'))
    } else {
        None
    }
}

/// Parse the upstream release version from a release or installed tag.
///
/// Accepts `v1.24.2`, `1.24.2` and the installed form `v1.24.2+cpu`.
pub fn release_version(tag: &str) -> Option<semver::Version> {
    let publisher_tag = tag.split_once('+').map_or(tag, |(publisher, _)| publisher);
    semver::Version::parse(publisher_tag.strip_prefix('v').unwrap_or(publisher_tag)).ok()
}

/// Whether an upstream release exports the C API Pumas requests.
pub fn is_compatible_version(version: &semver::Version) -> bool {
    version.major == 1 && version.minor >= u64::from(C_API_VERSION) && version.pre.is_empty()
}

/// Why a release tag cannot be installed, or `None` when it is compatible.
pub fn incompatibility_reason(tag: &str) -> Option<String> {
    match release_version(tag) {
        None => Some(format!(
            "ONNX Runtime release {tag} does not have a semantic version"
        )),
        Some(version) if !is_compatible_version(&version) => Some(format!(
            "ONNX Runtime {version} does not provide C API {C_API_VERSION}; \
             install ONNX Runtime 1.{C_API_VERSION} or newer"
        )),
        Some(_) => None,
    }
}

/// Upstream archive names for a variant on an OS/architecture, most specific
/// first. `os` and `arch` use `std::env::consts` spelling.
pub fn variant_archive_names(
    version: &semver::Version,
    variant: OnnxRuntimeVariant,
    os: &str,
    arch: &str,
) -> Vec<String> {
    let extension = if os == "windows" { "zip" } else { "tgz" };
    variant
        .platform_names(os, arch)
        .iter()
        .map(|platform| format!("onnxruntime-{platform}-{version}.{extension}"))
        .collect()
}

/// Upstream CPU archive names for an OS/architecture, most specific first.
pub fn platform_archive_names(version: &semver::Version, os: &str, arch: &str) -> Vec<String> {
    variant_archive_names(version, OnnxRuntimeVariant::Cpu, os, arch)
}

/// Select a variant's archive for a platform from a release's assets.
pub fn select_variant_archive<'a>(
    assets: &'a [GitHubAsset],
    version: &semver::Version,
    variant: OnnxRuntimeVariant,
    os: &str,
    arch: &str,
) -> Option<&'a GitHubAsset> {
    variant_archive_names(version, variant, os, arch)
        .iter()
        .find_map(|name| assets.iter().find(|asset| &asset.name == name))
}

/// Select the CPU archive for a platform from a release's assets.
pub fn select_platform_archive<'a>(
    assets: &'a [GitHubAsset],
    version: &semver::Version,
    os: &str,
    arch: &str,
) -> Option<&'a GitHubAsset> {
    select_variant_archive(assets, version, OnnxRuntimeVariant::Cpu, os, arch)
}

/// Select the CPU archive for the running platform.
pub fn select_host_archive<'a>(
    assets: &'a [GitHubAsset],
    version: &semver::Version,
) -> Option<&'a GitHubAsset> {
    select_platform_archive(
        assets,
        version,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> GitHubAsset {
        GitHubAsset {
            name: name.to_string(),
            size: 1,
            download_url: format!("https://example.invalid/{name}"),
            content_type: None,
        }
    }

    #[test]
    fn compatibility_follows_the_c_api_not_a_pinned_release() {
        for (tag, compatible) in [
            ("v1.24.2", true),
            ("v1.24.0", true),
            ("v1.25.1", true),
            ("v1.31.0+cpu", true),
            ("v1.23.2", false),
            ("v2.0.0", false),
            ("v1.25.0-rc1", false),
            ("nightly", false),
        ] {
            assert_eq!(incompatibility_reason(tag).is_none(), compatible, "{tag}");
        }
        assert!(incompatibility_reason("v1.23.2")
            .unwrap()
            .contains("1.24 or newer"));
    }

    #[test]
    fn platform_archive_selection_uses_exact_cpu_archive_names() {
        let version = semver::Version::parse("1.24.2").unwrap();
        let assets = [
            asset("onnxruntime-linux-x64-gpu-1.24.2.tgz"),
            asset("onnxruntime-linux-x64-1.24.2.tgz"),
            asset("onnxruntime-linux-aarch64-1.24.2.tgz"),
            asset("onnxruntime-osx-universal2-1.24.2.tgz"),
            asset("onnxruntime-osx-arm64-1.24.2.tgz"),
            asset("onnxruntime-win-x64-gpu-1.24.2.zip"),
            asset("onnxruntime-win-x64-1.24.2.zip"),
            asset("Microsoft.ML.OnnxRuntime.1.24.2.nupkg"),
        ];
        let select = |os, arch| {
            select_platform_archive(&assets, &version, os, arch).map(|asset| asset.name.as_str())
        };
        assert_eq!(
            select("linux", "x86_64"),
            Some("onnxruntime-linux-x64-1.24.2.tgz")
        );
        assert_eq!(
            select("linux", "aarch64"),
            Some("onnxruntime-linux-aarch64-1.24.2.tgz")
        );
        assert_eq!(
            select("macos", "aarch64"),
            Some("onnxruntime-osx-arm64-1.24.2.tgz")
        );
        assert_eq!(
            select("macos", "x86_64"),
            Some("onnxruntime-osx-universal2-1.24.2.tgz")
        );
        assert_eq!(
            select("windows", "x86_64"),
            Some("onnxruntime-win-x64-1.24.2.zip")
        );
        assert_eq!(select("windows", "aarch64"), None);
        assert_eq!(select("freebsd", "x86_64"), None);
    }

    #[test]
    fn gpu_variants_follow_upstream_cuda_archive_names_across_the_rename() {
        let select = |version: &str, names: &[&str], variant, os| {
            let version = semver::Version::parse(version).unwrap();
            let assets: Vec<_> = names.iter().map(|name| asset(name)).collect();
            select_variant_archive(&assets, &version, variant, os, "x86_64")
                .map(|asset| asset.name.clone())
        };
        // 1.26 and earlier name the CUDA 12 build `gpu`.
        assert_eq!(
            select(
                "1.26.0",
                &[
                    "onnxruntime-linux-x64-gpu-1.26.0.tgz",
                    "onnxruntime-linux-x64-gpu_cuda13-1.26.0.tgz"
                ],
                OnnxRuntimeVariant::Cuda12,
                "linux"
            )
            .as_deref(),
            Some("onnxruntime-linux-x64-gpu-1.26.0.tgz")
        );
        // 1.27 and later name it `gpu_cuda12`.
        assert_eq!(
            select(
                "1.31.1",
                &[
                    "onnxruntime-win-x64-gpu_cuda12-1.31.1.zip",
                    "onnxruntime-win-x64-gpu_cuda13-1.31.1.zip"
                ],
                OnnxRuntimeVariant::Cuda13,
                "windows"
            )
            .as_deref(),
            Some("onnxruntime-win-x64-gpu_cuda13-1.31.1.zip")
        );
        // macOS has no CUDA build; its CPU build carries CoreML instead.
        assert_eq!(
            select(
                "1.31.1",
                &["onnxruntime-osx-arm64-1.31.1.tgz"],
                OnnxRuntimeVariant::Cuda12,
                "macos"
            ),
            None
        );
    }

    #[test]
    fn device_mode_selects_the_variant_execution_provider_or_refuses_with_a_reason() {
        use OnnxRuntimeExecutionProvider as Ep;
        assert_eq!(
            execution_provider_for("v1.31.1+cuda12", RuntimeDeviceMode::Gpu),
            Ok(Ep::Cuda)
        );
        assert_eq!(
            execution_provider_for("v1.31.1+cuda13", RuntimeDeviceMode::Auto),
            Ok(Ep::Cuda)
        );
        assert_eq!(
            execution_provider_for("v1.31.1+cuda12", RuntimeDeviceMode::Cpu),
            Ok(Ep::Cpu)
        );
        let cpu_gpu = execution_provider_for("v1.31.1+cpu", RuntimeDeviceMode::Gpu);
        let cpu_auto = execution_provider_for("v1.31.1+cpu", RuntimeDeviceMode::Auto);
        if cfg!(target_os = "macos") {
            assert_eq!(cpu_gpu, Ok(Ep::CoreMl));
            assert_eq!(cpu_auto, Ok(Ep::CoreMl));
        } else {
            assert!(cpu_gpu.unwrap_err().contains("install a CUDA variant"));
            assert_eq!(cpu_auto, Ok(Ep::Cpu));
        }
        assert!(execution_provider_for("v1.31.1", RuntimeDeviceMode::Auto)
            .unwrap_err()
            .contains("build variant"));
        assert!(execution_provider_for("v1.31.1+cpu", RuntimeDeviceMode::Hybrid).is_err());
        for ep in [Ep::Cpu, Ep::Cuda, Ep::CoreMl] {
            assert_eq!(Ep::parse(ep.as_str()), Some(ep));
        }
    }
}
