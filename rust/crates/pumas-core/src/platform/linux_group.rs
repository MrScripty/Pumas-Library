//! Linux cooperating-process-group observation. Callers retain an unreaped
//! direct child whose PID is its PGID until group cleanup completes. The host
//! must not externally reap these children or enable SIGCHLD auto-reaping.
//! Escaped processes, hostile namespaces and external reapers are not contained.

use std::io;
use std::path::Path;
use std::process::ExitStatus;

use nix::sys::signal::{killpg, Signal};
use nix::unistd::{getpgid, Pid};

pub(crate) fn ensure_supported() -> io::Result<()> {
    if cfg!(target_env = "uclibc") {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Non-reaping conversion group observation is unavailable on uclibc",
        ))
    } else {
        Ok(())
    }
}

fn checked_pid(pid: u32) -> io::Result<Pid> {
    let pid = i32::try_from(pid)
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Expected positive owned child PID",
            )
        })?;
    Ok(Pid::from_raw(pid))
}

/// Observe terminal status without releasing the child's PID/PGID pin.
pub(crate) fn observe_exit(pid: u32) -> io::Result<Option<ExitStatus>> {
    ensure_supported()?;
    let pid = checked_pid(pid)?;
    #[cfg(not(target_env = "uclibc"))]
    {
        use rustix::process::{waitid, WaitId, WaitIdOptions};
        use std::os::unix::process::ExitStatusExt;
        let pid = rustix::process::Pid::from_raw(pid.as_raw()).expect("validated positive PID");
        loop {
            let result = waitid(
                WaitId::Pid(pid),
                WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG,
            );
            return match result {
                Ok(None) => Ok(None),
                Ok(Some(status)) => {
                    if let Some(code) = status.exit_status() {
                        Ok(Some(ExitStatus::from_raw(code << 8)))
                    } else if let Some(signal) = status.terminating_signal() {
                        // Preserve raw Linux signal numbers, including realtime
                        // signals not represented by nix's closed Signal enum.
                        Ok(Some(ExitStatus::from_raw(
                            signal | if status.dumped() { 0x80 } else { 0 },
                        )))
                    } else {
                        Err(io::Error::other(
                            "Unexpected nonterminal conversion child observation",
                        ))
                    }
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => Err(error.into()),
            };
        }
    }
    #[cfg(target_env = "uclibc")]
    {
        let _ = pid;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Non-reaping conversion observation unavailable",
        ))
    }
}

/// Signal only while direct-child ownership and its group identity are proven.
/// Call synchronously while retaining Child, never queue a PID-only signal task.
pub(crate) fn signal_group(pid: u32) -> io::Result<()> {
    let group = checked_pid(pid)?;
    observe_exit(pid)?;
    if getpgid(Some(group)).map_err(io::Error::from)? != group {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Conversion child is not its process-group leader",
        ));
    }
    killpg(group, Signal::SIGKILL).map_err(Into::into)
}

pub(crate) fn group_has_live_members(group: i32) -> io::Result<bool> {
    if group <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected positive process group",
        ));
    }
    scan_group(Path::new("/proc"), group)
}

fn parse_process_group(stat: &[u8]) -> Option<(char, i32)> {
    let separator = stat.windows(2).rposition(|bytes| bytes == b") ")?;
    let fields = std::str::from_utf8(&stat[separator + 2..]).ok()?;
    let mut fields = fields.split_whitespace();
    let state = fields.next()?.chars().next()?;
    fields.next()?;
    Some((state, fields.next()?.parse().ok()?))
}

