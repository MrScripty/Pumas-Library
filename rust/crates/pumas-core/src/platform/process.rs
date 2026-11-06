//! Platform-specific process management.
//!
//! This module provides cross-platform abstractions for process management,
//! including checking process status and termination.

#![warn(unsafe_code)]

use crate::error::{PumasError, Result};
use std::process::Command;
use tracing::{debug, warn};

/// Compare the physical identities of two open files without reading contents.
#[allow(unsafe_code)]
pub fn same_file_identity(left: &std::fs::File, right: &std::fs::File) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let left = left.metadata()?;
        let right = right.metadata()?;
        Ok(left.dev() == right.dev() && left.ino() == right.ino())
    }
    #[cfg(windows)]
    {
        use std::mem::zeroed;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let mut left_info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
        let mut right_info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
        // SAFETY: both file handles are live; the output buffers have the
        // exact Win32 structure layout and remain valid during each call.
        if unsafe { GetFileInformationByHandle(left.as_raw_handle(), &mut left_info) } == 0
            || unsafe { GetFileInformationByHandle(right.as_raw_handle(), &mut right_info) } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(
            left_info.dwVolumeSerialNumber == right_info.dwVolumeSerialNumber
                && left_info.nFileIndexHigh == right_info.nFileIndexHigh
                && left_info.nFileIndexLow == right_info.nFileIndexLow,
        )
    }
}

/// Check if a process with the given PID is alive.
///
/// # Platform Behavior
/// - **Linux/macOS**: Uses `kill(pid, 0)` signal check
/// - **Windows**: Tests the process object's termination signal without waiting
#[allow(unsafe_code)]
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Use kill(pid, 0) to check if process exists
        // Signal 0 doesn't actually send a signal, just checks if we can
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        // SAFETY: libc::kill with signal 0 does not deliver a signal or access
        // Rust-managed memory. The PID is range-checked before crossing the FFI
        // boundary.
        unsafe { libc::kill(pid, 0) == 0 }
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{
            CloseHandle, GetLastError, ERROR_ACCESS_DENIED, WAIT_OBJECT_0,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
        };

        // SAFETY: The handle has synchronization access, remains open during
        // the nonblocking wait, and is closed exactly once before returning.
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if !handle.is_null() {
                // OpenProcess can succeed after exit while another handle
                // retains the process object. Its signaled state proves exit,
                // even if the exit code happens to equal STILL_ACTIVE (259).
                let exited = WaitForSingleObject(handle, 0) == WAIT_OBJECT_0;
                CloseHandle(handle);
                !exited
            } else {
                // An inaccessible process is not evidence of a dead owner.
                GetLastError() == ERROR_ACCESS_DENIED
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Fallback: assume it exists
        warn!("Process alive check not implemented for this platform");
        true
    }
}

/// Configure a command to run independently from the launcher process.
///
/// # Platform Behavior
/// - **Linux/macOS**: Calls `setsid()` in the child after fork and before exec
/// - **Windows**: Uses `CREATE_NEW_PROCESS_GROUP`
/// - **Other**: Leaves the command unchanged
#[allow(unsafe_code)]
pub fn configure_detached_command(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // SAFETY: pre_exec runs in the child process after fork and before
        // exec. The closure only calls async-signal-safe setsid() and converts
        // errno to io::Error, so it avoids touching shared Rust state in the
        // post-fork child.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = command;
    }
}

/// The outcome of stopping a process named by a recorded identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedStop {
    /// The recorded process was running and has now stopped.
    Stopped,
    /// The recorded process was no longer running; nothing was signalled.
    AlreadyGone {
        /// Why the record no longer names a live process.
        reason: String,
    },
    /// Pumas could not prove custody of the live process and sent no signal
    /// (or the operating system refused the signal).
    Refused {
        /// Why the stop was refused.
        reason: String,
    },
}

