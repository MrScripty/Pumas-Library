//! Durable identity of a process Pumas launched, recorded beside its PID file.
//!
//! A bare PID is not custody: PIDs are reused, and PID files survive reboots.
//! Pumas therefore records who the process was when it started and compares
//! that record with the live process before any signal is sent:
//!
//! - **Linux**: the kernel boot id (`/proc/sys/kernel/random/boot_id`), the
//!   start time in clock ticks since boot (`/proc/<pid>/stat` field 22) and
//!   the kernel's executable path (`/proc/<pid>/exe`). Boot id plus start time
//!   is unique for the life of the machine, so a reused PID never matches.
//! - **macOS and Windows** (weaker, documented): the absolute start time in
//!   whole seconds and the canonical executable path, as reported by
//!   `sysinfo`. A reboot changes the absolute start time. A reused PID only
//!   matches if a process with the same executable starts within the same
//!   second, which Pumas accepts as residual risk on these platforms.
//!
//! The identity lives in a sidecar file (`<pid file>.identity`) so existing
//! readers that parse the PID file as a bare integer keep working. A PID file
//! without a sidecar is *unverified*: Pumas never signals it.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// The schema written to identity sidecars by this version.
const IDENTITY_SCHEMA: u32 = 1;

/// The suffix appended to a PID file path to name its identity sidecar.
pub const IDENTITY_SUFFIX: &str = ".identity";

/// What a recorded start time is measured in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartTimeClock {
    /// Clock ticks since boot (Linux `/proc/<pid>/stat`). Only meaningful
    /// together with the recorded boot id.
    LinuxBootTicks,
    /// Whole seconds since the Unix epoch (macOS and Windows).
    UnixSeconds,
}

/// The identity of one process at the time it was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
    /// Sidecar schema; readers refuse a schema they do not know.
    pub schema: u32,
    /// The recorded process ID.
    pub pid: u32,
    /// The kernel boot id, where the platform provides one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot_id: Option<String>,
    /// The process start time, in `start_time_clock` units.
    pub start_time: u64,
    /// The unit of `start_time`.
    pub start_time_clock: StartTimeClock,
    /// The executable the process was running when recorded.
    pub executable: PathBuf,
}

/// The result of comparing a recorded identity with the live process table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityCheck {
    /// The recorded process is still running.
    Alive,
    /// No running process holds the recorded identity: it exited, the PID was
    /// reused by another process, or the record predates the current boot.
    Gone {
        /// Why the record no longer names a live process.
        reason: String,
    },
    /// The live process could not be proven to be (or not to be) the recorded
    /// one. Callers must neither signal it nor discard its record.
    Unverifiable {
        /// What prevented verification.
        reason: String,
    },
}

/// A PID file read together with its identity sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PidRecord {
    /// The PID file names a process whose start identity was recorded.
    Verified(ProcessIdentity),
    /// The PID file has no usable identity (for example it was written by an
    /// older Pumas). Pumas never signals an unverified PID.
    Unverified {
        /// The PID in the file, when it parses.
        pid: Option<u32>,
        /// Why the record is unverified.
        reason: String,
    },
}

impl PidRecord {
    /// The recorded PID, when one parses.
    pub fn pid(&self) -> Option<u32> {
        match self {
            Self::Verified(identity) => Some(identity.pid),
            Self::Unverified { pid, .. } => *pid,
        }
    }
}

/// The sidecar path that holds the identity for `pid_file`.
pub fn identity_path(pid_file: &Path) -> PathBuf {
    let mut name = pid_file.as_os_str().to_owned();
    name.push(IDENTITY_SUFFIX);
    PathBuf::from(name)
}

/// Capture the identity of a live process.
pub fn capture(pid: u32) -> io::Result<ProcessIdentity> {
    imp::capture(pid)
}

/// Compare a recorded identity with the live process that holds its PID.
pub fn verify(recorded: &ProcessIdentity) -> IdentityCheck {
    if recorded.schema != IDENTITY_SCHEMA {
        return IdentityCheck::Unverifiable {
            reason: format!(
                "process identity schema {} is not supported (expected {IDENTITY_SCHEMA})",
                recorded.schema
            ),
        };
    }
    imp::verify(recorded)
}

