//! Launcher auto-updater functionality.
//!
//! Manages launcher updates via git, checking for new commits and applying updates.

use crate::error::{PumasError, Result};
use crate::models::CommitInfo;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Result of checking for launcher updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    /// Whether an update is available.
    pub has_update: bool,
    /// Current local commit SHA (short).
    pub current_commit: String,
    /// Latest remote commit SHA (short).
    pub latest_commit: String,
    /// Number of commits behind.
    pub commits_behind: i32,
    /// Recent commits from remote.
    pub commits: Vec<CommitInfo>,
    /// Current branch name.
    pub branch: String,
    /// Error message if check failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of applying a launcher update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApplyResult {
    /// Whether the update was successful.
    pub success: bool,
    /// Status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// New commit SHA after update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_commit: Option<String>,
    /// Previous commit SHA before update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_commit: Option<String>,
    /// Error message if update failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Cached update check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedUpdateCheck {
    /// When the check was performed.
    last_checked: DateTime<Utc>,
    /// The cached result.
    result: UpdateCheckResult,
}

/// Manages launcher self-updates via git.
pub struct LauncherUpdater {
    /// Root directory of the launcher repository.
    launcher_root: PathBuf,
    /// GitHub repository owner.
    repo_owner: String,
    /// GitHub repository name.
    repo_name: String,
    /// Path to cache file for update checks.
    cache_file: PathBuf,
}

impl LauncherUpdater {
    /// Create a new LauncherUpdater.
    ///
    /// # Arguments
    ///
    /// * `launcher_root` - Path to the launcher root directory (git repo root)
    pub fn new(launcher_root: impl AsRef<Path>) -> Self {
        let launcher_root = launcher_root.as_ref().to_path_buf();
        let cache_file = launcher_root
            .join("launcher-data")
            .join("cache")
            .join("launcher-update-check.json");

        Self {
            launcher_root,
            repo_owner: "MrScripty".to_string(),
            repo_name: "Pumas-Library".to_string(),
            cache_file,
        }
    }

    /// Check if the launcher is in a git repository.
    pub fn is_git_repo(&self) -> bool {
        self.launcher_root.join(".git").exists()
    }

