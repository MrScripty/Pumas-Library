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

mod cohere_asr_profile;
mod constraints;
mod dependencies;
mod installer;
mod launcher;
mod managed_depot_lease;
mod managed_python;
pub mod ollama;
mod operation_receipt;
mod progress;
pub mod size_calculator;
mod state;
mod torch_alternatives;
mod torch_preview;
mod torch_read_source;
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

/// The timer belongs to one completion, including repeated attempts of a tag.
#[derive(Clone, PartialEq, Eq)]
struct CompletedProgressIdentity {
    tag: String,
    started_at: String,
    completed_at: String,
}

impl CompletedProgressIdentity {
    fn capture(progress: &InstallationProgress) -> Option<Self> {
        Some(Self {
            tag: progress.tag.clone()?,
            started_at: progress.started_at.clone()?,
            completed_at: progress.completed_at.clone()?,
        })
    }
}

async fn clear_matching_completed_progress(
    tracker: &mut InstallationProgressTracker,
    app_id: AppId,
    expected: Option<CompletedProgressIdentity>,
) {
    let Some(expected) = expected else {
        return;
    };
    let Some(current) = tracker.get_current_state() else {
        return;
    };
    if CompletedProgressIdentity::capture(&current).as_ref() != Some(&expected)
        || (app_id == AppId::LlamaCpp && installer::has_native_cleanup_pending(&current))
    {
        return;
    }
    tracker.clear_completed_state_async().await;
}

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
    interrupt_after_native_rename: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    park_after_native_rename_marker: Option<PathBuf>,
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
        Self::new_with_github_client(launcher_root.into(), app_id, None).await
    }

    async fn new_with_github_client(
        launcher_root: PathBuf,
        app_id: AppId,
        configured_client: Option<Arc<GitHubClient>>,
    ) -> Result<Self> {
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
            let github_client = match configured_client {
                Some(client) => client,
                None => Arc::new(GitHubClient::new(cache_dir_for_setup)?),
            };
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
            interrupt_after_native_rename: None,
            #[cfg(test)]
            park_after_native_rename_marker: None,
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
        Self::new_with_acquisition_client(launcher_root.into(), app_id, acquisition, None).await
    }

    async fn new_with_acquisition_client(
        launcher_root: PathBuf,
        app_id: AppId,
        acquisition: Arc<AcquisitionService>,
        configured_client: Option<Arc<GitHubClient>>,
    ) -> Result<Self> {
        if app_id != AppId::LlamaCpp {
            return Err(PumasError::Config {
                message: "Shared artifact acquisition is currently required for llama.cpp".into(),
            });
        }
        let mut manager =
            Self::new_with_github_client(launcher_root, app_id, configured_client).await?;
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
        .with_acquisition_consumer(manager.acquisition_consumer.clone())
        .with_github_client(manager.github_client.clone());
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

    /// Construct a real native-install integration fixture on literal loopback.
    ///
    /// This non-default seam uses an isolated caller-owned root and the existing
    /// shared acquisition owner. The GitHub client has no token/credential loader;
    /// the source URL rejects user information, queries, fragments, and DNS names.
    /// No environment or product runtime setting selects this constructor.
    #[cfg(feature = "test-support")]
    pub async fn new_with_loopback_acquisition_fixture(
        launcher_root: impl Into<PathBuf>,
        acquisition: Arc<AcquisitionService>,
        api_base: String,
    ) -> Result<Self> {
        let launcher_root = launcher_root.into();
        // Validate before opening any installation owner or touching its store.
        let client = GitHubClient::with_loopback_api(
            launcher_root
                .join("launcher-data")
                .join(PathsConfig::CACHE_DIR_NAME),
            Duration::from_secs(3600),
            api_base,
        )?;
        Self::new_with_acquisition_client(
            launcher_root,
            AppId::LlamaCpp,
            acquisition,
            Some(Arc::new(client)),
        )
        .await
    }

    /// Observe the monotonic admission fence without waiting behind the install
    /// lock held by shutdown. This observation never authorizes a new operation.
    #[cfg(feature = "test-support")]
    pub fn acquisition_fixture_admission_closed(&self) -> bool {
        self.torch_shutting_down.load(Ordering::SeqCst)
    }

    /// Hold a synthetic finite effect in this manager's existing consumer scope.
    /// This fixture does not create an owner or grant filesystem authority.
    #[cfg(feature = "test-support")]
    pub async fn run_acquisition_fixture_effect<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.acquisition_consumer
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: "Native acquisition fixture requires the composed consumer".into(),
            })?
            .run_blocking("native integration fixture barrier", work)
            .await
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
        let installer = if let Some(once) = &self.interrupt_after_native_rename {
            installer.with_native_rename_interruption(once.clone())
        } else {
            installer
        };
        #[cfg(test)]
        let installer = if let Some(marker) = &self.park_after_native_rename_marker {
            installer.with_native_rename_park_marker(marker.clone())
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
            let completed_progress = progress_tracker
                .read()
                .await
                .get_current_state()
                .as_ref()
                .and_then(CompletedProgressIdentity::capture);
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
                        let mut tracker = progress_tracker.write().await;
                        clear_matching_completed_progress(
                            &mut tracker, app_id, completed_progress,
                        ).await;
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
        let cleanup_lock = native_versions_lock.clone();
        let remove = move || {
            if let Some(lock) = cleanup_lock {
                installer::mark_native_attempt_removed(&versions_for_removal, &removed_tag, lock)?
                    .into_result()?;
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
        let (server, observed, release, base_url, _requests, _stop_server, _digest) =
            native_archive_fixture_inner(root, false, true).await;
        (server, observed, release, base_url)
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    async fn native_archive_fixture_with_request_monitor(
        root: &Path,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<bool>,
        String,
        tokio::sync::mpsc::UnboundedReceiver<String>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (server, observed, release, base_url, requests, stop_server, _digest) =
            native_archive_fixture_inner(root, true, true).await;
        (server, observed, release, base_url, requests, stop_server)
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    async fn native_archive_fixture_without_server_with_request_monitor(
        root: &Path,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<bool>,
        String,
        tokio::sync::mpsc::UnboundedReceiver<String>,
        tokio::sync::oneshot::Sender<()>,
        String,
    ) {
        native_archive_fixture_inner(root, true, false).await
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    async fn native_archive_fixture_inner(
        root: &Path,
        monitor_requests: bool,
        include_server_binary: bool,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<bool>,
        String,
        tokio::sync::mpsc::UnboundedReceiver<String>,
        tokio::sync::oneshot::Sender<()>,
        String,
    ) {
        use sha2::Digest;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(compressed);
        if include_server_binary {
            let payload = b"#!/bin/sh\nprintf 'native-fixture'\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(payload.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            archive
                .append_data(&mut header, "distribution/llama-server", &payload[..])
                .unwrap();
        } else {
            let payload = b"native fixture README\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(payload.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(&mut header, "distribution/README.txt", &payload[..])
                .unwrap();
        }
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        let size = bytes.len() as u64;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let url = format!("{base_url}/archive");
        let digest = format!("{:x}", sha2::Sha256::digest(&bytes));
        let publisher_digest = digest.clone();
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
        let (request_tx, request_rx) = tokio::sync::mpsc::unbounded_channel();
        let (stop_server, stop_server_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            // Real HTTP release metadata supplies the exact publisher asset ID
            // and SHA; cached discovery has neither and cannot authorize bytes.
            let (mut metadata_stream, _) = if monitor_requests {
                tokio::time::timeout(Duration::from_secs(10), listener.accept())
                    .await
                    .expect("metadata request accept timed out")
                    .unwrap()
            } else {
                listener.accept().await.unwrap()
            };
            let mut request = [0; 4096];
            let read = if monitor_requests {
                tokio::time::timeout(Duration::from_secs(10), metadata_stream.read(&mut request))
                    .await
                    .expect("metadata request read timed out")
                    .unwrap()
            } else {
                metadata_stream.read(&mut request).await.unwrap()
            };
            let request_line = String::from_utf8_lossy(&request[..read])
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned();
            assert!(request_line
                .starts_with("GET /repos/ggml-org/llama.cpp/releases/tags/b1234 HTTP/1.1"));
            if monitor_requests {
                request_tx.send(request_line).unwrap();
            }
            let body = serde_json::to_vec(&serde_json::json!({
                "tag_name": "b1234", "assets": [{
                    "id": 1234, "name": "llama-b1234-bin-ubuntu-x64.tar.gz",
                    "size": size, "browser_download_url": url,
                "digest": format!("sha256:{publisher_digest}")
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
            let (mut stream, _) = if monitor_requests {
                tokio::time::timeout(Duration::from_secs(10), listener.accept())
                    .await
                    .expect("archive request accept timed out")
                    .unwrap()
            } else {
                listener.accept().await.unwrap()
            };
            let mut request = [0; 4096];
            let read = if monitor_requests {
                tokio::time::timeout(Duration::from_secs(10), stream.read(&mut request))
                    .await
                    .expect("archive request read timed out")
                    .unwrap()
            } else {
                stream.read(&mut request).await.unwrap()
            };
            assert!(read > 0);
            if monitor_requests {
                request_tx
                    .send(
                        String::from_utf8_lossy(&request[..read])
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .to_owned(),
                    )
                    .unwrap();
            }
            entered.send(()).unwrap();
            let send_archive = if monitor_requests {
                tokio::time::timeout(Duration::from_secs(10), wait)
                    .await
                    .expect("source fixture release timed out")
                    .unwrap()
            } else {
                wait.await.unwrap()
            };
            if send_archive {
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
            if monitor_requests {
                let mut stop_server_rx = stop_server_rx;
                loop {
                    tokio::select! {
                        _ = &mut stop_server_rx => {
                            let drain_deadline = tokio::time::Instant::now()
                                + Duration::from_secs(1);
                            loop {
                                let remaining = drain_deadline
                                    .saturating_duration_since(tokio::time::Instant::now());
                                if remaining.is_zero() {
                                    break;
                                }
                                match tokio::time::timeout(remaining, listener.accept()).await {
                                    Ok(Ok((mut stream, _))) => {
                                        request_tx
                                            .send("monitored-source connection accepted".into())
                                            .unwrap();
                                        let _ = tokio::time::timeout(
                                            Duration::from_millis(250),
                                            stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                                        ).await;
                                    }
                                    Ok(Err(error)) => panic!("source monitor drain failed: {error}"),
                                    Err(_) => break,
                                }
                            }
                            break;
                        }
                        accepted = listener.accept() => {
                            let (mut stream, _) = accepted.unwrap();
                            request_tx
                                .send("monitored-source connection accepted".into())
                                .unwrap();
                            let _ = tokio::time::timeout(
                                Duration::from_millis(250),
                                stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                            ).await;
                        }
                    }
                }
            }
        });
        (
            server,
            observed,
            release,
            base_url,
            request_rx,
            stop_server,
            digest,
        )
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

    #[tokio::test]
    async fn native_completed_progress_clears_only_its_own_nonpending_completion() {
        let root = TempDir::new().unwrap();
        let mut tracker = InstallationProgressTracker::new(root.path().to_owned());
        tracker.start_installation("ordinary", None, None, None);
        tracker.complete_installation(true);
        let ordinary = tracker
            .get_current_state()
            .as_ref()
            .and_then(CompletedProgressIdentity::capture);
        clear_matching_completed_progress(&mut tracker, AppId::LlamaCpp, ordinary).await;
        assert!(tracker.get_current_state().is_none());

        // A tag-only fence would clear these newer same-tag completions. Vary
        // each timestamp independently, without relying on wall-clock precision.
        for old_start in [true, false] {
            tracker.start_installation("same-tag", None, None, None);
            tracker.complete_installation(true);
            let current = tracker.get_current_state().unwrap();
            let current_identity = CompletedProgressIdentity::capture(&current).unwrap();
            let mut old_identity = current_identity.clone();
            if old_start {
                old_identity.started_at = "1970-01-01T00:00:00+00:00".into();
            } else {
                old_identity.completed_at = "1970-01-01T00:00:00+00:00".into();
            }
            clear_matching_completed_progress(&mut tracker, AppId::LlamaCpp, Some(old_identity))
                .await;
            assert!(tracker.get_current_state().is_some());
            clear_matching_completed_progress(
                &mut tracker,
                AppId::LlamaCpp,
                Some(current_identity),
            )
            .await;
            assert!(tracker.get_current_state().is_none());
        }

        for old_pending in [false, true] {
            for new_pending in [false, true] {
                tracker.start_installation("first", None, None, None);
                if old_pending {
                    tracker.update_stage(
                        pumas_library::models::InstallationStage::Setup,
                        100.0,
                        Some("Installed output verified; staging cleanup pending: first"),
                    );
                }
                tracker.complete_installation(true);
                let old = tracker
                    .get_current_state()
                    .as_ref()
                    .and_then(CompletedProgressIdentity::capture);
                if old_pending {
                    clear_matching_completed_progress(&mut tracker, AppId::LlamaCpp, old.clone())
                        .await;
                    assert!(tracker.get_current_state().is_some());
                }
                tracker.start_installation("unrelated-newer", None, None, None);
                if new_pending {
                    tracker.update_stage(
                        pumas_library::models::InstallationStage::Setup,
                        100.0,
                        Some("Installed output verified; staging cleanup pending: newer"),
                    );
                }
                tracker.complete_installation(true);
                let newer = tracker.get_current_state().unwrap();
                let expected = CompletedProgressIdentity::capture(&newer);
                clear_matching_completed_progress(&mut tracker, AppId::LlamaCpp, old).await;
                let observed = tracker.get_current_state().unwrap();
                assert_eq!(observed.tag, newer.tag);
                assert_eq!(observed.current_item, newer.current_item);
                clear_matching_completed_progress(&mut tracker, AppId::LlamaCpp, expected).await;
                assert_eq!(tracker.get_current_state().is_some(), new_pending);
            }
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_adopted_output_survives_legacy_or_mismatched_cleanup_custody() {
        for legacy in [false, true] {
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
            release.send(true).unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    match updates.recv().await.unwrap() {
                        ProgressUpdate::Completed { success: true } => break,
                        ProgressUpdate::Error { message } => {
                            panic!("installation failed: {message}")
                        }
                        _ => {}
                    }
                }
            })
            .await
            .unwrap();
            server.await.unwrap();
            manager.shutdown_installations().await.unwrap();
            api.shutdown_acquisition().await.unwrap();
            let versions = manager.versions_dir();
            let output = manager.version_path("b1234+cpu").join("bin/llama-server");
            let output_before = std::fs::read(&output).unwrap();
            let store_path = root.path().join("launcher-data/downloads.json");
            let store_before = std::fs::read(&store_path).unwrap();
            let store: serde_json::Value = serde_json::from_slice(&store_before).unwrap();
            let record = store["acquisitions"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .unwrap();
            assert_eq!(record["phase"]["state"], "adopted");
            let stage = versions.join(record["workspace"]["relative_target"].as_str().unwrap());
            assert!(!stage.exists());
            std::fs::create_dir(&stage).unwrap();
            std::fs::write(stage.join("sentinel"), b"unowned retained stage").unwrap();
            let identity_path = std::fs::read_dir(&versions)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".llama-attempt-")
                })
                .unwrap();
            let mut identity: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&identity_path).unwrap()).unwrap();
            if legacy {
                identity["schema_version"] = serde_json::json!(1);
                identity.as_object_mut().unwrap().remove("binding");
                identity.as_object_mut().unwrap().remove("cleanup_pending");
            } else {
                // Deterministic mismatch even on a filesystem reusing the
                // removed leaf's inode. This models retained v2 custody loss.
                identity["binding"]["components"][0][1] = serde_json::json!(0);
                identity["cleanup_pending"] =
                    serde_json::json!("retained cleanup requires reconciliation");
            }
            std::fs::write(&identity_path, serde_json::to_vec(&identity).unwrap()).unwrap();
            drop(manager);
            drop(api);
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
            let progress = reopened
                .progress_tracker
                .read()
                .await
                .get_current_state()
                .unwrap();
            assert_eq!(progress.success, Some(true));
            assert!(progress.error.is_none());
            assert!(progress.current_item.unwrap().contains("cleanup pending"));
            assert!(reopened
                .metadata_manager
                .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
                .unwrap()
                .is_some());
            assert_eq!(std::fs::read(&output).unwrap(), output_before);
            assert_eq!(
                std::fs::read(stage.join("sentinel")).unwrap(),
                b"unowned retained stage"
            );
            assert_eq!(std::fs::read(&store_path).unwrap(), store_before);
            reopened.shutdown_installations().await.unwrap();
            reopened_api.shutdown_acquisition().await.unwrap();
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_cancel_failed_clear_keeps_using_and_both_error_causes() {
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
            .unwrap();
        let store_path = root.path().join("launcher-data/downloads.json");
        let before: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store_path).unwrap()).unwrap();
        let record = before["acquisitions"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        let stage = manager
            .versions_dir()
            .join(record["workspace"]["relative_target"].as_str().unwrap());
        let input = record["files"][0]["path"].as_str().unwrap();
        let original_input = std::fs::read(stage.join(input)).unwrap();
        let retired = root.path().join("retired-stage");
        std::fs::rename(&stage, &retired).unwrap();
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("sentinel"), b"replacement").unwrap();
        assert!(manager.cancel_installation().await.unwrap());
        pause.resume.add_permits(1);
        let error = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match updates.recv().await.unwrap() {
                    ProgressUpdate::Error { message } => break message,
                    ProgressUpdate::Completed { success } => {
                        panic!("failed cancellation completed: {success}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        assert!(error.to_lowercase().contains("cancel"), "{error}");
        assert!(error.contains("binding changed"), "{error}");
        server.await.unwrap();
        assert!(manager.shutdown_installations().await.is_err());
        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(
            after["acquisitions"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .unwrap()["phase"]["state"],
            "using"
        );
        assert!(after["consumer_receipts"].as_object().unwrap().is_empty());
        assert_eq!(
            std::fs::read(stage.join("sentinel")).unwrap(),
            b"replacement"
        );
        assert_eq!(std::fs::read(retired.join(input)).unwrap(), original_input);
        assert!(!manager.version_path("b1234+cpu").exists());
        let _ = api.shutdown_acquisition().await;
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
    async fn native_receiptless_using_cold_reopen_preserves_custody_without_replay() {
        use sha2::Digest;
        use std::os::unix::fs::MetadataExt;

        // Include identity and write/change times: replacing or rewriting an
        // identical extracted file must still fail this zero-effect oracle.
        #[derive(Debug, PartialEq, Eq)]
        struct FileSnapshot {
            path: PathBuf,
            identity: (u64, u64, u32),
            times: (i64, i64, i64, i64),
            bytes: Vec<u8>,
        }
        fn snapshot(root: &Path) -> Vec<FileSnapshot> {
            walkdir::WalkDir::new(root)
                .sort_by_file_name()
                .into_iter()
                .map(|entry| {
                    let entry = entry.unwrap();
                    let metadata = std::fs::symlink_metadata(entry.path()).unwrap();
                    assert!(!metadata.file_type().is_symlink());
                    assert!(metadata.is_file() || metadata.is_dir());
                    FileSnapshot {
                        path: entry.path().strip_prefix(root).unwrap().to_path_buf(),
                        identity: (metadata.dev(), metadata.ino(), metadata.mode()),
                        times: (
                            metadata.mtime(),
                            metadata.mtime_nsec(),
                            metadata.ctime(),
                            metadata.ctime_nsec(),
                        ),
                        bytes: if metadata.is_file() {
                            std::fs::read(entry.path()).unwrap()
                        } else {
                            Vec::new()
                        },
                    }
                })
                .collect()
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
                base_url.clone(),
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
            .expect("extraction must finish before the completion receipt is issued");
        let store_path = root.path().join("launcher-data/downloads.json");
        let before_failure: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store_path).unwrap()).unwrap();
        assert!(before_failure["consumer_receipts"]
            .as_object()
            .unwrap()
            .is_empty());
        let record = before_failure["acquisitions"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(record["phase"]["state"], "using");
        assert_eq!(record["demand"]["consumer"], "runtime.llama.cpp");

        // Fail the preparation callback before it can issue a receipt or enter
        // cancellation withdrawal. This is a disposable retained-store replica
        // of the pre-receipt restart boundary, not a hard-process-crash claim.
        pause.resume.close();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match updates.recv().await.expect("preparation must settle") {
                    ProgressUpdate::Error { message } => {
                        assert!(
                            message.contains("Native receipt test pause closed"),
                            "{message}"
                        );
                        break;
                    }
                    ProgressUpdate::Completed { success } => {
                        panic!("receipt-free preparation completed: {success}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        assert!(manager.shutdown_installations().await.is_err());
        api.shutdown_acquisition().await.unwrap();
        let retained_bytes = std::fs::read(&store_path).unwrap();
        let retained: serde_json::Value = serde_json::from_slice(&retained_bytes).unwrap();
        assert_eq!(retained, before_failure);
        assert_eq!(retained["acquisitions"].as_object().unwrap().len(), 1);
        let workspace = manager
            .versions_dir()
            .join(record["workspace"]["relative_target"].as_str().unwrap());
        let archive = workspace.join(record["files"][0]["path"].as_str().unwrap());
        assert_eq!(
            record["files"][0]["sha256"],
            format!(
                "{:x}",
                sha2::Sha256::digest(std::fs::read(&archive).unwrap())
            )
        );
        // The installer wraps the extracted binary in a launcher. The exact
        // archive payload is checked above; later snapshots preserve the full
        // staged output without assuming its generated launcher bytes.
        assert!(!manager.version_path("b1234+cpu").exists());
        assert!(manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        let versions = manager.versions_dir();
        // Seed plausible orphan publication only after both owners have drained.
        // Matching executable bytes and installed metadata cannot authorize this
        // exact retained use without its consumer completion receipt.
        let destination = manager.version_path("b1234+cpu");
        let orphan_metadata = pumas_library::metadata::InstalledVersionMetadata {
            path: "b1234+cpu".into(),
            installed_date: "2026-09-29T00:00:00Z".into(),
            release_tag: "b1234+cpu".into(),
            release_date: Some("2026-09-29T00:00:00Z".into()),
            size: Some(record["files"][0]["bytes"].as_u64().unwrap()),
            dependencies_installed: Some(true),
            ..Default::default()
        };
        {
            let _lock = installer::NativeVersionsLock::try_acquire(&versions).unwrap();
            std::fs::create_dir_all(destination.join("bin")).unwrap();
            std::fs::copy(
                workspace.join("output/bin/llama-server"),
                destination.join("bin/llama-server"),
            )
            .unwrap();
            manager
                .metadata_manager
                .update_installed_version(
                    "b1234+cpu",
                    orphan_metadata.clone(),
                    Some(AppId::LlamaCpp),
                )
                .unwrap();
        }
        assert_eq!(
            std::fs::read(destination.join("bin/llama-server")).unwrap(),
            std::fs::read(workspace.join("output/bin/llama-server")).unwrap()
        );
        let retained_destination = snapshot(&destination);
        let retained_workspace = snapshot(&workspace);
        std::fs::write(
            versions.join("authored-sentinel"),
            b"preserve authored state",
        )
        .unwrap();
        let retained_versions = snapshot(&versions);
        let metadata = root.path().join("launcher-data/metadata");
        let metadata_path = metadata.join(format!(
            "versions-{}.json",
            AppId::LlamaCpp.to_string().to_lowercase()
        ));
        let retained_metadata_bytes = std::fs::read(&metadata_path).unwrap();
        let retained_metadata = snapshot(&metadata);
        drop(updates);
        drop(pause);
        drop(manager);
        drop(api);

        // The direct worker retains the injected client, so the loopback
        // listener can observe that recovery path. Public with_acquisition and
        // manager construction create their own clients; those paths are checked
        // for refusal below, but this listener does not observe their traffic.
        let source = tokio::net::TcpListener::bind(base_url.strip_prefix("http://").unwrap())
            .await
            .unwrap();
        let reopened_api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let new_direct = || {
            VersionInstaller::new(
                root.path().to_path_buf(),
                AppId::LlamaCpp,
                Arc::new(MetadataManager::new(root.path())),
                Arc::new(RwLock::new(InstallationProgressTracker::new(
                    root.path().join("launcher-data/cache"),
                ))),
                Arc::new(AtomicBool::new(false)),
            )
        };
        let consumer = Arc::new(
            reopened_api
                .acquisition()
                .open_consumer("runtime.llama.cpp")
                .unwrap(),
        );
        let store = reopened_api.acquisition().store().clone();
        let records = consumer
            .run_blocking("read receipt-free native fixture records", move || {
                store.acquisitions()
            })
            .await
            .unwrap();
        let direct = new_direct()
            .with_github_client(Arc::new(
                GitHubClient::with_loopback_api(
                    root.path().join("launcher-data/cache"),
                    Duration::from_secs(3600),
                    base_url.clone(),
                )
                .unwrap(),
            ))
            .with_acquisition_consumer(Some(consumer.clone()));
        let error = tokio::time::timeout(
            Duration::from_secs(5),
            direct.reconcile_retained_llama_cpp(records.into_values().collect()),
        )
        .await
        .expect("injected recovery worker must refuse without waiting for a source")
        .unwrap_err();
        assert!(
            matches!(
                &error,
                PumasError::Validation { field, message }
                    if field == "acquisition.consumer_recovery_required"
                        && message == "Retained consumer use has no authoritative completion receipt"
            ),
            "{error}"
        );
        // Drain the registered worker before inspecting the listener backlog.
        // Receipt-free durable custody can truthfully make shutdown fail.
        let _drain = tokio::time::timeout(Duration::from_secs(5), consumer.shutdown())
            .await
            .expect("injected recovery scope must drain");
        drop(direct);
        drop(consumer);
        let error = match tokio::time::timeout(
            Duration::from_secs(5),
            new_direct().with_acquisition(reopened_api.acquisition().clone()),
        )
        .await
        .expect("direct public cold recovery must refuse promptly")
        {
            Ok(_) => panic!("direct public constructor accepted receipt-free Using"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("Retained consumer use has no authoritative completion receipt"),
            "{error}"
        );
        let error = match tokio::time::timeout(
            Duration::from_secs(5),
            VersionManager::new_with_acquisition(
                root.path(),
                AppId::LlamaCpp,
                reopened_api.acquisition().clone(),
            ),
        )
        .await
        .expect("receipt-free recovery must refuse without waiting for a source")
        {
            Ok(_) => panic!("receipt-free Using must refuse cold recovery"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("Retained consumer use has no authoritative completion receipt"),
            "{error}"
        );
        // Full document equality also observes queue/admission/release state,
        // demand, manifest, verified files, lease generation and workspace ID.
        assert_eq!(std::fs::read(&store_path).unwrap(), retained_bytes);
        assert_eq!(snapshot(&versions), retained_versions);
        assert_eq!(snapshot(&workspace), retained_workspace);
        assert_eq!(snapshot(&destination), retained_destination);
        assert_eq!(
            std::fs::read(&metadata_path).unwrap(),
            retained_metadata_bytes
        );
        assert_eq!(snapshot(&metadata), retained_metadata);
        assert_eq!(
            serde_json::to_value(
                MetadataManager::new(root.path())
                    .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(orphan_metadata).unwrap()
        );

        // Missing v2 custody is a separate refusal boundary. Reopening must
        // not recreate the active attempt's leaf or reach a consumer callback;
        // orphan output, native attempt proof, and metadata remain untouched.
        std::fs::remove_dir_all(&workspace).unwrap();
        assert!(!workspace.exists());
        let missing_versions = snapshot(&versions);
        let missing_custody_consumer = Arc::new(
            reopened_api
                .acquisition()
                .open_consumer("runtime.llama.cpp")
                .unwrap(),
        );
        let store = reopened_api.acquisition().store().clone();
        let records = missing_custody_consumer
            .run_blocking("read missing-custody native fixture records", move || {
                store.acquisitions()
            })
            .await
            .unwrap();
        let missing_custody_installer = new_direct()
            .with_github_client(Arc::new(
                GitHubClient::with_loopback_api(
                    root.path().join("launcher-data/cache"),
                    Duration::from_secs(3600),
                    base_url,
                )
                .unwrap(),
            ))
            .with_acquisition_consumer(Some(missing_custody_consumer.clone()));
        let missing_custody = tokio::time::timeout(
            Duration::from_secs(5),
            missing_custody_installer.reconcile_retained_llama_cpp(records.into_values().collect()),
        )
        .await
        .expect("missing-custody recovery must stop promptly")
        .unwrap_err();
        assert!(
            matches!(
                &missing_custody,
                PumasError::Io {
                    source: Some(source),
                    ..
                } if source.kind() == std::io::ErrorKind::NotFound
            ),
            "missing custody must fail while reopening the bound stage: {missing_custody}"
        );
        assert!(
            tokio::time::timeout(Duration::from_secs(5), missing_custody_consumer.shutdown())
                .await
                .expect("missing-custody recovery scope must drain")
                .is_err()
        );
        drop(missing_custody_installer);
        drop(missing_custody_consumer);
        match tokio::time::timeout(Duration::from_millis(50), source.accept()).await {
            Err(_) => {}
            Ok(Ok(_)) => panic!("receiptless recovery contacted the source"),
            Ok(Err(error)) => panic!("source observation failed: {error}"),
        }
        drop(source);
        assert_eq!(std::fs::read(&store_path).unwrap(), retained_bytes);
        assert_eq!(snapshot(&destination), retained_destination);
        assert_eq!(
            std::fs::read(&metadata_path).unwrap(),
            retained_metadata_bytes
        );
        assert_eq!(snapshot(&metadata), retained_metadata);
        assert!(!archive.exists());
        assert!(!workspace.join("output").exists());
        assert!(
            !workspace.exists(),
            "cold reopen must not manufacture staging"
        );
        assert_eq!(snapshot(&versions), missing_versions);
        assert_eq!(
            std::fs::read(versions.join("authored-sentinel")).unwrap(),
            b"preserve authored state"
        );
        assert!(reopened_api.shutdown_acquisition().await.is_err());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_verified_archive_without_server_retains_custody_and_cold_reopen_refuses() {
        use sha2::Digest;
        use std::os::unix::fs::MetadataExt;

        type SnapshotEntry = (PathBuf, u64, u64, u32, i64, i64, i64, i64, Vec<u8>);

        struct AbortSourceOnDrop(tokio::task::JoinHandle<()>);

        impl Drop for AbortSourceOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }

        fn snapshot_workspace(
            root: &Path,
        ) -> std::result::Result<Vec<SnapshotEntry>, &'static str> {
            let mut snapshot = Vec::new();
            for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
                let entry = entry.map_err(|_| "workspace traversal failed")?;
                let metadata = std::fs::symlink_metadata(entry.path())
                    .map_err(|_| "workspace metadata lookup failed")?;
                if metadata.file_type().is_symlink() {
                    return Err("workspace unexpectedly contains a symlink");
                }
                let bytes = if metadata.is_file() {
                    std::fs::read(entry.path()).map_err(|_| "workspace file read failed")?
                } else if metadata.is_dir() {
                    Vec::new()
                } else {
                    return Err("workspace contains an unsupported file type");
                };
                snapshot.push((
                    entry
                        .path()
                        .strip_prefix(root)
                        .map_err(|_| "workspace entry escaped its root")?
                        .to_path_buf(),
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mode(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                    bytes,
                ));
            }
            Ok(snapshot)
        }

        let root = TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("launcher-data")).unwrap();
        let sentinel_path = root.path().join("launcher-data/authored-sentinel");
        std::fs::write(&sentinel_path, b"keep native authored state").unwrap();
        let acquisition = Arc::new(AcquisitionService::new(Arc::new(
            pumas_library::acquisition::AcquisitionStore::new(root.path()),
        )));
        let store = acquisition.store().clone();
        let mut manager = tokio::time::timeout(
            Duration::from_secs(8),
            VersionManager::new_with_acquisition(root.path(), AppId::LlamaCpp, acquisition.clone()),
        )
        .await
        .expect("initial native manager construction must be bounded")
        .expect("initial native manager construction must succeed");
        let (source, observed, release, base_url, mut requests, stop_source, publisher_sha256) =
            native_archive_fixture_without_server_with_request_monitor(root.path()).await;
        let mut source_task = AbortSourceOnDrop(source);
        manager.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                manager.cache_dir(),
                Duration::from_secs(3600),
                base_url.clone(),
            )
            .unwrap(),
        );

        let install_start =
            tokio::time::timeout(Duration::from_secs(5), manager.install_version("b1234+cpu"))
                .await;
        let install_started = install_start.as_ref().is_ok_and(|result| result.is_ok());
        let mut updates = match install_start {
            Ok(Ok(updates)) => Some(updates),
            _ => None,
        };
        let source_ready = tokio::time::timeout(Duration::from_secs(5), observed).await;
        let source_ready = source_ready.is_ok_and(|result| result.is_ok());
        let source_release = release.send(source_ready).is_ok();
        let install_error = if let Some(updates) = updates.as_mut() {
            tokio::time::timeout(Duration::from_secs(12), async {
                loop {
                    match updates.recv().await {
                        Some(ProgressUpdate::Error { message }) => return Some(message),
                        Some(ProgressUpdate::Completed { .. }) | None => return None,
                        Some(_) => {}
                    }
                }
            })
            .await
            .ok()
            .flatten()
        } else {
            None
        };
        let install_shutdown =
            tokio::time::timeout(Duration::from_secs(8), manager.shutdown_installations()).await;
        let version_path = manager.version_path("b1234+cpu");
        let manager_shutdown_ok = install_shutdown.is_ok_and(|result| result.is_err());
        drop(updates);
        let acquisition_shutdown =
            tokio::time::timeout(Duration::from_secs(8), acquisition.shutdown()).await;
        let default_version =
            tokio::time::timeout(Duration::from_secs(5), manager.get_default_version()).await;
        let installed_metadata = manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp));
        let sentinel_after_install = std::fs::read(&sentinel_path);
        drop(manager);
        drop(acquisition);

        let install_document_bytes = std::fs::read(root.path().join("downloads.json"));
        let install_document = install_document_bytes
            .as_ref()
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok());
        let records = store.acquisitions();
        let workspace = records
            .as_ref()
            .ok()
            .and_then(|records| records.values().next())
            .map(|record| {
                root.path()
                    .join(AppId::LlamaCpp.versions_dir_name())
                    .join(&record.workspace.relative_target)
            });
        let archive_path = records
            .as_ref()
            .ok()
            .and_then(|records| records.values().next())
            .and_then(|record| {
                let workspace = workspace.as_ref()?;
                let file = record.files.first()?;
                Some(workspace.join(&file.path))
            });
        let archive_before = archive_path
            .as_ref()
            .and_then(|archive| std::fs::read(archive).ok());
        let readme_path = workspace
            .as_ref()
            .map(|workspace| workspace.join("output/distribution/README.txt"));
        let readme_before = readme_path
            .as_ref()
            .and_then(|readme| std::fs::read(readme).ok());
        let retained_workspace_before = workspace
            .as_ref()
            .map(|workspace| snapshot_workspace(workspace));

        let reopened_acquisition = Arc::new(AcquisitionService::new(Arc::new(
            pumas_library::acquisition::AcquisitionStore::new(root.path()),
        )));
        let reopened_consumer = Arc::new(
            reopened_acquisition
                .open_consumer("runtime.llama.cpp")
                .unwrap(),
        );
        let reopened_store = reopened_acquisition.store().clone();
        let reopened_records_result = tokio::time::timeout(
            Duration::from_secs(5),
            reopened_consumer.run_blocking("read retained native failure record", move || {
                reopened_store.acquisitions()
            }),
        )
        .await;
        let reopened_records_read_ok = reopened_records_result
            .as_ref()
            .is_ok_and(|result| result.is_ok());
        let direct_installer = VersionInstaller::new(
            root.path().to_path_buf(),
            AppId::LlamaCpp,
            Arc::new(MetadataManager::new(root.path())),
            Arc::new(RwLock::new(InstallationProgressTracker::new(
                root.path().join("launcher-data/cache"),
            ))),
            Arc::new(AtomicBool::new(false)),
        )
        .with_github_client(Arc::new(
            GitHubClient::with_loopback_api(
                root.path().join("launcher-data/cache"),
                Duration::from_secs(3600),
                base_url,
            )
            .unwrap(),
        ))
        .with_acquisition_consumer(Some(reopened_consumer.clone()));
        let direct_recovery = match reopened_records_result {
            Ok(Ok(records)) => Some(
                tokio::time::timeout(
                    Duration::from_secs(5),
                    direct_installer.reconcile_retained_llama_cpp(records.into_values().collect()),
                )
                .await,
            ),
            _ => None,
        };
        let direct_consumer_shutdown =
            tokio::time::timeout(Duration::from_secs(5), reopened_consumer.shutdown()).await;
        drop(direct_installer);
        drop(reopened_consumer);
        let manager_recovery_error = match tokio::time::timeout(
            Duration::from_secs(8),
            VersionManager::new_with_acquisition(
                root.path(),
                AppId::LlamaCpp,
                reopened_acquisition.clone(),
            ),
        )
        .await
        {
            Ok(Err(error)) => Some(error),
            Ok(Ok(manager)) => {
                drop(manager);
                None
            }
            Err(_) => None,
        };
        let reopened_shutdown =
            tokio::time::timeout(Duration::from_secs(8), reopened_acquisition.shutdown()).await;
        let final_store_bytes = std::fs::read(root.path().join("downloads.json"));
        let final_metadata = MetadataManager::new(root.path())
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp));
        let final_sentinel = std::fs::read(&sentinel_path);
        let archive_after = archive_path.as_ref().map(std::fs::read);
        let readme_after = readme_path.as_ref().map(std::fs::read);
        let workspace_after = workspace
            .as_ref()
            .map(|workspace| snapshot_workspace(workspace));
        let final_destination_exists = root
            .path()
            .join(AppId::LlamaCpp.versions_dir_name())
            .join("b1234+cpu")
            .exists();
        let model_database_exists = root.path().join("models.db").exists();
        let _ = stop_source.send(());
        let source_shutdown =
            match tokio::time::timeout(Duration::from_secs(5), &mut source_task.0).await {
                Ok(result) => (result.is_ok(), true),
                Err(_) => {
                    source_task.0.abort();
                    (
                        false,
                        tokio::time::timeout(Duration::from_secs(5), &mut source_task.0)
                            .await
                            .is_ok(),
                    )
                }
            };
        let observed_requests: Vec<_> = std::iter::from_fn(|| requests.try_recv().ok()).collect();

        assert!(
            install_started && source_ready && source_release,
            "controlled source did not release the archive request"
        );
        let install_error =
            install_error.expect("native install must report a bounded preparation failure");
        assert!(
            install_error.contains("Could not find llama-server in extracted archive"),
            "unexpected native preparation result: {install_error}"
        );
        assert!(
            manager_shutdown_ok,
            "native installation owner did not report its failed task after drainage"
        );
        assert!(
            matches!(
                acquisition_shutdown,
                Ok(Err(PumasError::DownloadShutdownFailed { failures })) if failures > 0
            ),
            "initial acquisition owner did not report failed preparation after drain"
        );
        assert!(
            reopened_records_read_ok,
            "cold owner could not read the retained native acquisition record"
        );
        assert!(
            matches!(
                direct_recovery,
                Some(Ok(Err(PumasError::Validation { field, message })))
                    if field == "acquisition.consumer_recovery_required"
                        && message == "Retained consumer use has no authoritative completion receipt"
            ),
            "direct cold recovery did not return the exact receiptless-Using validation"
        );
        assert!(
            direct_consumer_shutdown.is_ok(),
            "direct recovery consumer did not drain within its deadline"
        );
        assert!(
            matches!(
                manager_recovery_error,
                Some(PumasError::Validation { field, message })
                    if field == "acquisition.consumer_recovery_required"
                        && message == "Retained consumer use has no authoritative completion receipt"
            ),
            "public manager reconstruction did not return the exact receiptless-Using validation"
        );
        assert!(
            reopened_shutdown.is_ok_and(|result| result.is_ok()),
            "cold acquisition owner did not drain after receiptless recovery refusal"
        );
        assert!(
            source_shutdown.0 && source_shutdown.1,
            "native source monitor did not drain or was not reaped after timeout"
        );
        assert_eq!(
            observed_requests,
            [
                "GET /repos/ggml-org/llama.cpp/releases/tags/b1234 HTTP/1.1",
                "GET /archive HTTP/1.1",
            ]
        );

        let install_document_bytes =
            install_document_bytes.expect("native acquisition store must be readable");
        let install_document =
            install_document.expect("native acquisition store must be valid JSON");
        assert_eq!(
            install_document["acquisitions"]
                .as_object()
                .map(|acquisitions| acquisitions.len()),
            Some(1)
        );
        assert!(install_document["consumer_receipts"]
            .as_object()
            .is_some_and(|receipts| receipts.is_empty()));
        assert!(install_document.get("queue_admissions").is_none());
        assert!(install_document.get("downloads").is_none());
        let records = records.expect("native acquisition records must remain readable");
        assert_eq!(records.len(), 1);
        let record = records
            .values()
            .next()
            .expect("one native acquisition must be retained");
        assert_eq!(record.demand.consumer, "runtime.llama.cpp");
        assert!(matches!(
            &record.phase,
            pumas_library::acquisition::AcquisitionPhase::Using { .. }
        ));
        assert_eq!(record.files.len(), 1);
        assert_eq!(record.files[0].sha256, publisher_sha256);
        let workspace = root
            .path()
            .join(AppId::LlamaCpp.versions_dir_name())
            .join(&record.workspace.relative_target);
        let archive = workspace.join(&record.files[0].path);
        let archive_bytes =
            std::fs::read(&archive).expect("publisher-verified archive must remain in custody");
        assert_eq!(archive_bytes.len() as u64, record.files[0].bytes);
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(&archive_bytes)),
            publisher_sha256
        );
        assert_eq!(archive_before.as_deref(), Some(archive_bytes.as_slice()));
        let readme_bytes = std::fs::read(workspace.join("output/distribution/README.txt"))
            .expect("verified native extraction README must remain in custody");
        assert_eq!(readme_bytes, b"native fixture README\n");
        assert_eq!(readme_before.as_deref(), Some(readme_bytes.as_slice()));
        assert_eq!(archive_after.unwrap().unwrap(), archive_bytes);
        assert_eq!(readme_after.unwrap().unwrap(), readme_bytes);
        assert_eq!(
            workspace_after.unwrap().unwrap(),
            retained_workspace_before.unwrap().unwrap(),
            "cold recovery changed retained native workspace state"
        );
        assert!(!workspace.join("output/distribution/llama-server").exists());
        assert!(!version_path.exists());
        assert!(matches!(default_version, Ok(Ok(None))));
        assert!(installed_metadata.unwrap().is_none());
        assert_eq!(
            sentinel_after_install.unwrap(),
            b"keep native authored state"
        );
        assert_eq!(final_store_bytes.unwrap(), install_document_bytes);
        assert!(final_metadata.unwrap().is_none());
        assert_eq!(final_sentinel.unwrap(), b"keep native authored state");
        assert!(!final_destination_exists);
        assert!(!model_database_exists);
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_receipt_post_rename_interruption_cold_reopen_refuses_changed_output_and_settles(
    ) {
        use sha2::Digest;
        use std::os::unix::fs::MetadataExt;

        type SnapshotEntry = (PathBuf, u64, u64, u32, i64, i64, i64, i64, Vec<u8>);
        type Snapshot = (Vec<SnapshotEntry>, String);
        type FileSnapshot = (u64, u64, u32, i64, i64, i64, i64, Vec<u8>);

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
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                    bytes,
                ));
            }
            (tree, format!("{:x}", digest.finalize()))
        }

        fn file_snapshot(path: &Path) -> Option<FileSnapshot> {
            let metadata = match std::fs::symlink_metadata(path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
                Err(error) => panic!("metadata lookup failed for {}: {error}", path.display()),
            };
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            Some((
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
                std::fs::read(path).unwrap(),
            ))
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
        let once = Arc::new(AtomicBool::new(true));
        manager.interrupt_after_native_rename = Some(once.clone());
        let mut updates = manager.install_version("b1234+cpu").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), observed)
            .await
            .unwrap()
            .unwrap();
        release.send(true).unwrap();
        let message = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match updates.recv().await.expect("installation must settle") {
                    ProgressUpdate::Error { message } => break message,
                    ProgressUpdate::Completed { success } => {
                        panic!("interrupted publication completed: {success}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        assert!(
            message.contains("Test interruption after native rename before metadata"),
            "{message}"
        );
        assert!(!once.load(Ordering::SeqCst), "interruption is one-shot");
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
        let metadata_path = root.path().join("launcher-data/metadata").join(format!(
            "versions-{}.json",
            AppId::LlamaCpp.to_string().to_lowercase()
        ));
        let read_store_bytes = || std::fs::read(&store_path).unwrap();
        let read_document =
            || -> serde_json::Value { serde_json::from_slice(&read_store_bytes()).unwrap() };
        let retained_store_state = file_snapshot(&store_path).unwrap();
        let retained_metadata_state = file_snapshot(&metadata_path);
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
        let destination = manager.version_path("b1234+cpu");
        let launcher = destination.join("bin/llama-server");
        let expected_launcher = concat!(
            "#!/bin/sh\n",
            "ROOT=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd) || exit 1\n",
            "BINARY_DIR=\"$ROOT\"/'distribution'\n",
            "export LD_LIBRARY_PATH=\"$BINARY_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}\"\n",
            "exec \"$BINARY_DIR/llama-server\" \"$@\"\n",
        )
        .as_bytes();
        assert_eq!(std::fs::read(&launcher).unwrap(), expected_launcher);
        assert!(!stage.exists());
        let published = snapshot(&destination);
        assert_eq!(receipt["payload"]["output_tree_sha256"], published.1);
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
        drop(once);
        drop(manager);
        drop(api);

        // Alter only this fixture's receipt-bound launcher, preserving its inode.
        std::fs::write(&launcher, b"altered receipt-bound launcher").unwrap();
        let altered = snapshot(&destination);
        // The fixture source server is already closed. This does not directly
        // observe whether recovery attempts a request.
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
        assert_eq!(file_snapshot(&store_path).unwrap(), retained_store_state);
        assert_eq!(read_document(), retained);
        assert_eq!(snapshot(&workspace), retained_workspace);
        assert_eq!(snapshot(&destination), altered);
        assert_eq!(file_snapshot(&metadata_path), retained_metadata_state);
        assert!(MetadataManager::new(root.path())
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
        drop(blocked_api);

        // Restore the exact fixture bytes in place, then baseline timestamps.
        std::fs::write(&launcher, expected_launcher).unwrap();
        let restored = snapshot(&destination);
        assert_eq!(restored.1, published.1);
        assert_eq!(
            restored
                .0
                .iter()
                .map(|entry| (&entry.0, entry.1, entry.2, entry.3, &entry.8))
                .collect::<Vec<_>>(),
            published
                .0
                .iter()
                .map(|entry| (&entry.0, entry.1, entry.2, entry.3, &entry.8))
                .collect::<Vec<_>>()
        );
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
        assert_eq!(snapshot(&destination), restored);
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
        let settled_metadata = file_snapshot(&metadata_path);
        drop(reopened);
        drop(reopened_api);

        let stable_api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let stable = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            stable_api.acquisition().clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            stable.get_installed_versions().await.unwrap(),
            vec!["b1234+cpu"]
        );
        assert_eq!(snapshot(&destination), restored);
        assert_eq!(read_document()["acquisitions"], settled["acquisitions"]);
        assert_eq!(
            read_document()["consumer_receipts"],
            settled["consumer_receipts"]
        );
        assert_eq!(file_snapshot(&metadata_path), settled_metadata);
        stable.shutdown_installations().await.unwrap();
        stable_api.shutdown_acquisition().await.unwrap();
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    struct NativeInstallChildGuard(Option<std::process::Child>);

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    impl NativeInstallChildGuard {
        fn child_mut(&mut self) -> &mut std::process::Child {
            self.0.as_mut().expect("child guard already disarmed")
        }

        fn disarm(&mut self) {
            self.0.take();
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    impl Drop for NativeInstallChildGuard {
        fn drop(&mut self) {
            let Some(mut child) = self.0.take() else {
                return;
            };
            let _ = child.kill();
            // Do not make test unwinding wait indefinitely. The detached
            // reaper owns the killed child and polls until try_wait observes
            // its terminal status, which also reaps it.
            let _ = std::thread::Builder::new()
                .name("native-install-test-child-reaper".into())
                .spawn(move || loop {
                    match child.try_wait() {
                        Ok(Some(_)) | Err(_) => break,
                        Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                    }
                });
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    #[ignore = "spawned only by the parent SIGKILL recovery regression"]
    async fn native_receipt_post_rename_sigkill_child() {
        const ROOT_ENV: &str = "PUMAS_NATIVE_SIGKILL_TEST_ROOT";
        const BASE_URL_ENV: &str = "PUMAS_NATIVE_SIGKILL_TEST_BASE_URL";
        const MARKER_ENV: &str = "PUMAS_NATIVE_SIGKILL_TEST_MARKER";

        let root = PathBuf::from(std::env::var_os(ROOT_ENV).expect("child root is required"));
        let base_url = std::env::var(BASE_URL_ENV).expect("child source URL is required");
        let marker = PathBuf::from(std::env::var_os(MARKER_ENV).expect("child marker is required"));
        let api = pumas_library::PumasApi::builder(&root)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let mut manager =
            VersionManager::new_with_acquisition(&root, AppId::LlamaCpp, api.acquisition().clone())
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
        manager.park_after_native_rename_marker = Some(marker);
        let _updates = manager.install_version("b1234+cpu").await.unwrap();

        // The parent owns the only termination authority for this dedicated
        // helper and kills it after observing the post-rename marker.
        std::future::pending::<()>().await;
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[tokio::test]
    async fn native_receipt_post_rename_sigkill_cold_reopen_recovers_without_source_replay() {
        use sha2::Digest;
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::process::ExitStatusExt;
        use std::process::Stdio;

        fn native_tree_sha256(root: &Path) -> String {
            use sha2::Digest;

            let mut digest = sha2::Sha256::new();
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .sort_by_file_name()
            {
                let entry = entry.unwrap();
                let relative = entry.path().strip_prefix(root).unwrap();
                let name = relative.to_str().unwrap().replace('\\', "/");
                digest.update((name.len() as u64).to_le_bytes());
                digest.update(name.as_bytes());
                let metadata = std::fs::symlink_metadata(entry.path()).unwrap();
                assert!(!metadata.file_type().is_symlink());
                digest.update(metadata.permissions().mode().to_le_bytes());
                if metadata.is_file() {
                    digest.update(b"file");
                    let bytes = std::fs::read(entry.path()).unwrap();
                    digest.update(format!("{:x}", sha2::Sha256::digest(&bytes)).as_bytes());
                } else {
                    assert!(metadata.is_dir());
                    digest.update(b"directory");
                }
            }
            format!("{:x}", digest.finalize())
        }

        const CHILD_TEST: &str = "version_manager::tests::native_receipt_post_rename_sigkill_child";
        const ROOT_ENV: &str = "PUMAS_NATIVE_SIGKILL_TEST_ROOT";
        const BASE_URL_ENV: &str = "PUMAS_NATIVE_SIGKILL_TEST_BASE_URL";
        const MARKER_ENV: &str = "PUMAS_NATIVE_SIGKILL_TEST_MARKER";

        let root = TempDir::new().unwrap();
        let control = TempDir::new().unwrap();
        let marker = control.path().join("post-rename-boundary");
        let (server, observed, release, base_url, mut requests, stop_server) =
            native_archive_fixture_with_request_monitor(root.path()).await;
        let executable = std::env::current_exe().unwrap();
        let child = std::process::Command::new(executable)
            .arg("--exact")
            .arg(CHILD_TEST)
            .arg("--ignored")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(ROOT_ENV, root.path())
            .env(BASE_URL_ENV, &base_url)
            .env(MARKER_ENV, &marker)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut child = NativeInstallChildGuard(Some(child));

        tokio::time::timeout(Duration::from_secs(5), observed)
            .await
            .expect("child did not request the controlled archive")
            .expect("source observation sender dropped");
        release.send(true).unwrap();
        for expected in [
            "GET /repos/ggml-org/llama.cpp/releases/tags/b1234 HTTP/1.1",
            "GET /archive HTTP/1.1",
        ] {
            let request = tokio::time::timeout(Duration::from_secs(5), requests.recv())
                .await
                .expect("initial source request was not observed")
                .expect("source monitor stopped before the initial requests");
            assert!(
                request.starts_with(expected),
                "unexpected initial request: {request}"
            );
        }

        let marker_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if marker.exists() {
                break;
            }
            if let Some(status) = child.child_mut().try_wait().unwrap() {
                panic!("child exited before the durable publication boundary: {status}");
            }
            assert!(
                tokio::time::Instant::now() < marker_deadline,
                "child did not reach the post-rename publication boundary"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            std::fs::read(&marker).unwrap(),
            b"native destination durable; metadata not published"
        );
        assert!(
            child.child_mut().try_wait().unwrap().is_none(),
            "child returned after reporting the publication boundary"
        );
        child.child_mut().kill().unwrap();
        let status = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = child.child_mut().try_wait().unwrap() {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("SIGKILLed installer child was not reaped");
        assert_eq!(
            status.signal(),
            Some(9),
            "child must end by SIGKILL: {status}"
        );
        child.disarm();

        let store_path = root.path().join("launcher-data/downloads.json");
        let retained: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store_path).unwrap()).unwrap();
        let acquisitions = retained["acquisitions"].as_object().unwrap();
        assert_eq!(acquisitions.len(), 1);
        assert_eq!(retained["consumer_receipts"].as_object().unwrap().len(), 1);
        let (acquisition_id, record) = acquisitions.iter().next().unwrap();
        assert_eq!(record["phase"]["state"], "using");
        let receipt = retained["consumer_receipts"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(receipt["owner"], "runtime.llama.cpp");
        assert_eq!(receipt["acquisition_id"], acquisition_id.as_str());
        assert_eq!(receipt["use_lease"], record["phase"]["lease"]);
        assert_eq!(receipt["demand"], record["demand"]);
        assert_eq!(receipt["manifest"], record["manifest"]);
        assert_eq!(receipt["workspace"], record["workspace"]);
        assert_eq!(receipt["verified_files"], record["files"]);

        let versions = root.path().join(AppId::LlamaCpp.versions_dir_name());
        let workspace = versions.join(record["workspace"]["relative_target"].as_str().unwrap());
        let stage = workspace.join("output");
        let destination = versions.join("b1234+cpu");
        assert!(workspace.is_dir(), "receipt custody workspace must remain");
        assert!(
            destination.is_dir(),
            "renamed native output must be present"
        );
        assert!(!stage.exists(), "published output stage must be absent");
        let files = record["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert!(workspace.join(files[0]["path"].as_str().unwrap()).is_file());
        assert_eq!(
            receipt["payload"]["output_tree_sha256"],
            native_tree_sha256(&destination)
        );
        let receipt_launcher = destination.join(
            receipt["payload"]["launcher_relative_path"]
                .as_str()
                .unwrap(),
        );
        assert_eq!(
            receipt["payload"]["launcher_sha256"],
            format!(
                "{:x}",
                sha2::Sha256::digest(std::fs::read(&receipt_launcher).unwrap())
            )
        );
        assert!(MetadataManager::new(root.path())
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());

        let recovery_api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let recovered = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            recovery_api.acquisition().clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            recovered.get_installed_versions().await.unwrap(),
            vec!["b1234+cpu"]
        );
        assert!(destination.join("bin/llama-server").is_file());
        assert!(!stage.exists());
        assert!(!workspace.exists());
        let recovered_metadata = recovered
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(recovered_metadata).unwrap(),
            receipt["payload"]["metadata"]
        );
        recovered.shutdown_installations().await.unwrap();
        recovery_api.shutdown_acquisition().await.unwrap();

        let settled: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(settled["consumer_receipts"], retained["consumer_receipts"]);
        assert_eq!(settled["acquisitions"].as_object().unwrap().len(), 1);
        let mut expected_record = record.clone();
        expected_record["phase"]["state"] = serde_json::json!("adopted");
        assert_eq!(settled["acquisitions"][acquisition_id], expected_record);
        drop(recovered);
        drop(recovery_api);

        let stable_api = pumas_library::PumasApi::builder(root.path())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let stable = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            stable_api.acquisition().clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            stable.get_installed_versions().await.unwrap(),
            vec!["b1234+cpu"]
        );
        assert_eq!(
            native_tree_sha256(&destination),
            receipt["payload"]["output_tree_sha256"]
        );
        assert_eq!(
            std::fs::read(&receipt_launcher).unwrap(),
            std::fs::read(destination.join("bin/llama-server")).unwrap()
        );
        assert_eq!(
            serde_json::to_value(
                stable
                    .metadata_manager
                    .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            receipt["payload"]["metadata"]
        );
        stable.shutdown_installations().await.unwrap();
        stable_api.shutdown_acquisition().await.unwrap();
        let after_stable_reopen: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store_path).unwrap()).unwrap();
        assert_eq!(after_stable_reopen["acquisitions"], settled["acquisitions"]);

        match tokio::time::timeout(Duration::from_secs(1), requests.recv()).await {
            Err(_) => {}
            Ok(Some(request)) => panic!("recovery replayed a controlled source request: {request}"),
            Ok(None) => panic!("source monitor closed before the no-replay check"),
        }
        drop(stop_server);
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .expect("source monitor did not stop within its deadline")
            .unwrap();
        match requests.try_recv() {
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
            | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {}
            Ok(request) => panic!("source monitor drain found an unexpected request: {request}"),
        }
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

    /// Model a new, unowned leaf appearing after successful owned cleanup.
    /// Keep the old record unchanged, and avoid accidental inode reuse making
    /// the replacement appear to have the old attempt's physical binding.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn native_replacement_workspace_fixture(
        versions: &Path,
        tag: &str,
        relative: &str,
        contents: &[u8],
    ) -> (PathBuf, PathBuf) {
        use pumas_library::acquisition::{ReservedDirectory, ReservedDirectoryBinding};
        use sha2::Digest;
        let _lock = installer::NativeVersionsLock::try_acquire(versions).unwrap();
        let digest = format!("{:x}", sha2::Sha256::digest(tag.as_bytes()));
        let attempt_path = versions.join(format!(".llama-attempt-{}.json", &digest[..24]));
        let attempt_bytes = std::fs::read(&attempt_path).unwrap();
        let attempt: serde_json::Value = serde_json::from_slice(&attempt_bytes).unwrap();
        assert_eq!(attempt["schema_version"], 2);
        let expected: ReservedDirectoryBinding =
            serde_json::from_value(attempt["binding"].clone()).unwrap();
        let workspace = versions.join(relative);
        assert!(
            !workspace.exists(),
            "owned publication cleanup must already have finished"
        );
        std::fs::create_dir(&workspace).unwrap();
        let observe = || {
            ReservedDirectory::capture(versions, Path::new(relative), Arc::new(()), || Ok(()))
                .unwrap()
        };
        let replacement = observe();
        if replacement.binding() == &expected {
            // Reserve the reused inode until its replacement has been created.
            // This changes only fixture files, never the durable custody proof.
            let occupied = tempfile::tempdir_in(versions.parent().unwrap()).unwrap();
            std::fs::rename(&workspace, occupied.path().join("reused-inode")).unwrap();
            std::fs::create_dir(&workspace).unwrap();
            let distinct = observe();
            assert_ne!(distinct.binding(), &expected);
        }
        drop(replacement);
        assert_ne!(observe().binding(), &expected);
        std::fs::write(workspace.join("stale"), contents).unwrap();
        assert_eq!(std::fs::read(&attempt_path).unwrap(), attempt_bytes);
        (workspace, attempt_path)
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
        let (stale_workspace, attempt_path) = native_replacement_workspace_fixture(
            &manager.versions_dir(),
            "b1234+cpu",
            &record.workspace.relative_target,
            b"registered recovery cleanup",
        );
        let before_attempt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&attempt_path).unwrap()).unwrap();
        assert!(before_attempt.get("cleanup_pending").is_none());
        let installed_before = manager
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
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
        assert_eq!(
            std::fs::read(stale_workspace.join("stale")).unwrap(),
            b"registered recovery cleanup"
        );
        let after_attempt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&attempt_path).unwrap()).unwrap();
        assert!(after_attempt["cleanup_pending"]
            .as_str()
            .unwrap()
            .contains("physical binding changed"));
        let mut expected_attempt = before_attempt;
        expected_attempt["cleanup_pending"] = after_attempt["cleanup_pending"].clone();
        assert_eq!(
            after_attempt, expected_attempt,
            "joined recovery may update diagnostics, not custody"
        );
        assert_eq!(
            serde_json::to_value(
                manager
                    .metadata_manager
                    .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(installed_before).unwrap()
        );
        assert!(installer::NativeVersionsLock::try_acquire(&manager.versions_dir()).is_ok());
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
        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        let record = document["acquisitions"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        let (stale_workspace, attempt_path) = native_replacement_workspace_fixture(
            &manager.versions_dir(),
            "b1234+cpu",
            record["workspace"]["relative_target"].as_str().unwrap(),
            b"retry cleanup after adoption",
        );
        // Direct public construction must finish adopted-output verification
        // while preserving this unowned replacement and reporting cleanup pending.
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
        assert_eq!(
            std::fs::read(stale_workspace.join("stale")).unwrap(),
            b"retry cleanup after adoption"
        );
        let pending: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&attempt_path).unwrap()).unwrap();
        assert!(pending["cleanup_pending"]
            .as_str()
            .unwrap()
            .contains("physical binding changed"));
        let progress = manager
            .progress_tracker
            .read()
            .await
            .get_current_state()
            .unwrap();
        assert_eq!(progress.success, Some(true));
        assert!(progress.error.is_none());
        assert!(progress.current_item.unwrap().contains("cleanup pending"));
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
        assert_eq!(
            std::fs::read(stale_workspace.join("stale")).unwrap(),
            b"retry cleanup after adoption"
        );
        // Remove only the replacement authored by this test, under the native
        // lock. Production recovery must never gain ownership merely to satisfy
        // the later explicit-removal/reinstall portion of this test.
        {
            let _lock =
                installer::NativeVersionsLock::try_acquire(&manager.versions_dir()).unwrap();
            std::fs::remove_file(stale_workspace.join("stale")).unwrap();
            std::fs::remove_dir(&stale_workspace).unwrap();
        }
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
        let reconciled_attempt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&attempt_path).unwrap()).unwrap();
        assert!(reconciled_attempt.get("cleanup_pending").is_none());
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
    async fn native_independent_acquisition_installs_and_repeated_shutdown_preserves_receipt() {
        use futures::FutureExt;
        use sha2::Digest;
        use std::os::unix::fs::MetadataExt;

        type SnapshotEntry = (PathBuf, u64, u64, u32, i64, i64, i64, i64, Vec<u8>);

        struct AbortSourceOnDrop(tokio::task::JoinHandle<()>);

        impl Drop for AbortSourceOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }

        fn snapshot_tree(root: &Path) -> Vec<SnapshotEntry> {
            walkdir::WalkDir::new(root)
                .sort_by_file_name()
                .into_iter()
                .map(|entry| {
                    let entry = entry.unwrap();
                    let metadata = std::fs::symlink_metadata(entry.path()).unwrap();
                    assert!(!metadata.file_type().is_symlink());
                    assert!(metadata.is_file() || metadata.is_dir());
                    (
                        entry.path().strip_prefix(root).unwrap().to_path_buf(),
                        metadata.dev(),
                        metadata.ino(),
                        metadata.mode(),
                        metadata.mtime(),
                        metadata.mtime_nsec(),
                        metadata.ctime(),
                        metadata.ctime_nsec(),
                        if metadata.is_file() {
                            std::fs::read(entry.path()).unwrap()
                        } else {
                            Vec::new()
                        },
                    )
                })
                .collect()
        }

        let root = TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("launcher-data")).unwrap();
        let sentinel_path = root.path().join("launcher-data/authored-sentinel");
        std::fs::write(&sentinel_path, b"keep native authored state").unwrap();

        let acquisition = Arc::new(AcquisitionService::new(Arc::new(
            pumas_library::acquisition::AcquisitionStore::new(root.path()),
        )));
        let store = acquisition.store().clone();
        let mut manager =
            VersionManager::new_with_acquisition(root.path(), AppId::LlamaCpp, acquisition.clone())
                .await
                .unwrap();
        let (source, observed, release, base_url, mut requests, stop_source) =
            native_archive_fixture_with_request_monitor(root.path()).await;
        let mut source_task = AbortSourceOnDrop(source);
        let mut release = Some(release);
        let mut stop_source = Some(stop_source);
        let exercise = std::panic::AssertUnwindSafe(async {
            manager.github_client = Arc::new(
                GitHubClient::with_loopback_api(
                    manager.cache_dir(),
                    Duration::from_secs(3600),
                    base_url,
                )
                .unwrap(),
            );

            let mut updates = tokio::time::timeout(
                Duration::from_secs(15),
                manager.install_version("b1234+cpu"),
            )
            .await
            .expect("native installation admission must finish within its deadline")
            .unwrap();
            tokio::time::timeout(Duration::from_secs(5), observed)
                .await
                .expect("native archive request must reach the controlled fixture")
                .unwrap();
            release.take().unwrap().send(true).unwrap();
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    match updates
                        .recv()
                        .await
                        .expect("native installation must settle")
                    {
                        ProgressUpdate::Completed { success: true } => break,
                        ProgressUpdate::Completed { success: false } => {
                            panic!("native installation reported unsuccessful completion")
                        }
                        ProgressUpdate::Error { message } => {
                            panic!("native installation failed: {message}")
                        }
                        _ => {}
                    }
                }
            })
            .await
            .expect("native installation must report terminal progress");

            let destination = manager.version_path("b1234+cpu");
            let launcher = destination.join("bin/llama-server");
            let output = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::process::Command::new(&launcher)
                    .kill_on_drop(true)
                    .current_dir(root.path())
                    .output(),
            )
            .await
            .expect("installed native launcher must finish within its deadline")
            .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"native-fixture");
            let metadata = manager
                .metadata_manager
                .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
                .unwrap()
                .expect("successful native installation must publish metadata");
            assert_eq!(metadata.path, "b1234+cpu");
            assert_eq!(metadata.release_tag, "b1234+cpu");
            let metadata_value = serde_json::to_value(&metadata).unwrap();

            let records = store.acquisitions().unwrap();
            assert_eq!(records.len(), 1);
            let (acquisition_id, record) = records.iter().next().unwrap();
            assert_eq!(record.demand.consumer, "runtime.llama.cpp");
            assert!(matches!(
                &record.phase,
                pumas_library::acquisition::AcquisitionPhase::Adopted { .. }
            ));
            let store_path = root.path().join("downloads.json");
            let store_bytes = std::fs::read(&store_path).unwrap();
            let document: serde_json::Value = serde_json::from_slice(&store_bytes).unwrap();
            let receipt: pumas_library::acquisition::AcquisitionConsumerReceipt =
                serde_json::from_value(
                    document["consumer_receipts"][acquisition_id.to_string()].clone(),
                )
                .expect("successful native publication must retain its completion receipt");
            assert_eq!(receipt.owner, "runtime.llama.cpp");
            assert_eq!(receipt.acquisition_id, acquisition_id.to_string());
            assert_eq!(receipt.demand, record.demand);
            assert_eq!(receipt.manifest, record.manifest);
            assert_eq!(receipt.workspace, record.workspace);
            assert_eq!(receipt.verified_files, record.files);
            if let pumas_library::acquisition::AcquisitionPhase::Adopted { lease } = &record.phase {
                assert_eq!(receipt.use_lease, lease.to_string());
            } else {
                unreachable!("adoption phase was checked above");
            }
            assert_eq!(receipt.payload["metadata"], metadata_value);
            assert_eq!(
                receipt.payload["launcher_relative_path"],
                "bin/llama-server"
            );
            assert_eq!(
                receipt.payload["launcher_sha256"],
                format!(
                    "{:x}",
                    sha2::Sha256::digest(std::fs::read(&launcher).unwrap())
                )
            );
            assert!(!manager.is_installing().await);

            let mut document_keys: Vec<_> = document
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            document_keys.sort_unstable();
            assert_eq!(
                document_keys,
                ["acquisitions", "consumer_receipts", "schema_version"]
            );
            assert_eq!(document["acquisitions"].as_object().unwrap().len(), 1);
            assert_eq!(document["consumer_receipts"].as_object().unwrap().len(), 1);
            let model_db = root.path().join("shared-resources/models/models.db");
            for suffix in ["", "-wal", "-shm", "-journal"] {
                assert!(
                    !model_db
                        .with_file_name(format!("models.db{suffix}"))
                        .exists(),
                    "independent runtime acquisition created model index artifact {}",
                    model_db
                        .with_file_name(format!("models.db{suffix}"))
                        .display()
                );
            }
            assert!(!root.path().join("launcher-data/downloads.json").exists());
            assert_eq!(
                std::fs::read(&sentinel_path).unwrap(),
                b"keep native authored state"
            );

            let before_shutdown = snapshot_tree(root.path());
            Ok::<_, ()>((
                before_shutdown,
                store_path,
                store_bytes,
                *acquisition_id,
                receipt,
            ))
        })
        .catch_unwind()
        .await;

        if let Some(release) = release.take() {
            let _ = release.send(true);
        }

        let manager_shutdown_first =
            tokio::time::timeout(Duration::from_secs(8), manager.shutdown_installations()).await;
        let manager_shutdown_second =
            tokio::time::timeout(Duration::from_secs(8), manager.shutdown_installations()).await;
        let acquisition_shutdown_first =
            tokio::time::timeout(Duration::from_secs(8), acquisition.shutdown()).await;
        let acquisition_shutdown_second =
            tokio::time::timeout(Duration::from_secs(8), acquisition.shutdown()).await;

        let (before_shutdown, exercise_panic) = match exercise {
            Ok(Ok(outcome)) => (Some(outcome), None),
            Ok(Err(())) => unreachable!("the exercise closure only returns a success value"),
            Err(panic) => (None, Some(panic)),
        };

        let post_shutdown =
            if let Some((before_shutdown, store_path, store_bytes, acquisition_id, receipt)) =
                &before_shutdown
            {
                Some(
                    std::panic::AssertUnwindSafe(async {
                        assert!(!manager.is_installing().await);
                        assert!(manager.installation_tasks.lock().unwrap().tasks.is_empty());

                        assert!(matches!(
                            acquisition.open_consumer("post-shutdown-probe"),
                            Err(PumasError::DownloadLifecycleClosed)
                        ));
                        assert!(matches!(
                            tokio::time::timeout(
                                Duration::from_secs(3),
                                manager.install_version("b5678+cpu")
                            )
                            .await
                            .expect("closed installation admission must return promptly"),
                            Err(PumasError::InstallationFailed { message })
                                if message == "Version manager is shutting down"
                        ));
                        assert_eq!(
                        snapshot_tree(root.path()),
                        *before_shutdown,
                        "repeated shutdown or closed admission changed installed or authored state"
                    );
                        assert_eq!(std::fs::read(store_path).unwrap(), *store_bytes);
                        let after_shutdown: serde_json::Value =
                            serde_json::from_slice(&std::fs::read(store_path).unwrap()).unwrap();
                        assert_eq!(
                            after_shutdown["consumer_receipts"][acquisition_id.to_string()],
                            serde_json::to_value(receipt).unwrap()
                        );
                    })
                    .catch_unwind()
                    .await,
                )
            } else {
                None
            };

        if let Some(stop_source) = stop_source.take() {
            let _ = stop_source.send(());
        }
        let source_result = match tokio::time::timeout(Duration::from_secs(5), &mut source_task.0)
            .await
        {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(_) => {
                source_task.0.abort();
                match tokio::time::timeout(Duration::from_secs(5), &mut source_task.0).await {
                    Ok(Ok(())) => {
                        Err("controlled native source timed out, then stopped after abort".into())
                    }
                    Ok(Err(error)) if error.is_cancelled() => Err(
                        "controlled native source timed out, then was aborted and joined".into(),
                    ),
                    Ok(Err(_)) => {
                        Err("controlled native source timed out, then failed after abort".into())
                    }
                    Err(_) => Err("controlled native source abort join also timed out".into()),
                }
            }
        };
        let observed_requests: Vec<_> = std::iter::from_fn(|| requests.try_recv().ok()).collect();

        let manager_shutdown_outcomes =
            [manager_shutdown_first, manager_shutdown_second].map(|result| format!("{result:?}"));
        let acquisition_shutdown_outcomes =
            [acquisition_shutdown_first, acquisition_shutdown_second]
                .map(|result| format!("{result:?}"));
        assert_eq!(
            manager_shutdown_outcomes,
            ["Ok(Ok(()))", "Ok(Ok(()))"],
            "both manager shutdown calls must finish and succeed"
        );
        assert_eq!(
            acquisition_shutdown_outcomes,
            ["Ok(Ok(()))", "Ok(Ok(()))"],
            "both acquisition shutdown calls must finish and succeed"
        );
        assert!(
            source_result.is_ok(),
            "controlled native source must stop: {source_result:?}"
        );
        if let Some(panic) = exercise_panic {
            std::panic::resume_unwind(panic);
        }
        if let Some(Err(panic)) = post_shutdown {
            std::panic::resume_unwind(panic);
        }
        assert_eq!(
            observed_requests,
            [
                "GET /repos/ggml-org/llama.cpp/releases/tags/b1234 HTTP/1.1",
                "GET /archive HTTP/1.1",
            ],
            "shutdown or closed admission caused unexpected source traffic"
        );
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
