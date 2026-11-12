//! Operator-confirmed release of a primary-instance row whose owner is gone.
//!
//! A Pumas process that is killed, crashes or loses power cannot remove its
//! instance row, and the next owner is refused for that library. A free store
//! lease after process loss is not sufficient evidence that no external
//! writer survived (see the local-discovery-bootstrap contract), so Pumas never
//! replaces such a row on its own. This is the explicit administrative
//! reconciliation the contract asks for:
//!
//! - [`LibraryRegistry::inspect_stale_instance`] reports a row as stale when
//!   the library's store lease is free (Linux, macOS), the row's process does
//!   not exist, and the row is a primary row. It changes nothing.
//! - [`LibraryRegistry::release_stale_instance`] removes that exact row after
//!   an operator confirmed it, re-checking every condition while holding the
//!   store lease and inside one registry write transaction.
//!
//! Windows has no qualified store lease yet, so there the release rests on
//! the process check and the operator's confirmation alone, and says so.

use super::*;
use crate::platform::store_lifetime::StoreLifetime;
use serde::{Deserialize, Serialize};

/// A primary-instance row no live owner can be shown to hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleInstance {
    pub library_path: PathBuf,
    pub pid: u32,
    pub started_at: String,
    pub version: Option<String>,
    /// Whether a free store lease backs the finding. False where the platform
    /// has no qualified lease (Windows); only the process check backs it.
    pub lease_checked: bool,
}

impl StaleInstance {
    fn from_entry(entry: &InstanceEntry, lease_checked: bool) -> Self {
        Self {
            library_path: entry.library_path.clone(),
            pid: entry.pid,
            started_at: entry.started_at.clone(),
            version: entry.version.clone(),
            lease_checked,
        }
    }

    fn describes(&self, entry: &InstanceEntry) -> bool {
        self.library_path == entry.library_path
            && self.pid == entry.pid
            && self.started_at == entry.started_at
            && self.version == entry.version
    }
}

/// Whether a process with this ID exists, counting one that exists but
/// belongs to another user (permission denied) as existing.
#[allow(unsafe_code)]
fn process_exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        if pid <= 0 {
            return true;
        }
        // SAFETY: signal 0 delivers nothing and touches no Rust memory; the
        // PID is range-checked above.
        if unsafe { libc::kill(pid, 0) } == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        crate::platform::process::is_process_alive(pid)
    }
}

fn refuse(message: impl Into<String>) -> PumasError {
    PumasError::Validation {
        field: "registry.stale_instance".into(),
        message: message.into(),
    }
}

/// Hold the store lease where the platform qualifies one. `Ok(None)` means
/// the platform has none (the finding then rests on the process check).
fn hold_store_lease(root: &Path) -> Result<Option<StoreLifetime>> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Ok(None);
    }
    StoreLifetime::acquire(root).map(Some).map_err(|_| {
        refuse(format!(
            "A running Pumas process still holds the library at {}; close it instead of releasing its entry",
            crate::platform::platform_display_path(root)
        ))
    })
}

impl LibraryRegistry {
    /// Report the library's primary-instance row when no live owner can be
    /// shown to hold it. Changes nothing; holds the store lease only while
    /// checking.
    pub fn inspect_stale_instance(&self, root: &Path) -> Result<Option<StaleInstance>> {
        let canonical = Self::canonicalize_library_path(root)?;
        let entry = {
            let conn = self.lock_conn()?;
            Self::read_instance_entry(&conn, &canonical.to_string_lossy())?
        };
        let Some(entry) = entry else {
            return Ok(None);
        };
        if process_exists(entry.pid) {
            return Ok(None);
        }
        let lease = match hold_store_lease(&canonical) {
            Ok(lease) => lease,
            Err(_) => return Ok(None),
        };
        Ok(Some(StaleInstance::from_entry(&entry, lease.is_some())))
    }

