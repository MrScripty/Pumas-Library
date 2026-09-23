//! Version management for supported inference runtimes.
//!
//! This module handles:
//! - Tracking installed, active, and default versions
//! - Installing new versions from GitHub releases
//! - Managing Python virtual environments and dependencies
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
//!     let manager = VersionManager::new("/path/to/pumas", AppId::Ollama).await?;
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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, warn};

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

fn active_version_path(root: &Path, app_id: AppId) -> PathBuf {
    // Preserve the established native-runtime marker; Torch must not overwrite
    // llama.cpp selection when a user activates an image runtime.
    root.join(if app_id == AppId::Torch {
        ".active-version-torch"
    } else {
        ".active-version"
    })
}

/// Main version manager coordinating all version operations.
#[derive(Clone)]
pub struct VersionManager {
    /// Root directory for launcher data.
    launcher_root: PathBuf,
    /// Inference runtime identifier.
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
    async fn get_installed_version_metadata(
        &self,
        tag: &str,
    ) -> Result<Option<pumas_library::metadata::InstalledVersionMetadata>> {
        let metadata_manager = self.metadata_manager.clone();
        let tag = tag.to_string();
        let app_id = self.app_id;
        tokio::task::spawn_blocking(move || {
            metadata_manager.get_installed_version(&tag, Some(app_id))
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join installed version metadata task: {}",
                err
            ))
        })?
    }

    /// Create a new version manager.
    ///
    /// # Arguments
    ///
    /// * `launcher_root` - Path to the launcher root directory
    /// * `app_id` - The application to manage versions for
    pub async fn new(launcher_root: impl Into<PathBuf>, app_id: AppId) -> Result<Self> {
        let launcher_root = launcher_root.into();

        if !app_id.has_version_manager() {
            return Err(PumasError::Config {
                message: format!(
                    "{} is provided in-process and does not use the version manager",
                    app_id
                ),
            });
        }

        let launcher_root = pumas_library::platform::paths::absolute_launcher_root(&launcher_root)?;

        if !path_exists(&launcher_root).await? {
            return Err(PumasError::Config {
                message: format!("Launcher root does not exist: {}", launcher_root.display()),
            });
        }

        let cache_dir = launcher_root
            .join("launcher-data")
            .join(PathsConfig::CACHE_DIR_NAME);
        let launcher_root_for_setup = launcher_root.clone();
        let cache_dir_for_setup = cache_dir.clone();
        let (metadata_manager, github_client) = tokio::task::spawn_blocking(move || {
            let metadata_manager = Arc::new(MetadataManager::new(&launcher_root_for_setup));
            metadata_manager.ensure_directories()?;
            let github_client = Arc::new(GitHubClient::new(cache_dir_for_setup)?);
            Ok::<_, PumasError>((metadata_manager, github_client))
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join version manager startup initialization task: {}",
                err
            ))
        })??;

        let progress_tracker = Arc::new(RwLock::new(
            InstallationProgressTracker::new_with_stale_cleanup(cache_dir.clone()).await,
        ));

        // Initialize state
        let state = Arc::new(RwLock::new(
            VersionState::new(&launcher_root, app_id, metadata_manager.clone()).await?,
        ));

        // Validate installed versions exist on disk (removes stale entries)
        {
            let mut state_guard = state.write().await;
            match state_guard.validate_installations().await {
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
        active_version_path(&self.launcher_root, self.app_id)
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
        if self.app_id == AppId::Torch {
            let current = self.get_active_version().await?;
            if current.as_deref() != Some(tag) {
                if let Some(current) = current {
                    self.ensure_torch_stopped(&current).await?;
                }
            }
        }
        let mut state = self.state.write().await;
        state.set_active_version(tag).await
    }

    /// Set the default version.
    pub async fn set_default_version(&self, tag: Option<&str>) -> Result<bool> {
        let mut state = self.state.write().await;
        state.set_default_version(tag).await
    }

    /// Get detailed version info for a specific tag.
    pub async fn get_version_info(
        &self,
        tag: &str,
    ) -> Result<Option<pumas_library::metadata::InstalledVersionMetadata>> {
        self.get_installed_version_metadata(tag).await
    }

    /// Get combined version status (all versions with their states).
    pub async fn get_version_status(&self) -> Result<VersionStatusReport> {
        let (installed, active, default) = {
            let state = self.state.read().await;
            (
                state.get_installed_tags(),
                state.get_active_version(),
                state.get_default_version(),
            )
        };

        let mut versions = Vec::new();
        for tag in &installed {
            let is_active = active.as_deref() == Some(tag);
            let is_default = default.as_deref() == Some(tag);
            let info = self.get_installed_version_metadata(tag).await?;

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
        state.validate_installations().await
    }

    // ========================================
    // GitHub releases
    // ========================================

    /// Get available releases from GitHub.
    pub async fn get_available_releases(
        &self,
        force_refresh: bool,
    ) -> Result<Vec<pumas_library::network::GitHubRelease>> {
        let mut releases = self
            .github_client
            .get_releases_for_app(self.app_id, force_refresh)
            .await?;
        if self.app_id == AppId::Torch {
            releases.retain(installer::is_torch_runtime_release);
        }
        Ok(releases)
    }

    /// Get a specific release by tag.
    pub async fn get_release_by_tag(
        &self,
        tag: &str,
        force_refresh: bool,
    ) -> Result<Option<pumas_library::network::GitHubRelease>> {
        let release = self
            .github_client
            .get_release_by_tag(self.app_id.github_repo(), tag, force_refresh)
            .await?;
        Ok(release.filter(|release| {
            self.app_id != AppId::Torch || installer::is_torch_runtime_release(release)
        }))
    }

    /// Get cache status for GitHub releases.
    pub async fn get_github_cache_status(&self) -> pumas_library::models::CacheStatus {
        self.github_client
            .get_cache_status(self.app_id.github_repo())
            .await
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
    pub async fn install_version(&self, tag: &str) -> Result<mpsc::Receiver<ProgressUpdate>> {
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
        let install_guard = self.install_lock.clone().lock_owned().await;
        // Resolve before recording installation state: discovery failure must not
        // leave a phantom installation that can never complete.
        let release = self.resolve_installable_release(tag).await?;
        if self.state.read().await.is_installed(tag) {
            return Err(PumasError::VersionAlreadyInstalled {
                tag: tag.to_string(),
            });
        }

        // Reset cancellation flag
        self.cancel_flag.store(false, Ordering::SeqCst);

        // Set installing tag
        {
            let mut installing = self.installing_tag.lock().await;
            *installing = Some(tag.to_string());
        }

        // Create progress channel
        let (tx, rx) = mpsc::channel(32);

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
            let _install_guard = install_guard;
            let result = installer.install_version(&tag, &release, tx.clone()).await;

            // Clear installing tag
            {
                let mut installing = installing_tag.lock().await;
                *installing = None;
            }

            // Update state on success
            if result.is_ok() {
                let mut state_guard = state.write().await;
                if let Err(e) = state_guard.refresh().await {
                    warn!("Failed to refresh state after installation: {}", e);
                }
            }

            // Send final status
            let _ = tx
                .send(match result {
                    Ok(_) => ProgressUpdate::Completed { success: true },
                    Err(e) => ProgressUpdate::Error {
                        message: e.to_string(),
                    },
                })
                .await;

            // Schedule progress state cleanup after frontend has time to poll final status
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let mut tracker = progress_tracker.write().await;
                tracker.clear_completed_state_async().await;
            });
        });

        Ok(rx)
    }

    async fn resolve_installable_release(
        &self,
        tag: &str,
    ) -> Result<pumas_library::network::GitHubRelease> {
        if self.app_id == AppId::LlamaCpp {
            return self
                .get_available_releases(false)
                .await?
                .into_iter()
                .find(|release| release.tag_name == tag)
                .ok_or_else(|| PumasError::VersionNotFound {
                    tag: tag.to_string(),
                });
        }

        self.get_release_by_tag(tag, false)
            .await?
            .ok_or_else(|| PumasError::VersionNotFound {
                tag: tag.to_string(),
            })
    }

    async fn ensure_torch_stopped(&self, tag: &str) -> Result<()> {
        if self.app_id != AppId::Torch {
            return Ok(());
        }
        let mut pid_paths = vec![self.version_path(tag).join("torch.pid")];
        let profiles = self
            .launcher_root
            .join("launcher-data/runtime-profiles/torch");
        match fs::read_dir(&profiles).await {
            Ok(mut entries) => {
                while let Some(entry) = entries
                    .next_entry()
                    .await
                    .map_err(|error| PumasError::io_with_path(error, &profiles))?
                {
                    pid_paths.push(entry.path().join("runtime.pid"));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(PumasError::io_with_path(error, profiles)),
        }
        // Profiles share the selected environment. Require them to stop before
        // changing runtime selection or deleting files used by a profile.
        for pid_path in pid_paths {
            match fs::read_to_string(&pid_path).await {
                Ok(pid) => {
                    let pid = pid.trim().parse::<u32>().map_err(|_| {
                        PumasError::Other(
                            "Cannot verify Torch process: invalid PID file".to_string(),
                        )
                    })?;
                    if pumas_library::platform::is_process_alive(pid) {
                        return Err(PumasError::Other(
                            "Stop Torch before switching or removing its runtime".to_string(),
                        ));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(PumasError::io_with_path(error, pid_path)),
            }
        }
        Ok(())
    }

    /// Remove an installed version.
    pub async fn remove_version(&self, tag: &str) -> Result<bool> {
        let _install_guard = self.install_lock.lock().await;
        self.ensure_torch_stopped(tag).await?;
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
        if path_exists(&version_path).await? {
            info!("Removing version directory: {}", version_path.display());
            fs::remove_dir_all(&version_path)
                .await
                .map_err(|e| PumasError::Io {
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
            state.refresh().await?;
        }

        info!("Removed version: {}", tag);
        Ok(true)
    }

    // ========================================
    // Dependency operations
    // ========================================

    /// Check dependencies for a version.
    pub async fn check_dependencies(
        &self,
        tag: &str,
    ) -> Result<pumas_library::models::DependencyStatus> {
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

        let constraints_manager = ConstraintsManager::new_with_cache(self.constraints_dir()).await;

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
            warn!("Missing dependencies for {}: {:?}", tag, deps.missing);
            // Install missing deps
            self.install_dependencies(tag, None).await?;
        }

        // Create launcher
        let launcher =
            VersionLauncher::new(self.launcher_root.clone(), self.app_id, self.logs_dir());

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
    use pumas_library::network::{GitHubAsset, GitHubRelease, ReleasesCache};
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const TORCH_ARCHIVE_NAME: &str = "pumas-torch-runtime-linux-x86_64.tar.gz";
    const TORCH_CHECKSUM_NAME: &str = "pumas-torch-runtime-linux-x86_64.tar.gz.sha256";

    fn torch_bundle(tag: &str) -> Vec<u8> {
        let recipe = format!(
            r#"{{"recipe_id":"{tag}","protocol":3,"capabilities":["image_generation"],"python":"3.12","platform":"linux-x86_64"}}"#
        );
        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        for (name, contents) in [
            ("runtime.json", recipe.as_str()),
            ("serve.py", ""),
            ("requirements.txt", "--no-index\n"),
            ("validate_runtime.py", ""),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(&mut header, name, contents.as_bytes())
                .unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    fn torch_release(tag: &str, base_url: &str, archive: &[u8]) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag.to_string(),
            name: tag.to_string(),
            published_at: "2026-01-01T00:00:00Z".to_string(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: [TORCH_ARCHIVE_NAME, TORCH_CHECKSUM_NAME]
                .into_iter()
                .map(|name| GitHubAsset {
                    name: name.to_string(),
                    size: if name == TORCH_ARCHIVE_NAME {
                        archive.len() as u64
                    } else {
                        64
                    },
                    download_url: format!("{base_url}/{tag}/{name}"),
                    content_type: None,
                })
                .collect(),
            html_url: format!("{base_url}/{tag}"),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
        }
    }

    async fn serve_torch_assets(
        assets: std::collections::HashMap<String, Vec<u8>>,
        request_count: usize,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for _ in 0..request_count {
                let (mut stream, _) =
                    tokio::time::timeout(Duration::from_secs(10), listener.accept())
                        .await
                        .expect("timed out waiting for a local runtime asset request")
                        .unwrap();
                let mut request = Vec::new();
                tokio::time::timeout(Duration::from_secs(10), async {
                    let mut buffer = [0; 1024];
                    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let count = stream.read(&mut buffer).await.unwrap();
                        assert_ne!(count, 0, "fixture request ended before headers");
                        request.extend_from_slice(&buffer[..count]);
                        assert!(request.len() <= 8192, "fixture request headers too large");
                    }
                })
                .await
                .expect("timed out reading local runtime asset request");
                let request = String::from_utf8(request).unwrap();
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .expect("fixture request has a request path");
                let body = assets
                    .get(path)
                    .unwrap_or_else(|| panic!("unexpected local asset request: {path}"));
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(header.as_bytes()).await.unwrap();
                stream.write_all(body).await.unwrap();
            }
        });
        (base_url, server)
    }

    async fn install_and_drain(manager: &VersionManager, tag: &str) {
        let mut progress = manager.install_version(tag).await.unwrap();
        let terminal = tokio::time::timeout(Duration::from_secs(60), async {
            while let Some(update) = progress.recv().await {
                match update {
                    ProgressUpdate::Completed { success } => return success,
                    ProgressUpdate::Error { message } => panic!("install {tag} failed: {message}"),
                    _ => {}
                }
            }
            panic!("install {tag} progress channel closed before completion");
        })
        .await
        .unwrap_or_else(|_| panic!("install {tag} did not reach terminal progress"));
        assert!(terminal, "install {tag} completed unsuccessfully");
        assert!(!manager.is_installing().await);
        assert!(manager.get_installing_tag().await.is_none());
    }

    async fn create_test_manager() -> (VersionManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create required directories
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("ollama-versions")).unwrap();

        let manager = VersionManager::new(temp_dir.path(), AppId::Ollama)
            .await
            .unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn torch_release_rejection_does_not_leave_an_install_in_progress() {
        let root = TempDir::new().unwrap();
        let cache = root.path().join("launcher-data/cache");
        let releases = pumas_library::network::ReleasesCache::new(cache, Duration::from_secs(3600));
        releases.set_disk(AppId::Torch.github_repo(), &[]).unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert!(matches!(
            manager.install_version("v2.10.0").await,
            Err(PumasError::VersionNotFound { .. })
        ));
        assert!(!manager.is_installing().await);
        assert!(manager.get_installation_progress().await.is_none());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn torch_runtime_update_lifecycle_uses_cached_releases_and_scoped_active_marker() {
        let root = TempDir::new().unwrap();
        let llama_marker = root.path().join(".active-version");
        std::fs::write(&llama_marker, "llama-cpp-fixture\n").unwrap();

        let older_tag = "torch-runtime-0.1.0";
        let newer_tag = "torch-runtime-0.2.0";
        let older_archive = torch_bundle(older_tag);
        let newer_archive = torch_bundle(newer_tag);
        let older_checksum = format!(
            "{:x}  {TORCH_ARCHIVE_NAME}\n",
            Sha256::digest(&older_archive)
        );
        let newer_checksum = format!(
            "{:x}  {TORCH_ARCHIVE_NAME}\n",
            Sha256::digest(&newer_archive)
        );
        let mut assets = std::collections::HashMap::new();
        for (tag, archive, checksum) in [
            (
                older_tag,
                older_archive.as_slice(),
                older_checksum.as_bytes(),
            ),
            (
                newer_tag,
                newer_archive.as_slice(),
                newer_checksum.as_bytes(),
            ),
        ] {
            assets.insert(format!("/{tag}/{TORCH_ARCHIVE_NAME}"), archive.to_vec());
            assets.insert(format!("/{tag}/{TORCH_CHECKSUM_NAME}"), checksum.to_vec());
        }
        let (base_url, server) = serve_torch_assets(assets, 4).await;
        let releases = vec![
            torch_release(older_tag, &base_url, &older_archive),
            torch_release(newer_tag, &base_url, &newer_archive),
            torch_release("legacy-pytorch-9.9.0", &base_url, &older_archive),
        ];
        let cache = ReleasesCache::new(
            root.path().join("launcher-data/cache"),
            Duration::from_secs(3600),
        );
        cache
            .set_disk(AppId::Torch.github_repo(), &releases)
            .unwrap();

        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        let discovered = manager.get_available_releases(false).await.unwrap();
        assert_eq!(
            discovered
                .iter()
                .map(|release| release.tag_name.as_str())
                .collect::<Vec<_>>(),
            vec![older_tag, newer_tag]
        );
        assert_eq!(manager.get_active_version().await.unwrap(), None);
        assert_eq!(manager.get_default_version().await.unwrap(), None);
        assert!(!manager.active_version_file().exists());

        for tag in [older_tag, newer_tag] {
            install_and_drain(&manager, tag).await;
            let runtime = manager.version_path(tag);
            assert!(runtime.join("runtime.json").is_file());
            assert!(runtime.join("venv/bin/python").is_file());
        }
        server.await.unwrap();

        let installed = manager.get_installed_versions().await.unwrap();
        assert_eq!(installed.len(), 2);
        assert!(installed.iter().any(|tag| tag == older_tag));
        assert!(installed.iter().any(|tag| tag == newer_tag));
        assert_eq!(manager.get_default_version().await.unwrap(), None);

        assert!(manager.set_active_version(newer_tag).await.unwrap());
        assert_eq!(
            manager.get_active_version().await.unwrap().as_deref(),
            Some(newer_tag)
        );
        assert_eq!(
            std::fs::read_to_string(manager.active_version_file()).unwrap(),
            newer_tag
        );
        assert_eq!(
            std::fs::read_to_string(&llama_marker).unwrap(),
            "llama-cpp-fixture\n"
        );

        let active_remove = manager.remove_version(newer_tag).await.unwrap_err();
        assert!(active_remove
            .to_string()
            .contains("Cannot remove the currently active version"));
        assert!(manager.version_path(newer_tag).exists());
        assert!(manager.remove_version(older_tag).await.unwrap());
        assert_eq!(
            manager.get_installed_versions().await.unwrap(),
            vec![newer_tag.to_string()]
        );
        assert!(!manager.version_path(older_tag).exists());
        assert!(manager.version_path(newer_tag).exists());
        assert_eq!(
            manager.get_active_version().await.unwrap().as_deref(),
            Some(newer_tag)
        );
        assert_eq!(manager.get_default_version().await.unwrap(), None);
        assert_eq!(
            std::fs::read_to_string(&llama_marker).unwrap(),
            "llama-cpp-fixture\n"
        );
    }

    #[tokio::test]
    async fn running_torch_environment_cannot_be_removed() {
        let root = TempDir::new().unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        let runtime = manager.version_path("torch-runtime-running");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(runtime.join("torch.pid"), std::process::id().to_string()).unwrap();
        let error = manager
            .remove_version("torch-runtime-running")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Stop Torch"));
        assert!(runtime.join("torch.pid").exists());
    }

    #[tokio::test]
    async fn managed_torch_profile_blocks_runtime_removal() {
        let root = TempDir::new().unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        let profile = root
            .path()
            .join("launcher-data/runtime-profiles/torch/image-profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join("runtime.pid"), std::process::id().to_string()).unwrap();
        let error = manager
            .remove_version("torch-runtime-profile")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Stop Torch"));
        assert!(profile.join("runtime.pid").exists());
    }

    #[tokio::test]
    async fn relative_launcher_root_is_captured_before_manager_paths() {
        let cwd = std::env::current_dir().unwrap();
        let temp = tempfile::tempdir_in(&cwd).unwrap();
        let relative = temp.path().strip_prefix(&cwd).unwrap();
        let manager = VersionManager::new(relative, AppId::LlamaCpp)
            .await
            .unwrap();
        assert_eq!(manager.launcher_root, temp.path());
        assert_eq!(
            manager.versions_dir(),
            temp.path().join("llama-cpp-versions")
        );
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let (manager, _temp) = create_test_manager().await;
        assert!(manager.versions_dir().ends_with("ollama-versions"));
    }

    #[tokio::test]
    async fn onnx_runtime_is_rejected_by_version_manager() {
        let temp_dir = TempDir::new().unwrap();

        let error = match VersionManager::new(temp_dir.path(), AppId::OnnxRuntime).await {
            Ok(_) => panic!("ONNX Runtime should not create a version manager"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("does not use the version manager"),
            "{error}"
        );
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
    async fn test_get_version_status_reads_installed_metadata() {
        let (manager, _temp) = create_test_manager().await;

        let metadata = pumas_library::metadata::InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            python_version: Some("3.11.0".to_string()),
            release_tag: "v1.0.0".to_string(),
            dependencies_installed: Some(true),
            release_date: None,
            release_notes: None,
            download_url: None,
            size: None,
            git_commit: None,
            requirements_hash: None,
        };

        {
            let mut state = manager.state.write().await;
            state.add_installed_version("v1.0.0", metadata).unwrap();
        }

        let status = manager.get_version_status().await.unwrap();
        assert_eq!(status.versions.len(), 1);
        assert_eq!(status.versions[0].tag, "v1.0.0");
        assert_eq!(
            status.versions[0].installed_date.as_deref(),
            Some("2024-01-01T00:00:00Z")
        );
        assert_eq!(status.versions[0].python_version.as_deref(), Some("3.11.0"));
        assert_eq!(status.versions[0].dependencies_installed, Some(true));
    }

    #[tokio::test]
    async fn test_path_helpers() {
        let (manager, temp) = create_test_manager().await;

        assert_eq!(manager.versions_dir(), temp.path().join("ollama-versions"));
        assert_eq!(manager.logs_dir(), temp.path().join("launcher-data/logs"));
        assert_eq!(manager.cache_dir(), temp.path().join("launcher-data/cache"));
        assert_eq!(
            manager.version_path("v1.0.0"),
            temp.path().join("ollama-versions/v1.0.0")
        );
    }
}
