//! Names the missing piece when the CUDA execution provider cannot initialize.
//!
//! The NVIDIA driver belongs to the operating system, not to a Pumas runtime:
//! Pumas only detects it here and points at the official download. It never
//! installs, tracks or removes it. The CUDA runtime and cuDNN libraries are the
//! ONNX Runtime build's own dependencies: Pumas installs them with each CUDA
//! version, so when they fail to load the remedy is reinstalling that version.
#![allow(unsafe_code)]

/// The driver's user-mode library, which every CUDA program loads.
#[cfg(windows)]
const DRIVER_LIBRARY: &str = "nvcuda.dll";
#[cfg(not(windows))]
const DRIVER_LIBRARY: &str = "libcuda.so.1";

/// NVIDIA's official driver download page.
pub(super) const NVIDIA_DRIVER_DOWNLOAD_URL: &str = "https://www.nvidia.com/en-us/drivers/";

/// What the host system's NVIDIA driver offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CudaDriverStatus {
    /// The driver library cannot be loaded: no NVIDIA driver is installed.
    Missing,
    /// The driver loaded but did not report the CUDA version it supports.
    Unreported,
    /// The newest CUDA version the driver supports, encoded as CUDA does
    /// (`major * 1000 + minor * 10`, so 12.8 is 12080).
    Supports(i32),
}

/// Ask the installed NVIDIA driver which CUDA version it supports.
pub(super) fn probe_driver() -> CudaDriverStatus {
    // SAFETY: This loads the system's NVIDIA driver by its fixed name, the
    // same library the CUDA execution provider loads; it is never a model or
    // request-supplied path.
    let Ok(library) = (unsafe { libloading::Library::new(DRIVER_LIBRARY) }) else {
        return CudaDriverStatus::Missing;
    };
    // SAFETY: The CUDA driver API exports `CUresult cuDriverGetVersion(int*)`
    // with the C ABI. The symbol borrows `library`, which outlives the call.
    let Ok(get_version) =
        (unsafe { library.get::<unsafe extern "C" fn(*mut i32) -> i32>(b"cuDriverGetVersion\0") })
    else {
        return CudaDriverStatus::Unreported;
    };
    let mut version = 0;
    // SAFETY: cuDriverGetVersion may be called before cuInit and only writes
    // one int through the valid pointer it is given.
    let result = unsafe { get_version(&mut version) };
    if result != 0 || version <= 0 {
        CudaDriverStatus::Unreported
    } else {
        CudaDriverStatus::Supports(version)
    }
}

/// Where the CUDA runtime and cuDNN libraries of the loaded build come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CudaLibraries {
    /// Pumas installed them with the ONNX Runtime version.
    Installed,
    /// The installed version has none; it was installed before Pumas
    /// installed CUDA libraries with CUDA builds.
    NotInstalled,
    /// A developer's `ORT_DYLIB_PATH` library, whose dependencies are the
    /// developer's to provide.
    External,
}

