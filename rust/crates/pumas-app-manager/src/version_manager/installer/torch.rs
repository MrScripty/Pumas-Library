//! Managed upstream PyTorch installation; lifecycle and state remain in VersionManager.

use super::*;
use crate::torch_client::{
    SUPPORTED_TORCH_PROTOCOL, TORCH_AUDIO_TRANSCRIPTION_CAPABILITY,
    TORCH_IMAGE_GENERATION_CAPABILITY,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

/// Installation checks may import sidecar code directly or in a child process.
/// Keep their imports from changing the runtime's retained source-byte tree or
/// writing bytecode next to the sidecar every runtime shares.
fn runtime_check_command(python: &Path, script: &Path) -> Command {
    let mut command = Command::new(python);
    command.arg(script).env("PYTHONDONTWRITEBYTECODE", "1");
    command
}

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
        // SAFETY: FileInformation is a #[repr(C)] struct of u32 fields and u32
        // arrays only, for which the all-zero bit pattern is a valid value; it
        // is plain output storage that the call below overwrites.
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
            std::fs::create_dir_all(&runtime).unwrap();
            std::fs::write(runtime.join("runtime.json"), "{}").unwrap();
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
            unavailable_reason: None,
            install_availability: None,
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

pub(crate) fn is_torch_runtime_release(release: &GitHubRelease) -> bool {
    !release.is_prerelease() && stable_torch_tag(&release.tag_name).is_some()
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

#[derive(Clone, Deserialize)]
struct DirectTorchResolution {
    release: String,
    torch: String,
    build: String,
    python: String,
    interpreter: String,
    implementation: String,
    platform: String,
    machine: String,
    /// General modalities this resolution carries.
    modalities: Vec<String>,
    /// General modalities left out, each with the resolver's reason.
    #[serde(default)]
    omitted_modalities: std::collections::BTreeMap<String, String>,
    /// Optional extras the resolution added.
    extras: Vec<String>,
    artifacts: Vec<crate::version_manager::TorchArtifact>,
}

/// Packages each general modality needs, as `resolve_runtime.py` resolves
/// them (image and audio models use the text stack too). A test keeps this
/// table equal to the resolver's.
const MODALITY_PACKAGES: &[(&str, &[&str])] = &[
    (
        "text",
        &[
            "transformers",
            "tokenizers",
            "huggingface-hub",
            "jinja2",
            "sentencepiece",
            "protobuf",
        ],
    ),
    ("image", &["torchvision", "diffusers", "accelerate", "peft"]),
    (
        "audio",
        &[
            "accelerate",
            "librosa",
            "soundfile",
            "soxr",
            "numpy",
            "scipy",
        ],
    ),
];

fn modality_packages(modality: &str) -> impl Iterator<Item = &'static str> {
    let own = MODALITY_PACKAGES
        .iter()
        .find(|(name, _)| *name == modality)
        .map_or(&[][..], |(_, packages)| *packages);
    let text = if modality == "text" {
        &[][..]
    } else {
        MODALITY_PACKAGES[0].1
    };
    text.iter().chain(own).copied()
}

struct DirectTorchSelection<'a> {
    version: &'a str,
    build: &'a str,
    minor: &'a str,
    /// Optional extras added to the general modalities.
    extras: &'a [String],
    python: &'a Path,
    /// The verified local copy of the official Torch wheel that pip resolved
    /// against, when shared acquisition downloaded it.
    local_torch_wheel: Option<&'a Path>,
}

/// Whether a pip report URL names exactly this existing local file.
fn report_names_local_file(url: &str, path: &Path) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    if url.scheme() != "file" {
        return false;
    }
    let Ok(reported) = url.to_file_path() else {
        return false;
    };
    matches!(
        (std::fs::canonicalize(reported), std::fs::canonicalize(path)),
        (Ok(reported), Ok(expected)) if reported == expected
    )
}

/// A report source compares by resolved local file, or else by exact URL.
fn installed_source(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .filter(|parsed| parsed.scheme() == "file")
        .and_then(|parsed| parsed.to_file_path().ok())
        .and_then(|path| std::fs::canonicalize(path).ok())
        .map_or_else(
            || url.to_owned(),
            |path| format!("local:{}", path.display()),
        )
}

/// The install selection a wheel set serves: release, build, Python,
/// dependency profile, platform, package-version choice and wheel role. A
/// later resolution of the same selection supersedes its unfinished sets.
/// Opting into verified package versions is a different selection: it can
/// resolve different wheels, and switching between the two choices must not
/// supersede the other's partial downloads.
fn torch_wheel_selection(
    tag: &str,
    build: &str,
    minor: &str,
    extras: &[String],
    verified_constraints: Option<&str>,
    role: &str,
) -> String {
    let version_choice = verified_constraints.map_or_else(
        || "current".to_owned(),
        |constraints| format!("verified-{:x}", Sha256::digest(constraints.as_bytes())),
    );
    let extras = if extras.is_empty() {
        "general".to_owned()
    } else {
        extras.join("+")
    };
    format!(
        "{tag}|{build}|python{minor}|{extras}|{}-{}|{version_choice}|{role}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// Recorded when the text stack (Transformers, tokenizers, Jinja) imports, so
/// text models such as embedders and rerankers can load. Not a sidecar
/// protocol capability.
const TORCH_TEXT_CAPABILITY: &str = "text_models";

/// Serving capabilities a probe found importable: image generation needs the
/// image modality, audio transcription the Cohere ASR family, and text work
/// the text modality. A capability the probe could not import is absent.
fn serving_capabilities(probed: &serde_json::Value) -> Vec<&'static str> {
    let importable = |key: &str| {
        matches!(
            probed[key]["status"].as_str(),
            Some("passed" | "inconclusive")
        )
    };
    [
        ("text_modality", TORCH_TEXT_CAPABILITY),
        ("image_modality", TORCH_IMAGE_GENERATION_CAPABILITY),
        ("cohere_asr", TORCH_AUDIO_TRANSCRIPTION_CAPABILITY),
    ]
    .into_iter()
    .filter(|(key, _)| importable(key))
    .map(|(_, capability)| capability)
    .collect()
}

/// The resolver's selected official Torch wheel, before acquisition.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedTorchWheel {
    url: String,
    sha256: String,
}

/// The selected Torch wheel must come from the official channel (or, for
/// macOS CPU, PyPI) before Pumas downloads it.
fn trusted_selected_torch_wheel(url: &str, build: &str, expected_torch: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    parsed.scheme() == "https"
        && !url.chars().any(char::is_whitespace)
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && matches!(parsed.port(), None | Some(443))
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && parsed.path().to_ascii_lowercase().ends_with(".whl")
        && ((matches!(
            parsed.host_str(),
            Some("download.pytorch.org" | "download-r2.pytorch.org")
        ) && parsed.path().starts_with(&format!("/whl/{build}/")))
            || trusted_macos_pypi_torch_wheel(
                &parsed,
                "torch",
                expected_torch,
                cfg!(target_os = "macos") && build == "cpu",
            ))
}

struct TorchAttemptContext<'a> {
    staging: &'a std::sync::Arc<TorchPendingStage>,
    log_path: &'a Path,
    progress_tx: &'a mpsc::Sender<ProgressUpdate>,
}

