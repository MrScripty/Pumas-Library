//! Atomic file operations for safe JSON persistence.
//!
//! Implements atomic writes using:
//! 1. Exclusively create a collision-resistant staging file
//! 2. Request file-data synchronization before replacement
//! 3. Atomic rename to target path
//! 4. Optional backup creation

#![warn(unsafe_code)]

use crate::{PumasError, Result};
use cap_std::fs::{Dir, OpenOptions as CapOpenOptions};
use serde::{de::DeserializeOwned, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

/// Cleanup state for a staging file owned by a durable publication attempt.
#[derive(Debug)]
#[cfg_attr(
    not(any(unix, windows)),
    allow(
        dead_code,
        reason = "unsupported target publication refuses admission; retain the shared durability contract"
    )
)]
pub(crate) enum StagingCleanup {
    NotRequired,
    Removed,
    Failed { error: PumasError },
}

/// A failure that happened before rename, while the old target was unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicPublishStage {
    #[cfg_attr(
        any(unix, windows),
        allow(dead_code, reason = "constructed by the unsupported target adapter")
    )]
    TargetAdmission,
    Serialization,
    #[cfg_attr(
        not(any(unix, windows)),
        allow(
            dead_code,
            reason = "unsupported target publication refuses admission; retain the shared durability contract"
        )
    )]
    Staging,
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicPublishFailureKind {
    #[cfg_attr(
        any(unix, windows),
        allow(dead_code, reason = "constructed by the unsupported target adapter")
    )]
    TargetUnavailable,
    InvalidData,
    Filesystem,
}

#[derive(Debug)]
pub(crate) struct AtomicPublishFailure {
    pub(crate) stage: AtomicPublishStage,
    pub(crate) kind: AtomicPublishFailureKind,
    pub(crate) error: PumasError,
    pub(crate) cleanup: StagingCleanup,
}

impl AtomicPublishFailure {
    pub(crate) fn into_error(self) -> PumasError {
        let Self {
            stage,
            kind,
            error,
            cleanup,
        } = self;
        debug!(?stage, ?kind, "Converting atomic publication failure");
        match cleanup {
            StagingCleanup::Failed { error: cleanup } => PumasError::Other(format!(
                "{}; staging cleanup also failed: {}",
                error, cleanup
            )),
            StagingCleanup::NotRequired | StagingCleanup::Removed => error,
        }
    }
}

/// Publication result for callers that require explicit durability classification.
#[derive(Debug)]
#[cfg_attr(
    not(any(unix, windows)),
    allow(
        dead_code,
        reason = "unsupported target publication refuses admission; retain the shared durability contract"
    )
)]
pub(crate) enum AtomicPublication {
    /// File contents and the held parent directory entry were synchronized.
    Durable,
    /// Rename succeeded, but synchronization of the held parent failed.
    PublishedDurabilityUnknown { error: PumasError },
    /// Rename or configured-parent identity had an ambiguous visible result.
    VisibilityUnknown {
        error: PumasError,
        cleanup: StagingCleanup,
    },
}

pub(crate) type AtomicPublishResult =
    std::result::Result<AtomicPublication, Box<AtomicPublishFailure>>;

#[cfg(any(not(any(unix, windows)), test))]
fn target_unavailable_failure(path: &Path) -> Box<AtomicPublishFailure> {
    Box::new(AtomicPublishFailure {
        stage: AtomicPublishStage::TargetAdmission,
        kind: AtomicPublishFailureKind::TargetUnavailable,
        error: PumasError::Io {
            message: "Durable JSON publication is unavailable on this target".to_string(),
            path: Some(path.to_path_buf()),
            source: Some(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "durable directory publication is unavailable on this target",
            )),
        },
        cleanup: StagingCleanup::NotRequired,
    })
}

/// Held authority for one JSON file in an already-existing parent directory.
pub(crate) struct AtomicJsonTarget {
    parent: Dir,
    #[cfg_attr(
        not(any(unix, windows)),
        allow(
            dead_code,
            reason = "unsupported target publication refuses admission; retain the shared durability contract"
        )
    )]
    parent_sync_file: File,
    parent_path: PathBuf,
    #[cfg_attr(
        not(any(unix, windows)),
        allow(
            dead_code,
            reason = "unsupported target publication refuses admission; retain the shared durability contract"
        )
    )]
    parent_identity: ParentIdentity,
    name: OsString,
    display_path: PathBuf,
    #[cfg_attr(
        not(any(unix, windows)),
        allow(
            dead_code,
            reason = "unsupported target publication refuses admission; retain the shared durability contract"
        )
    )]
    capability_validation: Option<Box<dyn Fn() -> Result<bool> + Send + Sync>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentIdentity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows { volume: u32, file: u64 },
    #[cfg(not(any(unix, windows)))]
    Unsupported,
}

#[cfg_attr(
    not(any(unix, windows)),
    allow(
        dead_code,
        reason = "unsupported target publication refuses admission; retain the shared durability contract"
    )
)]
trait DurablePublicationAdapter {
    fn temp_name(&self, target: &OsStr, attempt: u8) -> OsString;

    fn write_and_sync(&self, file: &mut cap_std::fs::File, contents: &[u8]) -> std::io::Result<()>;

    fn rename(&self, parent: &Dir, source: &OsStr, target: &OsStr) -> std::io::Result<()>;

    fn remove_file(&self, parent: &Dir, name: &OsStr) -> std::io::Result<()>;

    fn sync_parent(&self, parent: &File) -> std::io::Result<()>;
}

struct OsDurablePublicationAdapter;

impl DurablePublicationAdapter for OsDurablePublicationAdapter {
    fn temp_name(&self, target: &OsStr, attempt: u8) -> OsString {
        let mut name = target.to_os_string();
        name.push(format!(".{}.{}.tmp", Uuid::new_v4(), attempt));
        name
    }

    fn write_and_sync(&self, file: &mut cap_std::fs::File, contents: &[u8]) -> std::io::Result<()> {
        file.write_all(contents)?;
        file.flush()?;
        file.sync_all()
    }

