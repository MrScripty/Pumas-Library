//! Original-owner installed audio session. A session cannot qualify a runtime.
//! Shipping admission stays closed until its separate model/recipe policy exists.

#![allow(dead_code)] // Conditional producer; source-owned admission remains closed.

use super::audio_channel::PrivateAudioChannel;
use super::audio_client::OwnedAudioClient;
use super::audio_custody::AudioCustodyRegistry;
use super::audio_endpoint::{AudioEndpoints, OwnedAudioEndpoint};
use super::audio_runtime::AudioRuntimeOwner;
use crate::model_library::artifact_use::PreparedArtifactUse;
use crate::model_library::ModelLibrary;
use crate::models::RuntimeProfileId;
use crate::platform::managed_child::{ManagedChild, ManagedChildCustodySlot};
use crate::{PumasError, Result};
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Notify};

const STARTUP_BUDGET: Duration = Duration::from_secs(120);
const DRAIN_ATTEMPT: Duration = Duration::from_secs(5);

type Pipes = (std::process::ChildStdin, std::process::ChildStdout, u32);

struct Supervisor {
    stop: AtomicBool,
    done: AtomicBool,
    diagnostics_ok: AtomicBool,
    #[cfg(test)]
    fail_diagnostics: AtomicBool,
    changed: Notify,
    registry: Arc<AudioCustodyRegistry>,
    custody: Arc<ManagedChildCustodySlot>,
}
impl Supervisor {
    fn stop(&self) {
        self.registry.close_admission();
        self.stop.store(true, Ordering::Release);
    }
    async fn wait(&self) -> Result<()> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.done.load(Ordering::Acquire) {
                return if self.diagnostics_ok.load(Ordering::Acquire) {
                    Ok(())
                } else {
                    Err(failure(
                        "audio diagnostic stream did not reach confirmed EOF",
                    ))
                };
            }
            changed.await;
        }
    }
}
struct StartupGuard(Option<Arc<Supervisor>>);
impl Drop for StartupGuard {
    fn drop(&mut self) {
        if let Some(supervisor) = &self.0 {
            supervisor.stop();
        }
    }
}

/// Retaining an endpoint alone never retains session admission. The producer
/// must keep this owner; dropping it closes admission and requests exact drain.
#[must_use]
pub(crate) struct InstalledAudioSession {
    endpoint: OwnedAudioEndpoint,
    channel: Arc<PrivateAudioChannel>,
    supervisor: Arc<Supervisor>,
}
impl InstalledAudioSession {
    pub(crate) async fn launch(
        library: Arc<ModelLibrary>,
        endpoints: &AudioEndpoints,
        runtime: Arc<AudioRuntimeOwner>,
        selected: Arc<PreparedArtifactUse>,
        profile: RuntimeProfileId,
        generation: u64,
    ) -> Result<Self> {
        selected.validate_library_owner(&library)?;
        if !runtime.permits_selected(&selected) {
            return Err(failure(
                "audio session selection does not belong to runtime",
            ));
        }
        // Correlation only: neither this digest nor the hello reply qualifies
        // bytes. The opaque runtime's installed spawn gate remains authoritative.
        let source_id = format!("pumas-cohere-owned-v1:{}", selected.manifest_sha256());
        let registry = AudioCustodyRegistry::new(profile.clone(), generation);
        let supervisor = Arc::new(Supervisor {
            stop: AtomicBool::new(false),
            done: AtomicBool::new(false),
            diagnostics_ok: AtomicBool::new(true),
            #[cfg(test)]
            fail_diagnostics: AtomicBool::new(false),
            changed: Notify::new(),
            registry: registry.clone(),
            custody: ManagedChildCustodySlot::new(),
        });
        let mut startup = StartupGuard(Some(supervisor.clone()));
        let (send, receive) = oneshot::channel();
        let worker = supervisor.clone();
        let worker_library = library.clone();
        let worker_selected = selected.clone();
        // No await before the worker owns cleanup and startup cancellation owns
        // its stop request. Cancelling a waiter never cancels this blocking owner.
        tokio::task::spawn_blocking(move || {
            run_worker(worker, runtime, worker_selected, worker_library, send)
        });
        let startup_result = tokio::time::timeout(STARTUP_BUDGET, async {
            let (input, output, pid) = receive
                .await
                .map_err(|_| failure("audio worker startup lost"))?
                .map_err(|_| failure("audio installed child refused"))?;
            let stopped = supervisor.clone();
            let channel =
                PrivateAudioChannel::from_pipes(input, output, Arc::new(move || stopped.stop()))
                    .map_err(|_| failure("audio private pipes unavailable"))?;
            let client =
                OwnedAudioClient::bind(channel.clone(), registry, profile, generation, pid)
                    .await
                    .map_err(|_| failure("audio private binding refused"))?;
            let slot = client
                .load(selected, &source_id)
                .await
                .map_err(|_| failure("audio selected load refused"))?;
            let endpoint = endpoints.register(library, slot)?;
            Ok::<_, PumasError>((channel, endpoint))
        })
        .await
        .map_err(|_| failure("audio session startup timed out"))??;
        startup.0 = None;
        Ok(Self {
            channel: startup_result.0,
            endpoint: startup_result.1,
            supervisor,
        })
    }
    pub(crate) fn endpoint(&self) -> OwnedAudioEndpoint {
        self.endpoint.clone()
    }
    pub(crate) async fn stop_and_wait(&self) -> Result<()> {
        self.supervisor.stop();
        self.channel.quarantine();
        self.supervisor.wait().await
    }
}
impl Drop for InstalledAudioSession {
    fn drop(&mut self) {
        self.supervisor.stop();
        self.channel.quarantine();
    }
}
fn failure(message: &str) -> PumasError {
    PumasError::Other(message.into())
}

