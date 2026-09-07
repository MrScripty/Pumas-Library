//! Native conversion execution with cooperating Linux process-group custody.
//!
//! Both pipes are drained with bounded UTF-8 records. Callbacks must be short,
//! synchronous progress projections. Managed conversion workers retain this
//! future through cancellation/shutdown. Direct embedded callers must signal
//! cancellation and await it: dropping the future only invokes kill-on-drop,
//! which is not proof of cleanup. Linux keeps the group leader unreaped until
//! group cleanup completes; other targets retain foreground-only cleanup.
//! Escaped groups/namespaces and external child reapers are outside this contract.

use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use futures::FutureExt;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::time::{Instant, MissedTickBehavior};

use crate::cancel::CancellationToken;
use crate::{PumasError, Result};

const MAX_LINE_BYTES: usize = 64 * 1024;
const OBSERVATION_INTERVAL: Duration = Duration::from_millis(50);
const FINAL_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputStream {
    Stdout,
    Stderr,
}

fn failure(name: &str, reason: &str) -> PumasError {
    PumasError::ConversionFailed {
        message: format!("{name}: {reason}"),
    }
}

struct Records {
    pending: Vec<u8>,
}

impl Records {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    fn emit(
        &mut self,
        stream: OutputStream,
        callback: &mut impl FnMut(OutputStream, &str),
        name: &str,
    ) -> Result<()> {
        let line = std::str::from_utf8(&self.pending)
            .map_err(|_| failure(name, "subprocess output was not valid UTF-8"))?;
        callback(stream, line.strip_suffix('\r').unwrap_or(line));
        self.pending.clear();
        Ok(())
    }

    fn push(
        &mut self,
        bytes: &[u8],
        stream: OutputStream,
        callback: &mut impl FnMut(OutputStream, &str),
        name: &str,
    ) -> Result<()> {
        for byte in bytes {
            if *byte == b'\n' {
                self.emit(stream, callback, name)?;
            } else {
                if self.pending.len() == MAX_LINE_BYTES {
                    return Err(failure(name, "subprocess output record exceeded 64 KiB"));
                }
                self.pending.push(*byte);
            }
        }
        Ok(())
    }
}

pub(super) async fn run(
    command: &mut Command,
    name: &str,
    cancel: &CancellationToken,
    mut on_line: impl FnMut(OutputStream, &str) + Send,
) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(PumasError::ConversionCancelled);
    }
    #[cfg(target_os = "linux")]
    {
        super::linux_group::ensure_supported()
            .map_err(|error| failure(name, &format!("group custody unavailable: {error}")))?;
        command.process_group(0);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| failure(name, &format!("could not spawn subprocess: {error}")))?;
    // Keep Child outside the unwind boundary, so callback/stream panics cannot
    // drop it before the direct-child cleanup receipt has been observed.
    let result = std::panic::AssertUnwindSafe(async {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| failure(name, "stdout pipe unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| failure(name, "stderr pipe unavailable"))?;
        drain(&mut child, stdout, stderr, name, cancel, &mut on_line).await
    })
    .catch_unwind()
    .await
    .unwrap_or_else(|_| Err(failure(name, "subprocess output observer panicked")));
    // Linux must stop remaining group members even after a successful leader
    // exit. Its leader has only been observed, not reaped, inside drain.
    if cfg!(target_os = "linux") || result.is_err() {
        if let Err(cleanup_error) = cleanup(&mut child, name).await {
            return Err(match result {
                Err(operation_error) => PumasError::ConversionFailed {
                    message: format!("{operation_error}; cleanup also failed: {cleanup_error}"),
                },
                Ok(()) => cleanup_error,
            });
        }
    }
    result
}

