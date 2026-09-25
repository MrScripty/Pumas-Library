//! Version installation with progress reporting.
//!
//! Handles downloading, extracting, and setting up new versions.

mod torch;
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod torch_tests;
use super::managed_python::ManagedPythonIdentity;
pub(crate) use torch::is_torch_runtime_release;
pub(crate) use torch::retry_pending_torch_cleanup;
pub(crate) struct TorchInstallPlan {
    pub(crate) preview: crate::version_manager::TorchPreview,
    pub(crate) requirements: String,
    pub(crate) resolution: String,
    pub(crate) report: String,
    pub(crate) interpreter_path: PathBuf,
    pub(crate) interpreter_hash: String,
    pub(crate) managed_python: ManagedPythonIdentity,
}
#[cfg(test)]
pub(crate) use torch::TorchPublicationPause;

use crate::version_manager::progress::{InstallationProgressTracker, ProgressUpdate};
use chrono::Utc;
use futures::future::{BoxFuture, Shared};
use futures::FutureExt;
use pumas_library::config::{AppId, InstallationConfig, PathsConfig};
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_library::models::InstallationStage;
use pumas_library::network::{GitHubAsset, GitHubRelease};
use pumas_library::{PumasError, Result};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, warn};

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path).await.map_err(|e| PumasError::Io {
        message: format!("Failed to check path existence: {}", e),
        path: Some(path.to_path_buf()),
        source: Some(e),
    })
}

/// Coordinates Torch cancellation with the irreversible publication boundary.
pub(crate) struct TorchInstallControl(AtomicU8);

type TorchCleanupCompletion = Shared<BoxFuture<'static, std::result::Result<(), Arc<String>>>>;
type TorchChildReceipt = tokio::sync::watch::Receiver<Option<std::result::Result<(), Arc<String>>>>;

#[derive(Default)]
pub(crate) struct TorchCleanupTasks {
    state: StdMutex<TorchCleanupState>,
}

#[derive(Default)]
struct TorchCleanupState {
    closed: bool,
    tasks: Vec<JoinHandle<()>>,
    failures: Vec<String>,
    completion: Option<TorchCleanupCompletion>,
    child_drain_completion: Option<TorchCleanupCompletion>,
    residual_drain_completion: Option<TorchCleanupCompletion>,
    child_slots: Vec<Arc<pumas_library::platform::managed_child::ManagedChildCustodySlot>>,
    child_receipts: std::collections::HashMap<usize, TorchChildReceipt>,
}

fn start_child_drain(
    slots: Vec<Arc<pumas_library::platform::managed_child::ManagedChildCustodySlot>>,
) -> TorchCleanupCompletion {
    let worker = tokio::task::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        for slot in slots {
            loop {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() && slot.is_active() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Torch child cleanup deadline elapsed",
                    ));
                }
                match slot.drain(remaining) {
                    Ok(_) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Ok::<(), std::io::Error>(())
    });
    async move {
        worker
            .await
            .map_err(|error| Arc::new(error.to_string()))?
            .map_err(|error| Arc::new(error.to_string()))
    }
    .boxed()
    .shared()
}

impl TorchCleanupTasks {
    pub(crate) fn new_child_slot(
        &self,
    ) -> Result<Arc<pumas_library::platform::managed_child::ManagedChildCustodySlot>> {
        let mut state = self.state.lock().expect("Torch cleanup lock poisoned");
        if state.closed {
            return Err(PumasError::InstallationFailed {
                message: "Torch cleanup is closed".into(),
            });
        }
        state
            .child_slots
            .retain(|slot| slot.is_active() || Arc::strong_count(slot) > 1);
        let retained: std::collections::HashSet<usize> = state
            .child_slots
            .iter()
            .map(|slot| Arc::as_ptr(slot) as usize)
            .collect();
        state.child_receipts.retain(|key, _| retained.contains(key));
        let slot = pumas_library::platform::managed_child::ManagedChildCustodySlot::new();
        let (completion_tx, completion_rx) = tokio::sync::watch::channel(None);
        let supervised = slot.clone();
        state.tasks.push(tokio::spawn(async move {
            let result = if supervised.wait_for_park_or_completion().await {
                let draining = supervised.clone();
                let outcome = tokio::task::spawn_blocking(move || {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                    loop {
                        let remaining =
                            deadline.saturating_duration_since(std::time::Instant::now());
                        if remaining.is_zero() {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "Torch child cleanup deadline elapsed",
                            ));
                        }
                        match draining.drain(remaining) {
                            Ok(_) => return Ok(()),
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(std::time::Duration::from_millis(20));
                            }
                            Err(error) => return Err(error),
                        }
                    }
                })
                .await;
                match outcome {
                    Err(error) => Err(Arc::new(error.to_string())),
                    Ok(Err(error)) => Err(Arc::new(error.to_string())),
                    Ok(Ok(())) => Ok(()),
                }
            } else {
                Ok(())
            };
            if let Err(error) = &result {
                warn!(%error, "Torch child cleanup remains pending");
            }
            let _ = completion_tx.send(Some(result));
        }));
        state
            .child_receipts
            .insert(Arc::as_ptr(&slot) as usize, completion_rx);
        state.child_slots.push(slot.clone());
        Ok(slot)
    }

    pub(crate) async fn drain_child_slot(
        &self,
        slot: &Arc<pumas_library::platform::managed_child::ManagedChildCustodySlot>,
    ) -> Result<()> {
        let mut receipt = self
            .state
            .lock()
            .expect("Torch cleanup lock poisoned")
            .child_receipts
            .get(&(Arc::as_ptr(slot) as usize))
            .cloned()
            .ok_or_else(|| PumasError::InstallationFailed {
                message: "Torch child custody receipt absent".into(),
            })?;
        loop {
            if let Some(result) = receipt.borrow().clone() {
                return result.map_err(|_| PumasError::InstallationFailed {
                    message: "Torch process cleanup incomplete; retry is required".into(),
                });
            }
            receipt
                .changed()
                .await
                .map_err(|_| PumasError::InstallationFailed {
                    message: "Torch child cleanup supervisor stopped before receipt".into(),
                })?;
        }
    }

    pub(crate) async fn drain_child_slots(&self) -> Result<()> {
        let completion = {
            let mut state = self.state.lock().expect("Torch cleanup lock poisoned");
            if let Some(completion) = &state.child_drain_completion {
                completion.clone()
            } else {
                let completion = start_child_drain(state.child_slots.clone());
                state.child_drain_completion = Some(completion.clone());
                completion
            }
        };
        let result = completion.await;
        self.state
            .lock()
            .expect("Torch cleanup lock poisoned")
            .child_drain_completion = None;
        result.map_err(|_| PumasError::InstallationFailed {
            message: "Torch process cleanup incomplete; retry is required".into(),
        })
    }

    pub(crate) async fn drain_residual_child_slots(&self) -> Result<()> {
        let completion = {
            let mut state = self.state.lock().expect("Torch cleanup lock poisoned");
            if let Some(completion) = &state.residual_drain_completion {
                completion.clone()
            } else {
                let slots = state
                    .child_slots
                    .iter()
                    .filter(|slot| slot.is_cleanup_pending())
                    .cloned()
                    .collect();
                let completion = start_child_drain(slots);
                state.residual_drain_completion = Some(completion.clone());
                completion
            }
        };
        let result = completion.await;
        self.state
            .lock()
            .expect("Torch cleanup lock poisoned")
            .residual_drain_completion = None;
        result.map_err(|_| PumasError::InstallationFailed {
            message: "Torch process cleanup incomplete; retry is required".into(),
        })
    }

    pub(crate) fn schedule(&self, work: impl FnOnce() + Send + 'static) {
        let mut state = self.state.lock().expect("Torch cleanup lock poisoned");
        if !state.closed {
            let tasks = std::mem::take(&mut state.tasks);
            for mut task in tasks {
                if task.is_finished() {
                    match (&mut task).now_or_never() {
                        Some(Err(error)) => state.failures.push(error.to_string()),
                        Some(Ok(())) => continue,
                        None => state.tasks.push(task),
                    }
                } else {
                    state.tasks.push(task);
                }
            }
            state.tasks.push(tokio::task::spawn_blocking(work));
        }
    }

    pub(crate) fn close(&self) {
        self.state
            .lock()
            .expect("Torch cleanup lock poisoned")
            .closed = true;
    }

    pub(crate) async fn drain(&self) -> Result<()> {
        let completion = {
            let mut state = self.state.lock().expect("Torch cleanup lock poisoned");
            state.closed = true;
            if let Some(completion) = &state.completion {
                completion.clone()
            } else {
                let tasks = std::mem::take(&mut state.tasks);
                let mut failures = std::mem::take(&mut state.failures);
                // One-shot cleanup closures cannot be replayed after a panic.
                // Preserve that failure receipt; durable stage/publication
                // markers are retried at startup, and child slots have their
                // own retryable drain completion.
                // This supervisor owns every handle even if all callers waiting
                // on the shared receipt are cancelled.
                let supervisor = tokio::spawn(async move {
                    for task in tasks {
                        if let Err(error) = task.await {
                            failures.push(error.to_string());
                        }
                    }
                    if failures.is_empty() {
                        Ok(())
                    } else {
                        Err(Arc::new(failures.join("; ")))
                    }
                });
                let completion = async move {
                    supervisor
                        .await
                        .unwrap_or_else(|error| Err(Arc::new(error.to_string())))
                }
                .boxed()
                .shared();
                state.completion = Some(completion.clone());
                completion
            }
        };
        completion
            .await
            .map_err(|error| PumasError::Other(format!("Torch cleanup tasks failed: {error}")))
    }
}

