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
mod managed_python;
pub mod ollama;
mod progress;
pub mod size_calculator;
mod state;
mod torch_alternatives;
mod torch_preview;
mod torch_workspace;

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

use pumas_library::acquisition::{AcquisitionConsumer, AcquisitionService};
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

type InstallationTask = tokio::task::JoinHandle<std::result::Result<(), String>>;
type InstallationShutdown = futures::future::Shared<
    futures::future::BoxFuture<'static, std::result::Result<(), Arc<String>>>,
>;

#[derive(Default)]
struct InstallationTasks {
    tasks: Vec<InstallationTask>,
    failures: Vec<String>,
}

impl InstallationTasks {
    fn harvest_finished(&mut self) {
        use futures::FutureExt;
        for mut task in std::mem::take(&mut self.tasks) {
            if task.is_finished() {
                match (&mut task).now_or_never() {
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(error))) => self.failures.push(error),
                    Some(Err(error)) => self.failures.push(error.to_string()),
                    None => self.tasks.push(task),
                }
            } else {
                self.tasks.push(task);
            }
        }
    }
}

async fn wait_for_install_cancel(cancel_flag: Arc<AtomicBool>) {
    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn run_torch_operation_with_cancel<F, T>(
    operation: F,
    cancel_flag: Arc<AtomicBool>,
    cleanup: Arc<installer::TorchCleanupTasks>,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::select! {
        biased;
        _ = wait_for_install_cancel(cancel_flag) => {
            cleanup.drain_residual_child_slots().await?;
            Err(PumasError::InstallationFailed {
                message: "Installation cancelled by user".into(),
            })
        }
        result = operation => result,
    }
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
    /// Native release consumers share PumasApi's single acquisition service.
    acquisition_consumer: Option<Arc<AcquisitionConsumer>>,
    /// Version state tracker.
    state: Arc<RwLock<VersionState>>,
    /// Installation progress tracker.
    progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
    /// Cancellation flag for installations.
    cancel_flag: Arc<AtomicBool>,
    torch_control: Arc<installer::TorchInstallControl>,
    torch_cleanup: Arc<installer::TorchCleanupTasks>,
    torch_shutting_down: Arc<AtomicBool>,
    installation_tasks: Arc<std::sync::Mutex<InstallationTasks>>,
    installation_shutdown: Arc<Mutex<Option<InstallationShutdown>>>,
    #[cfg(test)]
    torch_admission_pause: Option<Arc<TorchAdmissionPause>>,
    torch_previews: torch_preview::TorchPreviews,
    torch_install_selections: torch_preview::TorchInstallSelections,
    #[cfg(test)]
    torch_publication_pause: Option<Arc<installer::TorchPublicationPause>>,
    #[cfg(test)]
    native_receipt_pause: Option<Arc<installer::TorchPublicationPause>>,
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
    /// llama.cpp installation requires [`Self::new_with_acquisition`] with the
    /// application's existing shared acquisition owner.
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
            acquisition_consumer: None,
            state,
            progress_tracker,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            torch_control: Arc::new(installer::TorchInstallControl::new()),
            torch_cleanup: Arc::new(installer::TorchCleanupTasks::default()),
            torch_shutting_down: Arc::new(AtomicBool::new(false)),
            installation_tasks: Arc::new(std::sync::Mutex::new(InstallationTasks::default())),
            installation_shutdown: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            torch_admission_pause: None,
            torch_previews: Arc::new(Mutex::new(Default::default())),
            torch_install_selections: Arc::new(Mutex::new(Default::default())),
            #[cfg(test)]
            torch_publication_pause: None,
            #[cfg(test)]
            native_receipt_pause: None,
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
            if let Some(lock) =
                Self::startup_torch_versions_lock(manager.try_torch_versions_lock_io())?
            {
                manager.state.write().await.refresh_with_lock(&lock).await?;
                let default = manager.state.read().await.get_default_version();
                if let Some(default) = default {
                    if let Err(error) = manager.verify_torch_manifest(&default).await {
                        warn!("Ignoring unusable default Torch runtime {default}: {error}");
                        manager
                            .state
                            .write()
                            .await
                            .set_default_version_with_lock(None, &lock)
                            .await?;
                    }
                }
                let active = manager.state.read().await.get_active_version();
                if let Some(active) = active {
                    if let Err(error) = manager.verify_torch_manifest(&active).await {
                        warn!("Ignoring unusable active Torch runtime {active}: {error}");
                        manager
                            .state
                            .write()
                            .await
                            .reset_torch_active_selection_with_lock(&lock)
                            .await?;
                    }
                } else if manager.state.read().await.get_default_version().is_some() {
                    manager
                        .state
                        .write()
                        .await
                        .reset_torch_active_selection_with_lock(&lock)
                        .await?;
                }
            }
        }
        Ok(manager)
    }

    /// Construct the llama.cpp manager with the application's existing shared
    /// acquisition owner. A second service/store is never created here.
    pub async fn new_with_acquisition(
        launcher_root: impl Into<PathBuf>,
        app_id: AppId,
        acquisition: Arc<AcquisitionService>,
    ) -> Result<Self> {
        if app_id != AppId::LlamaCpp {
            return Err(PumasError::Config {
                message: "Shared artifact acquisition is currently required for llama.cpp".into(),
            });
        }
        let mut manager = Self::new(launcher_root, app_id).await?;
        let consumer = Arc::new(acquisition.open_consumer("runtime.llama.cpp")?);
        manager.acquisition_consumer = Some(consumer.clone());
        let store = acquisition.store().clone();
        let records = consumer
            .run_blocking("look up retained native acquisitions", move || {
                store.acquisitions()
            })
            .await?;
        let installer = VersionInstaller::new(
            manager.launcher_root.clone(),
            app_id,
            manager.metadata_manager.clone(),
            manager.progress_tracker.clone(),
            manager.cancel_flag.clone(),
        )
        .with_acquisition_consumer(manager.acquisition_consumer.clone());
        if let Err(error) = installer
            .reconcile_retained_llama_cpp(records.into_values().collect())
            .await
        {
            let settlement = manager.shutdown_installations().await;
            return Err(match settlement {
                Ok(()) => error,
                Err(settlement) => PumasError::InstallationFailed {
                    message: format!("{error}; native recovery shutdown: {settlement}"),
                },
            });
        }
        manager.state.write().await.refresh().await?;
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
        self.state_snapshot(|state| state.get_installed_tags())
            .await
    }

    /// Get the currently active version tag.
    pub async fn get_active_version(&self) -> Result<Option<String>> {
        self.state_snapshot(|state| state.get_active_version())
            .await
    }

    /// Get the default version tag.
    pub async fn get_default_version(&self) -> Result<Option<String>> {
        self.state_snapshot(|state| state.get_default_version())
            .await
    }

    fn try_torch_versions_lock_io(&self) -> std::io::Result<Option<installer::TorchVersionsLock>> {
        if self.app_id != AppId::Torch {
            return Ok(None);
        }
        let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
        std::fs::create_dir_all(&versions_dir)?;
        Ok(Some(installer::TorchVersionsLock::try_acquire(
            &versions_dir,
        )?))
    }

    fn startup_torch_versions_lock(
        result: std::io::Result<Option<installer::TorchVersionsLock>>,
    ) -> Result<Option<installer::TorchVersionsLock>> {
        match result {
            Ok(lock) => Ok(lock),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                warn!(%error, "Torch startup selection normalization deferred");
                Ok(None)
            }
            Err(error) => Err(PumasError::from(error)),
        }
    }

    async fn acquire_torch_versions_lock_for_mutation(
        &self,
    ) -> Result<Option<installer::TorchVersionsLock>> {
        if self.app_id != AppId::Torch {
            return Ok(None);
        }
        let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
        std::fs::create_dir_all(&versions_dir).map_err(PumasError::from)?;
        installer::TorchVersionsLock::acquire_for_mutation(&versions_dir)
            .await
            .map(Some)
            .map_err(PumasError::from)
    }

    async fn state_snapshot<T>(&self, snapshot: impl FnOnce(&VersionState) -> T) -> Result<T> {
        match self.try_torch_versions_lock_io() {
            Ok(Some(lock)) => {
                let mut state = self.state.write().await;
                state.refresh_with_lock(&lock).await?;
                Ok(snapshot(&state))
            }
            Ok(None) => {
                let state = self.state.read().await;
                Ok(snapshot(&state))
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                let state = self.state.read().await;
                Ok(snapshot(&state))
            }
            Err(error) => Err(PumasError::from(error)),
        }
    }

    /// Set the active version.
    pub async fn set_active_version(&self, tag: &str) -> Result<bool> {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let torch_versions_lock = self.acquire_torch_versions_lock_for_mutation().await?;
        if let Some(lock) = &torch_versions_lock {
            self.state.write().await.refresh_with_lock(lock).await?;
        }
        #[cfg(test)]
        if let Some(proceeded) = &self.selection_proceeded {
            proceeded.store(true, Ordering::SeqCst);
        }
        if self.app_id == AppId::Torch {
            self.verify_torch_identity(tag).await?;
            let current = self.state.read().await.get_active_version();
            if current.as_deref() != Some(tag) {
                if let Some(current) = current {
                    self.ensure_torch_stopped(&current).await?;
                }
            }
        }
        let mut state = self.state.write().await;
        if let Some(lock) = &torch_versions_lock {
            state.set_active_version_with_lock(tag, lock).await
        } else {
            state.set_active_version(tag).await
        }
    }

    /// Set the default version.
    pub async fn set_default_version(&self, tag: Option<&str>) -> Result<bool> {
        let _lifecycle_guard = self.lifecycle_lock.lock().await;
        let torch_versions_lock = self.acquire_torch_versions_lock_for_mutation().await?;
        if let Some(lock) = &torch_versions_lock {
            self.state.write().await.refresh_with_lock(lock).await?;
        }
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
        if let Some(lock) = &torch_versions_lock {
            state.set_default_version_with_lock(tag, lock).await
        } else {
            state.set_default_version(tag).await
        }
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
        if self.app_id == AppId::Torch {
            return self
                .state_snapshot(|state| {
                    let installed = state.get_installed_tags();
                    let active = state.get_active_version();
                    let default = state.get_default_version();
                    let versions = installed
                        .iter()
                        .map(|tag| {
                            let info = state.get_installed_metadata(tag);
                            VersionStatusEntry {
                                tag: tag.clone(),
                                is_active: active.as_deref() == Some(tag),
                                is_default: default.as_deref() == Some(tag),
                                installed_date: info.map(|info| info.installed_date.clone()),
                                python_version: info.and_then(|info| info.python_version.clone()),
                                dependencies_installed: info
                                    .and_then(|info| info.dependencies_installed),
                            }
                        })
                        .collect();
                    VersionStatusReport {
                        versions,
                        active_version: active,
                        default_version: default,
                    }
                })
                .await;
        }
        let (installed, active, default) = self
            .state_snapshot(|state| {
                (
                    state.get_installed_tags(),
                    state.get_active_version(),
                    state.get_default_version(),
                )
            })
            .await?;

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

        if matches!(self.app_id, AppId::Torch | AppId::LlamaCpp)
            && !self.torch_control.request_cancel()
        {
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

    /// Close admission and drain every supported installation. A retained
    /// receipt survives cancelled shutdown waiters and preserves terminal failures.
    ///
    /// Installation failures, task panics and cleanup errors are returned after
    /// draining and remain observable on repeated calls. Publication that already
    /// won the cancellation race is allowed to finish. Owners must await this
    /// before shutting down the Tokio runtime.
    pub async fn shutdown_installations(&self) -> Result<()> {
        use futures::FutureExt;
        let completion = {
            let mut shutdown = self.installation_shutdown.lock().await;
            if let Some(completion) = &*shutdown {
                completion.clone()
            } else {
                {
                    let _installing = self.installing_tag.lock().await;
                    self.torch_shutting_down.store(true, Ordering::SeqCst);
                }
                let manager = self.clone();
                let worker =
                    tokio::spawn(async move {
                        // Cancellation itself may wait for progress state. Keep
                        // it inside the retained worker, so dropping a waiter
                        // cannot stop an already-started shutdown.
                        let mut errors = Vec::new();
                        if let Err(error) = manager.cancel_installation().await {
                            errors.push(error.to_string());
                        }
                        let _install_guard = manager.install_lock.lock().await;
                        let registered =
                            std::mem::take(&mut *manager.installation_tasks.lock().map_err(
                                |_| Arc::new("Installation task registry poisoned".into()),
                            )?);
                        errors.extend(registered.failures);
                        for task in registered.tasks {
                            match task.await {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => errors.push(error),
                                Err(error) => errors.push(error.to_string()),
                            }
                        }
                        let _lifecycle_guard = manager.lifecycle_lock.lock().await;
                        if let Err(error) = manager.state.write().await.shutdown_mutations().await {
                            errors.push(error.to_string());
                        }
                        if manager.app_id == AppId::Torch {
                            manager.torch_cleanup.close();
                            if let Err(error) = manager.torch_cleanup.drain().await {
                                errors.push(error.to_string());
                            }
                            if let Err(error) = manager.torch_cleanup.drain_child_slots().await {
                                errors.push(error.to_string());
                            }
                        }
                        if let Some(consumer) = &manager.acquisition_consumer {
                            if let Err(error) = consumer.shutdown().await {
                                errors.push(error.to_string());
                            }
                        }
                        if errors.is_empty() {
                            Ok(())
                        } else {
                            Err(Arc::new(errors.join("; ")))
                        }
                    });
                let completion = async move {
                    worker
                        .await
                        .unwrap_or_else(|error| Err(Arc::new(error.to_string())))
                }
                .boxed()
                .shared();
                *shutdown = Some(completion.clone());
                completion
            }
        };
        completion
            .await
            .map_err(|error| PumasError::InstallationFailed {
                message: (*error).clone(),
            })
    }

    /// Compatibility entry point; drains every supported runtime.
    pub async fn shutdown_torch_cleanup(&self) -> Result<()> {
        self.shutdown_installations().await
    }

    /// Install a version with progress channel.
    ///
    /// Returns a channel receiver for progress updates.
    /// Dropping the receiver does not cancel admitted work; use cancellation or
    /// `shutdown_installations` to settle the manager's owned installation.
    pub async fn install_version(&self, tag: &str) -> Result<mpsc::Receiver<ProgressUpdate>> {
        self.install_version_with_preview(tag, None).await
    }

    pub async fn install_version_with_preview(
        &self,
        tag: &str,
        preview_id: Option<&str>,
    ) -> Result<mpsc::Receiver<ProgressUpdate>> {
        if self.app_id == AppId::LlamaCpp && self.acquisition_consumer.is_none() {
            return Err(PumasError::Config {
                message: "llama.cpp installation requires the shared artifact acquisition service"
                    .into(),
            });
        }
        if self.app_id == AppId::Torch && torch_alternatives::stable_release_version(tag).is_none()
        {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_owned(),
            });
        }
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
        if self.torch_shutting_down.load(Ordering::SeqCst) {
            return Err(PumasError::InstallationFailed {
                message: "Version manager is shutting down".into(),
            });
        }
        // Release lookup and package resolution are part of installation work for
        // Torch. The UI/API must be able to enter a cancellable, visible state
        // without waiting for network discovery first.
        let release = if self.app_id == AppId::Torch {
            None
        } else {
            Some(self.resolve_installable_release(tag).await?)
        };
        #[cfg(test)]
        if let Some(pause) = &self.torch_admission_pause {
            pause.reached.notify_one();
            pause.resume.acquire().await.unwrap().forget();
        }
        if self.torch_shutting_down.load(Ordering::SeqCst) {
            return Err(PumasError::InstallationFailed {
                message: "Version manager is shutting down".into(),
            });
        }
        let torch_selection = if self.app_id == AppId::Torch {
            #[cfg(test)]
            if preview_id.is_none() && self.torch_stage_override.is_some() {
                None
            } else {
                Some(match preview_id {
                    Some(id) => self.consume_torch_install_selection(id, tag).await?,
                    None => torch_preview::TorchInstallSelection {
                        tag: tag.to_owned(),
                        build: "auto".to_owned(),
                        python: "auto".to_owned(),
                        adapter: "none".to_owned(),
                        created: std::time::Instant::now(),
                    },
                })
            }
            #[cfg(not(test))]
            Some(match preview_id {
                Some(id) => self.consume_torch_install_selection(id, tag).await?,
                None => torch_preview::TorchInstallSelection {
                    tag: tag.to_owned(),
                    build: "auto".to_owned(),
                    python: "auto".to_owned(),
                    adapter: "none".to_owned(),
                    created: std::time::Instant::now(),
                },
            })
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
        let mut installing = self.installing_tag.lock().await;
        // Registration is part of admission. A poisoned registry cannot leave
        // a task running without its completion capability.
        let mut registered =
            self.installation_tasks
                .lock()
                .map_err(|_| PumasError::InstallationFailed {
                    message: "Installation task registry poisoned".into(),
                })?;
        registered.harvest_finished();
        {
            if self.torch_shutting_down.load(Ordering::SeqCst) {
                return Err(PumasError::InstallationFailed {
                    message: "Version manager is shutting down".into(),
                });
            }
            self.cancel_flag.store(false, Ordering::SeqCst);
            if matches!(self.app_id, AppId::Torch | AppId::LlamaCpp) {
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
        .with_torch_cleanup(self.torch_cleanup.clone())
        .with_shutdown_flag(self.torch_shutting_down.clone())
        .with_github_client(self.github_client.clone())
        .with_acquisition_consumer(self.acquisition_consumer.clone());
        #[cfg(test)]
        let installer = if let Some(pause) = &self.native_receipt_pause {
            installer.with_native_receipt_pause(pause.clone())
        } else {
            installer
        };
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
        let manager = self.clone();
        let task_registry = self.installation_tasks.clone();

        let task = tokio::spawn(async move {
            let _install_guard = install_guard;
            if app_id == AppId::Torch {
                let mut tracker = progress_tracker.write().await;
                tracker.start_installation(&tag, None, None, None);
                tracker.update_stage(
                    pumas_library::models::InstallationStage::Resolving,
                    0.0,
                    Some("Preparing managed Python and resolving Torch packages"),
                );
                drop(tracker);
                let _ = tx
                    .send(ProgressUpdate::Setup {
                        message: "Preparing managed Python and resolving Torch packages…".into(),
                    })
                    .await;
            }
            let result = async {
                let (release, torch_input) = if app_id == AppId::Torch {
                    // The tag was admitted as stable semver. Wheel availability is
                    // established by the staged pip install, not GitHub metadata.
                    let release = Self::torch_release_for_install(&tag)?;
                    let input = match torch_selection {
                        Some(selection) if selection.adapter == "bundled" => {
                            let plan = run_torch_operation_with_cancel(
                                manager.resolve_torch_install_selection(&selection),
                                manager.cancel_flag.clone(),
                                manager.torch_cleanup.clone(),
                            )
                            .await?;
                            Some(installer::TorchInstallInput::Resolved(Box::new(plan)))
                        }
                        Some(selection) => Some(installer::TorchInstallInput::Selection(selection)),
                        None => None,
                    };
                    (release, input)
                } else {
                    (
                        release.expect("non-Torch install has a resolved release"),
                        None,
                    )
                };
                installer
                    .install_version_with_torch_input(&tag, &release, tx.clone(), torch_input)
                    .await
            }
            .await;

            if matches!(app_id, AppId::Torch | AppId::LlamaCpp) {
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
            if let Err(error) = &result {
                let mut tracker = progress_tracker.write().await;
                tracker.set_error(&error.to_string());
                tracker.complete_installation(false);
            }
            let terminal = result.as_ref().map(|_| ()).map_err(ToString::to_string);
            tokio::select! {
                _ = tx.send(match result {
                    Ok(_) => ProgressUpdate::Completed { success: true },
                    Err(e) => ProgressUpdate::Error { message: e.to_string() },
                }) => {},
                _ = wait_for_install_cancel(manager.torch_shutting_down.clone()) => {},
            }
            // Keep existing delayed progress cleanup, with observed lifecycle.
            let mut registered = task_registry
                .lock()
                .map_err(|_| "Installation task registry poisoned".to_owned())?;
            registered.harvest_finished();
            let cleanup = tokio::spawn(async move {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {
                        progress_tracker.write().await.clear_completed_state_async().await;
                    },
                    _ = wait_for_install_cancel(manager.torch_shutting_down.clone()) => {},
                }
                Ok(())
            });
            registered.tasks.push(cleanup);
            terminal
        });
        registered.tasks.push(task);
        drop(registered);
        drop(installing);

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

    fn torch_release_for_install(tag: &str) -> Result<pumas_library::network::GitHubRelease> {
        let version = torch_alternatives::stable_release_version(tag).ok_or_else(|| {
            PumasError::VersionNotFound {
                tag: tag.to_owned(),
            }
        })?;
        Ok(pumas_library::network::GitHubRelease {
            tag_name: tag.to_owned(),
            name: format!("PyTorch {version}"),
            // GitHub publication time is unknown without a metadata request.
            published_at: String::new(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: Vec::new(),
            html_url: format!("https://github.com/pytorch/pytorch/releases/tag/{tag}"),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
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
        let native_versions_lock = if self.app_id == AppId::LlamaCpp {
            Some(installer::NativeVersionsLock::acquire(self.versions_dir()).await?)
        } else {
            None
        };
        let torch_versions_lock = self.acquire_torch_versions_lock_for_mutation().await?;
        // Another backend may have changed metadata while this manager was open.
        if let Some(lock) = &torch_versions_lock {
            self.state.write().await.refresh_with_lock(lock).await?;
        } else if let Some(lock) = &native_versions_lock {
            self.state
                .write()
                .await
                .refresh_with_native_lock(lock)
                .await?;
        }
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

        // Keep directory removal and metadata deletion in one leased worker, so
        // cancelling this waiter cannot release mutation admission while the
        // filesystem and metadata effects are still running.
        let version_path = self.version_path(tag);
        let mutations = self.state.read().await.mutation_tasks();
        let metadata = self.metadata_manager.clone();
        let removed_tag = tag.to_owned();
        let app_id = self.app_id;
        let versions_for_removal = self.versions_dir();
        let remove = move || {
            if app_id == AppId::LlamaCpp {
                installer::mark_native_attempt_removed(&versions_for_removal, &removed_tag)?;
            }
            info!("Removing version directory: {}", version_path.display());
            match std::fs::remove_dir_all(&version_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(PumasError::io_with_path(error, &version_path)),
            }
            metadata.remove_installed_version(&removed_tag, Some(app_id))
        };
        if let Some(lock) = &torch_versions_lock {
            mutations.leased_transaction(lock, remove).await?;
        } else if let Some(lock) = &native_versions_lock {
            mutations.leased_transaction(lock, remove).await?;
        } else {
            mutations.leased_transaction(&(), remove).await?;
        }

        #[cfg(test)]
        if let Some(pause) = &self.removal_metadata_pause {
            pause.entered.notify_one();
            pause.proceed.notified().await;
        }

        // Refresh state
        {
            let mut state = self.state.write().await;
            if let Some(lock) = &torch_versions_lock {
                state.refresh_with_lock(lock).await?;
            } else if let Some(lock) = &native_versions_lock {
                state.refresh_with_native_lock(lock).await?;
            } else {
                state.refresh().await?;
            }
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

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    async fn native_archive_fixture(
        root: &Path,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<bool>,
        String,
    ) {
        use sha2::Digest;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let payload = b"#!/bin/sh\nprintf 'native-fixture'\n";
        let compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(compressed);
        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        archive
            .append_data(&mut header, "distribution/llama-server", &payload[..])
            .unwrap();
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        let size = bytes.len() as u64;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let url = format!("{base_url}/archive");
        let digest = format!("{:x}", sha2::Sha256::digest(&bytes));
        let releases = pumas_library::network::ReleasesCache::new(
            root.join("launcher-data/cache"),
            Duration::from_secs(3600),
        );
        releases
            .set_disk(
                AppId::LlamaCpp.github_repo(),
                &[pumas_library::network::GitHubRelease {
                    tag_name: "b1234".into(),
                    name: "Native fixture".into(),
                    published_at: "2026-09-29T00:00:00Z".into(),
                    body: None,
                    tarball_url: None,
                    zipball_url: None,
                    prerelease: false,
                    assets: vec![pumas_library::network::GitHubAsset {
                        name: "llama-b1234-bin-ubuntu-x64.tar.gz".into(),
                        size,
                        download_url: url.clone(),
                        content_type: Some("application/gzip".into()),
                    }],
                    html_url: "https://github.com/ggml-org/llama.cpp/releases/tag/b1234".into(),
                    total_size: Some(size),
                    archive_size: Some(size),
                    dependencies_size: None,
                }],
            )
            .unwrap();
        let (entered, observed) = tokio::sync::oneshot::channel();
        let (release, wait) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            // Real HTTP release metadata supplies the exact publisher asset ID
            // and SHA; cached discovery has neither and cannot authorize bytes.
            let (mut metadata_stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let read = metadata_stream.read(&mut request).await.unwrap();
            assert!(String::from_utf8_lossy(&request[..read])
                .starts_with("GET /repos/ggml-org/llama.cpp/releases/tags/b1234 HTTP/1.1"));
            let body = serde_json::to_vec(&serde_json::json!({
                "tag_name": "b1234", "assets": [{
                    "id": 1234, "name": "llama-b1234-bin-ubuntu-x64.tar.gz",
                    "size": size, "browser_download_url": url,
                    "digest": format!("sha256:{digest}")
                }]
            }))
            .unwrap();
            metadata_stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            metadata_stream.write_all(&body).await.unwrap();
            drop(metadata_stream);
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            entered.send(()).unwrap();
            if wait.await.unwrap() {
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            bytes.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                stream.write_all(&bytes).await.unwrap();
            }
        });
        (server, observed, release, base_url)
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn ollama_shutdown_cancels_stalled_headers_and_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for send_headers in [false, true] {
            let root = TempDir::new().unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/archive", listener.local_addr().unwrap());
            let releases = pumas_library::network::ReleasesCache::new(
                root.path().join("launcher-data/cache"),
                Duration::from_secs(3600),
            );
            releases
                .set_disk(
                    AppId::Ollama.github_repo(),
                    &[pumas_library::network::GitHubRelease {
                        tag_name: "v0.1.2".into(),
                        name: "Ollama fixture".into(),
                        published_at: "2026-09-29T00:00:00Z".into(),
                        body: None,
                        tarball_url: None,
                        zipball_url: None,
                        prerelease: false,
                        assets: vec![pumas_library::network::GitHubAsset {
                            name: "ollama-linux-amd64.tgz".into(),
                            size: 100,
                            download_url: url,
                            content_type: Some("application/gzip".into()),
                        }],
                        html_url: "https://github.com/ollama/ollama/releases/tag/v0.1.2".into(),
                        total_size: Some(100),
                        archive_size: Some(100),
                        dependencies_size: None,
                    }],
                )
                .unwrap();
            let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                assert!(stream.read(&mut request).await.unwrap() > 0);
                if send_headers {
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nx",
                        )
                        .await
                        .unwrap();
                }
                entered_tx.send(()).unwrap();
                release_rx.await.unwrap();
            });
            let manager = VersionManager::new(root.path(), AppId::Ollama)
                .await
                .unwrap();
            let mut updates = manager.install_version("v0.1.2").await.unwrap();
            tokio::time::timeout(Duration::from_secs(2), entered_rx)
                .await
                .unwrap()
                .unwrap();
            if send_headers {
                // Prove that body handling has started, rather than merely
                // testing the header cancellation branch twice.
                tokio::time::timeout(Duration::from_secs(2), async {
                    while let Some(update) = updates.recv().await {
                        if matches!(update, ProgressUpdate::Download { downloaded_bytes, .. } if downloaded_bytes > 0) {
                            return;
                        }
                    }
                    panic!("Ollama transfer ended without receiving its body");
                })
                .await
                .unwrap();
            }
            let error =
                tokio::time::timeout(Duration::from_secs(2), manager.shutdown_installations())
                    .await
                    .unwrap()
                    .unwrap_err()
                    .to_string();
            assert!(error.contains("cancelled"), "{error}");
            assert_eq!(
                manager
                    .shutdown_installations()
                    .await
                    .unwrap_err()
                    .to_string(),
                error
            );
            assert!(!manager.version_path("v0.1.2").exists());
            assert!(manager
                .metadata_manager
                .get_installed_version("v0.1.2", Some(AppId::Ollama))
                .unwrap()
                .is_none());
            release_tx.send(()).unwrap();
            server.await.unwrap();
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_manager_removal_shares_installation_admission() {
        let root = TempDir::new().unwrap();
        let manager = VersionManager::new(root.path(), AppId::LlamaCpp)
            .await
            .unwrap();
        for tag in ["b1234+cpu", "b1235+cpu"] {
            std::fs::create_dir(manager.version_path(tag)).unwrap();
            std::fs::write(manager.version_path(tag).join("llama-server"), "complete").unwrap();
            manager
                .state
                .write()
                .await
                .add_installed_version(
                    tag,
                    pumas_library::metadata::InstalledVersionMetadata {
                        path: tag.into(),
                        release_tag: tag.into(),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        manager.set_active_version("b1235+cpu").await.unwrap();
        let lease = installer::NativeVersionsLock::try_acquire(&manager.versions_dir()).unwrap();
        assert!(manager.remove_version("b1234+cpu").await.is_err());
        assert!(manager.version_path("b1234+cpu/llama-server").exists());
        assert!(manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_some());
        drop(lease);
        assert!(manager.remove_version("b1234+cpu").await.unwrap());
        assert!(!manager.version_path("b1234+cpu").exists());
        assert!(manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        assert!(manager
            .metadata_manager
            .get_installed_version("b1235+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_some());
        manager.shutdown_installations().await.unwrap();
    }

    #[test]
    fn native_removal_shutdown_observes_cancelled_waiter_completion_and_failure() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            for fail_removal in [false, true] {
                let root = TempDir::new().unwrap();
                let mut manager = VersionManager::new(root.path(), AppId::LlamaCpp)
                    .await
                    .unwrap();
                for tag in ["b1234+cpu", "b1235+cpu"] {
                    std::fs::create_dir(manager.version_path(tag)).unwrap();
                    std::fs::write(manager.version_path(tag).join("llama-server"), "complete")
                        .unwrap();
                    manager
                        .state
                        .write()
                        .await
                        .add_installed_version(
                            tag,
                            pumas_library::metadata::InstalledVersionMetadata {
                                path: tag.into(),
                                release_tag: tag.into(),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
                manager.set_active_version("b1235+cpu").await.unwrap();
                let pause = Arc::new(RemovalPause {
                    entered: tokio::sync::Notify::new(),
                    proceed: tokio::sync::Notify::new(),
                });
                manager.removal_pause = Some(pause.clone());
                let owner = manager.state.read().await.mutation_tasks();
                let removing = manager.clone();
                let waiter =
                    tokio::spawn(async move { removing.remove_version("b1234+cpu").await });
                pause.entered.notified().await;
                if fail_removal {
                    std::fs::remove_dir_all(manager.version_path("b1234+cpu")).unwrap();
                    std::fs::write(manager.version_path("b1234+cpu"), "retained").unwrap();
                }
                let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
                let (release_tx, release_rx) = std::sync::mpsc::channel();
                let blocker = tokio::task::spawn_blocking(move || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                });
                entered_rx.await.unwrap();
                pause.proceed.notify_one();
                tokio::time::timeout(Duration::from_secs(2), async {
                    while !owner.has_active_tasks() {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                waiter.abort();
                assert!(waiter.await.unwrap_err().is_cancelled());
                let shutting = manager.clone();
                let mut shutdown =
                    tokio::spawn(async move { shutting.shutdown_installations().await });
                assert!(
                    tokio::time::timeout(Duration::from_millis(30), &mut shutdown)
                        .await
                        .is_err()
                );
                assert!(manager.version_path("b1234+cpu").exists());
                assert!(
                    installer::NativeVersionsLock::try_acquire(&manager.versions_dir()).is_err()
                );
                // Cancel the first shutdown waiter as well. Its retained drain
                // still owns the queued removal and observes its terminal result.
                shutdown.abort();
                assert!(shutdown.await.unwrap_err().is_cancelled());
                release_tx.send(()).unwrap();
                blocker.await.unwrap();
                let outcome = manager
                    .shutdown_installations()
                    .await
                    .map_err(|error| error.to_string());
                if fail_removal {
                    assert!(outcome.as_ref().unwrap_err().contains("b1234+cpu"));
                    assert_eq!(
                        std::fs::read(manager.version_path("b1234+cpu")).unwrap(),
                        b"retained"
                    );
                } else {
                    assert!(outcome.is_ok());
                    assert!(!manager.version_path("b1234+cpu").exists());
                }
                assert_eq!(
                    manager
                        .metadata_manager
                        .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
                        .unwrap()
                        .is_some(),
                    fail_removal
                );
                assert_eq!(
                    manager
                        .shutdown_installations()
                        .await
                        .map_err(|error| error.to_string()),
                    outcome
                );
                assert!(manager.set_default_version(None).await.is_err());
            }
        });
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_cancel_before_completion_receipt_prevents_publication() {
        let root = TempDir::new().unwrap();
        let (server, observed, release, base_url) = native_archive_fixture(root.path()).await;
        let api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let mut manager = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        manager.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                manager.cache_dir(),
                Duration::from_secs(3600),
                base_url,
            )
            .unwrap(),
        );
        let pause = Arc::new(installer::TorchPublicationPause::new());
        manager.native_receipt_pause = Some(pause.clone());
        let mut updates = manager.install_version("b1234+cpu").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), observed)
            .await
            .unwrap()
            .unwrap();
        release.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), pause.reached.notified())
            .await
            .expect("native extraction and proof hashes must reach the receipt boundary");
        assert!(manager.cancel_installation().await.unwrap());
        pause.resume.add_permits(1);
        let message = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match updates
                    .recv()
                    .await
                    .expect("installation must send its terminal result")
                {
                    ProgressUpdate::Error { message } => break message,
                    ProgressUpdate::Completed { success } => {
                        panic!("cancelled installation reported completion: {success}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("cancelled attempt must settle");
        assert!(message.to_lowercase().contains("cancel"), "{message}");
        server.await.unwrap();
        // Shutdown joins the registered installer task and observes its failure.
        assert!(manager.shutdown_installations().await.is_err());
        assert!(!manager.is_installing().await);
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(document["acquisitions"].as_object().unwrap().len(), 1);
        assert!(document["consumer_receipts"]
            .as_object()
            .unwrap()
            .is_empty());
        assert!(!manager.version_path("b1234+cpu").exists());
        assert!(manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        assert_eq!(
            document["acquisitions"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .unwrap()["phase"]["state"],
            "withdrawn"
        );
        assert!(!std::fs::read_dir(manager.versions_dir())
            .unwrap()
            .any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".llama-install-")
            }));
        api.shutdown_acquisition().await.unwrap();
        drop(manager);
        drop(api);
        // A fresh service and manager must admit a new attempt for the same tag.
        let (retry_server, retry_observed, retry_release, retry_base_url) =
            native_archive_fixture(root.path()).await;
        let reopened_api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let mut reopened = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            reopened_api.acquisition().clone(),
        )
        .await
        .unwrap();
        reopened.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                reopened.cache_dir(),
                Duration::from_secs(3600),
                retry_base_url,
            )
            .unwrap(),
        );
        let mut retry_updates = reopened.install_version("b1234+cpu").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), retry_observed)
            .await
            .unwrap()
            .unwrap();
        retry_release.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match retry_updates.recv().await.unwrap() {
                    ProgressUpdate::Completed { success: true } => break,
                    ProgressUpdate::Error { message } => panic!("same-tag retry failed: {message}"),
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        retry_server.await.unwrap();
        reopened.shutdown_installations().await.unwrap();
        assert!(reopened
            .version_path("b1234+cpu")
            .join("bin/llama-server")
            .is_file());
        assert!(reopened
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_some());
        let retry_document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(retry_document["acquisitions"].as_object().unwrap().len(), 2);
        assert_eq!(
            retry_document["consumer_receipts"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
        reopened_api.shutdown_acquisition().await.unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_receipt_publication_conflict_cold_reopen_reconciles_without_reacquiring() {
        use sha2::Digest;
        use std::os::unix::fs::MetadataExt;

        // Record filesystem identity as well as every retained byte and the
        // receipt's tree hash. The fixture contains only regular files/directories.
        type SnapshotEntry = (PathBuf, u64, u64, u32, Vec<u8>);
        type Snapshot = (Vec<SnapshotEntry>, String);

        fn snapshot(root: &Path) -> Snapshot {
            let mut tree = Vec::new();
            let mut digest = sha2::Sha256::new();
            for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
                let entry = entry.unwrap();
                let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
                let metadata = std::fs::symlink_metadata(entry.path()).unwrap();
                assert!(!metadata.file_type().is_symlink());
                let name = relative.to_str().unwrap();
                digest.update((name.len() as u64).to_le_bytes());
                digest.update(name.as_bytes());
                digest.update(metadata.mode().to_le_bytes());
                let bytes = if metadata.is_file() {
                    let bytes = std::fs::read(entry.path()).unwrap();
                    digest.update(b"file");
                    digest.update(format!("{:x}", sha2::Sha256::digest(&bytes)).as_bytes());
                    bytes
                } else {
                    assert!(metadata.is_dir());
                    digest.update(b"directory");
                    Vec::new()
                };
                tree.push((
                    relative,
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mode(),
                    bytes,
                ));
            }
            (tree, format!("{:x}", digest.finalize()))
        }

        let root = TempDir::new().unwrap();
        let (server, observed, release, base_url) = native_archive_fixture(root.path()).await;
        let api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let mut manager = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        manager.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                manager.cache_dir(),
                Duration::from_secs(3600),
                base_url,
            )
            .unwrap(),
        );
        let pause = Arc::new(installer::TorchPublicationPause::new());
        manager.native_receipt_pause = Some(pause.clone());
        let mut updates = manager.install_version("b1234+cpu").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), observed)
            .await
            .unwrap()
            .unwrap();
        release.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), pause.reached.notified())
            .await
            .expect("extraction and proof must reach the receipt boundary");
        // This destination belongs solely to the test. Create the conflict
        // after extraction so the real receipt is durable before publication fails.
        let destination = manager.version_path("b1234+cpu");
        std::fs::create_dir_all(destination.join("bin")).unwrap();
        std::fs::write(destination.join("bin/llama-server"), b"test-owned conflict").unwrap();
        let conflict = snapshot(&destination);
        pause.resume.add_permits(1);
        let message = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match updates.recv().await.expect("installation must settle") {
                    ProgressUpdate::Error { message } => break message,
                    ProgressUpdate::Completed { success } => {
                        panic!("conflicting publication completed: {success}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        assert!(
            message.contains("Native version destination already exists"),
            "{message}"
        );
        server.await.unwrap();
        assert!(manager.shutdown_installations().await.is_err());
        assert!(!manager.is_installing().await);
        assert!(manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        // The supervisor drains, then reports the deliberately unresolved
        // receipt-bearing Using record as shutdown failure.
        assert!(api.shutdown_acquisition().await.is_err());
        let store_path = root.path().join("launcher-data/downloads.json");
        let read_document = || -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(&store_path).unwrap()).unwrap()
        };
        let retained = read_document();
        assert_eq!(retained["acquisitions"].as_object().unwrap().len(), 1);
        assert_eq!(retained["consumer_receipts"].as_object().unwrap().len(), 1);
        let (id, record) = retained["acquisitions"]
            .as_object()
            .unwrap()
            .iter()
            .next()
            .unwrap();
        assert_eq!(record["phase"]["state"], "using");
        let receipt = retained["consumer_receipts"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(receipt["owner"], "runtime.llama.cpp");
        assert_eq!(receipt["acquisition_id"], id.as_str());
        assert_eq!(receipt["use_lease"], record["phase"]["lease"]);
        assert_eq!(receipt["demand"], record["demand"]);
        assert_eq!(receipt["manifest"], record["manifest"]);
        assert_eq!(receipt["workspace"], record["workspace"]);
        assert_eq!(receipt["verified_files"], record["files"]);
        let workspace = manager
            .versions_dir()
            .join(record["workspace"]["relative_target"].as_str().unwrap());
        let stage = workspace.join("output");
        let staged = snapshot(&stage);
        assert_eq!(receipt["payload"]["output_tree_sha256"], staged.1);
        assert_eq!(record["files"].as_array().unwrap().len(), 1);
        let verified = &record["files"][0];
        let archive = workspace.join(verified["path"].as_str().unwrap());
        let archive_bytes = std::fs::read(&archive).unwrap();
        assert_eq!(verified["bytes"], archive_bytes.len() as u64);
        assert_eq!(
            verified["sha256"],
            format!("{:x}", sha2::Sha256::digest(&archive_bytes))
        );
        let retained_workspace = snapshot(&workspace);
        drop(updates);
        drop(pause);
        drop(manager);
        drop(api);

        // The source server is joined and closed. Neither reopen is allowed to
        // replace this acquisition/receipt or mutate retained bytes to make progress.
        let blocked_api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let error = match VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            blocked_api.acquisition().clone(),
        )
        .await
        {
            Ok(manager) => {
                manager.shutdown_installations().await.unwrap();
                panic!("conflicting output must refuse cold recovery")
            }
            Err(error) => error,
        };
        assert!(
            error.to_string().contains(
                "Native output differs from its durable llama.cpp receipt; recovery required"
            ),
            "{error}"
        );
        assert!(blocked_api.shutdown_acquisition().await.is_err());
        assert_eq!(read_document()["acquisitions"], retained["acquisitions"]);
        assert_eq!(
            read_document()["consumer_receipts"],
            retained["consumer_receipts"]
        );
        assert_eq!(snapshot(&workspace), retained_workspace);
        assert_eq!(snapshot(&destination), conflict);
        assert!(MetadataManager::new(root.path())
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        drop(blocked_api);

        // Remove only the test's conflicting directory, then compose fresh owners.
        std::fs::remove_dir_all(&destination).unwrap();
        let reopened_api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let reopened = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            reopened_api.acquisition().clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            reopened.get_installed_versions().await.unwrap(),
            vec!["b1234+cpu"]
        );
        assert_eq!(snapshot(&destination), staged);
        let installed = reopened
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(installed).unwrap(),
            receipt["payload"]["metadata"]
        );
        assert!(!stage.exists());
        assert!(!workspace.exists());
        reopened.shutdown_installations().await.unwrap();
        reopened_api.shutdown_acquisition().await.unwrap();
        let settled = read_document();
        assert_eq!(settled["consumer_receipts"], retained["consumer_receipts"]);
        assert_eq!(settled["acquisitions"].as_object().unwrap().len(), 1);
        let mut expected = record.clone();
        expected["phase"]["state"] = serde_json::json!("adopted");
        assert_eq!(settled["acquisitions"][id], expected);
        drop(reopened);
        drop(reopened_api);
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_direct_recovery_cancellation_retains_effect_until_shared_shutdown() {
        let root = TempDir::new().unwrap();
        let (server, observed, release, base_url) = native_archive_fixture(root.path()).await;
        let api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let mut manager = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        manager.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                manager.cache_dir(),
                Duration::from_secs(3600),
                base_url,
            )
            .unwrap(),
        );
        let mut updates = manager.install_version("b1234+cpu").await.unwrap();
        observed.await.unwrap();
        release.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match updates.recv().await.unwrap() {
                    ProgressUpdate::Completed { success: true } => break,
                    ProgressUpdate::Error { message } => panic!("native fixture failed: {message}"),
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        manager.shutdown_installations().await.unwrap();
        let record = api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        let attempt = record.demand.operation.rsplit_once(':').unwrap().1;
        use sha2::Digest;
        let digest = format!("{:x}", sha2::Sha256::digest(b"b1234+cpu"));
        let stale_workspace = manager
            .versions_dir()
            .join(format!(".llama-install-{}-{attempt}", &digest[..24]));
        std::fs::create_dir(&stale_workspace).unwrap();
        std::fs::write(
            stale_workspace.join("stale"),
            b"registered recovery cleanup",
        )
        .unwrap();
        let (pause, resume) = installer::NativeRecoveryPause::new();
        let pause = Arc::new(pause);
        let direct = VersionInstaller::new(
            root.path().to_path_buf(),
            AppId::LlamaCpp,
            manager.metadata_manager.clone(),
            manager.progress_tracker.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .with_native_recovery_pause(pause.clone());
        let acquisition = api.acquisition().clone();
        let constructor = tokio::spawn(async move { direct.with_acquisition(acquisition).await });
        tokio::time::timeout(Duration::from_secs(5), pause.reached.notified())
            .await
            .unwrap();
        constructor.abort();
        assert!(matches!(constructor.await, Err(error) if error.is_cancelled()));
        let acquisition = api.acquisition().clone();
        let mut shutdown = tokio::spawn(async move { acquisition.shutdown().await });
        let early = tokio::time::timeout(Duration::from_millis(50), &mut shutdown).await;
        let still_present = stale_workspace.exists();
        // Always release the real closure before checking outcomes so failures
        // cannot strand a blocking thread during test-runtime shutdown.
        resume.send(()).unwrap();
        assert!(
            early.is_err(),
            "shared shutdown must wait for the registered recovery effect"
        );
        assert!(still_present);
        tokio::time::timeout(Duration::from_secs(5), shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!stale_workspace.exists());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_archive_publishes_complete_output_and_reopens_metadata() {
        let root = TempDir::new().unwrap();
        let (server, observed, release, base_url) = native_archive_fixture(root.path()).await;
        let api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let mut manager = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        manager.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                manager.cache_dir(),
                Duration::from_secs(3600),
                base_url,
            )
            .unwrap(),
        );
        let mut updates = manager.install_version("b1234+cpu").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), observed)
            .await
            .unwrap()
            .unwrap();
        assert!(!manager.version_path("b1234+cpu").exists());
        assert!(manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        release.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match updates.recv().await.unwrap() {
                    ProgressUpdate::Completed { success: true } => break,
                    ProgressUpdate::Error { message } => {
                        panic!("Native installation failed: {message}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        manager.shutdown_installations().await.unwrap();
        let output =
            std::process::Command::new(manager.version_path("b1234+cpu").join("bin/llama-server"))
                .current_dir(root.path())
                .output()
                .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"native-fixture");
        use sha2::Digest;
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        let operation = document["acquisitions"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["demand"]["operation"]
            .as_str()
            .unwrap();
        let attempt = operation.rsplit_once(':').unwrap().1;
        let tag_digest = format!("{:x}", sha2::Sha256::digest(b"b1234+cpu"));
        let stale_workspace = manager
            .versions_dir()
            .join(format!(".llama-install-{}-{attempt}", &tag_digest[..24]));
        std::fs::create_dir(&stale_workspace).unwrap();
        std::fs::write(
            stale_workspace.join("stale"),
            b"retry cleanup after adoption",
        )
        .unwrap();
        // Direct public construction must finish the same retained adopted-use
        // recovery before it returns an installer that can admit new work.
        let direct = VersionInstaller::new(
            root.path().to_path_buf(),
            AppId::LlamaCpp,
            manager.metadata_manager.clone(),
            manager.progress_tracker.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .with_acquisition(api.acquisition().clone())
        .await
        .unwrap();
        assert!(!stale_workspace.exists());
        drop(direct);
        let reopened = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            reopened.get_installed_versions().await.unwrap(),
            vec!["b1234+cpu"]
        );
        assert!(std::fs::read_dir(manager.versions_dir())
            .unwrap()
            .all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".llama-install-")
            }));
        reopened.shutdown_installations().await.unwrap();
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(document["acquisitions"].as_object().unwrap().len(), 1);
        assert_eq!(document["consumer_receipts"].as_object().unwrap().len(), 1);
        let receipt = document["consumer_receipts"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(receipt["owner"], "runtime.llama.cpp");
        // Exercise explicit removal through the real manager and reinstall the
        // same tag with a new publisher-verified acquisition identity.
        let reinstall = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        let mut keep = reinstall
            .get_version_info("b1234+cpu")
            .await
            .unwrap()
            .unwrap();
        keep.path = "b9999+cpu".into();
        keep.release_tag = "b9999+cpu".into();
        std::fs::create_dir(reinstall.version_path("b9999+cpu")).unwrap();
        reinstall
            .state
            .write()
            .await
            .add_installed_version("b9999+cpu", keep)
            .unwrap();
        reinstall.set_active_version("b9999+cpu").await.unwrap();
        assert!(reinstall.remove_version("b1234+cpu").await.unwrap());
        reinstall.shutdown_installations().await.unwrap();
        let mut reinstall = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        assert!(!reinstall.version_path("b1234+cpu").exists());
        let (server, observed, release, base_url) = native_archive_fixture(root.path()).await;
        reinstall.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                reinstall.cache_dir(),
                Duration::from_secs(3600),
                base_url,
            )
            .unwrap(),
        );
        let mut updates = reinstall.install_version("b1234+cpu").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), observed)
            .await
            .unwrap()
            .unwrap();
        release.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(update) = updates.recv().await {
                match update {
                    ProgressUpdate::Completed { success: true } => return,
                    ProgressUpdate::Error { message } => panic!("Reinstall failed: {message}"),
                    _ => {}
                }
            }
            panic!("Reinstall ended without completion");
        })
        .await
        .unwrap();
        server.await.unwrap();
        reinstall.shutdown_installations().await.unwrap();
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        let records = document["acquisitions"].as_object().unwrap();
        assert_eq!(records.len(), 2);
        let demands: std::collections::BTreeSet<_> = records
            .values()
            .map(|record| record["demand"]["operation"].as_str().unwrap())
            .collect();
        assert_eq!(demands.len(), 2);
        assert_eq!(document["consumer_receipts"].as_object().unwrap().len(), 2);
        api.shutdown_intent().await.unwrap();
        api.shutdown_acquisition().await.unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_full_progress_channel_releases_on_cancel_and_shutdown_signals() {
        for shutdown_signal in [false, true] {
            let root = TempDir::new().unwrap();
            let (server, observed, release, base_url) = native_archive_fixture(root.path()).await;
            let acquisition = Arc::new(AcquisitionService::new(Arc::new(
                pumas_library::acquisition::AcquisitionStore::new(root.path()),
            )));
            let mut manager = VersionManager::new_with_acquisition(
                root.path(),
                AppId::LlamaCpp,
                acquisition.clone(),
            )
            .await
            .unwrap();
            manager.github_client = Arc::new(
                GitHubClient::with_loopback_api(
                    manager.cache_dir(),
                    Duration::from_secs(3600),
                    base_url,
                )
                .unwrap(),
            );
            let release_asset = manager
                .resolve_installable_release("b1234+cpu")
                .await
                .unwrap();
            let cancelled = Arc::new(AtomicBool::new(false));
            let shutting_down = Arc::new(AtomicBool::new(false));
            let direct = VersionInstaller::new(
                root.path().to_path_buf(),
                AppId::LlamaCpp,
                manager.metadata_manager.clone(),
                manager.progress_tracker.clone(),
                cancelled.clone(),
            )
            .with_shutdown_flag(shutting_down.clone())
            .with_acquisition_consumer(manager.acquisition_consumer.clone())
            .with_github_client(manager.github_client.clone());
            let (sender, mut receiver) = mpsc::channel(1);
            sender
                .send(ProgressUpdate::Setup {
                    message: "full".into(),
                })
                .await
                .unwrap();
            let install = tokio::spawn(async move {
                direct
                    .install_version("b1234+cpu", &release_asset, sender)
                    .await
            });
            tokio::time::timeout(Duration::from_secs(5), observed)
                .await
                .unwrap()
                .unwrap();
            release.send(true).unwrap();
            // The real host updates tracking immediately before its bounded
            // send. With the receiver untouched and both flags false, any
            // positive byte observation proves transfer progress is blocked.
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if manager
                        .get_installation_progress()
                        .await
                        .is_some_and(|progress| progress.downloaded_bytes.unwrap_or(0) > 0)
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("HTTP transfer must reach the full progress channel");
            assert!(!install.is_finished());
            if shutdown_signal {
                shutting_down.store(true, Ordering::SeqCst);
            } else {
                cancelled.store(true, Ordering::SeqCst);
            }
            let outcome = tokio::time::timeout(Duration::from_secs(5), install)
                .await
                .expect("control signal must release progress backpressure")
                .unwrap();
            server.await.unwrap();
            // This shutdown flag only releases observation backpressure. The
            // direct installation's publication control remains open, proving
            // the post-publication Setup send also settles while still full.
            if shutdown_signal {
                outcome.unwrap();
                assert!(manager.version_path("b1234+cpu").exists());
            } else {
                assert!(matches!(outcome, Err(PumasError::DownloadCancelled)));
                assert!(!manager.version_path("b1234+cpu").exists());
            }
            assert!(
                matches!(receiver.recv().await, Some(ProgressUpdate::Setup { message }) if message == "full")
            );
            assert!(receiver.recv().await.is_none());
            manager.shutdown_installations().await.unwrap();
            acquisition.shutdown().await.unwrap();
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_shutdown_cancels_stalled_transfer_and_retains_failed_outcome() {
        let root = TempDir::new().unwrap();
        let (server, observed, release, base_url) = native_archive_fixture(root.path()).await;
        let api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let mut manager = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        manager.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                manager.cache_dir(),
                Duration::from_secs(3600),
                base_url,
            )
            .unwrap(),
        );
        let _updates = manager.install_version("b1234+cpu").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), observed)
            .await
            .unwrap()
            .unwrap();
        let error = tokio::time::timeout(Duration::from_secs(2), manager.shutdown_installations())
            .await
            .unwrap()
            .unwrap_err()
            .to_string();
        assert!(error.to_ascii_lowercase().contains("cancelled"), "{error}");
        assert_eq!(
            manager
                .shutdown_installations()
                .await
                .unwrap_err()
                .to_string(),
            error
        );
        release.send(false).unwrap();
        server.await.unwrap();
        assert!(!manager.version_path("b1234+cpu").exists());
        assert!(manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        assert!(std::fs::read_dir(manager.versions_dir())
            .unwrap()
            .any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".llama-install-")
            }));
        api.shutdown_intent().await.unwrap();
        api.shutdown_acquisition().await.unwrap();
        assert!(!manager.is_installing().await);
        assert!(manager.get_installation_progress().await.unwrap().success == Some(false));
    }

    #[tokio::test]
    async fn native_shutdown_drains_registered_tasks_and_retains_failure() {
        let (manager, _root) = create_test_manager().await;
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            entered_tx.send(()).unwrap();
            release_rx.await.unwrap();
            Err("native terminal failure".into())
        });
        manager.installation_tasks.lock().unwrap().tasks.push(task);
        entered_rx.await.unwrap();
        let shutting = manager.clone();
        let waiter = tokio::spawn(async move { shutting.shutdown_installations().await });
        while !manager.torch_shutting_down.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        let repeated = manager.shutdown_installations();
        tokio::pin!(repeated);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut repeated)
                .await
                .is_err()
        );
        release_tx.send(()).unwrap();
        let error = repeated.await.unwrap_err().to_string();
        assert!(error.contains("native terminal failure"));
        assert_eq!(
            manager
                .shutdown_installations()
                .await
                .unwrap_err()
                .to_string(),
            error
        );
        assert!(manager.install_version("v-new").await.is_err());
    }

    #[tokio::test]
    async fn native_cancelled_shutdown_waiter_does_not_interrupt_cancellation() {
        let (manager, _root) = create_test_manager().await;
        *manager.installing_tag.lock().await = Some("owned-attempt".into());
        let tracker = manager.progress_tracker.write().await;
        let (completed, observed) = tokio::sync::oneshot::channel();
        let flag = manager.cancel_flag.clone();
        manager
            .installation_tasks
            .lock()
            .unwrap()
            .tasks
            .push(tokio::spawn(async move {
                wait_for_install_cancel(flag).await;
                completed.send(()).unwrap();
                Ok(())
            }));
        let owner = manager.clone();
        let waiter = tokio::spawn(async move { owner.shutdown_installations().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !manager.cancel_flag.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        drop(tracker);
        tokio::time::timeout(Duration::from_secs(2), observed)
            .await
            .unwrap()
            .unwrap();
        // The retained worker drains without requiring another shutdown caller.
        tokio::time::timeout(Duration::from_secs(2), async {
            while !manager.installation_tasks.lock().unwrap().tasks.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        manager.shutdown_installations().await.unwrap();
    }

    async fn create_torch_test_manager() -> (VersionManager, TempDir) {
        let root = TempDir::new().unwrap();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        (manager, root)
    }

    #[tokio::test]
    async fn torch_choices_and_selection_token_are_local_fast_and_single_use() {
        let (manager, root) = create_torch_test_manager().await;
        let options = tokio::time::timeout(Duration::from_secs(2), manager.torch_runtime_options())
            .await
            .expect("local Torch options must return promptly")
            .unwrap();
        assert_eq!(options["defaultBuild"], "auto");
        assert_eq!(options["pythons"][0]["id"], "auto");
        assert!(!root.path().join("launcher-data/managed-python").exists());
        let release_options = tokio::time::timeout(
            Duration::from_secs(2),
            manager.discover_torch_release_options("v2.14.0"),
        )
        .await
        .expect("advisory release options must return promptly")
        .unwrap();
        assert_eq!(
            release_options.status,
            TorchReleaseOptionsStatus::Inconclusive
        );
        assert!(!release_options.complete_scan);
        assert!(!root.path().join("launcher-data/managed-python").exists());

        let outcome = tokio::time::timeout(
            Duration::from_secs(2),
            manager.preview_torch_runtime("v2.14.0", "auto", "auto", "none"),
        )
        .await
        .expect("selection token creation must return promptly")
        .unwrap();
        let TorchPreviewOutcome::Ready { preview } = outcome else {
            panic!("a valid selection must be ready without artifact resolution");
        };
        assert!(preview.artifacts.is_empty());
        assert_eq!(preview.qualification, "unverified");
        assert!(!root.path().join("launcher-data/managed-python").exists());
        assert!(manager
            .consume_torch_install_selection(&preview.preview_id, "v2.14.1")
            .await
            .is_err());

        let TorchPreviewOutcome::Ready { preview } = manager
            .preview_torch_runtime("v2.14.0", "auto", "auto", "none")
            .await
            .unwrap()
        else {
            panic!("a valid selection must be ready");
        };
        assert!(manager
            .consume_torch_install_selection(&preview.preview_id, "v2.14.0")
            .await
            .is_ok());
        assert!(manager
            .consume_torch_install_selection(&preview.preview_id, "v2.14.0")
            .await
            .is_err());

        manager.torch_install_selections.lock().await.insert(
            "expired-selection".into(),
            torch_preview::TorchInstallSelection {
                tag: "v2.14.0".into(),
                build: "auto".into(),
                python: "auto".into(),
                adapter: "none".into(),
                created: std::time::Instant::now()
                    - torch_preview::PREVIEW_TTL
                    - Duration::from_secs(1),
            },
        );
        assert!(manager
            .consume_torch_install_selection("expired-selection", "v2.14.0")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn torch_admission_returns_during_tracker_contention_then_enters_visible_cancellable_state(
    ) {
        let (manager, _root) = create_torch_test_manager().await;
        let tracker_lock = manager.progress_tracker.write().await;
        let mut updates =
            tokio::time::timeout(Duration::from_secs(2), manager.install_version("v2.14.0"))
                .await
                .expect("install admission must return before network resolution")
                .unwrap();
        assert!(manager.is_installing().await);
        let cancel_manager = manager.clone();
        let cancel = tokio::spawn(async move { cancel_manager.cancel_installation().await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !manager.cancel_flag.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancellation must be recorded while progress tracking is contended");
        drop(tracker_lock);
        assert!(cancel.await.unwrap().unwrap());
        assert!(matches!(
            updates.recv().await,
            Some(ProgressUpdate::Setup { message }) if message.contains("resolving Torch packages")
        ));
        let progress = manager.get_installation_progress().await.unwrap();
        assert!(matches!(
            progress.stage,
            Some(
                pumas_library::models::InstallationStage::Resolving
                    | pumas_library::models::InstallationStage::Download
            )
        ));
        assert_eq!(progress.overall_progress, Some(0.0));
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(5), updates.recv())
                .await
                .unwrap(),
            Some(ProgressUpdate::Error { message }) if message.contains("cancelled")
        ));
        assert!(!manager.is_installing().await);
    }

    #[tokio::test]
    async fn torch_direct_install_enters_stage_without_release_cache_or_network_lookup() {
        let (manager, root) = create_torch_test_manager().await;
        let cache = root.path().join("launcher-data/cache");
        assert!(std::fs::read_dir(&cache).unwrap().next().is_none());
        let reached = Arc::new(AtomicBool::new(false));
        let reached_in_stage = reached.clone();
        let manager = manager.with_torch_stage_override(move |_| {
            reached_in_stage.store(true, Ordering::SeqCst);
            Err(PumasError::InstallationFailed {
                message: "stage reached without release lookup".into(),
            })
        });
        let mut updates = manager.install_version("v2.14.0").await.unwrap();
        let error = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(ProgressUpdate::Error { message }) = updates.recv().await {
                    break message;
                }
            }
        })
        .await
        .expect("Torch install should reach staging without release lookup");
        assert!(reached.load(Ordering::SeqCst));
        assert!(error.contains("stage reached without release lookup"));
        assert!(!cache.join("github-releases-pytorch-pytorch.json").exists());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancelling_install_resolution_drains_owned_resolver_process_group() {
        let workspace = Arc::new(tempfile::tempdir().unwrap());
        let pid_path = workspace.path().join("install-resolver.pid");
        let cleanup = Arc::new(installer::TorchCleanupTasks::default());
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut command = tokio::process::Command::new("sh");
        command
            .arg("-c")
            .arg("echo $$ > \"$1\"; sleep 30")
            .arg("sh")
            .arg(&pid_path);
        let operation_workspace = workspace.clone();
        let operation_cleanup = cleanup.clone();
        let operation = async move {
            let run = torch_preview::run_preview_resolver(
                command,
                &operation_workspace,
                Duration::from_secs(30),
                &operation_cleanup,
            )
            .await?;
            Err::<(), _>(PumasError::Other(format!(
                "Unexpected resolver completion: {run:?}"
            )))
        };
        let cancel_for_task = cancel_flag.clone();
        let cancel_task = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !pid_path.exists() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            cancel_for_task.store(true, Ordering::SeqCst);
        });
        let result = run_torch_operation_with_cancel(operation, cancel_flag, cleanup.clone()).await;
        cancel_task.await.unwrap();
        assert!(matches!(
            result,
            Err(PumasError::InstallationFailed { message }) if message.contains("cancelled")
        ));
        let pid: i32 = std::fs::read_to_string(workspace.path().join("install-resolver.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(!pumas_library::platform::linux_group::group_has_live_members(pid).unwrap());
        cleanup.drain().await.unwrap();
    }

    #[test]
    fn torch_startup_defers_only_lock_contention() {
        let deferred =
            VersionManager::startup_torch_versions_lock(Err(std::io::ErrorKind::WouldBlock.into()))
                .unwrap();
        assert!(deferred.is_none());

        let error = VersionManager::startup_torch_versions_lock(Err(
            std::io::ErrorKind::PermissionDenied.into(),
        ))
        .err()
        .unwrap();
        assert!(matches!(error, PumasError::Io { source: Some(source), .. }
            if source.kind() == std::io::ErrorKind::PermissionDenied));
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
        let runtime = manager.version_path(tag);
        std::fs::create_dir_all(&runtime).unwrap();
        let python = pumas_library::platform::paths::venv_python(&runtime);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        for required in ["runtime.json", "serve.py", "requirements.txt"] {
            std::fs::write(runtime.join(required), b"test fixture").unwrap();
        }
        std::fs::write(&python, b"test fixture").unwrap();
        if manager.app_id == AppId::Torch {
            std::fs::write(
                runtime.join("resolution.json"),
                r#"{"torch":"test-fixture"}"#,
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::write(&python, b"#!/bin/sh\necho test-fixture\n").unwrap();
                std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            #[cfg(windows)]
            {
                // Build a real PE executable without depending on installed Python.
                // The containing test TempDir owns both source and executable.
                let source = runtime.join("identity_fixture.rs");
                std::fs::write(&source, r#"fn main() { println!("test-fixture"); }"#).unwrap();
                let mut compiler = tokio::process::Command::new("rustc");
                compiler
                    .kill_on_drop(true)
                    .args(["--crate-name", "torch_identity_fixture", "--edition=2021"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&python);
                let output = tokio::time::timeout(Duration::from_secs(60), compiler.output())
                    .await
                    .expect("Windows fixture compilation timed out")
                    .expect("Windows fixture compiler must run");
                assert!(
                    output.status.success(),
                    "Windows fixture compilation failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
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

    #[cfg(unix)]
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
            pumas_library::platform::paths::venv_python(&runtime),
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

    #[tokio::test]
    async fn torch_removal_preserves_version_while_independent_install_owner_holds_lock() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "remove").await;
        manager.set_active_version("keep").await.unwrap();
        let other_manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();

        assert!(other_manager.remove_version("remove").await.is_err());
        assert!(other_manager.version_path("remove").exists());
        assert!(other_manager
            .metadata_manager
            .get_installed_version("remove", Some(AppId::Torch))
            .unwrap()
            .is_some());

        drop(owner);
        assert!(other_manager.remove_version("remove").await.unwrap());
        assert!(!other_manager.version_path("remove").exists());
        assert!(other_manager
            .metadata_manager
            .get_installed_version("remove", Some(AppId::Torch))
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn torch_mutations_wait_for_short_cross_process_handoffs() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "remove").await;
        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());

        for operation in ["active", "default", "remove"] {
            let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();
            let release = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(40)).await;
                drop(owner);
            });
            match operation {
                "active" => assert!(manager.set_active_version("keep").await.unwrap()),
                "default" => assert!(manager.set_default_version(Some("keep")).await.unwrap()),
                "remove" => assert!(manager.remove_version("remove").await.unwrap()),
                _ => unreachable!(),
            }
            release.await.unwrap();
        }
    }

    #[tokio::test]
    async fn torch_selection_and_validation_leave_metadata_untouched_while_owner_holds_lock() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "other").await;
        manager.set_active_version("keep").await.unwrap();
        let observer = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();

        assert!(observer.set_active_version("other").await.is_err());
        assert!(observer.set_default_version(Some("other")).await.is_err());
        assert!(observer.validate_installations().await.is_err());
        let versions = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(versions.installed.contains_key("keep"));
        assert!(versions.installed.contains_key("other"));
        assert_eq!(versions.last_selected_version.as_deref(), Some("keep"));
        assert_eq!(
            std::fs::read_to_string(manager.active_version_file()).unwrap(),
            "keep"
        );

        drop(owner);
        assert!(observer.set_default_version(Some("other")).await.unwrap());
        assert!(observer.set_active_version("other").await.unwrap());
        let versions = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(versions.installed.contains_key("keep"));
        assert_eq!(versions.default_version.as_deref(), Some("other"));
        assert_eq!(versions.last_selected_version.as_deref(), Some("other"));
    }

    #[tokio::test]
    async fn stale_torch_manager_selection_preserves_new_and_removed_installs() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        manager.set_active_version("keep").await.unwrap();
        let stale = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();

        register_test_version(&manager, "new").await;
        assert!(stale.set_default_version(Some("keep")).await.unwrap());
        let versions = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(versions.installed.contains_key("new"));

        manager.remove_version("new").await.unwrap();
        assert!(stale.set_active_version("keep").await.unwrap());
        let versions = manager
            .metadata_manager
            .load_versions(Some(AppId::Torch))
            .unwrap();
        assert!(!versions.installed.contains_key("new"));
        assert!(versions.installed.contains_key("keep"));
    }

    #[tokio::test]
    async fn torch_getters_refresh_selection_after_external_commit_and_remain_responsive_when_busy()
    {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "other").await;
        manager.set_active_version("keep").await.unwrap();
        manager.set_default_version(Some("keep")).await.unwrap();
        let observer = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        observer.set_active_version("other").await.unwrap();
        observer.set_default_version(Some("other")).await.unwrap();

        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(250), manager.get_active_version())
                .await
                .unwrap()
                .unwrap()
                .as_deref(),
            Some("keep")
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(250), manager.get_default_version())
                .await
                .unwrap()
                .unwrap()
                .as_deref(),
            Some("keep")
        );
        drop(owner);

        assert_eq!(
            manager.get_active_version().await.unwrap().as_deref(),
            Some("other")
        );
        assert_eq!(
            manager.get_default_version().await.unwrap().as_deref(),
            Some("other")
        );
        let status = manager.get_version_status().await.unwrap();
        assert_eq!(status.active_version.as_deref(), Some("other"));
        assert_eq!(status.default_version.as_deref(), Some("other"));
    }

    #[tokio::test]
    async fn torch_runtime_options_refresh_installs_and_removals_from_another_manager() {
        let (observer, root) = create_torch_test_manager().await;
        let owner = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        register_test_version(&owner, "v2.14.0").await;
        let options = observer.torch_runtime_options().await.unwrap();
        assert_eq!(options["installed"][0]["tag"], "v2.14.0");
        owner.remove_version("v2.14.0").await.unwrap();
        let options = observer.torch_runtime_options().await.unwrap();
        assert_eq!(options["installed"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn torch_status_uses_one_metadata_generation_while_versions_lock_is_busy() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "keep").await;
        register_test_version(&manager, "other").await;
        let before = manager.get_version_status().await.unwrap();
        let old_date = before
            .versions
            .iter()
            .find(|version| version.tag == "keep")
            .unwrap()
            .installed_date
            .clone();
        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();
        manager
            .metadata_manager
            .update_installed_version(
                "keep",
                pumas_library::metadata::InstalledVersionMetadata {
                    path: "keep".into(),
                    release_tag: "keep".into(),
                    installed_date: "2030-01-01T00:00:00Z".into(),
                    ..Default::default()
                },
                Some(AppId::Torch),
            )
            .unwrap();
        manager
            .metadata_manager
            .remove_installed_version("other", Some(AppId::Torch))
            .unwrap();

        let busy = manager.get_version_status().await.unwrap();
        assert_eq!(busy.versions.len(), 2);
        assert_eq!(
            busy.versions
                .iter()
                .find(|version| version.tag == "keep")
                .unwrap()
                .installed_date,
            old_date
        );
        drop(owner);
        let after = manager.get_version_status().await.unwrap();
        assert_eq!(after.versions.len(), 1);
        assert_eq!(after.versions[0].tag, "keep");
        assert_eq!(
            after.versions[0].installed_date.as_deref(),
            Some("2030-01-01T00:00:00Z")
        );
    }

    #[tokio::test]
    async fn torch_startup_ignores_transitional_active_marker_while_owner_holds_lock() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "old").await;
        register_test_version(&manager, "new").await;
        manager.set_active_version("old").await.unwrap();
        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();
        std::fs::write(manager.active_version_file(), "new").unwrap();

        let observer = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert_eq!(
            observer.get_active_version().await.unwrap().as_deref(),
            Some("old")
        );
        let error = observer.state.write().await.refresh().await.unwrap_err();
        assert!(
            matches!(error, PumasError::Io { source: Some(source), .. } if source.kind() == std::io::ErrorKind::WouldBlock)
        );
        assert_eq!(
            observer.get_active_version().await.unwrap().as_deref(),
            Some("old")
        );

        manager
            .metadata_manager
            .set_last_selected_version(Some("new"), Some(AppId::Torch))
            .unwrap();
        drop(owner);
        assert_eq!(
            observer.get_active_version().await.unwrap().as_deref(),
            Some("new")
        );
    }

    #[tokio::test]
    async fn torch_busy_startup_prefers_last_selected_over_default() {
        let (manager, root) = create_torch_test_manager().await;
        register_test_version(&manager, "old").await;
        register_test_version(&manager, "other").await;
        manager.set_active_version("old").await.unwrap();
        manager.set_default_version(Some("other")).await.unwrap();

        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();
        let observer = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert_eq!(
            observer.get_active_version().await.unwrap().as_deref(),
            Some("old")
        );
        drop(owner);
        assert_eq!(
            observer.get_active_version().await.unwrap().as_deref(),
            Some("old")
        );
    }

    #[tokio::test]
    async fn torch_startup_skips_stale_validation_when_install_owner_holds_lock() {
        let (manager, root) = create_torch_test_manager().await;
        manager
            .metadata_manager
            .update_installed_version(
                "incomplete",
                pumas_library::metadata::InstalledVersionMetadata {
                    path: "incomplete".into(),
                    release_tag: "incomplete".into(),
                    ..Default::default()
                },
                Some(AppId::Torch),
            )
            .unwrap();
        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        let owner = installer::TorchVersionsLock::try_acquire(&versions_dir).unwrap();

        let observer = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap();
        assert!(manager
            .metadata_manager
            .get_installed_version("incomplete", Some(AppId::Torch))
            .unwrap()
            .is_some());
        drop(owner);
        let removed = observer.validate_installations().await.unwrap();
        assert!(removed.removed_tags.contains(&"incomplete".to_string()));
        assert!(manager
            .metadata_manager
            .get_installed_version("incomplete", Some(AppId::Torch))
            .unwrap()
            .is_none());
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
    async fn empty_cached_torch_release_list_does_not_gate_installation() {
        let root = TempDir::new().unwrap();
        let cache = root.path().join("launcher-data/cache");
        let releases = pumas_library::network::ReleasesCache::new(cache, Duration::from_secs(3600));
        releases.set_disk(AppId::Torch.github_repo(), &[]).unwrap();
        let reached = Arc::new(AtomicBool::new(false));
        let reached_in_stage = reached.clone();
        let manager = VersionManager::new(root.path(), AppId::Torch)
            .await
            .unwrap()
            .with_torch_stage_override(move |_| {
                reached_in_stage.store(true, Ordering::SeqCst);
                Err(PumasError::InstallationFailed {
                    message: "stage reached after empty release cache".into(),
                })
            });
        let mut updates = manager.install_version("v2.10.0").await.unwrap();
        assert!(matches!(
            updates.recv().await,
            Some(ProgressUpdate::Setup { .. })
        ));
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(5), updates.recv())
                .await
                .unwrap(),
            Some(ProgressUpdate::Error { message }) if message.contains("stage reached after empty release cache")
        ));
        assert!(reached.load(Ordering::SeqCst));
        assert!(!manager.is_installing().await);
        assert!(manager
            .get_installation_progress()
            .await
            .is_some_and(|progress| progress.success == Some(false)));
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
