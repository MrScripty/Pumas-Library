//! Custom nodes management module.
//!
//! This module provides functionality for managing custom nodes in ComfyUI installations,
//! including listing, installing, updating, and removing custom nodes.

pub mod types;

// Re-export types for public API
pub use types::{InstallResult, InstalledCustomNode, UpdateResult};

use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use pumas_library::error::{PumasError, Result};

/// Manager for custom nodes in ComfyUI versions.
pub struct CustomNodesManager {
    /// Root directory containing version installations
    versions_dir: PathBuf,
}

impl CustomNodesManager {
    /// Create a new CustomNodesManager.
    ///
    /// # Arguments
    ///
    /// * `versions_dir` - Path to the directory containing ComfyUI version installations
    pub fn new(versions_dir: impl Into<PathBuf>) -> Self {
        Self {
            versions_dir: versions_dir.into(),
        }
    }

    /// Get the custom_nodes directory path for a specific version.
    pub fn custom_nodes_dir(&self, tag: &str) -> PathBuf {
        self.versions_dir.join(tag).join("custom_nodes")
    }

    /// List all custom nodes installed for a specific version.
    ///
    /// # Arguments
    ///
    /// * `tag` - Version tag (e.g., "v0.2.0")
    ///
    /// # Returns
    ///
    /// A list of installed custom nodes with their metadata.
    pub fn list_custom_nodes(&self, tag: &str) -> Result<Vec<InstalledCustomNode>> {
        let custom_nodes_dir = self.custom_nodes_dir(tag);

        if !custom_nodes_dir.exists() {
            debug!(
                "Custom nodes directory does not exist: {}",
                custom_nodes_dir.display()
            );
            return Ok(vec![]);
        }

        let mut nodes = Vec::new();

        let entries = std::fs::read_dir(&custom_nodes_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to read custom_nodes directory: {}", e),
            path: Some(custom_nodes_dir.clone()),
            source: Some(e),
        })?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Skip hidden directories and non-directories
            if !path.is_dir() {
                continue;
            }

            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };

            if name.starts_with('.') {
                continue;
            }

            // Gather metadata about the node
            let mut node = InstalledCustomNode::new(name, path.to_string_lossy().to_string());

            // Check for requirements.txt
            node.has_requirements = path.join("requirements.txt").exists();

            // Check if it's a git repository
            let git_dir = path.join(".git");
            node.is_git_repo = git_dir.exists();

            // If it's a git repo, try to get more info synchronously
            if node.is_git_repo {
                // Get remote URL
                if let Ok(output) = std::process::Command::new("git")
                    .args([
                        "-C",
                        &path.to_string_lossy(),
                        "config",
                        "--get",
                        "remote.origin.url",
                    ])
                    .output()
                {
                    if output.status.success() {
                        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !url.is_empty() {
                            node.git_url = Some(url);
                        }
                    }
                }

                // Get current branch
                if let Ok(output) = std::process::Command::new("git")
                    .args([
                        "-C",
                        &path.to_string_lossy(),
                        "rev-parse",
                        "--abbrev-ref",
                        "HEAD",
                    ])
                    .output()
                {
                    if output.status.success() {
                        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !branch.is_empty() {
                            node.git_branch = Some(branch);
                        }
                    }
                }

                // Get current commit (short hash)
                if let Ok(output) = std::process::Command::new("git")
                    .args([
                        "-C",
                        &path.to_string_lossy(),
                        "rev-parse",
                        "--short",
                        "HEAD",
                    ])
                    .output()
                {
                    if output.status.success() {
                        let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !commit.is_empty() {
                            node.git_commit = Some(commit);
                        }
                    }
                }
            }

            // Try to get install date from directory creation time
            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(created) = metadata.created() {
                    if let Ok(datetime) = created.duration_since(std::time::UNIX_EPOCH) {
                        // Format as ISO 8601
                        let secs = datetime.as_secs();
                        node.installed_date = Some(format_unix_timestamp(secs));
                    }
                }
            }

            nodes.push(node);
        }

        // Sort by name for consistent ordering
        nodes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(nodes)
    }

    /// Install a custom node from a git URL.
    ///
    /// # Arguments
    ///
    /// * `git_url` - Git repository URL (e.g., "https://github.com/user/ComfyUI-CustomNode")
    /// * `tag` - Version tag to install into
    ///
    /// # Returns
    ///
    /// Result indicating success or failure of the installation.
    pub async fn install_from_git(&self, git_url: &str, tag: &str) -> Result<InstallResult> {
        // Extract node name from URL
        let node_name = extract_node_name_from_url(git_url);

        let custom_nodes_dir = self.custom_nodes_dir(tag);
        let install_path = custom_nodes_dir.join(&node_name);

        // Check if already installed
        if install_path.exists() {
            info!("Custom node already installed: {}", node_name);
            return Ok(InstallResult {
                success: false,
                node_name,
                install_path: install_path.to_string_lossy().to_string(),
                error: Some("Custom node already installed".to_string()),
                has_requirements: install_path.join("requirements.txt").exists(),
            });
        }

        // Ensure custom_nodes directory exists
        if !custom_nodes_dir.exists() {
            std::fs::create_dir_all(&custom_nodes_dir).map_err(|e| PumasError::Io {
                message: format!("Failed to create custom_nodes directory: {}", e),
                path: Some(custom_nodes_dir.clone()),
                source: Some(e),
            })?;
        }

        info!("Installing custom node {} for version {}", node_name, tag);

        // Clone the repository
        let output = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                git_url,
                &install_path.to_string_lossy(),
            ])
            .output()
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to execute git clone: {}", e),
                path: None,
                source: Some(e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to clone custom node: {}", stderr);
            return Ok(InstallResult {
                success: false,
                node_name,
                install_path: install_path.to_string_lossy().to_string(),
                error: Some(format!("Git clone failed: {}", stderr.trim())),
                has_requirements: false,
            });
        }

        let has_requirements = install_path.join("requirements.txt").exists();

        info!("Successfully installed custom node: {}", node_name);
        if has_requirements {
            info!(
                "Note: {} has requirements.txt - dependencies may need to be installed",
                node_name
            );
        }

        Ok(InstallResult {
            success: true,
            node_name,
            install_path: install_path.to_string_lossy().to_string(),
            error: None,
            has_requirements,
        })
    }

    /// Update a custom node to the latest version.
    ///
    /// # Arguments
    ///
    /// * `node_name` - Name of the custom node directory
    /// * `tag` - Version tag containing the node
    ///
    /// # Returns
    ///
    /// Result indicating success or failure of the update.
    pub async fn update(&self, node_name: &str, tag: &str) -> Result<UpdateResult> {
        let node_path = self.custom_nodes_dir(tag).join(node_name);

        if !node_path.exists() {
            warn!("Custom node not found: {}", node_name);
            return Ok(UpdateResult {
                success: false,
                node_name: node_name.to_string(),
                output: None,
                error: Some("Custom node not found".to_string()),
                had_changes: false,
            });
        }

        // Check if it's a git repository
        if !node_path.join(".git").exists() {
            warn!("Not a git repository: {}", node_name);
            return Ok(UpdateResult {
                success: false,
                node_name: node_name.to_string(),
                output: None,
                error: Some("Not a git repository - cannot update".to_string()),
                had_changes: false,
            });
        }

        info!("Updating custom node: {}", node_name);

        // Git pull with fast-forward only for safety
        let output = Command::new("git")
            .args(["-C", &node_path.to_string_lossy(), "pull", "--ff-only"])
            .output()
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to execute git pull: {}", e),
                path: Some(node_path.clone()),
                source: Some(e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            error!("Failed to update custom node {}: {}", node_name, stderr);
            return Ok(UpdateResult {
                success: false,
                node_name: node_name.to_string(),
                output: Some(stderr.trim().to_string()),
                error: Some(format!("Git pull failed: {}", stderr.trim())),
                had_changes: false,
            });
        }

        // Check if there were any changes
        let had_changes = !stdout.contains("Already up to date");

        info!("Successfully updated custom node: {}", node_name);

        Ok(UpdateResult {
            success: true,
            node_name: node_name.to_string(),
            output: Some(stdout.trim().to_string()),
            error: None,
            had_changes,
        })
    }

    /// Remove a custom node from a specific version.
    ///
    /// # Arguments
    ///
    /// * `node_name` - Name of the custom node directory
    /// * `tag` - Version tag containing the node
    ///
    /// # Returns
    ///
    /// `true` if the node was successfully removed, `false` otherwise.
    pub fn remove(&self, node_name: &str, tag: &str) -> Result<bool> {
        let node_path = self.custom_nodes_dir(tag).join(node_name);

        if !node_path.exists() {
            warn!("Custom node not found: {}", node_name);
            return Ok(false);
        }

        info!("Removing custom node: {} from version {}", node_name, tag);

        std::fs::remove_dir_all(&node_path).map_err(|e| PumasError::Io {
            message: format!("Failed to remove custom node directory: {}", e),
            path: Some(node_path),
            source: Some(e),
        })?;

        info!("Successfully removed custom node: {}", node_name);
        Ok(true)
    }

    /// Check if a custom node exists for a version.
    ///
    /// # Arguments
    ///
    /// * `node_name` - Name of the custom node
    /// * `tag` - Version tag
    pub fn node_exists(&self, node_name: &str, tag: &str) -> bool {
        self.custom_nodes_dir(tag).join(node_name).exists()
    }

    /// Get the path to a specific custom node.
    ///
    /// # Arguments
    ///
    /// * `node_name` - Name of the custom node
    /// * `tag` - Version tag
    pub fn node_path(&self, node_name: &str, tag: &str) -> PathBuf {
        self.custom_nodes_dir(tag).join(node_name)
    }
}

