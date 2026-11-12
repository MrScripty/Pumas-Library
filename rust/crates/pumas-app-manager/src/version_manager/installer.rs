//! Version installation with progress reporting.
//!
//! Handles downloading, extracting, and setting up new versions.

#[cfg(test)]
mod native_archive_tests;
mod onnx_cuda;
mod onnx_cuda_availability;
#[cfg(test)]
mod onnx_cuda_tests;
mod python_index;
mod torch;
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod torch_tests;
pub(crate) use onnx_cuda::CudaWheelSources;
pub(crate) use onnx_cuda_availability::CudaAvailability;
pub(crate) use torch::is_torch_runtime_release;
pub(crate) use torch::retry_pending_torch_cleanup;
#[cfg(test)]
pub(crate) use torch::TorchPublicationPause;
pub(crate) use torch::TorchVersionsLock;

use crate::version_manager::progress::{InstallationProgressTracker, ProgressUpdate};
use chrono::Utc;
use futures::future::{BoxFuture, Shared};
use futures::FutureExt;
use pumas_library::acquisition::{
    AcquisitionConsumer, AcquisitionDemand, AcquisitionHttpRequest, AcquisitionHttpSource,
    AcquisitionRetryPolicy, AcquisitionWorkspace, ReservedDirectory, ReservedDirectoryBinding,
};
use pumas_library::config::{AppId, InstallationConfig, PathsConfig};
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_library::models::{InstallationProgress, InstallationStage};
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeAttemptIdentity {
    schema_version: u32,
    tag: String,
    attempt: String,
    removed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    binding: Option<ReservedDirectoryBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cleanup_pending: Option<String>,
}

fn native_attempt_path(versions: &Path, tag: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(tag.as_bytes()));
    versions.join(format!(".llama-attempt-{}.json", &digest[..24]))
}

fn native_workspace_path(versions: &Path, tag: &str, attempt: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(tag.as_bytes()));
    versions.join(format!(".llama-install-{}-{attempt}", &digest[..24]))
}

const NATIVE_CLEANUP_STATUS_PREFIX: &str = "Installed output verified; ";

pub(crate) fn has_native_cleanup_pending(progress: &InstallationProgress) -> bool {
    progress.success == Some(true)
        && progress
            .current_item
            .as_deref()
            .is_some_and(|item| item.starts_with(NATIVE_CLEANUP_STATUS_PREFIX))
}

/// Cleanup is independent of installed-output validity. Retained staging must
/// never turn an already verified publication into an installation failure.
#[derive(Debug)]
pub(crate) enum NativeCleanupReport {
    Removed,
    Absent,
    Retained {
        reason: String,
    },
    /// Staging was removed or already absent; only its durable status update failed.
    DiagnosticPending {
        reason: String,
    },
}

impl NativeCleanupReport {
    fn retained(error: impl std::fmt::Display) -> Self {
        Self::Retained {
            reason: error.to_string(),
        }
    }

    pub(crate) fn into_result(self) -> Result<()> {
        match self {
            Self::Removed | Self::Absent => Ok(()),
            Self::Retained { reason } | Self::DiagnosticPending { reason } => {
                Err(PumasError::InstallationFailed { message: reason })
            }
        }
    }

    fn pending_message(&self) -> Option<String> {
        match self {
            Self::Retained { reason } => Some(format!("staging cleanup pending: {reason}")),
            Self::DiagnosticPending { reason } => Some(format!(
                "staging is absent; cleanup-status update pending: {reason}"
            )),
            _ => None,
        }
    }
}

fn cleanup_native_workspace_if_present(
    versions: &Path,
    identity: &NativeAttemptIdentity,
    lock: NativeVersionsLock,
) -> NativeCleanupReport {
    let directory = native_workspace_path(versions, &identity.tag, &identity.attempt);
    if let Some(binding) = &identity.binding {
        match binding.matches_root(versions) {
            Ok(true) => {}
            Ok(false) => {
                return NativeCleanupReport::retained(
                    "Native staging root binding changed; reconciliation required",
                )
            }
            Err(error) => return NativeCleanupReport::retained(error),
        }
    }
    let report = match path_exists_sync(&directory) {
        Ok(false) => NativeCleanupReport::Absent,
        Err(error) => NativeCleanupReport::retained(error),
        Ok(true) => match NativeInstallWorkspace::reopen(versions, identity, lock.clone()) {
            Ok(workspace) => return workspace.cleanup(),
            Err(error) => NativeCleanupReport::retained(format!(
                "Native staging cleanup pending at {}: {error}",
                directory.display()
            )),
        },
    };
    // Legacy records remain byte-compatible. A present v1 leaf itself remains
    // durable reconciliation evidence; do not invent a physical binding for it.
    if let Some(binding) = &identity.binding {
        let persist = (|| {
            if !binding.matches_root(versions)? {
                return Err(PumasError::Other(
                    "Native cleanup diagnostic root changed".into(),
                ));
            }
            let mut updated = identity.clone();
            updated.cleanup_pending = report.pending_message();
            write_native_attempt(versions, &updated)
        })();
        if let Err(error) = persist {
            let reason = format!(
                "{}; cleanup diagnostic persistence failed: {error}",
                report
                    .pending_message()
                    .as_deref()
                    .unwrap_or("Native staging absent")
            );
            return match report {
                NativeCleanupReport::Absent => NativeCleanupReport::DiagnosticPending { reason },
                _ => NativeCleanupReport::retained(reason),
            };
        }
    }
    report
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
        if !matches!(identity.schema_version, 1 | 2)
            || (identity.schema_version == 1
                && (identity.binding.is_some() || identity.cleanup_pending.is_some()))
            || (identity.schema_version == 2 && identity.binding.is_none())
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
pub(crate) fn mark_native_attempt_removed(
    versions: &Path,
    tag: &str,
    lock: NativeVersionsLock,
) -> Result<NativeCleanupReport> {
    VersionInstaller::validate_native_tag(tag)?;
    if let Some(mut identity) = read_native_attempt(versions, tag)? {
        identity.removed = true;
        write_native_attempt(versions, &identity)?;
        return Ok(cleanup_native_workspace_if_present(
            versions, &identity, lock,
        ));
    }
    Ok(NativeCleanupReport::Absent)
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

/// The runtimes installed from publisher-verified native release archives.
///
/// Both share acquisition, custody, receipts and recovery. They differ in the
/// archive they select, how its contents are arranged, and the one regular file
/// a receipt names as the runtime's entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeRuntime {
    LlamaCpp,
    OnnxRuntime,
}

impl NativeRuntime {
    pub(crate) fn for_app(app_id: AppId) -> Result<Self> {
        match app_id {
            AppId::LlamaCpp => Ok(Self::LlamaCpp),
            AppId::OnnxRuntime => Ok(Self::OnnxRuntime),
            other => Err(PumasError::Config {
                message: format!("{other} is not installed from a native release archive"),
            }),
        }
    }

    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::LlamaCpp => "llama.cpp",
            Self::OnnxRuntime => "ONNX Runtime",
        }
    }

    /// The durable acquisition consumer identity. Persisted records name it,
    /// so an existing value must never change.
    pub(crate) fn consumer_owner(self) -> &'static str {
        match self {
            Self::LlamaCpp => "runtime.llama.cpp",
            Self::OnnxRuntime => "runtime.onnx-runtime",
        }
    }

    fn log_slug(self) -> &'static str {
        match self {
            Self::LlamaCpp => "llama-cpp",
            Self::OnnxRuntime => "onnx-runtime",
        }
    }

    /// The regular file a receipt names and hashes as the runtime entry point:
    /// the llama.cpp server launcher, or the ONNX Runtime library the in-process
    /// provider loads.
    pub(crate) fn entry_relative_path(self) -> &'static str {
        match self {
            Self::LlamaCpp if cfg!(windows) => "bin/llama-server.exe",
            Self::LlamaCpp => "bin/llama-server",
            Self::OnnxRuntime => pumas_library::onnx_runtime_distribution::library_relative_path(),
        }
    }
}