/// Write the identity sidecar for `pid_file` atomically (temporary file, then
/// rename), so a reader never observes a partial record.
pub fn write_identity(pid_file: &Path, identity: &ProcessIdentity) -> io::Result<()> {
    let target = identity_path(pid_file);
    let bytes = serde_json::to_vec(identity).map_err(io::Error::other)?;
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    io::Write::write_all(&mut staged, &bytes)?;
    staged.as_file().sync_all()?;
    staged.persist(&target).map_err(|error| error.error)?;
    Ok(())
}

/// Write a PID file and its identity: the sidecar first, then the bare PID,
/// so a PID file never names a process whose identity was not yet recorded.
pub fn write_record(pid_file: &Path, identity: &ProcessIdentity) -> io::Result<()> {
    write_identity(pid_file, identity)?;
    std::fs::write(pid_file, identity.pid.to_string())
}

/// Read a PID file and its identity sidecar. `Ok(None)` means no PID file.
pub fn read_record(pid_file: &Path) -> io::Result<Option<PidRecord>> {
    let text = match std::fs::read_to_string(pid_file) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let pid = text.trim().parse::<u32>().ok().filter(|pid| *pid > 0);
    let sidecar = match std::fs::read(identity_path(pid_file)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Some(PidRecord::Unverified {
                pid,
                reason: "the PID file has no identity record (it may predate identity \
                         recording); Pumas cannot prove it launched this process"
                    .into(),
            }));
        }
        Err(error) => return Err(error),
    };
    let identity: ProcessIdentity = match serde_json::from_slice(&sidecar) {
        Ok(identity) => identity,
        Err(error) => {
            return Ok(Some(PidRecord::Unverified {
                pid,
                reason: format!("the PID identity record is unreadable: {error}"),
            }));
        }
    };
    if pid != Some(identity.pid) {
        return Ok(Some(PidRecord::Unverified {
            pid,
            reason: format!(
                "the PID file does not match its identity record (identity names PID {})",
                identity.pid
            ),
        }));
    }
    Ok(Some(PidRecord::Verified(identity)))
}

