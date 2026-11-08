//! Cooperative lifetime exclusion for an existing managed CPython depot.
//! Provider mutation holds the permanent lock inode exclusively.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct ManagedDepotLease {
    file: Arc<File>,
}

impl ManagedDepotLease {
    pub(crate) fn mutation(depot: &Path) -> io::Result<Self> {
        let metadata = std::fs::symlink_metadata(depot)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "managed depot is not a non-link directory",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
        }
        let file = options.open(depot.join(".pumas-python-depot.lock"))?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "managed depot lock is not regular",
            ));
        }
        FileExt::try_lock_exclusive(&file)?;
        Ok(Self {
            file: Arc::new(file),
        })
    }
}

impl Drop for ManagedDepotLease {
    fn drop(&mut self) {
        if Arc::strong_count(&self.file) == 1 {
            let _ = FileExt::unlock(&*self.file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutation_is_exclusive_until_the_last_clone_leaves() {
        let depot = tempfile::tempdir().unwrap();
        let first = ManagedDepotLease::mutation(depot.path()).unwrap();
        let retained = first.clone();
        assert!(ManagedDepotLease::mutation(depot.path()).is_err());
        drop(first);
        assert!(ManagedDepotLease::mutation(depot.path()).is_err());
        drop(retained);
        ManagedDepotLease::mutation(depot.path()).unwrap();
        assert!(depot.path().join(".pumas-python-depot.lock").is_file());
    }
    #[cfg(unix)]
    #[test]
    fn linked_depot_or_lock_is_refused_without_touching_target() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(other.path(), root.path().join("alias")).unwrap();
        assert!(ManagedDepotLease::mutation(&root.path().join("alias")).is_err());
        let target = other.path().join("keep");
        std::fs::write(&target, b"untouched").unwrap();
        std::os::unix::fs::symlink(&target, root.path().join(".pumas-python-depot.lock")).unwrap();
        assert!(ManagedDepotLease::mutation(root.path()).is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"untouched");
    }
}
