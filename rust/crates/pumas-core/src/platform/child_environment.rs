//! The environment Pumas hands to the runtime processes it owns.
//!
//! Runtime children (Ollama, Torch, llama.cpp and the runtime-profile owner's
//! children) start from an empty environment. Only the variables below are
//! inherited from Pumas; everything a launch needs beyond them is passed
//! explicitly by its launch configuration, which is applied afterwards and
//! wins on conflict.
//!
//! Inherited: the platform basics a process needs to run (`PATH`, `HOME`,
//! user and locale, temporary and XDG directories, and on Windows the system
//! directories), GPU selection and driver discovery (CUDA, ROCm/HIP, Intel
//! Level Zero, Vulkan), proxy and TLS trust settings (the same transport set
//! the managed Python bootstrap forwards), and `OLLAMA_*` user settings.
//!
//! Not inherited, by design: loader injection (`LD_PRELOAD`, `LD_AUDIT`,
//! `DYLD_*`), the inherited `LD_LIBRARY_PATH`, interpreter injection
//! (`PYTHONPATH`, `PYTHONHOME`, `PYTHONSTARTUP` and other `PYTHON*`), and
//! credentials such as `HF_TOKEN`. A launch that needs a library path sets it
//! explicitly (the llama.cpp launch configuration does).

use std::ffi::{OsStr, OsString};
use std::process::Command;

/// Variables inherited by exact name.
const INHERITED: &[&str] = &[
    // Process basics.
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "LANGUAGE",
    "TZ",
    "TMPDIR",
    "TEMP",
    "TMP",
    "XDG_RUNTIME_DIR",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    // Windows system directories and identity. Winsock needs SystemRoot.
    "SystemRoot",
    "SystemDrive",
    "WINDIR",
    "ComSpec",
    "PATHEXT",
    "USERPROFILE",
    "USERNAME",
    "USERDOMAIN",
    "COMPUTERNAME",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramData",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "CommonProgramFiles",
    "CommonProgramFiles(x86)",
    "CommonProgramW6432",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_IDENTIFIER",
    "OS",
    // NVIDIA CUDA device selection and toolkit discovery.
    "CUDA_VISIBLE_DEVICES",
    "CUDA_DEVICE_ORDER",
    "CUDA_HOME",
    "CUDA_PATH",
    "NVIDIA_VISIBLE_DEVICES",
    "NVIDIA_DRIVER_CAPABILITIES",
    "PYTORCH_CUDA_ALLOC_CONF",
    // AMD ROCm/HIP.
    "HIP_VISIBLE_DEVICES",
    "ROCR_VISIBLE_DEVICES",
    "GPU_DEVICE_ORDINAL",
    "HSA_OVERRIDE_GFX_VERSION",
    "ROCM_PATH",
    "HIP_PATH",
    "PYTORCH_HIP_ALLOC_CONF",
    // Intel Level Zero / oneAPI.
    "ZE_AFFINITY_MASK",
    "ONEAPI_DEVICE_SELECTOR",
    // Vulkan driver discovery and llama.cpp Vulkan device selection.
    "VK_ICD_FILENAMES",
    "VK_DRIVER_FILES",
    "GGML_VK_VISIBLE_DEVICES",
    // Proxy and TLS trust (same set as the managed Python bootstrap).
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Variables inherited by prefix.
const INHERITED_PREFIXES: &[&str] = &[
    // Locale categories (LC_ALL, LC_CTYPE, ...).
    "LC_",
    // Versioned CUDA toolkit paths on Windows (CUDA_PATH_V12_4, ...).
    "CUDA_PATH_V",
    // User settings for the Ollama runtime.
    "OLLAMA_",
];

fn names_match(left: &str, right: &str) -> bool {
    // Windows environment names are case-insensitive.
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

fn prefix_matches(name: &str, prefix: &str) -> bool {
    name.len() >= prefix.len() && names_match(&name[..prefix.len()], prefix)
}

/// Whether a parent variable is inherited by an owned runtime child.
pub fn is_inherited(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    INHERITED.iter().any(|allowed| names_match(name, allowed))
        || INHERITED_PREFIXES
            .iter()
            .any(|prefix| name.is_char_boundary(prefix.len()) && prefix_matches(name, prefix))
}

/// The subset of `environment` an owned runtime child inherits.
pub fn inherited_environment(
    environment: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    environment
        .into_iter()
        .filter(|(name, _)| is_inherited(name))
        .collect()
}

/// Clear `command`'s environment and inherit only the allowlisted variables
/// from this process. Apply launch-specific variables after this call.
pub fn apply_owned_child_environment(command: &mut Command) {
    command.env_clear();
    command.envs(inherited_environment(std::env::vars_os()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_children_inherit_runtime_basics_but_not_injection_or_credentials() {
        let environment = [
            "PATH",
            "HOME",
            "LANG",
            "LC_ALL",
            "TMPDIR",
            "CUDA_VISIBLE_DEVICES",
            "HIP_VISIBLE_DEVICES",
            "HTTPS_PROXY",
            "SSL_CERT_FILE",
            "OLLAMA_MODELS",
            "LD_PRELOAD",
            "LD_AUDIT",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "PYTHONPATH",
            "PYTHONHOME",
            "PYTHONSTARTUP",
            "HF_TOKEN",
            "PUMAS_RELEASE_API_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
        ]
        .map(|name| (OsString::from(name), OsString::from("value")));
        let inherited: Vec<_> = inherited_environment(environment)
            .into_iter()
            .map(|(name, _)| name.into_string().unwrap())
            .collect();
        assert_eq!(
            inherited,
            [
                "PATH",
                "HOME",
                "LANG",
                "LC_ALL",
                "TMPDIR",
                "CUDA_VISIBLE_DEVICES",
                "HIP_VISIBLE_DEVICES",
                "HTTPS_PROXY",
                "SSL_CERT_FILE",
                "OLLAMA_MODELS",
            ]
        );
    }

    #[test]
    fn command_environment_is_cleared_before_inheriting() {
        let mut command = Command::new("runtime");
        command.env("LD_PRELOAD", "/tmp/inject.so");
        apply_owned_child_environment(&mut command);
        let configured: Vec<_> = command.get_envs().collect();
        assert!(configured
            .iter()
            .all(|(name, value)| value.is_some() && is_inherited(name)));
        assert!(!configured
            .iter()
            .any(|(name, _)| *name == OsStr::new("LD_PRELOAD")));
    }

    #[test]
    fn short_names_do_not_match_longer_prefixes() {
        assert!(!is_inherited(OsStr::new("LC")));
        assert!(!is_inherited(OsStr::new("OLLAMA")));
        assert!(is_inherited(OsStr::new("OLLAMA_HOST")));
    }
}
