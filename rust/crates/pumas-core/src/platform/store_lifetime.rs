//! Internal physical-root exclusion groundwork, not an instance-recovery policy.
//!
//! On Linux/macOS this locks an independently opened directory, so aliases converge on
//! one physical object and no replaceable lock-file inode is involved. Clones
//! share the open file description. Dropping the final share closes this
//! process's descriptor; a fork-inherited descriptor can extend native exclusion
//! until it closes (normally on exec). Acquisition does not inspect a PID, create
//! a marker, or modify a registry.
//!
//! Advisory exclusion covers only cooperating holders on filesystems that honor
//! native locks. It neither fences arbitrary path-based writes nor establishes
//! that an external child has stopped. Callers must retain a clone inside each
//! actual effect, including blocking closures, and use separate filesystem
//! capabilities/current-root checks. The existing primary composes lifetime
//! shares into its in-process owners; historical recovery remains disabled.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs::File;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::MetadataExt;

#[derive(Debug, thiserror::Error)]
pub(crate) enum StoreLeaseError {
    #[error("physical store is already held: {0}")]
    Busy(PathBuf),
    #[error("physical store path no longer names the held root: {0}")]
    RootChanged(PathBuf),
    #[error("physical store root is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("physical store directory locking is not implemented on this platform")]
    UnsupportedPlatform,
    #[error("physical store lease I/O failed: {0}")]
    Io(#[from] io::Error),
}

/// A process-local share of one native directory lifetime lock.
///
/// There is deliberately no explicit unlock, raw-handle escape, registry cleanup,
/// or serializable ownership token. This is not evidence of historical cessation.
#[derive(Clone, Debug)]
pub(crate) struct PhysicalStoreLease(Arc<LeaseInner>);

#[derive(Debug)]
struct LeaseInner {
    root: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    file: File,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    identity: (u64, u64),
}

impl PhysicalStoreLease {
    /// Nonblocking acquisition for an existing root. No directories or files are
    /// created. Every independent call opens a new native lock owner, including
    /// calls in this process. Only `Clone` deliberately shares an existing owner.
    pub(crate) fn try_acquire(root: &Path) -> Result<Self, StoreLeaseError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let root = root.canonicalize()?;
            // Reuse the capability opener's O_DIRECTORY/O_NOFOLLOW/O_NONBLOCK:
            // a raced replacement by a FIFO must never block this constructor.
            let file = super::capability_fs::open_pinned_directory(&root)
                .map_err(|error| {
                    if error.kind() == io::ErrorKind::NotADirectory {
                        StoreLeaseError::NotDirectory(root.clone())
                    } else {
                        StoreLeaseError::Io(error)
                    }
                })?
                .into_std_file();
            let metadata = file.metadata()?;
            if !metadata.is_dir() {
                return Err(StoreLeaseError::NotDirectory(root));
            }
            fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
                {
                    StoreLeaseError::Busy(root.clone())
                } else {
                    StoreLeaseError::Io(error)
                }
            })?;
            let lease = Self(Arc::new(LeaseInner {
                root,
                file,
                identity: (metadata.dev(), metadata.ino()),
            }));
            // A rename/replacement between open and lock acquisition must not
            // return a lease whose stored path already names another object.
            lease.require_current()?;
            Ok(lease)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = root;
            Err(StoreLeaseError::UnsupportedPlatform)
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.0.root
    }

    /// Check the captured pathname against the held object. This observation is
    /// not a capability and cannot make a later ambient path operation race-free.
    pub(crate) fn require_current(&self) -> Result<(), StoreLeaseError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let current = match std::fs::metadata(&self.0.root) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Err(StoreLeaseError::RootChanged(self.0.root.clone()));
                }
                Err(error) => return Err(error.into()),
            };
            let held = self.0.file.metadata()?;
            if !current.is_dir()
                || (current.dev(), current.ino()) != self.0.identity
                || (held.dev(), held.ino()) != self.0.identity
            {
                return Err(StoreLeaseError::RootChanged(self.0.root.clone()));
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err(StoreLeaseError::UnsupportedPlatform)
        }
    }

    /// Move a distinct lifetime share into the actual effect closure, rather
    /// than only its async requester or join observer. Dropping either waiter
    /// cannot unlock while this closure is still running (or waiting to run).
    pub(crate) fn retain_for_effect<F, T>(&self, effect: F) -> impl FnOnce() -> T
    where
        F: FnOnce() -> T,
    {
        let lease = self.clone();
        move || {
            let _lease = lease;
            effect()
        }
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
#[path = "store_lifetime/tests.rs"]
mod tests;

/// Internal composition lifetime. Empty for standalone legacy components and
/// platforms without directory locks; never a recovery qualification token.
#[derive(Clone, Debug, Default)]
pub(crate) struct StoreLifetime(Option<PhysicalStoreLease>);

impl StoreLifetime {
    /// Identity of the currently held directory; never a stand-alone authority.
    pub(crate) fn physical_identity(&self) -> crate::Result<(u64, u64)> {
        self.require_current()?;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            Ok(self.0.as_ref().expect("checked physical lease").0.identity)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err(crate::PumasError::InvalidParams {
                message: "physical identity unavailable".into(),
            })
        }
    }

    pub(crate) fn require_root(&self, root: &Path) -> crate::Result<()> {
        let lease = self
            .0
            .as_ref()
            .ok_or_else(|| crate::PumasError::InvalidParams {
                message: "qualified physical store lease is unavailable".into(),
            })?;
        if lease.root() != root {
            return Err(crate::PumasError::InvalidParams {
                message: "held physical store and selected root differ".into(),
            });
        }
        self.require_current()
    }

    pub(crate) fn require_current(&self) -> crate::Result<()> {
        self.0
            .as_ref()
            .ok_or_else(|| crate::PumasError::InvalidParams {
                message: "qualified physical store lease is unavailable".into(),
            })?
            .require_current()
            .map_err(|error| crate::PumasError::InvalidParams {
                message: format!("Cannot use held physical Pumas store: {error}"),
            })
    }

    pub(crate) fn acquire(root: &Path) -> crate::Result<Self> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            PhysicalStoreLease::try_acquire(root)
                .map(|lease| Self(Some(lease)))
                .map_err(|error| crate::PumasError::InvalidParams {
                    message: match error {
                        StoreLeaseError::Busy(path) => format!("Pumas library instance is already running for physical store {}. Drop existing owner handles before constructing another owner.", path.display()),
                        other => format!("Cannot own physical Pumas store: {other}"),
                    },
                })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = root;
            // Preserve the existing platform contract. This is deliberately
            // unqualified and cannot authorize historical-owner replacement.
            Ok(Self::default())
        }
    }

    pub(crate) fn retain_for_effect<F, T>(&self, effect: F) -> impl FnOnce() -> T
    where
        F: FnOnce() -> T,
    {
        let lifetime = self.clone();
        move || {
            let StoreLifetime(_lease) = lifetime;
            effect()
        }
    }

    pub(crate) fn spawn_blocking<F, T>(&self, effect: F) -> tokio::task::JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(self.retain_for_effect(effect))
    }
}
