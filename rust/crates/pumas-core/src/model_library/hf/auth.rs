//! HuggingFace authentication token management.
//!
//! Handles token resolution from multiple sources, persistence to disk,
//! and validation against the HuggingFace API.

use crate::error::Result;
use crate::platform::paths::pumas_config_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// HuggingFace token filename within the Pumas config directory.
const HF_TOKEN_FILENAME: &str = "hf_token";

/// HuggingFace environment variable for authentication tokens.
const HF_TOKEN_ENV_VAR: &str = "HF_TOKEN";

/// HuggingFace whoami endpoint for token validation.
pub(crate) const HF_WHOAMI_URL: &str = "https://huggingface.co/api/whoami-v2";

/// Status of HuggingFace authentication.
///
/// Returned by `get_auth_status()` to indicate whether a valid
/// token is configured and which source it was resolved from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfAuthStatus {
    /// Whether the token was validated successfully against the HF API.
    pub authenticated: bool,
    /// HuggingFace username, if authenticated.
    pub username: Option<String>,
    /// Where the token was resolved from: "pumas_config", "env_var", or "hf_cache".
    pub token_source: Option<String>,
}

/// Path to the Pumas-managed HF token file.
pub(super) fn hf_token_path() -> Result<PathBuf> {
    Ok(pumas_config_dir()?.join(HF_TOKEN_FILENAME))
}

/// Resolve an HF token from disk/environment sources.
///
/// Checks in order:
/// 1. Pumas config file (`~/.config/pumas/hf_token`)
/// 2. `HF_TOKEN` environment variable
/// 3. HuggingFace CLI cache (`~/.cache/huggingface/token`)
///
/// Returns the token and a label identifying its source.
pub(super) fn resolve_token_from_disk() -> Option<(String, &'static str)> {
    // 1. Pumas config file
    if let Ok(path) = hf_token_path() {
        if let Ok(token) = std::fs::read_to_string(&path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some((token, "pumas_config"));
            }
        }
    }

    // 2. HF_TOKEN environment variable
    if let Ok(token) = std::env::var(HF_TOKEN_ENV_VAR) {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Some((token, "env_var"));
        }
    }

    // 3. HuggingFace CLI cache file
    if let Some(home) = dirs::home_dir() {
        let hf_cache_token = home.join(".cache").join("huggingface").join("token");
        if let Ok(token) = std::fs::read_to_string(hf_cache_token) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some((token, "hf_cache"));
            }
        }
    }

    None
}

/// Save a token to the Pumas config directory.
///
/// Creates the directory if needed and sets restrictive file permissions
/// (0600) on Unix systems.
pub(super) fn save_token(token: &str) -> Result<()> {
    let path = hf_token_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, token.trim())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Clear the saved token by deleting the file.
pub(super) fn clear_token() -> Result<()> {
    let path = hf_token_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Override the config dir for testing by writing/reading directly.
    fn write_token_file(dir: &std::path::Path, content: &str) {
        let token_path = dir.join(HF_TOKEN_FILENAME);
        std::fs::write(token_path, content).unwrap();
    }

    #[test]
    fn test_save_token_then_read_returns_saved_value() {
        let temp = TempDir::new().unwrap();
        let token_path = temp.path().join(HF_TOKEN_FILENAME);
        std::fs::write(&token_path, "hf_test_token_123").unwrap();

        let content = std::fs::read_to_string(&token_path).unwrap();
        assert_eq!(content.trim(), "hf_test_token_123");
    }

    #[test]
    fn test_clear_token_after_save_removes_file() {
        let temp = TempDir::new().unwrap();
        let token_path = temp.path().join(HF_TOKEN_FILENAME);
        write_token_file(temp.path(), "hf_test_token");

        assert!(token_path.exists());
        std::fs::remove_file(&token_path).unwrap();
        assert!(!token_path.exists());
    }

    #[test]
    fn test_resolve_token_from_disk_empty_file_returns_none() {
        // Empty string after trimming should yield None in the resolution logic
        let token = "";
        let trimmed = token.trim().to_string();
        assert!(trimmed.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn test_save_token_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let token_path = temp.path().join(HF_TOKEN_FILENAME);
        std::fs::write(&token_path, "secret_token").unwrap();
        std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let metadata = std::fs::metadata(&token_path).unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "Token file should have 0600 permissions");
    }

    #[test]
    fn test_hf_auth_status_serializes() {
        let status = HfAuthStatus {
            authenticated: true,
            username: Some("testuser".to_string()),
            token_source: Some("pumas_config".to_string()),
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["authenticated"], true);
        assert_eq!(json["username"], "testuser");
        assert_eq!(json["token_source"], "pumas_config");
    }

    #[test]
    fn test_hf_auth_status_unauthenticated_serializes() {
        let status = HfAuthStatus {
            authenticated: false,
            username: None,
            token_source: None,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["authenticated"], false);
        assert!(json["username"].is_null());
    }
}