/// Explain a CUDA execution provider failure from the driver's status, the
/// CUDA major version the build needs (unknown for a developer's
/// `ORT_DYLIB_PATH` library) and where its CUDA libraries come from.
pub(super) fn cuda_failure_hint(
    status: CudaDriverStatus,
    required_major: Option<u32>,
    libraries: CudaLibraries,
) -> String {
    let needs = required_major.map_or_else(
        || "its CUDA major version".to_string(),
        |major| format!("CUDA {major}"),
    );
    let libraries_hint = match libraries {
        CudaLibraries::Installed => format!(
            "the {needs} runtime or cuDNN libraries Pumas installed with this ONNX Runtime \
             version did not load: they are missing or incomplete. Remove this version and \
             install it again to restore them"
        ),
        CudaLibraries::NotInstalled => format!(
            "this ONNX Runtime version has no {needs} runtime or cuDNN libraries: it was \
             installed before Pumas installed them with CUDA builds. Remove this version and \
             install it again, and Pumas installs them with it"
        ),
        CudaLibraries::External => format!(
            "the {needs} runtime or cuDNN libraries this ONNX Runtime library needs are \
             missing; they must be on the library path"
        ),
    };
    match status {
        CudaDriverStatus::Missing => format!(
            "; the NVIDIA driver is missing ({DRIVER_LIBRARY} could not be loaded). The driver \
             is part of the operating system and Pumas does not install it: install the \
             official NVIDIA driver from {NVIDIA_DRIVER_DOWNLOAD_URL}"
        ),
        CudaDriverStatus::Supports(version) => {
            let (major, minor) = (version / 1000, (version % 1000) / 10);
            match required_major {
                Some(required) if i64::from(major) < i64::from(required) => format!(
                    "; the NVIDIA driver is too old: it supports CUDA {major}.{minor}, but this \
                     build needs CUDA {required}. The driver is part of the operating system \
                     and Pumas does not install it: update it from \
                     {NVIDIA_DRIVER_DOWNLOAD_URL}"
                ),
                _ => format!(
                    "; the NVIDIA driver is present (it supports CUDA {major}.{minor}), so \
                     {libraries_hint}"
                ),
            }
        }
        CudaDriverStatus::Unreported => format!(
            "; the NVIDIA driver loaded but did not report its CUDA version. The build needs \
             a working NVIDIA driver (from {NVIDIA_DRIVER_DOWNLOAD_URL}); if the driver works, \
             {libraries_hint}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_driver_names_the_driver_and_the_official_download() {
        let hint = cuda_failure_hint(
            CudaDriverStatus::Missing,
            Some(12),
            CudaLibraries::Installed,
        );
        assert!(hint.contains("NVIDIA driver is missing"));
        assert!(hint.contains(DRIVER_LIBRARY));
        assert!(hint.contains(NVIDIA_DRIVER_DOWNLOAD_URL));
        assert!(hint.contains("does not install it"));
        assert!(!hint.contains("cuDNN"));
    }

    #[test]
    fn a_driver_older_than_the_build_says_it_is_too_old() {
        let hint = cuda_failure_hint(
            CudaDriverStatus::Supports(12080),
            Some(13),
            CudaLibraries::Installed,
        );
        assert!(hint.contains("driver is too old"));
        assert!(hint.contains("supports CUDA 12.8"));
        assert!(hint.contains("needs CUDA 13"));
        assert!(hint.contains(NVIDIA_DRIVER_DOWNLOAD_URL));
        assert!(!hint.contains("reinstall") && !hint.contains("install it again"));
    }

    #[test]
    fn a_sufficient_driver_points_at_the_libraries_pumas_installed() {
        let hint = cuda_failure_hint(
            CudaDriverStatus::Supports(13000),
            Some(13),
            CudaLibraries::Installed,
        );
        assert!(hint.contains("driver is present (it supports CUDA 13.0)"));
        assert!(hint.contains("CUDA 13 runtime or cuDNN libraries Pumas installed"));
        assert!(hint.contains("install it again"));
        assert!(!hint.contains("too old"));
        assert!(!hint.contains("library path"));

        let before = cuda_failure_hint(
            CudaDriverStatus::Supports(12040),
            Some(12),
            CudaLibraries::NotInstalled,
        );
        assert!(before.contains("installed before Pumas installed them"));
        assert!(before.contains("install it again"));

        let override_library = cuda_failure_hint(
            CudaDriverStatus::Supports(12040),
            None,
            CudaLibraries::External,
        );
        assert!(override_library.contains("its CUDA major version runtime or cuDNN"));
        assert!(override_library.contains("library path"));
    }

    #[test]
    fn an_unreported_driver_version_names_the_driver_and_the_libraries() {
        let hint = cuda_failure_hint(
            CudaDriverStatus::Unreported,
            Some(12),
            CudaLibraries::Installed,
        );
        assert!(hint.contains("did not report its CUDA version"));
        assert!(hint.contains("CUDA 12 runtime or cuDNN libraries Pumas installed"));
    }
}
