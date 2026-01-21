//! Custom node types and data structures.

use serde::{Deserialize, Serialize};

/// Information about an installed custom node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCustomNode {
    /// Name of the custom node (directory name)
    pub name: String,
    /// Full path to the custom node directory
    pub path: String,
    /// Git remote URL if available
    pub git_url: Option<String>,
    /// Date the node was installed (ISO 8601 format)
    pub installed_date: Option<String>,
    /// Whether the node has a requirements.txt file
    pub has_requirements: bool,
    /// Whether this is a git repository
    pub is_git_repo: bool,
    /// Current git branch if available
    pub git_branch: Option<String>,
    /// Current git commit hash (short) if available
    pub git_commit: Option<String>,
}

impl InstalledCustomNode {
    /// Create a new InstalledCustomNode with minimal information.
    pub fn new(name: String, path: String) -> Self {
        Self {
            name,
            path,
            git_url: None,
            installed_date: None,
            has_requirements: false,
            is_git_repo: false,
            git_branch: None,
            git_commit: None,
        }
    }
}

/// Result of a custom node installation operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// Whether the installation was successful
    pub success: bool,
    /// Name of the installed node
    pub node_name: String,
    /// Path where the node was installed
    pub install_path: String,
    /// Error message if installation failed
    pub error: Option<String>,
    /// Whether the node has requirements that need to be installed
    pub has_requirements: bool,
}

/// Result of a custom node update operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    /// Whether the update was successful
    pub success: bool,
    /// Name of the updated node
    pub node_name: String,
    /// Output from git pull
    pub output: Option<String>,
    /// Error message if update failed
    pub error: Option<String>,
    /// Whether there were any changes
    pub had_changes: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installed_custom_node_new() {
        let node = InstalledCustomNode::new(
            "ComfyUI-Manager".to_string(),
            "/path/to/custom_nodes/ComfyUI-Manager".to_string(),
        );

        assert_eq!(node.name, "ComfyUI-Manager");
        assert!(!node.has_requirements);
        assert!(!node.is_git_repo);
        assert!(node.git_url.is_none());
    }

    #[test]
    fn test_installed_custom_node_serialization() {
        let node = InstalledCustomNode {
            name: "TestNode".to_string(),
            path: "/test/path".to_string(),
            git_url: Some("https://github.com/user/TestNode".to_string()),
            installed_date: Some("2024-01-15T12:00:00Z".to_string()),
            has_requirements: true,
            is_git_repo: true,
            git_branch: Some("main".to_string()),
            git_commit: Some("abc1234".to_string()),
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("gitUrl"));
        assert!(json.contains("hasRequirements"));
        assert!(json.contains("isGitRepo"));
        assert!(json.contains("gitBranch"));
        assert!(json.contains("gitCommit"));

        // Test deserialization
        let deserialized: InstalledCustomNode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "TestNode");
        assert_eq!(
            deserialized.git_url,
            Some("https://github.com/user/TestNode".to_string())
        );
    }

    #[test]
    fn test_install_result_serialization() {
        let result = InstallResult {
            success: true,
            node_name: "TestNode".to_string(),
            install_path: "/path/to/node".to_string(),
            error: None,
            has_requirements: true,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("nodeName"));
        assert!(json.contains("installPath"));
    }
}