    fn rename(&self, parent: &Dir, source: &OsStr, target: &OsStr) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            crate::platform::capability_fs::rename_at(parent, source, parent, target, true)
        }
        #[cfg(not(windows))]
        {
            parent.rename(source, parent, target)
        }
    }

    fn remove_file(&self, parent: &Dir, name: &OsStr) -> std::io::Result<()> {
        parent.remove_file(name)
    }

    fn sync_parent(&self, parent: &File) -> std::io::Result<()> {
        #[cfg(any(unix, windows))]
        {
            crate::platform::capability_fs::sync_directory_file(parent)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = parent;
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "durable directory publication is unavailable on this target",
            ))
        }
    }
}

impl AtomicJsonTarget {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let parent_path = publication_parent(path).to_path_buf();
        let name = path
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| PumasError::Other("Atomic JSON target needs a file name".to_string()))?
            .to_os_string();
        #[cfg(unix)]
        let (parent, parent_sync_file) = {
            let parent_sync_file = File::open(&parent_path).map_err(|source| PumasError::Io {
                message: format!(
                    "Failed to open existing parent directory {}",
                    parent_path.display()
                ),
                path: Some(parent_path.clone()),
                source: Some(source),
            })?;
            let parent = Dir::from_std_file(
                parent_sync_file
                    .try_clone()
                    .map_err(|source| parent_io_error(&parent_path, "clone", source))?,
            );
            (parent, parent_sync_file)
        };
        #[cfg(not(unix))]
        let (parent, parent_sync_file) = {
            let parent =
                crate::platform::capability_fs::open_directory(&parent_path).map_err(|source| {
                    PumasError::Io {
                        message: format!(
                            "Failed to open existing parent directory {}",
                            parent_path.display()
                        ),
                        path: Some(parent_path.clone()),
                        source: Some(source),
                    }
                })?;
            let parent_sync_file = parent
                .try_clone()
                .map_err(|source| parent_io_error(&parent_path, "clone", source))?
                .into_std_file();
            (parent, parent_sync_file)
        };
        let parent_identity = parent_identity_from_file(&parent_sync_file, &parent_path)?;
        Ok(Self {
            parent,
            parent_sync_file,
            parent_path,
            parent_identity,
            name,
            display_path: path.to_path_buf(),
            capability_validation: None,
        })
    }

    /// Construct a publisher from already-held directory authority. The validator
    /// compares the configured capability chain with this exact parent; it must
    /// never obtain effect authority by reopening the display path.
    pub(crate) fn from_capability(
        parent: Dir,
        name: &OsStr,
        display_path: PathBuf,
        validate: impl Fn() -> Result<bool> + Send + Sync + 'static,
    ) -> Result<Self> {
        if Path::new(name).components().count() != 1
            || !matches!(
                Path::new(name).components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(PumasError::Other("Invalid publication filename".into()));
        }
        let parent_path = publication_parent(&display_path).to_path_buf();
        // cap-std directory handles may be O_PATH descriptors on Linux. Open
        // the held directory itself for syncing, without an ambient path.
        let parent_sync_file = parent.open(".")?.into_std();
        let parent_identity = parent_identity_from_file(&parent_sync_file, &parent_path)?;
        if !validate()? {
            return Err(PumasError::Other("Publication authority changed".into()));
        }
        Ok(Self {
            parent,
            parent_sync_file,
            parent_path,
            parent_identity,
            name: name.to_os_string(),
            display_path,
            capability_validation: Some(Box::new(validate)),
        })
    }

    pub(crate) fn read_json<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        let mut file = match self.parent.open(&self.name) {
            Ok(file) => file.into_std(),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(PumasError::Io {
                    message: format!("Failed to open {}", self.display_path.display()),
                    path: Some(self.display_path.clone()),
                    source: Some(source),
                });
            }
        };
        read_json_file(&mut file, &self.display_path).map(Some)
    }

    pub(crate) fn open_lock_file(&self, name: &str) -> Result<File> {
        let mut options = CapOpenOptions::new();
        options.read(true).write(true).create(true);
        self.parent
            .open_with(name, &options)
            .map(|file| file.into_std())
            .map_err(|source| PumasError::Io {
                message: format!(
                    "Failed to open store lock {} in {}",
                    name,
                    self.parent_path.display()
                ),
                path: Some(self.parent_path.join(name)),
                source: Some(source),
            })
    }

    pub(crate) fn publish_json<T: Serialize>(&self, data: &T) -> AtomicPublishResult {
        self.publish_json_with_adapter(data, &OsDurablePublicationAdapter)
    }

    fn publish_json_with_adapter<T: Serialize>(
        &self,
        data: &T,
        adapter: &impl DurablePublicationAdapter,
    ) -> AtomicPublishResult {
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (data, adapter);
            return Err(target_unavailable_failure(&self.display_path));
        }

        #[cfg(any(unix, windows))]
        {
            if let Some(validate) = &self.capability_validation {
                match validate() {
                    Ok(true) => {}
                    result => {
                        return Err(Box::new(AtomicPublishFailure {
                            stage: AtomicPublishStage::TargetAdmission,
                            kind: AtomicPublishFailureKind::TargetUnavailable,
                            error: result.err().unwrap_or_else(|| {
                                PumasError::Other("Publication authority changed".into())
                            }),
                            cleanup: StagingCleanup::NotRequired,
                        }));
                    }
                }
            }
            let serialized = serde_json::to_string_pretty(data).map_err(|source| {
                Box::new(AtomicPublishFailure {
                    stage: AtomicPublishStage::Serialization,
                    kind: AtomicPublishFailureKind::InvalidData,
                    error: PumasError::Json {
                        message: format!("Failed to serialize data: {source}"),
                        source: Some(source),
                    },
                    cleanup: StagingCleanup::NotRequired,
                })
            })?;
            serde_json::from_str::<serde_json::Value>(&serialized).map_err(|source| {
                Box::new(AtomicPublishFailure {
                    stage: AtomicPublishStage::Serialization,
                    kind: AtomicPublishFailureKind::InvalidData,
                    error: PumasError::Json {
                        message: format!("JSON validation failed: {source}"),
                        source: Some(source),
                    },
                    cleanup: StagingCleanup::NotRequired,
                })
            })?;

            let (temp_name, mut temp_file) = self.create_owned_staging(adapter)?;
            if let Err(source) = adapter.write_and_sync(&mut temp_file, serialized.as_bytes()) {
                drop(temp_file);
                return Err(Box::new(AtomicPublishFailure {
                    stage: AtomicPublishStage::Staging,
                    kind: AtomicPublishFailureKind::Filesystem,
                    error: PumasError::Io {
                        message: format!(
                            "Failed to write and sync staging file for {}",
                            self.display_path.display()
                        ),
                        path: Some(self.display_path.clone()),
                        source: Some(source),
                    },
                    cleanup: self.cleanup_staging(adapter, &temp_name),
                }));
            }
            drop(temp_file);

            if let Err(source) = adapter.rename(&self.parent, &temp_name, &self.name) {
                return Ok(AtomicPublication::VisibilityUnknown {
                    error: PumasError::Io {
                        message: format!(
                            "Rename result is ambiguous while publishing {}",
                            self.display_path.display()
                        ),
                        path: Some(self.display_path.clone()),
                        source: Some(source),
                    },
                    cleanup: self.cleanup_staging(adapter, &temp_name),
                });
            }

            if let Err(source) = adapter.sync_parent(&self.parent_sync_file) {
                let sync_error = parent_io_error(&self.parent_path, "sync", source);
                return match self.configured_parent_still_matches() {
                    Ok(true) => {
                        Ok(AtomicPublication::PublishedDurabilityUnknown { error: sync_error })
                    }
                    Ok(false) => Ok(AtomicPublication::VisibilityUnknown {
                        error: PumasError::Other(format!(
                            "Configured parent identity changed while parent sync was uncertain for {}: {}",
                            self.display_path.display(),
                            sync_error
                        )),
                        cleanup: StagingCleanup::NotRequired,
                    }),
                    Err(identity_error) => Ok(AtomicPublication::VisibilityUnknown {
                        error: PumasError::Other(format!(
                            "Could not verify configured parent identity while parent sync was uncertain for {}: {}; {}",
                            self.display_path.display(),
                            sync_error,
                            identity_error
                        )),
                        cleanup: StagingCleanup::NotRequired,
                    }),
                };
            }

            match self.configured_parent_still_matches() {
                Ok(true) => {
                    debug!("Durably published {}", self.display_path.display());
                    Ok(AtomicPublication::Durable)
                }
                Ok(false) => Ok(AtomicPublication::VisibilityUnknown {
                    error: PumasError::Other(format!(
                        "Configured parent identity changed while publishing {}",
                        self.display_path.display()
                    )),
                    cleanup: StagingCleanup::NotRequired,
                }),
                Err(error) => Ok(AtomicPublication::VisibilityUnknown {
                    error,
                    cleanup: StagingCleanup::NotRequired,
                }),
            }
        }
    }

    #[cfg(any(unix, windows))]
    fn create_owned_staging(
        &self,
        adapter: &impl DurablePublicationAdapter,
    ) -> std::result::Result<(OsString, cap_std::fs::File), Box<AtomicPublishFailure>> {
        for attempt in 0..16 {
            let name = adapter.temp_name(&self.name, attempt);
            let mut options = CapOpenOptions::new();
            options.write(true).create_new(true);
            match self.parent.open_with(&name, &options) {
                Ok(file) => return Ok((name, file)),
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(Box::new(AtomicPublishFailure {
                        stage: AtomicPublishStage::Staging,
                        kind: AtomicPublishFailureKind::Filesystem,
                        error: PumasError::Io {
                            message: format!(
                                "Failed to create unique staging file for {}",
                                self.display_path.display()
                            ),
                            path: Some(self.display_path.clone()),
                            source: Some(source),
                        },
                        cleanup: StagingCleanup::NotRequired,
                    }));
                }
            }
        }
        Err(Box::new(AtomicPublishFailure {
            stage: AtomicPublishStage::Staging,
            kind: AtomicPublishFailureKind::Filesystem,
            error: PumasError::Other(format!(
                "Could not allocate a unique staging file for {}",
                self.display_path.display()
            )),
            cleanup: StagingCleanup::NotRequired,
        }))
    }

    #[cfg(any(unix, windows))]
    fn cleanup_staging(
        &self,
        adapter: &impl DurablePublicationAdapter,
        name: &OsStr,
    ) -> StagingCleanup {
        match adapter.remove_file(&self.parent, name) {
            Ok(()) => StagingCleanup::Removed,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                StagingCleanup::NotRequired
            }
            Err(source) => StagingCleanup::Failed {
                error: PumasError::Io {
                    message: format!(
                        "Failed to clean staging file for {}",
                        self.display_path.display()
                    ),
                    path: Some(self.display_path.clone()),
                    source: Some(source),
                },
            },
        }
    }

    #[cfg(any(unix, windows))]
    fn configured_parent_still_matches(&self) -> Result<bool> {
        if let Some(validate) = &self.capability_validation {
            return validate();
        }
        let current = crate::platform::capability_fs::open_directory(&self.parent_path)
            .map_err(|source| {
                parent_io_error(&self.parent_path, "reopen for identity check", source)
            })?
            .into_std_file();
        Ok(parent_identity_from_file(&current, &self.parent_path)? == self.parent_identity)
    }
}

