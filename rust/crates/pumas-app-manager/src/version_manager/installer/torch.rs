//! Managed upstream PyTorch installation; lifecycle and state remain in VersionManager.

use super::*;
use crate::torch_client::{SUPPORTED_TORCH_PROTOCOL, TORCH_IMAGE_GENERATION_CAPABILITY};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

const MAX_TORCH_ORPHAN_QUARANTINES: usize = 2;
const MAX_TORCH_DOWNLOAD_SOURCE_BYTES: usize = 2048;
const TORCH_DOWNLOAD_SPEED_STALE_AFTER: Duration = Duration::from_secs(2);
pub(super) const TORCH_PUBLISHING_MARKER: &[u8] = b"metadata pending";

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct TorchDownloadProgress {
    #[serde(default)]
    source_url: Option<String>,
    active: bool,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    speed_bytes_per_sec: Option<f64>,
    #[serde(default)]
    measurement_available: bool,
}

fn safe_torch_download_source(source: &str) -> bool {
    if source.len() > MAX_TORCH_DOWNLOAD_SOURCE_BYTES
        || source
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(source) else {
        return false;
    };
    url.scheme() == "https"
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn read_torch_download_progress(path: &Path) -> Option<TorchDownloadProgress> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > 4096 {
        return None;
    }
    let progress: TorchDownloadProgress =
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if progress
        .source_url
        .as_deref()
        .is_some_and(|source| !safe_torch_download_source(source))
        || progress
            .speed_bytes_per_sec
            .is_some_and(|speed| !speed.is_finite() || speed < 0.0)
        || (!progress.active && progress.speed_bytes_per_sec.is_some())
        || progress
            .total_bytes
            .is_some_and(|total| progress.downloaded_bytes > total)
    {
        return None;
    }
    Some(progress)
}

/// This file is permanent: unlinking it could let another process lock a new
/// inode while an existing installer still holds the old one.
#[derive(Clone)]
pub(crate) struct TorchVersionsLock {
    _file: std::sync::Arc<std::fs::File>,
}

fn normalize_torch_lock_error(error: std::io::Error) -> std::io::Error {
    let contended = fs2::lock_contended_error().raw_os_error();
    if contended.is_some() && error.raw_os_error() == contended {
        std::io::Error::new(std::io::ErrorKind::WouldBlock, error)
    } else {
        error
    }
}

impl TorchVersionsLock {
    pub(crate) fn try_acquire(versions_dir: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(versions_dir.join(".torch-versions.lock"))?;
        file.try_lock_exclusive()
            .map_err(normalize_torch_lock_error)?;
        Ok(Self {
            _file: std::sync::Arc::new(file),
        })
    }

    /// Briefly wait for another mutation to finish without blocking the async runtime.
    pub(crate) async fn acquire_for_mutation(versions_dir: &Path) -> std::io::Result<Self> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        loop {
            match Self::try_acquire(versions_dir) {
                Ok(lock) => return Ok(lock),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let now = tokio::time::Instant::now();
                    if now >= deadline {
                        return Err(error);
                    }
                    tokio::time::sleep_until((now + Duration::from_millis(20)).min(deadline)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for TorchVersionsLock {
    fn drop(&mut self) {
        if std::sync::Arc::strong_count(&self._file) == 1 {
            let _ = FileExt::unlock(&*self._file);
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TorchDirectoryIdentity {
    volume: u64,
    file: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TorchPendingPublishMarker {
    owner: String,
    directory: TorchDirectoryIdentity,
    source_stage: String,
}

#[allow(unsafe_code)]
fn torch_directory_identity(path: &Path) -> std::io::Result<TorchDirectoryIdentity> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "Torch publication directory identity unavailable",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(TorchDirectoryIdentity {
            volume: metadata.dev(),
            file: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        use std::mem::zeroed;
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        #[repr(C)]
        #[allow(dead_code)]
        struct FileInformation {
            attributes: u32,
            creation_time: [u32; 2],
            access_time: [u32; 2],
            write_time: [u32; 2],
            volume: u32,
            size_high: u32,
            size_low: u32,
            links: u32,
            file_high: u32,
            file_low: u32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            #[link_name = "GetFileInformationByHandle"]
            fn get_file_information_by_handle(
                handle: *mut std::ffi::c_void,
                info: *mut FileInformation,
            ) -> i32;
        }
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;
        const FILE_SHARE_DELETE: u32 = 0x0000_0004;
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(path)?;
        let mut info: FileInformation = unsafe { zeroed() };
        // SAFETY: the directory handle remains live and info has the Win32 layout.
        if unsafe { get_file_information_by_handle(directory.as_raw_handle(), &mut info) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(TorchDirectoryIdentity {
            volume: u64::from(info.volume),
            file: (u64::from(info.file_high) << 32) | u64::from(info.file_low),
        })
    }
}

pub(super) fn write_pending_publish_marker(path: &Path, runtime: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let contents = serde_json::to_vec(&TorchPendingPublishMarker {
        owner: "metadata pending".into(),
        directory: torch_directory_identity(runtime)?,
        source_stage: runtime
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .filter(|name| name.starts_with(".torch-install-"))
            .ok_or_else(|| std::io::Error::other("Torch publication source stage absent"))?
            .to_owned(),
    })
    .map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&contents)?;
    file.sync_all()?;
    #[cfg(unix)]
    std::fs::File::open(
        path.parent()
            .ok_or_else(|| std::io::Error::other("Marker parent absent"))?,
    )?
    .sync_all()?;
    Ok(())
}

/// The sibling marker is created before child admission and survives a failed
/// TempDir removal on Windows, where an open wheel or DLL can lock a stage.
pub(super) struct TorchPendingStage {
    directory: tempfile::TempDir,
    marker: PathBuf,
    // Drop last, after stage and marker cleanup (including TempDir's Drop).
    _lock: TorchVersionsLock,
}

impl TorchPendingStage {
    fn new(versions_dir: &Path, tag: &str, lock: TorchVersionsLock) -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix(".torch-install-")
            .tempdir_in(versions_dir)
            .map_err(PumasError::from)?;
        let name = directory
            .path()
            .file_name()
            .ok_or_else(|| failed("Stage name absent"))?;
        let marker =
            versions_dir.join(format!(".torch-pending-cleanup-{}", name.to_string_lossy()));
        std::fs::write(&marker, tag).map_err(PumasError::from)?;
        Ok(Self {
            directory,
            marker,
            _lock: lock,
        })
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn scratch_path(&self) -> Result<PathBuf> {
        let scratch = self.path().join("temp");
        match std::fs::create_dir(&scratch) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(PumasError::from(error)),
        }
        let stage_root = std::fs::canonicalize(self.path()).map_err(PumasError::from)?;
        let scratch = std::fs::canonicalize(&scratch).map_err(PumasError::from)?;
        if !scratch.starts_with(&stage_root) || scratch == stage_root {
            return Err(failed(
                "Torch installer scratch directory escaped its stage",
            ));
        }
        if !scratch.is_dir() {
            return Err(failed("Torch installer scratch path is not a directory"));
        }
        Ok(scratch)
    }
}

impl Drop for TorchPendingStage {
    fn drop(&mut self) {
        match std::fs::remove_dir_all(self.path()) {
            Ok(()) => {
                let _ = std::fs::remove_file(&self.marker);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let _ = std::fs::remove_file(&self.marker);
            }
            Err(error) => {
                warn!(%error, path = %self.path().display(), "Torch stage retained for pending cleanup")
            }
        }
    }
}

fn spawn_blocking_with_stage<T: Send + 'static>(
    stage: Arc<TorchPendingStage>,
    work: impl FnOnce() -> T + Send + 'static,
) -> tokio::task::JoinHandle<T> {
    tokio::task::spawn_blocking(move || {
        let _stage = stage;
        work()
    })
}

pub(crate) fn retry_pending_torch_cleanup(
    versions_dir: &Path,
    metadata_manager: &MetadataManager,
) -> std::io::Result<()> {
    if !versions_dir.exists() {
        return Ok(());
    }
    let lock = TorchVersionsLock::try_acquire(versions_dir)?;
    retry_pending_torch_cleanup_locked(versions_dir, metadata_manager, &lock)
}

fn retry_pending_torch_cleanup_locked(
    versions_dir: &Path,
    metadata_manager: &MetadataManager,
    _lock: &TorchVersionsLock,
) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(versions_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let marker = entry.path();
        let Some(tag) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix(".torch-pending-publish-"))
            .map(str::to_owned)
        else {
            continue;
        };
        if stable_torch_tag(&tag).is_none() || !entry.file_type()?.is_file() {
            continue;
        }
        let marker_bytes = std::fs::read(&marker)?;
        let durable_owner = if marker_bytes == TORCH_PUBLISHING_MARKER {
            None // Legacy marker: retain the inner marker as ownership proof.
        } else {
            let Ok(record) = serde_json::from_slice::<TorchPendingPublishMarker>(&marker_bytes)
            else {
                continue;
            };
            if record.owner != "metadata pending"
                || !record.source_stage.starts_with(".torch-install-")
                || !record
                    .source_stage
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
            {
                continue;
            }
            Some((record.directory, record.source_stage))
        };
        let destination = versions_dir.join(&tag);
        let registered = metadata_manager
            .get_installed_version(&tag, Some(AppId::Torch))
            .map_err(std::io::Error::other)?
            .is_some();
        if registered {
            let inner = destination.join(".pumas-publishing");
            match std::fs::remove_file(&inner) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    warn!(%error, path = %inner.display(), "Registered Torch marker cleanup remains pending");
                    continue;
                }
            }
            std::fs::remove_file(marker)?;
            continue;
        }
        // The original directory still at its source means the rename never
        // published it. Release the pending marker before stage cleanup can
        // delete the source and allow native file IDs to be reused.
        if let Some((expected, source_stage)) = &durable_owner {
            let source = versions_dir.join(source_stage).join("runtime");
            if torch_directory_identity(&source).ok() == Some(*expected) {
                std::fs::remove_file(marker)?;
                continue;
            }
        }
        if destination.exists() {
            if let Some((expected, _)) = &durable_owner {
                if torch_directory_identity(&destination).ok() != Some(*expected) {
                    warn!(path = %destination.display(), "Torch pending publication does not own destination identity");
                    continue;
                }
            }
            match std::fs::read(destination.join(".pumas-publishing")) {
                Ok(contents) if contents == TORCH_PUBLISHING_MARKER => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if durable_owner.is_none() {
                        warn!(path = %destination.display(), "Torch publication ownership marker absent; pending cleanup retained");
                        continue;
                    }
                }
                Ok(_) => continue,
                Err(error) => return Err(error),
            }
            if let Err(error) = std::fs::remove_dir_all(&destination) {
                warn!(%error, path = %destination.display(), "Unregistered Torch publication remains pending cleanup");
                continue;
            }
        }
        std::fs::remove_file(marker)?;
    }
    let entries = match std::fs::read_dir(versions_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let marker = entry.path();
        let Some(stage_name) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix(".torch-pending-cleanup-"))
            .map(str::to_owned)
        else {
            continue;
        };
        if !stage_name.starts_with(".torch-install-")
            || !stage_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
            || !entry.file_type()?.is_file()
        {
            continue;
        }
        let tag = std::fs::read_to_string(&marker)?;
        if stable_torch_tag(&tag).is_none() {
            continue;
        }
        let stage = versions_dir.join(&stage_name);
        let quarantine = versions_dir.join(format!(".torch-quarantine-{stage_name}"));
        if stage.exists() && !quarantine.exists() {
            if let Err(error) =
                pumas_library::platform::filesystem::rename_directory_noreplace(&stage, &quarantine)
            {
                warn!(%error, path = %stage.display(), "Locked Torch stage remains pending cleanup");
                continue;
            }
        }
        if quarantine.exists() {
            if let Err(error) = std::fs::remove_dir_all(&quarantine) {
                warn!(%error, path = %quarantine.display(), "Torch quarantine remains pending cleanup");
                continue;
            }
        }
        if !stage.exists() && !quarantine.exists() {
            std::fs::remove_file(marker)?;
        }
    }
    Ok(())
}

