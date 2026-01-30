//! Platform-specific process management.
//!
//! This module provides cross-platform abstractions for process management,
//! including checking process status and termination.

use crate::error::{PumasError, Result};
use tracing::{debug, warn};

/// Check if a process with the given PID is alive.
///
/// # Platform Behavior
/// - **Linux/macOS**: Uses `kill(pid, 0)` signal check
/// - **Windows**: Uses `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION`
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Use kill(pid, 0) to check if process exists
        // Signal 0 doesn't actually send a signal, just checks if we can
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !handle.is_null() {
                CloseHandle(handle);
                true
            } else {
                false
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
/// - **Linux/macOS**: Sends signal directly to the process (not process group)
/// - **Windows**: Uses `taskkill /T` which already handles the tree
///
/// Note: We use `kill(pid, ...)` instead of `killpg(pid, ...)` because the process
/// may not be a process group leader. Using killpg with a non-leader PID would
/// send signals to the wrong (or non-existent) process group.
pub fn terminate_process_tree(pid: u32, timeout_ms: u64) -> Result<bool> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
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

        // Kill the process directly (not the process group)
        debug!("Sending SIGTERM to process {}", pid);
        if let Err(e) = kill(nix_pid, Signal::SIGTERM) {
            if e == nix::errno::Errno::ESRCH {
                return Ok(true);
            }
            warn!("Failed to send SIGTERM to {}: {}", pid, e);
        }

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

    #[test]
    fn test_terminate_nonexistent() {
        // Terminating a nonexistent process should succeed
        let result = terminate_process(4_000_000_000, 1000);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_find_processes() {
        // Should find at least something (like our test runner)
        let processes = find_processes_by_cmdline("rust");
        // May or may not find matches depending on how tests are run
        let _ = processes;
    }
}
