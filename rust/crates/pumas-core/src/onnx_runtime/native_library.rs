//! Loads the ONNX Runtime library of an installed runtime version (downloaded
//! by the version manager) into an ONNX host process, or a developer's
//! explicit `ORT_DYLIB_PATH` override. This executes trusted dependency code;
//! it cannot validate arbitrary foreign pointers from an untrusted shared
//! library. The SDK owns its immutable C API tables and version string. A held
//! loader handle keeps them live through preflight and ORT's process-wide
//! adoption. Failure drops the local handle; successful adoption leaves ORT
//! owning it.
//!
//! ORT reopens the library by path and keeps the first library a process
//! adopts. That is why each runtime profile runs its own ONNX host: one host
//! loads one installed version for its whole life. After adoption Pumas
//! compares ORT's API table with the one the preflighted handle exports, so a
//! file swapped after preflight, or a different library adopted earlier in the
//! same process, is refused with a reason.
#![allow(unsafe_code)]

use super::{OnnxRuntimeError, OnnxRuntimeLibrary};
use crate::onnx_runtime_distribution::{library_relative_path, C_API_VERSION};
use std::{
    ffi::{CStr, OsString},
    path::{Path, PathBuf},
};

/// Resolve the library to load: the developer override, else the selected
/// installed version. Without either the refusal says how to install one.
fn runtime_library_path(
    explicit: Option<OsString>,
    selected: Option<&OnnxRuntimeLibrary>,
) -> Result<PathBuf, OnnxRuntimeError> {
    let library = if let Some(path) = explicit {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(runtime_library_error(
                "ORT_DYLIB_PATH must be an absolute file path",
            ));
        }
        if !path.is_file() {
            return Err(runtime_library_error(
                "the ONNX Runtime library file named by ORT_DYLIB_PATH is missing",
            ));
        }
        path
    } else if let Some(selected) = selected {
        if !selected.path().is_file() {
            return Err(runtime_library_error(&format!(
                "the installed ONNX Runtime version {} is missing its library file {}; \
                 remove and reinstall that version",
                selected.version_tag(),
                library_relative_path()
            )));
        }
        selected.path().to_path_buf()
    } else {
        return Err(runtime_library_error(
            "no ONNX Runtime version is selected for this ONNX host",
        ));
    };
    library
        .canonicalize()
        .map_err(|_| runtime_library_error("cannot resolve the selected native library file"))
}

fn runtime_library_error(reason: &str) -> OnnxRuntimeError {
    OnnxRuntimeError {
        code: super::OnnxRuntimeErrorCode::Backend,
        field: Some("runtime_library".into()),
        message: format!(
            "{reason}; install a compatible ONNX Runtime version (1.{C_API_VERSION} or newer, \
             C API {C_API_VERSION}) from the ONNX Runtime versions in Pumas and launch an ONNX \
             Runtime profile with it. Developers may instead set ORT_DYLIB_PATH to an absolute \
             library file"
        ),
    }
}

pub(super) fn ensure_runtime_library(
    selected: Option<&OnnxRuntimeLibrary>,
) -> Result<(), OnnxRuntimeError> {
    let path = runtime_library_path(
        std::env::var_os(super::host_protocol::ORT_DYLIB_PATH_ENV),
        selected,
    )?;
    // Pinned ORT constructs native errors while handling loader failures, which
    // re-enters initialization. Reject those failures before entering ORT.
    let (_held_library, preflighted_api) = preflight_library(&path)?;
    // Load the explicitly selected library fallibly before Session's lazy API
    // initialization, and commit the environment with telemetry events off:
    // Pumas disables third-party telemetry. That API switch is what Windows
    // builds honor; the host's launch environment also sets
    // `ORT_DISABLE_TELEMETRY`, which POSIX builds read before any API call.
    // A second load in the same host finds the options already committed.
    let _committed = ort::environment::init_from(path)
        .map_err(|error| {
            runtime_library_error(&format!(
                "ONNX Runtime library could not be loaded: {error}"
            ))
        })?
        .with_telemetry(false)
        .commit();
    // While `_held_library` stays loaded, reopening the same file yields the
    // same image, so the API tables match exactly when ORT adopted it.
    let adopted_api: *const ort::sys::OrtApi = ort::api();
    if !std::ptr::eq(adopted_api, preflighted_api) {
        return Err(runtime_library_error(
            "ONNX Runtime adopted a different library than the one Pumas verified (another \
             library was loaded earlier in this ONNX host, or the file changed after \
             verification); restart the ONNX Runtime profile",
        ));
    }
    Ok(())
}