async fn drain(
    child: &mut Child,
    mut stdout: impl AsyncRead + Unpin,
    mut stderr: impl AsyncRead + Unpin,
    name: &str,
    cancel: &CancellationToken,
    callback: &mut (impl FnMut(OutputStream, &str) + Send),
) -> Result<()> {
    let mut stdout_records = Records::new();
    let mut stderr_records = Records::new();
    let mut stdout_buffer = [0_u8; 8192];
    let mut stderr_buffer = [0_u8; 8192];
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut status: Option<ExitStatus> = None;
    let mut exited_at = None;
    let mut tick = tokio::time::interval(OBSERVATION_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        if let Some(status) = status {
            if !stdout_open && !stderr_open {
                return if status.success() {
                    Ok(())
                } else {
                    Err(failure(
                        name,
                        &format!("subprocess exited unsuccessfully: {status}"),
                    ))
                };
            }
        }
        if cancel.is_cancelled() {
            return Err(PumasError::ConversionCancelled);
        }
        if exited_at.is_some_and(|at: Instant| at.elapsed() >= FINAL_DRAIN_TIMEOUT) {
            return Err(failure(
                name,
                "output pipes remained open after foreground process exit",
            ));
        }
        tokio::select! {
            _ = tick.tick() => {}
            observed = observe_foreground(child), if status.is_none() => {
                status = Some(observed.map_err(|error| {
                    failure(name, &format!("could not observe subprocess exit: {error}"))
                })?);
                exited_at = Some(Instant::now());
                #[cfg(target_os = "linux")]
                super::linux_group::signal_group(owned_pid(child, name)?)
                    .map_err(|error| failure(name, &format!("could not stop process group: {error}")))?;
            }
            read = stdout.read(&mut stdout_buffer), if stdout_open => {
                let count = read.map_err(|error| {
                    failure(name, &format!("could not read stdout: {error}"))
                })?;
                if count == 0 {
                    stdout_open = false;
                    if !stdout_records.pending.is_empty() {
                        stdout_records.emit(OutputStream::Stdout, callback, name)?;
                    }
                } else {
                    stdout_records.push(&stdout_buffer[..count], OutputStream::Stdout, callback, name)?;
                }
            }
            read = stderr.read(&mut stderr_buffer), if stderr_open => {
                let count = read.map_err(|error| {
                    failure(name, &format!("could not read stderr: {error}"))
                })?;
                if count == 0 {
                    stderr_open = false;
                    if !stderr_records.pending.is_empty() {
                        stderr_records.emit(OutputStream::Stderr, callback, name)?;
                    }
                } else {
                    stderr_records.push(&stderr_buffer[..count], OutputStream::Stderr, callback, name)?;
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
async fn observe_foreground(child: &mut Child) -> std::io::Result<ExitStatus> {
    child.wait().await
}

#[cfg(target_os = "linux")]
async fn observe_foreground(child: &mut Child) -> std::io::Result<ExitStatus> {
    let pid = child
        .id()
        .ok_or_else(|| std::io::Error::other("Child was already reaped"))?;
    loop {
        if let Some(status) = super::linux_group::observe_exit(pid)? {
            return Ok(status);
        }
        tokio::time::sleep(OBSERVATION_INTERVAL).await;
    }
}

#[cfg(target_os = "linux")]
fn owned_pid(child: &Child, name: &str) -> Result<u32> {
    child
        .id()
        .ok_or_else(|| failure(name, "process-group leader was already reaped"))
}

#[cfg(target_os = "linux")]
async fn cleanup(child: &mut Child, name: &str) -> Result<()> {
    let pid = owned_pid(child, name)?;
    let mut first_failure = None;
    loop {
        match super::linux_group::signal_group(pid) {
            Ok(()) => {
                // Only observation runs in blocking capacity. The retained async
                // owner keeps Child alive and performs every signal itself; a
                // dropped waiter cannot leave a queued signal with a stale PID.
                let observation = tokio::task::spawn_blocking(move || {
                    super::linux_group::group_has_live_members(
                        i32::try_from(pid).expect("validated Linux PID"),
                    )
                })
                .await;
                match observation {
                    Ok(Ok(false)) => break,
                    Ok(Ok(true)) => {}
                    Ok(Err(error)) => {
                        first_failure.get_or_insert_with(|| {
                            format!("could not observe process group: {error}")
                        });
                    }
                    Err(error) => {
                        first_failure.get_or_insert_with(|| {
                            format!("process-group observation worker failed: {error}")
                        });
                    }
                }
            }
            Err(error) => {
                first_failure
                    .get_or_insert_with(|| format!("could not stop owned process group: {error}"));
            }
        }
        tokio::time::sleep(OBSERVATION_INTERVAL).await;
    }
    // Never signal this numeric group after wait releases the leader identity.
    loop {
        match child.wait().await {
            Ok(_) => break,
            Err(error) => {
                first_failure
                    .get_or_insert_with(|| format!("could not reap process-group leader: {error}"));
                tokio::time::sleep(OBSERVATION_INTERVAL).await;
            }
        }
    }
    match first_failure {
        Some(reason) => Err(failure(name, &reason)),
        None => Ok(()),
    }
}

#[cfg(not(target_os = "linux"))]
async fn cleanup(child: &mut Child, name: &str) -> Result<()> {
    let mut first_failure = None;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if let Err(error) = child.start_kill() {
                    first_failure
                        .get_or_insert_with(|| format!("could not terminate subprocess: {error}"));
                }
            }
            Err(error) => {
                first_failure.get_or_insert_with(|| {
                    format!("could not inspect subprocess during cleanup: {error}")
                });
                if let Err(error) = child.start_kill() {
                    first_failure
                        .get_or_insert_with(|| format!("could not terminate subprocess: {error}"));
                }
            }
        }
        match tokio::time::timeout(OBSERVATION_INTERVAL, child.wait()).await {
            Ok(Ok(_)) => break,
            Ok(Err(error)) => {
                first_failure.get_or_insert_with(|| format!("could not reap subprocess: {error}"));
                tokio::time::sleep(OBSERVATION_INTERVAL).await;
            }
            Err(_) => {}
        }
    }
    match first_failure {
        Some(reason) => Err(failure(name, &reason)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_limit_utf8_and_crlf_are_explicit() {
        let mut records = Records::new();
        let mut emitted = 0;
        let mut callback = |_, line: &str| {
            emitted += 1;
            assert!(line.len() <= MAX_LINE_BYTES);
        };
        records
            .push(
                &vec![b'x'; MAX_LINE_BYTES],
                OutputStream::Stdout,
                &mut callback,
                "fixture",
            )
            .expect("exact limit");
        records
            .push(b"\n", OutputStream::Stdout, &mut callback, "fixture")
            .expect("emit max line");
        records
            .push(
                b"crlf\r\n",
                OutputStream::Stderr,
                &mut |stream, line| {
                    assert_eq!(stream, OutputStream::Stderr);
                    assert_eq!(line, "crlf");
                },
                "fixture",
            )
            .expect("CRLF");
        assert_eq!(emitted, 1);
        assert!(records
            .push(
                &vec![b'x'; MAX_LINE_BYTES + 1],
                OutputStream::Stdout,
                &mut |_, _| {},
                "fixture"
            )
            .is_err());
        assert_eq!(records.pending.len(), MAX_LINE_BYTES);
        let mut malformed = Records::new();
        assert!(malformed
            .push(
                b"\xff\n",
                OutputStream::Stdout,
                &mut |_, _| panic!("invalid UTF-8 not emitted"),
                "fixture"
            )
            .is_err());
    }

    #[cfg(unix)]
    fn shell(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn drains_both_full_pipes_and_final_unterminated_records() {
        let mut command = shell("i=0; while [ \"$i\" -lt 4096 ]; do printf 'stdout-0123456789012345678901234567890123456789012345678901234567890123456789\\n'; printf 'stderr-0123456789012345678901234567890123456789012345678901234567890123456789\\n' >&2; i=$((i+1)); done; printf 'stdout-final'; printf 'stderr-final' >&2");
        let mut counts = [0, 0];
        let mut finals = [false, false];
        tokio::time::timeout(
            Duration::from_secs(10),
            run(
                &mut command,
                "flood fixture",
                &CancellationToken::new(),
                |stream, line| {
                    let index = usize::from(stream == OutputStream::Stderr);
                    counts[index] += 1;
                    if line.ends_with("-final") {
                        finals[index] = true;
                    }
                },
            ),
        )
        .await
        .expect("dual pipe drain does not deadlock")
        .expect("child success");
        assert_eq!(counts, [4097, 4097]);
        assert_eq!(finals, [true, true]);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn quiet_child_cancellation_reaps_before_return() {
        let root = tempfile::tempdir().expect("fixture root");
        let marker = root.path().join("pid");
        let mut command = shell("echo $$ > \"$1\"; exec sleep 30");
        command.arg("fixture").arg(&marker);
        let token = CancellationToken::new();
        let cancel = async {
            let pid: u32 = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Ok(contents) = tokio::fs::read_to_string(&marker).await {
                        if let Ok(pid) = contents.trim().parse() {
                            break pid;
                        }
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("quiet fixture started");
            token.cancel();
            (pid, Instant::now())
        };
        let (result, (pid, cancelled_at)) = tokio::join!(
            run(&mut command, "quiet fixture", &token, |_, _| {}),
            cancel
        );
        assert!(matches!(result, Err(PumasError::ConversionCancelled)));
        assert!(cancelled_at.elapsed() < Duration::from_secs(2));
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "direct child reaped before cancellation result"
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn callback_panic_is_reported_only_after_foreground_cleanup() {
        let mut command = shell("printf '%s\\n' \"$$\"; exec sleep 30");
        let mut pid = None;
        let result = run(
            &mut command,
            "panic fixture",
            &CancellationToken::new(),
            |_, line| {
                pid = Some(line.parse::<u32>().expect("fixture PID"));
                panic!("controlled observer panic");
            },
        )
        .await;
        assert!(
            matches!(result, Err(PumasError::ConversionFailed { message }) if message.contains("observer panicked"))
        );
        assert!(
            !std::path::Path::new(&format!("/proc/{}", pid.expect("callback invoked"))).exists()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_exit_and_malformed_output_errors_are_reported() {
        let root = tempfile::tempdir().expect("root");
        let mut missing = Command::new(root.path().join("missing-executable"));
        assert!(run(
            &mut missing,
            "missing",
            &CancellationToken::new(),
            |_, _| {}
        )
        .await
        .is_err());
        let mut nonzero = shell("exit 7");
        assert!(
            matches!(run(&mut nonzero, "nonzero", &CancellationToken::new(), |_, _| {}).await, Err(PumasError::ConversionFailed { message }) if message.contains("nonzero") && message.contains('7'))
        );
        let mut malformed = shell("printf '\\377\\n'; exec sleep 30");
        assert!(
            matches!(run(&mut malformed, "malformed", &CancellationToken::new(), |_, _| {}).await, Err(PumasError::ConversionFailed { message }) if message.contains("UTF-8"))
        );
        let mut oversized = shell("i=0; while [ \"$i\" -lt 1200 ]; do printf '0123456789012345678901234567890123456789012345678901234567890123'; i=$((i+1)); done; exec sleep 30");
        assert!(
            matches!(run(&mut oversized, "oversized", &CancellationToken::new(), |_, _| {}).await, Err(PumasError::ConversionFailed { message }) if message.contains("64 KiB"))
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pre_cancelled_call_does_not_start_executable() {
        let root = tempfile::tempdir().expect("fixture root");
        let marker = root.path().join("started");
        let mut command = shell("printf started > \"$1\"");
        command.arg("fixture").arg(&marker);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            run(
                &mut command,
                "pre-cancelled fixture",
                &cancel,
                |_, _| panic!("pre-cancelled callback")
            )
            .await,
            Err(PumasError::ConversionCancelled)
        ));
        assert!(!marker.exists());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn group_descendants_stop_before_success_cancellation_or_observer_failure() {
        for outcome in ["success", "cancel", "panic"] {
            let script = if outcome == "success" {
                "sleep 30 >/dev/null 2>&1 & echo $!; exit 0"
            } else {
                "sleep 30 & echo $!; wait"
            };
            let mut command = shell(script);
            let token = CancellationToken::new();
            let mut descendant = None;
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                run(&mut command, "group fixture", &token, |stream, line| {
                    assert_eq!(stream, OutputStream::Stdout);
                    descendant = Some(line.parse::<u32>().expect("descendant PID"));
                    match outcome {
                        "cancel" => token.cancel(),
                        "panic" => panic!("controlled group observer panic"),
                        _ => {}
                    }
                }),
            )
            .await
            .expect("bounded controlled group cleanup");
            match outcome {
                "success" => result.expect("successful leader and stopped group"),
                "cancel" => assert!(matches!(result, Err(PumasError::ConversionCancelled))),
                "panic" => assert!(
                    matches!(result, Err(PumasError::ConversionFailed { message }) if message.contains("observer panicked"))
                ),
                _ => unreachable!(),
            }
            // Independent kernel observation for this single-threaded sleep
            // fixture. Orphan zombies are stopped; their reaper is not Pumas.
            let stat = std::fs::read_to_string(format!(
                "/proc/{}/stat",
                descendant.expect("fixture started")
            ));
            match stat {
                Ok(stat) => assert!(
                    stat.rsplit(") ").next().unwrap().starts_with("Z "),
                    "descendant still executing: {stat}"
                ),
                Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn inherited_pipe_has_bounded_failure_without_descendant_cleanup_claim() {
        // Inject the leader-exited/pipe-still-open condition at the drain seam.
        // The test owns the pipe holder independently and explicitly reaps it;
        // this does not assert ownership of a runner-created descendant.
        let mut holder = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("pipe holder");
        let holder_stdout = holder.stdout.take().expect("holder stdout");
        let holder_stderr = holder.stderr.take().expect("holder stderr");
        let mut leader = shell("exit 0");
        leader.process_group(0);
        let mut child = leader
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("short foreground leader");
        let start = Instant::now();
        let result = drain(
            &mut child,
            holder_stdout,
            holder_stderr,
            "inherited-pipe fixture",
            &CancellationToken::new(),
            &mut |_, _| {},
        )
        .await;
        holder
            .kill()
            .await
            .expect("stop and reap independently owned pipe holder");
        child.wait().await.expect("reap foreground leader");
        assert!(
            matches!(result, Err(PumasError::ConversionFailed { message }) if message.contains("pipes remained open"))
        );
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