impl TorchInstallControl {
    const IDLE: u8 = 0;
    const ACTIVE: u8 = 1;
    const CANCEL_REQUESTED: u8 = 2;
    const PUBLISHING: u8 = 3;
    const FINISHED: u8 = 4;

    pub(crate) fn new() -> Self {
        Self(AtomicU8::new(Self::IDLE))
    }

    /// Starts a new attempt without undoing a cancellation of an active attempt.
    pub(crate) fn start(&self) -> bool {
        let mut current = self.0.load(Ordering::SeqCst);
        loop {
            if current != Self::IDLE && current != Self::FINISHED {
                return false;
            }
            match self
                .0
                .compare_exchange(current, Self::ACTIVE, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return true,
                Err(observed) => current = observed,
            }
        }
    }

    pub(crate) fn request_cancel(&self) -> bool {
        match self.0.compare_exchange(
            Self::ACTIVE,
            Self::CANCEL_REQUESTED,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) | Err(Self::CANCEL_REQUESTED) => true,
            Err(_) => false,
        }
    }

    pub(crate) fn try_begin_publication(&self) -> bool {
        self.0
            .compare_exchange(
                Self::ACTIVE,
                Self::PUBLISHING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
    }

    pub(crate) fn finish(&self) {
        self.0.store(Self::FINISHED, Ordering::SeqCst);
    }
}

#[cfg(test)]
pub(crate) type TorchStageOverride = Arc<dyn Fn(&Path) -> Result<PathBuf> + Send + Sync>;

/// Handles version installation.
pub struct VersionInstaller {
    /// Root directory for launcher.
    launcher_root: PathBuf,
    /// Application ID.
    app_id: AppId,
    /// Metadata manager.
    metadata_manager: Arc<MetadataManager>,
    /// Progress tracker.
    progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
    /// Cancellation flag.
    cancel_flag: Arc<AtomicBool>,
    torch_control: Arc<TorchInstallControl>,
    torch_cleanup: Arc<TorchCleanupTasks>,
    torch_attempt_lock: Mutex<()>,
    #[cfg(test)]
    torch_stage_override: Option<TorchStageOverride>,
    #[cfg(test)]
    torch_publication_pause: Option<Arc<torch::TorchPublicationPause>>,
    #[cfg(test)]
    torch_stage_pause: Option<Arc<torch::TorchPublicationPause>>,
}

