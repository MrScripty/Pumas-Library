//! Upstream ONNX Runtime distributions that Pumas downloads and manages.
//!
//! ONNX Runtime is an external native runtime. The version manager downloads
//! official `microsoft/onnxruntime` release archives, and the in-process ONNX
//! provider loads the library from the selected installed version. This module
//! owns the facts both sides share: which releases are compatible with the C API
//! that the `ort` binding uses, which archive serves the current platform, and
//! where the library sits inside an installed version.
//!
//! Compatibility is derived, not pinned: any stable `1.x` release with
//! `x >= C_API_VERSION` exports the C API Pumas requests, because ONNX Runtime
//! keeps older API versions available in newer releases. A release that was
//! verified end to end is evidence of a working combination, not a requirement.

use crate::models::GitHubAsset;

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

/// The library file, relative to an installed version directory, that the
/// provider loads. On Linux and macOS upstream ships this name as a link to the
/// versioned library file; the link must resolve inside the version directory.
pub const fn library_relative_path() -> &'static str {
    if cfg!(target_os = "windows") {
        "lib/onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "lib/libonnxruntime.dylib"
    } else {
        "lib/libonnxruntime.so"
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

/// Upstream archive platform names for an OS/architecture, most specific first.
///
/// `os` and `arch` use `std::env::consts` spelling.
pub fn platform_archive_names(version: &semver::Version, os: &str, arch: &str) -> Vec<String> {
    let platforms: &[&str] = match (os, arch) {
        ("linux", "x86_64") => &["linux-x64"],
        ("linux", "aarch64") => &["linux-aarch64"],
        ("macos", "aarch64") => &["osx-arm64", "osx-universal2"],
        ("macos", "x86_64") => &["osx-x86_64", "osx-universal2"],
        ("windows", "x86_64") => &["win-x64"],
        ("windows", "aarch64") => &["win-arm64"],
        _ => &[],
    };
    let extension = if os == "windows" { "zip" } else { "tgz" };
    platforms
        .iter()
        .map(|platform| format!("onnxruntime-{platform}-{version}.{extension}"))
        .collect()
}

/// Select the CPU archive for a platform from a release's assets.
pub fn select_platform_archive<'a>(
    assets: &'a [GitHubAsset],
    version: &semver::Version,
    os: &str,
    arch: &str,
) -> Option<&'a GitHubAsset> {
    platform_archive_names(version, os, arch)
        .iter()
        .find_map(|name| assets.iter().find(|asset| &asset.name == name))
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
}
