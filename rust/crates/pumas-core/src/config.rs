//! Centralized configuration for Pumas Library.
//!
//! This module provides configuration constants for installation, network operations,
//! UI dimensions, and other system parameters.

use std::time::Duration;

/// Application-level configuration.
pub struct AppConfig;

impl AppConfig {
    pub const APP_NAME: &'static str = "Pumas Library";
    pub const GITHUB_REPO: &'static str = "comfyanonymous/ComfyUI";
    pub const LOG_FILE_MAX_BYTES: u64 = 10_485_760; // 10MB
    pub const LOG_FILE_BACKUP_COUNT: u32 = 5;
}

/// Configuration for installation process.
pub struct InstallationConfig;

impl InstallationConfig {
    // Package manager timeouts
    pub const UV_INSTALL_TIMEOUT: Duration = Duration::from_secs(600);
    pub const PIP_FALLBACK_TIMEOUT: Duration = Duration::from_secs(900);
    pub const VENV_CREATION_TIMEOUT: Duration = Duration::from_secs(120);

    // Subprocess timeouts
    pub const SUBPROCESS_QUICK_TIMEOUT: Duration = Duration::from_secs(5);
    pub const SUBPROCESS_STANDARD_TIMEOUT: Duration = Duration::from_secs(30);
    pub const SUBPROCESS_LONG_TIMEOUT: Duration = Duration::from_secs(60);
    pub const SUBPROCESS_STOP_TIMEOUT: Duration = Duration::from_secs(2);
    pub const SUBPROCESS_KILL_TIMEOUT: Duration = Duration::from_secs(1);

    // Download and network
    pub const DOWNLOAD_RETRY_ATTEMPTS: u32 = 3;
    pub const URL_FETCH_TIMEOUT: Duration = Duration::from_secs(15);
    pub const URL_QUICK_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

    // Server startup
    pub const SERVER_START_DELAY: Duration = Duration::from_secs(8);
}

/// UI dimensions and timing.
pub struct UiConfig;

impl UiConfig {
    pub const WINDOW_WIDTH: u32 = 400;
    pub const WINDOW_HEIGHT: u32 = 520;
    pub const LOADING_MIN_DURATION: Duration = Duration::from_millis(800);
    pub const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(4000);
    pub const PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(1000);
}

/// Network-related configuration.
pub struct NetworkConfig;

impl NetworkConfig {
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
    pub const QUICK_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
    pub const MAX_RETRIES: u32 = 3;
    pub const DOWNLOAD_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
    pub const DOWNLOAD_CHUNK_SIZE: usize = 8192;
    pub const DOWNLOAD_PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
    pub const DOWNLOAD_TEMP_SUFFIX: &'static str = ".part";
    pub const GITHUB_API_BASE: &'static str = "https://api.github.com";
    pub const GITHUB_RELEASES_PER_PAGE: u32 = 100;
    pub const GITHUB_RELEASES_MAX_PAGES: u32 = 10;
    pub const GITHUB_RELEASES_TTL: Duration = Duration::from_secs(3600);
}

/// Shared directory and path configurations.
pub struct PathsConfig;

impl PathsConfig {
    pub const CACHE_DIR_NAME: &'static str = "cache";
    pub const PIP_CACHE_DIR_NAME: &'static str = "pip";
    pub const SHARED_RESOURCES_DIR_NAME: &'static str = "shared-resources";
    pub const VERSIONS_DIR_NAME: &'static str = "versions";
    pub const ICONS_DIR_NAME: &'static str = "icons";
    pub const CONSTRAINTS_DIR_NAME: &'static str = "constraints";
    pub const CONSTRAINTS_CACHE_FILENAME: &'static str = "constraints-cache.json";
    pub const METADATA_DIR_NAME: &'static str = "metadata";
    pub const LOGS_DIR_NAME: &'static str = "logs";
}

/// App-specific configurations for multi-app support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppId {
    ComfyUI,
    Ollama,
    OpenWebUI,
    InvokeAI,
    KritaDiffusion,
}

impl AppId {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppId::ComfyUI => "comfyui",
            AppId::Ollama => "ollama",
            AppId::OpenWebUI => "openwebui",
            AppId::InvokeAI => "invokeai",
            AppId::KritaDiffusion => "kritadiffusion",
        }
    }

    pub fn github_repo(&self) -> &'static str {
        match self {
            AppId::ComfyUI => "comfyanonymous/ComfyUI",
            AppId::Ollama => "ollama/ollama",
            AppId::OpenWebUI => "open-webui/open-webui",
            AppId::InvokeAI => "invoke-ai/InvokeAI",
            AppId::KritaDiffusion => "Acly/krita-ai-diffusion",
        }
    }

    pub fn versions_dir_name(&self) -> &'static str {
        match self {
            AppId::ComfyUI => "comfyui-versions",
            AppId::Ollama => "ollama-versions",
            AppId::OpenWebUI => "openwebui-versions",
            AppId::InvokeAI => "invokeai-versions",
            AppId::KritaDiffusion => "kritadiffusion-versions",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "comfyui" => Some(AppId::ComfyUI),
            "ollama" => Some(AppId::Ollama),
            "openwebui" => Some(AppId::OpenWebUI),
            "invokeai" => Some(AppId::InvokeAI),
            "kritadiffusion" => Some(AppId::KritaDiffusion),
            _ => None,
        }
    }
}

impl Default for AppId {
    fn default() -> Self {
        AppId::ComfyUI
    }
}

impl std::fmt::Display for AppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_id_roundtrip() {
        for app_id in [
            AppId::ComfyUI,
            AppId::Ollama,
            AppId::OpenWebUI,
            AppId::InvokeAI,
            AppId::KritaDiffusion,
        ] {
            let s = app_id.as_str();
            let parsed = AppId::from_str(s).expect("Should parse");
            assert_eq!(app_id, parsed);
        }
    }

    #[test]
    fn test_timeouts_are_reasonable() {
        assert!(InstallationConfig::UV_INSTALL_TIMEOUT > Duration::from_secs(60));
        assert!(NetworkConfig::REQUEST_TIMEOUT > Duration::ZERO);
    }
}
