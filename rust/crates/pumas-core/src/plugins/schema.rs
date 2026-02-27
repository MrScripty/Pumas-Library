//! Plugin configuration schema.
//!
//! Defines the structure for plugin JSON configuration files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How the app is installed and managed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallationType {
    /// Standalone binary download (Ollama, etc.)
    Binary,
    /// Python virtual environment (ComfyUI, etc.)
    PythonVenv,
    /// Docker container
    Docker,
}

/// App capabilities that affect available features.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCapabilities {
    /// Whether the app supports version management.
    #[serde(default)]
    pub has_version_management: bool,
    /// Whether shortcuts can be created.
    #[serde(default)]
    pub supports_shortcuts: bool,
    /// Whether the app has dependencies to install.
    #[serde(default)]
    pub has_dependencies: bool,
    /// Whether to show connection URL/port info.
    #[serde(default)]
    pub has_connection_url: bool,
    /// Whether to show the model library panel.
    #[serde(default)]
    pub has_model_library: bool,
    /// Whether the app provides stats (memory usage, etc.).
    #[serde(default)]
    pub has_stats: bool,
}

/// Connection configuration for the app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    /// Default port the app runs on.
    pub default_port: u16,
    /// Protocol (http, https).
    #[serde(default = "default_protocol")]
    pub protocol: String,
    /// Health check endpoint path.
    #[serde(default)]
    pub health_endpoint: Option<String>,
}

fn default_protocol() -> String {
    "http".to_string()
}

/// Version filtering rules for GitHub releases.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFilter {
    /// Whether to include pre-release versions.
    #[serde(default)]
    pub include_prereleases: bool,
    /// Patterns to exclude from version list.
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// Platform-specific asset name patterns.
    #[serde(default)]
    pub platform_assets: HashMap<String, String>,
}

/// Model format compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCompatibility {
    /// Supported model formats (gguf, safetensors, etc.).
    #[serde(default)]
    pub supported_formats: Vec<String>,
    /// Command template for importing models.
    #[serde(default)]
    pub import_command: Option<String>,
}

/// An API endpoint definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpoint {
    /// HTTP method (GET, POST, etc.).
    #[serde(default = "default_method")]
    pub method: String,
    /// Endpoint path.
    pub endpoint: String,
    /// Request body template with {{placeholders}}.
    #[serde(default)]
    pub body_template: Option<serde_json::Value>,
    /// Response field mapping using JSONPath-like expressions.
    #[serde(default)]
    pub response_mapping: HashMap<String, String>,
    /// Polling interval in milliseconds (for stats endpoints).
    #[serde(default)]
    pub polling_interval_ms: Option<u64>,
}

fn default_method() -> String {
    "GET".to_string()
}

/// Python-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonConfig {
    /// Requirements file name.
    #[serde(default = "default_requirements")]
    pub requirements_file: String,
    /// Entry point script.
    #[serde(default = "default_entry_point")]
    pub entry_point: String,
    /// Required Python version.
    #[serde(default)]
    pub python_version: Option<String>,
}

fn default_requirements() -> String {
    "requirements.txt".to_string()
}

fn default_entry_point() -> String {
    "main.py".to_string()
}

/// A panel section type for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PanelSection {
    /// Section type identifier.
    #[serde(rename = "type")]
    pub section_type: String,
    /// Optional section-specific configuration.
    #[serde(default)]
    pub config: Option<serde_json::Value>,
}

