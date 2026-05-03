//! Shared handler utilities used across RPC domains.

use crate::server::AppState;
use pumas_app_manager::VersionManager;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Parse an RPC params object into a typed command at the handler boundary.
pub(crate) fn parse_params<T>(method: &str, params: &Value) -> pumas_library::Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(params.clone()).map_err(|error| {
        pumas_library::PumasError::InvalidParams {
            message: format!("Invalid params for {method}: {error}"),
        }
    })
}

/// Validate a required string that serde has already parsed.
pub(crate) fn validate_non_empty(value: String, field: &str) -> pumas_library::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(pumas_library::PumasError::InvalidParams {
            message: format!("Parameter '{field}' must not be empty"),
        });
    }
    Ok(trimmed.to_string())
}

/// Validate a renderer-supplied URL before opening it through the OS shell.
pub(crate) fn validate_external_url(value: String) -> pumas_library::Result<String> {
    let url = validate_non_empty(value, "url")?;
    let parsed =
        url::Url::parse(&url).map_err(|error| pumas_library::PumasError::InvalidParams {
            message: format!("Invalid url: {error}"),
        })?;
    match parsed.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(pumas_library::PumasError::InvalidParams {
            message: format!("Unsupported url scheme: {scheme}"),
        }),
    }
}

async fn canonicalize_local_path(value: String, field: &str) -> pumas_library::Result<PathBuf> {
    let raw = validate_non_empty(value, field)?;
    let path = PathBuf::from(&raw);
    tokio::fs::canonicalize(&path)
        .await
        .map_err(|source| match source.kind() {
            ErrorKind::NotFound => pumas_library::PumasError::InvalidParams {
                message: format!("Parameter '{field}' path not found: {}", path.display()),
            },
            _ => pumas_library::PumasError::Io {
                message: format!("Failed to canonicalize path: {}", path.display()),
                path: Some(path),
                source: Some(source),
            },
        })
}

pub(crate) async fn validate_existing_local_path(
    value: String,
    field: &str,
) -> pumas_library::Result<PathBuf> {
    canonicalize_local_path(value, field).await
}

pub(crate) async fn validate_local_write_target_path(
    value: String,
    field: &str,
) -> pumas_library::Result<PathBuf> {
    let raw = validate_non_empty(value, field)?;
    let candidate = PathBuf::from(&raw);

    match tokio::fs::symlink_metadata(&candidate).await {
        Ok(_) => return canonicalize_local_path(raw, field).await,
        Err(source) if source.kind() == ErrorKind::NotFound => {}
        Err(source) => {
            return Err(pumas_library::PumasError::Io {
                message: format!("Failed to inspect path: {}", candidate.display()),
                path: Some(candidate),
                source: Some(source),
            });
        }
    }

    let mut existing_ancestor = candidate.as_path();
    loop {
        match tokio::fs::metadata(existing_ancestor).await {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(pumas_library::PumasError::InvalidParams {
                        message: format!(
                            "Parameter '{field}' must resolve under a directory: {}",
                            existing_ancestor.display()
                        ),
                    });
                }

                let canonical_ancestor =
                    tokio::fs::canonicalize(existing_ancestor)
                        .await
                        .map_err(|source| pumas_library::PumasError::Io {
                            message: format!(
                                "Failed to canonicalize path: {}",
                                existing_ancestor.display()
                            ),
                            path: Some(existing_ancestor.to_path_buf()),
                            source: Some(source),
                        })?;
                let suffix = candidate.strip_prefix(existing_ancestor).map_err(|_| {
                    pumas_library::PumasError::Other(format!(
                        "Failed to normalize write target path: {}",
                        candidate.display()
                    ))
                })?;

                return Ok(if suffix.as_os_str().is_empty() {
                    canonical_ancestor
                } else {
                    canonical_ancestor.join(suffix)
                });
            }
            Err(source) if source.kind() == ErrorKind::NotFound => {
                existing_ancestor = existing_ancestor.parent().ok_or_else(|| {
                    pumas_library::PumasError::InvalidParams {
                        message: format!(
                            "Parameter '{field}' path is not rooted under an accessible directory: {}",
                            candidate.display()
                        ),
                    }
                })?;
            }
            Err(source) => {
                return Err(pumas_library::PumasError::Io {
                    message: format!("Failed to inspect path: {}", existing_ancestor.display()),
                    path: Some(existing_ancestor.to_path_buf()),
                    source: Some(source),
                });
            }
        }
    }
}