struct TorchOrphanQuarantine {
    tag: String,
    timestamp_ms: u64,
    path: PathBuf,
}

#[cfg(test)]
pub(crate) struct TorchPublicationPause {
    pub(crate) reached: tokio::sync::Notify,
    pub(crate) resume: tokio::sync::Semaphore,
}

#[cfg(test)]
impl TorchPublicationPause {
    pub(crate) fn new() -> Self {
        Self {
            reached: tokio::sync::Notify::new(),
            resume: tokio::sync::Semaphore::new(0),
        }
    }
}

#[cfg(test)]
#[path = "torch_upstream_contract_tests.rs"]
mod torch_upstream_contract_tests;

#[cfg(test)]
mod cross_process_recovery_tests {
    use super::*;
    use fs2::FileExt;

    #[tokio::test(start_paused = true)]
    async fn mutation_lock_waits_for_short_handoff() {
        let root = tempfile::tempdir().unwrap();
        let owner = TorchVersionsLock::try_acquire(root.path()).unwrap();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            drop(owner);
        });

        let acquired = TorchVersionsLock::acquire_for_mutation(root.path())
            .await
            .unwrap();
        drop(acquired);
    }

    #[tokio::test(start_paused = true)]
    async fn mutation_lock_rejects_owner_past_bound() {
        let root = tempfile::tempdir().unwrap();
        let _owner = TorchVersionsLock::try_acquire(root.path()).unwrap();
        let started = tokio::time::Instant::now();

        let error = TorchVersionsLock::acquire_for_mutation(root.path())
            .await
            .err()
            .unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert!(started.elapsed() >= Duration::from_millis(500));
    }

    #[tokio::test(start_paused = true)]
    async fn mutation_lock_returns_non_contention_errors_without_waiting() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".torch-versions.lock")).unwrap();
        let started = tokio::time::Instant::now();

        let error = TorchVersionsLock::acquire_for_mutation(root.path())
            .await
            .err()
            .unwrap();
        assert_ne!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(started.elapsed(), Duration::ZERO);
    }

    #[test]
    fn only_fs2_contention_is_normalized_to_would_block() {
        let raw_busy = fs2::lock_contended_error().raw_os_error().unwrap();
        let busy = normalize_torch_lock_error(std::io::Error::from_raw_os_error(raw_busy));
        assert_eq!(busy.kind(), std::io::ErrorKind::WouldBlock);

        let unrelated_code = raw_busy + 1;
        let unrelated =
            normalize_torch_lock_error(std::io::Error::from_raw_os_error(unrelated_code));
        assert_eq!(unrelated.raw_os_error(), Some(unrelated_code));
        let synthetic = normalize_torch_lock_error(std::io::Error::other("other failure"));
        assert_eq!(synthetic.kind(), std::io::ErrorKind::Other);
    }

    #[test]
    fn pending_stage_carries_the_lock_through_its_cleanup_lease() {
        let root = tempfile::tempdir().unwrap();
        let versions = root.path();
        let owner = TorchVersionsLock::try_acquire(versions).unwrap();
        let stage = TorchPendingStage::new(versions, "v2.9.1", owner).unwrap();
        std::fs::write(stage.path().join("payload"), b"active").unwrap();
        let marker = stage.marker.clone();
        let stage_path = stage.path().to_owned();
        let metadata = MetadataManager::new(versions);

        assert_eq!(
            retry_pending_torch_cleanup(versions, &metadata)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert!(stage_path.join("payload").exists());
        assert!(marker.exists());
        drop(stage);
        retry_pending_torch_cleanup(versions, &metadata).unwrap();
        assert!(!stage_path.exists() && !marker.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runtime_command_uses_stage_scratch_and_preserves_other_environment() {
        let root = tempfile::tempdir().unwrap();
        let launcher_root = root.path().join("launcher with spaces");
        let metadata = Arc::new(MetadataManager::new(&launcher_root));
        metadata.ensure_directories().unwrap();
        let tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
            launcher_root.join("launcher-data/cache"),
        )));
        let installer = VersionInstaller::new(
            launcher_root,
            AppId::Torch,
            metadata,
            tracker,
            Arc::new(AtomicBool::new(false)),
        );
        let versions = installer.versions_dir();
        std::fs::create_dir_all(&versions).unwrap();
        let stage = Arc::new(
            TorchPendingStage::new(
                &versions,
                "v2.9.1",
                TorchVersionsLock::try_acquire(&versions).unwrap(),
            )
            .unwrap(),
        );
        let expected_scratch = std::fs::canonicalize(stage.path()).unwrap().join("temp");
        let log = root.path().join("child.log");
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "printf '%s\\n' \"$TMPDIR\" \"$TMP\" \"$TEMP\" \"$TORCH_UNRELATED_ENV\"",
            ])
            .env("TORCH_UNRELATED_ENV", "retained value");
        let (progress_tx, _progress_rx) = mpsc::channel(4);

        installer
            .run_runtime_command(
                command,
                &log,
                "Testing scratch",
                &progress_tx,
                Some(stage.clone()),
            )
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&log)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            vec![
                expected_scratch.to_str().unwrap(),
                expected_scratch.to_str().unwrap(),
                expected_scratch.to_str().unwrap(),
                "retained value",
            ]
        );
        assert!(expected_scratch.is_dir());
        drop(stage);
        assert!(!expected_scratch.exists());

        let stage = Arc::new(
            TorchPendingStage::new(
                &versions,
                "v2.9.1",
                TorchVersionsLock::try_acquire(&versions).unwrap(),
            )
            .unwrap(),
        );
        std::fs::write(stage.path().join("temp"), b"not a directory").unwrap();
        let sentinel = root.path().join("child-ran");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "touch \"$1\"", "sh"]).arg(&sentinel);
        assert!(installer
            .run_runtime_command(
                command,
                &log,
                "Testing invalid scratch",
                &progress_tx,
                Some(stage)
            )
            .await
            .is_err());
        assert!(!sentinel.exists());
    }

    #[cfg(unix)]
    #[test]
    fn stage_scratch_rejects_a_link_outside_the_stage() {
        let root = tempfile::tempdir().unwrap();
        let owner = TorchVersionsLock::try_acquire(root.path()).unwrap();
        let stage = TorchPendingStage::new(root.path(), "v2.9.1", owner).unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, stage.path().join("temp")).unwrap();

        assert!(stage.scratch_path().is_err());

        std::fs::remove_file(stage.path().join("temp")).unwrap();
        let missing_outside = root.path().join("missing-outside");
        std::os::unix::fs::symlink(&missing_outside, stage.path().join("temp")).unwrap();
        assert!(stage.scratch_path().is_err());
        assert!(!missing_outside.exists());
    }

    #[tokio::test]
    async fn cancelled_staging_waiter_cannot_release_a_running_blocking_stage_task() {
        let root = tempfile::tempdir().unwrap();
        let versions = root.path();
        let owner = TorchVersionsLock::try_acquire(versions).unwrap();
        let stage = Arc::new(TorchPendingStage::new(versions, "v2.9.1", owner).unwrap());
        let stage_path = stage.path().to_owned();
        let runtime = stage_path.join("runtime");
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = spawn_blocking_with_stage(stage.clone(), move || {
            write_embedded_torch_runtime(&runtime).unwrap();
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        started_rx.await.unwrap();
        task.abort();
        drop(stage);

        let metadata = MetadataManager::new(versions);
        assert!(retry_pending_torch_cleanup(versions, &metadata).is_err());
        assert!(stage_path.join("runtime/runtime.json").exists());
        release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match retry_pending_torch_cleanup(versions, &metadata) {
                    Ok(()) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Err(error) => panic!("Torch recovery failed after staging: {error}"),
                }
            }
        })
        .await
        .unwrap();
        assert!(!stage_path.exists());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn finalization_uses_retained_python_version_without_running_runtime_python() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let metadata = Arc::new(MetadataManager::new(root.path()));
        metadata.ensure_directories().unwrap();
        let tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
            root.path().join("launcher-data/cache"),
        )));
        let installer = VersionInstaller::new(
            root.path().to_owned(),
            AppId::Torch,
            metadata.clone(),
            tracker,
            Arc::new(AtomicBool::new(false)),
        );
        let versions = installer.versions_dir();
        std::fs::create_dir_all(&versions).unwrap();
        let lease = Arc::new(
            TorchPendingStage::new(
                &versions,
                "v2.9.1",
                TorchVersionsLock::try_acquire(&versions).unwrap(),
            )
            .unwrap(),
        );
        let destination = versions.join("v2.9.1");
        let python = destination.join("venv/bin/python");
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        let sentinel = root.path().join("python-was-run");
        std::fs::write(
            &python,
            format!(
                "#!/bin/sh\ntouch '{}'\necho Python 0.0.0\n",
                sentinel.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
        let release = GitHubRelease {
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
        };
        let (progress_tx, _progress_rx) = mpsc::channel(4);
        installer
            .finalize_installation(
                "v2.9.1",
                &release,
                &destination,
                &progress_tx,
                lease,
                Some("Python 3.12.9".into()),
            )
            .await
            .unwrap();
        assert!(!sentinel.exists());
        let installed = metadata
            .get_installed_version("v2.9.1", Some(AppId::Torch))
            .unwrap()
            .unwrap();
        assert_eq!(installed.python_version.as_deref(), Some("Python 3.12.9"));
    }

    #[test]
    fn active_owner_keeps_stage_and_publication_until_its_lock_is_released() {
        let root = tempfile::tempdir().unwrap();
        let versions = root.path();
        let lock_path = versions.join(".torch-versions.lock");
        let owner = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();
        owner.try_lock_exclusive().unwrap();

        let stage = versions.join(".torch-install-owned");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("payload"), b"active stage").unwrap();
        let stage_marker = versions.join(".torch-pending-cleanup-.torch-install-owned");
        std::fs::write(&stage_marker, "v2.9.1").unwrap();

        let destination = versions.join("v2.9.1");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(
            destination.join(".pumas-publishing"),
            TORCH_PUBLISHING_MARKER,
        )
        .unwrap();
        std::fs::write(destination.join("payload"), b"active publication").unwrap();
        let publish_marker = versions.join(".torch-pending-publish-v2.9.1");
        std::fs::write(&publish_marker, TORCH_PUBLISHING_MARKER).unwrap();

        // A second opener models startup in another backend sharing this root.
        let observer = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        assert!(observer.try_lock_exclusive().is_err());
        let metadata = MetadataManager::new(versions);
        let busy = retry_pending_torch_cleanup(versions, &metadata);
        assert_eq!(busy.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
        assert!(stage.join("payload").exists());
        assert!(destination.join("payload").exists());
        assert!(stage_marker.exists() && publish_marker.exists());

        drop(observer);
        owner.unlock().unwrap();
        drop(owner);
        retry_pending_torch_cleanup(versions, &metadata).unwrap();
        assert!(!stage.exists() && !destination.exists());
        assert!(!stage_marker.exists() && !publish_marker.exists());
        assert!(lock_path.exists());
    }
}