fn consume_diagnostics(mut input: impl Read) -> io::Result<()> {
    let mut buffer = [0u8; 4096];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn run_worker(
    supervisor: Arc<Supervisor>,
    runtime: Arc<AudioRuntimeOwner>,
    selected: Arc<PreparedArtifactUse>,
    library: Arc<ModelLibrary>,
    send: oneshot::Sender<io::Result<Pipes>>,
) {
    let launch_runtime = runtime.clone();
    let registry = supervisor.registry.clone();
    run_owned_worker(
        supervisor,
        library,
        send,
        move |custody| {
            let spawned = launch_runtime.spawn_installed_child(selected, custody)?;
            Ok((spawned.child, spawned.diagnostics))
        },
        move |child| {
            registry
                .retain_runtime_and_attach(runtime, child)
                .map_err(|_| io::Error::other("audio registry attachment refused"))
        },
    );
}

fn run_owned_worker<T, F, A>(
    supervisor: Arc<Supervisor>,
    keepalive: Arc<T>,
    send: oneshot::Sender<io::Result<Pipes>>,
    create: F,
    attach: A,
) where
    T: Send + Sync + 'static,
    F: FnOnce(
        Arc<ManagedChildCustodySlot>,
    ) -> io::Result<(ManagedChild, std::process::ChildStderr)>,
    A: FnOnce(&mut ManagedChild) -> io::Result<()>,
{
    let mut child: Option<ManagedChild> = None;
    let mut diagnostics = None;
    let setup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> io::Result<Pipes> {
        if supervisor.stop.load(Ordering::Acquire) {
            return Err(io::Error::other("audio startup cancelled"));
        }
        let (spawned, diagnostic_pipe) = create(supervisor.custody.clone())?;
        child = Some(spawned);
        let owned = child
            .as_mut()
            .ok_or_else(|| io::Error::other("audio child missing"))?;
        // Physical library lifetime and runtime/model custody precede protocol.
        owned.attach_cleanup_lease(keepalive.clone());
        attach(owned)?;
        #[cfg(test)]
        if supervisor.fail_diagnostics.load(Ordering::Acquire) {
            return Err(io::Error::other("injected diagnostics admission failure"));
        }
        diagnostics = Some(
            std::thread::Builder::new()
                .name("pumas-audio-diagnostics".into())
                .spawn(move || {
                    // Consume without unbounded buffering or exposing arbitrary child
                    // output. EOF is joined only after the entire child tree drains.
                    consume_diagnostics(diagnostic_pipe)
                })?,
        );
        let pid = owned.id();
        let (input, output) = owned.take_private_stdio()?;
        Ok((input, output, pid))
    }))
    .unwrap_or_else(|_| Err(io::Error::other("audio startup panicked")));
    let setup_ok = setup.is_ok();
    if send.send(setup).is_err() || !setup_ok {
        supervisor.stop();
    }
    while !supervisor.stop.load(Ordering::Acquire) {
        match child.as_mut().map(ManagedChild::observe_exit) {
            Some(Ok(None)) => std::thread::sleep(Duration::from_millis(25)),
            _ => supervisor.stop(),
        }
    }
    supervisor.registry.close_admission();
    // Every unsuccessful observation/drain preserves the original child and
    // composite leases. Retry is owned here even after every API waiter leaves.
    loop {
        let drained = if let Some(owned) = &mut child {
            owned.terminate_and_drain(DRAIN_ATTEMPT).is_ok()
        } else if supervisor.custody.is_active() || supervisor.custody.has_parked_child() {
            supervisor.custody.drain(DRAIN_ATTEMPT).is_ok()
        } else {
            true
        };
        if drained {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(child);
    if let Some(reader) = diagnostics {
        if !matches!(reader.join(), Ok(Ok(()))) {
            supervisor.diagnostics_ok.store(false, Ordering::Release);
        }
    }
    drop(keepalive);
    supervisor.done.store(true, Ordering::Release);
    supervisor.changed.notify_waiters();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn diagnostic_read_error_is_not_eof() {
        struct FailedRead;
        impl Read for FailedRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("controlled diagnostic failure"))
            }
        }
        assert!(consume_diagnostics(FailedRead).is_err());
        assert!(consume_diagnostics(&b"bounded diagnostic bytes"[..]).is_ok());
    }

    fn supervisor() -> Arc<Supervisor> {
        Arc::new(Supervisor {
            stop: AtomicBool::new(false),
            done: AtomicBool::new(false),
            diagnostics_ok: AtomicBool::new(true),
            fail_diagnostics: AtomicBool::new(false),
            changed: Notify::new(),
            registry: AudioCustodyRegistry::new(
                RuntimeProfileId::parse("session-lifecycle-fixture").unwrap(),
                1,
            ),
            custody: ManagedChildCustodySlot::new(),
        })
    }
    fn child(
        custody: Arc<ManagedChildCustodySlot>,
    ) -> io::Result<(ManagedChild, std::process::ChildStderr)> {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 30"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = ManagedChild::spawn(&mut command, custody)?;
        let stderr = child.take_private_stderr()?;
        Ok((child, stderr))
    }
    async fn completed(supervisor: &Supervisor) {
        tokio::time::timeout(Duration::from_secs(10), supervisor.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(!supervisor.custody.is_active());
        assert!(!supervisor.custody.has_parked_child());
    }
    #[tokio::test]
    async fn cancelled_before_spawn_has_no_child_effect_or_retained_owner() {
        let supervisor = supervisor();
        drop(StartupGuard(Some(supervisor.clone())));
        let lease = Arc::new(());
        let weak = Arc::downgrade(&lease);
        let (send, receive) = oneshot::channel();
        let worker = supervisor.clone();
        let invoked = Arc::new(AtomicBool::new(false));
        let worker_invoked = invoked.clone();
        tokio::task::spawn_blocking(move || {
            run_owned_worker(
                worker,
                lease,
                send,
                move |_| {
                    worker_invoked.store(true, Ordering::Release);
                    Err(io::Error::other("cancelled startup must not spawn"))
                },
                |_| Ok(()),
            )
        });
        assert!(receive.await.unwrap().is_err());
        completed(&supervisor).await;
        assert!(weak.upgrade().is_none());
        assert!(!invoked.load(Ordering::Acquire));
    }
    #[tokio::test]
    async fn spawn_refusal_has_no_child_effect_or_retained_owner() {
        let supervisor = supervisor();
        let lease = Arc::new(());
        let weak = Arc::downgrade(&lease);
        let (send, receive) = oneshot::channel();
        let worker = supervisor.clone();
        tokio::task::spawn_blocking(move || {
            run_owned_worker(
                worker,
                lease,
                send,
                |_| Err(io::Error::other("controlled refusal")),
                |_| Ok(()),
            )
        });
        assert!(receive.await.unwrap().is_err());
        completed(&supervisor).await;
        assert!(weak.upgrade().is_none());
    }
    #[tokio::test]
    async fn attachment_and_diagnostic_failures_drain_original_child() {
        for diagnostic in [false, true] {
            let supervisor = supervisor();
            supervisor
                .fail_diagnostics
                .store(diagnostic, Ordering::Release);
            let lease = Arc::new(());
            let weak = Arc::downgrade(&lease);
            let (send, receive) = oneshot::channel();
            let worker = supervisor.clone();
            tokio::task::spawn_blocking(move || {
                run_owned_worker(worker, lease, send, child, move |_| {
                    if diagnostic {
                        Ok(())
                    } else {
                        Err(io::Error::other("controlled attachment failure"))
                    }
                })
            });
            assert!(receive.await.unwrap().is_err());
            completed(&supervisor).await;
            assert!(weak.upgrade().is_none());
        }
    }
    #[tokio::test]
    async fn lost_startup_waiter_still_drains_original_child() {
        let supervisor = supervisor();
        let lease = Arc::new(());
        let weak = Arc::downgrade(&lease);
        let (send, receive) = oneshot::channel();
        drop(receive);
        let worker = supervisor.clone();
        tokio::task::spawn_blocking(move || {
            run_owned_worker(worker, lease, send, child, |_| Ok(()))
        });
        completed(&supervisor).await;
        assert!(weak.upgrade().is_none());
    }
    #[cfg(not(target_env = "uclibc"))]
    #[tokio::test]
    async fn uncertain_drain_retains_owners_until_retry_succeeds() {
        let supervisor = supervisor();
        let lease = Arc::new(());
        let weak = Arc::downgrade(&lease);
        let (send, receive) = oneshot::channel();
        let (control_send, control_receive) = oneshot::channel();
        let worker = supervisor.clone();
        tokio::task::spawn_blocking(move || {
            run_owned_worker(
                worker,
                lease,
                send,
                move |custody| {
                    let (child, stderr) = child(custody)?;
                    let control = child.test_observation_failure_control();
                    control.store(true, Ordering::Release);
                    control_send.send(control).unwrap();
                    Ok((child, stderr))
                },
                |_| Ok(()),
            )
        });
        let _pipes = receive.await.unwrap().unwrap();
        let control = control_receive.await.unwrap();
        supervisor.stop();
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!supervisor.done.load(Ordering::Acquire));
        assert!(weak.upgrade().is_some());
        control.store(false, Ordering::Release);
        completed(&supervisor).await;
        assert!(weak.upgrade().is_none());
    }
}
