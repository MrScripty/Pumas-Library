//! Version management for ComfyUI, Ollama, and other supported applications.
//!
//! This module handles:
//! - Tracking installed, active, and default versions
//! - Installing new versions from GitHub releases
//! - Managing Python virtual environments and dependencies (ComfyUI)
//! - Installing pre-built binaries (Ollama)
//! - Launching application instances
//! - Installation progress tracking and cancellation
//!
//! # Architecture
//!
//! The version manager is organized into submodules:
//! - `state`: Version state tracking (active, installed, default)
//! - `installer`: Version installation with progress reporting
//! - `dependencies`: Python dependency management (uv/pip)
//! - `launcher`: Process launching with health checks
//! - `progress`: Installation progress tracking
//! - `constraints`: PyPI constraint resolution
//! - `ollama`: Ollama-specific binary installation
//!
//! # Example
//!
//! ```rust,ignore
//! use pumas_app_manager::VersionManager;
//! use pumas_library::AppId;
//!
//! #[tokio::main]
//! async fn main() -> pumas_library::Result<()> {
//!     let manager = VersionManager::new("/path/to/pumas", AppId::ComfyUI).await?;
//!
//!     // Get installed versions
//!     let installed = manager.get_installed_versions().await?;
//!     println!("Installed: {:?}", installed);
//!
//!     // Get active version
//!     if let Some(active) = manager.get_active_version().await? {
//!         println!("Active version: {}", active);
//!     }
//!
//!     Ok(())
//! }
//! ```

mod constraints;
mod dependencies;
mod installer;
mod launcher;
pub mod ollama;
mod progress;
pub mod size_calculator;
mod state;

pub use constraints::ConstraintsManager;
pub use dependencies::DependencyManager;
pub use installer::VersionInstaller;
pub use launcher::VersionLauncher;
pub use ollama::OllamaVersionManager;
pub use progress::{InstallationProgressTracker, PackageWeights, ProgressUpdate};
pub use size_calculator::{ReleaseSize, SizeBreakdown, SizeCalculator};
pub use state::VersionState;

use pumas_library::config::{AppId, PathsConfig};
use pumas_library::metadata::MetadataManager;
use pumas_library::models::InstallationProgress;
use pumas_library::network::GitHubClient;
use pumas_library::{PumasError, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, warn};

/// Main version manager coordinating all version operations.
pub struct VersionManager {
    /// Root directory for launcher data.
    launcher_root: PathBuf,
    /// Application ID (ComfyUI, Ollama, etc.).
    app_id: AppId,
    /// Metadata manager for JSON persistence.
    metadata_manager: Arc<MetadataManager>,
    /// GitHub client for fetching releases.
    github_client: Arc<GitHubClient>,
    /// Version state tracker.
    state: Arc<RwLock<VersionState>>,
    /// Installation progress tracker.
    progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
    /// Cancellation flag for installations.
    cancel_flag: Arc<AtomicBool>,
    /// Lock for serializing installations.
    install_lock: Arc<Mutex<()>>,
    /// Currently installing tag (exclusive access only).
    installing_tag: Arc<Mutex<Option<String>>>,
}