    /// Remove the row `expected` describes, which an operator confirmed.
    /// Every condition is checked again while holding the store lease (where
    /// qualified) and inside one write transaction; a row that changed, a
    /// process that exists, or a held lease refuses with the reason.
    pub fn release_stale_instance(&self, root: &Path, expected: &StaleInstance) -> Result<()> {
        let canonical = Self::canonicalize_library_path(root)?;
        if canonical != expected.library_path {
            return Err(refuse("the confirmed entry names a different library"));
        }
        let lease = hold_store_lease(&canonical)?;
        if lease.is_some() != expected.lease_checked {
            return Err(refuse(
                "the store lease check differs from the confirmed one; inspect the entry again",
            ));
        }
        if process_exists(expected.pid) {
            return Err(refuse(format!(
                "process {} exists; close it, or if it is not Pumas, inspect again later",
                expected.pid
            )));
        }
        let path_str = canonical.to_string_lossy().to_string();
        let mut conn = self.lock_conn()?;
        let transaction =
            conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = Self::read_instance_entry(&transaction, &path_str)?;
        if !current
            .as_ref()
            .is_some_and(|entry| expected.describes(entry))
        {
            return Err(refuse(
                "the library's entry changed since it was confirmed; inspect it again",
            ));
        }
        let removed = transaction.execute(
            "DELETE FROM instances WHERE library_path=?1 AND pid=?2 AND started_at=?3",
            params![path_str, expected.pid, expected.started_at],
        )?;
        if removed != 1 {
            return Err(refuse(
                "the library's entry changed while it was being released",
            ));
        }
        transaction.commit()?;
        tracing::warn!(
            library = %crate::platform::platform_display_path(&canonical),
            pid = expected.pid,
            started_at = %expected.started_at,
            version = expected.version.as_deref().unwrap_or("unknown"),
            lease_checked = expected.lease_checked,
            "Released a stale Pumas instance entry after operator confirmation"
        );
        drop(lease);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exited_pid() -> u32 {
        let mut child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" })
            .args(if cfg!(windows) {
                &["/C", "exit"][..]
            } else {
                &[][..]
            })
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    fn fixture() -> (tempfile::TempDir, LibraryRegistry, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        let root = temp.path().join("library");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        (temp, registry, root)
    }

    #[test]
    fn a_dead_owners_entry_is_reported_and_released_only_as_confirmed() {
        let (_temp, registry, root) = fixture();
        assert_eq!(registry.inspect_stale_instance(&root).unwrap(), None);
        let pid = exited_pid();
        assert!(matches!(
            registry.try_claim_instance(&root, pid).unwrap(),
            InstanceClaimResult::Claimed(_)
        ));
        let stale = registry.inspect_stale_instance(&root).unwrap().unwrap();
        assert_eq!((stale.pid, stale.library_path.clone()), (pid, root.clone()));
        assert_eq!(
            stale.lease_checked,
            cfg!(any(target_os = "linux", target_os = "macos"))
        );
        // Nothing changed by inspection; the entry still refuses a new owner.
        assert!(matches!(
            registry
                .try_claim_instance(&root, std::process::id())
                .unwrap(),
            InstanceClaimResult::Occupied(_)
        ));

        // A confirmation of a different entry is refused and removes nothing.
        let mut other = stale.clone();
        other.started_at = "2000-01-01T00:00:00Z".into();
        assert!(registry
            .release_stale_instance(&root, &other)
            .unwrap_err()
            .to_string()
            .contains("changed since it was confirmed"));
        assert!(registry.get_instance(&root).unwrap().is_some());

        registry.release_stale_instance(&root, &stale).unwrap();
        assert!(registry.get_instance(&root).unwrap().is_none());
        assert!(matches!(
            registry
                .try_claim_instance(&root, std::process::id())
                .unwrap(),
            InstanceClaimResult::Claimed(_)
        ));
    }

    #[test]
    fn a_live_process_keeps_its_entry() {
        let (_temp, registry, root) = fixture();
        registry
            .try_claim_instance(&root, std::process::id())
            .unwrap();
        assert_eq!(registry.inspect_stale_instance(&root).unwrap(), None);
        let claimed = registry.get_instance(&root).unwrap().unwrap();
        let confirmed = StaleInstance::from_entry(
            &claimed,
            cfg!(any(target_os = "linux", target_os = "macos")),
        );
        assert!(registry
            .release_stale_instance(&root, &confirmed)
            .unwrap_err()
            .to_string()
            .contains("exists"));
        assert!(registry.get_instance(&root).unwrap().is_some());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn a_held_store_lease_keeps_the_entry_even_when_its_pid_is_gone() {
        let (_temp, registry, root) = fixture();
        registry.try_claim_instance(&root, exited_pid()).unwrap();
        let stale = registry.inspect_stale_instance(&root).unwrap().unwrap();
        // Something (here, this test) holds the store: no release.
        let _held = StoreLifetime::acquire(&root).unwrap();
        assert_eq!(registry.inspect_stale_instance(&root).unwrap(), None);
        assert!(registry
            .release_stale_instance(&root, &stale)
            .unwrap_err()
            .to_string()
            .contains("still holds the library"));
        assert!(registry.get_instance(&root).unwrap().is_some());
    }

    #[cfg(windows)]
    #[test]
    fn windows_findings_rest_on_the_process_check_alone() {
        let (_temp, registry, root) = fixture();
        registry.try_claim_instance(&root, exited_pid()).unwrap();
        let stale = registry.inspect_stale_instance(&root).unwrap().unwrap();
        assert!(!stale.lease_checked, "Windows has no qualified store lease");
    }
}
