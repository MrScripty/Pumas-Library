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
mod torch_alternatives;
mod torch_preview;

pub use constraints::ConstraintsManager;
pub use dependencies::DependencyManager;
pub use installer::VersionInstaller;
pub use launcher::VersionLauncher;
pub use ollama::OllamaVersionManager;
pub use progress::{InstallationProgressTracker, PackageWeights, ProgressUpdate};
pub use size_calculator::{ReleaseSize, SizeBreakdown, SizeCalculator};
pub use state::VersionState;
pub use torch_alternatives::{
    TorchAlternativeDiscovery, TorchAlternativeMatch, TorchReleaseCombination,
    TorchReleaseDriverAvailability, TorchReleaseDriverStatus, TorchReleaseOptionsDiscovery,
    TorchReleaseOptionsStatus, TorchReleaseRecommendation,
};
pub use torch_preview::{
    TorchArtifact, TorchPreview, TorchPreviewOutcome, TorchPreviewRejectionReason,
};

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

#[cfg(test)]
struct RemovalPause {
    entered: tokio::sync::Notify,
    proceed: tokio::sync::Notify,
}

#[cfg(test)]
struct TorchAdmissionPause {
    reached: tokio::sync::Notify,
    resume: tokio::sync::Semaphore,
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
    torch_control: Arc<installer::TorchInstallControl>,
    torch_cleanup: Arc<installer::TorchCleanupTasks>,
    torch_shutting_down: Arc<AtomicBool>,
    #[cfg(test)]
    torch_admission_pause: Option<Arc<TorchAdmissionPause>>,
    torch_previews: torch_preview::TorchPreviews,
    #[cfg(test)]
    torch_publication_pause: Option<Arc<installer::TorchPublicationPause>>,
    #[cfg(test)]
    torch_stage_override: Option<installer::TorchStageOverride>,
    #[cfg(test)]
    removal_pause: Option<Arc<RemovalPause>>,
    #[cfg(test)]
    removal_metadata_pause: Option<Arc<RemovalPause>>,
    #[cfg(test)]
    selection_proceeded: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    default_selection_proceeded: Option<Arc<AtomicBool>>,
    /// Lock for serializing installations.
    install_lock: Arc<Mutex<()>>,
    /// Lock for serializing active/default selection with removal without waiting for downloads.
    lifecycle_lock: Arc<Mutex<()>>,
    /// Currently installing tag (exclusive access only).
    installing_tag: Arc<Mutex<Option<String>>>,
}

impl VersionManager {
    /// Hold Torch's selection/removal lease while checking and starting a
    /// runtime. Drop it after startup admission or a failed attempt.
    pub async fn torch_lifecycle_lease(&self) -> Result<tokio::sync::OwnedMutexGuard<()>> {
        if self.app_id != AppId::Torch {
            return Err(PumasError::Config {
                message: "Torch lifecycle lease requires a Torch manager".into(),
            });
        }
        Ok(self.lifecycle_lock.clone().lock_owned().await)
    }

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