impl VersionManager {
    /// Create a new version manager.
    ///
    /// # Arguments
    ///
    /// * `launcher_root` - Path to the launcher root directory
    /// * `app_id` - The application to manage versions for
    pub async fn new(launcher_root: impl Into<PathBuf>, app_id: AppId) -> Result<Self> {
        let launcher_root = launcher_root.into();

        if !launcher_root.exists() {
            return Err(PumasError::Config {
                message: format!("Launcher root does not exist: {}", launcher_root.display()),
            });
        }

        // Initialize components
        let metadata_manager = Arc::new(MetadataManager::new(&launcher_root));
        metadata_manager.ensure_directories()?;

        let cache_dir = launcher_root
            .join("launcher-data")
            .join(PathsConfig::CACHE_DIR_NAME);
        let github_client = Arc::new(GitHubClient::new(cache_dir.clone())?);

        let progress_tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
            cache_dir.clone(),
        )));

        // Initialize state
        let state = Arc::new(RwLock::new(
            VersionState::new(&launcher_root, app_id, metadata_manager.clone()).await?,
        ));

        // Validate installed versions exist on disk (removes stale entries)
        {
            let mut state_guard = state.write().await;
            match state_guard.validate_installations() {
                Ok(validation) => {
                    if !validation.removed_tags.is_empty() {
                        info!(
                            "Removed {} stale version entries for {:?}",
                            validation.removed_tags.len(),
                            app_id
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to validate installations for {:?}: {}", app_id, e);
                }
            }
        }

        Ok(Self {
            launcher_root,
            app_id,
            metadata_manager,
            github_client,
            state,
            progress_tracker,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            install_lock: Arc::new(Mutex::new(())),
            installing_tag: Arc::new(Mutex::new(None)),
        })
    }

    // ========================================
    // Path helpers
    // ========================================

    /// Get the versions directory for this app.
    pub fn versions_dir(&self) -> PathBuf {
        self.launcher_root.join(self.app_id.versions_dir_name())
    }

    /// Get the logs directory.
    pub fn logs_dir(&self) -> PathBuf {
        self.launcher_root
            .join("launcher-data")
            .join(PathsConfig::LOGS_DIR_NAME)
    }

    /// Get the cache directory.
    pub fn cache_dir(&self) -> PathBuf {
        self.launcher_root
            .join("launcher-data")
            .join(PathsConfig::CACHE_DIR_NAME)
    }

    /// Get the pip cache directory.
    pub fn pip_cache_dir(&self) -> PathBuf {
        self.cache_dir().join(PathsConfig::PIP_CACHE_DIR_NAME)
    }

    /// Get the constraints directory.
    pub fn constraints_dir(&self) -> PathBuf {
        self.cache_dir().join(PathsConfig::CONSTRAINTS_DIR_NAME)
    }

    /// Get the path to a specific version.
    pub fn version_path(&self, tag: &str) -> PathBuf {
        self.versions_dir().join(tag)
    }

    /// Get the active version file path.
    pub fn active_version_file(&self) -> PathBuf {
        self.launcher_root.join(".active-version")
    }

    // ========================================
    // State queries (delegated to VersionState)
    // ========================================

    /// Get list of installed version tags.
    pub async fn get_installed_versions(&self) -> Result<Vec<String>> {
        let state = self.state.read().await;
        Ok(state.get_installed_tags())
    }

    /// Get the currently active version tag.
    pub async fn get_active_version(&self) -> Result<Option<String>> {
        let state = self.state.read().await;
        Ok(state.get_active_version())
    }

    /// Get the default version tag.
    pub async fn get_default_version(&self) -> Result<Option<String>> {
        let state = self.state.read().await;
        Ok(state.get_default_version())
    }

    /// Set the active version.
    pub async fn set_active_version(&self, tag: &str) -> Result<bool> {
        let mut state = self.state.write().await;
        state.set_active_version(tag)
    }

    /// Set the default version.
    pub async fn set_default_version(&self, tag: Option<&str>) -> Result<bool> {
        let mut state = self.state.write().await;
        state.set_default_version(tag)
    }

    /// Get detailed version info for a specific tag.
    pub async fn get_version_info(
        &self,
        tag: &str,
    ) -> Result<Option<pumas_library::metadata::InstalledVersionMetadata>> {
        self.metadata_manager
            .get_installed_version(tag, Some(self.app_id))
    }

    /// Get combined version status (all versions with their states).
    pub async fn get_version_status(&self) -> Result<VersionStatusReport> {
        let state = self.state.read().await;
        let installed = state.get_installed_tags();
        let active = state.get_active_version();
        let default = state.get_default_version();

        let mut versions = Vec::new();
        for tag in &installed {
            let is_active = active.as_deref() == Some(tag);
            let is_default = default.as_deref() == Some(tag);
            let info = self.metadata_manager.get_installed_version(tag, Some(self.app_id))?;

            versions.push(VersionStatusEntry {
                tag: tag.clone(),
                is_active,
                is_default,
                installed_date: info.as_ref().map(|i| i.installed_date.clone()),
                python_version: info.as_ref().and_then(|i| i.python_version.clone()),
                dependencies_installed: info.as_ref().and_then(|i| i.dependencies_installed),
            });
        }

        Ok(VersionStatusReport {
            versions,
            active_version: active,
            default_version: default,
        })
    }

    /// Validate all installations and remove incomplete ones.
    pub async fn validate_installations(&self) -> Result<ValidationResult> {
        let mut state = self.state.write().await;
        state.validate_installations()
    }

    // ========================================
    // GitHub releases
    // ========================================

    /// Get available releases from GitHub.
    pub async fn get_available_releases(
        &self,
        force_refresh: bool,
    ) -> Result<Vec<pumas_library::network::GitHubRelease>> {
        self.github_client
            .get_releases_for_app(self.app_id, force_refresh)
            .await
    }

    /// Get a specific release by tag.
    pub async fn get_release_by_tag(
        &self,
        tag: &str,
        force_refresh: bool,
    ) -> Result<Option<pumas_library::network::GitHubRelease>> {
        self.github_client
            .get_release_by_tag(self.app_id.github_repo(), tag, force_refresh)
            .await
    }

    /// Get cache status for GitHub releases.
    pub fn get_github_cache_status(&self) -> pumas_library::models::CacheStatus {
        self.github_client
            .get_cache_status(self.app_id.github_repo())
    }

    // ========================================
    // Installation operations
    // ========================================

    /// Check if a version is currently being installed.
    pub async fn is_installing(&self) -> bool {
        self.installing_tag.lock().await.is_some()
    }

    /// Get the tag of the version currently being installed.
    pub async fn get_installing_tag(&self) -> Option<String> {
        self.installing_tag.lock().await.clone()
    }

    /// Get current installation progress.
    pub async fn get_installation_progress(&self) -> Option<InstallationProgress> {
        let tracker = self.progress_tracker.read().await;
        tracker.get_current_state()
    }

    /// Cancel the current installation.
    pub async fn cancel_installation(&self) -> Result<bool> {
        if !self.is_installing().await {
            return Ok(false);
        }

        info!("Cancelling installation");
        self.cancel_flag.store(true, Ordering::SeqCst);

        // Update progress tracker
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.set_error("Installation cancelled by user");
        }

        Ok(true)
    }

    /// Install a version with progress channel.
    ///
    /// Returns a channel receiver for progress updates.
    pub async fn install_version(
        &self,
        tag: &str,
    ) -> Result<mpsc::Receiver<ProgressUpdate>> {
        // Check if already installed
        {
            let state = self.state.read().await;
            if state.is_installed(tag) {
                return Err(PumasError::VersionAlreadyInstalled {
                    tag: tag.to_string(),
                });
            }
        }

        // Acquire install lock
        let _lock = self.install_lock.lock().await;

        // Reset cancellation flag
        self.cancel_flag.store(false, Ordering::SeqCst);

        // Set installing tag
        {
            let mut installing = self.installing_tag.lock().await;
            *installing = Some(tag.to_string());
        }

        // Create progress channel
        let (tx, rx) = mpsc::channel(32);

        // Get release info
        let release = self
            .get_release_by_tag(tag, false)
            .await?
            .ok_or_else(|| PumasError::VersionNotFound {
                tag: tag.to_string(),
            })?;

        // Create installer
        let installer = VersionInstaller::new(
            self.launcher_root.clone(),
            self.app_id,
            self.metadata_manager.clone(),
            self.progress_tracker.clone(),
            self.cancel_flag.clone(),
        );

        // Spawn installation task
        let tag = tag.to_string();
        let state = self.state.clone();
        let installing_tag = self.installing_tag.clone();
        let progress_tracker = self.progress_tracker.clone();

        tokio::spawn(async move {
            let result = installer.install_version(&tag, &release, tx.clone()).await;

            // Clear installing tag
            {
                let mut installing = installing_tag.lock().await;
                *installing = None;
            }

            // Update state on success
            if result.is_ok() {
                let mut state_guard = state.write().await;
                if let Err(e) = state_guard.refresh() {
                    warn!("Failed to refresh state after installation: {}", e);
                }
            }

            // Send final status
            let _ = tx.send(match result {
                Ok(_) => ProgressUpdate::Completed { success: true },
                Err(e) => ProgressUpdate::Error {
                    message: e.to_string(),
                },
            }).await;

            // Schedule progress state cleanup after frontend has time to poll final status
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let mut tracker = progress_tracker.write().await;
                tracker.clear_completed_state();
            });
        });

        Ok(rx)
    }

    /// Remove an installed version.
    pub async fn remove_version(&self, tag: &str) -> Result<bool> {
        // Check if installed
        {
            let state = self.state.read().await;
            if !state.is_installed(tag) {
                return Err(PumasError::VersionNotFound {
                    tag: tag.to_string(),
                });
            }
        }

        // Check if active
        {
            let state = self.state.read().await;
            if state.get_active_version().as_deref() == Some(tag) {
                return Err(PumasError::Other(
                    "Cannot remove the currently active version".to_string(),
                ));
            }
        }

        // Remove directory
        let version_path = self.version_path(tag);
        if version_path.exists() {
            info!("Removing version directory: {}", version_path.display());
            std::fs::remove_dir_all(&version_path).map_err(|e| PumasError::Io {
                message: format!("Failed to remove version directory: {}", e),
                path: Some(version_path),
                source: Some(e),
            })?;
        }

        // Remove from metadata
        self.metadata_manager
            .remove_installed_version(tag, Some(self.app_id))?;

        // Refresh state
        {
            let mut state = self.state.write().await;
            state.refresh()?;
        }

        info!("Removed version: {}", tag);
        Ok(true)
    }

    // ========================================
    // Dependency operations
    // ========================================

    /// Check dependencies for a version.
    pub async fn check_dependencies(&self, tag: &str) -> Result<pumas_library::models::DependencyStatus> {
        let dep_manager = DependencyManager::new(
            self.launcher_root.clone(),
            self.app_id,
            self.pip_cache_dir(),
        );
        dep_manager.check_dependencies(tag).await
    }

    /// Install dependencies for a version.
    pub async fn install_dependencies(
        &self,
        tag: &str,
        progress_tx: Option<mpsc::Sender<ProgressUpdate>>,
    ) -> Result<bool> {
        let dep_manager = DependencyManager::new(
            self.launcher_root.clone(),
            self.app_id,
            self.pip_cache_dir(),
        );

        let constraints_manager = ConstraintsManager::new(self.constraints_dir());

        dep_manager
            .install_dependencies(tag, &constraints_manager, progress_tx)
            .await
    }

    // ========================================
    // Launch operations
    // ========================================

    /// Launch a version.
    pub async fn launch_version(
        &self,
        tag: &str,
        extra_args: Option<Vec<String>>,
    ) -> Result<LaunchResult> {
        // Ensure version is active
        self.set_active_version(tag).await?;

        // Check dependencies
        let deps = self.check_dependencies(tag).await?;
        if !deps.missing.is_empty() {
            warn!(
                "Missing dependencies for {}: {:?}",
                tag, deps.missing
            );
            // Install missing deps
            self.install_dependencies(tag, None).await?;
        }

        // Create launcher
        let launcher = VersionLauncher::new(
            self.launcher_root.clone(),
            self.app_id,
            self.logs_dir(),
        );

        launcher.launch_version(tag, extra_args).await
    }
}

