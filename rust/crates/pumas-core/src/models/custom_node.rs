//! Custom node metadata types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Compatibility status for a custom node with a version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompatibilityStatus {
    Compatible,
    Incompatible,
    #[default]
    Unknown,
}

/// Per-version status of a custom node.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomNodeVersionStatus {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub git_commit: Option<String>,
    #[serde(default)]
    pub git_tag: Option<String>,
    #[serde(default)]
    pub install_date: Option<String>,
    #[serde(default)]
    pub compatibility_status: CompatibilityStatus,
    #[serde(default)]
    pub incompatibility_reason: Option<String>,
    #[serde(default)]
    pub conflicting_packages: Option<Vec<String>>,
    #[serde(default)]
    pub requirements_installed: bool,
}

/// Per-version configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionConfig {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub custom_nodes: HashMap<String, CustomNodeVersionStatus>,
    #[serde(default)]
    pub launch_args: Vec<String>,
    #[serde(default)]
    pub python_path: Option<String>,
    #[serde(default)]
    pub uv_path: Option<String>,
    #[serde(default)]
    pub requirements: HashMap<String, String>,
    #[serde(default)]
    pub requirements_hash: Option<String>,
}

/// Cached compatibility check result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityCache {
    #[serde(default)]
    pub status: CompatibilityStatus,
    #[serde(default)]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub requirements_hash: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub conflicting_packages: Option<Vec<String>>,
    #[serde(default)]
    pub additional_requirements: Option<Vec<String>>,
}

/// Global metadata about a custom node.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomNodeInfo {
    #[serde(default)]
    pub cache_repo: Option<String>,
    #[serde(default)]
    pub git_url: Option<String>,
    #[serde(default)]
    pub last_fetched: Option<String>,
    #[serde(default)]
    pub available_tags: Vec<String>,
    #[serde(default)]
    pub latest_commit: Option<String>,
    #[serde(default)]
    pub has_requirements: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub compatibility_cache: HashMap<String, CompatibilityCache>,
}

/// Root metadata structure for custom_nodes.json.
pub type CustomNodesMetadata = HashMap<String, CustomNodeInfo>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compatibility_status_default() {
        let status = CompatibilityStatus::default();
        assert_eq!(status, CompatibilityStatus::Unknown);
    }

    #[test]
    fn test_custom_node_info_serialization() {
        let info = CustomNodeInfo {
            git_url: Some("https://github.com/user/node".into()),
            has_requirements: true,
            ..Default::default()
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("gitUrl"));
        assert!(json.contains("hasRequirements"));
    }
}