/// Stop the process named by a recorded identity, gracefully then forcefully.
///
/// Every signal is preceded by an identity check (see
/// [`process_identity`](super::process_identity)); a PID whose identity no
/// longer matches is never signalled.
///
/// # Platform Behavior
/// - **Linux/macOS**: SIGTERM, wait up to `timeout_ms`, then SIGKILL. The
///   signal goes to the process group only while the verified process still
///   leads that group (Pumas launches legacy runtimes with `setsid`), so a
///   group is never signalled through a reused or foreign PID. Descendants
///   that outlive their leader receive the group SIGTERM but are not
///   force-killed afterwards, because nothing then proves the group is ours.
/// - **Windows**: `taskkill /PID {pid} /T /F` after the identity check. The
///   check and the kill are not atomic; see the weaker identity guarantee
///   documented in `process_identity`.
pub fn terminate_recorded_process(
    identity: &super::process_identity::ProcessIdentity,
    timeout_ms: u64,
) -> Result<RecordedStop> {
    use super::process_identity::{verify, IdentityCheck};

    match verify(identity) {
        IdentityCheck::Alive => {}
        IdentityCheck::Gone { reason } => return Ok(RecordedStop::AlreadyGone { reason }),
        IdentityCheck::Unverifiable { reason } => return Ok(RecordedStop::Refused { reason }),
    }

    #[cfg(unix)]
    {
        terminate_recorded_unix(identity, timeout_ms)
    }

    #[cfg(windows)]
    {
        let _ = timeout_ms;
        if !terminate_process_windows(identity.pid)? {
            return Ok(RecordedStop::Refused {
                reason: format!("taskkill could not stop process {}", identity.pid),
            });
        }
        match verify(identity) {
            IdentityCheck::Alive => Err(PumasError::Other(format!(
                "Process {} is still running after taskkill",
                identity.pid
            ))),
            _ => Ok(RecordedStop::Stopped),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = timeout_ms;
        Ok(RecordedStop::Refused {
            reason: "process termination is not implemented for this platform".into(),
        })
    }
}

#[cfg(unix)]
fn terminate_recorded_unix(
    identity: &super::process_identity::ProcessIdentity,
    timeout_ms: u64,
) -> Result<RecordedStop> {
    use super::process_identity::{verify, IdentityCheck};
    use nix::errno::Errno;
    use nix::sys::signal::{kill, killpg, Signal};
    use nix::unistd::{getpgid, Pid};
    use std::thread::sleep;
    use std::time::Duration;

    let Ok(raw) = i32::try_from(identity.pid) else {
        return Ok(RecordedStop::Refused {
            reason: format!("PID {} is out of range", identity.pid),
        });
    };
    let pid = Pid::from_raw(raw);
    // Signal only after a fresh identity check, and the group only while the
    // verified process still leads it.
    let send = |signal: Signal| -> Result<Option<RecordedStop>> {
        match verify(identity) {
            IdentityCheck::Alive => {}
            IdentityCheck::Gone { .. } => return Ok(Some(RecordedStop::Stopped)),
            IdentityCheck::Unverifiable { reason } => {
                return Ok(Some(RecordedStop::Refused { reason }))
            }
        }
        let result = if getpgid(Some(pid)) == Ok(pid) {
            killpg(pid, signal)
        } else {
            kill(pid, signal)
        };
        match result {
            Ok(()) => Ok(None),
            Err(Errno::ESRCH) => Ok(Some(RecordedStop::Stopped)),
            Err(Errno::EPERM) => Ok(Some(RecordedStop::Refused {
                reason: format!(
                    "the operating system denied signalling process {} (it belongs to another user)",
                    identity.pid
                ),
            })),
            Err(error) => Err(PumasError::Other(format!(
                "Failed to send {signal:?} to process {}: {error}",
                identity.pid
            ))),
        }
    };
    let gone = || !matches!(verify(identity), IdentityCheck::Alive);

    debug!("Sending SIGTERM to recorded process {}", identity.pid);
    if let Some(outcome) = send(Signal::SIGTERM)? {
        return Ok(outcome);
    }
    let interval = Duration::from_millis(100);
    for _ in 0..(timeout_ms / 100).max(1) {
        sleep(interval);
        if gone() {
            debug!("Recorded process {} stopped after SIGTERM", identity.pid);
            return Ok(RecordedStop::Stopped);
        }
    }
    debug!(
        "Recorded process {} still running, sending SIGKILL",
        identity.pid
    );
    if let Some(outcome) = send(Signal::SIGKILL)? {
        return Ok(outcome);
    }
    for _ in 0..10 {
        sleep(interval);
        if gone() {
            return Ok(RecordedStop::Stopped);
        }
    }
    Err(PumasError::Other(format!(
        "Process {} is still running after SIGKILL",
        identity.pid
    )))
}

#[cfg(windows)]
fn terminate_process_windows(pid: u32) -> Result<bool> {
    use std::process::Command;

    // Use taskkill with /F (force) and /T (tree - kill child processes too)
    debug!("Terminating process {} with taskkill", pid);

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F", "/T"])
        .output()
        .map_err(|e| PumasError::Other(format!("Failed to run taskkill: {}", e)))?;

    if output.status.success() {
        debug!("Process {} terminated successfully", pid);
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "not found" errors are OK - process already dead
        if stderr.contains("not found") || stderr.contains("not running") {
            Ok(true)
        } else {
            warn!("taskkill failed for {}: {}", pid, stderr);
            Ok(false)
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn terminate_owned_linux_group(
    child: &mut std::process::Child,
    timeout_ms: u64,
) -> Result<bool> {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::{getpgid, Pid};
    use std::thread::sleep;
    use std::time::Duration;

    let raw = child.id();
    let pid = Pid::from_raw(i32::try_from(raw).expect("Linux child PID fits pid_t"));
    let observe = || {
        super::linux_group::observe_exit(raw).map_err(|error| {
            PumasError::Other(format!(
                "Observing owned process-group leader {raw}: {error}"
            ))
        })
    };
    let signal = |signal| -> Result<()> {
        observe()?;
        let group = getpgid(Some(pid)).map_err(|error| {
            PumasError::Other(format!("Identifying owned process group {raw}: {error}"))
        })?;
        if group != pid {
            return Err(PumasError::Other(format!(
                "Owned process {raw} no longer identifies its process group"
            )));
        }
        killpg(pid, signal).map_err(|error| {
            PumasError::Other(format!("Signalling owned process group {raw}: {error}"))
        })
    };
    let drained = || -> Result<bool> {
        if observe()?.is_none() {
            return Ok(false);
        }
        match super::linux_group::group_has_live_members(pid.as_raw()) {
            Ok(live) => Ok(!live),
            // Uncertainty keeps the leader unreaped for the next observation.
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(PumasError::Other(format!(
                "Observing owned process group {raw}: {error}"
            ))),
        }
    };
    let mut reap = || -> Result<bool> {
        child.wait().map(|_| true).map_err(|error| {
            PumasError::Other(format!(
                "Reaping drained owned process-group leader {raw}: {error}"
            ))
        })
    };

    signal(Signal::SIGTERM)?;
    let interval = Duration::from_millis(100);
    for _ in 0..(timeout_ms / 100).max(1) {
        sleep(interval);
        if drained()? {
            return reap();
        }
    }
    signal(Signal::SIGKILL)?;
    sleep(interval);
    if drained()? {
        reap()
    } else {
        // A caller may retry while the owned child continues to pin PGID.
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_process_alive_self() {
        // Our own process should be alive
        let pid = std::process::id();
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_nonexistent() {
        // A very high PID should not exist
        assert!(!is_process_alive(4_000_000_000));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exited_owned_child_pins_its_pid_until_its_parent_reaps_it() {
        use super::super::linux_group;
        use std::time::Duration;

        let mut command = Command::new("bash");
        command.args(["-c", "exit 0"]);
        configure_detached_command(&mut command);
        let mut child = command.spawn().unwrap();
        let pid = child.id();
        wait_until(Duration::from_secs(5), || {
            linux_group::observe_exit(pid).unwrap().is_some()
        });
        assert!(linux_group::observe_exit(pid).unwrap().unwrap().success());
        // Signal zero observes PID existence, including an unreaped zombie.
        // This conservative ownership check must not release the PID pin.
        assert!(is_process_alive(pid));
        assert!(!linux_group::group_has_live_members(i32::try_from(pid).unwrap()).unwrap());
        assert!(child.wait().unwrap().success());
        assert!(!is_process_alive(pid));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn owned_tree_does_not_report_stopped_with_a_live_term_ignoring_member() {
        use super::super::linux_group;
        use nix::unistd::{getpgid, Pid};
        use std::os::unix::process::CommandExt;
        use std::time::Duration;

        let temp = tempfile::tempdir().unwrap();
        let ready = temp.path().join("ready");
        let mut child = Command::new("sleep")
            .arg("60")
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let group = Pid::from_raw(i32::try_from(pid).unwrap());
        // Own both direct children so this regression never depends on PID1
        // reaping an orphan. The worker cooperates with the same owned group.
        let mut worker = Command::new("bash")
            .args(["-c", "trap '' TERM; touch \"$READY_FILE\"; exec sleep 60"])
            .env("READY_FILE", &ready)
            .process_group(group.as_raw())
            .spawn()
            .unwrap();
        wait_until(Duration::from_secs(5), || ready.exists());
        assert_eq!(
            getpgid(Some(Pid::from_raw(i32::try_from(worker.id()).unwrap()))).unwrap(),
            group
        );
        assert!(linux_group::group_has_live_members(group.as_raw()).unwrap());
        let stopped = terminate_owned_linux_group(&mut child, 100);
        let leader_reaped = !is_process_alive(pid);
        let live = linux_group::group_has_live_members(group.as_raw());
        // Its unreaped Child pins the exact worker even when the old helper
        // already reaped the leader. Reap both before reporting an assertion.
        let _ = worker.kill();
        worker.wait().unwrap();
        let _ = child.wait();
        assert!(stopped.unwrap());
        assert!(leader_reaped);
        assert!(
            !live.unwrap(),
            "an exited leader is not evidence that its owned process group stopped"
        );
    }

    #[cfg(windows)]
    #[test]
    fn exited_process_with_retained_handle_is_not_alive() {
        // Retaining Child pins the Windows process object and prevents PID
        // reuse. Exit code 259 must not be mistaken for STILL_ACTIVE either.
        let mut child = Command::new("cmd")
            .args(["/d", "/c", "exit 259"])
            .spawn()
            .unwrap();
        let pid = child.id();
        assert_eq!(child.wait().unwrap().code(), Some(259));
        assert!(!is_process_alive(pid));
        drop(child);
    }

    #[test]
    fn recorded_stop_of_an_exited_identity_sends_nothing() {
        use super::super::process_identity::capture;
        let mut child = Command::new(if cfg!(windows) { "cmd" } else { "sleep" })
            .args(if cfg!(windows) {
                &["/d", "/c", "ping -n 30 127.0.0.1 >NUL"][..]
            } else {
                &["30"][..]
            })
            .spawn()
            .unwrap();
        let identity = capture(child.id()).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(matches!(
            terminate_recorded_process(&identity, 100).unwrap(),
            RecordedStop::AlreadyGone { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn recorded_stop_refuses_a_live_process_it_cannot_prove() {
        use super::super::process_identity::{capture, ProcessIdentity};
        let mut unrelated = Command::new("sleep").arg("30").spawn().unwrap();
        let recorded = ProcessIdentity {
            executable: std::path::PathBuf::from("/nonexistent/ollama"),
            ..capture(unrelated.id()).unwrap()
        };
        let outcome = terminate_recorded_process(&recorded, 100).unwrap();
        let survived = unrelated.try_wait().unwrap().is_none();
        let _ = unrelated.kill();
        unrelated.wait().unwrap();
        assert!(
            matches!(outcome, RecordedStop::Refused { .. }),
            "{outcome:?}"
        );
        assert!(survived, "an unproven process must not be signalled");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn recorded_stop_of_a_group_member_preserves_the_callers_group() {
        use super::super::process_identity::capture;
        let mut child = Command::new("sleep").arg("60").spawn().unwrap();
        let mut sibling = Command::new("sleep").arg("60").spawn().unwrap();
        let identity = capture(child.id()).unwrap();
        let result = terminate_recorded_process(&identity, 100);
        let sibling_running = sibling.try_wait().unwrap().is_none();
        if super::super::linux_group::observe_exit(child.id()).is_ok() {
            let _ = child.kill();
        }
        let _ = child.wait();
        let _ = sibling.kill();
        sibling.wait().unwrap();
        assert_eq!(result.unwrap(), RecordedStop::Stopped);
        assert!(sibling_running);
    }

    #[cfg(unix)]
    #[test]
    fn recorded_stop_stops_a_detached_process_group() {
        use super::super::process_identity::capture;
        use std::process::{Command, Stdio};
        use std::time::Duration;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let child_pid_file = temp_dir.path().join("child.pid");
        let ready_file = temp_dir.path().join("ready");

        let mut command = Command::new("bash");
        command
            .arg("-c")
            // Reap the owned descendant on TERM instead of delegating its
            // zombie to container PID1, which need not be a reaping init.
            .arg("trap 'wait' TERM; sleep 60 & echo $! > \"$CHILD_PID_FILE\"; touch \"$READY_FILE\"; wait")
            .env("CHILD_PID_FILE", &child_pid_file)
            .env("READY_FILE", &ready_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_detached_command(&mut command);

        let mut child = command.spawn().unwrap();
        let parent_pid = child.id();

        wait_until(Duration::from_secs(5), || ready_file.exists());
        let worker_pid = std::fs::read_to_string(&child_pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();

        assert!(is_process_alive(parent_pid));
        assert!(is_process_alive(worker_pid));

        let identity = capture(parent_pid).unwrap();
        let stopped = terminate_recorded_process(&identity, 1_000).unwrap();
        let _ = child.wait();

        assert_eq!(stopped, RecordedStop::Stopped);
        #[cfg(target_os = "linux")]
        assert!(!super::super::linux_group::group_has_live_members(
            i32::try_from(parent_pid).unwrap()
        )
        .unwrap());
        wait_until(Duration::from_secs(5), || !is_process_alive(worker_pid));
        assert!(!is_process_alive(worker_pid));
    }

    #[cfg(unix)]
    fn wait_until(timeout: std::time::Duration, mut condition: impl FnMut() -> bool) {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if condition() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
}