/// Complete plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
    /// Unique app identifier (e.g., "comfyui", "ollama").
    pub id: String,
    /// Display name for the UI.
    pub display_name: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Icon name (lucide icon identifier).
    #[serde(default)]
    pub icon: Option<String>,
    /// GitHub repository (owner/repo format).
    #[serde(default)]
    pub github_repo: Option<String>,
    /// Installation type.
    pub installation_type: InstallationType,

    /// App capabilities.
    #[serde(default)]
    pub capabilities: AppCapabilities,

    /// Connection configuration.
    #[serde(default)]
    pub connection: Option<ConnectionConfig>,

    /// Version filtering rules.
    #[serde(default)]
    pub version_filter: Option<VersionFilter>,

    /// Model compatibility settings.
    #[serde(default)]
    pub model_compatibility: Option<ModelCompatibility>,

    /// Python-specific config (for python-venv apps).
    #[serde(default)]
    pub python_config: Option<PythonConfig>,

    /// API endpoint definitions.
    #[serde(default)]
    pub api: HashMap<String, ApiEndpoint>,

    /// Panel layout sections.
    #[serde(default)]
    pub panel_layout: Vec<PanelSection>,

    /// Sidebar display priority (lower = higher priority).
    #[serde(default = "default_priority")]
    pub sidebar_priority: i32,

    /// Whether enabled by default.
    #[serde(default = "default_true")]
    pub enabled_by_default: bool,
}

fn default_priority() -> i32 {
    100
}

fn default_true() -> bool {
    true
}

impl PluginConfig {
    /// Get the connection URL for this app.
    pub fn connection_url(&self) -> Option<String> {
        self.connection
            .as_ref()
            .map(|c| format!("{}://localhost:{}", c.protocol, c.default_port))
    }

    /// Check if this plugin supports a specific model format.
    pub fn supports_format(&self, format: &str) -> bool {
        self.model_compatibility
            .as_ref()
            .map(|mc| {
                mc.supported_formats
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(format))
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_plugin() {
        let json = r#"{
            "id": "test-app",
            "displayName": "Test App",
            "description": "A test application",
            "installationType": "binary",
            "capabilities": {
                "hasVersionManagement": true,
                "hasConnectionUrl": true
            },
            "connection": {
                "defaultPort": 8080,
                "protocol": "http",
                "healthEndpoint": "/health"
            },
            "panelLayout": [
                {"type": "version_manager"},
                {"type": "connection_info"}
            ],
            "sidebarPriority": 10
        }"#;

        let config: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.id, "test-app");
        assert_eq!(config.display_name, "Test App");
        assert_eq!(config.installation_type, InstallationType::Binary);
        assert!(config.capabilities.has_version_management);
        assert!(config.capabilities.has_connection_url);
        assert_eq!(config.connection.as_ref().unwrap().default_port, 8080);
        assert_eq!(config.panel_layout.len(), 2);
        assert_eq!(config.sidebar_priority, 10);
    }

    #[test]
    fn test_connection_url() {
        let config = PluginConfig {
            id: "test".to_string(),
            display_name: "Test".to_string(),
            description: String::new(),
            icon: None,
            github_repo: None,
            installation_type: InstallationType::Binary,
            capabilities: AppCapabilities::default(),
            connection: Some(ConnectionConfig {
                default_port: 11434,
                protocol: "http".to_string(),
                health_endpoint: None,
            }),
            version_filter: None,
            model_compatibility: None,
            python_config: None,
            api: HashMap::new(),
            panel_layout: vec![],
            sidebar_priority: 100,
            enabled_by_default: true,
        };

        assert_eq!(
            config.connection_url(),
            Some("http://localhost:11434".to_string())
        );
    }

    #[test]
    fn test_supports_format() {
        let config = PluginConfig {
            id: "test".to_string(),
            display_name: "Test".to_string(),
            description: String::new(),
            icon: None,
            github_repo: None,
            installation_type: InstallationType::Binary,
            capabilities: AppCapabilities::default(),
            connection: None,
            version_filter: None,
            model_compatibility: Some(ModelCompatibility {
                supported_formats: vec!["gguf".to_string(), "safetensors".to_string()],
                import_command: None,
            }),
            python_config: None,
            api: HashMap::new(),
            panel_layout: vec![],
            sidebar_priority: 100,
            enabled_by_default: true,
        };

        assert!(config.supports_format("gguf"));
        assert!(config.supports_format("GGUF"));
        assert!(config.supports_format("safetensors"));
        assert!(!config.supports_format("onnx"));
    }
}