fn parent_io_error(parent: &Path, operation: &str, source: std::io::Error) -> PumasError {
    PumasError::Io {
        message: format!(
            "Failed to {operation} publication parent {}",
            parent.display()
        ),
        path: Some(parent.to_path_buf()),
        source: Some(source),
    }
}

fn parent_identity_from_file(file: &File, parent: &Path) -> Result<ParentIdentity> {
    #[cfg(not(windows))]
    let metadata = file
        .metadata()
        .map_err(|source| parent_io_error(parent, "inspect", source))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(ParentIdentity::Unix {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        use cap_primitives::fs::_WindowsByHandle;
        let metadata = cap_std::fs::Metadata::from_file(file)
            .map_err(|source| parent_io_error(parent, "inspect identity", source))?;
        match (metadata.volume_serial_number(), metadata.file_index()) {
            (Some(volume), Some(file)) => Ok(ParentIdentity::Windows { volume, file }),
            _ => Err(PumasError::Other(
                "Windows directory identity unavailable".into(),
            )),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        // Durable publication already rejects other targets. Do not call
        // nightly-only Windows metadata APIs for an unadmitted capability.
        let _ = metadata;
        Ok(ParentIdentity::Unsupported)
    }
}

/// Read and parse a JSON file.
///
/// Returns `None` if the file doesn't exist, or an error if parsing fails.
pub fn atomic_read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            crate::platform::capability_fs::check_missing_path(path)
                .map_err(|error| PumasError::io_with_path(error, path))?;
            return Ok(None);
        }

        Err(source) => {
            return Err(PumasError::Io {
                message: format!("Failed to open {}", path.display()),
                path: Some(path.to_path_buf()),
                source: Some(source),
            });
        }
    };

    read_json_file(&mut file, path).map(Some)
}