fn verify_native_runtime_output(
    runtime: NativeRuntime,
    root: &Path,
    proof: &NativeInstallReceiptV1,
) -> Result<()> {
    let expected_launcher = runtime.entry_relative_path();
    let root_metadata =
        std::fs::symlink_metadata(root).map_err(|error| PumasError::io_with_path(error, root))?;
    let launcher = root.join(expected_launcher);
    let launcher_metadata = std::fs::symlink_metadata(&launcher)
        .map_err(|error| PumasError::io_with_path(error, &launcher))?;
    if proof.launcher_relative_path != expected_launcher
        || !root_metadata.is_dir()
        || root_metadata.file_type().is_symlink()
        || !launcher_metadata.is_file()
        || launcher_metadata.file_type().is_symlink()
        || launcher_metadata.len() == 0
    {
        return Err(PumasError::InstallationFailed {
            message: "Native output has invalid owned paths or launcher; recovery required".into(),
        });
    }
    VersionInstaller::validate_native_output(root)
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
    tag: String,
    grant: ReservedDirectory,
    contents_cleared: AtomicBool,
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
        match (current, expected) {
            (Some(identity), Some(expected))
                if !identity.removed && identity.attempt == expected =>
            {
                Self::reopen(versions, &identity, lock)
            }
            (_, Some(_)) => Err(PumasError::Validation {
                field: "acquisition.consumer_recovery_required".into(),
                message: "Native recovery attempt was removed or changed".into(),
            }),
            (Some(identity), None) if !identity.removed => Self::reopen(versions, &identity, lock),
            (previous, None) => {
                if path_exists_sync(&versions.join(tag))? {
                    return Err(PumasError::VersionAlreadyInstalled { tag: tag.into() });
                }
                if let Some(previous) = previous {
                    // Never replace a retained record to manufacture ownership.
                    cleanup_native_workspace_if_present(versions, &previous, lock.clone())
                        .into_result()?;
                }
                let mut entropy = [0u8; 16];
                getrandom::fill(&mut entropy).map_err(|error| {
                    PumasError::Other(format!("Native attempt identity failed: {error}"))
                })?;
                let attempt: String = entropy.iter().map(|byte| format!("{byte:02x}")).collect();
                let directory = native_workspace_path(versions, tag, &attempt);
                // Creation precedes persistence. A crash here leaves an unclaimed
                // orphan, never an identity which can adopt a preexisting leaf.
                std::fs::create_dir(&directory)
                    .map_err(|error| PumasError::io_with_path(error, &directory))?;
                let grant = Self::capture(versions, &directory, &lock)?;
                sync_native_directory(&directory)?;
                sync_native_directory(versions)?;
                sync_native_directory(
                    versions
                        .parent()
                        .ok_or_else(|| PumasError::Other("Native versions parent absent".into()))?,
                )?;
                let identity = NativeAttemptIdentity {
                    schema_version: 2,
                    tag: tag.into(),
                    attempt: attempt.clone(),
                    removed: false,
                    binding: Some(grant.binding().clone()),
                    // Durable even if later custody loss prevents updating it.
                    cleanup_pending: Some(
                        "Native staging is retained until verified cleanup completes".into(),
                    ),
                };
                write_native_attempt(versions, &identity)?;
                Ok(Self {
                    directory,
                    attempt,
                    tag: tag.into(),
                    grant,
                    contents_cleared: AtomicBool::new(false),
                    _lock: lock,
                })
            }
        }
    }

    fn capture(
        versions: &Path,
        directory: &Path,
        lock: &NativeVersionsLock,
    ) -> Result<ReservedDirectory> {
        let relative =
            directory
                .strip_prefix(versions)
                .map_err(|_| PumasError::InstallationFailed {
                    message: "Native staging escaped the versions root".into(),
                })?;
        ReservedDirectory::capture(versions, relative, Arc::new(lock.clone()), || Ok(()))
    }

    fn reopen(
        versions: &Path,
        identity: &NativeAttemptIdentity,
        lock: NativeVersionsLock,
    ) -> Result<Self> {
        let binding = identity
            .binding
            .as_ref()
            .filter(|_| identity.schema_version == 2)
            .ok_or_else(|| PumasError::Validation {
                field: "acquisition.consumer_recovery_required".into(),
                message:
                    "Legacy native staging has no physical custody proof; reconciliation required"
                        .into(),
            })?;
        let directory = native_workspace_path(versions, &identity.tag, &identity.attempt);
        // Missing active staging is never recreated on restart.
        let grant = Self::capture(versions, &directory, &lock)?;
        if grant.binding() != binding {
            return Err(PumasError::Validation {
                field: "acquisition.consumer_recovery_required".into(),
                message: "Native staging physical binding changed; reconciliation required".into(),
            });
        }
        Ok(Self {
            directory,
            attempt: identity.attempt.clone(),
            tag: identity.tag.clone(),
            grant,
            contents_cleared: AtomicBool::new(false),
            _lock: lock,
        })
    }

    fn path(&self) -> &Path {
        &self.directory
    }

    /// Called with settled acquisition effects, while use custody still exists.
    fn revoke_and_clear(&self) -> Result<()> {
        self.grant.validate_root()?;
        let versions = self
            .directory
            .parent()
            .ok_or_else(|| PumasError::Other("Native workspace parent absent".into()))?;
        let mut identity = read_native_attempt(versions, &self.tag)?
            .ok_or_else(|| PumasError::Other("Cancelled native attempt identity absent".into()))?;
        if identity.attempt != self.attempt
            || identity.binding.as_ref() != Some(self.grant.binding())
        {
            return Err(PumasError::Other(
                "Cancelled native attempt identity changed".into(),
            ));
        }
        identity.removed = true;
        write_native_attempt(versions, &identity)?;
        self.grant.clear_contents()?;
        self.contents_cleared.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn cleanup(self) -> NativeCleanupReport {
        let Self {
            directory,
            attempt,
            tag,
            grant,
            _lock,
            ..
        } = self;
        let versions = directory.parent().expect("native workspace parent");
        let root_identity = grant.workspace_identity().clone();
        let outcome = grant.clear_contents().and_then(|()| grant.remove_empty());
        let mut reason = outcome.err().map(|error| {
            format!(
                "Native staging cleanup incomplete at {}; cleanup pending: {error}",
                directory.display()
            )
        });
        let persist = (|| {
            // Only the captured root may receive diagnostics. The permanent lock
            // serializes cooperating writers through this final compare/write.
            let relative = directory
                .strip_prefix(versions)
                .map_err(|_| PumasError::Other("Native workspace parent changed".into()))?;
            if AcquisitionWorkspace::identity_for_reserved_directory(versions, relative)?
                != root_identity
            {
                return Err(PumasError::Other(
                    "Native cleanup diagnostic root changed".into(),
                ));
            }
            let mut identity = read_native_attempt(versions, &tag)?
                .ok_or_else(|| PumasError::Other("Native cleanup attempt absent".into()))?;
            if identity.attempt != attempt {
                return Err(PumasError::Other("Native cleanup attempt changed".into()));
            }
            identity.cleanup_pending = reason.clone();
            write_native_attempt(versions, &identity)
        })();
        if let Err(error) = persist {
            if reason.is_none() {
                return NativeCleanupReport::DiagnosticPending {
                    reason: error.to_string(),
                };
            }
            reason = Some(format!(
                "{}; cleanup diagnostic persistence failed: {error}",
                reason.as_deref().expect("failed physical cleanup")
            ));
        }
        // Keep the lock through removal, parent sync, and diagnostic persistence.
        drop(_lock);
        match reason {
            Some(reason) => NativeCleanupReport::Retained { reason },
            None => NativeCleanupReport::Removed,
        }
    }
}

