//! Launcher auto-updater functionality.
//!
//! Manages launcher updates by comparing the packaged launcher version against
//! the latest GitHub release while still supporting git-based self-update flows
//! for developer checkouts.

use crate::error::{PumasError, Result};
use crate::models::CommitInfo;
use crate::network::{GitHubAsset, GitHubClient, GitHubRelease};
use chrono::{DateTime, Duration, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
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
    /// Current packaged launcher version.
    pub current_version: String,
    /// Latest available release version/tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    /// Latest release display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_name: Option<String>,
    /// GitHub release page URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    /// Best-matching direct download URL for the current platform.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// Release publish timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
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
#[derive(Clone)]
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

    fn current_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn collect_git_context(&self) -> (String, String, bool) {
        let current_commit = self.get_current_commit().unwrap_or_default();
        let is_git_repo = self.is_git_repo();
        let branch = if is_git_repo {
            self.get_current_branch()
        } else {
            String::new()
        };

        (current_commit, branch, is_git_repo)
    }

    fn corepack_command(&self) -> &'static str {
        if cfg!(windows) {
            "corepack.cmd"
        } else {
            "corepack"
        }
    }

    fn run_pnpm_command(&self, args: &[&str]) -> std::result::Result<(), String> {
        match Command::new(self.corepack_command())
            .args(pnpm_args(args))
            .current_dir(&self.launcher_root)
            .output()
        {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let details = if stderr.is_empty() { stdout } else { stderr };
                Err(if details.is_empty() {
                    format!("command exited with status {}", output.status)
                } else {
                    details
                })
            }
            Err(error) => Err(error.to_string()),
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
    async fn get_cached_update_info(&self) -> Option<CachedUpdateCheck> {
        match fs::read_to_string(&self.cache_file).await {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(cached) => Some(cached),
                Err(e) => {
                    debug!("Failed to parse cached update info: {}", e);
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                debug!("Failed to read cache file: {}", e);
                None
            }
        }
    }

    /// Save update check result to cache.
    async fn cache_update_info(&self, result: &UpdateCheckResult) {
        let cached = CachedUpdateCheck {
            last_checked: Utc::now(),
            result: result.clone(),
        };

        if let Some(parent) = self.cache_file.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                debug!("Failed to create cache directory: {}", e);
                return;
            }
        }

        match serde_json::to_string_pretty(&cached) {
            Ok(content) => {
                if let Err(e) = fs::write(&self.cache_file, content).await {
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
        let updater = self.clone();
        let (current_commit, branch, is_git_repo) =
            match tokio::task::spawn_blocking(move || updater.collect_git_context()).await {
                Ok(context) => context,
                Err(error) => {
                    return UpdateCheckResult {
                        has_update: false,
                        current_commit: String::new(),
                        latest_commit: String::new(),
                        commits_behind: 0,
                        commits: vec![],
                        branch: String::new(),
                        current_version: self.current_version(),
                        latest_version: None,
                        release_name: None,
                        release_url: None,
                        download_url: None,
                        published_at: None,
                        error: Some(format!(
                            "Failed to join launcher update context task: {}",
                            error
                        )),
                    };
                }
            };
        let current_version = self.current_version();

        // Check cache first (unless force refresh)
        if !force_refresh {
            if let Some(cached) = self.get_cached_update_info().await {
                // Cache valid for 1 hour
                if Utc::now() - cached.last_checked < Duration::hours(1) {
                    debug!("Using cached update info");
                    return cached.result;
                }
            }
        }

        let github_client = match GitHubClient::new(self.cache_dir()) {
            Ok(client) => client,
            Err(err) => {
                return self
                    .return_cached_or_error(
                        current_commit,
                        branch,
                        current_version,
                        format!("Failed to initialize GitHub client: {}", err),
                    )
                    .await;
            }
        };

        let repo = format!("{}/{}", self.repo_owner, self.repo_name);
        let release = match github_client.get_latest_release(&repo, force_refresh).await {
            Ok(Some(release)) => release,
            Ok(None) => {
                return self
                    .return_cached_or_error(
                        current_commit,
                        branch,
                        current_version,
                        "No GitHub releases found".to_string(),
                    )
                    .await;
            }
            Err(err) => {
                debug!("GitHub release check failed: {}", err);
                return self
                    .return_cached_or_error(
                        current_commit,
                        branch,
                        current_version,
                        format!("GitHub release check failed: {}", err),
                    )
                    .await;
            }
        };

        let latest_version = release.tag_name.clone();
        let has_update = is_newer_version(&current_version, &latest_version);
        let latest_commit = if is_git_repo {
            current_commit.clone()
        } else {
            String::new()
        };

        let result = UpdateCheckResult {
            has_update,
            current_commit,
            latest_commit,
            commits_behind: 0,
            commits: vec![],
            branch,
            current_version,
            latest_version: Some(latest_version),
            release_name: Some(release.name.clone()),
            release_url: Some(release.html_url.clone()),
            download_url: select_download_url(&release),
            published_at: Some(release.published_at.clone()),
            error: None,
        };

        // Cache the result
        self.cache_update_info(&result).await;

        info!(
            "Update check complete: hasUpdate={}, currentVersion={}, latestVersion={}",
            result.has_update,
            result.current_version,
            result.latest_version.as_deref().unwrap_or("unknown")
        );

        result
    }

    /// Return cached result if available, or an error result.
    async fn return_cached_or_error(
        &self,
        current_commit: String,
        branch: String,
        current_version: String,
        error_msg: String,
    ) -> UpdateCheckResult {
        if let Some(cached) = self.get_cached_update_info().await {
            debug!("Using cached update info (offline mode)");
            return cached.result;
        }

        UpdateCheckResult {
            has_update: false,
            current_commit,
            latest_commit: String::new(),
            commits_behind: 0,
            commits: vec![],
            branch,
            current_version,
            latest_version: None,
            release_name: None,
            release_url: None,
            download_url: None,
            published_at: None,
            error: Some(error_msg),
        }
    }

    fn cache_dir(&self) -> PathBuf {
        self.launcher_root.join("launcher-data").join("cache")
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

        // Step 3: Refresh workspace dependencies and rebuilt Node artifacts.
        if self.launcher_root.join("frontend").exists() {
            info!("Running pnpm install");
            if let Err(error_message) = self.run_pnpm_command(&["install", "--frozen-lockfile"]) {
                error!("pnpm install failed: {}", error_message);
                self.rollback(&current_commit);
                return UpdateApplyResult {
                    success: false,
                    message: None,
                    new_commit: None,
                    previous_commit: Some(current_commit.clone()),
                    error: Some(format!(
                        "Workspace install failed. Rolled back to {}.",
                        current_commit
                    )),
                };
            }

            info!("Building frontend");
            if let Err(error_message) =
                self.run_pnpm_command(&["--filter", "./frontend", "run", "build"])
            {
                error!("Frontend build failed: {}", error_message);
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

            info!("Building electron shell");
            if let Err(error_message) =
                self.run_pnpm_command(&["--filter", "./electron", "run", "build"])
            {
                error!("Electron build failed: {}", error_message);
                self.rollback(&current_commit);
                return UpdateApplyResult {
                    success: false,
                    message: None,
                    new_commit: None,
                    previous_commit: Some(current_commit.clone()),
                    error: Some(format!(
                        "Electron build failed. Rolled back to {}.",
                        current_commit
                    )),
                };
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

fn is_newer_version(current_version: &str, latest_version: &str) -> bool {
    let current = normalize_version(current_version);
    let latest = normalize_version(latest_version);

    match (Version::parse(&current), Version::parse(&latest)) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => latest != current,
    }
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches(['v', 'V']).to_string()
}

fn pnpm_args<'a>(args: &'a [&'a str]) -> Vec<&'a str> {
    let mut full_args = Vec::with_capacity(args.len() + 1);
    full_args.push("pnpm");
    full_args.extend_from_slice(args);
    full_args
}

fn select_download_url(release: &GitHubRelease) -> Option<String> {
    select_download_asset(&release.assets).map(|asset| asset.download_url.clone())
}

fn select_download_asset(assets: &[GitHubAsset]) -> Option<&GitHubAsset> {
    let preferred_patterns: &[&str] = match std::env::consts::OS {
        "linux" => &["appimage", "amd64.deb", ".deb", ".tar.gz", ".zip"],
        "windows" => &[".exe", ".msi", ".zip"],
        "macos" => &[".dmg", ".pkg", ".zip"],
        _ => &[".zip", ".tar.gz"],
    };

    preferred_patterns
        .iter()
        .find_map(|pattern| {
            assets
                .iter()
                .find(|asset| asset.name.to_ascii_lowercase().contains(pattern))
        })
        .or_else(|| assets.first())
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

    #[test]
    fn test_is_newer_version_handles_v_prefix() {
        assert!(is_newer_version("0.3.0", "v0.3.1"));
        assert!(!is_newer_version("0.3.1", "v0.3.1"));
    }

    #[test]
    fn test_select_download_asset_prefers_platform_installer() {
        let assets = vec![
            GitHubAsset {
                name: "pumas-library-electron_0.3.0_amd64.deb".into(),
                size: 1,
                download_url: "https://example.com/pumas.deb".into(),
                content_type: None,
            },
            GitHubAsset {
                name: "Pumas Library-0.3.0.AppImage".into(),
                size: 1,
                download_url: "https://example.com/pumas.appimage".into(),
                content_type: None,
            },
        ];

        let selected = select_download_asset(&assets).unwrap();

        if cfg!(target_os = "linux") {
            assert_eq!(selected.name, "Pumas Library-0.3.0.AppImage");
        } else {
            assert_eq!(selected.name, "pumas-library-electron_0.3.0_amd64.deb");
        }
    }

    #[test]
    fn test_pnpm_args_prefixes_corepack_subcommand() {
        assert_eq!(
            pnpm_args(&["install", "--frozen-lockfile"]),
            vec!["pnpm", "install", "--frozen-lockfile"]
        );
    }

    #[test]
    fn test_corepack_command_matches_platform() {
        let temp_dir = TempDir::new().unwrap();
        let updater = LauncherUpdater::new(temp_dir.path());

        if cfg!(windows) {
            assert_eq!(updater.corepack_command(), "corepack.cmd");
        } else {
            assert_eq!(updater.corepack_command(), "corepack");
        }
    }
}