#[cfg(all(test, windows))]
mod windows_cleanup_tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn locked_stage_stays_owned_and_retries_after_unlock() {
        let root = tempfile::tempdir().unwrap();
        let versions = root.path();
        let stage = versions.join(".torch-install-owned");
        std::fs::create_dir(&stage).unwrap();
        let payload = stage.join("locked.bin");
        std::fs::write(&payload, b"payload").unwrap();
        let marker = versions.join(".torch-pending-cleanup-.torch-install-owned");
        std::fs::write(&marker, "v2.9.1").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&payload)
            .unwrap();
        let metadata = MetadataManager::new(versions);
        retry_pending_torch_cleanup(versions, &metadata).unwrap();
        assert!(marker.exists());
        assert!(
            stage.exists()
                || versions
                    .join(".torch-quarantine-.torch-install-owned")
                    .exists()
        );
        drop(held);
        retry_pending_torch_cleanup(versions, &metadata).unwrap();
        assert!(!stage.exists());
        assert!(!marker.exists());
    }

    #[test]
    fn locked_unregistered_publication_retries_after_unlock() {
        let root = tempfile::tempdir().unwrap();
        let versions = root.path();
        let stage = versions.join(".torch-install-owned");
        let runtime = stage.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(runtime.join(".pumas-publishing"), TORCH_PUBLISHING_MARKER).unwrap();
        let payload = runtime.join("locked.bin");
        std::fs::write(&payload, b"payload").unwrap();
        let marker = versions.join(".torch-pending-publish-v2.9.1");
        write_pending_publish_marker(&marker, &runtime).unwrap();
        let destination = versions.join("v2.9.1");
        std::fs::rename(&runtime, &destination).unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(destination.join("locked.bin"))
            .unwrap();
        let metadata = MetadataManager::new(versions);
        retry_pending_torch_cleanup(versions, &metadata).unwrap();
        assert!(marker.exists() && destination.exists());
        drop(held);
        retry_pending_torch_cleanup(versions, &metadata).unwrap();
        assert!(!marker.exists() && !destination.exists());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod publication_identity_tests {
    use super::*;

    #[test]
    fn durable_identity_reclaims_partial_publication_after_inner_marker_loss() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join(".torch-install-owned");
        let runtime = stage.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(runtime.join(".pumas-publishing"), TORCH_PUBLISHING_MARKER).unwrap();
        std::fs::write(runtime.join("payload"), b"owned").unwrap();
        let marker = root.path().join(".torch-pending-publish-v2.9.1");
        write_pending_publish_marker(&marker, &runtime).unwrap();
        let destination = root.path().join("v2.9.1");
        std::fs::rename(&runtime, &destination).unwrap();
        std::fs::remove_file(destination.join(".pumas-publishing")).unwrap();
        let metadata = MetadataManager::new(root.path());
        retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
        assert!(!marker.exists() && !destination.exists());
    }

    #[test]
    fn pre_rename_marker_cannot_delete_raced_unrelated_destination() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join(".torch-install-owned");
        let runtime = stage.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(
            root.path()
                .join(".torch-pending-cleanup-.torch-install-owned"),
            "v2.9.1",
        )
        .unwrap();
        let marker = root.path().join(".torch-pending-publish-v2.9.1");
        write_pending_publish_marker(&marker, &runtime).unwrap();
        let unrelated = root.path().join("v2.9.1");
        std::fs::create_dir(&unrelated).unwrap();
        std::fs::write(unrelated.join("payload"), b"unrelated").unwrap();
        let metadata = MetadataManager::new(root.path());
        retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
        assert_eq!(
            std::fs::read(unrelated.join("payload")).unwrap(),
            b"unrelated"
        );
        assert!(!marker.exists());
        assert!(!stage.exists());
        assert!(!root
            .path()
            .join(".torch-pending-cleanup-.torch-install-owned")
            .exists());
    }
}

pub(crate) struct TorchRuntimeRecipe {
    #[cfg(test)]
    pub(crate) release_tag: &'static str,
    pub(crate) recipe_id: &'static str,
    pub(crate) torch_version: &'static str,
    pub(crate) torch_wheel_url: &'static str,
    pub(crate) torch_wheel_sha256: &'static str,
    pub(crate) torchvision_wheel_url: &'static str,
    pub(crate) torchvision_wheel_sha256: &'static str,
}

const TORCH_291: TorchRuntimeRecipe = TorchRuntimeRecipe {
    #[cfg(test)]
    release_tag: "v2.9.1",
    recipe_id: "torch-upstream-2.9.1-r1",
    torch_version: "2.9.1+cu130",
    torch_wheel_url: "https://download-r2.pytorch.org/whl/cu130/torch-2.9.1%2Bcu130-cp312-cp312-manylinux_2_28_x86_64.whl",
    torch_wheel_sha256: "e70e1b18881e6b3c1ce402d0a989da39f956a3a057526e03c354df23d704ce9b",
    torchvision_wheel_url: "https://download-r2.pytorch.org/whl/cu130/torchvision-0.24.1%2Bcu130-cp312-cp312-manylinux_2_28_x86_64.whl",
    torchvision_wheel_sha256: "6939dd403cc28ab0a46f53e6c86e2e852cf65771c1b0ddd09c44c541a1cdbad9",
};

pub(crate) fn torch_recipe_for_tag(tag: &str) -> Option<&'static TorchRuntimeRecipe> {
    match tag {
        "v2.9.1" => Some(&TORCH_291),
        _ => None,
    }
}

pub(crate) fn is_torch_runtime_release(release: &GitHubRelease) -> bool {
    !release.prerelease && stable_torch_tag(&release.tag_name).is_some()
}

fn stable_torch_tag(tag: &str) -> Option<&str> {
    let version = tag.strip_prefix('v')?;
    let parts: Vec<_> = version.split('.').collect();
    (parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit())))
    .then_some(version)
}

fn list_owned_torch_orphan_quarantines(
    versions_dir: &Path,
) -> std::io::Result<Vec<TorchOrphanQuarantine>> {
    let entries = match std::fs::read_dir(versions_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut quarantines = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(suffix) = name
            .to_str()
            .and_then(|name| name.strip_prefix(".torch-orphan-"))
        else {
            continue;
        };
        let Some((tag, timestamp)) = suffix.rsplit_once('-') else {
            continue;
        };
        let (Some(_), Ok(timestamp_ms)) = (stable_torch_tag(tag), timestamp.parse::<u64>()) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if !matches!(
            std::fs::read(path.join(".pumas-publishing")),
            Ok(contents) if contents.as_slice() == TORCH_PUBLISHING_MARKER
        ) {
            continue;
        }
        quarantines.push(TorchOrphanQuarantine {
            tag: tag.to_owned(),
            timestamp_ms,
            path,
        });
    }
    Ok(quarantines)
}

#[cfg(all(test, target_os = "linux"))]
pub(super) fn prune_torch_orphan_quarantines(
    versions_dir: &Path,
    max_keep: usize,
    only_tag: Option<&str>,
) -> std::io::Result<()> {
    if !versions_dir.exists() {
        return Ok(());
    }
    let lock = TorchVersionsLock::try_acquire(versions_dir)?;
    prune_torch_orphan_quarantines_locked(versions_dir, max_keep, only_tag, &lock)
}

fn prune_torch_orphan_quarantines_locked(
    versions_dir: &Path,
    max_keep: usize,
    only_tag: Option<&str>,
    _lock: &TorchVersionsLock,
) -> std::io::Result<()> {
    let mut quarantines = list_owned_torch_orphan_quarantines(versions_dir)?;
    quarantines.retain(|quarantine| only_tag.is_none_or(|tag| quarantine.tag == tag));
    quarantines.sort_by(|left, right| right.timestamp_ms.cmp(&left.timestamp_ms));
    for quarantine in quarantines.into_iter().skip(max_keep) {
        if let Err(error) = std::fs::remove_dir_all(quarantine.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error);
            }
        }
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
pub(super) fn schedule_torch_orphan_prune(
    cleanup: &TorchCleanupTasks,
    versions_dir: &Path,
    max_keep: usize,
    only_tag: Option<String>,
    phase: &'static str,
) {
    let versions_dir = versions_dir.to_owned();
    cleanup.schedule(move || {
        if let Err(error) =
            prune_torch_orphan_quarantines(&versions_dir, max_keep, only_tag.as_deref())
        {
            warn!(%error, phase, "Torch orphan cleanup failed");
        }
    });
}

/// Materialize the qualified sidecar and lock from bytes embedded in the app binary.
pub(crate) fn write_embedded_torch_runtime(destination: &Path) -> Result<()> {
    let lock = include_str!("../../../../../../torch-server/runtime/requirements.lock");
    for (name, url, hash) in [
        (
            "torch",
            TORCH_291.torch_wheel_url,
            TORCH_291.torch_wheel_sha256,
        ),
        (
            "torchvision",
            TORCH_291.torchvision_wheel_url,
            TORCH_291.torchvision_wheel_sha256,
        ),
    ] {
        if !lock.contains(&format!("{name} @ {url}"))
            || !lock.contains(&format!("--hash=sha256:{hash}"))
        {
            return Err(failed(format!(
                "Embedded lock disagrees with the qualified {name} wheel"
            )));
        }
    }
    if !include_str!("../../../../../../torch-server/validate_runtime.py").contains(&format!(
        "torch.__version__ != \"{}\"",
        TORCH_291.torch_version
    )) {
        return Err(failed(
            "Embedded runtime validator disagrees with the qualified Torch version",
        ));
    }
    std::fs::create_dir_all(destination).map_err(PumasError::from)?;
    for (name, contents) in [
        ("LICENSE", include_str!("../../../../../../LICENSE")),
        (
            "serve.py",
            include_str!("../../../../../../torch-server/serve.py"),
        ),
        (
            "validate_runtime.py",
            include_str!("../../../../../../torch-server/validate_runtime.py"),
        ),
        (
            "resolve_runtime.py",
            include_str!("../../../../../../torch-server/resolve_runtime.py"),
        ),
        (
            "probe_runtime.py",
            include_str!("../../../../../../torch-server/probe_runtime.py"),
        ),
        (
            "nunchaku_compat.py",
            include_str!("../../../../../../torch-server/nunchaku_compat.py"),
        ),
        (
            "control_api.py",
            include_str!("../../../../../../torch-server/control_api.py"),
        ),
        (
            "device_manager.py",
            include_str!("../../../../../../torch-server/device_manager.py"),
        ),
        (
            "diffusion.py",
            include_str!("../../../../../../torch-server/diffusion.py"),
        ),
        (
            "flux2.py",
            include_str!("../../../../../../torch-server/flux2.py"),
        ),
        (
            "image_api.py",
            include_str!("../../../../../../torch-server/image_api.py"),
        ),
        (
            "model_manager.py",
            include_str!("../../../../../../torch-server/model_manager.py"),
        ),
        (
            "openai_api.py",
            include_str!("../../../../../../torch-server/openai_api.py"),
        ),
        (
            "validation.py",
            include_str!("../../../../../../torch-server/validation.py"),
        ),
        (
            "loaders/__init__.py",
            include_str!("../../../../../../torch-server/loaders/__init__.py"),
        ),
        (
            "loaders/dllm_loader.py",
            include_str!("../../../../../../torch-server/loaders/dllm_loader.py"),
        ),
        (
            "loaders/safetensors_loader.py",
            include_str!("../../../../../../torch-server/loaders/safetensors_loader.py"),
        ),
        (
            "loaders/sherry_loader.py",
            include_str!("../../../../../../torch-server/loaders/sherry_loader.py"),
        ),
        ("requirements.txt", lock),
    ] {
        let path = destination.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(PumasError::from)?;
        }
        std::fs::write(path, contents).map_err(PumasError::from)?;
    }
    let recipe = serde_json::json!({
        "recipe_id": TORCH_291.recipe_id,
        "protocol": SUPPORTED_TORCH_PROTOCOL,
        "capabilities": [TORCH_IMAGE_GENERATION_CAPABILITY],
        "python": "3.12",
        "platform": "linux-x86_64",
    });
    std::fs::write(
        destination.join("runtime.json"),
        serde_json::to_vec_pretty(&recipe)
            .map_err(|e| failed(format!("Cannot serialize Torch recipe: {e}")))?,
    )
    .map_err(PumasError::from)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeRecipe {
    recipe_id: String,
    protocol: u32,
    capabilities: Vec<String>,
    python: String,
    platform: String,
}