/// Status report for all versions.
#[derive(Debug, Clone)]
pub struct VersionStatusReport {
    pub versions: Vec<VersionStatusEntry>,
    pub active_version: Option<String>,
    pub default_version: Option<String>,
}

/// Status entry for a single version.
#[derive(Debug, Clone)]
pub struct VersionStatusEntry {
    pub tag: String,
    pub is_active: bool,
    pub is_default: bool,
    pub installed_date: Option<String>,
    pub python_version: Option<String>,
    pub dependencies_installed: Option<bool>,
}

/// Result of version validation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationResult {
    pub removed_tags: Vec<String>,
    pub orphaned_dirs: Vec<PathBuf>,
    pub valid_count: usize,
}

/// Result of launching a version.
#[derive(Debug)]
pub struct LaunchResult {
    pub success: bool,
    pub log_file: Option<PathBuf>,
    pub error: Option<String>,
    pub ready: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_manager() -> (VersionManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create required directories
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("comfyui-versions")).unwrap();

        let manager = VersionManager::new(temp_dir.path(), AppId::ComfyUI)
            .await
            .unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let (manager, _temp) = create_test_manager().await;
        assert!(manager.versions_dir().ends_with("comfyui-versions"));
    }

    #[tokio::test]
    async fn test_get_installed_versions_empty() {
        let (manager, _temp) = create_test_manager().await;
        let installed = manager.get_installed_versions().await.unwrap();
        assert!(installed.is_empty());
    }

    #[tokio::test]
    async fn test_get_active_version_none() {
        let (manager, _temp) = create_test_manager().await;
        let active = manager.get_active_version().await.unwrap();
        assert!(active.is_none());
    }

    #[tokio::test]
    async fn test_path_helpers() {
        let (manager, temp) = create_test_manager().await;

        assert_eq!(
            manager.versions_dir(),
            temp.path().join("comfyui-versions")
        );
        assert_eq!(
            manager.logs_dir(),
            temp.path().join("launcher-data/logs")
        );
        assert_eq!(
            manager.cache_dir(),
            temp.path().join("launcher-data/cache")
        );
        assert_eq!(
            manager.version_path("v1.0.0"),
            temp.path().join("comfyui-versions/v1.0.0")
        );
    }
}