pub(crate) async fn validate_existing_local_file_path(
    value: String,
    field: &str,
) -> pumas_library::Result<PathBuf> {
    let path = canonicalize_local_path(value, field).await?;
    let metadata =
        tokio::fs::metadata(&path)
            .await
            .map_err(|source| pumas_library::PumasError::Io {
                message: format!("Failed to inspect path: {}", path.display()),
                path: Some(path.clone()),
                source: Some(source),
            })?;

    if metadata.is_file() {
        Ok(path)
    } else {
        Err(pumas_library::PumasError::InvalidParams {
            message: format!(
                "Parameter '{field}' must reference a file: {}",
                path.display()
            ),
        })
    }
}

pub(crate) async fn validate_existing_local_directory_path(
    value: String,
    field: &str,
) -> pumas_library::Result<PathBuf> {
    let path = canonicalize_local_path(value, field).await?;
    let metadata =
        tokio::fs::metadata(&path)
            .await
            .map_err(|source| pumas_library::PumasError::Io {
                message: format!("Failed to inspect path: {}", path.display()),
                path: Some(path.clone()),
                source: Some(source),
            })?;

    if metadata.is_dir() {
        Ok(path)
    } else {
        Err(pumas_library::PumasError::InvalidParams {
            message: format!(
                "Parameter '{field}' must reference a directory: {}",
                path.display()
            ),
        })
    }
}

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

pub(crate) async fn get_version_manager(state: &AppState, app_id: &str) -> Option<VersionManager> {
    let managers = state.version_managers.read().await;
    managers.get(app_id).cloned()
}

pub(crate) async fn require_version_manager(
    state: &AppState,
    app_id: &str,
) -> pumas_library::Result<VersionManager> {
    get_version_manager(state, app_id)
        .await
        .ok_or_else(|| pumas_library::PumasError::Config {
            message: format!("Version manager not initialized for app: {}", app_id),
        })
}

pub(crate) async fn path_exists(path: &Path) -> pumas_library::Result<bool> {
    tokio::fs::try_exists(path)
        .await
        .map_err(|source| pumas_library::PumasError::Io {
            message: format!("Failed to inspect path: {}", path.display()),
            path: Some(path.to_path_buf()),
            source: Some(source),
        })
}

pub(crate) async fn read_utf8_file(path: &Path) -> pumas_library::Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(|source| pumas_library::PumasError::Io {
            message: format!("Failed to read file: {}", path.display()),
            path: Some(path.to_path_buf()),
            source: Some(source),
        })
}

/// Extract the JSON header from a safetensors file.
///
/// Safetensors format: 8-byte header size (little-endian u64) followed by JSON header.
pub(crate) async fn extract_safetensors_header(path: &str) -> std::result::Result<Value, String> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;

    // Read header size (8 bytes, little-endian)
    let mut size_buf = [0u8; 8];
    file.read_exact(&mut size_buf)
        .await
        .map_err(|e| e.to_string())?;
    let header_size = u64::from_le_bytes(size_buf) as usize;

    // Sanity check
    if header_size > 100_000_000 {
        return Err("Header size too large".to_string());
    }

    // Read JSON header
    let mut header_buf = vec![0u8; header_size];
    file.read_exact(&mut header_buf)
        .await
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
    if let Some(vm) = get_version_manager(state, "comfyui").await {
        if let Ok(installed) = vm.get_installed_versions().await {
            let version_paths: HashMap<String, PathBuf> = installed
                .into_iter()
                .map(|tag| {
                    let path = vm.version_path(&tag);
                    (tag, path)
                })
                .collect();

            state.api.set_process_version_paths(version_paths).await;
        }
    }
}