fn failed(message: impl Into<String>) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

fn managed_python_identity_record(
    managed_python: &super::super::managed_python::ManagedPythonIdentity,
    interpreter_hash: &str,
) -> serde_json::Value {
    serde_json::json!({
        "provider": {
            "name": "uv",
            "version": managed_python.uv_version,
            "archiveSha256": managed_python.uv_archive_sha256,
        },
        "distribution": {
            "implementation": "CPython",
            "version": managed_python.version,
            "catalogKey": managed_python.catalog_key,
            "sourceUrl": managed_python.source_url,
            "targetTriple": managed_python.target_triple,
        },
        "executable": {
            "path": managed_python.executable,
            "sha256": interpreter_hash,
        },
    })
}

fn managed_python_record(plan: &super::TorchInstallPlan) -> serde_json::Value {
    managed_python_identity_record(&plan.managed_python, &plan.interpreter_hash)
}

fn record_managed_python(runtime: &Path, plan: &super::TorchInstallPlan) -> Result<()> {
    let path = runtime.join("runtime.json");
    let bytes = std::fs::read(&path).map_err(PumasError::from)?;
    let mut recipe: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| failed(format!("Invalid Torch runtime recipe: {error}")))?;
    if !recipe.is_object() {
        return Err(failed("Torch runtime recipe is not an object"));
    }
    recipe["managed_python"] = managed_python_record(plan);
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&recipe).map_err(|error| failed(error.to_string()))?,
    )
    .map_err(PumasError::from)
}

fn cuda_build_for_driver(version: (u32, u32, u32)) -> Option<&'static str> {
    if version >= (580, 0, 0) {
        Some("cu134")
    } else if version >= (525, 60, 13) {
        Some("cu129")
    } else if version >= (450, 80, 2) {
        Some("cu118")
    } else if version >= (440, 33, 0) {
        Some("cu102")
    } else if version >= (418, 39, 0) {
        Some("cu101")
    } else if version >= (410, 48, 0) {
        Some("cu100")
    } else {
        None
    }
}

fn cuda_build_candidates_for_driver(version: (u32, u32, u32)) -> Vec<&'static str> {
    let Some(maximum) = cuda_build_for_driver(version)
        .and_then(|build| build.strip_prefix("cu"))
        .and_then(|channel| channel.parse::<u32>().ok())
    else {
        return Vec::new();
    };
    // BUILDS is a fixed, ascending channel list. The maximum above follows
    // the same Linux NVIDIA driver floors used for release recommendations.
    super::super::torch_preview::BUILDS
        .iter()
        .rev()
        .copied()
        .filter(|build| {
            build
                .strip_prefix("cu")
                .and_then(|channel| channel.parse::<u32>().ok())
                .is_some_and(|channel| (100..=maximum).contains(&channel))
        })
        .take(4)
        .collect()
}

fn automatic_torch_builds_for_driver(version: (u32, u32, u32)) -> Vec<&'static str> {
    let mut builds = cuda_build_candidates_for_driver(version);
    builds.push("cpu");
    builds
}

fn retry_torch_python_candidate(exit_code: Option<i32>, automatic: bool) -> bool {
    automatic && matches!(exit_code, Some(2 | 4))
}

fn retry_direct_torch_attempt(
    exit_code: Option<i32>,
    python_auto: bool,
    cpu_fallback: bool,
) -> bool {
    retry_torch_python_candidate(exit_code, python_auto || cpu_fallback)
}

async fn automatic_torch_builds() -> Vec<&'static str> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return vec!["cpu"];
    }
    let mut command = Command::new("nvidia-smi");
    command
        .kill_on_drop(true)
        .args(["--query-gpu=driver_version", "--format=csv,noheader"]);
    let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(3), command.output()).await
    else {
        return vec!["cpu"];
    };
    if !output.status.success() {
        return vec!["cpu"];
    }
    let Ok(text) = std::str::from_utf8(&output.stdout) else {
        return vec!["cpu"];
    };
    let mut versions = text.lines().map(|line| {
        let mut parts = line.trim().split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        parts.next().is_none().then_some((major, minor, patch))
    });
    let Some(Some(mut lowest)) = versions.next() else {
        return vec!["cpu"];
    };
    for version in versions {
        let Some(version) = version else {
            return vec!["cpu"];
        };
        lowest = lowest.min(version);
    }
    automatic_torch_builds_for_driver(lowest)
}

#[derive(Deserialize)]
struct DirectTorchResolution {
    release: String,
    torch: String,
    build: String,
    python: String,
    interpreter: String,
    implementation: String,
    platform: String,
    machine: String,
    adapter: String,
    artifacts: Vec<crate::version_manager::TorchArtifact>,
}

fn validate_direct_torch_report(
    resolution: &DirectTorchResolution,
    report: &serde_json::Value,
    requirements: &str,
    version: &str,
    build: &str,
    minor: &str,
    adapter: &str,
    python: &Path,
) -> Result<()> {
    let expected_torch = if cfg!(target_os = "macos") && build == "cpu" {
        version.to_owned()
    } else {
        format!("{version}+{build}")
    };
    if resolution.release != version
        || resolution.torch != expected_torch
        || resolution.build != build
        || resolution.python != minor
        || resolution.adapter != adapter
        || resolution.implementation != "cpython"
        || Path::new(&resolution.interpreter) != python
        || resolution.artifacts.is_empty()
        || !match std::env::consts::OS {
            "linux" => resolution.machine == "x86_64" && resolution.platform.starts_with("Linux-"),
            "windows" => {
                resolution.machine == "AMD64" && resolution.platform.starts_with("Windows-")
            }
            "macos" => resolution.machine == "arm64" && resolution.platform.starts_with("macOS-"),
            _ => false,
        }
    {
        return Err(failed(
            "Torch installation report differs from the selection or host",
        ));
    }
    let entries = report["install"]
        .as_array()
        .ok_or_else(|| failed("Torch pip installation report has no artifact list"))?;
    if entries.len() != resolution.artifacts.len()
        || requirements.lines().count() != resolution.artifacts.len()
    {
        return Err(failed(
            "Torch report and hash lock have different artifact counts",
        ));
    }
    let mut names = std::collections::HashSet::new();
    let mut torch_count = 0;
    for (index, artifact) in resolution.artifacts.iter().enumerate() {
        if artifact.name.is_empty()
            || !artifact.name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-.".contains(&byte)
            })
            || !names.insert(artifact.name.as_str())
            || artifact.sha256.len() != 64
            || !artifact.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(failed(
                "Torch report contains an invalid distribution or SHA-256",
            ));
        }
        let url = reqwest::Url::parse(&artifact.url)
            .map_err(|_| failed("Torch report contains an invalid artifact URL"))?;
        let trusted_source = url.scheme() == "https"
            && !artifact.url.chars().any(char::is_whitespace)
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(url.port(), None | Some(443))
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path().to_ascii_lowercase().ends_with(".whl")
            && if artifact.name == "torch" || artifact.name == "torchvision" {
                matches!(
                    url.host_str(),
                    Some("download.pytorch.org" | "download-r2.pytorch.org")
                ) && url.path().starts_with(&format!("/whl/{build}/"))
            } else {
                (url.host_str() == Some("files.pythonhosted.org")
                    && url.path().starts_with("/packages/"))
                    || (matches!(
                        url.host_str(),
                        Some("download.pytorch.org" | "download-r2.pytorch.org")
                    ) && url.path().starts_with("/whl/"))
            };
        if !trusted_source {
            return Err(failed("Torch report contains an untrusted wheel source"));
        }
        if artifact.name == "torch" {
            torch_count += 1;
            if artifact.version != expected_torch {
                return Err(failed(
                    "Torch wheel version differs from the selected release",
                ));
            }
        }
        if artifact.name == "torchvision" {
            if adapter == "none"
                || !(artifact.version.ends_with(&format!("+{build}"))
                    || (cfg!(target_os = "macos")
                        && build == "cpu"
                        && artifact.version.split('.').count() == 3
                        && artifact
                            .version
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || byte == b'.')))
            {
                return Err(failed(
                    "Torchvision build differs from selected Torch build",
                ));
            }
        }
        let item = &entries[index];
        let report_name = item["metadata"]["name"]
            .as_str()
            .map(|name| name.to_ascii_lowercase().replace('_', "-"));
        if report_name.as_deref() != Some(artifact.name.as_str())
            || item["metadata"]["version"].as_str() != Some(artifact.version.as_str())
            || item["download_info"]["url"].as_str() != Some(artifact.url.as_str())
            || item["download_info"]["archive_info"]["hashes"]["sha256"].as_str()
                != Some(artifact.sha256.as_str())
            || requirements.lines().nth(index)
                != Some(
                    format!(
                        "{} @ {} --hash=sha256:{}",
                        artifact.name, artifact.url, artifact.sha256
                    )
                    .as_str(),
                )
        {
            return Err(failed("Torch report, resolution, and hash lock disagree"));
        }
    }
    if torch_count != 1 {
        return Err(failed("Torch report must contain one selected Torch wheel"));
    }
    for required in [
        "torch",
        "fastapi",
        "uvicorn",
        "psutil",
        "pillow",
        "safetensors",
    ] {
        if !names.contains(required) {
            return Err(failed("Torch report omitted a required core dependency"));
        }
    }
    if adapter == "flux2" {
        for required in [
            "torchvision",
            "diffusers",
            "transformers",
            "accelerate",
            "peft",
            "sentencepiece",
            "protobuf",
        ] {
            if !names.contains(required) {
                return Err(failed("Torch report omitted an image dependency"));
            }
        }
    }
    Ok(())
}