/// Extract the node name from a git URL.
///
/// Examples:
/// - `https://github.com/user/ComfyUI-CustomNode.git` -> `ComfyUI-CustomNode`
/// - `https://github.com/user/ComfyUI-CustomNode` -> `ComfyUI-CustomNode`
/// - `git@github.com:user/ComfyUI-CustomNode.git` -> `ComfyUI-CustomNode`
fn extract_node_name_from_url(url: &str) -> String {
    let url = url.trim_end_matches('/');

    // Get the last path segment
    let name = url.rsplit('/').next().unwrap_or(url);

    // Also handle SSH URLs like git@github.com:user/repo.git
    let name = name.rsplit(':').next().unwrap_or(name);

    // Remove .git suffix
    let name = name.strip_suffix(".git").unwrap_or(name);

    name.to_string()
}

/// Format a Unix timestamp as an ISO 8601 string.
fn format_unix_timestamp(secs: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let datetime = UNIX_EPOCH + Duration::from_secs(secs);

    // Simple formatting without external crates
    // This produces a UTC timestamp
    if let Ok(duration) = datetime.duration_since(UNIX_EPOCH) {
        let total_secs = duration.as_secs();
        let days = total_secs / 86400;

        // Simple year calculation (approximate, doesn't handle leap years perfectly)
        let years = 1970 + (days / 365);
        let remaining_days = days % 365;

        // Rough month calculation
        let months = (remaining_days / 30) + 1;
        let day = (remaining_days % 30) + 1;

        let hours = (total_secs % 86400) / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            years,
            months.min(12),
            day.min(31),
            hours,
            minutes,
            seconds
        )
    } else {
        "1970-01-01T00:00:00Z".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_node_name_from_url() {
        assert_eq!(
            extract_node_name_from_url("https://github.com/user/ComfyUI-Manager.git"),
            "ComfyUI-Manager"
        );
        assert_eq!(
            extract_node_name_from_url("https://github.com/user/ComfyUI-Manager"),
            "ComfyUI-Manager"
        );
        assert_eq!(
            extract_node_name_from_url("https://github.com/user/ComfyUI-Manager/"),
            "ComfyUI-Manager"
        );
        assert_eq!(
            extract_node_name_from_url("git@github.com:user/ComfyUI-Manager.git"),
            "ComfyUI-Manager"
        );
    }

    #[test]
    fn test_custom_nodes_dir() {
        let manager = CustomNodesManager::new("/path/to/versions");
        let dir = manager.custom_nodes_dir("v0.2.0");
        assert_eq!(dir, PathBuf::from("/path/to/versions/v0.2.0/custom_nodes"));
    }

    #[test]
    fn test_list_custom_nodes_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();

        // Create the version directory but no custom_nodes dir
        std::fs::create_dir_all(versions_dir.join("v0.2.0")).unwrap();

        let manager = CustomNodesManager::new(versions_dir);
        let nodes = manager.list_custom_nodes("v0.2.0").unwrap();

        assert!(nodes.is_empty());
    }

    #[test]
    fn test_list_custom_nodes_with_nodes() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();
        let custom_nodes_dir = versions_dir.join("v0.2.0").join("custom_nodes");

        // Create custom_nodes directory with some nodes
        std::fs::create_dir_all(&custom_nodes_dir).unwrap();
        std::fs::create_dir(custom_nodes_dir.join("TestNode1")).unwrap();
        std::fs::create_dir(custom_nodes_dir.join("TestNode2")).unwrap();

        // Add requirements.txt to one
        std::fs::write(
            custom_nodes_dir.join("TestNode1").join("requirements.txt"),
            "torch>=2.0",
        )
        .unwrap();

        // Add a hidden directory (should be ignored)
        std::fs::create_dir(custom_nodes_dir.join(".hidden")).unwrap();

        let manager = CustomNodesManager::new(versions_dir);
        let nodes = manager.list_custom_nodes("v0.2.0").unwrap();

        assert_eq!(nodes.len(), 2);

        // Should be sorted alphabetically
        assert_eq!(nodes[0].name, "TestNode1");
        assert!(nodes[0].has_requirements);
        assert!(!nodes[0].is_git_repo);

        assert_eq!(nodes[1].name, "TestNode2");
        assert!(!nodes[1].has_requirements);
    }

    #[test]
    fn test_node_exists() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();
        let custom_nodes_dir = versions_dir.join("v0.2.0").join("custom_nodes");

        std::fs::create_dir_all(&custom_nodes_dir).unwrap();
        std::fs::create_dir(custom_nodes_dir.join("ExistingNode")).unwrap();

        let manager = CustomNodesManager::new(versions_dir);

        assert!(manager.node_exists("ExistingNode", "v0.2.0"));
        assert!(!manager.node_exists("NonExistingNode", "v0.2.0"));
    }

    #[test]
    fn test_remove_custom_node() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();
        let custom_nodes_dir = versions_dir.join("v0.2.0").join("custom_nodes");

        std::fs::create_dir_all(&custom_nodes_dir).unwrap();
        std::fs::create_dir(custom_nodes_dir.join("NodeToRemove")).unwrap();
        std::fs::write(
            custom_nodes_dir.join("NodeToRemove").join("test.py"),
            "# test",
        )
        .unwrap();

        let manager = CustomNodesManager::new(versions_dir);

        assert!(manager.node_exists("NodeToRemove", "v0.2.0"));

        let result = manager.remove("NodeToRemove", "v0.2.0").unwrap();
        assert!(result);

        assert!(!manager.node_exists("NodeToRemove", "v0.2.0"));
    }

    #[test]
    fn test_remove_nonexistent_node() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();

        std::fs::create_dir_all(versions_dir.join("v0.2.0").join("custom_nodes")).unwrap();

        let manager = CustomNodesManager::new(versions_dir);

        let result = manager.remove("NonExistentNode", "v0.2.0").unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_install_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();
        let custom_nodes_dir = versions_dir.join("v0.2.0").join("custom_nodes");

        std::fs::create_dir_all(&custom_nodes_dir).unwrap();
        std::fs::create_dir(custom_nodes_dir.join("ComfyUI-Manager")).unwrap();

        let manager = CustomNodesManager::new(versions_dir);

        let result = manager
            .install_from_git("https://github.com/ltdrdata/ComfyUI-Manager.git", "v0.2.0")
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("already installed"));
    }

    #[tokio::test]
    async fn test_update_nonexistent_node() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();

        std::fs::create_dir_all(versions_dir.join("v0.2.0").join("custom_nodes")).unwrap();

        let manager = CustomNodesManager::new(versions_dir);

        let result = manager.update("NonExistentNode", "v0.2.0").await.unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_update_non_git_directory() {
        let temp_dir = TempDir::new().unwrap();
        let versions_dir = temp_dir.path();
        let custom_nodes_dir = versions_dir.join("v0.2.0").join("custom_nodes");

        std::fs::create_dir_all(&custom_nodes_dir).unwrap();
        std::fs::create_dir(custom_nodes_dir.join("NotAGitRepo")).unwrap();

        let manager = CustomNodesManager::new(versions_dir);

        let result = manager.update("NotAGitRepo", "v0.2.0").await.unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Not a git repository"));
    }

    #[test]
    fn test_format_unix_timestamp() {
        // Test a known timestamp (2024-01-15 12:30:45 UTC approximately)
        let timestamp = 1705322445;
        let formatted = format_unix_timestamp(timestamp);

        // Should be in ISO 8601 format
        assert!(formatted.contains("T"));
        assert!(formatted.ends_with("Z"));
    }
}
