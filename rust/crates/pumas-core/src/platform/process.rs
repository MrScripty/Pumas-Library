//! Platform-specific process management.
//!
//! This module provides cross-platform abstractions for process management,
//! including checking process status and termination.

#![warn(unsafe_code)]

use crate::error::{PumasError, Result};
use std::process::Command;
use tracing::{debug, warn};

/// Compare open file identities before deleting owned process metadata.
#[allow(unsafe_code)]
pub(crate) fn same_file_identity(
    left: &std::fs::File,
    right: &std::fs::File,
) -> std::io::Result<bool> {
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

/// Terminate a process gracefully, then forcefully if needed.
///
/// # Platform Behavior
/// - **Linux/macOS**: Sends SIGTERM, waits, then SIGKILL if still running
/// - **Windows**: Uses `taskkill /PID {pid} /F /T` to kill process tree
///
/// # Arguments
/// - `pid`: The process ID to terminate
/// - `timeout_ms`: How long to wait after graceful termination before force kill (Unix only)
///
/// # Returns
/// `true` if the process was terminated (or wasn't running), `false` on error
pub fn terminate_process(pid: u32, timeout_ms: u64) -> Result<bool> {
    #[cfg(not(unix))]
    let _ = timeout_ms;
    if !is_process_alive(pid) {
        debug!("Process {} is not running", pid);
        return Ok(true);
    }

    #[cfg(unix)]
    {
        terminate_process_unix(pid, timeout_ms)
    }

    #[cfg(windows)]
    {
        terminate_process_windows(pid)
    }

    #[cfg(not(any(unix, windows)))]
    {
        Err(PumasError::Other(
            "Process termination not implemented for this platform".into(),
        ))
    }
}

#[cfg(unix)]
fn terminate_process_unix(pid: u32, timeout_ms: u64) -> Result<bool> {
    use nix::sys::signal::{kill, Signal};
    use nix::sys::wait::{waitpid, WaitPidFlag};
    use nix::unistd::Pid;
    use std::thread::sleep;
    use std::time::Duration;

    let nix_pid = Pid::from_raw(pid as i32);

    // First try SIGTERM (graceful)
    debug!("Sending SIGTERM to process {}", pid);
    if let Err(e) = kill(nix_pid, Signal::SIGTERM) {
        if e == nix::errno::Errno::ESRCH {
            // Process doesn't exist
            return Ok(true);
        }
        warn!("Failed to send SIGTERM to {}: {}", pid, e);
    }

    // Wait for process to exit
    let wait_interval = Duration::from_millis(100);
    let iterations = (timeout_ms / 100).max(1);

    for _ in 0..iterations {
        sleep(wait_interval);
        // Try to reap zombie (non-blocking)
        let _ = waitpid(nix_pid, Some(WaitPidFlag::WNOHANG));
        if !is_process_alive(pid) {
            debug!("Process {} terminated gracefully", pid);
            return Ok(true);
        }
    }

    // Process still running, use SIGKILL
    debug!("Process {} still running, sending SIGKILL", pid);
    if let Err(e) = kill(nix_pid, Signal::SIGKILL) {
        if e == nix::errno::Errno::ESRCH {
            return Ok(true);
        }
        return Err(PumasError::Other(format!(
            "Failed to kill process {}: {}",
            pid, e
        )));
    }

    // Brief wait then reap the zombie
    sleep(Duration::from_millis(100));

    // Reap the zombie to remove it from process table
    let _ = waitpid(nix_pid, Some(WaitPidFlag::WNOHANG));

    Ok(!is_process_alive(pid))
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

/// Terminate a process and its children (Unix) or process tree (Windows).
///
/// # Platform Behavior
/// - **Linux/macOS**: Signals the process group first, then falls back to the
///   process PID if no matching group exists
/// - **Windows**: Uses `taskkill /T` which already handles the tree
///
/// This PID-only interface is best-effort and does not establish exclusive
/// custody of a Child or its reaper. Managed Linux children use the separate
/// borrowed-Child stop under their shared observation/stop mutex.
pub fn terminate_process_tree(pid: u32, timeout_ms: u64) -> Result<bool> {
    #[cfg(not(unix))]
    let _ = timeout_ms;
    #[cfg(unix)]
    {
        use nix::sys::signal::Signal;
        use nix::sys::wait::{waitpid, WaitPidFlag};
        use nix::unistd::Pid;
        use std::thread::sleep;
        use std::time::Duration;

        if !is_process_alive(pid) {
            debug!("Process {} is not running", pid);
            // Try to reap in case it's a zombie we haven't reaped yet
            let _ = waitpid(Pid::from_raw(pid as i32), Some(WaitPidFlag::WNOHANG));
            return Ok(true);
        }

        let nix_pid = Pid::from_raw(pid as i32);

        send_tree_signal(nix_pid, Signal::SIGTERM)?;

        // Wait for graceful termination
        let wait_interval = Duration::from_millis(100);
        let iterations = (timeout_ms / 100).max(1);

        for _ in 0..iterations {
            sleep(wait_interval);
            // Try to reap (non-blocking) - this handles zombies
            let _ = waitpid(nix_pid, Some(WaitPidFlag::WNOHANG));
            if !is_process_alive(pid) {
                debug!("Process {} terminated gracefully", pid);
                return Ok(true);
            }
        }

        // Process still running, use SIGKILL
        debug!("Process {} still running, sending SIGKILL", pid);
        send_tree_signal(nix_pid, Signal::SIGKILL)?;

        // Brief wait then reap the zombie
        sleep(Duration::from_millis(100));

        // Reap the zombie - this is critical!
        // waitpid() collects the exit status and removes the zombie from the process table.
        // Without this, the process stays as a zombie and is_process_alive() returns true.
        match waitpid(nix_pid, Some(WaitPidFlag::WNOHANG)) {
            Ok(status) => {
                debug!("Reaped process {}: {:?}", pid, status);
            }
            Err(e) => {
                // ECHILD means we're not the parent - that's fine, init will reap it
                if e != nix::errno::Errno::ECHILD {
                    debug!("waitpid({}) failed: {} (this is usually OK)", pid, e);
                }
            }
        }

        Ok(!is_process_alive(pid))
    }

    #[cfg(windows)]
    {
        // taskkill /T already handles the tree
        terminate_process_windows(pid)
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Fall back to single process termination
        terminate_process(pid, timeout_ms)
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

#[cfg(unix)]
fn send_tree_signal(pid: nix::unistd::Pid, signal: nix::sys::signal::Signal) -> Result<()> {
    use nix::sys::signal::{kill, killpg};

    let raw_pid = pid.as_raw();
    debug!(
        "Sending {:?} to process group {}, falling back to process if needed",
        signal, raw_pid
    );

    match killpg(pid, signal) {
        Ok(()) => return Ok(()),
        Err(nix::errno::Errno::ESRCH) => {
            debug!(
                "Process group {} not found while sending {:?}; trying process PID",
                raw_pid, signal
            );
        }
        Err(error) => {
            warn!(
                "Failed to send {:?} to process group {}: {}",
                signal, raw_pid, error
            );
        }
    }

    match kill(pid, signal) {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => Err(PumasError::Other(format!(
            "Failed to send {signal:?} to process {raw_pid}: {error}"
        ))),
    }
}

/// Scan for processes matching a pattern in their command line.
///
/// # Platform Behavior
/// - **Linux/macOS**: Uses `ps -eo pid=,args=`
/// - **Windows**: Uses `wmic process get processid,commandline`
///
/// Returns a list of (pid, cmdline) tuples.
pub fn find_processes_by_cmdline(pattern: &str) -> Vec<(u32, String)> {
    #[cfg(unix)]
    {
        find_processes_unix(pattern)
    }

    #[cfg(windows)]
    {
        find_processes_windows(pattern)
    }

    #[cfg(not(any(unix, windows)))]
    {
        vec![]
    }
}

#[cfg(unix)]
fn find_processes_unix(pattern: &str) -> Vec<(u32, String)> {
    use std::process::Command;

    let output = match Command::new("ps").args(["-eo", "pid=,args="]).output() {
        Ok(o) => o,
        Err(e) => {
            debug!("Failed to run ps: {}", e);
            return vec![];
        }
    };

    if !output.status.success() {
        return vec![];
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pattern_lower = pattern.to_lowercase();

    stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.len() != 2 {
                return None;
            }

            let pid: u32 = parts[0].trim().parse().ok()?;
            let cmdline = parts[1].trim();

            if cmdline.to_lowercase().contains(&pattern_lower) {
                Some((pid, cmdline.to_string()))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(windows)]
fn find_processes_windows(pattern: &str) -> Vec<(u32, String)> {
    use std::process::Command;

    let output = match Command::new("wmic")
        .args(["process", "get", "processid,commandline", "/format:csv"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            debug!("Failed to run wmic: {}", e);
            return vec![];
        }
    };

    if !output.status.success() {
        return vec![];
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pattern_lower = pattern.to_lowercase();

    stdout
        .lines()
        .skip(1) // Skip header
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            // CSV format: Node,CommandLine,ProcessId
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 3 {
                return None;
            }

            let cmdline = parts[1];
            let pid: u32 = parts[2].trim().parse().ok()?;

            if cmdline.to_lowercase().contains(&pattern_lower) {
                Some((pid, cmdline.to_string()))
            } else {
                None
            }
        })
        .collect()
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
    fn test_terminate_nonexistent() {
        // Terminating a nonexistent process should succeed
        let result = terminate_process(4_000_000_000, 1000);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pid_only_stop_preserves_another_owned_child_in_the_callers_group() {
        let mut child = Command::new("sleep").arg("60").spawn().unwrap();
        let mut sibling = Command::new("sleep").arg("60").spawn().unwrap();
        let result = terminate_process_tree(child.id(), 100);
        let sibling_running = sibling.try_wait().unwrap().is_none();
        if super::super::linux_group::observe_exit(child.id()).is_ok() {
            let _ = child.kill();
        }
        let _ = child.wait();
        let _ = sibling.kill();
        sibling.wait().unwrap();
        assert!(result.unwrap());
        assert!(sibling_running);
    }

    #[test]
    fn test_find_processes() {
        // Should find at least something (like our test runner)
        let processes = find_processes_by_cmdline("rust");
        // May or may not find matches depending on how tests are run
        let _ = processes;
    }

    #[cfg(unix)]
    #[test]
    fn test_terminate_process_tree_stops_detached_child_processes() {
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

        let stopped = terminate_process_tree(parent_pid, 1_000).unwrap();
        let _ = child.wait();

        assert!(stopped);
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