        let manager = Self {
            launcher_root,
            app_id,
            metadata_manager,
            github_client,
            state,
            progress_tracker,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            torch_control: Arc::new(installer::TorchInstallControl::new()),
            torch_cleanup: Arc::new(installer::TorchCleanupTasks::default()),
            torch_shutting_down: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            torch_admission_pause: None,
            torch_previews: Arc::new(Mutex::new(Default::default())),
            #[cfg(test)]
            torch_publication_pause: None,
            #[cfg(test)]
            torch_stage_override: None,
            #[cfg(test)]
            removal_pause: None,
            #[cfg(test)]
            removal_metadata_pause: None,
            #[cfg(test)]
            selection_proceeded: None,
            #[cfg(test)]
            default_selection_proceeded: None,
            install_lock: Arc::new(Mutex::new(())),
            lifecycle_lock: Arc::new(Mutex::new(())),
            installing_tag: Arc::new(Mutex::new(None)),
        };
        if app_id == AppId::Torch {
            if let Some(default) = manager.get_default_version().await? {
                if let Err(error) = manager.verify_torch_manifest(&default).await {
                    warn!("Ignoring unusable default Torch runtime {default}: {error}");
                    manager
                        .state
                        .write()
                        .await
                        .set_default_version(None)
                        .await?;
                }
            }
            if let Some(active) = manager.get_active_version().await? {
                if let Err(error) = manager.verify_torch_manifest(&active).await {
                    warn!("Ignoring unusable active Torch runtime {active}: {error}");
                    manager
                        .state
                        .write()
                        .await
                        .reset_torch_active_selection()
                        .await?;
                }
            } else if manager.get_default_version().await?.is_some() {
                manager
                    .state
                    .write()
                    .await
                    .reset_torch_active_selection()
                    .await?;
            }
        }
        Ok(manager)
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
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        #[cfg(test)]
        if let Some(proceeded) = &self.selection_proceeded {
            proceeded.store(true, Ordering::SeqCst);
        }
        if self.app_id == AppId::Torch {
            self.verify_torch_identity(tag).await?;
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
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        if self.app_id == AppId::Torch {
            if let Some(tag) = tag {
                self.verify_torch_identity(tag).await?;
            }
        }
        #[cfg(test)]
        if let Some(proceeded) = &self.default_selection_proceeded {
            proceeded.store(true, Ordering::SeqCst);
        }
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
            for release in &mut releases {
                // GitHub's archive is PyTorch source, not the managed wheel recipe.
                release.archive_size = None;
                release.total_size = None;
                release.dependencies_size = None;
            }
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
        Ok(release
            .filter(|release| {
                self.app_id != AppId::Torch || installer::is_torch_runtime_release(release)
            })
            .map(|mut release| {
                if self.app_id == AppId::Torch {
                    release.archive_size = None;
                    release.total_size = None;
                    release.dependencies_size = None;
                }
                release
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
        // Keep the tag lock until both the control transition and progress
        // update are recorded, so this request cannot spill into a new attempt.
        let installing = self.installing_tag.lock().await;
        if installing.is_none() {
            return Ok(false);
        }

        if self.app_id == AppId::Torch && !self.torch_control.request_cancel() {
            return Ok(false);
        }

        info!("Cancelling installation");
        self.cancel_flag.store(true, Ordering::SeqCst);

        // Update progress tracker
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.set_error("Installation cancelled by user");
        }

        drop(installing);

        Ok(true)
    }

    /// Stop admitting Torch cleanup work and wait for the current attempt and
    /// every cleanup task owned by this manager before server shutdown ends.
    pub async fn shutdown_torch_cleanup(&self) -> Result<()> {
        if self.app_id != AppId::Torch {
            return Ok(());
        }
        {
            let _installing = self.installing_tag.lock().await;
            self.torch_shutting_down.store(true, Ordering::SeqCst);
        }
        let _ = self.cancel_installation().await?;
        let _install_guard = self.install_lock.lock().await;
        self.torch_cleanup.close();
        self.torch_cleanup.drain().await
    }

    /// Install a version with progress channel.
    ///
    /// Returns a channel receiver for progress updates.
    pub async fn install_version(&self, tag: &str) -> Result<mpsc::Receiver<ProgressUpdate>> {
        self.install_version_with_preview(tag, None).await
    }

    pub async fn install_version_with_preview(
        &self,
        tag: &str,
        preview_id: Option<&str>,
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
        let install_guard = self.install_lock.clone().lock_owned().await;
        if self.app_id == AppId::Torch && self.torch_shutting_down.load(Ordering::SeqCst) {
            return Err(PumasError::InstallationFailed {
                message: "Torch version manager is shutting down".into(),
            });
        }
        // Resolve before recording installation state: discovery failure must not
        // leave a phantom installation that can never complete.
        let release = self.resolve_installable_release(tag).await?;
        #[cfg(test)]
        if let Some(pause) = &self.torch_admission_pause {
            pause.reached.notify_one();
            pause.resume.acquire().await.unwrap().forget();
        }
        let torch_plan = if self.app_id == AppId::Torch {
            match preview_id {
                Some(id) => {
                    let mut previews = self.torch_previews.lock().await;
                    let retained =
                        previews
                            .remove(id)
                            .ok_or_else(|| PumasError::InstallationFailed {
                                message: "Torch preview expired or unknown".into(),
                            })?;
                    if retained.created.elapsed() >= Duration::from_secs(30 * 60)
                        || retained.preview.tag != tag
                    {
                        return Err(PumasError::InstallationFailed {
                            message: "Torch preview does not match requested tag or has expired"
                                .into(),
                        });
                    }
                    Some(installer::TorchInstallPlan {
                        preview: retained.preview,
                        requirements: retained.requirements,
                        resolution: retained.resolution,
                        report: retained.report,
                        interpreter_path: retained.interpreter_path,
                        interpreter_hash: retained.interpreter_hash,
                    })
                }
                None if tag == "v2.9.1" => None,
                #[cfg(test)]
                None if self.torch_stage_override.is_some() => None,
                None => {
                    return Err(PumasError::InstallationFailed {
                        message: "A retained preview ID is required for this Torch runtime".into(),
                    })
                }
            }
        } else if preview_id.is_some() {
            return Err(PumasError::InstallationFailed {
                message: "Preview IDs are only valid for Torch".into(),
            });
        } else {
            None
        };
        if self.state.read().await.is_installed(tag) {
            return Err(PumasError::VersionAlreadyInstalled {
                tag: tag.to_string(),
            });
        }

        // Commit admission under the same lock used by cancellation. Shutdown
        // may begin during release resolution, before an installing tag exists.
        {
            let mut installing = self.installing_tag.lock().await;
            if self.app_id == AppId::Torch && self.torch_shutting_down.load(Ordering::SeqCst) {
                return Err(PumasError::InstallationFailed {
                    message: "Torch version manager is shutting down".into(),
                });
            }
            self.cancel_flag.store(false, Ordering::SeqCst);
            if self.app_id == AppId::Torch {
                self.torch_control.start();
            }
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
        )
        .with_torch_control(self.torch_control.clone())
        .with_torch_cleanup(self.torch_cleanup.clone());
        #[cfg(test)]
        let installer = if let Some(pause) = &self.torch_publication_pause {
            installer.with_torch_publication_pause(pause.clone())
        } else {
            installer
        };
        #[cfg(test)]
        let installer = if let Some(stage) = &self.torch_stage_override {
            installer.with_torch_stage_override(stage.clone())
        } else {
            installer
        };

        // Spawn installation task
        let tag = tag.to_string();
        let state = self.state.clone();
        let installing_tag = self.installing_tag.clone();
        let progress_tracker = self.progress_tracker.clone();
        let torch_control = self.torch_control.clone();
        let app_id = self.app_id;

        tokio::spawn(async move {
            let _install_guard = install_guard;
            let result = installer
                .install_version_with_torch_plan(&tag, &release, tx.clone(), torch_plan)
                .await;

            if app_id == AppId::Torch {
                torch_control.finish();
            }

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

    #[cfg(test)]
    pub(crate) fn with_torch_publication_pause(
        mut self,
        pause: Arc<installer::TorchPublicationPause>,
    ) -> Self {
        self.torch_publication_pause = Some(pause);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_torch_stage_override<F>(mut self, stage: F) -> Self
    where
        F: Fn(&Path) -> Result<PathBuf> + Send + Sync + 'static,
    {
        self.torch_stage_override = Some(Arc::new(stage));
        self
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
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
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

        #[cfg(test)]
        if let Some(pause) = &self.removal_pause {
            pause.entered.notify_one();
            pause.proceed.notified().await;
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

        #[cfg(test)]
        if let Some(pause) = &self.removal_metadata_pause {
            pause.entered.notify_one();
            pause.proceed.notified().await;
        }

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
    use tempfile::TempDir;

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

    async fn create_torch_test_manager() -> (VersionManager, TempDir) {
        let root = TempDir::new().unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        (manager, root)
    }

    #[tokio::test]
    async fn torch_shutdown_drains_cleanup_scheduled_by_active_install() {
        let (manager, _root) = create_torch_test_manager().await;
        let install_guard = manager.install_lock.lock().await;
        let shutdown_manager = manager.clone();
        let mut shutdown =
            tokio::spawn(async move { shutdown_manager.shutdown_torch_cleanup().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !manager.torch_shutting_down.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let (release, wait) = std::sync::mpsc::channel();
        let (started, observed) = std::sync::mpsc::channel();
        manager.torch_cleanup.schedule(move || {
            started.send(()).unwrap();
            wait.recv().unwrap();
        });
        tokio::task::spawn_blocking(move || observed.recv().unwrap())
            .await
            .unwrap();
        drop(install_guard);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut shutdown)
                .await
                .is_err()
        );
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), &mut shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(
            manager.install_version("v2.9.1").await,
            Err(PumasError::InstallationFailed { .. })
        ));
    }

    #[tokio::test]
    async fn torch_shutdown_rejects_install_paused_before_admission() {
        let root = TempDir::new().unwrap();
        let cache = root.path().join("launcher-data/cache");
        let releases = pumas_library::network::ReleasesCache::new(cache, Duration::from_secs(3600));
        releases
            .set_disk(
                AppId::Torch.github_repo(),
                &[pumas_library::network::GitHubRelease {
                    tag_name: "v2.9.1".into(),
                    name: "PyTorch 2.9.1".into(),
                    published_at: "2025-11-12T00:00:00Z".into(),
                    body: None,
                    tarball_url: None,
                    zipball_url: None,
                    prerelease: false,
                    assets: Vec::new(),
                    html_url: "https://github.com/pytorch/pytorch/releases/tag/v2.9.1".into(),
                    total_size: None,
                    archive_size: None,
                    dependencies_size: None,
                }],
            )
            .unwrap();
        let mut manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        let pause = Arc::new(TorchAdmissionPause {
            reached: tokio::sync::Notify::new(),
            resume: tokio::sync::Semaphore::new(0),
        });
        manager.torch_admission_pause = Some(pause.clone());
        let installing_manager = manager.clone();
        let install =
            tokio::spawn(async move { installing_manager.install_version("v2.9.1").await });
        tokio::time::timeout(Duration::from_secs(2), pause.reached.notified())
            .await
            .unwrap();
        let shutdown_manager = manager.clone();
        let shutdown = tokio::spawn(async move { shutdown_manager.shutdown_torch_cleanup().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !manager.torch_shutting_down.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        pause.resume.add_permits(1);
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), install)
                .await
                .unwrap()
                .unwrap(),
            Err(PumasError::InstallationFailed { .. })
        ));
        tokio::time::timeout(Duration::from_secs(2), shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!manager.is_installing().await);
        assert!(manager.get_installation_progress().await.is_none());
    }

    async fn register_test_version(manager: &VersionManager, tag: &str) {
        std::fs::create_dir_all(manager.version_path(tag)).unwrap();
        std::fs::create_dir_all(manager.version_path(tag).join("venv/bin")).unwrap();
        for required in [
            "runtime.json",
            "serve.py",
            "venv/bin/python",
            "requirements.txt",
        ] {
            std::fs::write(manager.version_path(tag).join(required), b"test fixture").unwrap();
        }
        if manager.app_id == AppId::Torch {
            std::fs::write(
                manager.version_path(tag).join("resolution.json"),
                r#"{"torch":"test-fixture"}"#,
            )
            .unwrap();
            let python = manager.version_path(tag).join("venv/bin/python");
            std::fs::write(&python, b"#!/bin/sh\necho test-fixture\n").unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let metadata = pumas_library::metadata::InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            python_version: None,
            release_tag: tag.to_string(),
            dependencies_installed: None,
            release_date: None,
            release_notes: None,
            download_url: None,
            size: None,
            git_commit: None,
            requirements_hash: None,
        };
        manager
            .state
            .write()
            .await
            .add_installed_version(tag, metadata)
            .unwrap();
    }

    #[tokio::test]
    async fn torch_activation_rejects_unregistered_path_before_reading_it() {
        let (manager, _root) = create_torch_test_manager().await;
        assert!(matches!(
            manager.set_active_version("../outside").await,
            Err(PumasError::Config { .. })
        ));
        assert!(matches!(
            manager.set_active_version("missing").await,
            Err(PumasError::VersionNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn torch_activation_requires_identity_manifest() {
        let (manager, _root) = create_torch_test_manager().await;
        register_test_version(&manager, "v2.10.0").await;
        std::fs::remove_file(manager.version_path("v2.10.0").join("runtime.json")).unwrap();
        let error = manager.set_active_version("v2.10.0").await.unwrap_err();
        assert!(error.to_string().contains("identity manifest"));
    }

    #[tokio::test]
    async fn qualified_legacy_torch_activation_uses_trusted_recipe_identity() {
        let (manager, _root) = create_torch_test_manager().await;
        register_test_version(&manager, "v2.9.1").await;
        let runtime = manager.version_path("v2.9.1");
        std::fs::remove_file(runtime.join("resolution.json")).unwrap();
        std::fs::write(
            runtime.join("runtime.json"),
            r#"{"recipe_id":"torch-upstream-2.9.1-r1"}"#,
        )
        .unwrap();
        std::fs::write(
            runtime.join("venv/bin/python"),
            b"#!/bin/sh\necho 2.9.1+cu130\n",
        )
        .unwrap();
        assert!(manager.set_active_version("v2.9.1").await.unwrap());
    }

    #[tokio::test]
    async fn restart_falls_back_from_invalid_active_torch_to_verified_default() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "good").await;
        register_test_version(&manager, "bad").await;
        manager.set_default_version(Some("good")).await.unwrap();
        manager.set_active_version("bad").await.unwrap();
        std::fs::remove_file(manager.version_path("bad").join("runtime.json")).unwrap();
        let restored = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert_eq!(
            restored.get_active_version().await.unwrap().as_deref(),
            Some("good")
        );
        assert_eq!(
            restored.get_default_version().await.unwrap().as_deref(),
            Some("good")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn removal_wins_race_with_switch_and_does_not_leave_removed_version_selected() {
        let (mut manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "remove").await;
        manager.set_active_version("keep").await.unwrap();
        manager.set_default_version(Some("keep")).await.unwrap();

        let pause = Arc::new(RemovalPause {
            entered: tokio::sync::Notify::new(),
            proceed: tokio::sync::Notify::new(),
        });
        manager.removal_pause = Some(pause.clone());
        let selection_proceeded = Arc::new(AtomicBool::new(false));
        manager.selection_proceeded = Some(selection_proceeded.clone());
        let removing_manager = manager.clone();
        let removing = tokio::spawn(async move { removing_manager.remove_version("remove").await });
        pause.entered.notified().await;

        let switching_manager = manager.clone();
        let (invoked_tx, invoked_rx) = tokio::sync::oneshot::channel();
        let switching = tokio::spawn(async move {
            invoked_tx.send(()).unwrap();
            switching_manager.set_active_version("remove").await
        });
        invoked_rx.await.unwrap();
        // On this single-thread runtime, the switch ran until it yielded at the
        // lifecycle lock. Without that lock it reaches the marker before yielding.
        assert!(!selection_proceeded.load(Ordering::SeqCst));
        assert!(!switching.is_finished());
        pause.proceed.notify_one();
        assert!(removing.await.unwrap().unwrap());
        assert!(matches!(
            switching.await.unwrap(),
            Err(PumasError::VersionNotFound { tag }) if tag == "remove"
        ));

        assert!(!manager.version_path("remove").exists());
        assert!(manager.version_path("keep").exists());
        assert_eq!(
            manager.get_installed_versions().await.unwrap(),
            vec!["keep"]
        );
        assert_eq!(
            manager.get_active_version().await.unwrap().as_deref(),
            Some("keep")
        );
        assert_eq!(
            manager.get_default_version().await.unwrap().as_deref(),
            Some("keep")
        );
        assert_eq!(
            std::fs::read_to_string(manager.active_version_file()).unwrap(),
            "keep"
        );
        assert_eq!(
            manager.active_version_file(),
            root.path().join(".active-version-torch")
        );
        let metadata = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(!metadata.installed.contains_key("remove"));
        assert!(metadata.installed.contains_key("keep"));
        assert_eq!(metadata.last_selected_version.as_deref(), Some("keep"));
        assert_eq!(metadata.default_version.as_deref(), Some("keep"));

        let reconstructed = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert_eq!(
            reconstructed.get_installed_versions().await.unwrap(),
            vec!["keep"]
        );
        assert_eq!(
            reconstructed.get_active_version().await.unwrap().as_deref(),
            Some("keep")
        );
        assert_eq!(
            reconstructed
                .get_default_version()
                .await
                .unwrap()
                .as_deref(),
            Some("keep")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn active_torch_version_cannot_be_removed_and_state_persists() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "selected").await;
        assert!(manager.set_active_version("selected").await.unwrap());
        assert!(manager.set_default_version(Some("selected")).await.unwrap());

        let error = manager.remove_version("selected").await.unwrap_err();
        assert!(error.to_string().contains("currently active"));
        assert!(manager.version_path("selected").exists());
        assert_eq!(
            manager.get_installed_versions().await.unwrap(),
            vec!["selected"]
        );
        assert_eq!(
            manager.get_active_version().await.unwrap().as_deref(),
            Some("selected")
        );
        assert_eq!(
            manager.get_default_version().await.unwrap().as_deref(),
            Some("selected")
        );
        assert_eq!(
            std::fs::read_to_string(manager.active_version_file()).unwrap(),
            "selected"
        );
        assert_eq!(
            manager.active_version_file(),
            root.path().join(".active-version-torch")
        );
        let metadata = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(metadata.installed.contains_key("selected"));
        assert_eq!(metadata.last_selected_version.as_deref(), Some("selected"));
        assert_eq!(metadata.default_version.as_deref(), Some("selected"));

        let reconstructed = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert_eq!(
            reconstructed.get_installed_versions().await.unwrap(),
            vec!["selected"]
        );
        assert_eq!(
            reconstructed.get_active_version().await.unwrap().as_deref(),
            Some("selected")
        );
        assert_eq!(
            reconstructed
                .get_default_version()
                .await
                .unwrap()
                .as_deref(),
            Some("selected")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn removal_finishes_before_defaulting_removed_torch_version() {
        let (mut manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "remove").await;
        manager.set_active_version("keep").await.unwrap();
        manager.set_default_version(Some("remove")).await.unwrap();

        let pause = Arc::new(RemovalPause {
            entered: tokio::sync::Notify::new(),
            proceed: tokio::sync::Notify::new(),
        });
        manager.removal_metadata_pause = Some(pause.clone());
        let default_proceeded = Arc::new(AtomicBool::new(false));
        manager.default_selection_proceeded = Some(default_proceeded.clone());
        let removing_manager = manager.clone();
        let removing = tokio::spawn(async move { removing_manager.remove_version("remove").await });
        pause.entered.notified().await;
        assert!(!manager.version_path("remove").exists());
        assert!(manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap()
            .default_version
            .is_none());

        let defaulting_manager = manager.clone();
        let (invoked_tx, invoked_rx) = tokio::sync::oneshot::channel();
        let defaulting = tokio::spawn(async move {
            invoked_tx.send(()).unwrap();
            defaulting_manager.set_default_version(Some("remove")).await
        });
        invoked_rx.await.unwrap();
        assert!(!default_proceeded.load(Ordering::SeqCst));
        assert!(!defaulting.is_finished());
        pause.proceed.notify_one();
        assert!(removing.await.unwrap().unwrap());
        assert!(matches!(
            defaulting.await.unwrap(),
            Err(PumasError::VersionNotFound { tag }) if tag == "remove"
        ));

        assert_eq!(
            manager.get_installed_versions().await.unwrap(),
            vec!["keep"]
        );
        assert_eq!(
            manager.get_active_version().await.unwrap().as_deref(),
            Some("keep")
        );
        assert_eq!(manager.get_default_version().await.unwrap(), None);
        assert_eq!(
            std::fs::read_to_string(manager.active_version_file()).unwrap(),
            "keep"
        );
        assert_eq!(
            manager.active_version_file(),
            root.path().join(".active-version-torch")
        );
        let metadata = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(!metadata.installed.contains_key("remove"));
        assert!(metadata.installed.contains_key("keep"));
        assert_eq!(metadata.last_selected_version.as_deref(), Some("keep"));
        assert_eq!(metadata.default_version, None);

        let reconstructed = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert_eq!(
            reconstructed.get_installed_versions().await.unwrap(),
            vec!["keep"]
        );
        assert_eq!(
            reconstructed.get_active_version().await.unwrap().as_deref(),
            Some("keep")
        );
        assert_eq!(reconstructed.get_default_version().await.unwrap(), None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn removing_inactive_default_torch_version_clears_default() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "remove").await;
        manager.set_active_version("keep").await.unwrap();
        assert!(manager.set_default_version(Some("remove")).await.unwrap());

        assert!(manager.remove_version("remove").await.unwrap());
        assert!(!manager.version_path("remove").exists());
        assert!(manager.version_path("keep").exists());
        assert_eq!(
            manager.get_installed_versions().await.unwrap(),
            vec!["keep"]
        );
        assert_eq!(
            manager.get_active_version().await.unwrap().as_deref(),
            Some("keep")
        );
        assert_eq!(manager.get_default_version().await.unwrap(), None);
        assert_eq!(
            std::fs::read_to_string(manager.active_version_file()).unwrap(),
            "keep"
        );
        assert_eq!(
            manager.active_version_file(),
            root.path().join(".active-version-torch")
        );
        let metadata = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(!metadata.installed.contains_key("remove"));
        assert!(metadata.installed.contains_key("keep"));
        assert_eq!(metadata.last_selected_version.as_deref(), Some("keep"));
        assert_eq!(metadata.default_version, None);

        let reconstructed = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert_eq!(
            reconstructed.get_installed_versions().await.unwrap(),
            vec!["keep"]
        );
        assert_eq!(
            reconstructed.get_active_version().await.unwrap().as_deref(),
            Some("keep")
        );
        assert_eq!(reconstructed.get_default_version().await.unwrap(), None);
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