    /// Check if there are uncommitted changes in the repository.
    pub fn has_uncommitted_changes(&self) -> bool {
        match Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.launcher_root)
            .output()
        {
            Ok(output) => !output.stdout.is_empty(),
            Err(e) => {
                debug!("Failed to check git status: {}", e);
                false
            }
        }
    }

    /// Get the current git commit SHA (short, 7 characters).
    fn get_current_commit(&self) -> Option<String> {
        match Command::new("git")
            .args(["rev-parse", "--short=7", "HEAD"])
            .current_dir(&self.launcher_root)
            .output()
        {
            Ok(output) if output.status.success() => {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
            Ok(output) => {
                debug!("git rev-parse failed: {:?}", output.stderr);
                None
            }
            Err(e) => {
                debug!("Failed to get current commit: {}", e);
                None
            }
        }
    }

    /// Get the current git branch name.
    fn get_current_branch(&self) -> String {
        match Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&self.launcher_root)
            .output()
        {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => "main".to_string(),
        }
    }

    /// Get the launcher version information.
    pub fn get_version_info(&self) -> serde_json::Value {
        let current_commit = self.get_current_commit().unwrap_or_default();
        let branch = self.get_current_branch();
        let is_git_repo = self.is_git_repo();

        serde_json::json!({
            "success": true,
            "version": env!("CARGO_PKG_VERSION"),
            "currentCommit": current_commit,
            "branch": branch,
            "isGitRepo": is_git_repo
        })
    }

    /// Read cached update check result.
    fn get_cached_update_info(&self) -> Option<CachedUpdateCheck> {
        if !self.cache_file.exists() {
            return None;
        }

        match std::fs::read_to_string(&self.cache_file) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(cached) => Some(cached),
                Err(e) => {
                    debug!("Failed to parse cached update info: {}", e);
                    None
                }
            },
            Err(e) => {
                debug!("Failed to read cache file: {}", e);
                None
            }
        }
    }

    /// Save update check result to cache.
    fn cache_update_info(&self, result: &UpdateCheckResult) {
        let cached = CachedUpdateCheck {
            last_checked: Utc::now(),
            result: result.clone(),
        };

        if let Some(parent) = self.cache_file.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                debug!("Failed to create cache directory: {}", e);
                return;
            }
        }

        match serde_json::to_string_pretty(&cached) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&self.cache_file, content) {
                    debug!("Failed to write cache file: {}", e);
                }
            }
            Err(e) => {
                debug!("Failed to serialize cache: {}", e);
            }
        }
    }

    /// Check for launcher updates via GitHub API.
    ///
    /// # Arguments
    ///
    /// * `force_refresh` - If true, bypass cache and fetch fresh data
    pub async fn check_for_updates(&self, force_refresh: bool) -> UpdateCheckResult {
        // Get current commit
        let current_commit = match self.get_current_commit() {
            Some(c) => c,
            None => {
                return UpdateCheckResult {
                    has_update: false,
                    current_commit: String::new(),
                    latest_commit: String::new(),
                    commits_behind: 0,
                    commits: vec![],
                    branch: self.get_current_branch(),
                    error: Some("Not a git repository".to_string()),
                };
            }
        };

        let branch = self.get_current_branch();

        // Check cache first (unless force refresh)
        if !force_refresh {
            if let Some(cached) = self.get_cached_update_info() {
                // Cache valid for 1 hour
                if Utc::now() - cached.last_checked < Duration::hours(1) {
                    debug!("Using cached update info");
                    return cached.result;
                }
            }
        }

        // Fetch from GitHub API
        let api_url = format!(
            "https://api.github.com/repos/{}/{}/commits?sha={}&per_page=10",
            self.repo_owner, self.repo_name, branch
        );

        debug!("Fetching commits from {}", api_url);

        let client = match reqwest::Client::builder()
            .user_agent("pumas-launcher")
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return self.return_cached_or_error(format!("Failed to create HTTP client: {}", e));
            }
        };

        let response = match client.get(&api_url).send().await {
            Ok(r) => r,
            Err(e) => {
                debug!("Network unavailable during update check: {}", e);
                return self.return_cached_or_error("Network unavailable".to_string());
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            return self.return_cached_or_error(format!("GitHub API error: {}", status));
        }

        let commits: Vec<GitHubCommit> = match response.json().await {
            Ok(c) => c,
            Err(e) => {
                return self.return_cached_or_error(format!("Failed to parse response: {}", e));
            }
        };

        if commits.is_empty() {
            return UpdateCheckResult {
                has_update: false,
                current_commit,
                latest_commit: String::new(),
                commits_behind: 0,
                commits: vec![],
                branch,
                error: Some("No commits found".to_string()),
            };
        }

        let latest_commit = commits[0].sha[..7].to_string();
        let has_update = latest_commit != current_commit;

        // Count commits behind and collect commit info
        let mut commits_behind = 0;
        let mut commit_list = Vec::new();

        for commit in &commits {
            let short_hash = commit.sha[..7].to_string();
            commit_list.push(CommitInfo {
                hash: short_hash.clone(),
                message: commit.commit.message.lines().next().unwrap_or("").to_string(),
                author: commit.commit.author.name.clone(),
                date: commit.commit.author.date.clone(),
            });

            if short_hash == current_commit {
                break;
            }
            commits_behind += 1;
        }

        let result = UpdateCheckResult {
            has_update,
            current_commit,
            latest_commit,
            commits_behind,
            commits: commit_list,
            branch,
            error: None,
        };

        // Cache the result
        self.cache_update_info(&result);

        info!(
            "Update check complete: hasUpdate={}, behind={}",
            has_update, commits_behind
        );

        result
    }

    /// Return cached result if available, or an error result.
    fn return_cached_or_error(&self, error_msg: String) -> UpdateCheckResult {
        if let Some(cached) = self.get_cached_update_info() {
            debug!("Using cached update info (offline mode)");
            return cached.result;
        }

        UpdateCheckResult {
            has_update: false,
            current_commit: self.get_current_commit().unwrap_or_default(),
            latest_commit: String::new(),
            commits_behind: 0,
            commits: vec![],
            branch: self.get_current_branch(),
            error: Some(error_msg),
        }
    }

    /// Apply launcher update by pulling latest changes and rebuilding.
    pub async fn apply_update(&self) -> UpdateApplyResult {
        // Safety checks
        if !self.is_git_repo() {
            return UpdateApplyResult {
                success: false,
                message: None,
                new_commit: None,
                previous_commit: None,
                error: Some("Not a git repository".to_string()),
            };
        }

        if self.has_uncommitted_changes() {
            return UpdateApplyResult {
                success: false,
                message: None,
                new_commit: None,
                previous_commit: None,
                error: Some(
                    "Uncommitted changes detected. Please commit or stash them first.".to_string(),
                ),
            };
        }

        let current_commit = match self.get_current_commit() {
            Some(c) => c,
            None => {
                return UpdateApplyResult {
                    success: false,
                    message: None,
                    new_commit: None,
                    previous_commit: None,
                    error: Some("Unable to determine current commit".to_string()),
                };
            }
        };

        info!("Starting update from commit {}", current_commit);
        let branch = self.get_current_branch();

        // Step 1: Git pull
        info!("Running git pull");
        let pull_result = Command::new("git")
            .args(["pull", "origin", &branch])
            .current_dir(&self.launcher_root)
            .output();

        let pull_output = match pull_result {
            Ok(output) => output,
            Err(e) => {
                error!("Git pull failed: {}", e);
                return UpdateApplyResult {
                    success: false,
                    message: None,
                    new_commit: None,
                    previous_commit: Some(current_commit),
                    error: Some(format!("Git pull failed: {}", e)),
                };
            }
        };

        if !pull_output.status.success() {
            let stderr = String::from_utf8_lossy(&pull_output.stderr);
            error!("Git pull failed: {}", stderr);
            return UpdateApplyResult {
                success: false,
                message: None,
                new_commit: None,
                previous_commit: Some(current_commit),
                error: Some(format!("Git pull failed: {}", stderr)),
            };
        }

        let stdout = String::from_utf8_lossy(&pull_output.stdout);
        if stdout.contains("Already up to date") || stdout.contains("Already up-to-date") {
            info!("Already up to date");
            return UpdateApplyResult {
                success: true,
                message: Some("Already up to date".to_string()),
                new_commit: Some(current_commit.clone()),
                previous_commit: Some(current_commit),
                error: None,
            };
        }

        let new_commit = self.get_current_commit().unwrap_or_default();
        info!("Updated to commit {}", new_commit);

        // Step 2: Update Python dependencies
        info!("Updating Python dependencies");
        let pip_result = Command::new("pip")
            .args(["install", "-r", "requirements.txt", "--upgrade"])
            .current_dir(&self.launcher_root)
            .output();

        if let Err(e) = pip_result {
            warn!("pip install warning: {}", e);
            // Don't fail on pip warnings
        }

        // Step 3: Update frontend dependencies
        let frontend_dir = self.launcher_root.join("frontend");
        if frontend_dir.exists() {
            info!("Running npm install");
            let npm_result = Command::new("npm")
                .args(["install"])
                .current_dir(&frontend_dir)
                .output();

            if let Err(e) = npm_result {
                warn!("npm install warning: {}", e);
            }

            // Step 4: Rebuild frontend
            info!("Running npm build");
            let build_result = Command::new("npm")
                .args(["run", "build"])
                .current_dir(&frontend_dir)
                .output();

            match build_result {
                Ok(output) if !output.status.success() => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error!("Frontend build failed: {}", stderr);
                    // Rollback
                    self.rollback(&current_commit);
                    return UpdateApplyResult {
                        success: false,
                        message: None,
                        new_commit: None,
                        previous_commit: Some(current_commit.clone()),
                        error: Some(format!(
                            "Frontend build failed. Rolled back to {}.",
                            current_commit
                        )),
                    };
                }
                Err(e) => {
                    error!("Frontend build failed: {}", e);
                    self.rollback(&current_commit);
                    return UpdateApplyResult {
                        success: false,
                        message: None,
                        new_commit: None,
                        previous_commit: Some(current_commit.clone()),
                        error: Some(format!(
                            "Frontend build failed. Rolled back to {}.",
                            current_commit
                        )),
                    };
                }
                Ok(_) => {}
            }
        }

        info!("Update completed successfully");
        UpdateApplyResult {
            success: true,
            message: Some("Update applied successfully. Please restart the launcher.".to_string()),
            new_commit: Some(new_commit),
            previous_commit: Some(current_commit),
            error: None,
        }
    }

    /// Rollback to a previous commit.
    fn rollback(&self, commit_sha: &str) {
        warn!("Rolling back to commit {}", commit_sha);
        match Command::new("git")
            .args(["reset", "--hard", commit_sha])
            .current_dir(&self.launcher_root)
            .output()
        {
            Ok(output) if output.status.success() => {
                info!("Rollback successful");
            }
            Ok(output) => {
                error!(
                    "Rollback failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => {
                error!("Rollback failed: {}", e);
            }
        }
    }

    /// Restart the launcher by spawning a new process.
    ///
    /// This spawns the launcher script and then the current process should exit.
    /// Returns success if the new process was spawned.
    pub fn restart_launcher(&self) -> Result<bool> {
        // Find the launcher script
        let launcher_script = self.launcher_root.join("launcher");

        if !launcher_script.exists() {
            return Err(PumasError::NotFound {
                resource: "Launcher script".to_string(),
            });
        }

        info!("Spawning new launcher process");

        // Spawn the new process
        match Command::new(&launcher_script)
            .current_dir(&self.launcher_root)
            .spawn()
        {
            Ok(_child) => {
                info!("New launcher process spawned successfully");
                Ok(true)
            }
            Err(e) => {
                error!("Failed to spawn launcher: {}", e);
                Err(PumasError::Other(format!(
                    "Failed to restart launcher: {}",
                    e
                )))
            }
        }
    }
}

/// GitHub commit structure from API.
#[derive(Debug, Deserialize)]
struct GitHubCommit {
    sha: String,
    commit: GitHubCommitInfo,
}

#[derive(Debug, Deserialize)]
struct GitHubCommitInfo {
    message: String,
    author: GitHubAuthor,
}

#[derive(Debug, Deserialize)]
struct GitHubAuthor {
    name: String,
    date: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let updater = LauncherUpdater::new(temp_dir.path());

        // No .git directory
        assert!(!updater.is_git_repo());

        // Create .git directory
        std::fs::create_dir(temp_dir.path().join(".git")).unwrap();
        assert!(updater.is_git_repo());
    }

    #[test]
    fn test_get_version_info() {
        let temp_dir = TempDir::new().unwrap();
        let updater = LauncherUpdater::new(temp_dir.path());

        let info = updater.get_version_info();
        assert!(info["success"].as_bool().unwrap());
        assert!(info["version"].as_str().is_some());
    }
}