/// Detect if running in a sandbox environment.
pub(crate) async fn detect_sandbox_environment() -> (bool, &'static str, Vec<&'static str>) {
    // Check for Flatpak
    if tokio::fs::try_exists("/.flatpak-info")
        .await
        .unwrap_or(false)
    {
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
    if tokio::fs::try_exists("/.dockerenv").await.unwrap_or(false) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;
    use tempfile::TempDir;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Command {
        #[serde(alias = "modelId")]
        model_id: String,
    }

    #[test]
    fn parse_params_accepts_aliases() {
        let command: Command = parse_params("test_method", &json!({"modelId": "abc"})).unwrap();

        assert_eq!(
            command,
            Command {
                model_id: "abc".to_string()
            }
        );
    }

    #[test]
    fn parse_params_rejects_missing_required_field() {
        let error = parse_params::<Command>("test_method", &json!({})).unwrap_err();

        assert!(error.to_string().contains("Invalid params for test_method"));
    }

    #[test]
    fn validate_external_url_rejects_non_web_schemes() {
        let error = validate_external_url("file:///tmp/example".to_string()).unwrap_err();

        assert!(error.to_string().contains("Unsupported url scheme"));
    }

    #[tokio::test]
    async fn validate_existing_local_file_path_canonicalizes_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("model.gguf");
        std::fs::write(&file_path, b"gguf").unwrap();

        let validated =
            validate_existing_local_file_path(file_path.to_string_lossy().to_string(), "file_path")
                .await
                .unwrap();

        assert_eq!(validated, file_path.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn validate_existing_local_file_path_rejects_missing_path() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("missing.gguf");

        let error =
            validate_existing_local_file_path(missing.to_string_lossy().to_string(), "file_path")
                .await
                .unwrap_err();

        assert!(error.to_string().contains("path not found"));
    }

    #[tokio::test]
    async fn validate_existing_local_directory_path_rejects_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("bundle.json");
        std::fs::write(&file_path, b"{}").unwrap();

        let error = validate_existing_local_directory_path(
            file_path.to_string_lossy().to_string(),
            "directory_path",
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("must reference a directory"));
    }

    #[tokio::test]
    async fn validate_local_write_target_path_preserves_missing_child_under_existing_parent() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("outputs").join("result.txt");

        let validated =
            validate_local_write_target_path(target.to_string_lossy().to_string(), "file_path")
                .await
                .unwrap();
        let expected = tokio::fs::canonicalize(temp_dir.path())
            .await
            .unwrap()
            .join("outputs")
            .join("result.txt");

        assert_eq!(validated, expected);
    }

    #[tokio::test]
    async fn validate_local_write_target_path_canonicalizes_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("existing.txt");
        std::fs::write(&file_path, b"ok").unwrap();

        let validated =
            validate_local_write_target_path(file_path.to_string_lossy().to_string(), "file_path")
                .await
                .unwrap();

        assert_eq!(validated, file_path.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn extract_safetensors_header_reads_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("model.safetensors");
        let header = json!({
            "__metadata__": {
                "architecture": "flux",
                "author": "test"
            }
        });
        let header_bytes = serde_json::to_vec(&header).unwrap();
        let mut file_bytes = Vec::with_capacity(8 + header_bytes.len());
        file_bytes.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
        file_bytes.extend_from_slice(&header_bytes);
        std::fs::write(&path, file_bytes).unwrap();

        let metadata = extract_safetensors_header(path.to_str().unwrap())
            .await
            .unwrap();

        assert_eq!(
            metadata,
            json!({
                "architecture": "flux",
                "author": "test"
            })
        );
    }

    #[tokio::test]
    async fn extract_safetensors_header_rejects_large_header() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("oversized.safetensors");
        let mut file_bytes = Vec::with_capacity(8);
        file_bytes.extend_from_slice(&(100_000_001_u64).to_le_bytes());
        std::fs::write(&path, file_bytes).unwrap();

        let error = extract_safetensors_header(path.to_str().unwrap())
            .await
            .unwrap_err();

        assert!(error.contains("Header size too large"));
    }
}