fn trusted_macos_pypi_torch_wheel(
    url: &reqwest::Url,
    name: &str,
    version: &str,
    macos_cpu: bool,
) -> bool {
    if !macos_cpu
        || !matches!(name, "torch" | "torchvision")
        || url.host_str() != Some("files.pythonhosted.org")
        || !url.path().starts_with("/packages/")
        || version.contains('+')
    {
        return false;
    }
    let Some(filename) = url.path().rsplit('/').next() else {
        return false;
    };
    let prefix = format!("{name}-{version}-");
    let Some(tags) = filename
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(".whl"))
    else {
        return false;
    };
    let mut parts = tags.split('-');
    let (Some(python), Some(abi), Some(platform), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    !python.is_empty()
        && !abi.is_empty()
        && platform.split('.').any(|tag| {
            tag.starts_with("macosx_") && (tag.ends_with("_arm64") || tag.ends_with("_universal2"))
        })
}

/// Whether Python reported the interpreter Pumas selected. Windows can name
/// one file by its 8.3 short path (`RUNNER~1`) or its long path, so a textual
/// mismatch falls back to comparing the resolved files.
fn same_interpreter(reported: &Path, selected: &Path) -> bool {
    reported == selected
        || matches!(
            (std::fs::canonicalize(reported), std::fs::canonicalize(selected)),
            (Ok(reported), Ok(selected)) if reported == selected
        )
}

fn validate_direct_torch_report(
    resolution: &DirectTorchResolution,
    report: &serde_json::Value,
    requirements: &str,
    selection: &DirectTorchSelection<'_>,
) -> Result<()> {
    let version = selection.version;
    let build = selection.build;
    let minor = selection.minor;
    let extras = selection.extras;
    let python = selection.python;
    let expected_torch = if cfg!(target_os = "macos") && build == "cpu" {
        version.to_owned()
    } else {
        format!("{version}+{build}")
    };
    if resolution.release != version
        || resolution.torch != expected_torch
        || resolution.build != build
        || resolution.python != minor
        || resolution.extras != extras
        || resolution.implementation != "cpython"
        || !same_interpreter(Path::new(&resolution.interpreter), python)
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
                (matches!(
                    url.host_str(),
                    Some("download.pytorch.org" | "download-r2.pytorch.org")
                ) && url.path().starts_with(&format!("/whl/{build}/")))
                    || trusted_macos_pypi_torch_wheel(
                        &url,
                        &artifact.name,
                        &artifact.version,
                        cfg!(target_os = "macos") && build == "cpu",
                    )
            } else if extras.iter().any(|extra| extra == "nunchaku") && artifact.name == "nunchaku"
            {
                // The extra's own wheel comes from its project's releases;
                // the resolver admits only its exact URL and hash.
                url.host_str() == Some("github.com")
                    && url
                        .path()
                        .starts_with("/nunchux-ai/nunchaku/releases/download/")
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
        if artifact.name == "torchvision"
            && (!resolution
                .modalities
                .iter()
                .any(|modality| modality == "image")
                || !(artifact.version.ends_with(&format!("+{build}"))
                    || (cfg!(target_os = "macos")
                        && build == "cpu"
                        && artifact.version.split('.').count() == 3
                        && artifact
                            .version
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || byte == b'.'))))
        {
            return Err(failed(
                "Torchvision build differs from selected Torch build",
            ));
        }
        let item = &entries[index];
        let report_name = item["metadata"]["name"]
            .as_str()
            .map(|name| name.to_ascii_lowercase().replace('_', "-"));
        // pip read an acquired Torch wheel locally; the lock keeps its official URL.
        let reported_source = match (artifact.name == "torch", selection.local_torch_wheel) {
            (true, Some(local)) => item["download_info"]["url"]
                .as_str()
                .is_some_and(|url| report_names_local_file(url, local)),
            _ => item["download_info"]["url"].as_str() == Some(artifact.url.as_str()),
        };
        if report_name.as_deref() != Some(artifact.name.as_str())
            || item["metadata"]["version"].as_str() != Some(artifact.version.as_str())
            || !reported_source
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
    // Every general modality is resolved or left out with a reason, and a
    // resolved modality brings all of its packages.
    let known = |modality: &str| MODALITY_PACKAGES.iter().any(|(name, _)| *name == modality);
    if resolution
        .modalities
        .iter()
        .any(|modality| !known(modality))
        || resolution
            .omitted_modalities
            .iter()
            .any(|(modality, reason)| {
                !known(modality)
                    || reason.trim().is_empty()
                    || resolution.modalities.contains(modality)
            })
        || MODALITY_PACKAGES.iter().any(|(modality, _)| {
            !resolution
                .modalities
                .iter()
                .any(|resolved| resolved == modality)
                && !resolution.omitted_modalities.contains_key(*modality)
        })
    {
        return Err(failed(
            "Torch report must resolve each general modality or name why it was left out",
        ));
    }
    for modality in &resolution.modalities {
        if modality_packages(modality).any(|required| !names.contains(required)) {
            return Err(failed(format!(
                "Torch report omitted a {modality} dependency"
            )));
        }
    }
    for extra in extras {
        if !super::super::torch_preview::EXTRAS.contains(&extra.as_str())
            || !names.contains(extra.as_str())
        {
            return Err(failed("Torch report omitted a requested extra's wheel"));
        }
    }
    Ok(())
}