fn read_stat(path: &Path) -> io::Result<Option<(char, i32)>> {
    match std::fs::read(path) {
        Ok(bytes) => parse_process_group(&bytes)
            .map(Some)
            .ok_or_else(|| io::Error::other("Invalid process-group stat observation")),
        // procfs can return ESRCH if the task exits after its stat inode was
        // opened. Like ENOENT before open, this establishes disappearance, not
        // a failed observation of a still-present task.
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                || error.raw_os_error() == Some(nix::libc::ESRCH) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn live(state: char) -> bool {
    !matches!(state, 'Z' | 'X' | 'x')
}

fn scan_group(proc_root: &Path, group: i32) -> io::Result<bool> {
    for entry in std::fs::read_dir(proc_root)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let process = entry.path();
        let Some((state, observed_group)) = read_stat(&process.join("stat"))? else {
            continue;
        };
        if observed_group != group {
            continue;
        }
        if live(state) {
            return Ok(true);
        }
        // A zombie thread-group leader can still have live worker threads.
        let tasks = match std::fs::read_dir(process.join("task")) {
            Ok(tasks) => tasks,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if read_stat(&process.join("stat"))?.is_none() {
                    continue;
                }
                return Err(io::Error::other(
                    "Process remains present but task enumeration is unavailable",
                ));
            }
            Err(error) => return Err(error),
        };
        let mut observed_tasks = 0;
        let mut missing_task = false;
        for task in tasks {
            let task = task?;
            match read_stat(&task.path().join("stat"))? {
                Some((state, task_group)) => {
                    observed_tasks += 1;
                    if task_group == group && live(state) {
                        return Ok(true);
                    }
                }
                None => missing_task = true,
            }
        }
        if (observed_tasks == 0 || missing_task) && read_stat(&process.join("stat"))?.is_some() {
            return Err(io::Error::other(
                "Incomplete conversion task-state observation",
            ));
        }
    }
    Ok(false)
}