fn preflight_library(
    path: &Path,
) -> Result<(libloading::Library, *const ort::sys::OrtApi), OnnxRuntimeError> {
    // SAFETY: Only the host-selected, separately verified SDK is executable
    // authority here, never a model/request-supplied path. SDK initialization
    // obeys its native ABI contract and retains no borrowed Rust inputs.
    let library = unsafe { libloading::Library::new(path) }.map_err(|error| {
        runtime_library_error(&format!(
            "ONNX Runtime library could not be loaded: {error}"
        ))
    })?;
    // SAFETY: The trusted SDK exports OrtGetApiBase with this exact C ABI, as
    // declared by the maintained ort-sys bindings. The symbol borrows `library`.
    let get_base = unsafe {
        library
            .get::<unsafe extern "system" fn() -> *const ort::sys::OrtApiBase>(b"OrtGetApiBase\0")
    }
    .map_err(|_| runtime_library_error("ONNX Runtime library lacks OrtGetApiBase"))?;
    // SAFETY: The retained SDK handle keeps the function and its returned
    // immutable API table live. The SDK contract permits concurrent calls.
    let base = unsafe { get_base() };
    if base.is_null() {
        return Err(runtime_library_error(
            "ONNX Runtime returned a null API base",
        ));
    }
    // SAFETY: Non-null `base` comes from the trusted SDK and points to its live
    // initialized OrtApiBase table. GetVersionString returns a static C string.
    let version = unsafe { ((*base).GetVersionString)() };
    if version.is_null() {
        return Err(runtime_library_error(
            "ONNX Runtime returned a null version string",
        ));
    }
    // SAFETY: The SDK promises a NUL-terminated version string that remains
    // live while `library` is held; it is borrowed only within this function.
    let version = unsafe { CStr::from_ptr(version) }
        .to_str()
        .map_err(|_| runtime_library_error("ONNX Runtime returned an invalid version string"))?;
    let version = semver::Version::parse(version)
        .map_err(|_| runtime_library_error("ONNX Runtime returned an invalid version"))?;
    if version.major != 1 || version.minor < u64::from(ort::MINOR_VERSION) {
        return Err(runtime_library_error(
            "ONNX Runtime is incompatible with C API 24",
        ));
    }
    // SAFETY: The retained immutable SDK API table owns GetApi and accepts a
    // version number. A null result means this version is unsupported.
    let api = unsafe { ((*base).GetApi)(ort::sys::ORT_API_VERSION) };
    if api.is_null() {
        return Err(runtime_library_error(
            "ONNX Runtime does not provide C API 24",
        ));
    }
    Ok((library, api))
}

#[cfg(test)]
mod runtime_library_tests {
    use super::*;

    fn installed_version(root: &Path, tag: &str, with_library: bool) -> OnnxRuntimeLibrary {
        let version_dir = root.join(tag);
        let library = version_dir.join(library_relative_path());
        std::fs::create_dir_all(library.parent().unwrap()).unwrap();
        if with_library {
            std::fs::write(&library, b"installed library fixture").unwrap();
        }
        OnnxRuntimeLibrary::installed_version(tag, &version_dir)
    }

    #[test]
    fn explicit_runtime_path_is_required_and_never_falls_back_when_invalid() {
        let fixture = tempfile::tempdir().unwrap();
        let selected = fixture.path().join("selected-library");
        std::fs::write(&selected, b"selection fixture").unwrap();
        let installed = installed_version(fixture.path(), "v1.24.2+cpu", true);
        assert_eq!(
            runtime_library_path(Some(selected.clone().into_os_string()), Some(&installed))
                .unwrap(),
            selected.canonicalize().unwrap(),
            "the developer override wins over the installed version"
        );
        for path in [OsString::new(), OsString::from("libonnxruntime.so")] {
            let error = runtime_library_path(Some(path), Some(&installed)).unwrap_err();
            assert_eq!(error.field.as_deref(), Some("runtime_library"));
            assert!(error.message.contains("absolute file path"));
        }
        let error = runtime_library_path(
            Some(fixture.path().join("missing-library").into_os_string()),
            Some(&installed),
        )
        .unwrap_err();
        assert!(error.message.contains("ORT_DYLIB_PATH is missing"));
    }

    #[test]
    fn installed_version_library_is_loaded_and_its_absence_is_explained() {
        let fixture = tempfile::tempdir().unwrap();
        let installed = installed_version(fixture.path(), "v1.24.2+cpu", true);
        assert_eq!(
            runtime_library_path(None, Some(&installed)).unwrap(),
            installed.path().canonicalize().unwrap()
        );

        let broken = installed_version(fixture.path(), "v1.25.0+cpu", false);
        let error = runtime_library_path(None, Some(&broken)).unwrap_err();
        assert!(error.message.contains("v1.25.0+cpu"), "{}", error.message);
        assert!(error.message.contains("reinstall"), "{}", error.message);
    }

    #[test]
    fn without_a_selected_version_the_refusal_says_how_to_install_one() {
        let fixture = tempfile::tempdir().unwrap();
        // A library beside the executable is never picked up: no package ships one.
        let name = Path::new(library_relative_path()).file_name().unwrap();
        std::fs::write(fixture.path().join(name), b"planted library").unwrap();

        let error = runtime_library_path(None, None).unwrap_err();
        assert_eq!(error.field.as_deref(), Some("runtime_library"));
        for expected in [
            "no ONNX Runtime version is selected",
            "install a compatible ONNX Runtime version",
            "1.24 or newer",
            "ORT_DYLIB_PATH",
        ] {
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }

    #[cfg(unix)]
    #[test]
    fn explicit_runtime_path_resolves_a_symlinked_temporary_directory() {
        use std::os::unix::fs::symlink;

        let fixture = tempfile::tempdir().unwrap();
        let directory = fixture.path().join("native-sdk");
        let alias = fixture.path().join("temporary-directory-alias");
        std::fs::create_dir(&directory).unwrap();
        symlink(&directory, &alias).unwrap();
        let selected = alias.join("selected-library");
        std::fs::write(&selected, b"selection fixture").unwrap();

        let resolved = runtime_library_path(Some(selected.clone().into_os_string()), None).unwrap();
        assert_eq!(resolved, selected.canonicalize().unwrap());
        assert_eq!(
            resolved,
            directory.join("selected-library").canonicalize().unwrap()
        );
    }
}