impl VersionInstaller {
    /// Create a new version installer.
    ///
    /// The supplied cancellation flag is a cooperative request observed at
    /// installer checkpoints; setting it directly does not report whether the
    /// request was accepted. Torch cancellation acknowledged by `VersionManager`
    /// is serialized with runtime publication. A direct flag update after the
    /// installer's final cancellation checkpoint may race with successful
    /// publication.
    pub fn new(
        launcher_root: PathBuf,
        app_id: AppId,
        metadata_manager: Arc<MetadataManager>,
        progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            launcher_root,
            app_id,
            metadata_manager,
            progress_tracker,
            cancel_flag,
            torch_control: Arc::new(TorchInstallControl::new()),
            torch_cleanup: Arc::new(TorchCleanupTasks::default()),
            torch_attempt_lock: Mutex::new(()),
            #[cfg(test)]
            torch_stage_override: None,
            #[cfg(test)]
            torch_publication_pause: None,
            #[cfg(test)]
            torch_stage_pause: None,
        }
    }

    /// Drain Torch quarantine cleanup after the last direct install call.
    /// Owners using `VersionInstaller` without `VersionManager` must await this
    /// before shutting down their Tokio runtime.
    pub async fn shutdown_torch_cleanup(&self) -> Result<()> {
        self.torch_cleanup.close();
        let tasks = self.torch_cleanup.drain().await;
        let children = self.torch_cleanup.drain_child_slots().await;
        tasks?;
        children
    }

    pub(crate) fn with_torch_control(mut self, control: Arc<TorchInstallControl>) -> Self {
        self.torch_control = control;
        self
    }

    pub(crate) fn with_torch_cleanup(mut self, cleanup: Arc<TorchCleanupTasks>) -> Self {
        self.torch_cleanup = cleanup;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_torch_publication_pause(
        mut self,
        pause: Arc<torch::TorchPublicationPause>,
    ) -> Self {
        self.torch_publication_pause = Some(pause);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_torch_stage_pause(
        mut self,
        pause: Arc<torch::TorchPublicationPause>,
    ) -> Self {
        self.torch_stage_pause = Some(pause);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_torch_stage_override(mut self, stage: TorchStageOverride) -> Self {
        self.torch_stage_override = Some(stage);
        self
    }

    /// Install a version from a GitHub release.
    /// Dispatches to app-specific installation method based on app_id.
    pub async fn install_version(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        self.install_version_with_torch_plan(tag, release, progress_tx, None)
            .await
    }

    pub(crate) async fn install_version_with_torch_plan(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
        torch_plan: Option<TorchInstallPlan>,
    ) -> Result<()> {
        match self.app_id {
            AppId::Ollama => self.install_ollama_binary(tag, release, progress_tx).await,
            AppId::LlamaCpp => {
                self.install_llama_cpp_binary(tag, release, progress_tx)
                    .await
            }
            AppId::Torch => {
                self.install_torch_runtime(tag, release, progress_tx, torch_plan)
                    .await
            }
            AppId::OnnxRuntime => Err(PumasError::Other(
                "ONNX Runtime is embedded and cannot be installed as a version".to_string(),
            )),
        }
    }

    /// Install Ollama binary from pre-built release assets.
    /// Unlike Python apps, Ollama is distributed as a pre-compiled binary.
    async fn install_ollama_binary(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Starting Ollama binary installation for {}", tag);

        // Select platform-appropriate asset (e.g., ollama-linux-amd64.tgz)
        let asset = self.select_ollama_asset(&release.assets)?;
        let download_url = &asset.download_url;
        let total_size = asset.size;
        let asset_name = asset.name.clone();

        info!(
            "Selected Ollama asset: {} ({} bytes)",
            asset_name, total_size
        );

        // Create log file
        let log_dir = self.logs_dir();
        fs::create_dir_all(&log_dir).await.ok();
        let log_path = log_dir.join(format!(
            "install-ollama-{}-{}.log",
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        // Start progress tracking with actual asset size from GitHub
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.start_installation(
                tag,
                Some(total_size),
                None,
                Some(log_path.to_string_lossy().as_ref()),
            );
        }

        // Use download cache directory to avoid re-downloading on reinstalls
        let cache_downloads = self
            .launcher_root
            .join("launcher-data")
            .join("cache")
            .join("downloads");
        fs::create_dir_all(&cache_downloads)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create download cache directory: {}", e),
                path: Some(cache_downloads.clone()),
                source: Some(e),
            })?;

        let archive_path = cache_downloads.join(&asset_name);

        // Check if we have a valid cached download
        let cache_valid = if path_exists(&archive_path).await? {
            match fs::metadata(&archive_path).await {
                Ok(meta) if meta.len() == total_size => {
                    info!(
                        "Using cached download: {} ({} bytes)",
                        asset_name, total_size
                    );
                    true
                }
                Ok(meta) => {
                    info!(
                        "Cached download size mismatch ({} != {}), re-downloading",
                        meta.len(),
                        total_size
                    );
                    let _ = fs::remove_file(&archive_path).await;
                    false
                }
                Err(_) => {
                    let _ = fs::remove_file(&archive_path).await;
                    false
                }
            }
        } else {
            false
        };

        // Download binary asset (skip if cache is valid)
        let result = self
            .do_ollama_install(
                tag,
                release,
                download_url,
                total_size,
                &asset_name,
                &archive_path,
                cache_valid,
                &progress_tx,
            )
            .await;

        // Keep cached download on success, remove on failure
        if result.is_err() {
            let _ = fs::remove_file(&archive_path).await;
        }

        // Update progress tracker
        {
            let mut tracker = self.progress_tracker.write().await;
            if let Err(error) = result.as_ref() {
                tracker.set_error(&error.to_string());
            }
            tracker.complete_installation(result.is_ok());
        }

        result
    }

    /// Install llama.cpp from upstream pre-built release archives.
    async fn install_llama_cpp_binary(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Starting llama.cpp binary installation for {}", tag);

        let asset = self.select_llama_cpp_asset(&release.assets)?;
        let download_url = &asset.download_url;
        let total_size = asset.size;
        let asset_name = asset.name.clone();

        info!(
            "Selected llama.cpp asset: {} ({} bytes)",
            asset_name, total_size
        );

        let log_dir = self.logs_dir();
        fs::create_dir_all(&log_dir).await.ok();
        let log_path = log_dir.join(format!(
            "install-llama-cpp-{}-{}.log",
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.start_installation(
                tag,
                Some(total_size),
                None,
                Some(log_path.to_string_lossy().as_ref()),
            );
        }

        let cache_downloads = self
            .launcher_root
            .join("launcher-data")
            .join("cache")
            .join("downloads");
        fs::create_dir_all(&cache_downloads)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create download cache directory: {}", e),
                path: Some(cache_downloads.clone()),
                source: Some(e),
            })?;

        let archive_path = cache_downloads.join(&asset_name);
        let cache_valid = self
            .is_cached_download_valid(&archive_path, &asset_name, total_size)
            .await?;

        let result = self
            .do_llama_cpp_install(
                tag,
                release,
                download_url,
                total_size,
                &asset_name,
                &archive_path,
                cache_valid,
                &progress_tx,
            )
            .await;

        if result.is_err() {
            let _ = fs::remove_file(&archive_path).await;
        }

        {
            let mut tracker = self.progress_tracker.write().await;
            if let Err(error) = result.as_ref() {
                tracker.set_error(&error.to_string());
            }
            tracker.complete_installation(result.is_ok());
        }

        result
    }

    /// Execute Ollama installation steps.
    #[allow(clippy::too_many_arguments)]
    async fn do_ollama_install(
        &self,
        tag: &str,
        release: &GitHubRelease,
        download_url: &str,
        total_size: u64,
        asset_name: &str,
        archive_path: &Path,
        cache_valid: bool,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        // Check cancellation
        self.check_cancelled()?;

        // Step 1: Download binary asset (skip if using cache)
        if cache_valid {
            // Update progress to show we're using cache
            {
                let mut tracker = self.progress_tracker.write().await;
                tracker.update_stage(
                    InstallationStage::Download,
                    100.0,
                    Some("Using cached download"),
                );
            }
            let _ = progress_tx
                .send(ProgressUpdate::Download {
                    downloaded_bytes: total_size,
                    total_bytes: Some(total_size),
                    speed_bytes_per_sec: None,
                })
                .await;
        } else {
            self.download_archive(download_url, archive_path, progress_tx)
                .await?;
        }

        // Check cancellation
        self.check_cancelled()?;

        // Step 2: Create version directory
        let version_dir = self.versions_dir().join(tag);
        fs::create_dir_all(&version_dir)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create version directory: {}", e),
                path: Some(version_dir.clone()),
                source: Some(e),
            })?;

        // Step 3: Extract binary from archive
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Extract,
                0.0,
                Some("Extracting binary..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Extract,
                message: "Extracting binary...".to_string(),
            })
            .await;

        let archive_path = archive_path.to_path_buf();
        let version_dir_for_extract = version_dir.clone();
        let asset_name = asset_name.to_string();
        tokio::task::spawn_blocking(move || {
            Self::extract_ollama_binary(&archive_path, &version_dir_for_extract, &asset_name)
        })
        .await
        .map_err(|e| {
            PumasError::Other(format!("Failed to join Ollama extraction task: {}", e))
        })??;

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Extract,
                100.0,
                Some("Extraction complete"),
            );
        }

        // Check cancellation
        self.check_cancelled()?;

        // Step 4: Finalize (no venv, no deps - just mark as installed)
        self.finalize_ollama_installation(tag, release, &version_dir, progress_tx)
            .await?;

        info!("Ollama installation of {} completed successfully", tag);
        Ok(())
    }

    async fn is_cached_download_valid(
        &self,
        archive_path: &Path,
        asset_name: &str,
        total_size: u64,
    ) -> Result<bool> {
        if !path_exists(archive_path).await? {
            return Ok(false);
        }

        match fs::metadata(archive_path).await {
            Ok(meta) if meta.len() == total_size => {
                info!(
                    "Using cached download: {} ({} bytes)",
                    asset_name, total_size
                );
                Ok(true)
            }
            Ok(meta) => {
                info!(
                    "Cached download size mismatch ({} != {}), re-downloading",
                    meta.len(),
                    total_size
                );
                let _ = fs::remove_file(archive_path).await;
                Ok(false)
            }
            Err(_) => {
                let _ = fs::remove_file(archive_path).await;
                Ok(false)
            }
        }
    }

    /// Execute llama.cpp installation steps.
    #[allow(clippy::too_many_arguments)]
    async fn do_llama_cpp_install(
        &self,
        tag: &str,
        release: &GitHubRelease,
        download_url: &str,
        total_size: u64,
        asset_name: &str,
        archive_path: &Path,
        cache_valid: bool,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        self.check_cancelled()?;

        if cache_valid {
            {
                let mut tracker = self.progress_tracker.write().await;
                tracker.update_stage(
                    InstallationStage::Download,
                    100.0,
                    Some("Using cached download"),
                );
            }
            let _ = progress_tx
                .send(ProgressUpdate::Download {
                    downloaded_bytes: total_size,
                    total_bytes: Some(total_size),
                    speed_bytes_per_sec: None,
                })
                .await;
        } else {
            self.download_archive(download_url, archive_path, progress_tx)
                .await?;
        }

        self.check_cancelled()?;

        let version_dir = self.versions_dir().join(tag);
        if path_exists(&version_dir).await? {
            fs::remove_dir_all(&version_dir)
                .await
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to remove existing version directory: {}", e),
                    path: Some(version_dir.clone()),
                    source: Some(e),
                })?;
        }
        fs::create_dir_all(&version_dir)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create version directory: {}", e),
                path: Some(version_dir.clone()),
                source: Some(e),
            })?;

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Extract,
                0.0,
                Some("Extracting binary archive..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Extract,
                message: "Extracting binary archive...".to_string(),
            })
            .await;

        let archive_path = archive_path.to_path_buf();
        let version_dir_for_extract = version_dir.clone();
        let asset_name = asset_name.to_string();
        tokio::task::spawn_blocking(move || {
            Self::extract_llama_cpp_binary(&archive_path, &version_dir_for_extract, &asset_name)
        })
        .await
        .map_err(|e| {
            PumasError::Other(format!("Failed to join llama.cpp extraction task: {}", e))
        })??;

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Extract,
                100.0,
                Some("Extraction complete"),
            );
        }

        self.check_cancelled()?;

        self.finalize_llama_cpp_installation(tag, release, &version_dir, progress_tx)
            .await?;

        info!("llama.cpp installation of {} completed successfully", tag);
        Ok(())
    }

    /// Select the appropriate Ollama binary asset for the current platform.
    /// Uses exact matching to avoid selecting variant builds (ROCm, Jetpack, etc.).
    fn select_ollama_asset<'a>(&self, assets: &'a [GitHubAsset]) -> Result<&'a GitHubAsset> {
        let os = std::env::consts::OS;
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            _ => std::env::consts::ARCH,
        };

        // Exact patterns for standard binaries (excludes -rocm, -jetpack variants)
        let exact_patterns = [
            format!("ollama-{}-{}.tar.zst", os, arch), // Primary (current format)
            format!("ollama-{}-{}.tgz", os, arch),     // Legacy format
            format!("ollama-{}-{}.tar.gz", os, arch),  // Legacy format
            format!("ollama-{}-{}.zip", os, arch),     // Windows
        ];

        assets
            .iter()
            .find(|a| exact_patterns.contains(&a.name))
            .ok_or_else(|| PumasError::InstallationFailed {
                message: format!(
                    "No Ollama binary found for {}-{}. Looking for: {:?}. Available assets: {:?}",
                    os,
                    arch,
                    exact_patterns,
                    assets.iter().map(|a| &a.name).collect::<Vec<_>>()
                ),
            })
    }

    /// Select the llama.cpp binary asset for the current platform.
    fn select_llama_cpp_asset<'a>(&self, assets: &'a [GitHubAsset]) -> Result<&'a GitHubAsset> {
        let os = std::env::consts::OS;
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => std::env::consts::ARCH,
        };
        Self::select_llama_cpp_asset_for_platform(
            assets,
            os,
            arch,
            Self::host_prefers_llama_cpp_gpu_asset(),
        )
    }

    fn select_llama_cpp_asset_for_platform<'a>(
        assets: &'a [GitHubAsset],
        os: &str,
        arch: &str,
        prefer_gpu_asset: bool,
    ) -> Result<&'a GitHubAsset> {
        let platform = match os {
            "linux" => "ubuntu",
            "macos" => "macos",
            "windows" => "win",
            _ => os,
        };
        if prefer_gpu_asset {
            if let Some(asset) = Self::find_llama_cpp_gpu_asset(assets, os, platform, arch) {
                return Ok(asset);
            }
        }
        Self::find_llama_cpp_cpu_asset(assets, platform, arch)
            .or_else(|| Self::find_llama_cpp_gpu_asset(assets, os, platform, arch))
            .ok_or_else(|| PumasError::InstallationFailed {
                message: format!(
                    "No llama.cpp binary found for {}-{}. Available assets: {:?}",
                    platform,
                    arch,
                    assets.iter().map(|a| &a.name).collect::<Vec<_>>()
                ),
            })
    }

    fn host_prefers_llama_cpp_gpu_asset() -> bool {
        std::path::Path::new("/proc/driver/nvidia/version").exists()
            || std::path::Path::new("/dev/nvidia0").exists()
            || std::path::Path::new("/dev/kfd").exists()
            || std::fs::read_dir("/dev/dri")
                .map(|entries| {
                    entries
                        .filter_map(std::result::Result::ok)
                        .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
                })
                .unwrap_or(false)
    }

    fn find_llama_cpp_gpu_asset<'a>(
        assets: &'a [GitHubAsset],
        os: &str,
        platform: &str,
        arch: &str,
    ) -> Option<&'a GitHubAsset> {
        let preferred_flavors: &[&str] = match os {
            "linux" => &["vulkan", "rocm", "sycl"],
            "windows" => &["cuda-13", "cuda-12", "vulkan", "hip", "sycl"],
            _ => &[],
        };
        preferred_flavors.iter().find_map(|flavor| {
            assets.iter().find(|asset| {
                let name = asset.name.to_ascii_lowercase();
                Self::is_llama_cpp_platform_archive(&name, platform, arch) && name.contains(flavor)
            })
        })
    }

    fn find_llama_cpp_cpu_asset<'a>(
        assets: &'a [GitHubAsset],
        platform: &str,
        arch: &str,
    ) -> Option<&'a GitHubAsset> {
        let excluded = [
            "source",
            "cudart",
            "cuda",
            "vulkan",
            "rocm",
            "hip",
            "sycl",
            "openvino",
            "android",
            "ios",
            "xcframework",
            "openeuler",
            "kleidiai",
        ];
        assets.iter().find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            Self::is_llama_cpp_platform_archive(&name, platform, arch)
                && !excluded.iter().any(|pattern| name.contains(pattern))
        })
    }

    fn is_llama_cpp_platform_archive(name: &str, platform: &str, arch: &str) -> bool {
        name.starts_with("llama-")
            && name.contains("-bin-")
            && name.contains(platform)
            && name.contains(arch)
            && (name.ends_with(".zip") || name.ends_with(".tar.gz") || name.ends_with(".tgz"))
    }

    /// Extract Ollama binary from archive format.
    /// Ollama releases are distributed as:
    /// - Linux: ollama-linux-amd64.tar.zst (Zstandard compressed tar, current format)
    /// - Linux (legacy): ollama-linux-amd64.tgz (gzip compressed tar)
    /// - macOS: ollama-darwin-arm64.tar.zst
    /// - Windows: ollama-windows-amd64.zip (containing ollama.exe)
    fn extract_ollama_binary(
        archive_path: &Path,
        version_dir: &Path,
        asset_name: &str,
    ) -> Result<()> {
        info!("Extracting Ollama binary from {}", asset_name);

        if asset_name.ends_with(".tar.zst") {
            // Extract tar.zst (Zstandard compressed tar - current Ollama format)
            Self::extract_tar_zst(archive_path, version_dir)?;
        } else if asset_name.ends_with(".tgz") || asset_name.ends_with(".tar.gz") {
            // Extract tar.gz (legacy format)
            Self::extract_tarball(archive_path, version_dir)?;
        } else if asset_name.ends_with(".zip") {
            // Extract zip
            Self::extract_zip(archive_path, version_dir)?;
        } else {
            // Raw binary (e.g., ollama-linux-amd64 without extension)
            let binary_name = if cfg!(windows) {
                "ollama.exe"
            } else {
                "ollama"
            };
            let dest = version_dir.join(binary_name);
            std::fs::copy(archive_path, &dest).map_err(|e| PumasError::Io {
                message: format!("Failed to copy binary: {}", e),
                path: Some(dest.clone()),
                source: Some(e),
            })?;
        }

        // Find and make the binary executable on Unix
        Self::finalize_ollama_binary(version_dir)?;

        info!("Ollama binary extraction complete");
        Ok(())
    }

    /// Extract a llama.cpp binary archive and ensure llama-server is executable.
    fn extract_llama_cpp_binary(
        archive_path: &Path,
        version_dir: &Path,
        asset_name: &str,
    ) -> Result<()> {
        info!("Extracting llama.cpp binary from {}", asset_name);

        if asset_name.ends_with(".tar.zst") {
            Self::extract_tar_zst(archive_path, version_dir)?;
        } else if asset_name.ends_with(".tgz") || asset_name.ends_with(".tar.gz") {
            Self::extract_tarball(archive_path, version_dir)?;
        } else if asset_name.ends_with(".zip") {
            Self::extract_zip(archive_path, version_dir)?;
        } else {
            return Err(PumasError::InstallationFailed {
                message: format!("Unsupported llama.cpp archive format: {}", asset_name),
            });
        }

        let server_binary = Self::find_named_binary(version_dir, &["llama-server", "server"])?
            .ok_or_else(|| PumasError::InstallationFailed {
                message: "Could not find llama-server in extracted archive".to_string(),
            })?;
        let launch_binary = Self::install_llama_cpp_launch_binary(version_dir, &server_binary)?;
        Self::make_binary_executable(&launch_binary)?;

        info!(
            "llama.cpp server binary available at {}",
            launch_binary.display()
        );
        Ok(())
    }

    fn install_llama_cpp_launch_binary(version_dir: &Path, source: &Path) -> Result<PathBuf> {
        let binary_name = if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        };
        let final_path = version_dir.join("bin").join(binary_name);

        if source == final_path {
            return Ok(final_path);
        }

        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: format!("Failed to create llama.cpp binary parent directory: {}", e),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }

        #[cfg(unix)]
        {
            let binary_dir = source
                .parent()
                .ok_or_else(|| PumasError::InstallationFailed {
                    message: format!(
                        "Could not determine llama.cpp binary directory for {}",
                        source.display()
                    ),
                })?;
            let binary_file = source
                .file_name()
                .ok_or_else(|| PumasError::InstallationFailed {
                    message: format!(
                        "Could not determine llama.cpp binary filename for {}",
                        source.display()
                    ),
                })?;
            let wrapper = format!(
                "#!/bin/sh\nBINARY_DIR={}\nexport LD_LIBRARY_PATH=\"$BINARY_DIR${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}\"\nexec \"$BINARY_DIR/{}\" \"$@\"\n",
                shell_single_quote(&binary_dir.to_string_lossy()),
                binary_file.to_string_lossy().replace('"', "\\\"")
            );
            std::fs::write(&final_path, wrapper).map_err(|e| PumasError::Io {
                message: format!("Failed to create llama.cpp launch wrapper: {}", e),
                path: Some(final_path.clone()),
                source: Some(e),
            })?;
            info!(
                "Created llama.cpp launch wrapper at {} for {}",
                final_path.display(),
                source.display()
            );
            Ok(final_path)
        }

        #[cfg(not(unix))]
        {
            std::fs::copy(source, &final_path).map_err(|e| PumasError::Io {
                message: format!(
                    "Failed to copy llama.cpp server binary into launch path: {}",
                    e
                ),
                path: Some(final_path.clone()),
                source: Some(e),
            })?;
            info!(
                "Copied llama.cpp server binary from {} to {}",
                source.display(),
                final_path.display()
            );

            Ok(final_path)
        }
    }

    /// Extract a .tar.zst archive (Zstandard compressed tar).
    fn extract_tar_zst(archive_path: &Path, dest_dir: &Path) -> Result<()> {
        info!("Extracting tar.zst archive to {}", dest_dir.display());

        let file = File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open archive: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(e),
        })?;

        let decoder = zstd::Decoder::new(BufReader::new(file)).map_err(|e| PumasError::Io {
            message: format!("Failed to create zstd decoder: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(std::io::Error::other(e)),
        })?;

        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest_dir).map_err(|e| PumasError::Io {
            message: format!("Failed to extract tar.zst: {}", e),
            path: Some(dest_dir.to_path_buf()),
            source: Some(e),
        })?;

        Ok(())
    }

    /// Find the ollama binary in the extracted directory and make it executable.
    fn finalize_ollama_binary(version_dir: &Path) -> Result<()> {
        let binary_name = if cfg!(windows) {
            "ollama.exe"
        } else {
            "ollama"
        };
        let final_path = version_dir.join(binary_name);

        if !final_path.exists() {
            let found = Self::find_ollama_binary(version_dir)?.ok_or_else(|| {
                PumasError::InstallationFailed {
                    message: "Could not find Ollama binary in extracted archive".to_string(),
                }
            })?;

            if let Some(parent) = final_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                    message: format!("Failed to create binary parent directory: {}", e),
                    path: Some(parent.to_path_buf()),
                    source: Some(e),
                })?;
            }

            std::fs::rename(&found, &final_path).map_err(|e| PumasError::Io {
                message: format!("Failed to move Ollama binary into launch path: {}", e),
                path: Some(final_path.clone()),
                source: Some(e),
            })?;
            info!(
                "Moved Ollama binary from {} to {}",
                found.display(),
                final_path.display()
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&final_path)
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to get binary metadata: {}", e),
                    path: Some(final_path.clone()),
                    source: Some(e),
                })?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&final_path, perms).map_err(|e| PumasError::Io {
                message: format!("Failed to set binary permissions: {}", e),
                path: Some(final_path.clone()),
                source: Some(e),
            })?;
            info!("Set executable permissions on {}", final_path.display());
        }

        Ok(())
    }

    fn find_ollama_binary(dir: &Path) -> Result<Option<PathBuf>> {
        let binary_names = if cfg!(windows) {
            ["ollama.exe", "ollama"]
        } else {
            ["ollama", "ollama.exe"]
        };

        for entry in std::fs::read_dir(dir).map_err(|e| PumasError::Io {
            message: format!("Failed to read extracted directory: {}", e),
            path: Some(dir.to_path_buf()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: format!("Failed to read extracted entry: {}", e),
                path: Some(dir.to_path_buf()),
                source: Some(e),
            })?;
            let path = entry.path();

            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    if binary_names.contains(&name) {
                        return Ok(Some(path));
                    }
                }
            } else if path.is_dir() {
                if let Some(found) = Self::find_ollama_binary(&path)? {
                    return Ok(Some(found));
                }
            }
        }

        Ok(None)
    }

    fn find_named_binary(dir: &Path, binary_names: &[&str]) -> Result<Option<PathBuf>> {
        let names: Vec<String> = if cfg!(windows) {
            binary_names
                .iter()
                .flat_map(|name| [format!("{name}.exe"), (*name).to_string()])
                .collect()
        } else {
            binary_names
                .iter()
                .flat_map(|name| [(*name).to_string(), format!("{name}.exe")])
                .collect()
        };

        for entry in std::fs::read_dir(dir).map_err(|e| PumasError::Io {
            message: format!("Failed to read extracted directory: {}", e),
            path: Some(dir.to_path_buf()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: format!("Failed to read extracted entry: {}", e),
                path: Some(dir.to_path_buf()),
                source: Some(e),
            })?;
            let path = entry.path();

            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    if names.iter().any(|candidate| candidate == name) {
                        return Ok(Some(path));
                    }
                }
            } else if path.is_dir() {
                if let Some(found) = Self::find_named_binary(&path, binary_names)? {
                    return Ok(Some(found));
                }
            }
        }

        Ok(None)
    }

    fn make_binary_executable(path: &Path) -> Result<()> {
        #[cfg(not(unix))]
        let _ = path;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to get binary metadata: {}", e),
                    path: Some(path.to_path_buf()),
                    source: Some(e),
                })?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).map_err(|e| PumasError::Io {
                message: format!("Failed to set binary permissions: {}", e),
                path: Some(path.to_path_buf()),
                source: Some(e),
            })?;
        }
        Ok(())
    }

    /// Finalize Ollama installation (create metadata, no Python/venv).
    async fn finalize_ollama_installation(
        &self,
        tag: &str,
        release: &GitHubRelease,
        _version_dir: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Finalizing Ollama installation for {}", tag);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Setup,
                0.0,
                Some("Finalizing installation..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Setup,
                message: "Finalizing installation...".to_string(),
            })
            .await;

        // Find the download URL for metadata
        let download_url = release
            .assets
            .iter()
            .find(|a| {
                a.name.contains("linux") || a.name.contains("darwin") || a.name.contains("windows")
            })
            .map(|a| a.download_url.clone());

        // Create metadata entry (no Python version for Ollama)
        let metadata = InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: Utc::now().to_rfc3339(),
            release_tag: tag.to_string(),
            python_version: None, // Ollama is a Go binary, no Python
            git_commit: None,
            release_date: Some(release.published_at.clone()),
            release_notes: release.body.clone(),
            download_url,
            size: release.archive_size,
            requirements_hash: None,
            dependencies_installed: Some(true), // No dependencies needed
        };

        // Save metadata
        self.metadata_manager
            .update_installed_version(tag, metadata, Some(self.app_id))?;

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Setup,
                100.0,
                Some("Installation complete"),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::Setup {
                message: "Installation complete".to_string(),
            })
            .await;

        info!("Ollama installation of {} finalized", tag);
        Ok(())
    }

    /// Finalize llama.cpp installation metadata.
    async fn finalize_llama_cpp_installation(
        &self,
        tag: &str,
        release: &GitHubRelease,
        _version_dir: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Finalizing llama.cpp installation for {}", tag);

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Setup,
                0.0,
                Some("Finalizing installation..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Setup,
                message: "Finalizing installation...".to_string(),
            })
            .await;

        let (download_url, size) = self
            .select_llama_cpp_asset(&release.assets)
            .map(|asset| (Some(asset.download_url.clone()), Some(asset.size)))
            .unwrap_or((None, release.archive_size));

        let metadata = InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: Utc::now().to_rfc3339(),
            release_tag: tag.to_string(),
            python_version: None,
            git_commit: None,
            release_date: Some(release.published_at.clone()),
            release_notes: release.body.clone(),
            download_url,
            size,
            requirements_hash: None,
            dependencies_installed: Some(true),
        };

        self.metadata_manager
            .update_installed_version(tag, metadata, Some(self.app_id))?;

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Setup,
                100.0,
                Some("Installation complete"),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::Setup {
                message: "Installation complete".to_string(),
            })
            .await;

        info!("llama.cpp installation of {} finalized", tag);
        Ok(())
    }

    async fn download_archive(
        &self,
        url: &str,
        archive_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Downloading archive from {}", url);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Download,
                0.0,
                Some("Starting download..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Download,
                message: "Starting download...".to_string(),
            })
            .await;

        // Create HTTP client with appropriate timeouts for large downloads
        // - connect_timeout: time to establish connection (15s is fine)
        // - NO overall timeout: downloads can take a long time for large files (1.6 GB+)
        let client = reqwest::Client::builder()
            .connect_timeout(InstallationConfig::URL_FETCH_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: Some(e.to_string()),
            })?;

        // Start download with retry
        let mut response = None;
        for attempt in 1..=InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS {
            match client.get(url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        response = Some(resp);
                        break;
                    } else {
                        warn!(
                            "Download attempt {} failed with status {}",
                            attempt,
                            resp.status()
                        );
                    }
                }
                Err(e) => {
                    warn!("Download attempt {} failed: {}", attempt, e);
                    if attempt == InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS {
                        return Err(PumasError::Network {
                            message: format!("Download failed after {} attempts: {}", attempt, e),
                            cause: Some(e.to_string()),
                        });
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
                }
            }
        }

        let response = response.ok_or_else(|| PumasError::Network {
            message: "Download failed - no successful response".to_string(),
            cause: None,
        })?;

        let total_size = response.content_length();

        // Create output file
        let mut file = fs::File::create(archive_path)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create archive file: {}", e),
                path: Some(archive_path.to_path_buf()),
                source: Some(e),
            })?;

        // Download with progress
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let start_time = std::time::Instant::now();

        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            // Check cancellation
            self.check_cancelled()?;

            let chunk = chunk.map_err(|e| PumasError::Network {
                message: format!("Error reading download chunk: {}", e),
                cause: Some(e.to_string()),
            })?;

            file.write_all(&chunk).await.map_err(|e| PumasError::Io {
                message: format!("Failed to write to archive: {}", e),
                path: Some(archive_path.to_path_buf()),
                source: Some(e),
            })?;

            downloaded += chunk.len() as u64;

            // Calculate speed
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                Some(downloaded as f64 / elapsed)
            } else {
                None
            };

            // Update progress
            {
                let mut tracker = self.progress_tracker.write().await;
                tracker.update_download_progress(downloaded, total_size, speed);
            }

            let _ = progress_tx
                .send(ProgressUpdate::Download {
                    downloaded_bytes: downloaded,
                    total_bytes: total_size,
                    speed_bytes_per_sec: speed,
                })
                .await;
        }

        // Tokio can acknowledge the final write while its blocking file work
        // is still queued. Checksum and extraction readers reopen this path,
        // so finish all writes before reporting the download complete.
        file.flush().await.map_err(|e| PumasError::Io {
            message: format!("Failed to flush archive: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(e),
        })?;

        // Add to completed items
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.add_completed_item("archive", "archive", Some(downloaded));
        }

        info!("Download complete: {} bytes", downloaded);
        Ok(())
    }

    fn extract_zip(archive_path: &Path, extract_dir: &Path) -> Result<()> {
        let file = File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open zip archive: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(e),
        })?;

        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| PumasError::InstallationFailed {
                message: format!("Invalid zip archive: {}", e),
            })?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| PumasError::InstallationFailed {
                    message: format!("Failed to read zip entry {}: {}", i, e),
                })?;

            let outpath = match file.enclosed_name() {
                Some(path) => extract_dir.join(path),
                None => continue,
            };

            if file.is_dir() {
                std::fs::create_dir_all(&outpath).map_err(|e| PumasError::Io {
                    message: format!("Failed to create directory: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;
            } else {
                if let Some(parent) = outpath.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                            message: format!("Failed to create parent directory: {}", e),
                            path: Some(parent.to_path_buf()),
                            source: Some(e),
                        })?;
                    }
                }

                let mut outfile = File::create(&outpath).map_err(|e| PumasError::Io {
                    message: format!("Failed to create file: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;

                std::io::copy(&mut file, &mut outfile).map_err(|e| PumasError::Io {
                    message: format!("Failed to extract file: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;
            }

            // Set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode)).ok();
                }
            }
        }

        Ok(())
    }

    fn extract_tarball(archive_path: &Path, extract_dir: &Path) -> Result<()> {
        let file = File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open tarball: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(e),
        })?;

        let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
        let mut archive = tar::Archive::new(decoder);

        archive
            .unpack(extract_dir)
            .map_err(|e| PumasError::InstallationFailed {
                message: format!("Failed to extract tarball: {}", e),
            })?;

        Ok(())
    }

    async fn finalize_installation(
        &self,
        tag: &str,
        release: &GitHubRelease,
        version_dir: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Finalizing installation for {}", tag);

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Setup,
                0.0,
                Some("Finalizing installation..."),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::StageChanged {
                stage: InstallationStage::Setup,
                message: "Finalizing installation...".to_string(),
            })
            .await;

        // Get Python version
        let venv_python = pumas_library::platform::paths::venv_python(version_dir);
        let python_version = if path_exists(&venv_python).await? {
            let output = tokio::process::Command::new(&venv_python)
                .args(["--version"])
                .output()
                .await
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string());
            output
        } else {
            None
        };

        // Create metadata entry
        let metadata = InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: Utc::now().to_rfc3339(),
            release_tag: tag.to_string(),
            python_version,
            git_commit: None, // Could extract from git log if needed
            release_date: Some(release.published_at.clone()),
            release_notes: release.body.clone(),
            download_url: if self.app_id == AppId::Torch {
                if let Some(recipe) = torch::torch_recipe_for_tag(tag) {
                    Some(recipe.torch_wheel_url.to_string())
                } else {
                    std::fs::read(version_dir.join("resolution.json"))
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                        .and_then(|resolution| {
                            resolution["artifacts"]
                                .as_array()?
                                .iter()
                                .find(|artifact| {
                                    artifact["name"]
                                        .as_str()
                                        .is_some_and(|name| name.eq_ignore_ascii_case("torch"))
                                })?["url"]
                                .as_str()
                                .map(str::to_string)
                        })
                }
            } else {
                release.zipball_url.clone().or(release.tarball_url.clone())
            },
            size: if self.app_id == AppId::Torch {
                None
            } else {
                release.total_size
            },
            requirements_hash: None, // Could compute if needed
            dependencies_installed: Some(true),
        };

        // Persist outside the async executor; publication is not complete until
        // the shared metadata owner has durably recorded the installation.
        let manager = self.metadata_manager.clone();
        let installed_tag = tag.to_string();
        let app_id = self.app_id;
        tokio::task::spawn_blocking(move || {
            manager.update_installed_version(&installed_tag, metadata, Some(app_id))
        })
        .await
        .map_err(|error| PumasError::Other(format!("Runtime metadata task failed: {error}")))??;

        // Update progress
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.update_stage(
                InstallationStage::Setup,
                100.0,
                Some("Installation complete"),
            );
        }
        let _ = progress_tx
            .send(ProgressUpdate::Setup {
                message: "Installation complete".to_string(),
            })
            .await;

        info!("Installation of {} finalized", tag);
        Ok(())
    }

    fn check_cancelled(&self) -> Result<()> {
        if self.cancel_flag.load(Ordering::SeqCst) {
            Err(PumasError::InstallationFailed {
                message: "Installation cancelled by user".to_string(),
            })
        } else {
            Ok(())
        }
    }

    fn versions_dir(&self) -> PathBuf {
        self.launcher_root.join(self.app_id.versions_dir_name())
    }

    fn logs_dir(&self) -> PathBuf {
        self.launcher_root
            .join("launcher-data")
            .join(PathsConfig::LOGS_DIR_NAME)
    }

    fn slugify_tag(&self, tag: &str) -> String {
        tag.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>()
            .to_lowercase()
    }
}

