//! Version installation with progress reporting.
//!
//! Handles downloading, extracting, and setting up new versions.

mod torch;
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod torch_tests;
use super::managed_python::ManagedPythonIdentity;
pub(crate) use torch::is_torch_runtime_release;
pub(crate) use torch::retry_pending_torch_cleanup;
pub(crate) use torch::TorchVersionsLock;
pub(crate) struct TorchInstallPlan {
    pub(crate) preview: crate::version_manager::TorchPreview,
    pub(crate) requirements: String,
    pub(crate) resolution: String,
    pub(crate) report: String,
    pub(crate) interpreter_path: PathBuf,
    pub(crate) interpreter_hash: String,
    pub(crate) managed_python: ManagedPythonIdentity,
}
pub(crate) enum TorchInstallInput {
    Selection(super::torch_preview::TorchInstallSelection),
    Resolved(Box<TorchInstallPlan>),
}
#[cfg(test)]
pub(crate) use torch::TorchPublicationPause;

use crate::version_manager::progress::{InstallationProgressTracker, ProgressUpdate};
use chrono::Utc;
use futures::future::{BoxFuture, Shared};
use futures::FutureExt;
use pumas_library::acquisition::{
    AcquisitionConsumer, AcquisitionDemand, AcquisitionHttpRequest, AcquisitionHttpSource,
    AcquisitionRetryPolicy, AcquisitionWorkspace,
};
use pumas_library::config::{AppId, InstallationConfig, PathsConfig};
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_library::models::InstallationStage;
use pumas_library::network::{GitHubAsset, GitHubClient, GitHubRelease, RetryConfig};
use pumas_library::{PumasError, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Instant;
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

fn sync_native_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    let directory = File::open(path);
    #[cfg(windows)]
    let directory = {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
    };
    #[cfg(not(any(unix, windows)))]
    let directory: std::io::Result<File> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Native directory durability is unsupported",
    ));
    directory
        .and_then(|directory| directory.sync_all())
        .map_err(|error| PumasError::io_with_path(error, path))
}

fn sync_native_file(path: &Path) -> Result<()> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| PumasError::io_with_path(error, path))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(PumasError::InstallationFailed {
            message: format!(
                "Native durability requires a regular file: {}",
                path.display()
            ),
        });
    }
    #[cfg(unix)]
    let file = File::open(path);
    #[cfg(windows)]
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path);
    #[cfg(not(any(unix, windows)))]
    let file: std::io::Result<File> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Native file durability is unsupported",
    ));
    file.and_then(|file| file.sync_all())
        .map_err(|error| PumasError::io_with_path(error, path))
}

fn sync_native_tree(root: &Path) -> Result<()> {
    VersionInstaller::validate_native_output(root)?;
    let mut directories = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| PumasError::InstallationFailed {
            message: format!("Native durability traversal failed: {error}"),
        })?;
        if entry.file_type().is_file() {
            sync_native_file(entry.path())?;
        } else if entry.file_type().is_dir() {
            directories.push(entry.path().to_path_buf());
        }
    }
    for directory in directories.into_iter().rev() {
        sync_native_directory(&directory)?;
    }
    Ok(())
}

fn sync_native_metadata(versions: &Path, app_id: AppId) -> Result<()> {
    let launcher = versions
        .parent()
        .ok_or_else(|| PumasError::InstallationFailed {
            message: "Native versions have no launcher root".into(),
        })?;
    let metadata_dir = launcher.join("launcher-data/metadata");
    let path = metadata_dir.join(format!(
        "versions-{}.json",
        app_id.to_string().to_lowercase()
    ));
    sync_native_file(&path)?;
    sync_native_directory(&metadata_dir)?;
    sync_native_directory(&launcher.join("launcher-data"))?;
    sync_native_directory(launcher)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeAttemptIdentity {
    schema_version: u32,
    tag: String,
    attempt: String,
    removed: bool,
}

fn native_attempt_path(versions: &Path, tag: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(tag.as_bytes()));
    versions.join(format!(".llama-attempt-{}.json", &digest[..24]))
}

fn native_workspace_path(versions: &Path, tag: &str, attempt: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(tag.as_bytes()));
    versions.join(format!(".llama-install-{}-{attempt}", &digest[..24]))
}

fn cleanup_native_workspace_if_present(directory: &Path, versions: &Path) -> Result<()> {
    if !path_exists_sync(directory)? {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(directory)
        .map_err(|error| PumasError::io_with_path(error, directory))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(PumasError::InstallationFailed {
            message: "Native workspace is not a safe owned directory; reclamation required".into(),
        });
    }
    std::fs::remove_dir_all(directory).map_err(|error| PumasError::Io {
        message: format!("Native workspace cleanup incomplete; reclamation required: {error}"),
        path: Some(directory.to_path_buf()),
        source: Some(error),
    })?;
    sync_native_directory(versions)
}

fn read_native_attempt(versions: &Path, tag: &str) -> Result<Option<NativeAttemptIdentity>> {
    let path = native_attempt_path(versions, tag);
    if path_exists_sync(&path)? {
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| PumasError::io_with_path(error, &path))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(PumasError::Validation {
                field: "acquisition.consumer_recovery_required".into(),
                message: "Native attempt identity is not a regular file".into(),
            });
        }
    }
    let identity: Option<NativeAttemptIdentity> = pumas_library::metadata::atomic_read_json(&path)?;
    if let Some(identity) = &identity {
        if identity.schema_version != 1
            || identity.tag != tag
            || identity.attempt.len() != 32
            || !identity
                .attempt
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(PumasError::Validation {
                field: "acquisition.consumer_recovery_required".into(),
                message: "Native attempt identity is malformed".into(),
            });
        }
    }
    Ok(identity)
}

fn write_native_attempt(versions: &Path, identity: &NativeAttemptIdentity) -> Result<()> {
    pumas_library::metadata::atomic_write_json(
        &native_attempt_path(versions, &identity.tag),
        identity,
        false,
    )?;
    sync_native_directory(versions)
}

/// Called only while removal holds the permanent native versions lease.
/// Persist revocation before deleting output so restart can never republish it.
pub(crate) fn mark_native_attempt_removed(versions: &Path, tag: &str) -> Result<()> {
    VersionInstaller::validate_native_tag(tag)?;
    if let Some(mut identity) = read_native_attempt(versions, tag)? {
        identity.removed = true;
        write_native_attempt(versions, &identity)?;
        let directory = native_workspace_path(versions, tag, &identity.attempt);
        cleanup_native_workspace_if_present(&directory, versions)?;
    }
    Ok(())
}

fn path_exists_sync(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PumasError::io_with_path(error, path)),
    }
}

fn hash_metadata(metadata: &InstalledVersionMetadata) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(metadata)?)
    ))
}

fn metadata_matches(left: &InstalledVersionMetadata, right: &InstalledVersionMetadata) -> bool {
    matches!((hash_metadata(left), hash_metadata(right)), (Ok(left), Ok(right)) if left == right)
}

fn hash_regular_file(path: &Path) -> Result<String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| PumasError::io_with_path(error, path))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(PumasError::InstallationFailed {
            message: format!("Native receipt requires a regular file: {}", path.display()),
        });
    }
    let mut file = File::open(path).map_err(|error| PumasError::io_with_path(error, path))?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|error| PumasError::io_with_path(error, path))?;
    Ok(format!("{:x}", digest.finalize()))
}

fn hash_native_tree(root: &Path) -> Result<String> {
    let metadata =
        std::fs::symlink_metadata(root).map_err(|error| PumasError::io_with_path(error, root))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(PumasError::InstallationFailed {
            message: "Native output root is not a directory".into(),
        });
    }
    VersionInstaller::validate_native_output(root)?;
    let mut digest = Sha256::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry.map_err(|error| PumasError::InstallationFailed {
            message: format!("Could not hash native output: {error}"),
        })?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| PumasError::Other(error.to_string()))?;
        let name = relative
            .to_str()
            .ok_or_else(|| PumasError::InstallationFailed {
                message: "Native output name is not UTF-8".into(),
            })?
            .replace('\\', "/");
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|error| PumasError::io_with_path(error, entry.path()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            digest.update(metadata.permissions().mode().to_le_bytes());
        }
        if metadata.file_type().is_symlink() {
            digest.update(b"link");
            let target = std::fs::read_link(entry.path())
                .map_err(|error| PumasError::io_with_path(error, entry.path()))?;
            let target = target
                .to_str()
                .ok_or_else(|| PumasError::InstallationFailed {
                    message: "Native link target is not UTF-8".into(),
                })?;
            digest.update((target.len() as u64).to_le_bytes());
            digest.update(target.as_bytes());
        } else if metadata.is_file() {
            digest.update(b"file");
            digest.update(hash_regular_file(entry.path())?.as_bytes());
        } else if metadata.is_dir() {
            digest.update(b"directory");
        } else {
            return Err(PumasError::InstallationFailed {
                message: "Native output contains a special file".into(),
            });
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_llama_cpp_output(root: &Path, proof: &LlamaCppInstallReceiptV1) -> Result<()> {
    let expected_launcher = if cfg!(windows) {
        "bin/llama-server.exe"
    } else {
        "bin/llama-server"
    };
    if proof.launcher_relative_path != expected_launcher
        || hash_native_tree(root)? != proof.output_tree_sha256
        || hash_regular_file(&root.join(expected_launcher))? != proof.launcher_sha256
    {
        return Err(PumasError::InstallationFailed {
            message: "Native output differs from its durable llama.cpp receipt; recovery required"
                .into(),
        });
    }
    Ok(())
}

/// Coordinates Torch cancellation with the irreversible publication boundary.
pub(crate) struct TorchInstallControl(AtomicU8);

/// Mutable output and input bytes belong to one native attempt. The permanent
/// lock file coordinates native installers and metadata mutations; it must
/// never be unlinked.
struct NativeInstallWorkspace {
    // Implicit destruction must preserve uncertain work. Reclamation is an
    // explicit fallible operation after all file/extraction effects settle.
    directory: PathBuf,
    attempt: String,
    _lock: NativeVersionsLock,
}

#[derive(Clone)]
pub(crate) struct NativeVersionsLock(Arc<File>);

impl Drop for NativeVersionsLock {
    fn drop(&mut self) {
        // Explicit unlock also releases locks inherited by a concurrently
        // spawning child before that child's close-on-exec takes effect.
        if Arc::strong_count(&self.0) == 1 {
            let _ = fs2::FileExt::unlock(&*self.0);
        }
    }
}

impl NativeVersionsLock {
    pub(crate) fn try_acquire(versions: &Path) -> Result<Self> {
        std::fs::create_dir_all(versions)
            .map_err(|error| PumasError::io_with_path(error, versions))?;
        let lock_path = versions.join(".llama-install.lock");
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| PumasError::io_with_path(error, &lock_path))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .map_err(|error| PumasError::io_with_path(error, &lock_path))?;
        Ok(Self(Arc::new(lock)))
    }

    pub(crate) async fn acquire(versions: PathBuf) -> Result<Self> {
        tokio::task::spawn_blocking(move || Self::try_acquire(&versions))
            .await
            .map_err(|error| {
                PumasError::Other(format!("Native mutation admission failed: {error}"))
            })?
    }
}