/// Remove a PID file and its identity sidecar. Missing files are not errors.
pub fn remove_record(pid_file: &Path) -> io::Result<()> {
    for path in [identity_path(pid_file), pid_file.to_path_buf()] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{IdentityCheck, ProcessIdentity, StartTimeClock, IDENTITY_SCHEMA};
    use std::io;
    use std::path::{Path, PathBuf};

    const PROC: &str = "/proc";

    pub(super) fn boot_id() -> io::Result<String> {
        Ok(
            std::fs::read_to_string(Path::new(PROC).join("sys/kernel/random/boot_id"))?
                .trim()
                .to_owned(),
        )
    }

    /// State character and start time (field 22) from `/proc/<pid>/stat`.
    pub(super) fn parse_stat(stat: &[u8]) -> Option<(char, u64)> {
        // The command name (field 2) may contain spaces and parentheses; the
        // fields after its final ") " are fixed. Field 3 is the state.
        let separator = stat.windows(2).rposition(|bytes| bytes == b") ")?;
        let fields = std::str::from_utf8(&stat[separator + 2..]).ok()?;
        let mut fields = fields.split_whitespace();
        let state = fields.next()?.chars().next()?;
        // Fields 4..=21 precede the start time (field 22).
        let start_time = fields.nth(18)?.parse().ok()?;
        Some((state, start_time))
    }

    fn read_stat(pid: u32) -> io::Result<Option<(char, u64)>> {
        match std::fs::read(Path::new(PROC).join(pid.to_string()).join("stat")) {
            Ok(bytes) => parse_stat(&bytes)
                .map(Some)
                .ok_or_else(|| io::Error::other("unparseable /proc stat record")),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(libc::ESRCH) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn read_exe(pid: u32) -> io::Result<PathBuf> {
        std::fs::read_link(Path::new(PROC).join(pid.to_string()).join("exe"))
    }

    fn exited(state: char) -> bool {
        matches!(state, 'Z' | 'X' | 'x')
    }

    pub(super) fn capture(pid: u32) -> io::Result<ProcessIdentity> {
        let (state, start_time) = read_stat(pid)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("process {pid} is not running"),
            )
        })?;
        if exited(state) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("process {pid} has already exited"),
            ));
        }
        Ok(ProcessIdentity {
            schema: IDENTITY_SCHEMA,
            pid,
            boot_id: Some(boot_id()?),
            start_time,
            start_time_clock: StartTimeClock::LinuxBootTicks,
            executable: read_exe(pid)?,
        })
    }

    pub(super) fn verify(recorded: &ProcessIdentity) -> IdentityCheck {
        if recorded.start_time_clock != StartTimeClock::LinuxBootTicks {
            return IdentityCheck::Unverifiable {
                reason: "the identity was recorded on a different platform".into(),
            };
        }
        let current_boot = match boot_id() {
            Ok(boot) => boot,
            Err(error) => {
                return IdentityCheck::Unverifiable {
                    reason: format!("cannot read the kernel boot id: {error}"),
                }
            }
        };
        if recorded.boot_id.as_deref() != Some(current_boot.as_str()) {
            return IdentityCheck::Gone {
                reason: "the record was written before the current boot".into(),
            };
        }
        let (state, start_time) = match read_stat(recorded.pid) {
            Ok(Some(stat)) => stat,
            Ok(None) => {
                return IdentityCheck::Gone {
                    reason: format!("process {} has exited", recorded.pid),
                }
            }
            Err(error) => {
                return IdentityCheck::Unverifiable {
                    reason: format!("cannot observe process {}: {error}", recorded.pid),
                }
            }
        };
        if start_time != recorded.start_time {
            return IdentityCheck::Gone {
                reason: format!(
                    "PID {} now belongs to a different process (start time differs)",
                    recorded.pid
                ),
            };
        }
        if exited(state) {
            return IdentityCheck::Gone {
                reason: format!("process {} has exited", recorded.pid),
            };
        }
        match read_exe(recorded.pid) {
            Ok(executable) if executable == recorded.executable => IdentityCheck::Alive,
            Ok(executable) => IdentityCheck::Unverifiable {
                reason: format!(
                    "process {} now runs {} instead of the recorded {}",
                    recorded.pid,
                    executable.display(),
                    recorded.executable.display()
                ),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => IdentityCheck::Gone {
                reason: format!("process {} has exited", recorded.pid),
            },
            Err(error) => IdentityCheck::Unverifiable {
                reason: format!(
                    "cannot read the executable of process {}: {error}",
                    recorded.pid
                ),
            },
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::{IdentityCheck, ProcessIdentity, StartTimeClock, IDENTITY_SCHEMA};
    use std::io;
    use std::path::PathBuf;
    use sysinfo::{Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};

    struct Observed {
        start_time: u64,
        executable: Option<PathBuf>,
        exited: bool,
    }

    fn observe(pid: u32) -> Option<Observed> {
        let mut system = System::new();
        let target = Pid::from_u32(pid);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[target]),
            true,
            ProcessRefreshKind::new().with_exe(UpdateKind::Always),
        );
        let process = system.process(target)?;
        Some(Observed {
            start_time: process.start_time(),
            executable: process
                .exe()
                .map(|path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf())),
            exited: matches!(
                process.status(),
                ProcessStatus::Zombie | ProcessStatus::Dead
            ),
        })
    }

    pub(super) fn capture(pid: u32) -> io::Result<ProcessIdentity> {
        let observed = observe(pid).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("process {pid} is not running"),
            )
        })?;
        if observed.exited {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("process {pid} has already exited"),
            ));
        }
        let executable = observed.executable.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("cannot read the executable of process {pid}"),
            )
        })?;
        Ok(ProcessIdentity {
            schema: IDENTITY_SCHEMA,
            pid,
            boot_id: None,
            start_time: observed.start_time,
            start_time_clock: StartTimeClock::UnixSeconds,
            executable,
        })
    }

    pub(super) fn verify(recorded: &ProcessIdentity) -> IdentityCheck {
        if recorded.start_time_clock != StartTimeClock::UnixSeconds {
            return IdentityCheck::Unverifiable {
                reason: "the identity was recorded on a different platform".into(),
            };
        }
        let Some(observed) = observe(recorded.pid) else {
            return IdentityCheck::Gone {
                reason: format!("process {} has exited", recorded.pid),
            };
        };
        if observed.start_time != recorded.start_time {
            return IdentityCheck::Gone {
                reason: format!(
                    "PID {} now belongs to a different process (start time differs)",
                    recorded.pid
                ),
            };
        }
        if observed.exited {
            return IdentityCheck::Gone {
                reason: format!("process {} has exited", recorded.pid),
            };
        }
        match observed.executable {
            Some(executable) if executable == recorded.executable => IdentityCheck::Alive,
            Some(executable) => IdentityCheck::Unverifiable {
                reason: format!(
                    "process {} now runs {} instead of the recorded {}",
                    recorded.pid,
                    executable.display(),
                    recorded.executable.display()
                ),
            },
            None => IdentityCheck::Unverifiable {
                reason: format!("cannot read the executable of process {}", recorded.pid),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_identity_round_trips_and_verifies_alive() {
        let temp = tempfile::tempdir().unwrap();
        let pid_file = temp.path().join("runtime.pid");
        let identity = capture(std::process::id()).unwrap();
        write_record(&pid_file, &identity).unwrap();
        // Existing readers keep parsing the PID file as a bare integer.
        assert_eq!(
            std::fs::read_to_string(&pid_file).unwrap(),
            std::process::id().to_string()
        );
        let record = read_record(&pid_file).unwrap().unwrap();
        assert_eq!(record, PidRecord::Verified(identity.clone()));
        assert_eq!(verify(&identity), IdentityCheck::Alive);
        remove_record(&pid_file).unwrap();
        assert!(!pid_file.exists());
        assert!(!identity_path(&pid_file).exists());
        assert_eq!(read_record(&pid_file).unwrap(), None);
    }

    #[test]
    fn bare_pid_file_is_unverified() {
        let temp = tempfile::tempdir().unwrap();
        let pid_file = temp.path().join("ollama.pid");
        std::fs::write(&pid_file, std::process::id().to_string()).unwrap();
        match read_record(&pid_file).unwrap().unwrap() {
            PidRecord::Unverified { pid, reason } => {
                assert_eq!(pid, Some(std::process::id()));
                assert!(reason.contains("no identity record"));
            }
            other => panic!("expected an unverified record, got {other:?}"),
        }
    }

    #[test]
    fn mismatched_pid_file_and_identity_is_unverified() {
        let temp = tempfile::tempdir().unwrap();
        let pid_file = temp.path().join("ollama.pid");
        let identity = capture(std::process::id()).unwrap();
        write_identity(&pid_file, &identity).unwrap();
        std::fs::write(&pid_file, "1").unwrap();
        assert!(matches!(
            read_record(&pid_file).unwrap().unwrap(),
            PidRecord::Unverified { pid: Some(1), .. }
        ));
    }

    #[test]
    fn reused_pid_is_gone_and_changed_executable_is_unverifiable() {
        let identity = capture(std::process::id()).unwrap();
        let reused = ProcessIdentity {
            start_time: identity.start_time.wrapping_add(1),
            ..identity.clone()
        };
        assert!(matches!(verify(&reused), IdentityCheck::Gone { .. }));
        let other_exe = ProcessIdentity {
            executable: PathBuf::from("/nonexistent/other-runtime"),
            ..identity.clone()
        };
        assert!(matches!(
            verify(&other_exe),
            IdentityCheck::Unverifiable { .. }
        ));
        let unknown_schema = ProcessIdentity {
            schema: IDENTITY_SCHEMA + 1,
            ..identity
        };
        assert!(matches!(
            verify(&unknown_schema),
            IdentityCheck::Unverifiable { .. }
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn previous_boot_record_is_gone() {
        let identity = ProcessIdentity {
            boot_id: Some("00000000-0000-0000-0000-000000000000".into()),
            ..capture(std::process::id()).unwrap()
        };
        match verify(&identity) {
            IdentityCheck::Gone { reason } => assert!(reason.contains("before the current boot")),
            other => panic!("expected gone, got {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stat_parser_handles_hostile_command_names() {
        let mut stat = String::from("42 (evil) S 1 2 3) S");
        for field in 4..=21 {
            stat.push_str(&format!(" {field}"));
        }
        stat.push_str(" 987654 23 24");
        assert_eq!(imp::parse_stat(stat.as_bytes()), Some(('S', 987_654)));
        assert_eq!(imp::parse_stat(b"42 (short) S 1"), None);
    }

    #[test]
    fn exited_child_identity_is_gone() {
        let mut child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sleep" })
            .args(if cfg!(windows) {
                &["/d", "/c", "ping -n 30 127.0.0.1 >NUL"][..]
            } else {
                &["30"][..]
            })
            .spawn()
            .unwrap();
        let identity = capture(child.id()).unwrap();
        assert_eq!(verify(&identity), IdentityCheck::Alive);
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(matches!(verify(&identity), IdentityCheck::Gone { .. }));
    }
}
