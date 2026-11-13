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
    /// Processes Pumas recorded starting (runtime profiles, launched apps)
    /// that are still running. While any is, the entry is not released.
    pub surviving_processes: Vec<SurvivingProcess>,
}

/// A process Pumas recorded starting that still runs, or that cannot be
/// proven to have exited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurvivingProcess {
    pub pid: u32,
    /// The PID record, relative to the library root.
    pub record: PathBuf,
    /// The executable it was recorded running, when the record has one.
    pub executable: Option<PathBuf>,
}

impl StaleInstance {
    fn from_entry(entry: &InstanceEntry, lease_checked: bool) -> Self {
        Self {
            library_path: entry.library_path.clone(),
            pid: entry.pid,
            started_at: entry.started_at.clone(),
            version: entry.version.clone(),
            lease_checked,
            surviving_processes: Vec::new(),
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

/// Seconds since the Unix epoch at which `pid` started, when it can be read.
fn process_start_unix_seconds(pid: u32) -> Option<i64> {
    use crate::platform::process_identity::{capture, StartTimeClock};
    let identity = capture(pid).ok()?;
    match identity.start_time_clock {
        StartTimeClock::UnixSeconds => i64::try_from(identity.start_time).ok(),
        StartTimeClock::LinuxBootTicks => {
            #[cfg(target_os = "linux")]
            {
                let stat = std::fs::read_to_string("/proc/stat").ok()?;
                let boot: i64 = stat
                    .lines()
                    .find_map(|line| line.strip_prefix("btime "))?
                    .trim()
                    .parse()
                    .ok()?;
                // SAFETY: sysconf reads a configuration value; no memory is shared.
                #[allow(unsafe_code)]
                let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
                (ticks > 0).then(|| boot + (identity.start_time / ticks as u64) as i64)
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        }
    }
}

/// Whether the entry's owner may still be running: its PID exists and was not
/// reused by a process that started after the entry was written. An unknown
/// start time counts as possibly the owner.
fn owner_may_be_running(pid: u32, started_at: &str) -> bool {
    if !process_exists(pid) {
        return false;
    }
    let (Some(process_start), Ok(entry_start)) = (
        process_start_unix_seconds(pid),
        chrono::DateTime::parse_from_rfc3339(started_at),
    ) else {
        return true;
    };
    // The owner writes its entry after it starts; allow clock granularity.
    process_start <= entry_start.timestamp() + 2
}

const MAX_PID_RECORDS: usize = 256;

/// PID records Pumas keeps for processes it starts: runtime profiles under
/// `launcher-data/runtime-profiles/<provider>/<profile>/` and launched apps
/// under `<app>-versions/<version>/`.
fn pid_records(root: &Path) -> Vec<PathBuf> {
    let mut records = Vec::new();
    let mut scan = |dir: PathBuf| {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "pid") && records.len() < MAX_PID_RECORDS {
                records.push(path);
            }
        }
    };
    let grandchildren = |dir: PathBuf| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .flat_map(|entry| {
                std::fs::read_dir(entry.path())
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    for dir in grandchildren(root.join("launcher-data").join("runtime-profiles")) {
        scan(dir);
    }
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let versions = entry.file_name().to_string_lossy().ends_with("-versions");
        if versions && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            for version in std::fs::read_dir(entry.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                if version.file_type().is_ok_and(|kind| kind.is_dir()) {
                    scan(version.path());
                }
            }
        }
    }
    records
}

/// Recorded processes that still run, or that cannot be proven to have exited.
fn surviving_processes(root: &Path) -> Vec<SurvivingProcess> {
    use crate::platform::process_identity::{read_record, verify, IdentityCheck, PidRecord};
    pid_records(root)
        .into_iter()
        .filter_map(|record| {
            let relative = record.strip_prefix(root).unwrap_or(&record).to_path_buf();
            match read_record(&record).ok()?? {
                PidRecord::Verified(identity) => match verify(&identity) {
                    IdentityCheck::Gone { .. } => None,
                    IdentityCheck::Alive | IdentityCheck::Unverifiable { .. } => {
                        Some(SurvivingProcess {
                            pid: identity.pid,
                            record: relative,
                            executable: Some(identity.executable),
                        })
                    }
                },
                PidRecord::Unverified { pid: Some(pid), .. } if process_exists(pid) => {
                    Some(SurvivingProcess {
                        pid,
                        record: relative,
                        executable: None,
                    })
                }
                PidRecord::Unverified { .. } => None,
            }
        })
        .collect()
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
        if owner_may_be_running(entry.pid, &entry.started_at) {
            return Ok(None);
        }
        let lease = match hold_store_lease(&canonical) {
            Ok(lease) => lease,
            Err(_) => return Ok(None),
        };
        let mut stale = StaleInstance::from_entry(&entry, lease.is_some());
        stale.surviving_processes = surviving_processes(&canonical);
        Ok(Some(stale))
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
        if owner_may_be_running(expected.pid, &expected.started_at) {
            return Err(refuse(format!(
                "process {} is running and may be the Pumas that wrote the entry; close it first",
                expected.pid
            )));
        }
        let survivors = surviving_processes(&canonical);
        if !survivors.is_empty() {
            let named = survivors
                .iter()
                .map(|process| format!("{} ({})", process.pid, process.record.display()))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(refuse(format!(
                "processes the old Pumas started are still running: {named}; stop them first"
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
            .contains("is running and may be the Pumas"));
        assert!(registry.get_instance(&root).unwrap().is_some());
    }

    /// Rewrite the row's start time, as an entry written long before the
    /// process now holding its PID started.
    fn backdate(registry: &LibraryRegistry, root: &Path) {
        registry
            .lock_conn()
            .unwrap()
            .execute(
                "UPDATE instances SET started_at='2000-01-01T00:00:00Z' WHERE library_path=?1",
                params![root.to_string_lossy()],
            )
            .unwrap();
    }

    #[test]
    fn a_pid_reused_by_a_newer_process_does_not_keep_the_entry() {
        let (_temp, registry, root) = fixture();
        // This test process holds the PID now, but started long after.
        registry
            .try_claim_instance(&root, std::process::id())
            .unwrap();
        backdate(&registry, &root);
        let stale = registry.inspect_stale_instance(&root).unwrap().unwrap();
        assert_eq!(stale.pid, std::process::id());
        registry.release_stale_instance(&root, &stale).unwrap();
        assert!(registry.get_instance(&root).unwrap().is_none());
    }

    #[test]
    fn a_recorded_child_still_running_blocks_the_release() {
        let (_temp, registry, root) = fixture();
        registry.try_claim_instance(&root, exited_pid()).unwrap();
        // A runtime profile the old owner started, still running (this test).
        let profile = root.join("launcher-data/runtime-profiles/llama-cpp/default");
        std::fs::create_dir_all(&profile).unwrap();
        let pid_file = profile.join("runtime.pid");
        std::fs::write(&pid_file, std::process::id().to_string()).unwrap();
        crate::platform::process_identity::write_record(
            &pid_file,
            &crate::platform::process_identity::capture(std::process::id()).unwrap(),
        )
        .unwrap();
        let stale = registry.inspect_stale_instance(&root).unwrap().unwrap();
        assert_eq!(stale.surviving_processes.len(), 1);
        assert_eq!(stale.surviving_processes[0].pid, std::process::id());
        assert_eq!(
            stale.surviving_processes[0].record,
            PathBuf::from("launcher-data/runtime-profiles/llama-cpp/default/runtime.pid")
        );
        let refusal = registry
            .release_stale_instance(&root, &stale)
            .unwrap_err()
            .to_string();
        assert!(refusal.contains("still running"), "{refusal}");
        assert!(registry.get_instance(&root).unwrap().is_some());
        // Once it has stopped, the release goes ahead.
        crate::platform::process_identity::remove_record(&pid_file).unwrap();
        registry.release_stale_instance(&root, &stale).unwrap();
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