#[cfg(all(test, not(target_env = "uclibc")))]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    struct FixtureChild(Child);

    impl std::ops::Deref for FixtureChild {
        type Target = Child;
        fn deref(&self) -> &Child {
            &self.0
        }
    }

    impl std::ops::DerefMut for FixtureChild {
        fn deref_mut(&mut self) -> &mut Child {
            &mut self.0
        }
    }

    impl Drop for FixtureChild {
        fn drop(&mut self) {
            if observe_exit(self.0.id()).is_ok() {
                // The child remains ours even during test assertion unwind.
                // Nongroup fixtures reject group signalling but can be killed
                // directly. Already-reaped identities are never signalled.
                let _ = signal_group(self.0.id());
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    fn until(mut condition: impl FnMut() -> bool) {
        let start = Instant::now();
        while !condition() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "bounded process fixture observation"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn finish(child: &mut Child) {
        signal_group(child.id()).expect("signal owned fixture group");
        until(|| {
            matches!(
                group_has_live_members(i32::try_from(child.id()).expect("pid")),
                Ok(false)
            )
        });
        child.wait().expect("reap pinned fixture leader");
    }

    #[test]
    fn terminal_observation_retains_waitable_leader_and_reaped_ids_cannot_signal() {
        let mut child = FixtureChild(
            Command::new("sh")
                .args(["-c", "exit 7"])
                .process_group(0)
                .spawn()
                .expect("fixture leader"),
        );
        let pid = child.id();
        until(|| observe_exit(pid).expect("owned observation").is_some());
        assert_eq!(
            observe_exit(pid)
                .expect("repeat observation")
                .expect("exit")
                .code(),
            Some(7)
        );
        assert_eq!(child.wait().expect("still waitable").code(), Some(7));
        assert!(
            signal_group(pid).is_err(),
            "reaped PID must never be signalled"
        );
        assert!(signal_group(0).is_err());
    }

    #[test]
    fn nongroup_child_is_rejected_without_signalling_the_host_group() {
        let mut child = FixtureChild(
            Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("nongroup child"),
        );
        assert!(signal_group(child.id()).is_err());
        assert!(child.try_wait().expect("still alive").is_none());
        child.kill().expect("stop fixture");
        child.wait().expect("reap fixture");
    }

    #[test]
    fn realtime_signal_exit_remains_observable_and_cleanup_can_finish() {
        use std::os::unix::process::ExitStatusExt;
        let root = tempfile::tempdir().expect("realtime fixture root");
        let source = root.path().join("realtime_fixture.c");
        let executable = root.path().join("realtime_fixture");
        std::fs::write(&source, "#include <signal.h>\n#include <stdio.h>\nint main(void) { printf(\"%d\\n\", SIGRTMIN); fflush(stdout); raise(SIGRTMIN); return 2; }\n").expect("realtime fixture source");
        let compiler = Command::new("cc")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Linux test prerequisite: cc");
        assert!(
            compiler.status.success(),
            "{}",
            String::from_utf8_lossy(&compiler.stderr)
        );
        let mut child = FixtureChild(
            Command::new(executable)
                .stdout(std::process::Stdio::piped())
                .process_group(0)
                .spawn()
                .expect("realtime fixture"),
        );
        let pid = child.id();
        until(|| {
            observe_exit(pid)
                .expect("realtime terminal observation")
                .is_some()
        });
        let mut signal_text = String::new();
        std::io::Read::read_to_string(
            &mut child.stdout.take().expect("signal output"),
            &mut signal_text,
        )
        .expect("read realtime signal number");
        let expected: i32 = signal_text.trim().parse().expect("SIGRTMIN value");
        for _ in 0..2 {
            assert_eq!(
                observe_exit(pid)
                    .expect("repeat realtime observation")
                    .expect("terminal status")
                    .signal(),
                Some(expected)
            );
        }
        signal_group(pid).expect("owned realtime zombie can still validate group signalling");
        until(|| {
            matches!(
                group_has_live_members(i32::try_from(pid).expect("PID")),
                Ok(false)
            )
        });
        assert_eq!(
            child.wait().expect("final realtime child wait").signal(),
            Some(expected)
        );
        assert!(signal_group(pid).is_err(), "no signalling after final reap");
    }

    #[test]
    fn exited_group_leader_pins_descendant_cleanup() {
        let mut child = FixtureChild(
            Command::new("sh")
                .args(["-c", "sleep 30 & exit 0"])
                .process_group(0)
                .spawn()
                .expect("leader with descendant"),
        );
        until(|| observe_exit(child.id()).expect("observe leader").is_some());
        assert!(
            group_has_live_members(i32::try_from(child.id()).expect("pid"))
                .expect("live descendant")
        );
        finish(&mut child);
    }

    #[test]
    fn zombie_thread_leader_does_not_hide_live_worker() {
        let root = tempfile::tempdir().expect("fixture root");
        let source = root.path().join("thread_fixture.c");
        let executable = root.path().join("thread_fixture");
        std::fs::write(&source, "#include <pthread.h>\n#include <unistd.h>\nstatic void *worker(void *unused) { (void)unused; sleep(10); return 0; }\nint main(void) { pthread_t t; if (pthread_create(&t, 0, worker, 0)) return 2; pthread_exit(0); }\n").expect("fixture source");
        let compiler = Command::new("cc")
            .arg("-pthread")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Linux test prerequisite: cc and pthreads");
        assert!(
            compiler.status.success(),
            "{}",
            String::from_utf8_lossy(&compiler.stderr)
        );
        let mut child = FixtureChild(
            Command::new(executable)
                .process_group(0)
                .spawn()
                .expect("thread fixture"),
        );
        until(|| {
            matches!(
                read_stat(&Path::new("/proc").join(child.id().to_string()).join("stat")),
                Ok(Some(('Z', _)))
            )
        });
        assert!(
            !matches!(
                group_has_live_members(i32::try_from(child.id()).expect("pid")),
                Ok(false)
            ),
            "zombie leader cannot prove no live worker"
        );
        finish(&mut child);
    }

    #[test]
    fn reaped_process_stat_is_absent_even_when_opened_before_exit() {
        use std::os::fd::AsRawFd;

        let mut child = FixtureChild(
            Command::new("sleep")
                .arg("30")
                .process_group(0)
                .spawn()
                .expect("procfs fixture child"),
        );
        let stat = std::fs::File::open(format!("/proc/{}/stat", child.id()))
            .expect("open live process stat");
        child.kill().expect("stop fixture child");
        child.wait().expect("reap fixture child");
        // Reopen the retained procfs inode to deterministically model a process
        // exiting between the scanner's open and read, without a timing race.
        let retained = std::path::PathBuf::from(format!("/proc/self/fd/{}", stat.as_raw_fd()));
        let error = std::fs::read(&retained).expect_err("reaped procfs task is unavailable");
        assert_eq!(error.raw_os_error(), Some(nix::libc::ESRCH));
        assert!(read_stat(&retained)
            .expect("disappeared process is absent")
            .is_none());
    }

    #[test]
    fn parser_and_ambiguous_task_visibility_fail_closed() {
        assert_eq!(
            parse_process_group(b"81 (worker (name)\xff) S 4 81 81"),
            Some(('S', 81))
        );
        assert_eq!(parse_process_group(b"invalid"), None);
        let root = tempfile::tempdir().expect("proc fixture");
        let process = root.path().join("81");
        std::fs::create_dir(&process).expect("process");
        std::fs::write(process.join("stat"), b"81 (leader) Z 4 81 81").expect("zombie leader");
        assert!(scan_group(root.path(), 81).is_err());
        let task = process.join("task/82");
        std::fs::create_dir_all(&task).expect("worker task");
        std::fs::write(task.join("stat"), b"82 (worker) S 4 81 81").expect("live worker");
        assert!(scan_group(root.path(), 81).expect("thread scan"));
    }
}
