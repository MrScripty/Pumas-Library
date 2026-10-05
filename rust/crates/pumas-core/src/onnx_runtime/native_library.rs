//! Loads only the host operator's separately verified native SDK or the
//! release-packaged adjacent SDK. This executes trusted dependency code; it
//! cannot validate arbitrary foreign pointers from an untrusted shared library.
//! The SDK owns its immutable C API tables and version string. A held loader
//! handle keeps them live through preflight and ORT's process-wide adoption.
//! Failure drops the local handle; successful adoption leaves ORT owning it.
#![allow(unsafe_code)]

use super::OnnxRuntimeError;
use std::{
    ffi::{CStr, OsString},
    path::{Path, PathBuf},
};

fn runtime_library_path(
    explicit: Option<OsString>,
    executable: std::io::Result<PathBuf>,
) -> Result<PathBuf, OnnxRuntimeError> {
    let library = match explicit {
        Some(path) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err(runtime_library_error(
                    "ORT_DYLIB_PATH must be an absolute file path",
                ));
            }
            path
        }
        None => {
            let executable = executable
                .map_err(|_| runtime_library_error("cannot locate the packaged executable"))?;
            let directory = executable.parent().ok_or_else(|| {
                runtime_library_error("cannot locate the packaged executable directory")
            })?;
            #[cfg(target_os = "windows")]
            let name = "onnxruntime.dll";
            #[cfg(target_os = "macos")]
            let name = "libonnxruntime.dylib";
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            let name = "libonnxruntime.so";
            directory.join(name)
        }
    };
    if !library.is_file() {
        return Err(runtime_library_error(
            "the selected ONNX Runtime library file is missing",
        ));
    }
    library
        .canonicalize()
        .map_err(|_| runtime_library_error("cannot resolve the selected native library file"))
}

fn runtime_library_error(reason: &str) -> OnnxRuntimeError {
    OnnxRuntimeError {
        code: super::OnnxRuntimeErrorCode::Backend,
        field: Some("runtime_library".into()),
        message: format!(
            "{reason}; separately provision a compatible ONNX Runtime (C API 24; qualification pin 1.24.2) \
             and set ORT_DYLIB_PATH to its absolute library file, or package it beside the executable. \
             Cargo and Pumas do not download ONNX Runtime"
        ),
    }
}

pub(super) fn ensure_runtime_library() -> Result<(), OnnxRuntimeError> {
    let path = runtime_library_path(std::env::var_os("ORT_DYLIB_PATH"), std::env::current_exe())?;
    // Pinned ORT constructs native errors while handling loader failures, which
    // re-enters initialization. Reject those failures before entering ORT.
    let _held_library = preflight_library(&path)?;
    // Load the explicitly selected library fallibly before Session's lazy API
    // initialization. Leave environment settings with the embedding application.
    let _environment_builder = ort::environment::init_from(path).map_err(|error| {
        runtime_library_error(&format!(
            "ONNX Runtime library could not be loaded: {error}"
        ))
    })?;
    Ok(())
}

fn preflight_library(path: &Path) -> Result<libloading::Library, OnnxRuntimeError> {
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
    if unsafe { ((*base).GetApi)(ort::sys::ORT_API_VERSION) }.is_null() {
        return Err(runtime_library_error(
            "ONNX Runtime does not provide C API 24",
        ));
    }
    Ok(library)
}

#[cfg(test)]
mod runtime_library_tests {
    use super::*;

    #[test]
    fn explicit_runtime_path_is_required_and_never_falls_back_when_invalid() {
        let fixture = tempfile::tempdir().unwrap();
        let selected = fixture.path().join("selected-library");
        std::fs::write(&selected, b"selection fixture").unwrap();
        let unavailable_executable = || Err(std::io::Error::other("unavailable executable"));
        assert_eq!(
            runtime_library_path(
                Some(selected.clone().into_os_string()),
                unavailable_executable()
            )
            .unwrap(),
            selected
        );
        for path in [OsString::new(), OsString::from("libonnxruntime.so")] {
            let error = runtime_library_path(Some(path), unavailable_executable()).unwrap_err();
            assert_eq!(error.field.as_deref(), Some("runtime_library"));
            assert!(error.message.contains("absolute file path"));
        }
        let error = runtime_library_path(
            Some(fixture.path().join("missing-library").into_os_string()),
            Ok(fixture.path().join("pumas-rpc")),
        )
        .unwrap_err();
        assert!(error.message.contains("file is missing"));
        assert!(runtime_library_path(None, Ok(fixture.path().join("pumas-rpc"))).is_err());
    }
}