fn read_json_file<T: DeserializeOwned>(file: &mut File, path: &Path) -> Result<T> {
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|source| PumasError::Io {
            message: format!("Failed to read {}", path.display()),
            path: Some(path.to_path_buf()),
            source: Some(source),
        })?;
    serde_json::from_str(&contents).map_err(|source| PumasError::Json {
        message: format!("Failed to parse {}: {source}", path.display()),
        source: Some(source),
    })
}

trait LegacyPublicationAdapter {
    fn rename(&self, source: &Path, target: &Path) -> std::io::Result<()>;
}

struct OsLegacyPublicationAdapter;

impl LegacyPublicationAdapter for OsLegacyPublicationAdapter {
    fn rename(&self, source: &Path, target: &Path) -> std::io::Result<()> {
        fs::rename(source, target)
    }
}

struct LegacyTempFileCleanup {
    path: PathBuf,
    armed: bool,
}

impl LegacyTempFileCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for LegacyTempFileCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn create_owned_legacy_staging(path: &Path) -> Result<(PathBuf, File, LegacyTempFileCleanup)> {
    for attempt in 0..16_u8 {
        let temp_path = path.with_extension(format!("json.{}.{}.tmp", Uuid::new_v4(), attempt));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => {
                let cleanup = LegacyTempFileCleanup::new(temp_path.clone());
                return Ok((temp_path, file, cleanup));
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(PumasError::Io {
                    message: format!("Failed to create temp file {}", temp_path.display()),
                    path: Some(temp_path),
                    source: Some(source),
                });
            }
        }
    }

    Err(PumasError::Io {
        message: format!(
            "Failed to allocate an exclusively owned temp file for {}",
            path.display()
        ),
        path: Some(path.to_path_buf()),
        source: Some(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "all legacy atomic-write staging names collided",
        )),
    })
}

/// Write data to a JSON file atomically.
///
/// This function:
/// 1. Serializes data to an exclusively owned, collision-resistant staging file
/// 2. Validates the JSON by re-parsing
/// 3. Calls fsync before replacement
/// 4. Optionally creates a .bak backup
/// 5. Atomically renames temp file to target
#[allow(unsafe_code)]
pub fn atomic_write_json<T: Serialize>(path: &Path, data: &T, keep_backup: bool) -> Result<()> {
    write_json_and_rename(path, data, keep_backup, &OsLegacyPublicationAdapter)
}

