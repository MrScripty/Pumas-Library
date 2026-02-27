//! Shared handler utilities used across RPC domains.

use crate::server::AppState;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Extract an optional string parameter, supporting both snake_case and camelCase.
pub(crate) fn get_str_param<'a>(params: &'a Value, snake: &str, camel: &str) -> Option<&'a str> {
    params
        .get(snake)
        .or_else(|| params.get(camel))
        .and_then(|v| v.as_str())
}

/// Extract a required string parameter or return an error.
pub(crate) fn require_str_param(
    params: &Value,
    snake: &str,
    camel: &str,
) -> pumas_library::Result<String> {
    get_str_param(params, snake, camel)
        .map(String::from)
        .ok_or_else(|| pumas_library::PumasError::InvalidParams {
            message: format!("Missing required parameter: {}", snake),
        })
}

/// Extract an optional bool parameter, supporting both snake_case and camelCase.
pub(crate) fn get_bool_param(params: &Value, snake: &str, camel: &str) -> Option<bool> {
    params
        .get(snake)
        .or_else(|| params.get(camel))
        .and_then(|v| v.as_bool())
}

/// Extract an optional i64 parameter, supporting both snake_case and camelCase.
pub(crate) fn get_i64_param(params: &Value, snake: &str, camel: &str) -> Option<i64> {
    params
        .get(snake)
        .or_else(|| params.get(camel))
        .and_then(|v| v.as_i64())
}

/// Extract the JSON header from a safetensors file.
///
/// Safetensors format: 8-byte header size (little-endian u64) followed by JSON header.
pub(crate) fn extract_safetensors_header(path: &str) -> std::result::Result<Value, String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;

    // Read header size (8 bytes, little-endian)
    let mut size_buf = [0u8; 8];
    file.read_exact(&mut size_buf).map_err(|e| e.to_string())?;
    let header_size = u64::from_le_bytes(size_buf) as usize;

    // Sanity check
    if header_size > 100_000_000 {
        return Err("Header size too large".to_string());
    }

    // Read JSON header
    let mut header_buf = vec![0u8; header_size];
    file.read_exact(&mut header_buf)
        .map_err(|e| e.to_string())?;

    // Parse JSON - the header contains tensor metadata, not model metadata
    // Safetensors stores tensor shapes/dtypes, not general metadata like GGUF
    let header: Value = serde_json::from_slice(&header_buf).map_err(|e| e.to_string())?;

    // Extract __metadata__ field if present (some safetensors files include this)
    if let Some(metadata) = header.get("__metadata__") {
        Ok(metadata.clone())
    } else {
        // Return tensor info as metadata
        Ok(header)
    }
}

/// Synchronize version paths from ComfyUI version_manager to process_manager.
///
/// This ensures the process manager knows about all installed version directories
/// so it can properly detect and clean up PID files.
pub(crate) async fn sync_version_paths_to_process_manager(state: &AppState) {
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        // Get installed versions
        if let Ok(installed) = vm.get_installed_versions().await {
            let version_paths: HashMap<String, PathBuf> = installed
                .into_iter()
                .map(|tag| {
                    let path = vm.version_path(&tag);
                    (tag, path)
                })
                .collect();

            // Update process manager
            drop(managers); // Release version_managers lock first
            state.api.set_process_version_paths(version_paths).await;
        }
    }
}

/// Detect if running in a sandbox environment.
pub(crate) fn detect_sandbox_environment() -> (bool, &'static str, Vec<&'static str>) {
    // Check for Flatpak
    if std::path::Path::new("/.flatpak-info").exists() {
        return (
            true,
            "flatpak",
            vec![
                "Limited filesystem access",
                "May need portal permissions for some operations",
            ],
        );
    }

    // Check for Snap
    if std::env::var("SNAP").is_ok() {
        return (
            true,
            "snap",
            vec![
                "Limited filesystem access",
                "Strict confinement may limit features",
            ],
        );
    }

    // Check for Docker
    if std::path::Path::new("/.dockerenv").exists() {
        return (
            true,
            "docker",
            vec!["Running in container", "GPU access may require --gpus flag"],
        );
    }

    // Check for AppImage
    if std::env::var("APPIMAGE").is_ok() {
        return (true, "appimage", vec!["Running as AppImage bundle"]);
    }

    (false, "none", vec![])
}