/// The phase-2 pip report must install exactly the accepted lock: the same
/// distributions, versions, sources and SHA-256 digests, no more and no fewer.
/// A distribution in `local_files` must come from that acquired local file;
/// every other one from its locked URL.
fn validate_locked_install_report(
    resolution: &DirectTorchResolution,
    report: &serde_json::Value,
    local_files: &std::collections::HashMap<String, PathBuf>,
) -> Result<()> {
    let entries = report["install"]
        .as_array()
        .ok_or_else(|| failed("Torch pip installation report has no artifact list"))?;
    let mut installed = entries
        .iter()
        .map(|item| {
            Some((
                item["metadata"]["name"]
                    .as_str()?
                    .to_ascii_lowercase()
                    .replace('_', "-"),
                item["metadata"]["version"].as_str()?.to_owned(),
                installed_source(item["download_info"]["url"].as_str()?),
                item["download_info"]["archive_info"]["hashes"]["sha256"]
                    .as_str()?
                    .to_ascii_lowercase(),
            ))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| failed("Torch pip installation report has an incomplete artifact"))?;
    let mut locked = resolution
        .artifacts
        .iter()
        .map(|artifact| {
            let source = match local_files.get(&artifact.name) {
                Some(path) => format!(
                    "local:{}",
                    std::fs::canonicalize(path)
                        .map_err(|_| failed("An acquired Torch wheel is missing"))?
                        .display()
                ),
                None => artifact.url.clone(),
            };
            Ok((
                artifact.name.clone(),
                artifact.version.clone(),
                source,
                artifact.sha256.to_ascii_lowercase(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    installed.sort();
    locked.sort();
    if installed != locked {
        return Err(failed(
            "Torch installed artifacts differ from the accepted hash lock",
        ));
    }
    Ok(())
}

enum DirectTorchAttempt {
    Installed(PathBuf),
    Retry,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct StagedFilesManifest {
    pub(crate) files: Vec<StagedFile>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct StagedFile {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) size: u64,
}

pub(crate) fn validate_staged_files(target: &Path, manifest: &StagedFilesManifest) -> Result<()> {
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
        // Content, not size: a same-size substitution must be refused too.
        let mut file = std::fs::File::open(entry.path())
            .map_err(|_| failed("Cannot read Torch staged package file"))?;
        let mut digest = Sha256::new();
        let size = std::io::copy(&mut file, &mut digest)
            .map_err(|_| failed("Cannot read Torch staged package file"))?;
        if size != item.size
            || !format!("{:x}", digest.finalize()).eq_ignore_ascii_case(&item.sha256)
        {
            return Err(failed(format!(
                "Torch staged file differs from its manifest: {name}"
            )));
        }
        seen += 1;
    }
    if seen != expected.len() {
        return Err(failed("Torch staged wheel manifest names missing files"));
    }
    Ok(())
}

fn venv_packages_dir(runtime: &Path, python_minor: &str) -> PathBuf {
    // Windows venvs have no per-minor directory.
    #[cfg(windows)]
    let _ = python_minor;
    #[cfg(windows)]
    let packages = runtime.join("venv").join("Lib").join("site-packages");
    #[cfg(not(windows))]
    let packages = runtime
        .join("venv")
        .join("lib")
        .join(format!("python{python_minor}"))
        .join("site-packages");
    packages
}

/// Exact files produced in a fresh private venv, before dependency resolution.
/// This is a one-install bootstrap snapshot, never an exemption in capture.
pub(crate) struct BootstrapPackages {
    runtime: PathBuf,
    root: PathBuf,
    members: Vec<BootstrapMember>,
    directories: Vec<PathBuf>,
}

struct BootstrapMember {
    path: String,
    size: u64,
    file: std::fs::File,
}

impl BootstrapPackages {
    pub(crate) fn capture(runtime: &Path, python_minor: &str) -> Result<Self> {
        Self::capture_root(runtime, &venv_packages_dir(runtime, python_minor))
    }

    fn capture_root(runtime: &Path, root: &Path) -> Result<Self> {
        for parent in root
            .ancestors()
            .take_while(|path| path.starts_with(runtime))
        {
            let metadata = std::fs::symlink_metadata(parent).map_err(PumasError::from)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(failed(
                    "Venv bootstrap root contains a link or non-directory",
                ));
            }
        }
        let mut files = Vec::new();
        let mut directories = Vec::new();
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = entry.map_err(|_| failed("Cannot inspect fresh venv bootstrap"))?;
            if entry.file_type().is_symlink() {
                return Err(failed("Venv bootstrap contains a link"));
            }
            if entry.path() == root {
                if !entry.file_type().is_dir() {
                    return Err(failed("Venv bootstrap root is not a directory"));
                }
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| failed("Venv bootstrap escaped its package root"))?;
            if entry.file_type().is_dir() {
                directories.push(relative.to_owned());
            } else if entry.file_type().is_file() {
                let file = std::fs::File::open(entry.path()).map_err(PumasError::from)?;
                let size = file.metadata().map_err(PumasError::from)?.len();
                files.push(BootstrapMember {
                    path: relative
                        .to_str()
                        .ok_or_else(|| failed("Invalid bootstrap path"))?
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                    size,
                    file,
                });
            } else {
                return Err(failed("Venv bootstrap contains a special file"));
            }
            if files.len() + directories.len() > 200_000 {
                return Err(failed("Venv bootstrap exceeds namespace bound"));
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        directories.sort_by(|left, right| {
            right
                .components()
                .count()
                .cmp(&left.components().count())
                .then_with(|| left.cmp(right))
        });
        Ok(Self {
            runtime: runtime.to_owned(),
            root: root.to_owned(),
            members: files,
            directories,
        })
    }

    pub(crate) fn remove(&self) -> Result<()> {
        // Check the entire bootstrap namespace before deleting any byte. New,
        // replaced or linked members cannot become cleanup exemptions.
        let observed = Self::capture_root(&self.runtime, &self.root)?;
        if observed.directories != self.directories || observed.members.len() != self.members.len()
        {
            return Err(failed(
                "Venv bootstrap changed during dependency resolution",
            ));
        }
        for (original, current) in self.members.iter().zip(&observed.members) {
            if original.path != current.path
                || original.size != current.size
                || !pumas_library::platform::process::same_file_identity(
                    &original.file,
                    &current.file,
                )
                .map_err(PumasError::from)?
            {
                return Err(failed(
                    "Venv bootstrap changed during dependency resolution",
                ));
            }
        }
        for member in &self.members {
            std::fs::remove_file(self.root.join(&member.path)).map_err(PumasError::from)?;
        }
        for directory in &self.directories {
            // Remove only the recorded, now-empty directories; never recurse
            // through a package tree that may already contain selected bytes.
            std::fs::remove_dir(self.root.join(directory)).map_err(PumasError::from)?;
        }
        if std::fs::read_dir(&self.root)
            .map_err(PumasError::from)?
            .next()
            .is_some()
        {
            return Err(failed("Venv bootstrap package root is not empty"));
        }
        Ok(())
    }
}

pub(crate) fn move_verified_packages(
    target: &Path,
    runtime: &Path,
    python_minor: &str,
) -> Result<()> {
    let packages = venv_packages_dir(runtime, python_minor);
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
    selection: &DirectTorchSelection<'_>,
    target: &Path,
    bootstrap: &BootstrapPackages,
    manifest: &StagedFilesManifest,
) -> Result<()> {
    validate_direct_torch_report(resolution, report, requirements, selection)?;
    validate_staged_files(target, manifest)?;
    if bootstrap.root != venv_packages_dir(&bootstrap.runtime, selection.minor) {
        return Err(failed("Venv bootstrap does not match selected Python"));
    }
    bootstrap.remove()?;
    move_verified_packages(target, &bootstrap.runtime, selection.minor)
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

    /// Shared acquisition for this attempt's wheels, or the reason the
    /// attempt keeps pip's own (non-resumable) downloads.
    fn torch_wheel_acquisition(
        &self,
        staging: &std::sync::Arc<TorchPendingStage>,
        log_path: &Path,
    ) -> std::result::Result<super::torch_wheels::TorchWheelAcquisition, String> {
        let consumer = self
            .acquisition_consumer
            .clone()
            .ok_or("this installer was created without the shared acquisition owner")?;
        if consumer.owner() != super::torch_wheels::TORCH_WHEEL_CONSUMER {
            return Err(format!(
                "the configured acquisition scope belongs to {}",
                consumer.owner()
            ));
        }
        let root = super::super::torch_workspace::managed_torch_wheel_dir(&self.launcher_root)
            .map_err(|error| format!("the wheel workspace root is unavailable: {error}"))?;
        let client = super::torch_wheels::wheel_http_client(
            super::torch_wheels::WHEEL_TRANSFER_STALL_TIMEOUT,
        )
        .map_err(|error| format!("the wheel HTTP client could not be created: {error}"))?;
        Ok(super::torch_wheels::TorchWheelAcquisition {
            consumer,
            root,
            lease: std::sync::Arc::new(staging._lock.clone()),
            cancel_flag: self.cancel_flag.clone(),
            shutdown_flag: self.shutdown_flag.clone(),
            progress: self.progress_tracker.clone(),
            log_path: log_path.to_path_buf(),
            client,
            retry: AcquisitionRetryPolicy {
                attempts: Some(InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS),
                elapsed: Duration::ZERO,
                backoff: RetryConfig::new()
                    .with_max_attempts(InstallationConfig::DOWNLOAD_RETRY_ATTEMPTS)
                    .with_base_delay(Duration::from_secs(2)),
            },
        })
    }

    /// Record why some wheels keep pip's download, which cannot resume.
    fn note_torch_pip_downloads(&self, log_path: &Path, reason: &str) {
        warn!(%reason, "Torch wheels download through pip without resume");
        let note = format!(
            "Pumas shared download unavailable ({reason}); pip downloads these wheels and \
             cannot resume them after a network failure\n"
        );
        if let Err(error) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .and_then(|mut log| std::io::Write::write_all(&mut log, note.as_bytes()))
        {
            warn!(%error, "Torch download fallback note was not written to the install log");
        }
    }

    async fn announce_torch_wheel_download(
        &self,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
        message: &str,
    ) {
        self.progress_tracker.write().await.update_stage(
            InstallationStage::Dependencies,
            0.0,
            Some(message),
        );
        let _ = progress_tx.try_send(ProgressUpdate::StageChanged {
            stage: InstallationStage::Dependencies,
            message: message.to_owned(),
        });
    }

    /// How a failed or successful attempt leaves its wheels: a user-cancelled
    /// attempt removes them; shutdown (which also sets the cancel flag) and
    /// any other failure keep them for the next try.
    fn torch_wheel_use(&self, succeeded: bool) -> super::torch_wheels::WheelUse {
        if succeeded {
            super::torch_wheels::WheelUse::Consumed
        } else if self.cancel_flag.load(Ordering::SeqCst)
            && !self.shutdown_flag.load(Ordering::SeqCst)
        {
            super::torch_wheels::WheelUse::Discard
        } else {
            super::torch_wheels::WheelUse::Retain
        }
    }

    async fn stage_selected_torch_runtime(
        &self,
        selection: &super::super::torch_preview::TorchInstallSelection,
        staging: &std::sync::Arc<TorchPendingStage>,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        if selection
            .extras
            .iter()
            .any(|extra| !super::super::torch_preview::EXTRAS.contains(&extra.as_str()))
        {
            return Err(failed("Unknown Torch extra"));
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
                        &TorchAttemptContext {
                            staging,
                            log_path,
                            progress_tx,
                        },
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
        context: &TorchAttemptContext<'_>,
    ) -> Result<DirectTorchAttempt> {
        let staging = context.staging;
        let log_path = context.log_path;
        let progress_tx = context.progress_tx;
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

        // The runtime holds only the interpreter environment, its packages and
        // their records. The resolver and probe run from this Pumas's sidecar,
        // the same code every installed runtime launches.
        let sidecar =
            pumas_library::runtime_profiles::materialize_torch_sidecar_async(&self.launcher_root)
                .await?;
        let runtime = staging.path().join("runtime");
        let runtime_for_write = runtime.clone();
        spawn_blocking_with_stage(staging.clone(), move || {
            std::fs::create_dir_all(&runtime_for_write).map_err(PumasError::from)
        })
        .await
        .map_err(|error| failed(format!("Runtime staging task failed: {error}")))??;
        let mut venv = Command::new(&managed_python.executable);
        venv.args(["-I", "-B", "-m", "venv"])
            .arg(runtime.join("venv"))
            .env("PYTHONDONTWRITEBYTECODE", "1");
        self.run_runtime_command(
            venv,
            log_path,
            &format!("Creating managed Python {minor} environment"),
            progress_tx,
            Some(staging.clone()),
        )
        .await?;

        let bootstrap = BootstrapPackages::capture(&runtime, minor)?;
        let version = selection
            .tag
            .strip_prefix('v')
            .ok_or_else(|| failed("Invalid Torch release tag"))?;
        let python = pumas_library::platform::paths::venv_python(&runtime);
        let packages = runtime.join("staged-packages");
        let resolver_output = runtime.join("resolver-result");
        let download_progress_path = resolver_output.join("download-progress.json");
        let pip_cache = super::super::torch_workspace::managed_pip_cache_dir(&self.launcher_root)?;
        let resolver_command = |phase: &[&str]| {
            let mut command = Command::new(&python);
            command
                .args(["-I", "-B"])
                .arg(sidecar.join("resolve_runtime.py"))
                .args(phase)
                .args(["--version", version, "--build", build])
                .args(
                    selection
                        .extras
                        .iter()
                        .flat_map(|extra| ["--extra", extra.as_str()]),
                )
                .arg("--progress-file")
                .arg(&download_progress_path)
                .arg("--output")
                .arg(&resolver_output)
                .arg("--cache-dir")
                .arg(&pip_cache);
            command
        };
        let expected_torch = if cfg!(target_os = "macos") && build == "cpu" {
            version.to_owned()
        } else {
            format!("{version}+{build}")
        };
        // The install selection a wheel set serves. A later resolution of the
        // same selection supersedes its unfinished sets that it no longer names.
        let wheel_selection = |role: &str| {
            torch_wheel_selection(
                &selection.tag,
                build,
                minor,
                &selection.extras,
                selection.verified_constraints.as_deref(),
                role,
            )
        };

        // Phase 0, with shared acquisition: select the official Torch wheel
        // and download it resumably with SHA-256 verification, so the
        // resolution below reads that verified local copy instead of having
        // pip download it (pip discards a partial wheel when its retries end).
        let wheel_acquisition = match self.torch_wheel_acquisition(staging, log_path) {
            Ok(acquisition) => Some(acquisition),
            Err(reason) => {
                self.note_torch_pip_downloads(log_path, &reason);
                None
            }
        };
        let mut selected_torch = None;
        let mut held_torch = None;
        if let Some(acquisition) = &wheel_acquisition {
            std::fs::create_dir_all(&resolver_output).map_err(PumasError::from)?;
            let mut select = Command::new(&python);
            select
                .args(["-I", "-B"])
                .arg(sidecar.join("resolve_runtime.py"))
                .arg("--select-torch-wheel")
                .args(["--version", version, "--build", build])
                .args(
                    selection
                        .extras
                        .iter()
                        .flat_map(|extra| ["--extra", extra.as_str()]),
                )
                .arg("--output")
                .arg(&resolver_output);
            let status = self
                .run_runtime_command_status(
                    select,
                    log_path,
                    &format!("Selecting the official Torch {build} wheel for Python {minor}"),
                    progress_tx,
                    Some(staging.clone()),
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
                    "Torch wheel selection failed ({status}); see installation log"
                )));
            }
            let selected: SelectedTorchWheel = serde_json::from_slice(
                &std::fs::read(resolver_output.join("torch-wheel.json"))
                    .map_err(PumasError::from)?,
            )
            .map_err(|error| failed(format!("Invalid Torch wheel selection: {error}")))?;
            if !trusted_selected_torch_wheel(&selected.url, build, &expected_torch) {
                return Err(failed(
                    "Torch wheel selection names an untrusted wheel source",
                ));
            }
            match super::torch_wheels::LockedWheel::new("torch", &selected.url, &selected.sha256) {
                Ok(wheel) => {
                    self.announce_torch_wheel_download(
                        progress_tx,
                        &format!("Downloading the verified Torch {build} wheel"),
                    )
                    .await;
                    match acquisition
                        .hold(&wheel_selection("torch"), vec![wheel.clone()], false)
                        .await?
                    {
                        Ok(held) => held_torch = Some((held, wheel)),
                        Err(reason) => self.note_torch_pip_downloads(
                            log_path,
                            &format!("the Torch wheel cannot use it: {reason}"),
                        ),
                    }
                }
                Err(reason) => self.note_torch_pip_downloads(log_path, &reason),
            }
            selected_torch = Some(selected);
        }

        let mut held_rest = None;
        let outcome = async {
            let local_torch = held_torch.as_ref().map(|(held, wheel)| held.file(wheel));
            // Phase 1: resolve once (a pip dry run) into a per-install hash
            // lock. The resolver pins the Torch wheel by the URL and SHA-256
            // that the official index publishes for this interpreter.
            let mut resolve = resolver_command(&[]);
            if let Some(selected) = &selected_torch {
                resolve
                    .arg("--torch-wheel")
                    .arg(&selected.url)
                    .arg("--torch-sha256")
                    .arg(&selected.sha256);
            }
            if let Some(local) = &local_torch {
                resolve.arg("--torch-local-wheel").arg(local);
            }
            let constraints = selection
                .verified_constraints
                .clone()
                .into_iter()
                .collect::<Vec<_>>();
            if !constraints.is_empty() {
                std::fs::create_dir_all(&resolver_output).map_err(PumasError::from)?;
                let path = resolver_output.join("install-constraints.txt");
                std::fs::write(&path, constraints.concat()).map_err(PumasError::from)?;
                resolve.arg("--constraints").arg(path);
            }
            let status = self
                .run_runtime_command_status_with_download_progress(
                    resolve,
                    log_path,
                    &format!("Resolving official Torch {build} wheels for Python {minor}"),
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
                    "Torch package resolution failed ({status}); see installation log"
                )));
            }
            let resolution: DirectTorchResolution = serde_json::from_slice(
                &std::fs::read(resolver_output.join("resolution.json"))
                    .map_err(PumasError::from)?,
            )
            .map_err(|error| failed(format!("Invalid Torch installation report: {error}")))?;
            let report: serde_json::Value = serde_json::from_slice(
                &std::fs::read(resolver_output.join("pip-resolution.json"))
                    .map_err(PumasError::from)?,
            )
            .map_err(|error| failed(format!("Invalid Torch pip report: {error}")))?;
            let requirements = std::fs::read_to_string(resolver_output.join("requirements.txt"))
                .map_err(PumasError::from)?;
            let direct_selection = DirectTorchSelection {
                version,
                build,
                minor,
                extras: &selection.extras,
                python: &python,
                local_torch_wheel: local_torch.as_deref(),
            };
            // Nothing is installed until this lock is accepted.
            validate_direct_torch_report(&resolution, &report, &requirements, &direct_selection)?;
            if let Some(selected) = &selected_torch {
                if !resolution.artifacts.iter().any(|artifact| {
                    artifact.name == "torch"
                        && artifact.url == selected.url
                        && artifact.sha256.eq_ignore_ascii_case(&selected.sha256)
                }) {
                    return Err(failed(
                        "Torch resolution differs from the selected Torch wheel",
                    ));
                }
            }

            // Download every other locked wheel through shared acquisition,
            // so the installation below reads only verified local files.
            let mut local_files = std::collections::HashMap::new();
            let mut wheel_dirs = Vec::new();
            if let (Some((held, wheel)), Some(local)) = (&held_torch, &local_torch) {
                local_files.insert(wheel.name.clone(), local.clone());
                wheel_dirs.push(held.directory().to_path_buf());
            }
            if let Some(acquisition) = &wheel_acquisition {
                let rest = resolution
                    .artifacts
                    .iter()
                    .filter(|artifact| !local_files.contains_key(&artifact.name))
                    .map(|artifact| {
                        super::torch_wheels::LockedWheel::new(
                            &artifact.name,
                            &artifact.url,
                            &artifact.sha256,
                        )
                    })
                    .collect::<std::result::Result<Vec<_>, String>>();
                match rest {
                    Ok(rest) if rest.is_empty() => {}
                    Ok(rest) => {
                        self.announce_torch_wheel_download(
                            progress_tx,
                            &format!("Downloading {} verified Torch dependency wheels", rest.len()),
                        )
                        .await;
                        match acquisition.hold(&wheel_selection("dependencies"), rest.clone(), true).await? {
                            Ok(held) => {
                                for wheel in &rest {
                                    local_files.insert(wheel.name.clone(), held.file(wheel));
                                }
                                wheel_dirs.push(held.directory().to_path_buf());
                                held_rest = Some(held);
                            }
                            Err(reason) => self.note_torch_pip_downloads(
                                log_path,
                                &format!("the dependency wheels cannot use it: {reason}"),
                            ),
                        }
                    }
                    Err(reason) => self.note_torch_pip_downloads(
                        log_path,
                        &format!("a dependency wheel cannot use it: {reason}"),
                    ),
                }
            }

            // Phase 2: install exactly the accepted lock with
            // `--require-hashes --no-deps --no-index`, from the acquired
            // local wheels where they exist.
            let installed = async {
                let mut install = resolver_command(&["--install"]);
                install.arg("--target").arg(&packages);
                for directory in &wheel_dirs {
                    install.arg("--wheel-dir").arg(directory);
                }
                let status = self
                    .run_runtime_command_status_with_download_progress(
                        install,
                        log_path,
                        &format!("Installing locked Torch {build} wheels for Python {minor}"),
                        progress_tx,
                        Some(staging.clone()),
                        Some(&download_progress_path),
                    )
                    .await?;
                if !status.success() {
                    return Err(failed(format!(
                        "Torch locked package installation failed ({status}); see installation log"
                    )));
                }
                if std::fs::read_to_string(resolver_output.join("requirements.txt"))
                    .map_err(PumasError::from)?
                    != requirements
                {
                    return Err(failed("Torch hash lock changed during installation"));
                }
                let installed_report: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(resolver_output.join("pip-install.json"))
                        .map_err(PumasError::from)?,
                )
                .map_err(|error| {
                    failed(format!("Invalid Torch pip installation report: {error}"))
                })?;
                validate_locked_install_report(&resolution, &installed_report, &local_files)?;
                // No staged package code runs until the staged files match the
                // manifest checked against each wheel's RECORD.
                let manifest: StagedFilesManifest = serde_json::from_slice(
                    &std::fs::read(resolver_output.join("installed-files.json"))
                        .map_err(PumasError::from)?,
                )
                .map_err(|error| failed(format!("Invalid Torch staged file manifest: {error}")))?;
                validate_and_move_direct_torch_packages(
                    &resolution,
                    &report,
                    &requirements,
                    &direct_selection,
                    &packages,
                    &bootstrap,
                    &manifest,
                )
            }
            .await;
            installed?;
            for name in [
                "requirements.txt",
                "resolution.json",
                "pip-resolution.json",
                "pip-install.json",
                "installed-files.json",
            ] {
                std::fs::copy(resolver_output.join(name), runtime.join(name))
                    .map_err(PumasError::from)?;
            }
            let extras_label = if selection.extras.is_empty() {
                "general".to_owned()
            } else {
                selection.extras.join("+")
            };
            let mut recipe = serde_json::json!({
                "recipe_id": format!("upstream-install-{}-{}-{}-{}", selection.tag, build, minor, extras_label),
                "protocol": SUPPORTED_TORCH_PROTOCOL,
                "capabilities": [],
                "qualification": "not verified by Pumas",
                "build": build,
                "python": format!("python{minor}"),
                "modalities": resolution.modalities,
                "omitted_modalities": resolution.omitted_modalities,
                "extras": selection.extras,
                "managed_python": managed_python_identity_record(&managed_python, &interpreter_hash),
                "artifacts": resolution.artifacts,
            });
            std::fs::write(
                runtime.join("runtime.json"),
                serde_json::to_vec_pretty(&recipe).map_err(|error| failed(error.to_string()))?,
            )
            .map_err(PumasError::from)?;
            let mut probe = runtime_check_command(&python, &sidecar.join("probe_runtime.py"));
            probe.arg("--runtime").arg(&runtime);
            self.run_runtime_command(
                probe,
                log_path,
                "Checking installed Torch identity and CPU operation",
                progress_tx,
                Some(staging.clone()),
            )
            .await?;
            // Record what the probe found importable, not what was selected.
            let probed: serde_json::Value = serde_json::from_slice(
                &std::fs::read(runtime.join("probe-results.json")).map_err(PumasError::from)?,
            )
            .map_err(|error| failed(format!("Invalid Torch probe results: {error}")))?;
            recipe["capabilities"] = serde_json::json!(serving_capabilities(&probed["capabilities"]));
            std::fs::write(
                runtime.join("runtime.json"),
                serde_json::to_vec_pretty(&recipe).map_err(|error| failed(error.to_string()))?,
            )
            .map_err(PumasError::from)?;
            Ok(DirectTorchAttempt::Installed(runtime.clone()))
        }
        .await;
        // Both sets stay held until the attempt's outcome, including the
        // runtime probe, is known; a failed attempt keeps them both for the
        // next try. A retry moves to another wheel, so its set is discarded.
        let usage = match &outcome {
            Ok(DirectTorchAttempt::Retry) => super::torch_wheels::WheelUse::Discard,
            outcome => self.torch_wheel_use(outcome.is_ok()),
        };
        let mut outcome = outcome;
        for held in [held_rest, held_torch.map(|(held, _)| held)]
            .into_iter()
            .flatten()
        {
            outcome = settled_result(outcome, held.finish(usage).await);
        }
        outcome
    }

    pub(super) async fn install_torch_runtime(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
        selection: Option<super::super::torch_preview::TorchInstallSelection>,
    ) -> Result<()> {
        let _attempt = self
            .torch_attempt_lock
            .try_lock()
            .map_err(|_| failed("Torch installation already active"))?;
        self.torch_control.start();
        let result = self
            .install_torch_runtime_inner(tag, release, progress_tx, selection)
            .await;
        self.torch_control.finish();
        result
    }

    async fn install_torch_runtime_inner(
        &self,
        tag: &str,
        release: &GitHubRelease,
        progress_tx: mpsc::Sender<ProgressUpdate>,
        selection: Option<super::super::torch_preview::TorchInstallSelection>,
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
        let selection = selection.as_ref();
        if selection.is_some_and(|selection| selection.tag != tag) {
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
        if selection.is_none() {
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
            .stage_torch_runtime(selection, &staging, &log_path, &progress_tx)
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
                            None,
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
        selection: Option<&super::super::torch_preview::TorchInstallSelection>,
        staging: &std::sync::Arc<TorchPendingStage>,
        log_path: &Path,
        progress_tx: &mpsc::Sender<ProgressUpdate>,
    ) -> Result<PathBuf> {
        #[cfg(test)]
        if let Some(stage) = &self.torch_stage_override {
            return stage(staging.path());
        }
        if let Some(selection) = selection {
            return self
                .stage_selected_torch_runtime(selection, staging, log_path, progress_tx)
                .await;
        }
        Err(failed(
            "A retained preview is required for this Torch runtime",
        ))
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
    fn macos_pypi_torch_wheels_require_matching_identity_and_platform() {
        let source = "https://files.pythonhosted.org/packages/ee/90/torch-2.14.0-cp314-cp314-macosx_14_0_arm64.whl";
        let url = reqwest::Url::parse(source).unwrap();
        assert!(trusted_macos_pypi_torch_wheel(
            &url, "torch", "2.14.0", true
        ));
        assert!(!trusted_macos_pypi_torch_wheel(
            &url, "torch", "2.14.0", false
        ));
        for altered in [
            source.replace("torch-2.14.0", "torch-2.13.0"),
            source.replace("torch-2.14.0", "other-2.14.0"),
            source.replace("macosx_14_0_arm64", "manylinux_2_28_x86_64"),
            source.replace("files.pythonhosted.org", "files.pythonhosted.org.evil.test"),
        ] {
            let altered = reqwest::Url::parse(&altered).unwrap();
            assert!(!trusted_macos_pypi_torch_wheel(
                &altered, "torch", "2.14.0", true
            ));
        }
    }

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
            modalities: Vec::new(),
            omitted_modalities: ["text", "image", "audio"]
                .into_iter()
                .map(|modality| (modality.to_owned(), "fixture resolves core only".to_owned()))
                .collect(),
            extras: Vec::new(),
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
        let selection = DirectTorchSelection {
            version: "2.14.0",
            build: "cpu",
            minor: "3.12",
            extras: &[],
            python,
            local_torch_wheel: None,
        };
        let check = |resolution: &DirectTorchResolution, report: &serde_json::Value, lock: &str| {
            validate_direct_torch_report(resolution, report, lock, &selection)
        };
        assert!(check(&resolution, &report, &requirements).is_ok());

        // Resolved against the acquired Torch wheel: pip reports that exact
        // local file, while the lock keeps the official URL.
        let acquired = tempfile::tempdir().unwrap();
        let local = acquired.path().join("torch-fixture.whl");
        std::fs::write(&local, b"verified wheel").unwrap();
        let other = acquired.path().join("other.whl");
        std::fs::write(&other, b"other wheel").unwrap();
        let local_selection = DirectTorchSelection {
            local_torch_wheel: Some(&local),
            ..selection
        };
        let mut local_report = report.clone();
        local_report["install"][0]["download_info"]["url"] = reqwest::Url::from_file_path(&local)
            .unwrap()
            .as_str()
            .into();
        validate_direct_torch_report(&resolution, &local_report, &requirements, &local_selection)
            .unwrap();
        // The official URL is not the local file, and neither is another file.
        assert!(validate_direct_torch_report(
            &resolution,
            &report,
            &requirements,
            &local_selection
        )
        .is_err());
        local_report["install"][0]["download_info"]["url"] = reqwest::Url::from_file_path(&other)
            .unwrap()
            .as_str()
            .into();
        assert!(validate_direct_torch_report(
            &resolution,
            &local_report,
            &requirements,
            &local_selection
        )
        .is_err());
        // Only Torch may come from a local file during resolution.
        let mut local_dependency = report.clone();
        local_dependency["install"][1]["download_info"]["url"] =
            reqwest::Url::from_file_path(&local)
                .unwrap()
                .as_str()
                .into();
        assert!(check(&resolution, &local_dependency, &requirements).is_err());
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
        std::fs::create_dir_all(venv_packages_dir(&runtime, "3.12")).unwrap();
        let bootstrap = BootstrapPackages::capture(&runtime, "3.12").unwrap();
        let publish = |lock: &str| {
            validate_and_move_direct_torch_packages(
                &resolution,
                &report,
                lock,
                &selection,
                &target,
                &bootstrap,
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
    fn interpreter_identity_survives_alternate_spellings_of_one_file() {
        let root = tempfile::tempdir().unwrap();
        let python = root.path().join("python");
        std::fs::write(&python, b"interpreter").unwrap();
        let other = root.path().join("other");
        std::fs::write(&other, b"interpreter").unwrap();
        // `..` survives Path comparison (an interior `.` would not), so only the
        // resolved-file fallback can match this spelling, as with 8.3 names.
        std::fs::create_dir(root.path().join("bin")).unwrap();
        let indirect = root.path().join("bin").join("..").join("python");
        assert_ne!(indirect, python);
        assert!(same_interpreter(&python, &python));
        assert!(same_interpreter(&indirect, &python));
        assert!(!same_interpreter(&other, &python));
        assert!(!same_interpreter(&root.path().join("missing"), &python));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn general_modality_report_is_complete_before_staged_publication() {
        let names = [
            "torch",
            "fastapi",
            "uvicorn",
            "psutil",
            "pillow",
            "safetensors",
            "transformers",
            "tokenizers",
            "huggingface-hub",
            "jinja2",
            "sentencepiece",
            "protobuf",
            "accelerate",
            "librosa",
            "soundfile",
            "soxr",
            "numpy",
            "scipy",
            "numba",
            "llvmlite",
        ];
        let mut resolution = DirectTorchResolution {
            release: "2.14.0".into(),
            torch: "2.14.0+cpu".into(),
            build: "cpu".into(),
            python: "3.12".into(),
            interpreter: "/fixture/python".into(),
            implementation: "cpython".into(),
            platform: "Linux-fixture".into(),
            machine: "x86_64".into(),
            // The image packages had no compatible wheel here.
            modalities: vec!["text".into(), "audio".into()],
            omitted_modalities: [(
                "image".to_owned(),
                "No compatible binary wheel for torchvision".to_owned(),
            )]
            .into_iter()
            .collect(),
            extras: Vec::new(),
            artifacts: names
                .iter()
                .map(|name| crate::version_manager::TorchArtifact {
                    name: (*name).into(),
                    version: if *name == "torch" {
                        "2.14.0+cpu"
                    } else {
                        "1.0.0"
                    }
                    .into(),
                    url: if *name == "torch" {
                        "https://download.pytorch.org/whl/cpu/torch/torch-fixture.whl".into()
                    } else {
                        format!("https://files.pythonhosted.org/packages/{name}-fixture.whl")
                    },
                    sha256: "a".repeat(64),
                })
                .collect(),
        };
        let selection = DirectTorchSelection {
            version: "2.14.0",
            build: "cpu",
            minor: "3.12",
            extras: &[],
            python: Path::new("/fixture/python"),
            local_torch_wheel: None,
        };
        let records = |resolution: &DirectTorchResolution| {
            let report = serde_json::json!({"install":resolution.artifacts.iter().map(|a| serde_json::json!({"metadata":{"name":a.name,"version":a.version},"download_info":{"url":a.url,"archive_info":{"hashes":{"sha256":a.sha256}}}})).collect::<Vec<_>>()});
            let lock = resolution
                .artifacts
                .iter()
                .map(|a| format!("{} @ {} --hash=sha256:{}", a.name, a.url, a.sha256))
                .collect::<Vec<_>>()
                .join("\n");
            (report, lock)
        };
        let check = |resolution: &DirectTorchResolution| {
            let (report, lock) = records(resolution);
            validate_direct_torch_report(resolution, &report, &lock, &selection)
        };
        check(&resolution).unwrap();

        // A resolved modality must bring all of its packages.
        let mut incomplete = resolution.artifacts.clone();
        incomplete.retain(|artifact| artifact.name != "soundfile");
        let complete = std::mem::replace(&mut resolution.artifacts, incomplete);
        assert!(check(&resolution)
            .unwrap_err()
            .to_string()
            .contains("audio dependency"));
        resolution.artifacts = complete;

        // A modality is resolved or left out with a reason, never silently.
        let reason = resolution.omitted_modalities.remove("image").unwrap();
        assert!(check(&resolution).is_err());
        resolution
            .omitted_modalities
            .insert("image".into(), "  ".into());
        assert!(check(&resolution).is_err());
        resolution.omitted_modalities.insert("image".into(), reason);

        // Torchvision belongs to the image modality.
        let mut with_vision = resolution.clone();
        with_vision
            .artifacts
            .push(crate::version_manager::TorchArtifact {
                name: "torchvision".into(),
                version: "0.29.0+cpu".into(),
                url: "https://download.pytorch.org/whl/cpu/torchvision-fixture.whl".into(),
                sha256: "a".repeat(64),
            });
        assert!(check(&with_vision).is_err());

        // A requested extra must be in the report.
        let nunchaku = ["nunchaku".to_owned()];
        let extra_selection = DirectTorchSelection {
            extras: &nunchaku,
            ..selection
        };
        let mut with_extra = resolution.clone();
        with_extra.extras = nunchaku.to_vec();
        let (report, lock) = records(&with_extra);
        assert!(
            validate_direct_torch_report(&with_extra, &report, &lock, &extra_selection)
                .unwrap_err()
                .to_string()
                .contains("extra")
        );

        // A complete report stages its packages.
        let directory = tempfile::tempdir().unwrap();
        let runtime = directory.path().join("runtime");
        let target = runtime.join("staged-packages");
        std::fs::create_dir_all(&target).unwrap();
        let bytes = b"controlled inert wheel member";
        std::fs::write(target.join("selected.py"), bytes).unwrap();
        let manifest = StagedFilesManifest {
            files: vec![StagedFile {
                path: "selected.py".into(),
                sha256: format!("{:x}", Sha256::digest(bytes)),
                size: bytes.len() as u64,
            }],
        };
        std::fs::create_dir_all(venv_packages_dir(&runtime, "3.12")).unwrap();
        let bootstrap = BootstrapPackages::capture(&runtime, "3.12").unwrap();
        let (report, lock) = records(&resolution);
        validate_and_move_direct_torch_packages(
            &resolution,
            &report,
            &lock,
            &selection,
            &target,
            &bootstrap,
            &manifest,
        )
        .unwrap();
        assert!(runtime
            .join("venv/lib/python3.12/site-packages/selected.py")
            .exists());
    }

    #[test]
    fn modality_packages_match_the_resolver() {
        let resolver = include_str!("../../../../../../torch-server/resolve_runtime.py");
        let tuple = |name: &str| {
            let start = resolver
                .find(&format!("\n{name} = ("))
                .unwrap_or_else(|| panic!("resolver defines {name}"));
            let body = &resolver[start..];
            let end = body.find(')').unwrap();
            body[..end]
                .split('"')
                .skip(1)
                .step_by(2)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        for (modality, constant) in [("text", "TEXT"), ("image", "IMAGE"), ("audio", "AUDIO")] {
            let own = MODALITY_PACKAGES
                .iter()
                .find(|(name, _)| *name == modality)
                .unwrap()
                .1;
            assert_eq!(own.to_vec(), tuple(constant), "{modality}");
        }
        assert!(modality_packages("image").any(|name| name == "transformers"));
        assert!(modality_packages("audio").any(|name| name == "jinja2"));
        assert!(!modality_packages("text").any(|name| name == "diffusers"));
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

    #[test]
    fn bootstrap_cleanup_refuses_namespace_or_binding_changes_before_removal() {
        for mutation in ["extra", "bytecode", "replacement", "missing", "directory"] {
            let root = tempfile::tempdir().unwrap();
            let runtime = root.path().join("runtime");
            let packages = venv_packages_dir(&runtime, "3.12");
            std::fs::create_dir_all(packages.join("pip")).unwrap();
            std::fs::write(packages.join("pip/__init__.py"), b"bootstrap").unwrap();
            std::fs::write(packages.join("pip/retained.py"), b"must survive refusal").unwrap();
            let bootstrap = BootstrapPackages::capture(&runtime, "3.12").unwrap();
            match mutation {
                "extra" => std::fs::write(packages.join("unexpected.py"), b"unreported").unwrap(),
                "bytecode" => {
                    std::fs::write(packages.join("pip/new.pyc"), b"unreported cache").unwrap()
                }
                "replacement" => {
                    let replacement = packages.join("pip/successor.py");
                    std::fs::write(&replacement, b"bootstrap").unwrap();
                    std::fs::rename(replacement, packages.join("pip/__init__.py")).unwrap();
                }
                "missing" => std::fs::remove_file(packages.join("pip/__init__.py")).unwrap(),
                _ => std::fs::create_dir(packages.join("unexpected-empty")).unwrap(),
            }
            assert!(bootstrap.remove().is_err(), "{mutation}");
            assert_eq!(
                std::fs::read(packages.join("pip/retained.py")).unwrap(),
                b"must survive refusal"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_cleanup_refuses_linked_files_and_rebound_package_roots() {
        for root_alias in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let runtime = root.path().join("runtime");
            let packages = venv_packages_dir(&runtime, "3.12");
            std::fs::create_dir_all(packages.join("pip")).unwrap();
            std::fs::write(packages.join("pip/__init__.py"), b"bootstrap").unwrap();
            let bootstrap = BootstrapPackages::capture(&runtime, "3.12").unwrap();
            if root_alias {
                let retained = root.path().join("retained-packages");
                std::fs::rename(&packages, &retained).unwrap();
                std::os::unix::fs::symlink(&retained, &packages).unwrap();
            } else {
                std::fs::write(packages.join("pip/retained.py"), b"bootstrap").unwrap();
                std::fs::remove_file(packages.join("pip/__init__.py")).unwrap();
                std::os::unix::fs::symlink("retained.py", packages.join("pip/__init__.py"))
                    .unwrap();
            }
            assert!(bootstrap.remove().is_err());
            assert!(packages.join("pip/__init__.py").exists());
        }
    }

    #[test]
    fn staged_files_are_compared_by_content_not_size() {
        let target = tempfile::tempdir().unwrap();
        let contents = b"VALUE = 'genuine wheel member'\n";
        std::fs::write(target.path().join("member.py"), contents).unwrap();
        let manifest = StagedFilesManifest {
            files: vec![StagedFile {
                path: "member.py".into(),
                sha256: format!("{:x}", Sha256::digest(contents)),
                size: contents.len() as u64,
            }],
        };
        validate_staged_files(target.path(), &manifest).unwrap();

        // Same length, different bytes: a size comparison would accept it.
        let tampered = b"VALUE = 'altered wheel member'\n";
        assert_eq!(tampered.len(), contents.len());
        std::fs::write(target.path().join("member.py"), tampered).unwrap();
        let error = validate_staged_files(target.path(), &manifest)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("differs from its manifest: member.py"),
            "{error}"
        );
    }

    #[test]
    fn verified_version_choice_is_its_own_wheel_selection() {
        let select = |constraints: Option<&str>, role: &str| {
            torch_wheel_selection("v2.9.1", "cu130", "3.12", &[], constraints, role)
        };
        let current = select(None, "dependencies");
        let verified = select(Some("transformers==4.57.6\n"), "dependencies");
        // Toggling recorded versions must not supersede the other choice.
        assert_ne!(current, verified);
        assert_ne!(
            verified,
            select(Some("transformers==4.57.7\n"), "dependencies")
        );
        // Retrying the same choice resumes the same selection.
        assert_eq!(
            verified,
            select(Some("transformers==4.57.6\n"), "dependencies")
        );
        assert_ne!(current, select(None, "torch"));
    }

    #[test]
    fn locked_install_report_must_install_exactly_the_accepted_lock() {
        let artifact = |name: &str, sha: char| crate::version_manager::TorchArtifact {
            name: name.into(),
            version: "1.0".into(),
            url: format!("https://files.pythonhosted.org/packages/{name}-1.0.whl"),
            sha256: sha.to_string().repeat(64),
        };
        let resolution = DirectTorchResolution {
            release: "2.14.0".into(),
            torch: "2.14.0+cpu".into(),
            build: "cpu".into(),
            python: "3.12".into(),
            interpreter: "/staged/venv/python".into(),
            implementation: "cpython".into(),
            platform: "Linux-fixture".into(),
            machine: "x86_64".into(),
            modalities: Vec::new(),
            omitted_modalities: Default::default(),
            extras: Vec::new(),
            artifacts: vec![artifact("torch", 'a'), artifact("fastapi", 'b')],
        };
        let report_of = |artifacts: &[crate::version_manager::TorchArtifact]| {
            serde_json::json!({
                "install": artifacts.iter().rev().map(|artifact| serde_json::json!({
                    "metadata": {"name": artifact.name, "version": artifact.version},
                    "download_info": {"url": artifact.url, "archive_info": {"hashes": {"sha256": artifact.sha256}}}
                })).collect::<Vec<_>>()
            })
        };
        let remote = std::collections::HashMap::new();
        // Order does not matter; content does.
        validate_locked_install_report(&resolution, &report_of(&resolution.artifacts), &remote)
            .unwrap();
        for changed in [
            vec![artifact("torch", 'a'), artifact("fastapi", 'c')],
            vec![artifact("torch", 'a')],
            vec![
                artifact("torch", 'a'),
                artifact("fastapi", 'b'),
                artifact("extra", 'd'),
            ],
        ] {
            assert!(
                validate_locked_install_report(&resolution, &report_of(&changed), &remote).is_err()
            );
        }

        // Acquired wheels must install from exactly their verified local files.
        let acquired = tempfile::tempdir().unwrap();
        let local = |name: &str| {
            let path = acquired.path().join(format!("{name}-1.0.whl"));
            std::fs::write(&path, name).unwrap();
            path
        };
        let local_files = std::collections::HashMap::from([
            ("torch".to_owned(), local("torch")),
            ("fastapi".to_owned(), local("fastapi")),
        ]);
        let from_files = |files: &std::collections::HashMap<String, PathBuf>| {
            let mut artifacts = resolution.artifacts.clone();
            for artifact in &mut artifacts {
                if let Some(path) = files.get(&artifact.name) {
                    artifact.url = reqwest::Url::from_file_path(path).unwrap().into();
                }
            }
            report_of(&artifacts)
        };
        validate_locked_install_report(&resolution, &from_files(&local_files), &local_files)
            .unwrap();
        // A network install of an acquired wheel is refused...
        assert!(validate_locked_install_report(
            &resolution,
            &report_of(&resolution.artifacts),
            &local_files
        )
        .is_err());
        // ...and so is any local file the acquisition did not provide.
        let swapped = std::collections::HashMap::from([
            ("torch".to_owned(), local_files["fastapi"].clone()),
            ("fastapi".to_owned(), local_files["torch"].clone()),
        ]);
        assert!(
            validate_locked_install_report(&resolution, &from_files(&swapped), &local_files)
                .is_err()
        );
        // A partly acquired set keeps network sources for the rest.
        let torch_only =
            std::collections::HashMap::from([("torch".to_owned(), local_files["torch"].clone())]);
        validate_locked_install_report(&resolution, &from_files(&torch_only), &torch_only).unwrap();
        assert!(validate_locked_install_report(
            &resolution,
            &from_files(&local_files),
            &torch_only
        )
        .is_err());
    }
}