#[cfg(unix)]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torch_cancel_and_publication_are_mutually_exclusive() {
        let control = TorchInstallControl::new();
        assert!(control.start());
        assert!(control.request_cancel());
        assert!(control.request_cancel());
        assert!(!control.try_begin_publication());
        control.finish();

        assert!(control.start());
        assert!(control.try_begin_publication());
        assert!(!control.request_cancel());
        control.finish();
        assert!(!control.request_cancel());
    }

    #[test]
    fn runtime_zip_extraction_preserves_stored_and_deflated_payloads() {
        use std::io::Write;
        use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

        let temp = tempfile::TempDir::new().unwrap();
        for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
            let archive_path = temp.path().join(format!("{method:?}.zip"));
            let output = temp.path().join(format!("{method:?}-extracted"));
            let mut writer = ZipWriter::new(File::create(&archive_path).unwrap());
            writer
                .start_file(
                    "runtime/bin/server",
                    SimpleFileOptions::default().compression_method(method),
                )
                .unwrap();
            let payload = b"runtime distribution fixture\n".repeat(128);
            writer.write_all(&payload).unwrap();
            writer.finish().unwrap();

            VersionInstaller::extract_zip(&archive_path, &output).unwrap();
            assert_eq!(
                std::fs::read(output.join("runtime/bin/server")).unwrap(),
                payload
            );
        }
    }

    #[test]
    fn test_slugify_tag() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let metadata_manager = Arc::new(MetadataManager::new(temp_dir.path()));
        let progress_tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
            temp_dir.path().to_path_buf(),
        )));

        let installer = VersionInstaller::new(
            temp_dir.path().to_path_buf(),
            AppId::Torch,
            metadata_manager,
            progress_tracker,
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(installer.slugify_tag("v1.0.0"), "v100");
        assert_eq!(installer.slugify_tag("v1.0.0-beta.1"), "v100-beta1");
    }

    #[test]
    fn test_finalize_ollama_binary_moves_nested_binary_to_launch_path() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let version_dir = temp_dir.path().join("v0.22.1");
        let nested_bin_dir = version_dir.join("bin");
        let binary_name = if cfg!(windows) {
            "ollama.exe"
        } else {
            "ollama"
        };
        std::fs::create_dir_all(&nested_bin_dir).unwrap();
        std::fs::write(nested_bin_dir.join(binary_name), b"binary").unwrap();

        VersionInstaller::finalize_ollama_binary(&version_dir).unwrap();

        let launch_binary = version_dir.join(binary_name);
        assert!(launch_binary.exists());
        assert_eq!(std::fs::read(&launch_binary).unwrap(), b"binary");
    }

    #[test]
    fn test_finalize_ollama_binary_fails_when_binary_missing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let version_dir = temp_dir.path().join("v0.22.1");
        std::fs::create_dir_all(&version_dir).unwrap();

        let result = VersionInstaller::finalize_ollama_binary(&version_dir);

        assert!(matches!(result, Err(PumasError::InstallationFailed { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn llama_cpp_launch_binary_uses_wrapper_with_runtime_library_path() {
        use std::os::unix::fs::PermissionsExt;

        let cwd = std::env::current_dir().unwrap();
        let temp_dir = tempfile::tempdir_in(&cwd).unwrap();
        let relative_root = temp_dir.path().strip_prefix(&cwd).unwrap();
        let absolute_root =
            pumas_library::platform::paths::absolute_launcher_root(relative_root).unwrap();
        let version_dir = absolute_root.join("b9090+vulkan");
        let archive_dir = version_dir.join("llama-b9090");
        let source = archive_dir.join("llama-server");
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::write(&source, b"#!/bin/sh\nprintf 'owned-wrapper-fixture'\n").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).unwrap();

        let launch_binary =
            VersionInstaller::install_llama_cpp_launch_binary(&version_dir, &source).unwrap();
        VersionInstaller::make_binary_executable(&launch_binary).unwrap();

        assert_eq!(launch_binary, version_dir.join("bin/llama-server"));
        let wrapper = std::fs::read_to_string(&launch_binary).unwrap();
        assert!(wrapper.contains("LD_LIBRARY_PATH"));
        assert!(wrapper.contains(archive_dir.to_string_lossy().as_ref()));
        let mode = std::fs::metadata(&launch_binary)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        let unrelated_cwd = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(&launch_binary)
            .current_dir(unrelated_cwd.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"owned-wrapper-fixture");
    }

    #[test]
    fn llama_cpp_asset_selection_prefers_linux_vulkan_when_gpu_is_available() {
        let assets = vec![
            github_asset("llama-b9082-bin-ubuntu-x64.tar.gz"),
            github_asset("llama-b9082-bin-ubuntu-vulkan-x64.tar.gz"),
            github_asset("llama-b9082-bin-ubuntu-rocm-7.2-x64.tar.gz"),
        ];

        let selected =
            VersionInstaller::select_llama_cpp_asset_for_platform(&assets, "linux", "x64", true)
                .unwrap();

        assert_eq!(selected.name, "llama-b9082-bin-ubuntu-vulkan-x64.tar.gz");
    }

    #[test]
    fn llama_cpp_asset_selection_keeps_cpu_default_without_gpu() {
        let assets = vec![
            github_asset("llama-b9082-bin-ubuntu-x64.tar.gz"),
            github_asset("llama-b9082-bin-ubuntu-vulkan-x64.tar.gz"),
        ];

        let selected =
            VersionInstaller::select_llama_cpp_asset_for_platform(&assets, "linux", "x64", false)
                .unwrap();

        assert_eq!(selected.name, "llama-b9082-bin-ubuntu-x64.tar.gz");
    }

    #[test]
    fn llama_cpp_asset_selection_accepts_explicit_vulkan_only_release() {
        let assets = vec![github_asset("llama-b9082-bin-ubuntu-vulkan-x64.tar.gz")];

        let selected =
            VersionInstaller::select_llama_cpp_asset_for_platform(&assets, "linux", "x64", false)
                .unwrap();

        assert_eq!(selected.name, "llama-b9082-bin-ubuntu-vulkan-x64.tar.gz");
    }

    fn github_asset(name: &str) -> GitHubAsset {
        GitHubAsset {
            name: name.to_string(),
            size: 1,
            download_url: format!("https://example.invalid/{name}"),
            content_type: None,
        }
    }
}