fn cleanup_settled_native_workspace(custody: Arc<NativeInstallWorkspace>) -> NativeCleanupReport {
    match Arc::try_unwrap(custody) {
        Ok(custody) => custody.cleanup(),
        Err(custody) => NativeCleanupReport::retained(format!(
            "Native staging cleanup pending: workspace remains in use at {}",
            custody.path().display()
        )),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeInstallReceiptV1 {
    schema_version: u32,
    tag: String,
    metadata: InstalledVersionMetadata,
    metadata_sha256: String,
    output_tree_sha256: String,
    launcher_relative_path: String,
    launcher_sha256: String,
}

struct PreparedNativeInstall {
    use_set: pumas_library::acquisition::AcquiredArtifactUse,
    stage: PathBuf,
    destination: PathBuf,
    custody: Arc<NativeInstallWorkspace>,
}

struct NativePublication {
    versions: PathBuf,
    app_id: AppId,
    manager: Arc<MetadataManager>,
    custody: Arc<NativeInstallWorkspace>,
    stage: PathBuf,
    destination: PathBuf,
    proof: NativeInstallReceiptV1,
    #[cfg(test)]
    interrupt_after_native_rename: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    park_after_native_rename_marker: Option<PathBuf>,
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

struct NativeHttpAttemptHost {
    cancelled_transfer_workspace: Option<Arc<NativeInstallWorkspace>>,
    cancel_flag: Arc<AtomicBool>,
    shutdown_flag: Arc<AtomicBool>,
    progress_tracker: Arc<RwLock<InstallationProgressTracker>>,
    progress_tx: mpsc::Sender<ProgressUpdate>,
    total_size: Option<u64>,
    /// Bytes of the files of a multi-file set that are already acquired.
    acquired_bytes: u64,
    started: Instant,
}

#[async_trait::async_trait]
impl pumas_library::acquisition::HttpAttemptHost for NativeHttpAttemptHost {
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
        let downloaded = self.acquired_bytes.saturating_add(downloaded_for_file);
        let elapsed = self.started.elapsed().as_secs_f64();
        let speed = (elapsed > 0.0).then_some(downloaded as f64 / elapsed);
        self.progress_tracker
            .write()
            .await
            .update_download_progress(downloaded, self.total_size, speed);
        send_install_progress(
            &self.progress_tx,
            ProgressUpdate::Download {
                downloaded_bytes: downloaded,
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
impl pumas_library::acquisition::AcquisitionHost for NativeHttpAttemptHost {
    fn cancelled_transfer_cleanup(&mut self) -> Option<Box<dyn FnOnce() -> Result<()> + Send>> {
        self.cancelled_transfer_workspace.take().map(|custody| {
            Box::new(move || custody.revoke_and_clear()) as Box<dyn FnOnce() -> Result<()> + Send>
        })
    }

    fn file_acquired(&mut self, _index: usize, bytes: u64) {
        self.acquired_bytes = self.acquired_bytes.saturating_add(bytes);
    }

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
    let workspace = custody.grant.acquisition_workspace()?;
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

/// Publisher evidence a downloaded or cached native archive must match before
/// it may be extracted.
#[derive(Clone, Debug)]
struct ExpectedArchive {
    sha256: String,
    size: Option<u64>,
}

impl ExpectedArchive {
    fn from_selection(
        selection: &pumas_library::acquisition::GitHubReleaseAssetSelection,
    ) -> Result<Self> {
        let file =
            selection
                .manifest()
                .files()
                .first()
                .ok_or_else(|| PumasError::InstallationFailed {
                    message: "Verified release selection has no file".into(),
                })?;
        let sha256 = file
            .expected_sha256()
            .ok_or_else(|| PumasError::InstallationFailed {
                message: "Verified release selection has no publisher SHA-256 digest".into(),
            })?
            .value()
            .to_owned();
        Ok(Self {
            sha256,
            size: file.expected_size(),
        })
    }

    fn verify(&self, sha256: &str, size: u64) -> Result<()> {
        if self.size.is_some_and(|expected| expected != size) {
            return Err(PumasError::InstallationFailed {
                message: format!(
                    "Archive is {size} bytes but the publisher declares {} bytes",
                    self.size.unwrap_or_default()
                ),
            });
        }
        if !sha256.eq_ignore_ascii_case(&self.sha256) {
            return Err(PumasError::InstallationFailed {
                message: format!(
                    "Archive SHA-256 {sha256} does not match the publisher digest {}",
                    self.sha256
                ),
            });
        }
        Ok(())
    }

    /// Hash an open archive and rewind it, so the caller extracts exactly the
    /// bytes that were verified.
    fn verify_file(&self, file: &mut File, path: &Path) -> Result<()> {
        use std::io::{Seek, SeekFrom};
        let metadata = file
            .metadata()
            .map_err(|error| PumasError::io_with_path(error, path))?;
        if !metadata.is_file() {
            return Err(PumasError::InstallationFailed {
                message: format!("Archive is not a regular file: {}", path.display()),
            });
        }
        let mut digest = Sha256::new();
        let size = std::io::copy(file, &mut digest)
            .map_err(|error| PumasError::io_with_path(error, path))?;
        self.verify(&format!("{:x}", digest.finalize()), size)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| PumasError::io_with_path(error, path))?;
        Ok(())
    }
}

/// One Ollama archive selected for installation and its publisher evidence.
struct OllamaArchive<'a> {
    tag: &'a str,
    release: &'a GitHubRelease,
    client: &'a reqwest::Client,
    download_url: &'a str,
    /// The release's stable asset URL, recorded in installation metadata.
    recorded_download_url: String,
    total_size: u64,
    asset_name: &'a str,
    archive_path: &'a Path,
    cache_valid: bool,
    expected: &'a ExpectedArchive,
}

/// Cumulative extraction bounds for one native runtime archive, derived from
/// the size of the (already verified) archive itself rather than a flat cap,
/// so a large GPU runtime and a small CPU build are each held to bounds that
/// scale with what the publisher actually shipped.
struct ExtractionBudget {
    archive_bytes: u64,
    max_bytes: u64,
    max_entries: u64,
    bytes: u64,
    entries: u64,
}

impl ExtractionBudget {
    /// Native runtime payloads (executables and shared libraries) compress
    /// roughly 2-6x under gzip, zstd or deflate. Allowing 64x leaves an order
    /// of magnitude of headroom for unusually compressible releases while still
    /// refusing a decompression bomb, whose ratio is typically in the
    /// thousands.
    const MAX_EXPANSION_RATIO: u64 = 64;
    /// Every entry costs at least its header and name in the archive: a zip
    /// entry needs a 30-byte local and a 46-byte central header, and a
    /// compressed tar header block with a distinct name does not shrink below
    /// a few tens of bytes. Sixteen bytes per entry is therefore a generous
    /// lower bound, and an archive claiming more entries than that is refused.
    const MIN_ARCHIVE_BYTES_PER_ENTRY: u64 = 16;

    fn for_archive(archive: &File, dest_dir: &Path) -> Result<Self> {
        let archive_bytes = archive
            .metadata()
            .map_err(|error| PumasError::io_with_path(error, dest_dir))?
            .len();
        Ok(Self::for_archive_bytes(archive_bytes))
    }

    fn for_archive_bytes(archive_bytes: u64) -> Self {
        Self {
            archive_bytes,
            max_bytes: archive_bytes.saturating_mul(Self::MAX_EXPANSION_RATIO),
            max_entries: archive_bytes / Self::MIN_ARCHIVE_BYTES_PER_ENTRY,
            bytes: 0,
            entries: 0,
        }
    }

    /// Admit one entry of `entry_bytes` declared bytes, or refuse the archive
    /// with the limit that was crossed and why.
    fn admit(&mut self, entry_bytes: u64) -> Result<()> {
        self.entries += 1;
        if self.entries > self.max_entries {
            return Err(PumasError::InstallationFailed {
                message: format!(
                    "Archive refused: more than {} entries in a {}-byte archive (at most one entry per {} archive bytes is accepted)",
                    self.max_entries,
                    self.archive_bytes,
                    Self::MIN_ARCHIVE_BYTES_PER_ENTRY
                ),
            });
        }
        match self.bytes.checked_add(entry_bytes) {
            Some(bytes) if bytes <= self.max_bytes => self.bytes = bytes,
            _ => {
                return Err(PumasError::InstallationFailed {
                message: format!(
                    "Archive refused: extracted content exceeds {} bytes, {}x the {}-byte archive, which indicates a decompression bomb",
                    self.max_bytes,
                    Self::MAX_EXPANSION_RATIO,
                    self.archive_bytes
                ),
            })
            }
        }
        Ok(())
    }
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
    /// Where the CUDA libraries of ONNX Runtime CUDA builds are resolved
    /// from, and what the version listing knows about which builds install.
    cuda: Arc<CudaAvailability>,
    torch_control: Arc<TorchInstallControl>,
    torch_cleanup: Arc<TorchCleanupTasks>,
    torch_attempt_lock: Mutex<()>,
    #[cfg(test)]
    native_receipt_pause: Option<Arc<TorchPublicationPause>>,
    #[cfg(test)]
    interrupt_after_native_rename: Option<Arc<AtomicBool>>,
    #[cfg(test)]
    park_after_native_rename_marker: Option<PathBuf>,
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
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
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
            cuda: Arc::new(CudaAvailability::new(CudaWheelSources::official())),
            torch_control: Arc::new(TorchInstallControl::new()),
            torch_cleanup: Arc::new(TorchCleanupTasks::default()),
            torch_attempt_lock: Mutex::new(()),
            #[cfg(test)]
            native_receipt_pause: None,
            #[cfg(test)]
            interrupt_after_native_rename: None,
            #[cfg(test)]
            park_after_native_rename_marker: None,
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

    /// Configure direct native-archive installation (llama.cpp or ONNX Runtime)
    /// with the existing shared acquisition capability. Initializes the GitHub
    /// metadata client from this installer's cache; creates no acquisition
    /// store, service, or downloader. Reads retained records from that same
    /// store and reconciles native uses before returning an installer that can
    /// admit new work. Recovery failure drains this consumer scope before
    /// returning the error; the caller retains the shared service's shutdown
    /// responsibility.
    /// External customer compatibility acceptance remains pending.
    pub async fn with_acquisition(
        mut self,
        acquisition: Arc<pumas_library::acquisition::AcquisitionService>,
    ) -> Result<Self> {
        let runtime = NativeRuntime::for_app(self.app_id).map_err(|_| PumasError::Config {
            message: "Shared artifact acquisition is supported for llama.cpp and ONNX Runtime"
                .into(),
        })?;
        let cache = self
            .launcher_root
            .join("launcher-data")
            .join(PathsConfig::CACHE_DIR_NAME);
        let store = acquisition.store().clone();
        let consumer = Arc::new(acquisition.open_consumer(runtime.consumer_owner())?);
        self.acquisition_consumer = Some(consumer.clone());
        let configured = async {
            let (client, records) = consumer
                .run_blocking("initialize native installer recovery", move || {
                    Ok((GitHubClient::new(cache)?, store.acquisitions()?))
                })
                .await?;
            self.github_client = Some(Arc::new(client));
            self.reconcile_retained_native_archives(records.into_values().collect())
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

    #[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
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

    #[cfg(test)]
    pub(crate) fn with_native_rename_park_marker(mut self, marker: PathBuf) -> Self {
        self.park_after_native_rename_marker = Some(marker);
        self
    }

    /// Share the version listing's CUDA library sources and index answers.
    pub(crate) fn with_cuda_availability(mut self, cuda: Arc<CudaAvailability>) -> Self {
        self.cuda = cuda;
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
    pub(crate) async fn reconcile_retained_native_archives(
        &self,
        records: Vec<pumas_library::acquisition::AcquisitionRecord>,
    ) -> Result<()> {
        let runtime = NativeRuntime::for_app(self.app_id)?;
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
                let proof: NativeInstallReceiptV1 = serde_json::from_value(receipt.payload)
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
                let verify_and_clean = move || {
                    #[cfg(test)]
                    if let Some(pause) = recovery_pause {
                        pause.block();
                    }
                    let _lock = NativeVersionsLock::try_acquire(&versions_for_cleanup)?;
                    let current = read_native_attempt(&versions_for_cleanup, &tag_for_cleanup)?;
                    let Some(current) = current else {
                        return Err(PumasError::Validation {
                            field: "acquisition.consumer_recovery_required".into(),
                            message: "Adopted native receipt has no attempt identity".into(),
                        });
                    };
                    if current.removed || current.attempt != attempt_for_cleanup {
                        return Ok(NativeCleanupReport::Absent);
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
                    let destination = versions_for_cleanup.join(&tag_for_cleanup);
                    let metadata = std::fs::symlink_metadata(&destination)
                        .map_err(|error| PumasError::io_with_path(error, &destination))?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(PumasError::InstallationFailed {
                            message: format!(
                                "Adopted {} output is not a safe directory",
                                runtime.display_name()
                            ),
                        });
                    }
                    Self::validate_native_receipt(runtime, &tag_for_cleanup, &proof, &destination)?;
                    verify_native_runtime_output(runtime, &destination, &proof)?;
                    match manager.get_installed_version(&tag_for_cleanup, Some(app_id))? {
                        Some(installed) if metadata_matches(&installed, &proof.metadata) => {}
                        _ => {
                            return Err(PumasError::InstallationFailed {
                                message: format!(
                                    "Adopted {} metadata does not match its receipt",
                                    runtime.display_name()
                                ),
                            })
                        }
                    }
                    let actual_workspace = AcquisitionWorkspace::identity_for_reserved_directory(
                        &versions_for_cleanup,
                        relative_workspace,
                    )?;
                    if actual_workspace != expected_workspace {
                        return Ok(NativeCleanupReport::retained(
                            "Native staging cleanup pending: workspace root changed",
                        ));
                    }
                    Ok(cleanup_native_workspace_if_present(
                        &versions_for_cleanup,
                        &current,
                        _lock,
                    ))
                };
                let cleanup = consumer
                    .run_blocking(
                        "verify adopted native output and clean workspace",
                        verify_and_clean,
                    )
                    .await?;
                self.report_native_cleanup(&tag, &cleanup, None).await;
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
            let recovery_status_tag = tag.clone();
            let custody_for_reconcile = custody.clone();
            let manager = self.metadata_manager.clone();
            let app_id = self.app_id;
            consumer
                .reconcile(
                    record.demand,
                    record.manifest,
                    workspace,
                    move |receipt, use_set| async move {
                        let proof: NativeInstallReceiptV1 = serde_json::from_value(receipt.payload)
                            .map_err(|error| PumasError::Validation {
                                field: "acquisition.consumer_recovery_required".into(),
                                message: format!("Retained native receipt is malformed: {error}"),
                            })?;
                        use_set
                            .run_blocking("reconcile retained native installation", move || {
                                Self::reconcile_native_installation(
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
            let cleanup = consumer
                .run_blocking("clean reconciled native workspace", move || {
                    #[cfg(test)]
                    if let Some(pause) = recovery_pause {
                        pause.block();
                    }
                    Ok(cleanup_settled_native_workspace(custody))
                })
                .await
                .unwrap_or_else(NativeCleanupReport::retained);
            self.report_native_cleanup(&recovery_status_tag, &cleanup, None)
                .await;
        }
        Ok(())
    }

    async fn report_native_cleanup(
        &self,
        tag: &str,
        report: &NativeCleanupReport,
        sender: Option<&mpsc::Sender<ProgressUpdate>>,
    ) {
        if let Some(reason) = report.pending_message() {
            let message = format!("{NATIVE_CLEANUP_STATUS_PREFIX}{reason}");
            let mut tracker = self.progress_tracker.write().await;
            let current = tracker.get_current_state();
            if current.as_ref().and_then(|state| state.tag.as_deref()) != Some(tag)
                || current.as_ref().is_some_and(|state| state.error.is_some())
            {
                tracker.start_installation(tag, None, None, None);
            }
            tracker.update_stage(InstallationStage::Setup, 100.0, Some(&message));
            tracker.complete_installation(true);
            drop(tracker);
            if let Some(sender) = sender {
                self.send_progress(sender, ProgressUpdate::Setup { message })
                    .await;
            }
        } else {
            let mut tracker = self.progress_tracker.write().await;
            if tracker.get_current_state().as_ref().is_some_and(|state| {
                state.tag.as_deref() == Some(tag) && has_native_cleanup_pending(state)
            }) {
                tracker.clear_completed_state_async().await;
            }
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
        torch_selection: Option<super::torch_preview::TorchInstallSelection>,
    ) -> Result<()> {
        match self.app_id {
            AppId::Ollama => self.install_ollama_binary(tag, release, progress_tx).await,
            AppId::LlamaCpp | AppId::OnnxRuntime => {
                self.install_native_archive(tag, release, progress_tx).await
            }
            AppId::Torch => {
                self.install_torch_runtime(tag, release, progress_tx, torch_selection)
                    .await
            }
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
        Self::validate_native_tag(tag)?;
        info!("Starting Ollama binary installation for {}", tag);

        // Select platform-appropriate asset (e.g., ollama-linux-amd64.tgz)
        let asset = self.select_ollama_asset(&release.assets)?;
        let asset_name = asset.name.clone();
        let recorded_download_url = asset.download_url.clone();

        // Like llama.cpp, the archive is authorized only by the publisher's
        // SHA-256 from a fresh GitHub API response; cached discovery has no
        // digest and cannot authorize bytes.
        let github = self
            .github_client
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: "Ollama requires the configured shared GitHub asset resolver".into(),
            })?;
        let selection = github
            .resolve_release_asset(self.app_id.github_repo(), &release.tag_name, &asset_name)
            .await
            .map_err(|error| PumasError::InstallationFailed {
                message: format!("Could not verify Ollama release asset: {error}"),
            })?;
        let expected = ExpectedArchive::from_selection(&selection)?;
        let download_url = selection.download_url().to_owned();
        let total_size = expected.size.unwrap_or(asset.size);
        let client = self.native_download_client(&download_url)?;

        info!(
            "Selected publisher-verified Ollama asset: {} ({} bytes)",
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

        // A cached archive is reused only when it still hashes to the
        // publisher digest; a matching size alone proves nothing.
        let cache_valid = Self::cached_archive_matches(&archive_path, &expected).await?;
        if cache_valid {
            info!(
                "Using cached download re-verified by SHA-256: {} ({} bytes)",
                asset_name, total_size
            );
        }

        // Download binary asset (skip if cache is valid)
        let result = self
            .do_ollama_install(
                OllamaArchive {
                    tag,
                    release,
                    client: &client,
                    download_url: &download_url,
                    recorded_download_url,
                    total_size,
                    asset_name: &asset_name,
                    archive_path: &archive_path,
                    cache_valid,
                    expected: &expected,
                },
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

    /// Install llama.cpp or ONNX Runtime from upstream pre-built release archives.
    async fn install_native_archive(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        let runtime = NativeRuntime::for_app(self.app_id)?;
        let _attempt =
            self.torch_attempt_lock
                .try_lock()
                .map_err(|_| PumasError::InstallationFailed {
                    message: format!("{} installation already active", runtime.display_name()),
                })?;
        self.torch_control.start();
        let result = self
            .install_native_archive_inner(runtime, tag, release, progress_tx)
            .await;
        self.torch_control.finish();
        result
    }

    fn select_native_asset<'a>(
        &self,
        runtime: NativeRuntime,
        tag: &str,
        assets: &'a [GitHubAsset],
    ) -> Result<&'a GitHubAsset> {
        match runtime {
            NativeRuntime::LlamaCpp => self.select_llama_cpp_asset(assets),
            NativeRuntime::OnnxRuntime => Self::select_onnx_runtime_asset(tag, assets),
        }
    }

    /// Select the archive of the build variant an ONNX Runtime tag names
    /// (`+cpu`, `+cuda12`, `+cuda13`) for this platform. A release that does
    /// not export the C API Pumas requests is refused with its reason.
    fn select_onnx_runtime_asset<'a>(
        tag: &str,
        assets: &'a [GitHubAsset],
    ) -> Result<&'a GitHubAsset> {
        use pumas_library::onnx_runtime_distribution as distribution;
        if let Some(reason) = distribution::incompatibility_reason(tag) {
            return Err(PumasError::InstallationFailed { message: reason });
        }
        let version =
            distribution::release_version(tag).ok_or_else(|| PumasError::InstallationFailed {
                message: format!("ONNX Runtime release {tag} does not have a semantic version"),
            })?;
        let variant = distribution::OnnxRuntimeVariant::from_tag(tag).ok_or_else(|| {
            PumasError::InstallationFailed {
                message: format!(
                    "ONNX Runtime tag {tag} does not name a build variant (+cpu, +cuda12, +cuda13)"
                ),
            }
        })?;
        let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
        distribution::select_variant_archive(assets, &version, variant, os, arch).ok_or_else(|| {
            PumasError::InstallationFailed {
                message: format!(
                    "ONNX Runtime {version} publishes no {} archive for {os}-{arch}; \
                     expected one of {:?}",
                    variant.label(),
                    distribution::variant_archive_names(&version, variant, os, arch)
                ),
            }
        })
    }

    /// The CUDA library wheels an ONNX Runtime CUDA tag installs, or `None`
    /// for every other install. An unfinished attempt of the same tag keeps
    /// the selection it started with, so its retained downloads resume;
    /// otherwise the current compatible releases are resolved.
    async fn onnx_cuda_selection(
        &self,
        runtime: NativeRuntime,
        tag: &str,
    ) -> Result<Option<onnx_cuda::CudaWheelSelection>> {
        use pumas_library::onnx_runtime_distribution as distribution;
        if runtime != NativeRuntime::OnnxRuntime {
            return Ok(None);
        }
        let Some(cuda_major) = distribution::OnnxRuntimeVariant::from_tag(tag)
            .and_then(|variant| variant.cuda_major())
        else {
            return Ok(None);
        };
        let platform =
            python_index::WheelPlatform::host().ok_or_else(|| PumasError::InstallationFailed {
                message: format!(
                    "ONNX Runtime CUDA builds and NVIDIA's CUDA libraries exist only for Linux \
                     x64 and Windows x64, not {}-{}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ),
            })?;
        let release =
            distribution::release_version(tag).ok_or_else(|| PumasError::InstallationFailed {
                message: format!("ONNX Runtime release {tag} does not have a semantic version"),
            })?;
        let versions = self.versions_dir();
        let retained_tag = tag.to_owned();
        let retained = tokio::task::spawn_blocking(move || {
            let identity = read_native_attempt(&versions, &retained_tag)
                .ok()
                .flatten()?;
            if identity.removed {
                return None;
            }
            let path = native_workspace_path(&versions, &retained_tag, &identity.attempt)
                .join(onnx_cuda::SELECTION_FILE);
            onnx_cuda::read_selection(&path).ok().flatten()
        })
        .await
        .map_err(|error| PumasError::Other(format!("CUDA selection lookup failed: {error}")))?;
        if let Some(selection) =
            retained.filter(|selection| selection.serves(tag, platform, cuda_major))
        {
            info!(
                "Resuming the CUDA {cuda_major} libraries selected for {tag}: {}",
                onnx_cuda::describe(&selection)
            );
            return Ok(Some(selection));
        }
        // An install reads the indexes as they are now, and what it finds
        // also answers the version listing. A refusal names the listed builds
        // that install instead. Resolution only reads package indexes, so a
        // cancel or shutdown abandons it at once instead of waiting for slow
        // requests.
        self.cuda.refresh();
        let resolution = async {
            match onnx_cuda::resolve(self.cuda.sources(), tag, &release, cuda_major, platform).await
            {
                Ok(selection) => {
                    self.cuda
                        .record(tag, onnx_cuda_availability::CudaBuildOutcome::Installs);
                    Ok(selection)
                }
                Err(error) => match onnx_cuda_availability::refusal_reason(&error) {
                    None => Err(error),
                    Some(reason) => {
                        self.cuda.record(
                            tag,
                            onnx_cuda_availability::CudaBuildOutcome::Unavailable(reason.clone()),
                        );
                        Err(PumasError::InstallationFailed {
                            message: self.cuda.explain(tag, &reason).await,
                        })
                    }
                },
            }
        };
        let selection = tokio::select! {
            selection = resolution => selection?,
            _ = super::wait_for_install_cancel(self.cancel_flag.clone()) => {
                return Err(Self::cancellation_error());
            }
            _ = super::wait_for_install_cancel(self.shutdown_flag.clone()) => {
                return Err(Self::cancellation_error());
            }
        };
        info!(
            "Selected the CUDA {cuda_major} libraries for {tag}: {}",
            onnx_cuda::describe(&selection)
        );
        Ok(Some(selection))
    }

    fn extract_native_archive_from(
        runtime: NativeRuntime,
        archive: File,
        stage: &Path,
        asset_name: &str,
    ) -> Result<()> {
        match runtime {
            NativeRuntime::LlamaCpp => {
                Self::extract_llama_cpp_binary_from(archive, stage, asset_name)
            }
            NativeRuntime::OnnxRuntime => {
                Self::extract_onnx_runtime_from(archive, stage, asset_name)
            }
        }
    }

    async fn install_native_archive_inner(
        &self,
        runtime: NativeRuntime,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        let runtime_name = runtime.display_name();
        let consumer = self
            .acquisition_consumer
            .clone()
            .ok_or_else(|| PumasError::Config {
                message: format!("{runtime_name} requires shared acquisition; configure with VersionInstaller::with_acquisition"),
            })?;
        Self::validate_native_tag(tag)?;
        info!("Starting {runtime_name} installation for {}", tag);

        let asset = self.select_native_asset(runtime, tag, &release.assets)?;
        let asset_name = asset.name.clone();
        let github = self
            .github_client
            .as_ref()
            .ok_or_else(|| PumasError::Config {
                message: format!(
                    "{runtime_name} requires the configured shared GitHub asset resolver"
                ),
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
                message: format!("Could not verify {runtime_name} release asset: {error}"),
            })?;
        let selected_file =
            selection
                .manifest()
                .files()
                .first()
                .ok_or_else(|| PumasError::InstallationFailed {
                    message: format!("Verified {runtime_name} release selection has no file"),
                })?;
        info!(
            "Selected publisher-verified {runtime_name} asset: {} ({} bytes)",
            asset_name,
            selected_file.expected_size().unwrap_or_default()
        );
        let download_url = selection.download_url().to_owned();

        // A CUDA build of ONNX Runtime brings its CUDA runtime and cuDNN
        // libraries: they join the archive in one verified download set.
        let cuda = self.onnx_cuda_selection(runtime, tag).await?;
        let (manifest, source_urls) = match &cuda {
            None => (selection.manifest().clone(), vec![download_url.clone()]),
            Some(cuda) => (
                onnx_cuda::composite_manifest(selection.manifest(), cuda)?,
                std::iter::once(download_url.clone())
                    .chain(cuda.wheels.iter().map(|wheel| wheel.url.clone()))
                    .collect(),
            ),
        };
        let total_size = manifest.total_bytes();
        self.check_cancelled()?;

        let log_dir = self.logs_dir();
        let log_path = log_dir.join(format!(
            "install-{}-{}-{}.log",
            runtime.log_slug(),
            self.slugify_tag(tag),
            Utc::now().format("%Y%m%d-%H%M%S")
        ));

        // Tracking starts before the first effect: a failure before this point
        // created nothing, and the version manager treats it as an outcome.
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.start_installation(
                tag,
                total_size,
                None,
                Some(log_path.to_string_lossy().as_ref()),
            );
        }

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
        if let Some(cuda) = &cuda {
            // Keep the selection with the attempt, so a retried or resumed
            // install downloads exactly these files.
            let path = custody.path().join(onnx_cuda::SELECTION_FILE);
            let selection = cuda.clone();
            consumer
                .run_blocking("keep the CUDA library selection", move || {
                    if onnx_cuda::read_selection(&path).ok().flatten().as_ref() != Some(&selection)
                    {
                        onnx_cuda::write_selection(&path, &selection)?;
                    }
                    Ok(())
                })
                .await?;
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
                    let proof: NativeInstallReceiptV1 = serde_json::from_value(receipt.payload)
                        .map_err(|error| PumasError::Validation {
                            field: "acquisition.consumer_receipt".into(),
                            message: format!("{runtime_name} receipt is malformed: {error}"),
                        })?;
                    if cancel_for_reconcile.load(Ordering::SeqCst)
                        || !control_for_reconcile.try_begin_publication()
                    {
                        return Err(VersionInstaller::cancellation_error());
                    }
                    use_set
                        .run_blocking("reconcile verified native installation", move || {
                            Self::reconcile_native_installation(
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

        let cancellation_settled = Arc::new(AtomicBool::new(false));
        let result = match reconcile {
            Ok(Some(())) => {
                drop(workspace);
                Ok(())
            }
            Ok(None) => {
                #[cfg(feature = "test-support")]
                let fixture_client = self
                    .github_client
                    .as_ref()
                    .map(|client| client.loopback_fixture_http_client(&download_url))
                    .transpose()?
                    .flatten();
                #[cfg(not(feature = "test-support"))]
                let fixture_client: Option<reqwest::Client> = None;
                let client = if let Some(client) = fixture_client {
                    client
                } else {
                    reqwest::Client::builder()
                        .connect_timeout(InstallationConfig::URL_FETCH_TIMEOUT)
                        .user_agent("pumas-library")
                        .build()
                        .map_err(|error| PumasError::Network {
                            message: "Failed to create shared artifact HTTP client".into(),
                            cause: Some(error.to_string()),
                        })?
                };
                let host = NativeHttpAttemptHost {
                    cancelled_transfer_workspace: Some(custody.clone()),
                    cancel_flag: self.cancel_flag.clone(),
                    shutdown_flag: self.shutdown_flag.clone(),
                    progress_tracker: self.progress_tracker.clone(),
                    progress_tx: progress_tx.clone(),
                    total_size,
                    acquired_bytes: 0,
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
                #[cfg(test)]
                let park_after_native_rename_marker = self.park_after_native_rename_marker.clone();
                let cancel_for_prepare = self.cancel_flag.clone();
                let control_for_prepare = self.torch_control.clone();
                let custody_for_prepare = custody.clone();
                let settled_for_prepare = cancellation_settled.clone();
                let request = AcquisitionHttpRequest {
                    demand,
                    manifest,
                    workspace,
                    sources: source_urls
                        .into_iter()
                        .map(|url| AcquisitionHttpSource {
                            url,
                            authorization: None,
                        })
                        .collect(),
                    retry,
                };
                let cuda_for_prepare = cuda.clone();
                consumer
                    .acquire_http(
                        request,
                        client,
                        Box::new(host),
                        move |use_set| async move {
                            let archive = use_set.open_file(0).await?;
                            let mut cuda_wheels = Vec::new();
                            if let Some(cuda) = &cuda_for_prepare {
                                for index in 0..cuda.wheels.len() {
                                    cuda_wheels.push(use_set.open_file(index + 1).await?);
                                }
                            }
                            let stage_for_extract = stage.clone();
                            let custody_for_extract = custody_for_prepare.clone();
                            let asset_name = asset_for_prepare.clone();
                            let extracted = use_set
                                .run_blocking("extract publisher-verified native archive", move || {
                                    custody_for_extract.grant.validate()?;
                                    if path_exists_sync(&stage_for_extract)? {
                                        return Err(PumasError::InstallationFailed {
                                            message: format!("{runtime_name} output stage already contains unresolved work"),
                                        });
                                    }
                                    std::fs::create_dir(&stage_for_extract)
                                        .map_err(|error| PumasError::io_with_path(error, &stage_for_extract))?;
                                    VersionInstaller::extract_native_archive_from(
                                        runtime,
                                        archive,
                                        &stage_for_extract,
                                        &asset_name,
                                    )?;
                                    if let Some(cuda) = &cuda_for_prepare {
                                        let platform = python_index::WheelPlatform::host()
                                            .ok_or_else(|| PumasError::InstallationFailed {
                                                message: "ONNX Runtime CUDA libraries exist only for Linux x64 and Windows x64".into(),
                                            })?;
                                        onnx_cuda::extract_libraries(
                                            cuda_wheels,
                                            cuda,
                                            &stage_for_extract,
                                            platform,
                                        )?;
                                    }
                                    sync_native_tree(&stage_for_extract)?;
                                    sync_native_directory(stage_for_extract.parent().ok_or_else(|| PumasError::Other("Native output parent absent".into()))?)?;
                                    let output_tree_sha256 = hash_native_tree(&stage_for_extract)?;
                                    let launcher =
                                        stage_for_extract.join(runtime.entry_relative_path());
                                    let launcher_sha256 = hash_regular_file(&launcher)?;
                                    Ok((output_tree_sha256, launcher_sha256))
                                })
                                .await?;
                            let metadata_sha256 = hash_metadata(&metadata_for_prepare)?;
                            let proof = NativeInstallReceiptV1 {
                                schema_version: 1,
                                tag: tag_for_prepare,
                                metadata: metadata_for_prepare,
                                metadata_sha256,
                                output_tree_sha256: extracted.0,
                                launcher_relative_path: runtime.entry_relative_path().into(),
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
                                let withdrawal = use_set.withdraw_after_cleanup(move || {
                                    custody_for_prepare.revoke_and_clear()
                                }).await;
                                if withdrawal.is_ok() {
                                    settled_for_prepare.store(true, Ordering::SeqCst);
                                }
                                return settled_result(Err(VersionInstaller::cancellation_error()), withdrawal);
                            }
                            let prepared = PreparedNativeInstall {
                                use_set,
                                stage,
                                destination: destination.clone(),
                                custody: custody_for_prepare,
                            };
                            Ok((prepared, serde_json::to_value(proof)?))
                        },
                        move |prepared, issued| async move {
                            let proof: NativeInstallReceiptV1 =
                                serde_json::from_value(issued.payload).map_err(|error| {
                                    PumasError::Validation {
                                        field: "acquisition.consumer_receipt".into(),
                                        message: format!("{runtime_name} publication receipt is malformed: {error}"),
                                    }
                                })?;
                            prepared
                                .use_set
                                .run_blocking("publish verified native installation", move || {
                                    VersionInstaller::publish_native_installation(
                                        NativePublication {
                                            versions: versions_for_publish,
                                            app_id,
                                            manager: manager_for_publish,
                                            custody: prepared.custody,
                                            stage: prepared.stage,
                                            destination: prepared.destination,
                                            proof,
                                            #[cfg(test)]
                                            interrupt_after_native_rename,
                                            #[cfg(test)]
                                            park_after_native_rename_marker,
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

        // The transfer hook returns the original cancellation only after its
        // exact withdrawal is durable. Cleared bytes alone do not prove that
        // settlement succeeded; retain the shell after withdrawal uncertainty.
        if matches!(&result, Err(PumasError::DownloadCancelled))
            && custody.contents_cleared.load(Ordering::SeqCst)
        {
            cancellation_settled.store(true, Ordering::SeqCst);
        }
        let cleanup = if result.is_ok() || cancellation_settled.load(Ordering::SeqCst) {
            Some(
                consumer
                    .run_blocking("clean settled native workspace", move || {
                        Ok(cleanup_settled_native_workspace(custody))
                    })
                    .await
                    .unwrap_or_else(NativeCleanupReport::retained),
            )
        } else {
            None
        };
        // Failed clear retains acquisition custody. A cancelled operation keeps
        // both its cancellation cause and any later shell-cleanup error.
        let result = match (
            &result,
            cleanup
                .as_ref()
                .and_then(NativeCleanupReport::pending_message),
        ) {
            (Err(error), Some(cleanup)) => Err(PumasError::InstallationFailed {
                message: format!("{error}; {cleanup}"),
            }),
            _ => result,
        };

        {
            let mut tracker = self.progress_tracker.write().await;
            if let Err(error) = result.as_ref() {
                tracker.set_error(&error.to_string());
            }
            tracker.complete_installation(result.is_ok());
        }

        if result.is_ok() {
            if let Some(cleanup) = &cleanup {
                self.report_native_cleanup(tag, cleanup, Some(&progress_tx))
                    .await;
            }
        }

        result
    }

    /// Execute Ollama installation steps.
    async fn do_ollama_install(
        &self,
        archive: OllamaArchive<'_>,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        let OllamaArchive {
            tag,
            release,
            client,
            download_url,
            recorded_download_url,
            total_size,
            asset_name,
            archive_path,
            cache_valid,
            expected,
        } = archive;
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
            self.download_archive(client, download_url, archive_path, expected, progress_tx)
                .await?;
        }

        // Check cancellation
        self.check_cancelled()?;

        // Step 2: Extract into a private stage beside the version directory.
        // Nothing reaches `<versions>/<tag>` until the output is validated.
        let versions = self.versions_dir();
        fs::create_dir_all(&versions)
            .await
            .map_err(|e| PumasError::io_with_path(e, &versions))?;
        let stage = tempfile::Builder::new()
            .prefix(".ollama-install-")
            .tempdir_in(&versions)
            .map_err(|e| PumasError::io_with_path(e, &versions))?;

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
        let stage_for_extract = stage.path().to_path_buf();
        let asset_name = asset_name.to_string();
        let expected_for_extract = expected.clone();
        tokio::task::spawn_blocking(move || {
            // Hash and extract the same open handle, so the bytes extracted
            // are exactly the bytes that matched the publisher digest.
            let mut archive = File::open(&archive_path)
                .map_err(|e| PumasError::io_with_path(e, &archive_path))?;
            expected_for_extract.verify_file(&mut archive, &archive_path)?;
            Self::extract_ollama_binary_from(archive, &stage_for_extract, &asset_name)
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

        // Step 4: Publish the stage and its metadata (no venv, no deps)
        self.finalize_ollama_installation(
            tag,
            release,
            stage.path(),
            recorded_download_url,
            total_size,
            progress_tx,
        )
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

    fn publish_native_installation(publication: NativePublication) -> Result<()> {
        let NativePublication {
            versions,
            app_id,
            manager,
            custody,
            stage,
            destination,
            proof,
            #[cfg(test)]
            interrupt_after_native_rename,
            #[cfg(test)]
            park_after_native_rename_marker,
        } = publication;
        custody.grant.validate()?;
        Self::validate_native_receipt(NativeRuntime::for_app(app_id)?, &proof.tag, &proof, &stage)?;
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
            #[cfg(test)]
            if let Some(marker) = park_after_native_rename_marker {
                let pending_marker = marker.with_extension("pending");
                std::fs::write(
                    &pending_marker,
                    b"native destination durable; metadata not published",
                )
                .expect("write native post-rename test boundary marker");
                std::fs::rename(&pending_marker, &marker)
                    .expect("publish native post-rename test boundary marker");
                loop {
                    std::thread::park();
                }
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

    fn reconcile_native_installation(
        versions: &Path,
        tag: &str,
        app_id: AppId,
        manager: &MetadataManager,
        custody: Arc<NativeInstallWorkspace>,
        proof: NativeInstallReceiptV1,
    ) -> Result<()> {
        custody.grant.validate()?;
        let runtime = NativeRuntime::for_app(app_id)?;
        Self::validate_native_receipt(
            runtime,
            tag,
            &proof,
            custody.path().join("output").as_path(),
        )?;
        let destination = versions.join(tag);
        let staged_output = custody.path().join("output");
        match std::fs::symlink_metadata(&destination) {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(PumasError::InstallationFailed {
                        message: format!(
                            "Retained {} destination is not a directory",
                            runtime.display_name()
                        ),
                    });
                }
                verify_native_runtime_output(runtime, &destination, &proof)?;
                if path_exists_sync(&staged_output)? {
                    return Err(PumasError::InstallationFailed {
                        message: format!(
                            "Both staged and published {} output exist",
                            runtime.display_name()
                        ),
                    });
                }
                match manager.get_installed_version(tag, Some(app_id))? {
                    Some(current) if metadata_matches(&current, &proof.metadata) => {}
                    Some(_) => {
                        return Err(PumasError::InstallationFailed {
                            message: format!(
                                "Installed {} metadata conflicts with its receipt",
                                runtime.display_name()
                            ),
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
                verify_native_runtime_output(runtime, &staged_output, &proof)?;
                if manager.get_installed_version(tag, Some(app_id))?.is_some() {
                    return Err(PumasError::InstallationFailed {
                        message: format!(
                            "Installed metadata exists without its {} output",
                            runtime.display_name()
                        ),
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

    fn validate_native_receipt(
        runtime: NativeRuntime,
        tag: &str,
        proof: &NativeInstallReceiptV1,
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
                message: format!(
                    "{} receipt does not match its exact version or metadata",
                    runtime.display_name()
                ),
            });
        }
        if path_exists_sync(output)? {
            verify_native_runtime_output(runtime, output, proof)?;
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
        // A runtime variant (`b1234+cuda-12`) carries only the archive the user
        // chose; install exactly that one, whatever its flavor.
        let mut platform_archives = assets.iter().filter(|asset| {
            Self::is_llama_cpp_platform_archive(&asset.name.to_ascii_lowercase(), platform, arch)
        });
        if let (Some(only), None) = (platform_archives.next(), platform_archives.next()) {
            return Ok(only);
        }
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

    /// Extract a verified Ollama archive into `version_dir`, then validate the
    /// whole output before the launch binary is located and made executable.
    ///
    /// Ollama releases are distributed as:
    /// - Linux: ollama-linux-amd64.tar.zst (Zstandard compressed tar, current format)
    /// - Linux (legacy): ollama-linux-amd64.tgz (gzip compressed tar)
    /// - macOS: ollama-darwin-arm64.tar.zst
    /// - Windows: ollama-windows-amd64.zip (containing ollama.exe)
    fn extract_ollama_binary_from(
        archive: File,
        version_dir: &Path,
        asset_name: &str,
    ) -> Result<()> {
        info!("Extracting Ollama binary from {}", asset_name);

        if asset_name.ends_with(".tar.zst") {
            // Zstandard-compressed tar: the current Ollama format.
            Self::extract_tar_zst_from(archive, version_dir)?;
        } else if asset_name.ends_with(".tgz") || asset_name.ends_with(".tar.gz") {
            Self::extract_tarball_from(archive, version_dir)?;
        } else if asset_name.ends_with(".zip") {
            Self::extract_zip_from(archive, version_dir)?;
        } else {
            return Err(PumasError::InstallationFailed {
                message: format!("Unsupported Ollama archive format: {asset_name}"),
            });
        }

        // No link may leave the output and no special file may exist before
        // anything inside it is renamed or has its permissions changed.
        Self::validate_native_output(version_dir)?;
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

    /// Extract an upstream ONNX Runtime archive so the version directory holds
    /// its `lib/`, `include/` and notices, and the library the provider loads
    /// is a regular file at [`NativeRuntime::entry_relative_path`].
    ///
    /// Upstream archives wrap their contents in one `onnxruntime-<platform>-<version>`
    /// directory, which is removed. On Linux and macOS the unversioned library
    /// name is a link to the versioned file; the file takes the unversioned name
    /// and the versioned name becomes the link, so every upstream name still
    /// resolves and the receipt can hash one regular file.
    fn extract_onnx_runtime_from(archive: File, stage: &Path, asset_name: &str) -> Result<()> {
        info!("Extracting ONNX Runtime from {}", asset_name);
        if asset_name.ends_with(".tgz") || asset_name.ends_with(".tar.gz") {
            Self::extract_tarball_from(archive, stage)?;
        } else if asset_name.ends_with(".zip") {
            Self::extract_zip_from(archive, stage)?;
        } else {
            return Err(PumasError::InstallationFailed {
                message: format!("Unsupported ONNX Runtime archive format: {asset_name}"),
            });
        }
        Self::validate_native_output(stage)?;
        Self::hoist_single_archive_root(stage)?;
        let entry = NativeRuntime::OnnxRuntime.entry_relative_path();
        Self::materialize_linked_entry(stage, Path::new(entry))?;
        Self::validate_native_output(stage)?;
        let metadata = std::fs::symlink_metadata(stage.join(entry))
            .map_err(|error| PumasError::io_with_path(error, stage.join(entry)))?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(PumasError::InstallationFailed {
                message: format!("ONNX Runtime archive library {entry} is empty"),
            });
        }
        Ok(())
    }

    /// Move the contents of an archive's single top-level directory into `stage`.
    fn hoist_single_archive_root(stage: &Path) -> Result<()> {
        let entries = std::fs::read_dir(stage)
            .map_err(|error| PumasError::io_with_path(error, stage))?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|error| PumasError::io_with_path(error, stage))?;
        let [only] = entries.as_slice() else {
            return Ok(());
        };
        let file_type = only
            .file_type()
            .map_err(|error| PumasError::io_with_path(error, only.path()))?;
        if !file_type.is_dir() {
            return Ok(());
        }
        // A child may share the wrapper's name; move the wrapper aside first.
        let wrapper = stage.join(".onnx-runtime-archive-root");
        std::fs::rename(only.path(), &wrapper)
            .map_err(|error| PumasError::io_with_path(error, &wrapper))?;
        for child in std::fs::read_dir(&wrapper)
            .map_err(|error| PumasError::io_with_path(error, &wrapper))?
        {
            let child = child.map_err(|error| PumasError::io_with_path(error, &wrapper))?;
            let destination = stage.join(child.file_name());
            std::fs::rename(child.path(), &destination)
                .map_err(|error| PumasError::io_with_path(error, &destination))?;
        }
        std::fs::remove_dir(&wrapper).map_err(|error| PumasError::io_with_path(error, &wrapper))
    }

    /// Make `relative` a regular file when the archive ships it as a link to a
    /// sibling file, and leave the sibling name as a link to it.
    fn materialize_linked_entry(stage: &Path, relative: &Path) -> Result<()> {
        let entry = stage.join(relative);
        let metadata = match std::fs::symlink_metadata(&entry) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(PumasError::InstallationFailed {
                    message: format!(
                        "ONNX Runtime archive has no {}; it is not a supported runtime archive",
                        relative.display()
                    ),
                })
            }
            Err(error) => return Err(PumasError::io_with_path(error, &entry)),
        };
        if !metadata.file_type().is_symlink() {
            return Ok(());
        }
        // `validate_native_output` already proved the link is relative and
        // resolves to a regular file inside the stage.
        let target = entry
            .canonicalize()
            .map_err(|error| PumasError::io_with_path(error, &entry))?;
        let directory = entry
            .parent()
            .ok_or_else(|| PumasError::Other("ONNX Runtime library has no parent".into()))?
            .canonicalize()
            .map_err(|error| PumasError::io_with_path(error, &entry))?;
        if target.parent() != Some(directory.as_path()) {
            return Err(PumasError::InstallationFailed {
                message: "ONNX Runtime library link must name a file in the same directory".into(),
            });
        }
        std::fs::remove_file(&entry).map_err(|error| PumasError::io_with_path(error, &entry))?;
        std::fs::rename(&target, &entry)
            .map_err(|error| PumasError::io_with_path(error, &entry))?;
        #[cfg(unix)]
        {
            let entry_name = entry
                .file_name()
                .ok_or_else(|| PumasError::Other("ONNX Runtime library has no name".into()))?;
            std::os::unix::fs::symlink(entry_name, &target)
                .map_err(|error| PumasError::io_with_path(error, &target))?;
        }
        #[cfg(not(unix))]
        {
            std::fs::copy(&entry, &target)
                .map_err(|error| PumasError::io_with_path(error, &target))?;
        }
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
            } else if !entry.file_type().is_file() && !entry.file_type().is_dir() {
                return Err(PumasError::InstallationFailed {
                    message: "Native output contains a special file".into(),
                });
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

    fn extract_tar_zst_from(file: File, dest_dir: &Path) -> Result<()> {
        let budget = ExtractionBudget::for_archive(&file, dest_dir)?;
        let decoder = zstd::Decoder::new(BufReader::new(file)).map_err(|e| PumasError::Io {
            message: format!("Failed to create zstd decoder: {}", e),
            path: Some(dest_dir.to_path_buf()),
            source: Some(std::io::Error::other(e)),
        })?;
        Self::unpack_tar_within(tar::Archive::new(decoder), dest_dir, budget, "tar.zst")
    }

    /// Unpack a tar stream entry by entry so the cumulative size and entry
    /// count stay inside the archive's [`ExtractionBudget`]. As in
    /// `tar::Archive::unpack`, directory entries are applied last so their
    /// permissions cannot block descendants, and `unpack_in` keeps every entry
    /// inside `dest_dir`. Permissions are not preserved, so the tar crate masks
    /// each mode to `0o777`: no setuid, setgid or sticky bit is applied.
    fn unpack_tar_within<R: std::io::Read>(
        mut archive: tar::Archive<R>,
        dest_dir: &Path,
        mut budget: ExtractionBudget,
        format: &str,
    ) -> Result<()> {
        archive.set_preserve_permissions(false);
        archive.set_preserve_ownerships(false);
        archive.set_unpack_xattrs(false);
        std::fs::create_dir_all(dest_dir).map_err(|e| PumasError::io_with_path(e, dest_dir))?;
        let dest_dir = dest_dir
            .canonicalize()
            .map_err(|e| PumasError::io_with_path(e, dest_dir))?;
        let failed = |e: std::io::Error| PumasError::InstallationFailed {
            message: format!("Failed to extract {format}: {e}"),
        };
        let mut directories = Vec::new();
        for entry in archive.entries().map_err(failed)? {
            let mut entry = entry.map_err(failed)?;
            budget.admit(entry.size())?;
            if entry.header().entry_type() == tar::EntryType::Directory {
                directories.push(entry);
            } else if !entry.unpack_in(&dest_dir).map_err(failed)? {
                return Err(PumasError::InstallationFailed {
                    message: format!(
                        "{format} entry has no safe path inside the extraction directory"
                    ),
                });
            }
        }
        directories.sort_by(|a, b| b.path_bytes().cmp(&a.path_bytes()));
        for mut directory in directories {
            directory.unpack_in(&dest_dir).map_err(failed)?;
        }
        Ok(())
    }

    /// Move the Ollama binary to its launch path and make it executable.
    ///
    /// `version_dir` is this attempt's private stage, already checked by
    /// [`Self::validate_native_output`]. The binary is inspected with
    /// `symlink_metadata` and must be a non-empty regular file: a link would
    /// let `set_permissions`, which follows links, act on another file.
    fn finalize_ollama_binary(version_dir: &Path) -> Result<()> {
        let binary_name = if cfg!(windows) {
            "ollama.exe"
        } else {
            "ollama"
        };
        let final_path = version_dir.join(binary_name);

        match std::fs::symlink_metadata(&final_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let found = Self::find_ollama_binary(version_dir)?.ok_or_else(|| {
                    PumasError::InstallationFailed {
                        message: "Could not find Ollama binary in extracted archive".to_string(),
                    }
                })?;
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
            Err(error) => return Err(PumasError::io_with_path(error, &final_path)),
        }

        let metadata = std::fs::symlink_metadata(&final_path)
            .map_err(|e| PumasError::io_with_path(e, &final_path))?;
        if metadata.file_type().is_symlink() {
            return Err(PumasError::InstallationFailed {
                message: "Ollama archive supplies its binary as a symbolic link; only a regular file is accepted".into(),
            });
        }
        if !metadata.is_file() {
            return Err(PumasError::InstallationFailed {
                message: "Ollama archive binary is not a regular file".into(),
            });
        }
        if metadata.len() == 0 {
            return Err(PumasError::InstallationFailed {
                message: "Extracted Ollama binary is empty".into(),
            });
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&final_path, std::fs::Permissions::from_mode(0o755)).map_err(
                |e| PumasError::Io {
                    message: format!("Failed to set binary permissions: {}", e),
                    path: Some(final_path.clone()),
                    source: Some(e),
                },
            )?;
            info!("Set executable permissions on {}", final_path.display());
        }

        Ok(())
    }

    /// Find the first entry named like the Ollama binary without following
    /// links. A matching link is returned, not skipped, so that the caller
    /// refuses it with a reason instead of reporting a missing binary.
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
            let file_type = entry
                .file_type()
                .map_err(|e| PumasError::io_with_path(e, &path))?;

            if file_type.is_dir() {
                if let Some(found) = Self::find_ollama_binary(&path)? {
                    return Ok(Some(found));
                }
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| binary_names.contains(&name))
            {
                return Ok(Some(path));
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

    /// Finalize Ollama installation: publish the validated stage as the
    /// version directory together with its metadata (no Python/venv).
    async fn finalize_ollama_installation(
        &self,
        tag: &str,
        release: &GitHubRelease,
        stage: &Path,
        download_url: String,
        archive_size: u64,
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

        // Create metadata entry (no Python version for Ollama)
        let metadata = InstalledVersionMetadata {
            path: tag.to_string(),
            installed_date: Utc::now().to_rfc3339(),
            release_tag: tag.to_string(),
            python_version: None, // Ollama is a Go binary, no Python
            git_commit: None,
            release_date: Some(release.published_at.clone()),
            release_notes: release.body.clone(),
            download_url: Some(download_url),
            size: Some(archive_size),
            requirements_hash: None,
            dependencies_installed: Some(true), // No dependencies needed
        };

        // Publish the stage and its metadata together: a metadata failure
        // withdraws the newly published directory back into the stage.
        let destination = self.versions_dir().join(tag);
        if path_exists(&destination).await? {
            return Err(PumasError::InstallationFailed {
                message: format!(
                    "Ollama version directory {} already exists; remove it and retry",
                    destination.display()
                ),
            });
        }
        let stage = stage.to_path_buf();
        let manager = self.metadata_manager.clone();
        let app_id = self.app_id;
        let tag_for_publish = tag.to_owned();
        tokio::task::spawn_blocking(move || {
            Self::publish_native_stage(&stage, &destination, || {
                manager
                    .update_installed_version(&tag_for_publish, metadata, Some(app_id))
                    .map_err(|error| (error, true))
            })
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join Ollama publication task: {e}")))??;

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

    /// HTTPS-only client for publisher artifact downloads, with timeouts for
    /// large files: a connect timeout but no overall timeout (1.6 GB+). Test
    /// builds reuse a validated loopback fixture's transport for its origin.
    fn native_download_client(&self, download_url: &str) -> Result<reqwest::Client> {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(client) = self
            .github_client
            .as_ref()
            .map(|client| client.loopback_fixture_http_client(download_url))
            .transpose()?
            .flatten()
        {
            return Ok(client);
        }
        #[cfg(not(any(test, feature = "test-support")))]
        let _ = download_url;
        reqwest::Client::builder()
            .connect_timeout(InstallationConfig::URL_FETCH_TIMEOUT)
            .user_agent("pumas-library")
            .https_only(true)
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: Some(e.to_string()),
            })
    }

    /// Whether a cached archive still hashes to the publisher digest. A
    /// mismatching or unreadable cache entry is removed so it is downloaded
    /// again.
    async fn cached_archive_matches(
        archive_path: &Path,
        expected: &ExpectedArchive,
    ) -> Result<bool> {
        if !path_exists(archive_path).await? {
            return Ok(false);
        }
        let path = archive_path.to_path_buf();
        let expected = expected.clone();
        let verified = tokio::task::spawn_blocking(move || {
            let mut file = File::open(&path).map_err(|e| PumasError::io_with_path(e, &path))?;
            expected.verify_file(&mut file, &path)
        })
        .await
        .map_err(|e| PumasError::Other(format!("Failed to join cache verification task: {e}")))?;
        match verified {
            Ok(()) => Ok(true),
            Err(error) => {
                warn!("Discarding cached download that failed re-verification: {error}");
                let _ = fs::remove_file(archive_path).await;
                Ok(false)
            }
        }
    }

    /// Download `url` to `archive_path`, hashing as it streams. The bytes are
    /// written to a sibling `.part` file and renamed into place only after
    /// they match the publisher digest and size, so the cache never holds an
    /// unverified archive.
    async fn download_archive(
        &self,
        client: &reqwest::Client,
        url: &str,
        archive_path: &Path,
        expected: &ExpectedArchive,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<()> {
        info!("Downloading archive from {}", url);
        let part_path = archive_path.with_file_name(format!(
            "{}.part",
            archive_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| PumasError::Other("Archive cache path has no file name".into()))?
        ));

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

        let total_size = expected.size.or(response.content_length());

        // Create output file
        let mut file = fs::File::create(&part_path)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create archive file: {}", e),
                path: Some(part_path.clone()),
                source: Some(e),
            })?;

        // Cancellation interrupts network waits only. File operations and
        // their settlement remain awaited before the stage can be reclaimed.
        let transfer = async {
            let mut downloaded: u64 = 0;
            let mut digest = Sha256::new();
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

                downloaded = downloaded
                    .checked_add(chunk.len() as u64)
                    .filter(|total| expected.size.is_none_or(|size| *total <= size))
                    .ok_or_else(|| PumasError::InstallationFailed {
                        message: format!(
                            "Download exceeds the publisher's declared size of {} bytes",
                            expected.size.unwrap_or_default()
                        ),
                    })?;
                digest.update(&chunk);
                file.write_all(&chunk).await.map_err(|e| PumasError::Io {
                    message: format!("Failed to write to archive: {}", e),
                    path: Some(part_path.clone()),
                    source: Some(e),
                })?;

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
            Ok((downloaded, format!("{:x}", digest.finalize())))
        }
        .await;

        let verified = settle_archive_file(file, &part_path, transfer)
            .await
            .and_then(|(downloaded, sha256)| {
                expected.verify(&sha256, downloaded)?;
                Ok(downloaded)
            });
        let downloaded = match verified {
            Ok(downloaded) => downloaded,
            Err(error) => {
                let _ = fs::remove_file(&part_path).await;
                return Err(error);
            }
        };
        fs::rename(&part_path, archive_path)
            .await
            .map_err(|e| PumasError::io_with_path(e, archive_path))?;

        // Add to completed items
        {
            let mut tracker = self.progress_tracker.write().await;
            tracker.add_completed_item("archive", "archive", Some(downloaded));
        }

        info!("Download complete: {} bytes", downloaded);
        Ok(())
    }

    /// Extracts a ZIP archive below `extract_dir`. A root or drive prefix is
    /// dropped from an entry's name; an entry whose name would still leave
    /// that directory, such as one starting with `..`, rejects the archive.
    #[cfg(test)]
    fn extract_zip(archive_path: &Path, extract_dir: &Path) -> Result<()> {
        let file = File::open(archive_path).map_err(|e| PumasError::Io {
            message: format!("Failed to open zip archive: {}", e),
            path: Some(archive_path.to_path_buf()),
            source: Some(e),
        })?;
        Self::extract_zip_from(file, extract_dir)
    }

    fn extract_zip_from(file: File, extract_dir: &Path) -> Result<()> {
        let mut budget = ExtractionBudget::for_archive(&file, extract_dir)?;
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
            // The declared size is admitted against the cumulative budget, and
            // the copy below is held to it, so an entry that inflates past its
            // header cannot exceed the budget either.
            let declared = file.size();
            budget.admit(declared)?;

            let outpath = match file.enclosed_name() {
                Some(path) => extract_dir.join(path),
                None => {
                    return Err(PumasError::InstallationFailed {
                        message: format!(
                            "Zip entry {i} has no safe path inside the extraction directory"
                        ),
                    })
                }
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

                let written = std::io::copy(
                    &mut std::io::Read::take(&mut file, declared.saturating_add(1)),
                    &mut outfile,
                )
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to extract file: {}", e),
                    path: Some(outpath.clone()),
                    source: Some(e),
                })?;
                if written > declared {
                    return Err(PumasError::InstallationFailed {
                        message: format!(
                            "Zip entry {i} expands beyond its declared size of {declared} bytes"
                        ),
                    });
                }
            }

            // Only permission bits are applied: an archive's setuid, setgid
            // and sticky bits are never honoured.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(
                        &outpath,
                        std::fs::Permissions::from_mode(mode & 0o777),
                    )
                    .ok();
                }
            }
        }

        Ok(())
    }

    fn extract_tarball_from(file: File, extract_dir: &Path) -> Result<()> {
        let budget = ExtractionBudget::for_archive(&file, extract_dir)?;
        let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
        Self::unpack_tar_within(tar::Archive::new(decoder), extract_dir, budget, "tarball")
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
                {
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
            unavailable_reason: None,
            install_availability: None,
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
        mark_native_attempt_removed(root.path(), "b1234+cpu", lock.clone())
            .unwrap()
            .into_result()
            .unwrap();
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
        reinstalled.cleanup().into_result().unwrap();
    }

    #[test]
    fn native_v2_reopen_rejects_replacement_and_missing_stage_without_mutation() {
        for replace_root in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let versions = temp.path().join("versions");
            let first = NativeInstallWorkspace::create(&versions, "fixture").unwrap();
            let path = first.path().to_owned();
            let relative = path.strip_prefix(&versions).unwrap().to_owned();
            std::fs::write(path.join("owned"), b"owned").unwrap();
            let record = read_native_attempt(&versions, "fixture").unwrap().unwrap();
            assert_eq!(record.schema_version, 2);
            assert!(record.binding.is_some());
            assert!(record.cleanup_pending.is_some());
            drop(first);
            let retired = temp.path().join("retired");
            std::fs::rename(if replace_root { &versions } else { &path }, &retired).unwrap();
            std::fs::create_dir_all(&path).unwrap();
            if replace_root {
                std::fs::copy(
                    native_attempt_path(&retired, "fixture"),
                    native_attempt_path(&versions, "fixture"),
                )
                .unwrap();
            }
            std::fs::write(path.join("sentinel"), b"replacement").unwrap();
            assert!(NativeInstallWorkspace::create(&versions, "fixture").is_err());
            let lock = NativeVersionsLock::try_acquire(&versions).unwrap();
            let report = cleanup_native_workspace_if_present(&versions, &record, lock);
            assert!(matches!(report, NativeCleanupReport::Retained { .. }));
            assert_eq!(
                std::fs::read(path.join("sentinel")).unwrap(),
                b"replacement"
            );
            let original = if replace_root {
                retired.join(relative)
            } else {
                retired
            };
            assert_eq!(std::fs::read(original.join("owned")).unwrap(), b"owned");
        }
        let root = tempfile::tempdir().unwrap();
        let first = NativeInstallWorkspace::create(root.path(), "missing").unwrap();
        let path = first.path().to_owned();
        let attempt = first.attempt.clone();
        drop(first);
        std::fs::remove_dir(&path).unwrap();
        assert!(
            NativeInstallWorkspace::create_for_attempt(root.path(), "missing", Some(&attempt))
                .is_err()
        );
        assert!(!path.exists());
        let identity = read_native_attempt(root.path(), "missing")
            .unwrap()
            .unwrap();
        let lock = NativeVersionsLock::try_acquire(root.path()).unwrap();
        assert!(matches!(
            cleanup_native_workspace_if_present(root.path(), &identity, lock),
            NativeCleanupReport::Absent
        ));
    }

    #[test]
    fn native_legacy_present_is_retained_and_absent_cleanup_is_harmless() {
        let root = tempfile::tempdir().unwrap();
        let legacy = NativeAttemptIdentity {
            schema_version: 1,
            tag: "fixture".into(),
            attempt: "a".repeat(32),
            removed: false,
            binding: None,
            cleanup_pending: None,
        };
        write_native_attempt(root.path(), &legacy).unwrap();
        let path = native_workspace_path(root.path(), "fixture", &legacy.attempt);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("sentinel"), b"legacy").unwrap();
        let record_before = std::fs::read(native_attempt_path(root.path(), "fixture")).unwrap();
        assert!(NativeInstallWorkspace::create(root.path(), "fixture").is_err());
        let lock = NativeVersionsLock::try_acquire(root.path()).unwrap();
        let report = cleanup_native_workspace_if_present(root.path(), &legacy, lock.clone());
        assert!(matches!(report, NativeCleanupReport::Retained { .. }));
        assert_eq!(std::fs::read(path.join("sentinel")).unwrap(), b"legacy");
        assert_eq!(
            std::fs::read(native_attempt_path(root.path(), "fixture")).unwrap(),
            record_before
        );
        // An absent legacy leaf needs no fabricated binding or schema migration.
        std::fs::remove_file(path.join("sentinel")).unwrap();
        std::fs::remove_dir(path).unwrap();
        assert!(matches!(
            cleanup_native_workspace_if_present(root.path(), &legacy, lock),
            NativeCleanupReport::Absent
        ));
        assert_eq!(
            std::fs::read(native_attempt_path(root.path(), "fixture")).unwrap(),
            record_before
        );
    }

    #[test]
    fn native_creation_crash_orphan_is_not_adopted_by_a_later_attempt() {
        let root = tempfile::tempdir().unwrap();
        let orphan = native_workspace_path(root.path(), "fixture", &"f".repeat(32));
        std::fs::create_dir(&orphan).unwrap();
        std::fs::write(
            orphan.join("sentinel"),
            b"created before identity persistence",
        )
        .unwrap();
        assert!(read_native_attempt(root.path(), "fixture")
            .unwrap()
            .is_none());
        let admitted = NativeInstallWorkspace::create(root.path(), "fixture").unwrap();
        assert_ne!(admitted.path(), orphan);
        assert_eq!(
            std::fs::read(orphan.join("sentinel")).unwrap(),
            b"created before identity persistence"
        );
        let identity = read_native_attempt(root.path(), "fixture")
            .unwrap()
            .unwrap();
        assert_eq!(identity.binding.as_ref(), Some(admitted.grant.binding()));
        admitted.cleanup().into_result().unwrap();
        assert!(orphan.exists());
    }

    #[test]
    fn native_cancellation_clears_before_releasing_acquisition_and_removes_shell_after() {
        let root = tempfile::tempdir().unwrap();
        let (custody, workspace) =
            prepare_native_acquisition_workspace(root.path().to_owned(), "fixture".into(), None)
                .unwrap();
        let path = custody.path().to_owned();
        std::fs::write(path.join("input"), b"owned").unwrap();
        custody.revoke_and_clear().unwrap();
        assert!(
            read_native_attempt(root.path(), "fixture")
                .unwrap()
                .unwrap()
                .removed
        );
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
        assert!(custody.grant.acquisition_workspace().is_err());
        assert!(custody.grant.clone().remove_empty().is_err());
        assert!(NativeVersionsLock::try_acquire(root.path()).is_err());
        drop(workspace);
        cleanup_settled_native_workspace(custody)
            .into_result()
            .unwrap();
        assert!(!path.exists());
        assert!(read_native_attempt(root.path(), "fixture")
            .unwrap()
            .unwrap()
            .cleanup_pending
            .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn native_failed_clear_preserves_cancellation_and_cleanup_causes() {
        let root = tempfile::tempdir().unwrap();
        let (custody, workspace) =
            prepare_native_acquisition_workspace(root.path().to_owned(), "fixture".into(), None)
                .unwrap();
        let path = custody.path().to_owned();
        std::fs::write(path.join("owned"), b"original").unwrap();
        let retired = root.path().join("retired");
        std::fs::rename(&path, &retired).unwrap();
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("sentinel"), b"replacement").unwrap();
        let error = settled_result::<()>(
            Err(VersionInstaller::cancellation_error()),
            custody.revoke_and_clear(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("cancelled"), "{error}");
        assert!(error.contains("binding changed"), "{error}");
        assert!(!custody.contents_cleared.load(Ordering::SeqCst));
        let identity = read_native_attempt(root.path(), "fixture")
            .unwrap()
            .unwrap();
        assert!(identity.removed && identity.cleanup_pending.is_some());
        assert_eq!(
            std::fs::read(path.join("sentinel")).unwrap(),
            b"replacement"
        );
        assert_eq!(std::fs::read(retired.join("owned")).unwrap(), b"original");
        assert!(NativeVersionsLock::try_acquire(root.path()).is_err());
        drop(workspace);
        drop(custody);
    }

    #[test]
    fn native_removed_stage_with_failed_diagnostic_is_not_reported_retained() {
        let root = tempfile::tempdir().unwrap();
        let custody = NativeInstallWorkspace::create(root.path(), "fixture").unwrap();
        let path = custody.path().to_owned();
        let saved_record = root.path().join("saved-attempt.json");
        std::fs::rename(native_attempt_path(root.path(), "fixture"), &saved_record).unwrap();
        let report = custody.cleanup();
        assert!(!path.exists());
        assert!(matches!(
            report,
            NativeCleanupReport::DiagnosticPending { .. }
        ));
        assert!(report
            .pending_message()
            .unwrap()
            .contains("staging is absent"));
        assert!(saved_record.is_file());
    }

    #[tokio::test]
    async fn native_cleanup_pending_remains_successful_and_visible() {
        let root = tempfile::tempdir().unwrap();
        let tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
            root.path().to_owned(),
        )));
        let installer = VersionInstaller::new(
            root.path().to_owned(),
            AppId::LlamaCpp,
            Arc::new(MetadataManager::new(root.path())),
            tracker.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        let report = NativeCleanupReport::retained("custody mismatch");
        let (tx, mut rx) = mpsc::channel(1);
        installer
            .report_native_cleanup("fixture", &report, Some(&tx))
            .await;
        let state = tracker.read().await.get_current_state().unwrap();
        assert_eq!(state.success, Some(true));
        assert!(state.error.is_none());
        assert!(state
            .current_item
            .as_deref()
            .unwrap()
            .contains("custody mismatch"));
        assert!(state.current_item.unwrap().contains("cleanup pending"));
        assert!(
            matches!(rx.recv().await, Some(ProgressUpdate::Setup { message }) if message.contains("cleanup pending"))
        );
        let report = NativeCleanupReport::DiagnosticPending {
            reason: "status write failed".into(),
        };
        installer
            .report_native_cleanup("fixture", &report, None)
            .await;
        let state = tracker.read().await.get_current_state().unwrap();
        assert_eq!(state.success, Some(true));
        assert!(state.error.is_none());
        let message = state.current_item.unwrap();
        assert!(message.contains("staging is absent"));
        assert!(!message.contains("staging cleanup pending"));
        installer
            .report_native_cleanup("unrelated", &NativeCleanupReport::Removed, None)
            .await;
        assert!(tracker.read().await.get_current_state().is_some());
        installer
            .report_native_cleanup("fixture", &NativeCleanupReport::Removed, None)
            .await;
        assert!(tracker.read().await.get_current_state().is_none());
    }

    #[test]
    fn native_receipt_reconciles_owned_publication_without_rechecking_checksums() {
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
        let proof = NativeInstallReceiptV1 {
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
        VersionInstaller::reconcile_native_installation(
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
        // Already-published recovery checks owned paths and metadata.
        VersionInstaller::reconcile_native_installation(
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
        VersionInstaller::reconcile_native_installation(
            &versions,
            "b1234+cpu",
            AppId::LlamaCpp,
            &manager,
            custody.clone(),
            proof.clone(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(&published_launcher).unwrap(),
            b"changed output"
        );
        // An empty launcher is unavailable irrespective of checksum policy.
        std::fs::write(&published_launcher, b"").unwrap();
        let error = VersionInstaller::reconcile_native_installation(
            &versions,
            "b1234+cpu",
            AppId::LlamaCpp,
            &manager,
            custody,
            proof,
        )
        .unwrap_err();
        assert!(error.to_string().contains("recovery required"));
        assert_eq!(std::fs::read(published_launcher).unwrap(), b"");
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

    /// Writes a one-entry ZIP whose stored name is `name`. The writer
    /// normalizes names, so `name` replaces a same-length placeholder in both
    /// the local and central headers.
    fn zip_with_raw_entry_name(path: &Path, placeholder: &str, name: &str) {
        use std::io::Write;
        use zip::{write::SimpleFileOptions, ZipWriter};

        let mut archive = std::io::Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(&mut archive);
        writer
            .start_file(placeholder, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"payload").unwrap();
        writer.finish().unwrap();
        let mut bytes = archive.into_inner();
        let mut replaced = 0;
        for start in 0..=bytes.len() - placeholder.len() {
            if &bytes[start..start + placeholder.len()] == placeholder.as_bytes() {
                bytes[start..start + name.len()].copy_from_slice(name.as_bytes());
                replaced += 1;
            }
        }
        assert_eq!(replaced, 2, "local and central header names");
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn runtime_zip_extraction_rejects_a_parent_relative_entry() {
        let temp = tempfile::TempDir::new().unwrap();
        let archive_path = temp.path().join("parent.zip");
        let output = temp.path().join("extracted");
        zip_with_raw_entry_name(&archive_path, "xx/escape", "../escape");

        let error = VersionInstaller::extract_zip(&archive_path, &output).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no safe path inside the extraction directory"),
            "{error}"
        );
        assert!(!temp.path().join("escape").exists());
    }

    #[test]
    fn runtime_zip_extraction_keeps_an_absolute_entry_inside() {
        let temp = tempfile::TempDir::new().unwrap();
        let archive_path = temp.path().join("absolute.zip");
        let output = temp.path().join("extracted");
        zip_with_raw_entry_name(&archive_path, "xabsolute", "/absolute");

        VersionInstaller::extract_zip(&archive_path, &output).unwrap();
        assert_eq!(std::fs::read(output.join("absolute")).unwrap(), b"payload");
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
        next.cleanup().into_result().unwrap();
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn native_cleanup_preserves_replaced_root_or_workspace_contents() {
        for replace_root in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let versions = root.path().join("versions");
            let custody = NativeInstallWorkspace::create(&versions, "fixture").unwrap();
            let original_path = custody.path().to_owned();
            let relative = original_path.strip_prefix(&versions).unwrap().to_owned();
            std::fs::write(original_path.join("owned"), b"owned input").unwrap();
            let retired = root.path().join("retired");
            let source = if replace_root {
                versions.clone()
            } else {
                original_path.clone()
            };
            std::fs::rename(&source, &retired).unwrap();
            std::fs::create_dir_all(&original_path).unwrap();
            std::fs::write(original_path.join("sentinel"), b"unrelated replacement").unwrap();
            let retained_original = if replace_root {
                retired.join(relative)
            } else {
                retired
            };

            let outcome = custody.cleanup().into_result();

            assert_eq!(
                std::fs::read(original_path.join("sentinel"))
                    .ok()
                    .as_deref(),
                Some(b"unrelated replacement".as_slice()),
                "cleanup must never delete replacement contents (root={replace_root})",
            );
            assert!(
                outcome.is_err(),
                "changed binding must require reconciliation"
            );
            assert_eq!(
                std::fs::read(retained_original.join("owned")).unwrap(),
                b"owned input"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn native_cleanup_failure_reports_and_retains_its_path() {
        let root = tempfile::tempdir().unwrap();
        let custody = NativeInstallWorkspace::create(root.path(), "fixture").unwrap();
        let path = custody.path().to_owned();
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, "retained uncertainty").unwrap();
        let error = custody.cleanup().into_result().unwrap_err().to_string();
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
            custody.cleanup().into_result().unwrap();
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

    #[test]
    fn llama_cpp_asset_selection_installs_the_chosen_linux_cuda_variant() {
        for name in [
            "llama-b12099-bin-ubuntu-cuda-12.8-x64.tar.gz",
            "llama-b12099-bin-ubuntu-cuda-13.4-x64.tar.gz",
        ] {
            let assets = vec![github_asset(name)];
            for prefer_gpu in [true, false] {
                let selected = VersionInstaller::select_llama_cpp_asset_for_platform(
                    &assets, "linux", "x64", prefer_gpu,
                )
                .unwrap();
                assert_eq!(selected.name, name);
            }
        }
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