enum DirectTorchAttempt {
    Installed(PathBuf),
    Retry,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedFilesManifest {
    files: Vec<StagedFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedFile {
    path: String,
    sha256: String,
    size: u64,
}

fn validate_staged_files(target: &Path, manifest: &StagedFilesManifest) -> Result<()> {
    if manifest.files.is_empty() || manifest.files.len() > 200_000 {
        return Err(failed("Torch staged file manifest is empty or oversized"));
    }
    let mut expected = std::collections::HashMap::new();
    for item in &manifest.files {
        let path = Path::new(&item.path);
        if item.path.len() > 1024
            || item.path.contains('\\')
            || path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            || item.sha256.len() != 64
            || !item.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || expected.insert(item.path.as_str(), item).is_some()
        {
            return Err(failed("Torch staged file manifest has an invalid entry"));
        }
    }
    let mut seen = 0;
    for entry in walkdir::WalkDir::new(target).follow_links(false) {
        let entry = entry.map_err(|_| failed("Cannot inspect Torch staged packages"))?;
        if entry.file_type().is_symlink() {
            return Err(failed("Torch staged packages contain a symlink"));
        }
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            return Err(failed("Torch staged packages contain a special file"));
        }
        let relative = entry
            .path()
            .strip_prefix(target)
            .map_err(|_| failed("Torch staged file escaped its target"))?;
        let name = relative
            .to_str()
            .ok_or_else(|| failed("Torch staged file name is invalid"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let item = expected
            .get(name.as_str())
            .ok_or_else(|| failed("Torch staged packages contain an unreported file"))?;
        let mut file = std::fs::File::open(entry.path()).map_err(PumasError::from)?;
        let mut hasher = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            use std::io::Read;
            let count = file.read(&mut buffer).map_err(PumasError::from)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
            size += count as u64;
        }
        if size != item.size || format!("{:x}", hasher.finalize()) != item.sha256 {
            return Err(failed(
                "Torch staged file differs from the validated wheel manifest",
            ));
        }
        seen += 1;
    }
    if seen != expected.len() {
        return Err(failed("Torch staged wheel manifest names missing files"));
    }
    Ok(())
}

fn move_verified_packages(target: &Path, runtime: &Path, python_minor: &str) -> Result<()> {
    #[cfg(windows)]
    let packages = runtime.join("venv").join("Lib").join("site-packages");
    #[cfg(not(windows))]
    let packages = runtime
        .join("venv")
        .join("lib")
        .join(format!("python{python_minor}"))
        .join("site-packages");
    std::fs::create_dir_all(&packages).map_err(PumasError::from)?;
    let entries = std::fs::read_dir(target)
        .map_err(PumasError::from)?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(PumasError::from)?;
    if entries.is_empty() {
        return Err(failed("Torch installer staged no packages"));
    }
    for entry in &entries {
        let file_type = entry.file_type().map_err(PumasError::from)?;
        let destination = packages.join(entry.file_name());
        if file_type.is_symlink() || destination.exists() {
            return Err(failed(
                "Torch package staging contains a link or venv collision",
            ));
        }
    }
    for entry in entries {
        std::fs::rename(entry.path(), packages.join(entry.file_name()))
            .map_err(PumasError::from)?;
    }
    std::fs::remove_dir(target).map_err(PumasError::from)?;
    Ok(())
}

fn validate_and_move_direct_torch_packages(
    resolution: &DirectTorchResolution,
    report: &serde_json::Value,
    requirements: &str,
    version: &str,
    build: &str,
    minor: &str,
    adapter: &str,
    python: &Path,
    target: &Path,
    runtime: &Path,
    manifest: &StagedFilesManifest,
) -> Result<()> {
    validate_direct_torch_report(
        resolution,
        report,
        requirements,
        version,
        build,
        minor,
        adapter,
        python,
    )?;
    validate_staged_files(target, manifest)?;
    move_verified_packages(target, runtime, minor)
}

impl VersionInstaller {
    async fn run_provider_with_cancel<F, T>(&self, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let outcome = tokio::select! {
            biased;
            _ = async {
                loop {
                    if self.cancel_flag.load(Ordering::SeqCst) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            } => None,
            result = operation => Some(result),
        };
        match outcome {
            Some(result) => result,
            None => {
                // The losing provider future has been dropped before draining all
                // registered children, including its newly abandoned uv child.
                self.torch_cleanup.drain_child_slots().await?;
                Err(failed("Installation cancelled by user"))
            }
        }
    }

    async fn stage_selected_torch_runtime(
        &self,
        selection: &super::super::torch_preview::TorchInstallSelection,
        staging: &std::sync::Arc<TorchPendingStage>,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        if selection.adapter != "none" && selection.adapter != "flux2" {
            return Err(failed(
                "This Torch dependency profile requires a resolved plan",
            ));
        }
        let builds: Vec<String> = if selection.build == "auto" {
            automatic_torch_builds()
                .await
                .into_iter()
                .map(str::to_owned)
                .collect()
        } else if super::super::torch_preview::valid_torch_channel(&selection.build) {
            vec![selection.build.clone()]
        } else {
            return Err(failed("Invalid Torch build channel"));
        };
        self.check_cancelled()?;
        let python_root = self.launcher_root.join("launcher-data/managed-python");
        let candidates = self
            .run_provider_with_cancel(
                super::super::torch_alternatives::managed_torch_candidate_minors(
                    &python_root,
                    &self.torch_cleanup,
                ),
            )
            .await?;
        self.check_cancelled()?;
        let minors = if selection.python == "auto" {
            candidates
        } else {
            let selected = selection.python.strip_prefix("python");
            selected
                .filter(|selected| candidates.iter().any(|candidate| candidate == *selected))
                .map(str::to_owned)
                .into_iter()
                .collect()
        };
        if minors.is_empty() {
            return Err(failed(
                "No managed CPython candidate is available for Torch",
            ));
        }
        for build in builds {
            for minor in &minors {
                self.check_cancelled()?;
                let attempt = self
                    .stage_selected_torch_attempt(
                        selection,
                        &build,
                        minor,
                        &python_root,
                        staging,
                        log_path,
                        progress_tx,
                    )
                    .await?;
                match attempt {
                    DirectTorchAttempt::Installed(runtime) => return Ok(runtime),
                    DirectTorchAttempt::Retry => {
                        let runtime = staging.path().join("runtime");
                        spawn_blocking_with_stage(staging.clone(), move || {
                            std::fs::remove_dir_all(runtime)
                        })
                        .await
                        .map_err(|error| {
                            failed(format!("Torch retry cleanup task failed: {error}"))
                        })?
                        .map_err(PumasError::from)?;
                    }
                }
            }
        }
        Err(failed(
            "No managed Python candidate has compatible Torch wheels and dependencies",
        ))
    }

    async fn stage_selected_torch_attempt(
        &self,
        selection: &super::super::torch_preview::TorchInstallSelection,
        build: &str,
        minor: &str,
        python_root: &Path,
        staging: &std::sync::Arc<TorchPendingStage>,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<DirectTorchAttempt> {
        let managed_python = self
            .run_provider_with_cancel(async {
                super::super::managed_python::ensure_managed_torch_interpreter(
                    python_root,
                    &self.torch_cleanup,
                    minor,
                )
                .await
                .map_err(|error| failed(error.message))
            })
            .await?;
        self.check_cancelled()?;
        let interpreter_hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read(&managed_python.executable).map_err(PumasError::from)?)
        );

        let runtime = staging.path().join("runtime");
        let runtime_for_write = runtime.clone();
        spawn_blocking_with_stage(staging.clone(), move || {
            write_embedded_torch_runtime(&runtime_for_write)
        })
        .await
        .map_err(|error| failed(format!("Runtime staging task failed: {error}")))??;
        std::fs::remove_file(runtime.join("requirements.txt")).map_err(PumasError::from)?;
        std::fs::remove_file(runtime.join("validate_runtime.py")).map_err(PumasError::from)?;
        let mut venv = Command::new(&managed_python.executable);
        venv.args(["-I", "-m", "venv"]).arg(runtime.join("venv"));
        self.run_runtime_command(
            venv,
            log_path,
            &format!("Creating managed Python {minor} environment"),
            progress_tx,
            Some(staging.clone()),
        )
        .await?;

        let version = selection
            .tag
            .strip_prefix('v')
            .ok_or_else(|| failed("Invalid Torch release tag"))?;
        let python = pumas_library::platform::paths::venv_python(&runtime);
        let packages = runtime.join("staged-packages");
        let resolver_output = runtime.join("resolver-result");
        let download_progress_path = resolver_output.join("download-progress.json");
        let pip_cache = super::super::torch_workspace::managed_pip_cache_dir(&self.launcher_root)?;
        let mut install = Command::new(&python);
        install
            .arg("-I")
            .arg(runtime.join("resolve_runtime.py"))
            .args(["--install", "--version", version, "--build", build])
            .args(["--adapter", &selection.adapter])
            .arg("--progress-file")
            .arg(&download_progress_path)
            .arg("--target")
            .arg(&packages)
            .arg("--output")
            .arg(&resolver_output)
            .arg("--cache-dir")
            .arg(pip_cache);
        let status = self
            .run_runtime_command_status_with_download_progress(
                install,
                log_path,
                &format!(
                    "Resolving and installing official Torch {build} wheels for Python {minor}"
                ),
                progress_tx,
                Some(staging.clone()),
                Some(&download_progress_path),
            )
            .await?;
        if !status.success() {
            if retry_direct_torch_attempt(
                status.code(),
                selection.python == "auto",
                selection.build == "auto" && build != "cpu",
            ) {
                return Ok(DirectTorchAttempt::Retry);
            }
            return Err(failed(format!(
                "Torch package installation failed ({status}); see installation log"
            )));
        }
        // No staged package code runs until the resolver has validated the
        // installation report and published its lock and resolution manifest.
        let resolution: DirectTorchResolution = serde_json::from_slice(
            &std::fs::read(resolver_output.join("resolution.json")).map_err(PumasError::from)?,
        )
        .map_err(|error| failed(format!("Invalid Torch installation report: {error}")))?;
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(resolver_output.join("pip-resolution.json"))
                .map_err(PumasError::from)?,
        )
        .map_err(|error| failed(format!("Invalid Torch pip report: {error}")))?;
        let requirements = std::fs::read_to_string(resolver_output.join("requirements.txt"))
            .map_err(PumasError::from)?;
        let manifest: StagedFilesManifest = serde_json::from_slice(
            &std::fs::read(resolver_output.join("installed-files.json"))
                .map_err(PumasError::from)?,
        )
        .map_err(|error| failed(format!("Invalid Torch staged file manifest: {error}")))?;
        validate_and_move_direct_torch_packages(
            &resolution,
            &report,
            &requirements,
            version,
            build,
            minor,
            &selection.adapter,
            &python,
            &packages,
            &runtime,
            &manifest,
        )?;
        for name in [
            "requirements.txt",
            "resolution.json",
            "pip-resolution.json",
            "installed-files.json",
        ] {
            std::fs::copy(resolver_output.join(name), runtime.join(name))
                .map_err(PumasError::from)?;
        }
        let recipe = serde_json::json!({
            "recipe_id": format!("upstream-install-{}-{}-{}-{}", selection.tag, build, minor, selection.adapter),
            "protocol": SUPPORTED_TORCH_PROTOCOL,
            "capabilities": [TORCH_IMAGE_GENERATION_CAPABILITY],
            "qualification": "not verified by Pumas",
            "build": build,
            "python": format!("python{minor}"),
            "adapter": selection.adapter,
            "managed_python": managed_python_identity_record(&managed_python, &interpreter_hash),
            "artifacts": resolution.artifacts,
        });
        std::fs::write(
            runtime.join("runtime.json"),
            serde_json::to_vec_pretty(&recipe).map_err(|error| failed(error.to_string()))?,
        )
        .map_err(PumasError::from)?;
        let mut probe = Command::new(&python);
        probe.arg(runtime.join("probe_runtime.py"));
        self.run_runtime_command(
            probe,
            log_path,
            "Checking installed Torch identity and CPU operation",
            progress_tx,
            Some(staging.clone()),
        )
        .await?;
        Ok(DirectTorchAttempt::Installed(runtime))
    }

    async fn stage_resolved_torch_runtime(
        &self,
        plan: &TorchInstallPlan,
        staging: &std::sync::Arc<TorchPendingStage>,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        let runtime = staging.path().join("runtime");
        let runtime_for_write = runtime.clone();
        spawn_blocking_with_stage(staging.clone(), move || {
            write_embedded_torch_runtime(&runtime_for_write)
        })
        .await
        .map_err(|e| failed(format!("Runtime staging task failed: {e}")))??;
        std::fs::remove_file(runtime.join("validate_runtime.py")).map_err(PumasError::from)?;
        let observed_hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read(&plan.interpreter_path).map_err(PumasError::from)?)
        );
        if observed_hash != plan.interpreter_hash
            || std::fs::canonicalize(&plan.interpreter_path).map_err(PumasError::from)?
                != plan.managed_python.executable
        {
            return Err(failed("Selected Python executable changed after preview"));
        }
        let interpreter = &plan.interpreter_path;
        let mut venv = Command::new(interpreter);
        venv.args(["-I", "-m", "venv"]).arg(runtime.join("venv"));
        self.run_runtime_command(
            venv,
            log_path,
            &format!("Creating Python environment with {}", interpreter.display()),
            progress_tx,
            Some(staging.clone()),
        )
        .await?;
        std::fs::write(runtime.join("requirements.txt"), &plan.requirements)
            .map_err(PumasError::from)?;
        std::fs::write(runtime.join("resolution.json"), &plan.resolution)
            .map_err(PumasError::from)?;
        std::fs::write(runtime.join("pip-resolution.json"), &plan.report)
            .map_err(PumasError::from)?;
        let recipe = serde_json::json!({
            "recipe_id": format!("upstream-preview-{}-{}-{}-{}", plan.preview.tag, plan.preview.build, plan.preview.python, plan.preview.adapter),
            "protocol": SUPPORTED_TORCH_PROTOCOL,
            "capabilities": [TORCH_IMAGE_GENERATION_CAPABILITY],
            "qualification": "not verified by Pumas",
            "build": plan.preview.build,
            "python": plan.preview.python,
            "adapter": plan.preview.adapter,
            "managed_python": managed_python_record(plan),
            "artifacts": plan.preview.artifacts,
        });
        std::fs::write(
            runtime.join("runtime.json"),
            serde_json::to_vec_pretty(&recipe).map_err(|e| failed(e.to_string()))?,
        )
        .map_err(PumasError::from)?;
        let python = pumas_library::platform::paths::venv_python(&runtime);
        let pip_cache = super::super::torch_workspace::managed_pip_cache_dir(&self.launcher_root)?;
        let download_progress_path = runtime.join("download-progress.json");
        let mut install = Command::new(&python);
        install
            .arg("-I")
            .arg(runtime.join("resolve_runtime.py"))
            .arg("--_pumas-pip-progress-worker")
            .arg(&download_progress_path)
            .args([
                "--isolated",
                "install",
                "--no-deps",
                "--require-hashes",
                "--only-binary=:all:",
                "--disable-pip-version-check",
                "-r",
            ])
            .arg(runtime.join("requirements.txt"))
            .arg("--cache-dir")
            .arg(pip_cache);
        let status = self
            .run_runtime_command_status_with_download_progress(
                install,
                log_path,
                "Installing resolved wheel artifacts",
                progress_tx,
                Some(staging.clone()),
                Some(&download_progress_path),
            )
            .await?;
        if !status.success() {
            return Err(failed(
                "Installing resolved wheel artifacts failed; see installation log",
            ));
        }
        let mut probe = Command::new(&python);
        probe.arg(runtime.join("probe_runtime.py"));
        self.run_runtime_command(
            probe,
            log_path,
            "Checking installed Torch identity and CPU operation",
            progress_tx,
            Some(staging.clone()),
        )
        .await?;
        Ok(runtime)
    }

    pub(super) async fn install_torch_runtime(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
        input: Option<TorchInstallInput>,
    ) -> Result<()> {
        let _attempt = self
            .torch_attempt_lock
            .try_lock()
            .map_err(|_| failed("Torch installation already active"))?;
        self.torch_control.start();
        let result = self
            .install_torch_runtime_inner(tag, release, progress_tx, input)
            .await;
        self.torch_control.finish();
        result
    }

    async fn install_torch_runtime_inner(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
        input: Option<TorchInstallInput>,
    ) -> Result<()> {
        if !cfg!(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(
                target_os = "windows",
                target_arch = "x86_64",
                target_env = "msvc"
            ),
            all(target_os = "macos", target_arch = "aarch64")
        )) {
            return Err(failed("Managed Torch is unsupported on this platform"));
        }
        if tag != release.tag_name || !is_torch_runtime_release(release) {
            return Err(failed(
                "Unsupported upstream PyTorch release or mismatched tag",
            ));
        }
        let plan = match input.as_ref() {
            Some(TorchInstallInput::Resolved(plan)) => Some(plan),
            _ => None,
        };
        let selection = match input.as_ref() {
            Some(TorchInstallInput::Selection(selection)) => Some(selection),
            _ => None,
        };
        let recipe = if selection.is_none()
            && cfg!(all(target_os = "linux", target_arch = "x86_64"))
            && plan.is_none_or(|p| p.preview.qualification == "qualified")
        {
            torch_recipe_for_tag(tag)
        } else {
            None
        };
        if plan.is_some_and(|p| p.preview.tag != tag)
            || selection.is_some_and(|selection| selection.tag != tag)
        {
            return Err(failed("Torch preview tag mismatch"));
        }
        if !tag
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._".contains(&c))
        {
            return Err(failed("Invalid Torch runtime version tag"));
        }
        let versions_dir = self.versions_dir();
        fs::create_dir_all(&versions_dir)
            .await
            .map_err(PumasError::from)?;
        self.torch_cleanup.drain_residual_child_slots().await?;
        let versions_lock = TorchVersionsLock::acquire_for_mutation(&versions_dir)
            .await
            .map_err(PumasError::from)?;
        retry_pending_torch_cleanup_locked(&versions_dir, &self.metadata_manager, &versions_lock)
            .map_err(PumasError::from)?;
        if let Err(error) = prune_torch_orphan_quarantines_locked(
            &versions_dir,
            MAX_TORCH_ORPHAN_QUARANTINES,
            None,
            &versions_lock,
        ) {
            warn!(%error, "Torch orphan cleanup before install failed");
        }
        let destination = versions_dir.join(tag);
        if path_exists(&destination).await? {
            if path_exists(&destination.join(".pumas-publishing")).await?
                && self
                    .metadata_manager
                    .get_installed_version(tag, Some(AppId::Torch))?
                    .is_none()
            {
                let quarantine = versions_dir.join(format!(
                    ".torch-orphan-{tag}-{}",
                    Utc::now().timestamp_millis()
                ));
                let from = destination.clone();
                let recovery_lease = versions_lock.clone();
                tokio::task::spawn_blocking(move || {
                    let _recovery_lease = recovery_lease;
                    pumas_library::platform::filesystem::rename_directory_noreplace(
                        &from,
                        &quarantine,
                    )
                })
                .await
                .map_err(|e| failed(format!("Orphan recovery task failed: {e}")))?
                .map_err(PumasError::from)?;
                if let Err(error) = prune_torch_orphan_quarantines_locked(
                    &versions_dir,
                    MAX_TORCH_ORPHAN_QUARANTINES,
                    None,
                    &versions_lock,
                ) {
                    warn!(%error, "Torch orphan cleanup after recovery failed");
                }
            } else {
                return Err(failed(
                    "Runtime directory already exists; refusing to replace existing files",
                ));
            }
        }
        if recipe.is_none() && input.is_none() {
            #[cfg(test)]
            if self.torch_stage_override.is_none() {
                return Err(failed(
                    "A retained preview is required for this Torch runtime",
                ));
            }
            #[cfg(not(test))]
            return Err(failed(
                "A retained preview is required for this Torch runtime",
            ));
        }
        // Staging shares the publication filesystem; failed attempts never enter
        // installed-version state. TempDir removes this attempt on every exit.
        let staging =
            std::sync::Arc::new(TorchPendingStage::new(&versions_dir, tag, versions_lock)?);
        let logs = self.logs_dir();
        fs::create_dir_all(&logs).await.map_err(PumasError::from)?;
        let log_path = logs.join(format!(
            "install-{}-{}.log",
            tag,
            Utc::now().timestamp_millis()
        ));
        self.progress_tracker.write().await.start_installation(
            tag,
            None,
            None,
            Some(log_path.to_string_lossy().as_ref()),
        );
        let result = self
            .stage_torch_runtime(
                tag,
                recipe,
                plan,
                selection,
                &staging,
                &log_path,
                &progress_tx,
            )
            .await;
        #[cfg(test)]
        if result.is_ok() && self.torch_stage_override.is_some() {
            if let Some(pause) = &self.torch_stage_pause {
                pause.reached.notify_one();
                pause
                    .resume
                    .acquire()
                    .await
                    .map_err(|e| failed(format!("Staging pause failed: {e}")))?
                    .forget();
            }
        }
        let result = async {
            match result {
                Ok(runtime) => {
                    self.check_cancelled()?;
                    std::fs::write(runtime.join(".pumas-publishing"), TORCH_PUBLISHING_MARKER)
                        .map_err(PumasError::from)?;
                    if !self.torch_control.try_begin_publication() {
                        return Err(failed(
                            "Torch installation was cancelled before publication",
                        ));
                    }
                    #[cfg(test)]
                    if let Some(pause) = &self.torch_publication_pause {
                        pause.reached.notify_one();
                        pause
                            .resume
                            .acquire()
                            .await
                            .map_err(|e| failed(format!("Publication pause failed: {e}")))?
                            .forget();
                    }
                    let pending = versions_dir.join(format!(".torch-pending-publish-{tag}"));
                    write_pending_publish_marker(&pending, &runtime).map_err(PumasError::from)?;
                    let publish_to = destination.clone();
                    let published = spawn_blocking_with_stage(staging.clone(), move || {
                        pumas_library::platform::filesystem::rename_directory_noreplace(
                            &runtime,
                            &publish_to,
                        )
                    })
                    .await
                    .map_err(|error| failed(format!("Runtime publication task failed: {error}")))?;
                    if let Err(error) = published {
                        std::fs::remove_file(&pending).map_err(PumasError::from)?;
                        return Err(PumasError::from(error));
                    }
                    let result = self
                        .finalize_installation(
                            tag,
                            release,
                            &destination,
                            &progress_tx,
                            staging.clone(),
                            plan
                                .map(|plan| format!("Python {}", plan.managed_python.version)),
                        )
                        .await;
                    if result.is_err() {
                        // The durable ownership marker survives a Windows
                        // file lock and is retried at startup and install.
                        let rollback_destination = destination.clone();
                        match spawn_blocking_with_stage(staging.clone(), move || {
                            std::fs::remove_dir_all(rollback_destination)
                        })
                        .await
                        .map_err(|error| failed(format!("Torch rollback task failed: {error}")))?
                        {
                            Ok(()) => std::fs::remove_file(&pending).map_err(PumasError::from)?,
                            Err(error) => warn!(%error, path = %destination.display(), "Unregistered Torch publication retained for cleanup"),
                        }
                    }
                    if result.is_ok() {
                        match std::fs::remove_file(destination.join(".pumas-publishing")) {
                            Ok(()) => std::fs::remove_file(&pending).map_err(PumasError::from)?,
                            Err(error) => warn!(%error, "Installed Torch publication marker could not be removed"),
                        }
                    }
                    result
                }
                Err(error) => Err(error),
            }
        }
        .await;
        let mut tracker = self.progress_tracker.write().await;
        if let Err(error) = &result {
            tracker.set_error(&error.to_string());
        }
        let successful = result.is_ok();
        tracker.complete_installation(successful);
        drop(tracker);
        if successful {
            if let Err(error) =
                prune_torch_orphan_quarantines_locked(&versions_dir, 0, Some(tag), &staging._lock)
            {
                warn!(%error, "Torch orphan cleanup after install failed");
            }
        }
        drop(staging);
        result
    }

    async fn stage_torch_runtime(
        &self,
        _tag: &str,
        recipe_spec: Option<&TorchRuntimeRecipe>,
        plan: Option<&TorchInstallPlan>,
        selection: Option<&super::super::torch_preview::TorchInstallSelection>,
        staging: &std::sync::Arc<TorchPendingStage>,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        #[cfg(test)]
        if let Some(stage) = &self.torch_stage_override {
            return stage(staging.path());
        }
        if let Some(plan) = plan.filter(|p| p.preview.qualification != "qualified") {
            return self
                .stage_resolved_torch_runtime(plan, staging, log_path, progress_tx)
                .await;
        }
        if let Some(selection) = selection {
            return self
                .stage_selected_torch_runtime(selection, staging, log_path, progress_tx)
                .await;
        }
        let recipe_spec = recipe_spec.expect("checked above");
        let runtime = staging.path().join("runtime");
        let runtime_for_write = runtime.clone();
        spawn_blocking_with_stage(staging.clone(), move || {
            write_embedded_torch_runtime(&runtime_for_write)
        })
        .await
        .map_err(|e| failed(format!("Runtime staging task failed: {e}")))??;
        self.check_cancelled()?;
        let recipe: RuntimeRecipe = serde_json::from_slice(
            &fs::read(runtime.join("runtime.json"))
                .await
                .map_err(PumasError::from)?,
        )
        .map_err(|e| failed(format!("Invalid runtime recipe: {e}")))?;
        if recipe.recipe_id != recipe_spec.recipe_id {
            return Err(failed("Embedded Torch recipe identity mismatch"));
        }
        if recipe.protocol != SUPPORTED_TORCH_PROTOCOL {
            return Err(failed(format!(
                "Runtime recipe protocol {} does not match required protocol {SUPPORTED_TORCH_PROTOCOL}",
                recipe.protocol
            )));
        }
        if !recipe
            .capabilities
            .iter()
            .any(|capability| capability == TORCH_IMAGE_GENERATION_CAPABILITY)
        {
            return Err(failed(format!(
                "Runtime recipe is missing required capability {TORCH_IMAGE_GENERATION_CAPABILITY}"
            )));
        }
        if recipe.python != "3.12" || recipe.platform != "linux-x86_64" {
            return Err(failed(
                "Runtime recipe does not match Python 3.12 on linux-x86_64",
            ));
        }
        if let Some(plan) = plan {
            let observed_hash = format!(
                "{:x}",
                Sha256::digest(std::fs::read(&plan.interpreter_path).map_err(PumasError::from)?)
            );
            if observed_hash != plan.interpreter_hash
                || std::fs::canonicalize(&plan.interpreter_path).map_err(PumasError::from)?
                    != plan.managed_python.executable
            {
                return Err(failed("Selected Python executable changed after preview"));
            }
            record_managed_python(&runtime, plan)?;
        }
        let plan = plan.ok_or_else(|| {
            failed("The bundled Torch runtime requires a retained managed Python preview")
        })?;
        let interpreter = plan.interpreter_path.as_path();
        let mut python_check = Command::new(interpreter);
        python_check.args([
            "-I",
            "-c",
            "import sys; assert sys.version_info[:2] == (3,12)",
        ]);
        self.run_runtime_command(
            python_check,
            log_path,
            "Checking Python 3.12",
            progress_tx,
            Some(staging.clone()),
        )
        .await?;
        let mut venv = Command::new(interpreter);
        venv.args(["-I", "-m", "venv"]).arg(runtime.join("venv"));
        self.run_runtime_command(
            venv,
            log_path,
            "Creating managed environment",
            progress_tx,
            Some(staging.clone()),
        )
        .await?;
        let python = pumas_library::platform::paths::venv_python(&runtime);
        let pip_cache = super::super::torch_workspace::managed_pip_cache_dir(&self.launcher_root)?;
        let download_progress_path = runtime.join("download-progress.json");
        let mut install = Command::new(&python);
        install
            .arg("-I")
            .arg(runtime.join("resolve_runtime.py"))
            .arg("--_pumas-pip-progress-worker")
            .arg(&download_progress_path)
            .args([
                "--isolated",
                "install",
                "--require-hashes",
                "--only-binary=:all:",
                "--disable-pip-version-check",
                "-r",
            ])
            .arg(runtime.join("requirements.txt"))
            .arg("--cache-dir")
            .arg(pip_cache);
        let status = self
            .run_runtime_command_status_with_download_progress(
                install,
                log_path,
                "Installing locked runtime dependencies",
                progress_tx,
                Some(staging.clone()),
                Some(&download_progress_path),
            )
            .await?;
        if !status.success() {
            return Err(failed(
                "Installing locked runtime dependencies failed; see installation log",
            ));
        }
        let mut validate = Command::new(&python);
        validate
            .arg(runtime.join("validate_runtime.py"))
            .current_dir(&runtime)
            .env("HF_HUB_OFFLINE", "1")
            .env("PYTHONNOUSERSITE", "1");
        self.run_runtime_command(
            validate,
            log_path,
            "Validating GPU and sidecar protocol",
            progress_tx,
            Some(staging.clone()),
        )
        .await?;
        let resolution = serde_json::json!({
            "torch": recipe_spec.torch_version,
            "build": "cu130",
            "python": "3.12",
            "adapter": "bundled",
            "managed_python": managed_python_record(plan),
            "artifacts": plan.preview.artifacts.clone(),
        });
        std::fs::write(
            runtime.join("resolution.json"),
            serde_json::to_vec_pretty(&resolution).map_err(|e| failed(e.to_string()))?,
        )
        .map_err(PumasError::from)?;
        let mut probe = Command::new(&python);
        probe.arg(runtime.join("probe_runtime.py"));
        self.run_runtime_command(
            probe,
            log_path,
            "Recording core and adapter probe evidence",
            progress_tx,
            Some(staging.clone()),
        )
        .await?;
        Ok(runtime)
    }

    pub(super) async fn run_runtime_command(
        &self,
        command: Command,
        log_path: &Path,
        stage: &str,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
        stage_lease: Option<std::sync::Arc<TorchPendingStage>>,
    ) -> Result<()> {
        let status = self
            .run_runtime_command_status(command, log_path, stage, progress_tx, stage_lease)
            .await?;
        if status.success() {
            Ok(())
        } else {
            Err(failed(format!(
                "{stage} failed ({status}); see installation log"
            )))
        }
    }

    async fn run_runtime_command_status(
        &self,
        command: Command,
        log_path: &Path,
        stage: &str,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
        stage_lease: Option<std::sync::Arc<TorchPendingStage>>,
    ) -> Result<std::process::ExitStatus> {
        self.run_runtime_command_status_with_download_progress(
            command,
            log_path,
            stage,
            progress_tx,
            stage_lease,
            None,
        )
        .await
    }

    async fn run_runtime_command_status_with_download_progress(
        &self,
        mut command: Command,
        log_path: &Path,
        stage: &str,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
        stage_lease: Option<std::sync::Arc<TorchPendingStage>>,
        download_progress_path: Option<&Path>,
    ) -> Result<std::process::ExitStatus> {
        self.check_cancelled()?;
        if let Some(stage_lease) = &stage_lease {
            let scratch = stage_lease.scratch_path()?;
            command
                .env("TMPDIR", &scratch)
                .env("TMP", &scratch)
                .env("TEMP", &scratch);
        }
        self.progress_tracker.write().await.update_stage(
            if download_progress_path.is_some() {
                InstallationStage::Dependencies
            } else {
                InstallationStage::Setup
            },
            0.0,
            Some(stage),
        );
        let progress_update = if download_progress_path.is_some() {
            ProgressUpdate::StageChanged {
                stage: InstallationStage::Dependencies,
                message: stage.to_string(),
            }
        } else {
            ProgressUpdate::Setup {
                message: stage.to_string(),
            }
        };
        let _ = progress_tx.try_send(progress_update);
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(PumasError::from)?;
        command
            .stdout(Stdio::from(log.try_clone().map_err(PumasError::from)?))
            .stderr(Stdio::from(log));
        let custody = self.torch_cleanup.new_child_slot()?;
        let mut child = pumas_library::platform::managed_child::ManagedChild::spawn(
            command.as_std_mut(),
            custody.clone(),
        )
        .map_err(|error| failed(format!("{stage}: {error}")))?;
        if let Some(stage_lease) = stage_lease {
            child.attach_cleanup_lease(stage_lease);
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3600);
        let mut last_download_progress: Option<TorchDownloadProgress> = None;
        let mut last_download_progress_change_at: Option<tokio::time::Instant> = None;
        let mut download_speed_expired = false;
        loop {
            let observed = child.observe_exit();
            let cancelled = self.cancel_flag.load(Ordering::SeqCst);
            let timed_out = tokio::time::Instant::now() >= deadline;
            if let Some(path) = download_progress_path {
                if let Some(download_progress) = read_torch_download_progress(path) {
                    if last_download_progress.as_ref() != Some(&download_progress) {
                        self.progress_tracker.write().await.update_network_transfer(
                            download_progress.source_url.as_deref(),
                            download_progress.active,
                            download_progress.downloaded_bytes,
                            download_progress.total_bytes,
                            download_progress.speed_bytes_per_sec,
                            download_progress.measurement_available,
                        );
                        last_download_progress = Some(download_progress);
                        last_download_progress_change_at = Some(tokio::time::Instant::now());
                        download_speed_expired = false;
                    } else if !download_speed_expired
                        && last_download_progress.as_ref().is_some_and(|progress| {
                            progress.active && progress.speed_bytes_per_sec.is_some()
                        })
                        && last_download_progress_change_at.is_some_and(|changed_at| {
                            changed_at.elapsed() >= TORCH_DOWNLOAD_SPEED_STALE_AFTER
                        })
                    {
                        let progress = last_download_progress.as_ref().expect("checked above");
                        self.progress_tracker.write().await.update_network_transfer(
                            progress.source_url.as_deref(),
                            true,
                            progress.downloaded_bytes,
                            progress.total_bytes,
                            None,
                            progress.measurement_available,
                        );
                        download_speed_expired = true;
                    }
                }
            }
            if cancelled || timed_out || !matches!(&observed, Ok(None)) {
                drop(child);
                self.torch_cleanup
                    .drain_child_slot(&custody)
                    .await
                    .map_err(|_| {
                        failed(
                        "Torch installer cleanup incomplete; owned process cleanup remains pending",
                    )
                    })?;
                if let Some(path) = download_progress_path {
                    let _ = std::fs::remove_file(path);
                }
                self.check_cancelled()?;
                if timed_out {
                    return Err(failed(format!(
                        "{stage} exceeded the installation deadline"
                    )));
                }
                let status = observed
                    .map_err(PumasError::from)?
                    .ok_or_else(|| failed("Missing terminal installer status"))?;
                return Ok(status);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

#[cfg(test)]
mod download_progress_tests {
    use super::*;

    #[test]
    fn progress_source_accepts_any_safe_https_host_and_path() {
        for source in [
            "https://download.pytorch.org/whl/cu134/torch-2.14.0.whl",
            "https://download-r2.pytorch.org/whl/cpu/torch-2.14.0.whl",
            "https://files.pythonhosted.org/packages/ab/pkg-1.0.whl",
            "https://github.com/nunchux-ai/nunchaku/releases/download/v1.2.0/nunchaku-1.2.0.whl",
            "https://mirror.example.net:8443/releases/model.safetensors",
        ] {
            assert!(safe_torch_download_source(source), "{source}");
        }
        for source in [
            "http://files.pythonhosted.org/packages/pkg-1.0.whl",
            "https://user:secret@files.pythonhosted.org/packages/pkg-1.0.whl",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl?token=secret",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl?",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl#download",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl#",
            "https://mirror.example.net/releases/model file.bin",
            &format!("https://mirror.example.net/{}", "a".repeat(2048)),
        ] {
            assert!(!safe_torch_download_source(source), "{source}");
        }
    }

    #[test]
    fn progress_file_rejects_secret_sources_and_bad_rates() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("download-progress.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "source_url":"https://example.com/torch.whl?token=secret",
                "active":true,
                "downloaded_bytes":1,
                "total_bytes":2,
                "speed_bytes_per_sec":1.0,
                "measurement_available":true
            })
            .to_string(),
        )
        .unwrap();
        assert!(read_torch_download_progress(&path).is_none());

        std::fs::write(
            &path,
            serde_json::json!({
                "source_url":"https://mirror.example.net:8443/releases/model.safetensors",
                "active":true,
                "downloaded_bytes":3,
                "total_bytes":2,
                "speed_bytes_per_sec":1.0
            })
            .to_string(),
        )
        .unwrap();
        assert!(read_torch_download_progress(&path).is_none());

        std::fs::write(
            &path,
            serde_json::json!({
                "source_url":"https://mirror.example.net:8443/releases/model.safetensors",
                "active":true,
                "downloaded_bytes":1,
                "total_bytes":2,
                "speed_bytes_per_sec":10.0,
                "measurement_available":true
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            read_torch_download_progress(&path),
            Some(TorchDownloadProgress {
                source_url: Some(
                    "https://mirror.example.net:8443/releases/model.safetensors".into(),
                ),
                active: true,
                downloaded_bytes: 1,
                total_bytes: Some(2),
                speed_bytes_per_sec: Some(10.0),
                measurement_available: true,
            })
        );

        std::fs::write(
            &path,
            serde_json::json!({
                "source_url":null,
                "active":true,
                "downloaded_bytes":1,
                "total_bytes":2,
                "speed_bytes_per_sec":1.0,
                "measurement_available":true
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            read_torch_download_progress(&path),
            Some(TorchDownloadProgress {
                source_url: None,
                active: true,
                downloaded_bytes: 1,
                total_bytes: Some(2),
                speed_bytes_per_sec: Some(1.0),
                measurement_available: true,
            })
        );
    }
}

#[cfg(test)]
mod managed_python_provenance_tests {
    use super::*;

    #[test]
    fn direct_install_report_requires_trusted_hash_locked_wheels_before_probe() {
        let torch_version = if cfg!(target_os = "macos") {
            "2.14.0"
        } else {
            "2.14.0+cpu"
        };
        let artifacts = [
            "torch",
            "fastapi",
            "uvicorn",
            "psutil",
            "pillow",
            "safetensors",
        ]
        .into_iter()
        .map(|name| crate::version_manager::TorchArtifact {
            name: name.into(),
            version: if name == "torch" {
                torch_version.into()
            } else {
                "1.0".into()
            },
            url: if name == "torch" {
                "https://download.pytorch.org/whl/cpu/torch/torch-fixture.whl".into()
            } else {
                format!("https://files.pythonhosted.org/packages/{name}-fixture.whl")
            },
            sha256: "a".repeat(64),
        })
        .collect::<Vec<_>>();
        let (platform, machine) = match std::env::consts::OS {
            "linux" => ("Linux-fixture", "x86_64"),
            "windows" => ("Windows-fixture", "AMD64"),
            "macos" => ("macOS-fixture", "arm64"),
            _ => return,
        };
        let mut resolution = DirectTorchResolution {
            release: "2.14.0".into(),
            torch: torch_version.into(),
            build: "cpu".into(),
            python: "3.12".into(),
            interpreter: "/staged/venv/python".into(),
            implementation: "cpython".into(),
            platform: platform.into(),
            machine: machine.into(),
            adapter: "none".into(),
            artifacts,
        };
        let report = serde_json::json!({
            "install": resolution.artifacts.iter().map(|artifact| serde_json::json!({
                "metadata": {"name": artifact.name, "version": artifact.version},
                "download_info": {"url": artifact.url, "archive_info": {"hashes": {"sha256": artifact.sha256}}}
            })).collect::<Vec<_>>()
        });
        let requirements = resolution
            .artifacts
            .iter()
            .map(|artifact| {
                format!(
                    "{} @ {} --hash=sha256:{}",
                    artifact.name, artifact.url, artifact.sha256
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let python = Path::new("/staged/venv/python");
        let check = |resolution: &DirectTorchResolution, report: &serde_json::Value, lock: &str| {
            validate_direct_torch_report(
                resolution, report, lock, "2.14.0", "cpu", "3.12", "none", python,
            )
        };
        assert!(check(&resolution, &report, &requirements).is_ok());
        resolution.artifacts[0].url = "https://example.com/torch.whl".into();
        assert!(check(&resolution, &report, &requirements).is_err());
        resolution.artifacts[0].url = report["install"][0]["download_info"]["url"]
            .as_str()
            .unwrap()
            .into();
        resolution.artifacts[0].sha256 = "invalid".into();
        assert!(check(&resolution, &report, &requirements).is_err());
        resolution.artifacts[0].sha256 = "a".repeat(64);
        assert!(check(&resolution, &report, "torch @ https://download.pytorch.org/whl/cpu/torch/torch-fixture.whl --hash=sha256:bad").is_err());

        let stage = tempfile::tempdir().unwrap();
        let runtime = stage.path().join("runtime");
        let target = runtime.join("staged-packages");
        std::fs::create_dir_all(&target).unwrap();
        let contents = "raise RuntimeError('executed')";
        std::fs::write(target.join("verified.pth"), contents).unwrap();
        let manifest = StagedFilesManifest {
            files: vec![StagedFile {
                path: "verified.pth".into(),
                sha256: format!("{:x}", Sha256::digest(contents.as_bytes())),
                size: contents.len() as u64,
            }],
        };
        let publish = |lock: &str| {
            validate_and_move_direct_torch_packages(
                &resolution,
                &report,
                lock,
                "2.14.0",
                "cpu",
                "3.12",
                "none",
                python,
                &target,
                &runtime,
                &manifest,
            )
        };
        assert!(publish("malformed lock").is_err());
        assert!(target.join("verified.pth").exists());
        #[cfg(windows)]
        let installed = runtime.join("venv/Lib/site-packages/verified.pth");
        #[cfg(not(windows))]
        let installed = runtime.join("venv/lib/python3.12/site-packages/verified.pth");
        assert!(!installed.exists());
        std::fs::write(target.join("unreported.pth"), "evil").unwrap();
        assert!(publish(&requirements).is_err());
        assert!(!installed.exists());
        std::fs::remove_file(target.join("unreported.pth")).unwrap();
        publish(&requirements).unwrap();
        assert!(installed.exists());
        assert!(!target.exists());
    }

    #[test]
    fn automatic_cuda_channel_uses_existing_linux_driver_floors() {
        assert_eq!(cuda_build_for_driver((579, 99, 99)), Some("cu129"));
        assert_eq!(cuda_build_for_driver((580, 0, 0)), Some("cu134"));
        assert_eq!(cuda_build_for_driver((525, 60, 13)), Some("cu129"));
        assert_eq!(cuda_build_for_driver((450, 80, 2)), Some("cu118"));
        assert_eq!(cuda_build_for_driver((440, 33, 0)), Some("cu102"));
        assert_eq!(cuda_build_for_driver((409, 99, 99)), None);
    }

    #[test]
    fn automatic_build_candidates_try_supported_cuda_channels_before_cpu() {
        let builds = automatic_torch_builds_for_driver((580, 0, 0));
        assert_eq!(builds, ["cu134", "cu132", "cu130", "cu129", "cpu"]);
        assert!(!builds.contains(&"cu75"));

        // An unsupported cu134 wheel can proceed to cu132 and stop there.
        assert!(retry_direct_torch_attempt(Some(2), false, true));
        assert_eq!(builds[1], "cu132");
        assert!(!retry_direct_torch_attempt(Some(1), false, true));
    }

    #[test]
    fn automatic_build_candidates_reach_cpu_after_unsupported_cuda_wheels() {
        let builds = automatic_torch_builds_for_driver((525, 60, 13));
        assert_eq!(builds, ["cu129", "cu128", "cu126", "cu124", "cpu"]);
        assert!(builds[..builds.len() - 1]
            .iter()
            .all(|_| retry_direct_torch_attempt(Some(4), false, true)));
        assert_eq!(automatic_torch_builds_for_driver((409, 99, 99)), ["cpu"]);
    }

    #[test]
    fn automatic_python_fallback_only_accepts_conclusive_wheel_failures() {
        for code in [2, 4] {
            assert!(retry_torch_python_candidate(Some(code), true));
            assert!(!retry_torch_python_candidate(Some(code), false));
            assert!(retry_direct_torch_attempt(Some(code), false, true));
            assert!(!retry_direct_torch_attempt(Some(code), false, false));
        }
        for code in [1, 3, 75, 124] {
            assert!(!retry_torch_python_candidate(Some(code), true));
        }
        assert!(!retry_torch_python_candidate(None, true));
    }

    #[test]
    fn validated_staged_packages_move_into_venv_only_without_collisions() {
        let root = tempfile::tempdir().unwrap();
        let runtime = root.path().join("runtime");
        let target = runtime.join("staged-packages");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("torch.py"), "trusted staged wheel").unwrap();
        move_verified_packages(&target, &runtime, "3.12").unwrap();
        #[cfg(windows)]
        let installed = runtime.join("venv/Lib/site-packages/torch.py");
        #[cfg(not(windows))]
        let installed = runtime.join("venv/lib/python3.12/site-packages/torch.py");
        assert_eq!(
            std::fs::read_to_string(installed).unwrap(),
            "trusted staged wheel"
        );
        assert!(!target.exists());
    }

    #[tokio::test]
    async fn installed_runtime_recipe_retains_managed_python_provider_identity() {
        let root = tempfile::tempdir().unwrap();
        let runtime = root.path().join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(
            runtime.join("runtime.json"),
            r#"{"recipe_id":"upstream-preview-v2.14.0-cpu-python3.14-none"}"#,
        )
        .unwrap();
        let plan = super::TorchInstallPlan {
            preview: crate::version_manager::TorchPreview {
                preview_id: "preview-id".into(),
                tag: "v2.14.0".into(),
                build: "cpu".into(),
                python: "python3.14".into(),
                adapter: "none".into(),
                artifacts: Vec::new(),
                qualification: "unverified".into(),
                expires_in_seconds: 1800,
            },
            requirements: String::new(),
            resolution: String::new(),
            report: String::new(),
            interpreter_path: root.path().join("python3.14"),
            interpreter_hash: "b".repeat(64),
            managed_python: crate::version_manager::managed_python::ManagedPythonIdentity {
                python: "python3.14".into(),
                version: "3.14.0".into(),
                catalog_key: "cpython-3.14.0+20250901-x86_64-unknown-linux-gnu-install_only".into(),
                source_url: "https://github.com/astral-sh/python-build-standalone/releases/download/20250901/cpython-3.14.0%2B20250901-x86_64-unknown-linux-gnu-install_only.tar.zst".into(),
                executable: root.path().join("python3.14"),
                target_triple: "x86_64-unknown-linux-gnu".into(),
                uv_version: "0.12.19".into(),
                uv_archive_sha256: "a".repeat(64),
            },
        };

        record_managed_python(&runtime, &plan).unwrap();

        let recipe: serde_json::Value =
            serde_json::from_slice(&fs::read(runtime.join("runtime.json")).await.unwrap()).unwrap();
        assert_eq!(recipe["managed_python"]["provider"]["name"], "uv");
        assert_eq!(recipe["managed_python"]["provider"]["version"], "0.12.19");
        assert_eq!(
            recipe["managed_python"]["provider"]["archiveSha256"],
            "a".repeat(64)
        );
        assert_eq!(
            recipe["managed_python"]["distribution"]["version"],
            "3.14.0"
        );
        assert_eq!(
            recipe["managed_python"]["distribution"]["targetTriple"],
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            recipe["managed_python"]["executable"]["sha256"],
            "b".repeat(64)
        );
    }
}