impl NativeInstallWorkspace {
    #[cfg(test)]
    fn create(versions: &Path, tag: &str) -> Result<Self> {
        Self::create_for_attempt(versions, tag, None)
    }

    fn create_for_attempt(versions: &Path, tag: &str, expected: Option<&str>) -> Result<Self> {
        VersionInstaller::validate_native_tag(tag)?;
        let lock = NativeVersionsLock::try_acquire(versions)?;
        let current = read_native_attempt(versions, tag)?;
        let identity = match (current, expected) {
            (Some(identity), Some(expected))
                if !identity.removed && identity.attempt == expected =>
            {
                identity
            }
            (_, Some(_)) => {
                return Err(PumasError::Validation {
                    field: "acquisition.consumer_recovery_required".into(),
                    message: "Native recovery attempt was removed or changed".into(),
                })
            }
            (Some(identity), None) if !identity.removed => identity,
            (previous, None) => {
                if path_exists_sync(&versions.join(tag))? {
                    return Err(PumasError::VersionAlreadyInstalled { tag: tag.into() });
                }
                if previous.is_some() {
                    mark_native_attempt_removed(versions, tag)?;
                }
                let mut entropy = [0u8; 16];
                getrandom::fill(&mut entropy).map_err(|error| {
                    PumasError::Other(format!("Native attempt identity failed: {error}"))
                })?;
                let identity = NativeAttemptIdentity {
                    schema_version: 1,
                    tag: tag.into(),
                    attempt: entropy.iter().map(|byte| format!("{byte:02x}")).collect(),
                    removed: false,
                };
                write_native_attempt(versions, &identity)?;
                identity
            }
        };
        let directory = native_workspace_path(versions, tag, &identity.attempt);
        std::fs::create_dir_all(&directory)
            .map_err(|error| PumasError::io_with_path(error, &directory))?;
        let metadata = std::fs::symlink_metadata(&directory)
            .map_err(|error| PumasError::io_with_path(error, &directory))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(PumasError::InstallationFailed {
                message: "Native acquisition stage is not a held directory".into(),
            });
        }
        sync_native_directory(versions)?;
        sync_native_directory(
            versions
                .parent()
                .ok_or_else(|| PumasError::Other("Native versions parent absent".into()))?,
        )?;
        Ok(Self {
            directory,
            attempt: identity.attempt,
            _lock: lock,
        })
    }

    fn path(&self) -> &Path {
        &self.directory
    }

    fn cleanup(self) -> Result<()> {
        std::fs::remove_dir_all(&self.directory).map_err(|error| PumasError::Io {
            message: format!("Native staging cleanup incomplete; retained workspace requires reconciliation: {error}"),
            path: Some(self.directory.clone()),
            source: Some(error),
        })?;
        sync_native_directory(
            self.directory
                .parent()
                .ok_or_else(|| PumasError::Other("Native workspace parent absent".into()))?,
        )
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LlamaCppInstallReceiptV1 {
    schema_version: u32,
    tag: String,
    metadata: InstalledVersionMetadata,
    metadata_sha256: String,
    output_tree_sha256: String,
    launcher_relative_path: String,
    launcher_sha256: String,
}

struct PreparedLlamaCppInstall {
    use_set: pumas_library::acquisition::AcquiredArtifactUse,
    stage: PathBuf,
    destination: PathBuf,
    custody: Arc<NativeInstallWorkspace>,
}

struct LlamaCppPublication {
    versions: PathBuf,
    app_id: AppId,
    manager: Arc<MetadataManager>,
    custody: Arc<NativeInstallWorkspace>,
    stage: PathBuf,
    destination: PathBuf,
    proof: LlamaCppInstallReceiptV1,
    #[cfg(test)]
    interrupt_after_native_rename: Option<Arc<AtomicBool>>,
}

async fn send_install_progress(
    sender: &mpsc::Sender<ProgressUpdate>,
    update: ProgressUpdate,
    cancel_flag: Arc<AtomicBool>,
    shutdown_flag: Arc<AtomicBool>,
) {
    tokio::select! {
        _ = sender.send(update) => {},
        _ = super::wait_for_install_cancel(shutdown_flag) => {},
        _ = super::wait_for_install_cancel(cancel_flag) => {},
    }
}

struct LlamaCppHttpAttemptHost {
    cancel_flag: Arc<AtomicBool>,
    shutdown_flag: Arc<AtomicBool>,
    progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
    progress_tx: mpsc::Sender<ProgressUpdate>,
    total_size: Option<u64>,
    started: Instant,
}

#[async_trait::async_trait]
impl pumas_library::acquisition::HttpAttemptHost for LlamaCppHttpAttemptHost {
    async fn pause_requested(&self) {
        super::wait_for_install_cancel(self.cancel_flag.clone()).await;
    }

    fn pause_requested_now(&self) -> bool {
        false
    }

    fn cancel_requested(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    async fn record_progress(&mut self, downloaded_for_file: u64) -> Result<()> {
        let elapsed = self.started.elapsed().as_secs_f64();
        let speed = (elapsed > 0.0).then_some(downloaded_for_file as f64 / elapsed);
        self.progress_tracker
            .write()
            .await
            .update_download_progress(downloaded_for_file, self.total_size, speed);
        send_install_progress(
            &self.progress_tx,
            ProgressUpdate::Download {
                downloaded_bytes: downloaded_for_file,
                total_bytes: self.total_size,
                speed_bytes_per_sec: speed,
            },
            self.cancel_flag.clone(),
            self.shutdown_flag.clone(),
        )
        .await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl pumas_library::acquisition::AcquisitionHost for LlamaCppHttpAttemptHost {
    async fn retry(
        &mut self,
        _attempt: u32,
        _delay: Option<std::time::Duration>,
        _error: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
}

fn prepare_native_acquisition_workspace(
    versions: PathBuf,
    tag: String,
    expected: Option<String>,
) -> Result<(Arc<NativeInstallWorkspace>, AcquisitionWorkspace)> {
    let custody = Arc::new(NativeInstallWorkspace::create_for_attempt(
        &versions,
        &tag,
        expected.as_deref(),
    )?);
    let relative = custody
        .path()
        .strip_prefix(&versions)
        .map_err(|_| PumasError::InstallationFailed {
            message: "Native acquisition stage escaped the versions root".into(),
        })?
        .to_path_buf();
    let execution_lease: Arc<dyn Send + Sync> = Arc::new(custody._lock.clone());
    let validator_lease = execution_lease.clone();
    let workspace = AcquisitionWorkspace::from_reserved_directory(
        &versions,
        &relative,
        execution_lease,
        move || {
            let _held = &validator_lease;
            Ok(())
        },
    )?;
    Ok((custody, workspace))
}

fn settled_result<T>(outcome: Result<T>, settlement: Result<()>) -> Result<T> {
    match (outcome, settlement) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(settlement)) => Err(PumasError::InstallationFailed {
            message: format!("{error}; settlement incomplete: {settlement}"),
        }),
    }
}

async fn settle_archive_file<T>(mut file: fs::File, path: &Path, outcome: Result<T>) -> Result<T> {
    let settlement = file.flush().await.map_err(|error| PumasError::Io {
        message: format!("Failed to flush archive: {error}"),
        path: Some(path.to_path_buf()),
        source: Some(error),
    });
    // Even an unsuccessful flush must settle queued blocking file work before
    // the stage owner may reclaim bytes. Dropping Tokio File is insufficient.
    drop(file.into_std().await);
    settled_result(outcome, settlement)
}

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

impl TorchCleanupState {
    fn harvest_finished_tasks(&mut self) {
        for mut task in std::mem::take(&mut self.tasks) {
            if task.is_finished() {
                match (&mut task).now_or_never() {
                    Some(Err(error)) => self.failures.push(error.to_string()),
                    Some(Ok(())) => continue,
                    None => self.tasks.push(task),
                }
            } else {
                self.tasks.push(task);
            }
        }
    }
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
        state.harvest_finished_tasks();
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

    #[cfg(test)]
    pub(crate) fn schedule(&self, work: impl FnOnce() + Send + 'static) {
        let mut state = self.state.lock().expect("Torch cleanup lock poisoned");
        if !state.closed {
            state.harvest_finished_tasks();
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
    shutdown_flag: Arc<AtomicBool>,
    github_client: Option<Arc<GitHubClient>>,
    acquisition_consumer: Option<Arc<AcquisitionConsumer>>,
    torch_control: Arc<TorchInstallControl>,
    torch_cleanup: Arc<TorchCleanupTasks>,
    torch_attempt_lock: Mutex<()>,
    #[cfg(test)]
    native_receipt_pause: Option<Arc<TorchPublicationPause>>,
    #[cfg(test)]
    interrupt_after_native_rename: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    native_recovery_pause: Option<Arc<NativeRecoveryPause>>,
    #[cfg(test)]
    torch_stage_override: Option<TorchStageOverride>,
    #[cfg(test)]
    torch_publication_pause: Option<Arc<torch::TorchPublicationPause>>,
    #[cfg(test)]
    torch_stage_pause: Option<Arc<torch::TorchPublicationPause>>,
}

#[cfg(test)]
pub(crate) struct NativeRecoveryPause {
    pub(crate) reached: tokio::sync::Notify,
    resume: StdMutex<std::sync::mpsc::Receiver<()>>,
}

#[cfg(test)]
impl NativeRecoveryPause {
    pub(crate) fn new() -> (Self, std::sync::mpsc::Sender<()>) {
        let (resume, receiver) = std::sync::mpsc::channel();
        (
            Self {
                reached: tokio::sync::Notify::new(),
                resume: StdMutex::new(receiver),
            },
            resume,
        )
    }

    fn block(&self) {
        self.reached.notify_one();
        let _ = self.resume.lock().unwrap().recv();
    }
}

impl VersionInstaller {
    /// Create a new version installer.
    ///
    /// For llama.cpp, configure the returned installer with
    /// [`Self::with_acquisition`] before calling [`Self::install_version`].
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
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            github_client: None,
            acquisition_consumer: None,
            torch_control: Arc::new(TorchInstallControl::new()),
            torch_cleanup: Arc::new(TorchCleanupTasks::default()),
            torch_attempt_lock: Mutex::new(()),
            #[cfg(test)]
            native_receipt_pause: None,
            #[cfg(test)]
            interrupt_after_native_rename: None,
            #[cfg(test)]
            native_recovery_pause: None,
            #[cfg(test)]
            torch_stage_override: None,
            #[cfg(test)]
            torch_publication_pause: None,
            #[cfg(test)]
            torch_stage_pause: None,
        }
    }

    /// Configure direct llama.cpp installation with the existing shared
    /// acquisition capability. Initializes the GitHub metadata client from this
    /// installer's cache; creates no acquisition store, service, or downloader.
    /// Reads retained records from that same store and reconciles native uses
    /// before returning an installer that can admit new work. Recovery failure
    /// drains this consumer scope before returning the error; the caller retains
    /// the shared service's shutdown responsibility.
    /// External customer compatibility acceptance remains pending.
    pub async fn with_acquisition(
        mut self,
        acquisition: Arc<pumas_library::acquisition::AcquisitionService>,
    ) -> Result<Self> {
        if self.app_id != AppId::LlamaCpp {
            return Err(PumasError::Config {
                message: "Shared artifact acquisition is currently supported for llama.cpp".into(),
            });
        }
        let cache = self
            .launcher_root
            .join("launcher-data")
            .join(PathsConfig::CACHE_DIR_NAME);
        let store = acquisition.store().clone();
        let consumer = Arc::new(acquisition.open_consumer("runtime.llama.cpp")?);
        self.acquisition_consumer = Some(consumer.clone());
        let configured = async {
            let (client, records) = consumer
                .run_blocking("initialize native installer recovery", move || {
                    Ok((GitHubClient::new(cache)?, store.acquisitions()?))
                })
                .await?;
            self.github_client = Some(Arc::new(client));
            self.reconcile_retained_llama_cpp(records.into_values().collect())
                .await
        }
        .await;
        if let Err(error) = configured {
            return Err(match consumer.shutdown().await {
                Ok(()) => error,
                Err(settlement) => PumasError::InstallationFailed {
                    message: format!("{error}; native recovery shutdown: {settlement}"),
                },
            });
        }
        Ok(self)
    }

    #[cfg(test)]
    pub(crate) fn with_native_recovery_pause(mut self, pause: Arc<NativeRecoveryPause>) -> Self {
        self.native_recovery_pause = Some(pause);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_native_receipt_pause(mut self, pause: Arc<TorchPublicationPause>) -> Self {
        self.native_receipt_pause = Some(pause);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_native_rename_interruption(mut self, once: Arc<AtomicBool>) -> Self {
        self.interrupt_after_native_rename = Some(once);
        self
    }

    pub(crate) fn with_github_client(mut self, client: Arc<GitHubClient>) -> Self {
        self.github_client = Some(client);
        self
    }

    pub(crate) fn with_acquisition_consumer(
        mut self,
        consumer: Option<Arc<AcquisitionConsumer>>,
    ) -> Self {
        self.acquisition_consumer = consumer;
        self
    }

    /// Reconcile historic native uses from the shared store before admitting
    /// new work. Publisher lookup cannot change a retained manifest or receipt.
    pub(crate) async fn reconcile_retained_llama_cpp(
        &self,
        records: Vec<pumas_library::acquisition::AcquisitionRecord>,
    ) -> Result<()> {
        let consumer = self
            .acquisition_consumer
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: "Native recovery requires the shared acquisition consumer".into(),
            })?;
        for record in records {
            if record.demand.consumer != consumer.owner()
                || !matches!(
                    &record.phase,
                    pumas_library::acquisition::AcquisitionPhase::Using { .. }
                        | pumas_library::acquisition::AcquisitionPhase::Adopted { .. }
                )
            {
                continue;
            }
            let (version_platform, attempt) =
                record
                    .demand
                    .operation
                    .rsplit_once(':')
                    .ok_or_else(|| PumasError::Validation {
                        field: "acquisition.consumer_recovery_required".into(),
                        message: "Retained native demand has no exact attempt identity".into(),
                    })?;
            let (tag, platform) =
                version_platform
                    .rsplit_once(':')
                    .ok_or_else(|| PumasError::Validation {
                        field: "acquisition.consumer_recovery_required".into(),
                        message: "Retained native demand has no exact version/platform".into(),
                    })?;
            Self::validate_native_tag(tag)?;
            if platform != format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH) {
                return Err(PumasError::Validation {
                    field: "acquisition.consumer_recovery_required".into(),
                    message: "Retained native acquisition belongs to another platform".into(),
                });
            }
            let tag = tag.to_owned();
            let versions = self.versions_dir();
            let lookup_versions = versions.clone();
            let lookup_tag = tag.clone();
            let current = consumer
                .run_blocking("read retained native attempt", move || {
                    read_native_attempt(&lookup_versions, &lookup_tag)
                })
                .await?
                .ok_or_else(|| PumasError::Validation {
                    field: "acquisition.consumer_recovery_required".into(),
                    message: "Retained native acquisition has no attempt identity".into(),
                })?;
            if current.removed || current.attempt != attempt {
                // Removal revokes the attempt before output reclamation. A new
                // current attempt proves explicit removal/reinstall occurred.
                continue;
            }
            if matches!(
                &record.phase,
                pumas_library::acquisition::AcquisitionPhase::Adopted { .. }
            ) {
                let receipt_consumer = consumer.clone();
                let retained = record.clone();
                let receipt = consumer
                    .run_blocking("read retained native completion receipt", move || {
                        receipt_consumer.completion_receipt(&retained)
                    })
                    .await?
                    .ok_or_else(|| PumasError::Validation {
                        field: "acquisition.consumer_recovery_required".into(),
                        message: "Adopted native installation has no exact completion receipt"
                            .into(),
                    })?;
                let proof: LlamaCppInstallReceiptV1 = serde_json::from_value(receipt.payload)
                    .map_err(|error| PumasError::Validation {
                        field: "acquisition.consumer_recovery_required".into(),
                        message: format!("Adopted native receipt is malformed: {error}"),
                    })?;
                let versions_for_cleanup = versions.clone();
                let tag_for_cleanup = tag.to_owned();
                let attempt_for_cleanup = attempt.to_owned();
                let expected_workspace = record.workspace.clone();
                let manager = self.metadata_manager.clone();
                let app_id = self.app_id;
                #[cfg(test)]
                let recovery_pause = self.native_recovery_pause.clone();
                consumer.run_blocking("verify adopted native output and clean workspace", move || {
                    #[cfg(test)]
                    if let Some(pause) = recovery_pause { pause.block(); }
                    let _lock = NativeVersionsLock::try_acquire(&versions_for_cleanup)?;
                    let current = read_native_attempt(&versions_for_cleanup, &tag_for_cleanup)?;
                    let Some(current) = current else {
                        return Err(PumasError::Validation {
                            field: "acquisition.consumer_recovery_required".into(),
                            message: "Adopted native receipt has no attempt identity".into(),
                        });
                    };
                    if current.removed || current.attempt != attempt_for_cleanup {
                        return Ok(());
                    }
                    let workspace = native_workspace_path(
                        &versions_for_cleanup,
                        &tag_for_cleanup,
                        &attempt_for_cleanup,
                    );
                    let relative_workspace = workspace
                        .strip_prefix(&versions_for_cleanup)
                        .map_err(|_| PumasError::Validation {
                            field: "acquisition.consumer_recovery_required".into(),
                            message: "Adopted native workspace escaped its versions root".into(),
                        })?;
                    let actual_workspace =
                        pumas_library::acquisition::AcquisitionWorkspace::identity_for_reserved_directory(
                            &versions_for_cleanup,
                            relative_workspace,
                        )?;
                    if actual_workspace != expected_workspace {
                        return Err(PumasError::Validation {
                            field: "acquisition.consumer_recovery_required".into(),
                            message: "Adopted native workspace identity changed".into(),
                        });
                    }
                    let destination = versions_for_cleanup.join(&tag_for_cleanup);
                    let metadata = std::fs::symlink_metadata(&destination)
                        .map_err(|error| PumasError::io_with_path(error, &destination))?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(PumasError::InstallationFailed {
                            message: "Adopted llama.cpp output is not a safe directory".into(),
                        });
                    }
                    Self::validate_llama_cpp_receipt(&tag_for_cleanup, &proof, &destination)?;
                    verify_llama_cpp_output(&destination, &proof)?;
                    match manager.get_installed_version(&tag_for_cleanup, Some(app_id))? {
                        Some(installed) if metadata_matches(&installed, &proof.metadata) => {}
                        _ => {
                            return Err(PumasError::InstallationFailed {
                                message: "Adopted llama.cpp metadata does not match its receipt"
                                    .into(),
                            })
                        }
                    }
                    cleanup_native_workspace_if_present(&workspace, &versions_for_cleanup)
                })
                .await?;
                continue;
            }
            let recovery_versions = versions.clone();
            let recovery_tag = tag.clone();
            let recovery_attempt = attempt.to_owned();
            let (custody, workspace) = consumer
                .run_blocking("open retained native workspace", move || {
                    prepare_native_acquisition_workspace(
                        recovery_versions,
                        recovery_tag,
                        Some(recovery_attempt),
                    )
                })
                .await?;
            let custody_for_reconcile = custody.clone();
            let manager = self.metadata_manager.clone();
            let app_id = self.app_id;
            consumer
                .reconcile(
                    record.demand,
                    record.manifest,
                    workspace,
                    move |receipt, use_set| async move {
                        let proof: LlamaCppInstallReceiptV1 =
                            serde_json::from_value(receipt.payload).map_err(|error| {
                                PumasError::Validation {
                                    field: "acquisition.consumer_recovery_required".into(),
                                    message: format!(
                                        "Retained native receipt is malformed: {error}"
                                    ),
                                }
                            })?;
                        use_set
                            .run_blocking("reconcile retained native installation", move || {
                                Self::reconcile_llama_cpp_installation(
                                    &versions,
                                    &tag,
                                    app_id,
                                    &manager,
                                    custody_for_reconcile,
                                    proof,
                                )
                            })
                            .await
                    },
                )
                .await?
                .ok_or_else(|| PumasError::Validation {
                    field: "acquisition.consumer_recovery_required".into(),
                    message: "Retained native use disappeared before reconciliation".into(),
                })?;
            #[cfg(test)]
            let recovery_pause = self.native_recovery_pause.clone();
            consumer
                .run_blocking("clean reconciled native workspace", move || {
                    #[cfg(test)]
                    if let Some(pause) = recovery_pause {
                        pause.block();
                    }
                    Arc::try_unwrap(custody)
                        .map_err(|workspace| PumasError::InstallationFailed {
                            message: format!(
                                "Reconciled native workspace remains in use: {}",
                                workspace.path().display()
                            ),
                        })?
                        .cleanup()
                })
                .await?;
        }
        Ok(())
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

    pub(crate) fn with_shutdown_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.shutdown_flag = flag;
        self
    }

    async fn send_progress(&self, sender: &mpsc::Sender<ProgressUpdate>, update: ProgressUpdate) {
        send_install_progress(
            sender,
            update,
            self.cancel_flag.clone(),
            self.shutdown_flag.clone(),
        )
        .await;
    }

    async fn wait_for_cancellation(&self) {
        tokio::select! {
            _ = super::wait_for_install_cancel(self.shutdown_flag.clone()) => {},
            _ = super::wait_for_install_cancel(self.cancel_flag.clone()) => {},
        }
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
        self.install_version_with_torch_input(tag, release, progress_tx, None)
            .await
    }

    pub(crate) async fn install_version_with_torch_input(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
        torch_input: Option<TorchInstallInput>,
    ) -> Result<()> {
        match self.app_id {
            AppId::Ollama => self.install_ollama_binary(tag, release, progress_tx).await,
            AppId::LlamaCpp => {
                self.install_llama_cpp_binary(tag, release, progress_tx)
                    .await
            }
            AppId::Torch => {
                self.install_torch_runtime(tag, release, progress_tx, torch_input)
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
        let _attempt =
            self.torch_attempt_lock
                .try_lock()
                .map_err(|_| PumasError::InstallationFailed {
                    message: "llama.cpp installation already active".into(),
                })?;
        self.torch_control.start();
        let result = self
            .install_llama_cpp_binary_inner(tag, release, progress_tx)
            .await;
        self.torch_control.finish();
        result
    }

    async fn install_llama_cpp_binary_inner(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        let consumer = self
            .acquisition_consumer
            .clone()
            .ok_or_else(|| PumasError::Config {
                message: "llama.cpp requires shared acquisition; configure with VersionInstaller::with_acquisition".into(),
            })?;
        Self::validate_native_tag(tag)?;
        info!("Starting llama.cpp binary installation for {}", tag);

        let asset = self.select_llama_cpp_asset(&release.assets)?;
        let asset_name = asset.name.clone();
        let github = self
            .github_client
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: "llama.cpp requires the configured shared GitHub asset resolver".into(),
            })?;
        // GitHub discovery appends a runtime flavor to the upstream release
        // tag. Resolve the publisher release, while retaining the full local tag.
        let publisher_tag = release
            .tag_name
            .rsplit_once('+')
            .map_or(release.tag_name.as_str(), |(publisher, _)| publisher);
        let selection = github
            .resolve_release_asset(self.app_id.github_repo(), publisher_tag, &asset_name)
            .await
            .map_err(|error| PumasError::InstallationFailed {
                message: format!("Could not verify llama.cpp release asset: {error}"),
            })?;
        let manifest = selection.manifest().clone();
        let selected_file =
            manifest
                .files()
                .first()
                .ok_or_else(|| PumasError::InstallationFailed {
                    message: "Verified llama.cpp release selection has no file".into(),
                })?;
        let total_size = selected_file.expected_size();
        let download_url = selection.download_url().to_owned();

        info!(
            "Selected publisher-verified llama.cpp asset: {} ({} bytes)",
            asset_name,
            total_size.unwrap_or_default()
        );

        let log_dir = self.logs_dir();
        let log_path = log_dir.join(format!(
            "install-llama-cpp-{}-{}.log",
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        let versions = self.versions_dir();
        let workspace_versions = versions.clone();
        let workspace_tag = tag.to_owned();
        let (custody, workspace) = consumer
            .run_blocking("prepare native installation workspace", move || {
                std::fs::create_dir_all(&log_dir).ok();
                std::fs::create_dir_all(&workspace_versions)
                    .map_err(|error| PumasError::io_with_path(error, &workspace_versions))?;
                prepare_native_acquisition_workspace(workspace_versions, workspace_tag, None)
            })
            .await?;

        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.start_installation(
                tag,
                total_size,
                None,
                Some(log_path.to_string_lossy().as_ref()),
            );
        }

        let demand = AcquisitionDemand {
            consumer: consumer.owner().to_owned(),
            operation: format!(
                "{tag}:{}-{}:{}",
                std::env::consts::OS,
                std::env::consts::ARCH,
                custody.attempt
            ),
        };
        let custody_for_reconcile = custody.clone();
        let manager_for_reconcile = self.metadata_manager.clone();
        let versions_for_reconcile = versions.clone();
        let tag_for_reconcile = tag.to_owned();
        let cancel_for_reconcile = self.cancel_flag.clone();
        let control_for_reconcile = self.torch_control.clone();
        let app_id = self.app_id;
        let reconcile = consumer
            .reconcile(
                demand.clone(),
                manifest.clone(),
                workspace.clone(),
                move |receipt, use_set| async move {
                    let proof: LlamaCppInstallReceiptV1 = serde_json::from_value(receipt.payload)
                        .map_err(|error| {
                        PumasError::Validation {
                            field: "acquisition.consumer_receipt".into(),
                            message: format!("llama.cpp receipt is malformed: {error}"),
                        }
                    })?;
                    if cancel_for_reconcile.load(Ordering::SeqCst)
                        || !control_for_reconcile.try_begin_publication()
                    {
                        return Err(VersionInstaller::cancellation_error());
                    }
                    use_set
                        .run_blocking("reconcile verified llama.cpp installation", move || {
                            Self::reconcile_llama_cpp_installation(
                                &versions_for_reconcile,
                                &tag_for_reconcile,
                                app_id,
                                &manager_for_reconcile,
                                custody_for_reconcile,
                                proof,
                            )
                        })
                        .await?;
                    Ok(())
                },
            )
            .await;

        let result = match reconcile {
            Ok(Some(())) => Ok(()),
            Ok(None) => {
                let client = reqwest::Client::builder()
                    .connect_timeout(InstallationConfig::URL_FETCH_TIMEOUT)
                    .user_agent("pumas-library")
                    .build()
                    .map_err(|error| PumasError::Network {
                        message: "Failed to create shared artifact HTTP client".into(),
                        cause: Some(error.to_string()),
                    })?;
                let host = LlamaCppHttpAttemptHost {
                    cancel_flag: self.cancel_flag.clone(),
                    shutdown_flag: self.shutdown_flag.clone(),
                    progress_tracker: self.progress_tracker.clone(),
                    progress_tx: progress_tx.clone(),
                    total_size,
                    started: Instant::now(),
                };
                let retry = AcquisitionRetryPolicy {
                    attempts: Some(InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS),
                    elapsed: std::time::Duration::ZERO,
                    backoff: RetryConfig::new()
                        .with_max_attempts(InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS)
                        .with_base_delay(std::time::Duration::from_secs(2)),
                };
                let stage = custody.path().join("output");
                let destination = versions.join(tag);
                let asset_for_prepare = asset_name.clone();
                let tag_for_prepare = tag.to_owned();
                let release_date = Some(release.published_at.clone());
                let release_notes = release.body.clone();
                let metadata = InstalledVersionMetadata {
                    path: tag.to_owned(),
                    installed_date: Utc::now().to_rfc3339(),
                    release_tag: tag.to_owned(),
                    python_version: None,
                    git_commit: None,
                    release_date,
                    release_notes,
                    // Retrieval URLs are ephemeral source access, not durable
                    // installation provenance.
                    download_url: None,
                    size: total_size,
                    requirements_hash: None,
                    dependencies_installed: Some(true),
                };
                let metadata_for_prepare = metadata.clone();
                let versions_for_publish = versions.clone();
                let manager_for_publish = self.metadata_manager.clone();
                let progress_for_publish = self.progress_tracker.clone();
                let progress_tx_for_publish = progress_tx.clone();
                let cancel_for_progress = self.cancel_flag.clone();
                let shutdown_for_progress = self.shutdown_flag.clone();
                #[cfg(test)]
                let native_receipt_pause = self.native_receipt_pause.clone();
                #[cfg(test)]
                let interrupt_after_native_rename = self.interrupt_after_native_rename.clone();
                let cancel_for_prepare = self.cancel_flag.clone();
                let control_for_prepare = self.torch_control.clone();
                let custody_for_prepare = custody.clone();
                let versions_for_cancel = versions.clone();
                let tag_for_cancel = tag.to_owned();
                let request = AcquisitionHttpRequest {
                    demand,
                    manifest,
                    workspace,
                    sources: vec![AcquisitionHttpSource {
                        url: download_url,
                        authorization: None,
                    }],
                    retry,
                };
                consumer
                    .acquire_http(
                        request,
                        client,
                        Box::new(host),
                        move |use_set| async move {
                            let archive = use_set.open_file(0).await?;
                            let stage_for_extract = stage.clone();
                            let asset_name = asset_for_prepare.clone();
                            let extracted = use_set
                                .run_blocking("extract publisher-verified llama.cpp archive", move || {
                                    if path_exists_sync(&stage_for_extract)? {
                                        return Err(PumasError::InstallationFailed {
                                            message: "llama.cpp output stage already contains unresolved work".into(),
                                        });
                                    }
                                    std::fs::create_dir(&stage_for_extract)
                                        .map_err(|error| PumasError::io_with_path(error, &stage_for_extract))?;
                                    VersionInstaller::extract_llama_cpp_binary_from(
                                        archive,
                                        &stage_for_extract,
                                        &asset_name,
                                    )?;
                                    sync_native_tree(&stage_for_extract)?;
                                    sync_native_directory(stage_for_extract.parent().ok_or_else(|| PumasError::Other("Native output parent absent".into()))?)?;
                                    let output_tree_sha256 = hash_native_tree(&stage_for_extract)?;
                                    let launcher = stage_for_extract.join(if cfg!(windows) {
                                        "bin/llama-server.exe"
                                    } else {
                                        "bin/llama-server"
                                    });
                                    let launcher_sha256 = hash_regular_file(&launcher)?;
                                    Ok((output_tree_sha256, launcher_sha256))
                                })
                                .await?;
                            let metadata_sha256 = hash_metadata(&metadata_for_prepare)?;
                            let proof = LlamaCppInstallReceiptV1 {
                                schema_version: 1,
                                tag: tag_for_prepare,
                                metadata: metadata_for_prepare,
                                metadata_sha256,
                                output_tree_sha256: extracted.0,
                                launcher_relative_path: if cfg!(windows) {
                                    "bin/llama-server.exe".into()
                                } else {
                                    "bin/llama-server".into()
                                },
                                launcher_sha256: extracted.1,
                            };
                            #[cfg(test)]
                            if let Some(pause) = native_receipt_pause {
                                pause.reached.notify_one();
                                let permit = pause.resume.acquire().await.map_err(|_| {
                                    PumasError::Other("Native receipt test pause closed".into())
                                })?;
                                permit.forget();
                            }
                            // Once this receipt can be persisted, restart will
                            // finish publication from its staged output. Win
                            // the cancellation race first so an accepted
                            // cancel cannot turn into a later installation.
                            if cancel_for_prepare.load(Ordering::SeqCst)
                                || !control_for_prepare.try_begin_publication()
                            {
                                use_set.withdraw_after_cleanup(move || {
                                    let mut identity = read_native_attempt(&versions_for_cancel, &tag_for_cancel)?
                                        .ok_or_else(|| PumasError::Other("Cancelled native attempt identity absent".into()))?;
                                    if identity.attempt != custody_for_prepare.attempt || identity.removed {
                                        return Err(PumasError::Other("Cancelled native attempt identity changed".into()));
                                    }
                                    identity.removed = true;
                                    write_native_attempt(&versions_for_cancel, &identity)?;
                                    cleanup_native_workspace_if_present(custody_for_prepare.path(), &versions_for_cancel)
                                }).await?;
                                return Err(VersionInstaller::cancellation_error());
                            }
                            let prepared = PreparedLlamaCppInstall {
                                use_set,
                                stage,
                                destination: destination.clone(),
                                custody: custody_for_prepare,
                            };
                            Ok((prepared, serde_json::to_value(proof)?))
                        },
                        move |prepared, issued| async move {
                            let proof: LlamaCppInstallReceiptV1 =
                                serde_json::from_value(issued.payload).map_err(|error| {
                                    PumasError::Validation {
                                        field: "acquisition.consumer_receipt".into(),
                                        message: format!("llama.cpp publication receipt is malformed: {error}"),
                                    }
                                })?;
                            prepared
                                .use_set
                                .run_blocking("publish verified llama.cpp installation", move || {
                                    VersionInstaller::publish_llama_cpp_installation(
                                        LlamaCppPublication {
                                            versions: versions_for_publish,
                                            app_id,
                                            manager: manager_for_publish,
                                            custody: prepared.custody,
                                            stage: prepared.stage,
                                            destination: prepared.destination,
                                            proof,
                                            #[cfg(test)]
                                            interrupt_after_native_rename,
                                        },
                                    )
                                })
                                .await?;
                            progress_for_publish.write().await.update_stage(
                                InstallationStage::Setup,
                                100.0,
                                Some("Installation complete"),
                            );
                            send_install_progress(
                                &progress_tx_for_publish,
                                ProgressUpdate::Setup {
                                    message: "Installation complete".into(),
                                },
                                cancel_for_progress,
                                shutdown_for_progress,
                            )
                            .await;
                            Ok(())
                        },
                    )
                    .await
            }
            Err(error) => Err(error),
        };

        let result = match result {
            Err(PumasError::DownloadPaused) if self.cancel_flag.load(Ordering::SeqCst) => {
                Err(PumasError::DownloadCancelled)
            }
            result => result,
        };

        let result = if result.is_ok() {
            let cleanup = consumer
                .run_blocking("clean published native workspace", move || {
                    Arc::try_unwrap(custody)
                        .map_err(|workspace| PumasError::InstallationFailed {
                            message: format!(
                                "Native workspace remains in use: {}",
                                workspace.path().display()
                            ),
                        })?
                        .cleanup()
                })
                .await;
            settled_result(result, cleanup)
        } else {
            result
        };

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
            self.send_progress(
                progress_tx,
                ProgressUpdate::Download {
                    downloaded_bytes: total_size,
                    total_bytes: Some(total_size),
                    speed_bytes_per_sec: None,
                },
            )
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
        self.send_progress(
            progress_tx,
            ProgressUpdate::StageChanged {
                stage: InstallationStage::Extract,
                message: "Extracting binary...".to_string(),
            },
        )
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

    fn publish_native_stage(
        stage: &Path,
        destination: &Path,
        finalize: impl FnOnce() -> std::result::Result<(), (PumasError, bool)>,
    ) -> Result<()> {
        match std::fs::symlink_metadata(destination) {
            Ok(_) => {
                return Err(PumasError::InstallationFailed {
                    message: "Native version destination already exists".into(),
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(PumasError::io_with_path(error, destination)),
        }
        sync_native_tree(stage)?;
        std::fs::rename(stage, destination)
            .map_err(|e| PumasError::io_with_path(e, destination))?;
        sync_native_directory(
            stage
                .parent()
                .ok_or_else(|| PumasError::Other("Native stage parent absent".into()))?,
        )?;
        sync_native_directory(
            destination
                .parent()
                .ok_or_else(|| PumasError::Other("Native destination parent absent".into()))?,
        )?;
        if let Err((error, can_withdraw)) = finalize() {
            if !can_withdraw {
                return Err(error);
            }
            // Only this attempt's newly published directory may be withdrawn.
            std::fs::rename(destination, stage).map_err(|rollback| {
                PumasError::InstallationFailed {
                    message: format!(
                        "Native finalization failed: {error}; stage withdrawal failed: {rollback}"
                    ),
                }
            })?;
            sync_native_directory(
                stage
                    .parent()
                    .ok_or_else(|| PumasError::Other("Native stage parent absent".into()))?,
            )?;
            sync_native_directory(
                destination
                    .parent()
                    .ok_or_else(|| PumasError::Other("Native destination parent absent".into()))?,
            )?;
            return Err(error);
        }
        Ok(())
    }

    fn publish_llama_cpp_installation(publication: LlamaCppPublication) -> Result<()> {
        let LlamaCppPublication {
            versions,
            app_id,
            manager,
            custody,
            stage,
            destination,
            proof,
            #[cfg(test)]
            interrupt_after_native_rename,
        } = publication;
        Self::validate_llama_cpp_receipt(&proof.tag, &proof, &stage)?;
        if manager
            .get_installed_version(&proof.tag, Some(app_id))?
            .is_some()
        {
            return Err(PumasError::VersionAlreadyInstalled {
                tag: proof.tag.clone(),
            });
        }
        Self::publish_native_stage(&stage, &destination, || {
            // This test-only interruption models loss after publication is
            // durable but before native metadata is committed.
            #[cfg(test)]
            if interrupt_after_native_rename
                .as_ref()
                .is_some_and(|once| once.swap(false, Ordering::SeqCst))
            {
                return Err((
                    PumasError::InstallationFailed {
                        message: "Test interruption after native rename before metadata".into(),
                    },
                    false,
                ));
            }
            match manager.update_installed_version(
                &proof.tag,
                proof.metadata.clone(),
                Some(app_id),
            ) {
                Ok(()) => Ok(()),
                Err(error) => match manager.get_installed_version(&proof.tag, Some(app_id)) {
                    Ok(None) => Err((error, true)),
                    _ => Err((
                        PumasError::InstallationFailed {
                            message: format!("Native metadata publication is uncertain; retained output requires reconciliation: {error}"),
                        },
                        false,
                    )),
                },
            }
        })?;
        sync_native_metadata(&versions, app_id)?;
        drop(custody);
        Ok(())
    }

    fn reconcile_llama_cpp_installation(
        versions: &Path,
        tag: &str,
        app_id: AppId,
        manager: &MetadataManager,
        custody: Arc<NativeInstallWorkspace>,
        proof: LlamaCppInstallReceiptV1,
    ) -> Result<()> {
        Self::validate_llama_cpp_receipt(tag, &proof, custody.path().join("output").as_path())?;
        let destination = versions.join(tag);
        let staged_output = custody.path().join("output");
        match std::fs::symlink_metadata(&destination) {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(PumasError::InstallationFailed {
                        message: "Retained llama.cpp destination is not a directory".into(),
                    });
                }
                verify_llama_cpp_output(&destination, &proof)?;
                if path_exists_sync(&staged_output)? {
                    return Err(PumasError::InstallationFailed {
                        message: "Both staged and published llama.cpp output exist".into(),
                    });
                }
                match manager.get_installed_version(tag, Some(app_id))? {
                    Some(current) if metadata_matches(&current, &proof.metadata) => {}
                    Some(_) => {
                        return Err(PumasError::InstallationFailed {
                            message: "Installed llama.cpp metadata conflicts with its receipt"
                                .into(),
                        })
                    }
                    None => manager.update_installed_version(
                        tag,
                        proof.metadata.clone(),
                        Some(app_id),
                    )?,
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                verify_llama_cpp_output(&staged_output, &proof)?;
                if manager.get_installed_version(tag, Some(app_id))?.is_some() {
                    return Err(PumasError::InstallationFailed {
                        message: "Installed metadata exists without its llama.cpp output".into(),
                    });
                }
                Self::publish_native_stage(&staged_output, &destination, || {
                    match manager.update_installed_version(
                        tag,
                        proof.metadata.clone(),
                        Some(app_id),
                    ) {
                        Ok(()) => Ok(()),
                        Err(error) => match manager.get_installed_version(tag, Some(app_id)) {
                            Ok(None) => Err((error, true)),
                            _ => Err((
                                PumasError::InstallationFailed {
                                    message: format!(
                                        "Native metadata recovery is uncertain: {error}"
                                    ),
                                },
                                false,
                            )),
                        },
                    }
                })?;
            }
            Err(error) => return Err(PumasError::io_with_path(error, &destination)),
        }
        sync_native_tree(&destination)?;
        sync_native_directory(versions)?;
        sync_native_metadata(versions, app_id)?;
        drop(custody);
        Ok(())
    }

    fn validate_llama_cpp_receipt(
        tag: &str,
        proof: &LlamaCppInstallReceiptV1,
        output: &Path,
    ) -> Result<()> {
        if proof.schema_version != 1
            || proof.tag != tag
            || proof.metadata.path != tag
            || proof.metadata.release_tag != tag
            || hash_metadata(&proof.metadata)? != proof.metadata_sha256
        {
            return Err(PumasError::Validation {
                field: "acquisition.consumer_receipt".into(),
                message: "llama.cpp receipt does not match its exact version or metadata".into(),
            });
        }
        if path_exists_sync(output)? {
            verify_llama_cpp_output(output, proof)?;
        }
        Ok(())
    }

    fn validate_native_tag(tag: &str) -> Result<()> {
        if tag.is_empty()
            || !tag.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+')
            })
            || tag == "."
            || tag == ".."
        {
            return Err(PumasError::InstallationFailed {
                message: "Invalid native version tag".into(),
            });
        }
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
    #[cfg(test)]
    fn extract_llama_cpp_binary(
        archive_path: &Path,
        version_dir: &Path,
        asset_name: &str,
    ) -> Result<()> {
        let archive = File::open(archive_path)
            .map_err(|error| PumasError::io_with_path(error, archive_path))?;
        Self::extract_llama_cpp_binary_from(archive, version_dir, asset_name)
    }

    fn extract_llama_cpp_binary_from(
        archive: File,
        version_dir: &Path,
        asset_name: &str,
    ) -> Result<()> {
        info!("Extracting llama.cpp binary from {}", asset_name);

        if asset_name.ends_with(".tar.zst") {
            Self::extract_tar_zst_from(archive, version_dir)?;
        } else if asset_name.ends_with(".tgz") || asset_name.ends_with(".tar.gz") {
            Self::extract_tarball_from(archive, version_dir)?;
        } else if asset_name.ends_with(".zip") {
            Self::extract_zip_from(archive, version_dir)?;
        } else {
            return Err(PumasError::InstallationFailed {
                message: format!("Unsupported llama.cpp archive format: {}", asset_name),
            });
        }

        Self::validate_native_output(version_dir)?;
        let server_binary = Self::find_named_binary(version_dir, &["llama-server", "server"])?
            .ok_or_else(|| PumasError::InstallationFailed {
                message: "Could not find llama-server in extracted archive".to_string(),
            })?;
        if std::fs::metadata(&server_binary)
            .map_err(|e| PumasError::io_with_path(e, &server_binary))?
            .len()
            == 0
        {
            return Err(PumasError::InstallationFailed {
                message: "Extracted llama.cpp server is empty".into(),
            });
        }
        let launch_binary = Self::install_llama_cpp_launch_binary(version_dir, &server_binary)?;
        Self::make_binary_executable(&launch_binary)?;

        info!(
            "llama.cpp server binary available at {}",
            launch_binary.display()
        );
        Ok(())
    }

    fn validate_native_output(root: &Path) -> Result<()> {
        let canonical_root = root
            .canonicalize()
            .map_err(|e| PumasError::io_with_path(e, root))?;
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = entry.map_err(|e| PumasError::InstallationFailed {
                message: format!("Invalid native output: {e}"),
            })?;
            if entry.file_type().is_symlink() {
                let target = entry
                    .path()
                    .canonicalize()
                    .map_err(|e| PumasError::io_with_path(e, entry.path()))?;
                let link = std::fs::read_link(entry.path())
                    .map_err(|e| PumasError::io_with_path(e, entry.path()))?;
                if link.is_absolute() || !target.starts_with(&canonical_root) || !target.is_file() {
                    return Err(PumasError::InstallationFailed {
                        message: "Native archive contains an unsafe link".into(),
                    });
                }
            }
        }
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
            let relative_dir = binary_dir.strip_prefix(version_dir).map_err(|_| {
                PumasError::InstallationFailed {
                    message: "llama.cpp binary is outside staged output".into(),
                }
            })?;
            let wrapper = format!(
                "#!/bin/sh\nROOT=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd) || exit 1\nBINARY_DIR=\"$ROOT\"/{}\nexport LD_LIBRARY_PATH=\"$BINARY_DIR${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}\"\nexec \"$BINARY_DIR/{}\" \"$@\"\n",
                shell_single_quote(&relative_dir.to_string_lossy()),
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
        Self::extract_tar_zst_from(file, dest_dir)
    }

    fn extract_tar_zst_from(file: File, dest_dir: &Path) -> Result<()> {
        let decoder = zstd::Decoder::new(BufReader::new(file)).map_err(|e| PumasError::Io {
            message: format!("Failed to create zstd decoder: {}", e),
            path: Some(dest_dir.to_path_buf()),
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
        self.send_progress(
            progress_tx,
            ProgressUpdate::StageChanged {
                stage: InstallationStage::Setup,
                message: "Finalizing installation...".to_string(),
            },
        )
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
        self.send_progress(
            progress_tx,
            ProgressUpdate::Setup {
                message: "Installation complete".to_string(),
            },
        )
        .await;

        info!("Ollama installation of {} finalized", tag);
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
        self.send_progress(
            progress_tx,
            ProgressUpdate::StageChanged {
                stage: InstallationStage::Download,
                message: "Starting download...".to_string(),
            },
        )
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
            self.check_cancelled()?;
            let request = tokio::select! {
                result = client.get(url).send() => result,
                _ = self.wait_for_cancellation() => return Err(Self::cancellation_error()),
            };
            match request {
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
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))) => {},
                        _ = self.wait_for_cancellation() => return Err(Self::cancellation_error()),
                    }
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

        // Cancellation interrupts network waits only. File operations and
        // their settlement remain awaited before the stage can be reclaimed.
        let transfer = async {
            let mut downloaded: u64 = 0;
            let mut stream = response.bytes_stream();
            let start_time = std::time::Instant::now();

            use futures::StreamExt;
            loop {
                let chunk = tokio::select! {
                    chunk = stream.next() => chunk,
                    _ = self.wait_for_cancellation() => {
                        return Err(Self::cancellation_error());
                    },
                };
                let Some(chunk) = chunk else {
                    break;
                };
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

                self.send_progress(
                    progress_tx,
                    ProgressUpdate::Download {
                        downloaded_bytes: downloaded,
                        total_bytes: total_size,
                        speed_bytes_per_sec: speed,
                    },
                )
                .await;
            }

            self.check_cancelled()?;
            Ok(downloaded)
        }
        .await;

        let downloaded = settle_archive_file(file, archive_path, transfer).await?;

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
        Self::extract_zip_from(file, extract_dir)
    }

    fn extract_zip_from(file: File, extract_dir: &Path) -> Result<()> {
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
        Self::extract_tarball_from(file, extract_dir)
    }

    fn extract_tarball_from(file: File, extract_dir: &Path) -> Result<()> {
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
        publication_lease: Arc<torch::TorchPendingStage>,
        python_version: Option<String>,
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
        self.send_progress(
            progress_tx,
            ProgressUpdate::StageChanged {
                stage: InstallationStage::Setup,
                message: "Finalizing installation...".to_string(),
            },
        )
        .await;

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
            let _publication_lease = publication_lease;
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
        self.send_progress(
            progress_tx,
            ProgressUpdate::Setup {
                message: "Installation complete".to_string(),
            },
        )
        .await;

        info!("Installation of {} finalized", tag);
        Ok(())
    }

    fn check_cancelled(&self) -> Result<()> {
        if self.cancel_flag.load(Ordering::SeqCst) || self.shutdown_flag.load(Ordering::SeqCst) {
            Err(Self::cancellation_error())
        } else {
            Ok(())
        }
    }

    fn cancellation_error() -> PumasError {
        PumasError::InstallationFailed {
            message: "Installation cancelled".into(),
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

    #[tokio::test]
    async fn unconfigured_direct_llama_cpp_installer_rejects_before_native_mutation() {
        let root = tempfile::tempdir().unwrap();
        let installer = crate::version_manager::VersionInstaller::new(
            root.path().to_path_buf(),
            AppId::LlamaCpp,
            Arc::new(MetadataManager::new(root.path())),
            Arc::new(RwLock::new(InstallationProgressTracker::new(
                root.path().to_path_buf(),
            ))),
            Arc::new(AtomicBool::new(false)),
        );
        // Refusal must precede even release validation/resolution, so no remote
        // fixture or assets are needed to establish the construction contract.
        let release = GitHubRelease {
            tag_name: "b1234+cpu".into(),
            name: "legacy direct fixture".into(),
            published_at: "2026-09-30T00:00:00Z".into(),
            body: None,
            tarball_url: None,
            zipball_url: None,
            prerelease: false,
            assets: Vec::new(),
            html_url: "https://github.com/ggml-org/llama.cpp/releases/tag/b1234".into(),
            total_size: None,
            archive_size: None,
            dependencies_size: None,
        };
        let (progress, _updates) = mpsc::channel(1);
        let error = installer
            .install_version("b1234+cpu", &release, progress)
            .await
            .unwrap_err();
        assert!(
            matches!(error, PumasError::Config { message } if message.contains("with_acquisition"))
        );
        assert!(!installer.versions_dir().exists());
        assert!(!installer.logs_dir().exists());
        assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
    }

    #[tokio::test]
    async fn public_direct_installer_configures_shared_llama_cpp_acquisition() {
        use pumas_library::acquisition::{
            AcquisitionCapacity, AcquisitionService, AcquisitionStore,
        };
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(AcquisitionStore::new(root.path()));
        let acquisition = Arc::new(
            AcquisitionService::with_capacity(
                store.clone(),
                AcquisitionCapacity {
                    scopes: 1,
                    ..AcquisitionCapacity::default()
                },
            )
            .unwrap(),
        );
        let installer = crate::version_manager::VersionInstaller::new(
            root.path().to_path_buf(),
            AppId::LlamaCpp,
            Arc::new(MetadataManager::new(root.path())),
            Arc::new(RwLock::new(
                crate::version_manager::InstallationProgressTracker::new(root.path().to_path_buf()),
            )),
            Arc::new(AtomicBool::new(false)),
        )
        .with_acquisition(acquisition.clone())
        .await
        .unwrap();
        assert!(installer.github_client.is_some());
        assert_eq!(
            installer.acquisition_consumer.as_ref().unwrap().owner(),
            "runtime.llama.cpp"
        );
        assert!(Arc::ptr_eq(acquisition.store(), &store));
        // The direct installer consumes this owner's only scope reservation.
        assert!(matches!(
            acquisition.open_consumer("second"),
            Err(PumasError::AcquisitionCapacityExhausted { resource: "scopes" })
        ));
        drop(installer);
        acquisition.shutdown().await.unwrap();
    }

    #[test]
    fn native_attempt_reopen_preserves_identity_and_remove_reinstall_renews_it() {
        let root = tempfile::tempdir().unwrap();
        let first = NativeInstallWorkspace::create(root.path(), "b1234+cpu").unwrap();
        let first_identity = first.attempt.clone();
        let first_path = first.path().to_path_buf();
        drop(first);
        let reopened = NativeInstallWorkspace::create(root.path(), "b1234+cpu").unwrap();
        assert_eq!(reopened.attempt, first_identity);
        assert_eq!(reopened.path(), first_path);
        drop(reopened);
        let lock = NativeVersionsLock::try_acquire(root.path()).unwrap();
        mark_native_attempt_removed(root.path(), "b1234+cpu").unwrap();
        assert!(!first_path.exists());
        drop(lock);
        assert!(NativeInstallWorkspace::create_for_attempt(
            root.path(),
            "b1234+cpu",
            Some(&first_identity)
        )
        .is_err());
        let reinstalled = NativeInstallWorkspace::create(root.path(), "b1234+cpu").unwrap();
        assert_ne!(reinstalled.attempt, first_identity);
        assert_ne!(reinstalled.path(), first_path);
        reinstalled.cleanup().unwrap();
    }

    #[test]
    fn native_receipt_reconciles_staged_publication_and_rejects_changed_output() {
        let root = tempfile::tempdir().unwrap();
        let versions = root.path().join("llama-cpp-versions");
        let custody = Arc::new(NativeInstallWorkspace::create(&versions, "b1234+cpu").unwrap());
        let stage = custody.path().join("output");
        std::fs::create_dir_all(stage.join("bin")).unwrap();
        let launcher = stage.join("bin").join(if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        });
        std::fs::write(&launcher, b"verified native output").unwrap();
        let metadata = InstalledVersionMetadata {
            path: "b1234+cpu".into(),
            installed_date: "2026-09-30T00:00:00Z".into(),
            release_tag: "b1234+cpu".into(),
            python_version: None,
            git_commit: None,
            release_date: None,
            release_notes: None,
            download_url: None,
            size: Some(22),
            requirements_hash: None,
            dependencies_installed: Some(true),
        };
        let proof = LlamaCppInstallReceiptV1 {
            schema_version: 1,
            tag: "b1234+cpu".into(),
            metadata_sha256: hash_metadata(&metadata).unwrap(),
            metadata,
            output_tree_sha256: hash_native_tree(&stage).unwrap(),
            launcher_relative_path: if cfg!(windows) {
                "bin/llama-server.exe".into()
            } else {
                "bin/llama-server".into()
            },
            launcher_sha256: hash_regular_file(&launcher).unwrap(),
        };
        // Reopen the stable workspace after a prepare-before-publish interruption.
        drop(custody);
        let custody = Arc::new(NativeInstallWorkspace::create(&versions, "b1234+cpu").unwrap());
        let manager = MetadataManager::new(root.path());
        manager.ensure_directories().unwrap();
        VersionInstaller::reconcile_llama_cpp_installation(
            &versions,
            "b1234+cpu",
            AppId::LlamaCpp,
            &manager,
            custody.clone(),
            proof.clone(),
        )
        .unwrap();
        assert!(!stage.exists());
        let destination = versions.join("b1234+cpu");
        assert!(metadata_matches(
            &manager
                .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
                .unwrap()
                .unwrap(),
            &proof.metadata
        ));
        // Already-published recovery checks exact bytes and metadata before success.
        VersionInstaller::reconcile_llama_cpp_installation(
            &versions,
            "b1234+cpu",
            AppId::LlamaCpp,
            &manager,
            custody.clone(),
            proof.clone(),
        )
        .unwrap();
        std::fs::write(
            destination.join(&proof.launcher_relative_path),
            b"changed output",
        )
        .unwrap();
        let published_launcher = destination.join(&proof.launcher_relative_path);
        let error = VersionInstaller::reconcile_llama_cpp_installation(
            &versions,
            "b1234+cpu",
            AppId::LlamaCpp,
            &manager,
            custody,
            proof,
        )
        .unwrap_err();
        assert!(error.to_string().contains("recovery required"));
        assert_eq!(
            std::fs::read(published_launcher).unwrap(),
            b"changed output"
        );
    }

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
        assert!(wrapper.contains("llama-b9090"));
        assert!(!wrapper.contains(version_dir.to_string_lossy().as_ref()));
        let mode = std::fs::metadata(&launch_binary)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        let unrelated_cwd = tempfile::tempdir().unwrap();
        let published = absolute_root.join("published");
        std::fs::rename(&version_dir, &published).unwrap();
        let output = std::process::Command::new(published.join("bin/llama-server"))
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
    fn native_publication_failure_preserves_stage_and_existing_output() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join("stage");
        let destination = root.path().join("version");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("complete"), "new").unwrap();
        let error = VersionInstaller::publish_native_stage(&stage, &destination, || {
            Err((PumasError::Other("metadata failure".into()), true))
        })
        .unwrap_err();
        assert!(error.to_string().contains("metadata failure"));
        assert!(!destination.exists());
        assert_eq!(std::fs::read(stage.join("complete")).unwrap(), b"new");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("retained"), "old").unwrap();
        assert!(
            VersionInstaller::publish_native_stage(&stage, &destination, || panic!(
                "must not finalize existing output"
            ))
            .is_err()
        );
        assert_eq!(std::fs::read(destination.join("retained")).unwrap(), b"old");
    }

    #[test]
    fn native_uncertain_metadata_publication_retains_complete_output() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join("stage");
        let destination = root.path().join("version");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("complete"), "owned").unwrap();
        assert!(
            VersionInstaller::publish_native_stage(&stage, &destination, || {
                Err((PumasError::Other("uncertain publication".into()), false))
            })
            .is_err()
        );
        assert!(!stage.exists());
        assert_eq!(
            std::fs::read(destination.join("complete")).unwrap(),
            b"owned"
        );
    }

    #[cfg(unix)]
    #[test]
    fn native_publication_preserves_a_dangling_destination_link() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join("stage");
        let destination = root.path().join("version");
        std::fs::create_dir(&stage).unwrap();
        std::os::unix::fs::symlink("missing", &destination).unwrap();
        assert!(
            VersionInstaller::publish_native_stage(&stage, &destination, || {
                panic!("must not finalize over retained link")
            })
            .is_err()
        );
        assert_eq!(
            std::fs::read_link(&destination).unwrap(),
            Path::new("missing")
        );
        assert!(stage.exists());
    }

    #[tokio::test]
    async fn native_progress_backpressure_ends_on_cancel_and_shutdown() {
        let root = tempfile::tempdir().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let shutting_down = Arc::new(AtomicBool::new(false));
        let installer = VersionInstaller::new(
            root.path().to_path_buf(),
            AppId::LlamaCpp,
            Arc::new(MetadataManager::new(root.path())),
            Arc::new(RwLock::new(InstallationProgressTracker::new(
                root.path().to_path_buf(),
            ))),
            cancelled.clone(),
        )
        .with_shutdown_flag(shutting_down.clone());
        let (sender, _receiver) = mpsc::channel(1);
        sender
            .send(ProgressUpdate::Setup {
                message: "full".into(),
            })
            .await
            .unwrap();
        for flag in [&cancelled, &shutting_down] {
            let blocked = installer.send_progress(
                &sender,
                ProgressUpdate::Setup {
                    message: "pending".into(),
                },
            );
            tokio::pin!(blocked);
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(20), &mut blocked)
                    .await
                    .is_err()
            );
            flag.store(true, Ordering::SeqCst);
            tokio::time::timeout(std::time::Duration::from_secs(1), blocked)
                .await
                .unwrap();
            flag.store(false, Ordering::SeqCst);
        }
    }

    #[test]
    fn native_invalid_archive_cannot_touch_retained_output() {
        let root = tempfile::tempdir().unwrap();
        let retained = root.path().join("version");
        std::fs::create_dir(&retained).unwrap();
        std::fs::write(retained.join("retained"), "old").unwrap();
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        let archive = root.path().join("archive");
        std::fs::write(&archive, "invalid tar").unwrap();
        assert!(VersionInstaller::extract_llama_cpp_binary(
            &archive,
            stage.path(),
            "native.tar.gz"
        )
        .is_err());
        assert_eq!(std::fs::read(retained.join("retained")).unwrap(), b"old");
    }

    #[cfg(unix)]
    #[test]
    fn native_output_rejects_external_and_directory_links() {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::write(external.path().join("server"), "retained").unwrap();
        std::os::unix::fs::symlink(
            external.path().join("server"),
            root.path().join("llama-server"),
        )
        .unwrap();
        assert!(VersionInstaller::validate_native_output(root.path()).is_err());
        std::fs::remove_file(root.path().join("llama-server")).unwrap();
        std::os::unix::fs::symlink(".", root.path().join("loop")).unwrap();
        assert!(VersionInstaller::validate_native_output(root.path()).is_err());
    }

    #[tokio::test]
    async fn native_worker_retains_custody_after_waiter_is_cancelled() {
        let root = tempfile::tempdir().unwrap();
        let custody = Arc::new(NativeInstallWorkspace::create(root.path(), "fixture").unwrap());
        let path = custody.path().to_path_buf();
        std::fs::write(path.join("archive"), "owned").unwrap();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker_custody = custody.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let _held = worker_custody;
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            assert_eq!(
                std::fs::read(_held.path().join("archive")).unwrap(),
                b"owned"
            );
        });
        entered_rx.await.unwrap();
        drop(custody);
        worker.abort(); // spawn_blocking is already running and cannot be aborted.
        assert!(path.exists());
        assert!(NativeInstallWorkspace::create(root.path(), "fixture").is_err());
        release_tx.send(()).unwrap();
        worker.await.unwrap();
        // Cancellation loses the receipt, so implicit destruction preserves
        // bytes for reconciliation even after the blocking worker has settled.
        assert!(path.exists());
        let next = NativeInstallWorkspace::create(root.path(), "fixture").unwrap();
        next.cleanup().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn native_cleanup_failure_reports_and_retains_its_path() {
        let root = tempfile::tempdir().unwrap();
        let custody = NativeInstallWorkspace::create(root.path(), "fixture").unwrap();
        let path = custody.path().to_owned();
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, "retained uncertainty").unwrap();
        let error = custody.cleanup().unwrap_err().to_string();
        assert!(error.contains("cleanup incomplete"));
        assert!(error.contains(&path.display().to_string()));
        assert_eq!(std::fs::read(&path).unwrap(), b"retained uncertainty");
        assert!(NativeInstallWorkspace::create(root.path(), "fixture").is_err());
    }

    #[test]
    fn native_cancelled_archive_settles_queued_writes_before_reclamation() {
        let root = tempfile::tempdir().unwrap();
        let custody = NativeInstallWorkspace::create(root.path(), "fixture").unwrap();
        let path = custody.path().join("archive");
        let file = std::fs::File::create(&path).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            });
            entered_rx.await.unwrap();
            let mut file = fs::File::from_std(file);
            // Tokio accepts this buffer while its blocking write is queued
            // behind the occupied worker. Cancellation is then observed.
            file.write_all(b"pending native bytes").await.unwrap();
            let write_path = path.clone();
            let mut settlement = tokio::spawn(async move {
                settle_archive_file::<()>(
                    file,
                    &write_path,
                    Err(VersionInstaller::cancellation_error()),
                )
                .await
            });
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(30), &mut settlement)
                    .await
                    .is_err()
            );
            assert!(path.exists());
            assert!(NativeVersionsLock::try_acquire(root.path()).is_err());
            release_tx.send(()).unwrap();
            blocker.await.unwrap();
            assert!(settlement
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("cancelled"));
            assert_eq!(std::fs::read(&path).unwrap(), b"pending native bytes");
            custody.cleanup().unwrap();
            assert!(!path.exists());
        });
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