#[allow(unsafe_code)]
fn write_json_and_rename<T: Serialize>(
    path: &Path,
    data: &T,
    keep_backup: bool,
    adapter: &impl LegacyPublicationAdapter,
) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: format!("Failed to create directory {}", parent.display()),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }
    }

    // Serialize to string with pretty printing
    let serialized = serde_json::to_string_pretty(data).map_err(|e| PumasError::Json {
        message: format!("Failed to serialize data: {}", e),
        source: Some(e),
    })?;

    // Validate JSON by re-parsing
    serde_json::from_str::<serde_json::Value>(&serialized).map_err(|e| PumasError::Json {
        message: format!("JSON validation failed: {}", e),
        source: Some(e),
    })?;

    // Create the staging file exclusively before arming its cleanup owner.
    let (temp_path, mut file, mut temp_cleanup) = create_owned_legacy_staging(path)?;

    // Write to temp file
    {
        file.write_all(serialized.as_bytes())
            .map_err(|e| PumasError::Io {
                message: format!("Failed to write temp file {}", temp_path.display()),
                path: Some(temp_path.clone()),
                source: Some(e),
            })?;

        file.flush().map_err(|e| PumasError::Io {
            message: format!("Failed to flush temp file {}", temp_path.display()),
            path: Some(temp_path.clone()),
            source: Some(e),
        })?;

        // Request file-data synchronization before replacement.
        #[cfg(unix)]
        {
            // SAFETY: file.as_raw_fd() is a valid descriptor owned by `file`
            // for the duration of this call. fsync does not retain the
            // descriptor after returning.
            let sync_result = unsafe { libc::fsync(file.as_raw_fd()) };
            if sync_result != 0 {
                return Err(PumasError::Io {
                    message: format!("Failed to sync temp file {}", temp_path.display()),
                    path: Some(temp_path.clone()),
                    source: Some(std::io::Error::last_os_error()),
                });
            }
        }

        #[cfg(not(unix))]
        {
            file.sync_all().map_err(|e| PumasError::Io {
                message: format!("Failed to sync temp file {}", temp_path.display()),
                path: Some(temp_path.clone()),
                source: Some(e),
            })?;
        }
    }
    drop(file);

    // Create backup if requested and target exists
    if keep_backup && path.exists() {
        let backup_path = path.with_extension("json.bak");
        if let Err(e) = fs::copy(path, &backup_path) {
            warn!("Failed to create backup {}: {}", backup_path.display(), e);
            // Continue anyway - backup failure is not fatal
        } else {
            debug!("Created backup: {}", backup_path.display());
        }
    }

    // Atomic rename
    adapter
        .rename(&temp_path, path)
        .map_err(|e| PumasError::Io {
            message: format!(
                "Failed to rename {} to {}",
                temp_path.display(),
                path.display()
            ),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;
    temp_cleanup.disarm();

    debug!("Atomically wrote {}", path.display());
    Ok(())
}

fn publication_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::io;
    use tempfile::TempDir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_atomic_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        // Write
        atomic_write_json(&path, &data, false).unwrap();
        assert!(path.exists());

        // Read
        let read_data: Option<TestData> = atomic_read_json(&path).unwrap();
        assert_eq!(read_data, Some(data));
    }

    #[test]
    fn test_atomic_write_creates_backup() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");

        let data1 = TestData {
            name: "first".to_string(),
            value: 1,
        };
        let data2 = TestData {
            name: "second".to_string(),
            value: 2,
        };

        // First write
        atomic_write_json(&path, &data1, true).unwrap();

        // Second write with backup
        atomic_write_json(&path, &data2, true).unwrap();

        // Check backup exists
        let backup_path = path.with_extension("json.bak");
        assert!(backup_path.exists());

        // Verify backup contains first data
        let backup_data: Option<TestData> = atomic_read_json(&backup_path).unwrap();
        assert_eq!(backup_data, Some(data1));

        // Verify current file contains second data
        let current_data: Option<TestData> = atomic_read_json(&path).unwrap();
        assert_eq!(current_data, Some(data2));
    }

    #[test]
    fn atomic_write_json_never_overwrites_or_deletes_a_preexisting_legacy_temp() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        format!("{:?}", std::thread::current().id()).hash(&mut hasher);
        let legacy_temp = path.with_extension(format!(
            "json.{}.{}.tmp",
            std::process::id(),
            hasher.finish()
        ));
        fs::write(&legacy_temp, b"foreign staging sentinel").unwrap();

        atomic_write_json(
            &path,
            &TestData {
                name: "new".to_string(),
                value: 7,
            },
            false,
        )
        .unwrap();

        assert_eq!(fs::read(&legacy_temp).unwrap(), b"foreign staging sentinel");
        assert_eq!(
            atomic_read_json::<TestData>(&path).unwrap(),
            Some(TestData {
                name: "new".to_string(),
                value: 7,
            })
        );
    }

    #[test]
    fn test_atomic_read_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("nonexistent.json");

        let result: Option<TestData> = atomic_read_json(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn atomic_read_only_treats_not_found_as_absent() {
        let temp_dir = TempDir::new().unwrap();
        let non_directory = temp_dir.path().join("not-a-directory");
        fs::write(&non_directory, b"file").unwrap();

        let error = atomic_read_json::<TestData>(&non_directory.join("test.json")).unwrap_err();

        assert!(error.to_string().contains("Failed to open"), "{error}");
    }

    #[test]
    fn target_unavailable_failure_has_closed_stage_and_kind() {
        let failure = target_unavailable_failure(Path::new("unsupported/downloads.json"));

        assert_eq!(failure.stage, AtomicPublishStage::TargetAdmission);
        assert_eq!(failure.kind, AtomicPublishFailureKind::TargetUnavailable);
        assert!(matches!(
            failure.error,
            PumasError::Io {
                source: Some(ref source),
                ..
            } if source.kind() == io::ErrorKind::Unsupported
        ));
        assert!(matches!(failure.cleanup, StagingCleanup::NotRequired));
    }

    #[test]
    fn test_atomic_write_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("nested").join("dir").join("test.json");

        let data = TestData {
            name: "nested".to_string(),
            value: 99,
        };

        atomic_write_json(&path, &data, false).unwrap();
        assert!(path.exists());
    }

    #[cfg(any(unix, windows))]
    struct FaultPublicationAdapter {
        fail_staging_write: bool,
        fail_rename: bool,
        fail_rename_after_effect: bool,
        fail_parent_sync: bool,
        fail_cleanup: bool,
        temp_name: Option<OsString>,
    }

    #[cfg(any(unix, windows))]
    impl DurablePublicationAdapter for FaultPublicationAdapter {
        fn temp_name(&self, target: &OsStr, attempt: u8) -> OsString {
            self.temp_name.clone().unwrap_or_else(|| {
                let mut name = target.to_os_string();
                name.push(format!(".test-{attempt}.tmp"));
                name
            })
        }

        fn write_and_sync(&self, file: &mut cap_std::fs::File, contents: &[u8]) -> io::Result<()> {
            if self.fail_staging_write {
                Err(io::Error::other("injected staging write failure"))
            } else {
                OsDurablePublicationAdapter.write_and_sync(file, contents)
            }
        }

        fn rename(&self, parent: &Dir, source: &OsStr, target: &OsStr) -> io::Result<()> {
            if self.fail_rename_after_effect {
                OsDurablePublicationAdapter.rename(parent, source, target)?;
                return Err(io::Error::other("injected post-effect rename failure"));
            }
            if self.fail_rename {
                Err(io::Error::other("injected ambiguous rename failure"))
            } else {
                OsDurablePublicationAdapter.rename(parent, source, target)
            }
        }

        fn remove_file(&self, parent: &Dir, name: &OsStr) -> io::Result<()> {
            if self.fail_cleanup {
                Err(io::Error::other("injected cleanup failure"))
            } else {
                parent.remove_file(name)
            }
        }

        fn sync_parent(&self, parent: &File) -> io::Result<()> {
            if self.fail_parent_sync {
                Err(io::Error::other("injected parent-sync failure"))
            } else {
                OsDurablePublicationAdapter.sync_parent(parent)
            }
        }
    }

    #[cfg(any(unix, windows))]
    fn temp_publication_files(directory: &Path) -> Vec<std::path::PathBuf> {
        fs::read_dir(directory)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|extension| extension == "tmp"))
            .collect()
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_rename_error_is_visibility_unknown_and_cleans_owned_temp() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let old = TestData {
            name: "old".to_string(),
            value: 1,
        };
        let new = TestData {
            name: "new".to_string(),
            value: 2,
        };
        atomic_write_json(&path, &old, false).unwrap();

        let target = AtomicJsonTarget::open(&path).unwrap();
        let publication = target
            .publish_json_with_adapter(
                &new,
                &FaultPublicationAdapter {
                    fail_staging_write: false,
                    fail_rename: true,
                    fail_rename_after_effect: false,
                    fail_parent_sync: false,
                    fail_cleanup: false,
                    temp_name: None,
                },
            )
            .unwrap();

        assert!(matches!(
            publication,
            AtomicPublication::VisibilityUnknown {
                cleanup: StagingCleanup::Removed,
                ..
            }
        ));
        assert_eq!(atomic_read_json(&path).unwrap(), Some(old));
        assert!(temp_publication_files(temp_dir.path()).is_empty());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_post_rename_sync_failure_reports_visible_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let old = TestData {
            name: "old".to_string(),
            value: 1,
        };
        let new = TestData {
            name: "new".to_string(),
            value: 2,
        };
        atomic_write_json(&path, &old, false).unwrap();

        let target = AtomicJsonTarget::open(&path).unwrap();
        let publication = target
            .publish_json_with_adapter(
                &new,
                &FaultPublicationAdapter {
                    fail_staging_write: false,
                    fail_rename: false,
                    fail_rename_after_effect: false,
                    fail_parent_sync: true,
                    fail_cleanup: false,
                    temp_name: None,
                },
            )
            .unwrap();

        assert!(matches!(
            publication,
            AtomicPublication::PublishedDurabilityUnknown { .. }
        ));
        assert_eq!(atomic_read_json(&path).unwrap(), Some(new));
        assert!(temp_publication_files(temp_dir.path()).is_empty());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_syncs_file_and_parent_before_durable_outcome() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let data = TestData {
            name: "durable".to_string(),
            value: 3,
        };

        let publication = AtomicJsonTarget::open(&path)
            .unwrap()
            .publish_json(&data)
            .unwrap();

        assert!(
            matches!(publication, AtomicPublication::Durable),
            "{publication:?}"
        );
        assert_eq!(atomic_read_json(&path).unwrap(), Some(data));
        assert!(temp_publication_files(temp_dir.path()).is_empty());
    }

    #[cfg(not(any(unix, windows)))]
    #[test]
    fn atomic_publish_refuses_unsupported_targets_without_effects() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        fs::write(&path, b"original bytes").unwrap();
        let target = AtomicJsonTarget::open(&path).unwrap();

        let failure = target
            .publish_json(&TestData {
                name: "new".to_string(),
                value: 2,
            })
            .unwrap_err();

        assert_eq!(failure.stage, AtomicPublishStage::TargetAdmission);
        assert_eq!(failure.kind, AtomicPublishFailureKind::TargetUnavailable);
        assert!(matches!(failure.cleanup, StagingCleanup::NotRequired));
        assert_eq!(fs::read(&path).unwrap(), b"original bytes");
        assert_eq!(fs::read_dir(temp_dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn atomic_publish_requires_a_preexisting_parent() {
        let temp_dir = TempDir::new().unwrap();
        let missing_parent = temp_dir.path().join("missing");
        let path = missing_parent.join("test.json");
        assert!(AtomicJsonTarget::open(&path).is_err());
        assert!(!missing_parent.exists());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_post_effect_rename_error_remains_visibility_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let new = TestData {
            name: "new".to_string(),
            value: 5,
        };
        let target = AtomicJsonTarget::open(&path).unwrap();

        let publication = target
            .publish_json_with_adapter(
                &new,
                &FaultPublicationAdapter {
                    fail_staging_write: false,
                    fail_rename: false,
                    fail_rename_after_effect: true,
                    fail_parent_sync: false,
                    fail_cleanup: false,
                    temp_name: None,
                },
            )
            .unwrap();

        assert!(matches!(
            publication,
            AtomicPublication::VisibilityUnknown {
                cleanup: StagingCleanup::NotRequired,
                ..
            }
        ));
        assert_eq!(atomic_read_json(&path).unwrap(), Some(new));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_reports_cleanup_failure_without_hiding_rename_ambiguity() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let data = TestData {
            name: "new".to_string(),
            value: 6,
        };
        let target = AtomicJsonTarget::open(&path).unwrap();

        let publication = target
            .publish_json_with_adapter(
                &data,
                &FaultPublicationAdapter {
                    fail_staging_write: false,
                    fail_rename: true,
                    fail_rename_after_effect: false,
                    fail_parent_sync: false,
                    fail_cleanup: true,
                    temp_name: None,
                },
            )
            .unwrap();

        match publication {
            AtomicPublication::VisibilityUnknown {
                cleanup: StagingCleanup::Failed { error },
                ..
            } => assert!(error.to_string().contains("clean staging"), "{error}"),
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert_eq!(temp_publication_files(temp_dir.path()).len(), 1);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_pre_rename_failure_reports_secondary_cleanup_failure() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let old = TestData {
            name: "old".to_string(),
            value: 1,
        };
        atomic_write_json(&path, &old, false).unwrap();
        let target = AtomicJsonTarget::open(&path).unwrap();

        let failure = target
            .publish_json_with_adapter(
                &TestData {
                    name: "new".to_string(),
                    value: 10,
                },
                &FaultPublicationAdapter {
                    fail_staging_write: true,
                    fail_rename: false,
                    fail_rename_after_effect: false,
                    fail_parent_sync: false,
                    fail_cleanup: true,
                    temp_name: None,
                },
            )
            .unwrap_err();

        assert!(failure.error.to_string().contains("write and sync"));
        assert!(matches!(failure.cleanup, StagingCleanup::Failed { .. }));
        assert_eq!(atomic_read_json(&path).unwrap(), Some(old));
        assert_eq!(temp_publication_files(temp_dir.path()).len(), 1);
    }

    #[cfg(any(unix, windows))]
    struct CollisionAdapter {
        foreign_name: OsString,
    }

    #[cfg(any(unix, windows))]
    impl DurablePublicationAdapter for CollisionAdapter {
        fn temp_name(&self, _target: &OsStr, _attempt: u8) -> OsString {
            self.foreign_name.clone()
        }

        fn write_and_sync(&self, file: &mut cap_std::fs::File, contents: &[u8]) -> io::Result<()> {
            OsDurablePublicationAdapter.write_and_sync(file, contents)
        }

        fn rename(&self, parent: &Dir, source: &OsStr, target: &OsStr) -> io::Result<()> {
            OsDurablePublicationAdapter.rename(parent, source, target)
        }

        fn remove_file(&self, parent: &Dir, name: &OsStr) -> io::Result<()> {
            parent.remove_file(name)
        }

        fn sync_parent(&self, parent: &File) -> io::Result<()> {
            OsDurablePublicationAdapter.sync_parent(parent)
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_never_truncates_or_cleans_a_foreign_staging_collision() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");
        let foreign_name = OsString::from("test.json.foreign.tmp");
        let foreign_path = temp_dir.path().join(&foreign_name);
        fs::write(&foreign_path, b"foreign sentinel").unwrap();
        let target = AtomicJsonTarget::open(&path).unwrap();

        let failure = target
            .publish_json_with_adapter(
                &TestData {
                    name: "new".to_string(),
                    value: 7,
                },
                &CollisionAdapter {
                    foreign_name: foreign_name.clone(),
                },
            )
            .unwrap_err();

        assert!(matches!(failure.cleanup, StagingCleanup::NotRequired));
        assert_eq!(fs::read(foreign_path).unwrap(), b"foreign sentinel");
        assert!(!path.exists());
    }

    #[cfg(any(unix, windows))]
    struct ReplaceParentAfterRenameAdapter {
        configured_parent: PathBuf,
        moved_parent: PathBuf,
        fail_parent_sync: bool,
    }

    #[cfg(any(unix, windows))]
    impl DurablePublicationAdapter for ReplaceParentAfterRenameAdapter {
        fn temp_name(&self, target: &OsStr, attempt: u8) -> OsString {
            let mut name = target.to_os_string();
            name.push(format!(".replace-{attempt}.tmp"));
            name
        }

        fn write_and_sync(&self, file: &mut cap_std::fs::File, contents: &[u8]) -> io::Result<()> {
            OsDurablePublicationAdapter.write_and_sync(file, contents)
        }

        fn rename(&self, parent: &Dir, source: &OsStr, target: &OsStr) -> io::Result<()> {
            OsDurablePublicationAdapter.rename(parent, source, target)?;
            fs::rename(&self.configured_parent, &self.moved_parent)?;
            fs::create_dir(&self.configured_parent)?;
            fs::write(
                self.configured_parent.join("sentinel"),
                b"replacement directory",
            )
        }

        fn remove_file(&self, parent: &Dir, name: &OsStr) -> io::Result<()> {
            parent.remove_file(name)
        }

        fn sync_parent(&self, parent: &File) -> io::Result<()> {
            if self.fail_parent_sync {
                Err(io::Error::other("injected parent-sync failure"))
            } else {
                OsDurablePublicationAdapter.sync_parent(parent)
            }
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn atomic_publish_never_calls_replaced_configured_parent_durable() {
        let temp_dir = TempDir::new().unwrap();
        let configured_parent = temp_dir.path().join("store");
        let moved_parent = temp_dir.path().join("store-moved");
        fs::create_dir(&configured_parent).unwrap();
        let path = configured_parent.join("test.json");
        let target = AtomicJsonTarget::open(&path).unwrap();

        let publication = target
            .publish_json_with_adapter(
                &TestData {
                    name: "held".to_string(),
                    value: 8,
                },
                &ReplaceParentAfterRenameAdapter {
                    configured_parent: configured_parent.clone(),
                    moved_parent: moved_parent.clone(),
                    fail_parent_sync: false,
                },
            )
            .unwrap();

        assert!(matches!(
            publication,
            AtomicPublication::VisibilityUnknown { .. }
        ));
        assert_eq!(
            fs::read(configured_parent.join("sentinel")).unwrap(),
            b"replacement directory"
        );
        assert_eq!(
            atomic_read_json::<TestData>(&moved_parent.join("test.json")).unwrap(),
            Some(TestData {
                name: "held".to_string(),
                value: 8,
            })
        );
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn capability_publisher_does_not_reopen_display_path() {
        let temp = TempDir::new().unwrap();
        let parent = crate::platform::capability_fs::open_directory(temp.path()).unwrap();
        let target = AtomicJsonTarget::from_capability(
            parent,
            OsStr::new(".pumas_download"),
            PathBuf::from("/unavailable/display/.pumas_download"),
            || Ok(true),
        )
        .unwrap();
        let marker = serde_json::json!({"files":["model.gguf"]});
        assert!(matches!(
            target.publish_json(&marker).unwrap(),
            AtomicPublication::Durable
        ));
        assert_eq!(
            atomic_read_json::<serde_json::Value>(&temp.path().join(".pumas_download")).unwrap(),
            Some(marker)
        );
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn capability_publisher_preserves_publication_ambiguity() {
        for (rename_failure, sync_failure) in [(true, false), (false, true)] {
            let temp = TempDir::new().unwrap();
            let parent = crate::platform::capability_fs::open_directory(temp.path()).unwrap();
            let target = AtomicJsonTarget::from_capability(
                parent,
                OsStr::new("marker"),
                temp.path().join("marker"),
                || Ok(true),
            )
            .unwrap();
            let publication = target
                .publish_json_with_adapter(
                    &serde_json::json!({"files":[]}),
                    &FaultPublicationAdapter {
                        fail_rename_after_effect: rename_failure,
                        fail_parent_sync: sync_failure,
                        fail_staging_write: false,
                        fail_rename: false,
                        fail_cleanup: false,
                        temp_name: None,
                    },
                )
                .unwrap();
            if rename_failure {
                assert!(matches!(
                    publication,
                    AtomicPublication::VisibilityUnknown { .. }
                ));
            } else {
                assert!(matches!(
                    publication,
                    AtomicPublication::PublishedDurabilityUnknown { .. }
                ));
            }
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn capability_publisher_rejects_authority_change_before_staging() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let temp = TempDir::new().unwrap();
        let valid = Arc::new(AtomicBool::new(true));
        let check = valid.clone();
        let target = AtomicJsonTarget::from_capability(
            crate::platform::capability_fs::open_directory(temp.path()).unwrap(),
            OsStr::new("marker"),
            temp.path().join("marker"),
            move || Ok(check.load(Ordering::SeqCst)),
        )
        .unwrap();
        valid.store(false, Ordering::SeqCst);
        let failure = target.publish_json(&serde_json::json!({})).unwrap_err();
        assert_eq!(failure.stage, AtomicPublishStage::TargetAdmission);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn capability_parent_replacement_after_rename_is_visibility_unknown() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("model");
        fs::create_dir(&path).unwrap();
        let root = crate::platform::capability_fs::open_directory(temp.path()).unwrap();
        let parent = crate::platform::capability_fs::open_directory(&path).unwrap();
        let identity =
            parent_identity_from_file(&parent.open(".").unwrap().into_std(), &path).unwrap();
        let display = path.clone();
        let target = AtomicJsonTarget::from_capability(
            parent,
            OsStr::new("marker"),
            path.join("marker"),
            move || {
                Ok(parent_identity_from_file(
                    &root.open_dir("model")?.open(".")?.into_std(),
                    &display,
                )? == identity)
            },
        )
        .unwrap();
        let moved = temp.path().join("moved");
        assert!(matches!(
            target
                .publish_json_with_adapter(
                    &serde_json::json!({"files":[]}),
                    &ReplaceParentAfterRenameAdapter {
                        configured_parent: path.clone(),
                        moved_parent: moved.clone(),
                        fail_parent_sync: false,
                    }
                )
                .unwrap(),
            AtomicPublication::VisibilityUnknown { .. }
        ));
        assert!(moved.join("marker").exists());
        assert!(!path.join("marker").exists());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn parent_sync_failure_after_parent_replacement_is_visibility_unknown() {
        let temp_dir = TempDir::new().unwrap();
        let configured_parent = temp_dir.path().join("store");
        let moved_parent = temp_dir.path().join("store-moved");
        fs::create_dir(&configured_parent).unwrap();
        let path = configured_parent.join("test.json");
        let target = AtomicJsonTarget::open(&path).unwrap();

        let publication = target
            .publish_json_with_adapter(
                &TestData {
                    name: "held".to_string(),
                    value: 8,
                },
                &ReplaceParentAfterRenameAdapter {
                    configured_parent: configured_parent.clone(),
                    moved_parent,
                    fail_parent_sync: true,
                },
            )
            .unwrap();

        assert!(matches!(
            publication,
            AtomicPublication::VisibilityUnknown { .. }
        ));
        assert_eq!(
            fs::read(configured_parent.join("sentinel")).unwrap(),
            b"replacement directory"
        );
    }

    #[cfg(any(unix, windows))]
    struct ExitDuringRenameAdapter {
        after_effect: bool,
    }

    #[cfg(any(unix, windows))]
    impl DurablePublicationAdapter for ExitDuringRenameAdapter {
        fn temp_name(&self, target: &OsStr, attempt: u8) -> OsString {
            let mut name = target.to_os_string();
            name.push(format!(".interrupt-{attempt}.tmp"));
            name
        }

        fn write_and_sync(&self, file: &mut cap_std::fs::File, contents: &[u8]) -> io::Result<()> {
            OsDurablePublicationAdapter.write_and_sync(file, contents)
        }

        fn rename(&self, parent: &Dir, source: &OsStr, target: &OsStr) -> io::Result<()> {
            if self.after_effect {
                OsDurablePublicationAdapter.rename(parent, source, target)?;
                std::process::exit(72);
            }
            std::process::exit(71);
        }

        fn remove_file(&self, parent: &Dir, name: &OsStr) -> io::Result<()> {
            parent.remove_file(name)
        }

        fn sync_parent(&self, parent: &File) -> io::Result<()> {
            OsDurablePublicationAdapter.sync_parent(parent)
        }
    }

    #[cfg(any(unix, windows))]
    #[test]
    #[ignore = "subprocess helper invoked by interrupted_rename_reopens_as_old_or_new_without_success"]
    fn atomic_publish_interruption_child() {
        let Some(path) = std::env::var_os("PUMAS_ATOMIC_CHILD_PATH") else {
            return;
        };
        let after_effect = std::env::var("PUMAS_ATOMIC_AFTER_EFFECT").unwrap() == "1";
        let target = AtomicJsonTarget::open(Path::new(&path)).unwrap();
        let _ = target.publish_json_with_adapter(
            &TestData {
                name: "new".to_string(),
                value: 9,
            },
            &ExitDuringRenameAdapter { after_effect },
        );
        panic!("interruption adapter returned unexpectedly");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn interrupted_rename_reopens_as_old_or_new_without_success() {
        for (after_effect, exit_code, expected_name) in [(false, 71, "old"), (true, 72, "new")] {
            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path().join("test.json");
            atomic_write_json(
                &path,
                &TestData {
                    name: "old".to_string(),
                    value: 1,
                },
                false,
            )
            .unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--ignored")
                .arg("--exact")
                .arg("metadata::atomic::tests::atomic_publish_interruption_child")
                .arg("--nocapture")
                .env("PUMAS_ATOMIC_CHILD_PATH", &path)
                .env(
                    "PUMAS_ATOMIC_AFTER_EFFECT",
                    if after_effect { "1" } else { "0" },
                )
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(exit_code));
            assert_eq!(
                atomic_read_json::<TestData>(&path).unwrap().unwrap().name,
                expected_name
            );
        }
    }
}
