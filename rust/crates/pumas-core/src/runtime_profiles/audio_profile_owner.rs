//! Profile lifecycle for conditionally qualified installed audio. No policy grants.
use super::audio_endpoint::{AudioEndpoints, OwnedAudioEndpoint};
use super::audio_runtime::AudioRuntimeOwner;
use super::audio_session::{InstalledAudioSession, SessionControl};
use super::RuntimeProfileOperationGuard;
use crate::model_library::{artifact_use::PreparedArtifactUse, ModelLibrary};
use crate::models::RuntimeProfileId;
use crate::{PumasError, Result};
use futures::FutureExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch};

type Terminal = Option<std::result::Result<(), String>>;

struct Record {
    generation: u64,
    model: String,
    profile_guard: Mutex<Option<Arc<RuntimeProfileOperationGuard>>>,
    control: SessionControl,
    endpoint: Mutex<Option<OwnedAudioEndpoint>>,
    terminal: watch::Receiver<Terminal>,
}
impl Record {
    async fn join(&self) -> Result<bool> {
        let mut terminal = self.terminal.clone();
        loop {
            if let Some(result) = terminal.borrow_and_update().clone() {
                return result.map(|()| true).map_err(PumasError::Other);
            }
            if terminal.changed().await.is_err() {
                self.control.stop();
                self.control.join().await?;
                return Err(failure("audio lifecycle worker lost"));
            }
        }
    }
}
#[derive(Default)]
struct Registry {
    closed: bool,
    sessions: HashMap<RuntimeProfileId, Arc<Record>>,
}
#[derive(Default)]
pub(crate) struct AudioProfileOwner {
    registry: Arc<Mutex<Registry>>,
}
impl std::fmt::Debug for AudioProfileOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioProfileOwner").finish_non_exhaustive()
    }
}
struct CancelOnDrop(Option<SessionControl>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(control) = &self.0 {
            control.stop();
        }
    }
}
impl Drop for AudioProfileOwner {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.closed = true;
            for session in registry.sessions.values() {
                session.control.stop();
            }
        }
    }
}
impl AudioProfileOwner {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn launch<F, Fut>(
        &self,
        library: Arc<ModelLibrary>,
        endpoints: Arc<AudioEndpoints>,
        profile: RuntimeProfileId,
        model: String,
        generation: u64,
        guard: RuntimeProfileOperationGuard,
        serving: Arc<crate::serving::ServingService>,
        prepare: F,
    ) -> Result<OwnedAudioEndpoint>
    where
        F: FnOnce(Arc<RuntimeProfileOperationGuard>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(Arc<AudioRuntimeOwner>, Arc<PreparedArtifactUse>)>>
            + Send
            + 'static,
    {
        let guard = Arc::new(guard);
        let (terminal_send, terminal) = watch::channel(None);
        let record = Arc::new(Record {
            generation,
            model,
            profile_guard: Mutex::new(Some(guard.clone())),
            control: SessionControl::new(profile.clone(), generation),
            endpoint: Mutex::new(None),
            terminal,
        });
        {
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| failure("audio profile registry poisoned"))?;
            if registry.closed || registry.sessions.contains_key(&profile) {
                return Err(failure("audio profile admission unavailable"));
            }
            registry.sessions.insert(profile.clone(), record.clone());
        }
        let mut cancelled = CancelOnDrop(Some(record.control.clone()));
        let (send, receive) = oneshot::channel();
        let registry = self.registry.clone();
        tokio::spawn(async move {
            let _task_cancel = CancelOnDrop(Some(record.control.clone()));
            let startup = async {
                // Stop closes admission immediately, but this owner still joins
                // already admitted preparation before any child can be launched.
                let (runtime, selected) = match prepare(guard.clone()).await {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        let _ = send.send(Err(error));
                        return;
                    }
                };
                let result = tokio::select! {
                    biased;
                    _ = record.control.stopped() => Err(failure("audio profile startup cancelled")),
                    result = InstalledAudioSession::launch(
                        library, &endpoints, runtime, selected, profile.clone(), generation,
                        record.control.clone(), guard.clone(),
                    ) => result,
                };
                match result {
                    Ok(session) => {
                        if let Ok(mut endpoint) = record.endpoint.lock() {
                            *endpoint = Some(session.endpoint());
                        }
                        if send.send(Ok(session.endpoint())).is_err() {
                            record.control.stop();
                        }
                        record.control.stopped().await;
                        drop(session);
                    }
                    Err(error) => {
                        let _ = send.send(Err(error));
                    }
                }
            };
            let task_panicked = std::panic::AssertUnwindSafe(startup)
                .catch_unwind()
                .await
                .is_err();
            record.control.stop();
            let mut drained = record
                .control
                .join()
                .await
                .map_err(|error| error.to_string());
            if task_panicked && drained.is_ok() {
                drained = Err("audio lifecycle worker panicked after confirmed drain".into());
            }
            // Child and diagnostics have joined, even when the latter failed.
            if let Ok(mut endpoint) = record.endpoint.lock() {
                if let Some(endpoint) = endpoint.take() {
                    endpoints.retire_drained(&endpoint);
                }
            }
            serving.record_profile_unavailable(&profile).await;
            // Retain the shared profile guard through startup cancellation and all
            // uncertain drainage. Updates/deletes/other providers cannot overlap.
            if let Ok(mut registry) = registry.lock() {
                if registry
                    .sessions
                    .get(&profile)
                    .is_some_and(|current| Arc::ptr_eq(current, &record))
                {
                    registry.sessions.remove(&profile);
                }
            }
            match record.profile_guard.lock() {
                Ok(mut retained) => {
                    retained.take();
                }
                Err(_) => {
                    drained = Err("audio profile guard custody poisoned".into());
                }
            }
            drop(guard);
            terminal_send.send_replace(Some(drained));
        });
        let endpoint = receive
            .await
            .map_err(|_| failure("audio startup worker lost"))??;
        cancelled.0 = None;
        Ok(endpoint)
    }
    pub(crate) fn with_running_generation<T>(
        &self,
        profile: &RuntimeProfileId,
        generation: u64,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("audio registry poisoned"))?;
        let record = registry
            .sessions
            .get(profile)
            .ok_or_else(|| failure("audio generation missing"))?;
        if record.generation != generation
            || record.control.is_stopped()
            || !record
                .endpoint
                .lock()
                .map_err(|_| failure("audio endpoint poisoned"))?
                .as_ref()
                .is_some_and(OwnedAudioEndpoint::available)
        {
            return Err(failure("audio generation is not running"));
        }
        f()
    }
    pub(crate) fn generation_for_model(
        &self,
        profile: &RuntimeProfileId,
        model: &str,
    ) -> Result<Option<u64>> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("audio registry poisoned"))?;
        Ok(registry
            .sessions
            .get(profile)
            .filter(|record| record.model == model)
            .map(|record| record.generation))
    }
    pub(crate) fn cancel_generation(&self, profile: &RuntimeProfileId, generation: u64) {
        if let Ok(registry) = self.registry.lock() {
            if let Some(record) = registry
                .sessions
                .get(profile)
                .filter(|record| record.generation == generation)
            {
                record.control.stop();
            }
        }
    }
    pub(crate) fn statuses(&self) -> Result<Vec<crate::models::RuntimeProfileStatus>> {
        use crate::models::{RuntimeLifecycleState, RuntimeProfileStatus};
        let registry = self
            .registry
            .lock()
            .map_err(|_| failure("audio registry poisoned"))?;
        registry
            .sessions
            .iter()
            .map(|(profile, record)| {
                let ready = record
                    .endpoint
                    .lock()
                    .map_err(|_| failure("audio endpoint poisoned"))?
                    .as_ref()
                    .is_some_and(OwnedAudioEndpoint::available);
                Ok(RuntimeProfileStatus {
                    profile_id: profile.clone(),
                    state: if record.control.is_stopped() {
                        RuntimeLifecycleState::Stopping
                    } else if ready {
                        RuntimeLifecycleState::Running
                    } else {
                        RuntimeLifecycleState::Starting
                    },
                    endpoint_url: None,
                    pid: None,
                    log_path: None,
                    last_error: None,
                })
            })
            .collect()
    }
    pub(crate) async fn stop(
        &self,
        profile: &RuntimeProfileId,
        generation: Option<u64>,
    ) -> Result<Option<Result<bool>>> {
        let record = self
            .registry
            .lock()
            .map_err(|_| failure("audio profile registry poisoned"))?
            .sessions
            .get(profile)
            .cloned();
        let Some(record) = record else {
            return Ok(None);
        };
        if generation.is_some_and(|expected| expected != record.generation) {
            return Ok(Some(Ok(false)));
        }
        record.control.stop();
        Ok(Some(record.join().await))
    }
    pub(crate) async fn close_and_drain(&self) -> Result<Vec<(RuntimeProfileId, Result<bool>)>> {
        let records = {
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| failure("audio profile registry poisoned"))?;
            registry.closed = true;
            let records: Vec<_> = registry
                .sessions
                .iter()
                .map(|(id, record)| (id.clone(), record.clone()))
                .collect();
            for (_, record) in &records {
                record.control.stop();
            }
            records
        };
        let mut results = Vec::new();
        for (id, record) in records {
            results.push((id, record.join().await));
        }
        Ok(results)
    }
}
fn failure(message: &str) -> PumasError {
    PumasError::Other(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn admitted(
        owner: &AudioProfileOwner,
        generation: u64,
    ) -> (RuntimeProfileId, Arc<Record>, watch::Sender<Terminal>) {
        let profile = RuntimeProfileId::parse("installed-audio-lifecycle-fixture").unwrap();
        let (send, terminal) = watch::channel(None);
        let record = Arc::new(Record {
            generation,
            model: "fixture/model".into(),
            profile_guard: Mutex::new(None),
            control: SessionControl::new(profile.clone(), generation),
            endpoint: Mutex::new(None),
            terminal,
        });
        owner
            .registry
            .lock()
            .unwrap()
            .sessions
            .insert(profile.clone(), record.clone());
        (profile, record, send)
    }

    #[tokio::test]
    async fn stale_generation_does_not_stop_successor() {
        let owner = AudioProfileOwner::default();
        let (profile, record, terminal) = admitted(&owner, 2);
        assert!(!owner
            .stop(&profile, Some(1))
            .await
            .unwrap()
            .unwrap()
            .unwrap());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), record.control.stopped())
                .await
                .is_err()
        );
        terminal.send_replace(Some(Ok(())));
        assert!(owner
            .stop(&profile, Some(2))
            .await
            .unwrap()
            .unwrap()
            .unwrap());
        record.control.stopped().await;
    }

    #[tokio::test]
    async fn cancelled_stop_waiter_does_not_reopen_admission_or_lose_join() {
        let owner = AudioProfileOwner::default();
        let (profile, record, terminal) = admitted(&owner, 4);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), owner.stop(&profile, Some(4)))
                .await
                .is_err()
        );
        record.control.stopped().await;
        assert!(record.terminal.borrow().is_none());
        terminal.send_replace(Some(Ok(())));
        assert!(owner
            .stop(&profile, Some(4))
            .await
            .unwrap()
            .unwrap()
            .unwrap());
    }

    #[tokio::test]
    async fn shutdown_closes_before_join_and_retries_wait_after_cancellation() {
        let owner = AudioProfileOwner::default();
        let (_, record, terminal) = admitted(&owner, 6);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), owner.close_and_drain())
                .await
                .is_err()
        );
        assert!(owner.registry.lock().unwrap().closed);
        record.control.stopped().await;
        terminal.send_replace(Some(Ok(())));
        let results = owner.close_and_drain().await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(*results[0].1.as_ref().unwrap());
    }

    #[tokio::test]
    async fn startup_waiter_and_owner_drop_request_exact_stop() {
        let owner = AudioProfileOwner::default();
        let (_, record, _terminal) = admitted(&owner, 8);
        drop(CancelOnDrop(Some(record.control.clone())));
        record.control.stopped().await;
        let (_, next, _next_terminal) = admitted(&owner, 9);
        drop(owner);
        next.control.stopped().await;
    }
    #[tokio::test]
    async fn lost_worker_is_observed_and_never_reported_as_success() {
        let owner = AudioProfileOwner::default();
        let (profile, record, terminal) = admitted(&owner, 10);
        drop(terminal);
        let result = tokio::time::timeout(Duration::from_secs(1), owner.stop(&profile, Some(10)))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(result.is_err());
        record.control.stopped().await;
    }
    #[tokio::test]
    async fn preparation_stop_retains_guard_until_join_and_releases_retained_waiters() {
        let root = tempfile::TempDir::new().unwrap();
        let service = super::super::RuntimeProfileService::with_provider_registry_and_adapters(
            root.path(),
            crate::providers::ProviderRegistry::builtin(),
            super::super::RuntimeProviderAdapters::builtin(),
        );
        let profile = RuntimeProfileId::parse("pending-audio-preparation").unwrap();
        let guard = service.begin_profile_operation(profile.clone()).unwrap();
        let library = Arc::new(
            ModelLibrary::new(root.path().join("library"))
                .await
                .unwrap(),
        );
        let serving = Arc::new(crate::serving::ServingService::with_provider_registry(
            crate::providers::ProviderRegistry::builtin(),
        ));
        let owner = Arc::new(AudioProfileOwner::default());
        let start_owner = owner.clone();
        let start_profile = profile.clone();
        let (entered, started) = oneshot::channel();
        let (release, finish) = oneshot::channel();
        let caller = tokio::spawn(async move {
            start_owner
                .launch(
                    library,
                    Arc::new(AudioEndpoints::default()),
                    start_profile,
                    "fixture/model".into(),
                    12,
                    guard,
                    serving,
                    move |guard| async move {
                        let _guard = guard;
                        entered.send(()).unwrap();
                        finish.await.unwrap();
                        Err(failure("controlled preparation refusal"))
                    },
                )
                .await
        });
        started.await.unwrap();
        assert_eq!(
            owner
                .generation_for_model(&profile, "fixture/model")
                .unwrap(),
            Some(12)
        );
        let retained_waiter = owner.registry.lock().unwrap().sessions[&profile].clone();
        let stopped = owner.stop(&profile, Some(12));
        tokio::pin!(stopped);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut stopped)
                .await
                .is_err()
        );
        assert!(service.begin_profile_operation(profile.clone()).is_err());
        caller.abort();
        release.send(()).unwrap();
        assert!(stopped.await.unwrap().unwrap().unwrap());
        assert!(retained_waiter.profile_guard.lock().unwrap().is_none());
        assert!(service.begin_profile_operation(profile.clone()).is_ok());
    }

    #[cfg(not(target_env = "uclibc"))]
    #[tokio::test]
    async fn lost_started_worker_joins_uncertain_original_child_before_error() {
        let owner = AudioProfileOwner::default();
        let (control, fault, weak) =
            super::super::audio_session::controlled_uncertain_session().await;
        let profile = RuntimeProfileId::parse("lost-started-fixture").unwrap();
        let (send, terminal) = watch::channel(None);
        let record = Arc::new(Record {
            generation: 11,
            model: "fixture/model".into(),
            profile_guard: Mutex::new(None),
            control,
            endpoint: Mutex::new(None),
            terminal,
        });
        owner
            .registry
            .lock()
            .unwrap()
            .sessions
            .insert(profile.clone(), record);
        drop(send);
        assert!(
            tokio::time::timeout(Duration::from_millis(150), owner.stop(&profile, Some(11)))
                .await
                .is_err()
        );
        assert!(weak.upgrade().is_some());
        fault.store(false, std::sync::atomic::Ordering::Release);
        let result = tokio::time::timeout(Duration::from_secs(10), owner.stop(&profile, Some(11)))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(result.is_err());
        assert!(weak.upgrade().is_none());
    }
}
