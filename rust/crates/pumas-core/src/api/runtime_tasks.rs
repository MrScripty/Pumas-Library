//! Runtime task ownership for primary API background and finite effect work.

use crate::{PumasError, Result};
use futures::FutureExt;
use std::collections::BTreeMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex, Weak};
use tokio::runtime::Handle;
use tokio::sync::{oneshot, watch};
use tokio::task::AbortHandle;

#[derive(Clone)]
pub(crate) struct RuntimeTasks {
    handle: Handle,
    inner: Arc<Mutex<OwnerState>>,
    owner_refs: Arc<()>,
}

struct OwnerState {
    closed: bool,
    next_background_id: u64,
    next_operation_id: u64,
    background: BTreeMap<u64, AbortHandle>,
    operations: BTreeMap<u64, OperationEntry>,
    failures: Vec<String>,
    drain_started: bool,
    drain_tx: watch::Sender<Option<DrainOutcome>>,
    progress_version: u64,
    progress_tx: watch::Sender<u64>,
    tail_started: bool,
    tail_tx: watch::Sender<Option<DrainOutcome>>,
}

struct OperationEntry {
    operation: &'static str,
    accepting_nested: bool,
    outer_observed: bool,
    nested_active: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DrainOutcome {
    failures: Vec<String>,
}

impl DrainOutcome {
    fn public_result(&self) -> Result<()> {
        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(PumasError::Other(format!(
                "Runtime task shutdown observed {} owner failure(s): {}",
                self.failures.len(),
                self.failures.join("; ")
            )))
        }
    }
}

/// Identity carried by one admitted finite operation.
///
/// A context admits blocking effects only while its originating operation is
/// live. It deliberately holds a weak owner reference so settled entries do
/// not keep the registry alive.
#[derive(Clone)]
pub(crate) struct RuntimeTaskContext {
    handle: Handle,
    owner: Weak<Mutex<OwnerState>>,
    operation_id: u64,
}

impl RuntimeTasks {
    pub(crate) fn new() -> Self {
        let (drain_tx, _) = watch::channel(None);
        let (progress_tx, _) = watch::channel(0);
        let (tail_tx, _) = watch::channel(None);
        Self {
            handle: Handle::current(),
            inner: Arc::new(Mutex::new(OwnerState {
                closed: false,
                next_background_id: 1,
                next_operation_id: 1,
                background: BTreeMap::new(),
                operations: BTreeMap::new(),
                failures: Vec::new(),
                drain_started: false,
                drain_tx,
                progress_version: 0,
                progress_tx,
                tail_started: false,
                tail_tx,
            })),
            owner_refs: Arc::new(()),
        }
    }

    /// Register best-effort background work.
    pub(crate) fn spawn<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let mut state = self.inner.lock().expect("runtime task owner poisoned");
        if state.closed {
            return;
        }
        let background_id = state.next_background_id;
        state.next_background_id = state
            .next_background_id
            .checked_add(1)
            .expect("runtime background identity exhausted");
        let (start_tx, start_rx) = oneshot::channel();
        let handle = self.handle.spawn(async move {
            if start_rx.await.is_ok() {
                AssertUnwindSafe(task).catch_unwind().await
            } else {
                Ok(())
            }
        });
        state
            .background
            .insert(background_id, handle.abort_handle());
        let inner = Arc::clone(&self.inner);
        self.handle.spawn(async move {
            let result = handle.await;
            let mut state = inner.lock().expect("runtime task owner poisoned");
            state.background.remove(&background_id);
            match result {
                Ok(Err(payload)) => state.failures.push(format!(
                    "background runtime task panicked: {}",
                    panic_message(payload)
                )),
                Err(error) if !error.is_cancelled() => state
                    .failures
                    .push(format!("background runtime task join failed: {error}")),
                _ => {}
            }
            maybe_finish_drain(&mut state);
        });
        let _ = start_tx.send(());
    }

    /// Synchronously admit finite work and return its independently owned result.
    pub(crate) fn start_owned<T, F, Fut>(
        &self,
        operation: &'static str,
        function: F,
    ) -> Result<oneshot::Receiver<Result<T>>>
    where
        T: Send + 'static,
        F: FnOnce(RuntimeTaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let mut state = self.inner.lock().expect("runtime task owner poisoned");
        if state.closed {
            return Err(owner_closed());
        }
        let operation_id = state.next_operation_id;
        state.next_operation_id = state
            .next_operation_id
            .checked_add(1)
            .ok_or_else(|| owner_failure("runtime operation identity exhausted"))?;

        let context = RuntimeTaskContext {
            handle: self.handle.clone(),
            owner: Arc::downgrade(&self.inner),
            operation_id,
        };
        let inner = Arc::clone(&self.inner);
        let (start_tx, start_rx) = oneshot::channel();
        let (result_tx, result_rx) = oneshot::channel();
        let outer = self.handle.spawn(async move {
            if start_rx.await.is_err() {
                return;
            }
            let outcome = AssertUnwindSafe(async move { function(context).await })
                .catch_unwind()
                .await;
            let result = match outcome {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) => {
                    archive_failure(
                        &inner,
                        format!("owned operation `{operation}` failed: {error}"),
                    );
                    Err(error)
                }
                Err(payload) => {
                    let message = format!(
                        "owned operation `{operation}` panicked: {}",
                        panic_message(payload)
                    );
                    archive_failure(&inner, message.clone());
                    Err(owner_failure(&message))
                }
            };
            if let Ok(mut state) = inner.lock() {
                if let Some(entry) = state.operations.get_mut(&operation_id) {
                    entry.accepting_nested = false;
                }
            } else {
                archive_failure(&inner, format!("owned operation `{operation}` poisoned owner"));
            }
            if let Err(Err(error)) = result_tx.send(result) {
                tracing::error!(operation, error = %error, "unclaimed owned runtime operation error");
            }
        });
        state.operations.insert(
            operation_id,
            OperationEntry {
                operation,
                accepting_nested: true,
                outer_observed: false,
                nested_active: 0,
            },
        );
        let inner = Arc::clone(&self.inner);
        self.handle.spawn(async move {
            let result = outer.await;
            let mut state = inner.lock().expect("runtime task owner poisoned");
            if let Err(error) = result {
                state.failures.push(format!(
                    "owned operation `{operation}` join failed: {error}"
                ));
            }
            if let Some(entry) = state.operations.get_mut(&operation_id) {
                entry.accepting_nested = false;
                entry.outer_observed = true;
            }
            settle_operation_if_ready(&mut state, operation_id);
            maybe_finish_drain(&mut state);
        });
        let _ = start_tx.send(());
        Ok(result_rx)
    }

    pub(crate) async fn run_owned<T, F, Fut>(
        &self,
        operation: &'static str,
        function: F,
    ) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(RuntimeTaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.start_owned(operation, function)?
            .await
            .map_err(|_| owner_failure("owned runtime operation result channel closed"))?
    }

    // The direct close gate is staged for callers that must split admission
    // closure from draining; ordered shutdown currently closes internally.
    #[allow(dead_code)]
    pub(crate) fn close(&self) {
        self.inner
            .lock()
            .expect("runtime task owner poisoned")
            .closed = true;
    }

    pub(crate) async fn shutdown_owned(&self) -> Result<()> {
        let mut receiver = {
            let mut state = self.inner.lock().expect("runtime task owner poisoned");
            state.closed = true;
            for handle in state.background.values() {
                handle.abort();
            }
            let receiver = state.drain_tx.subscribe();
            if !state.drain_started {
                state.drain_started = true;
                maybe_finish_drain(&mut state);
            }
            receiver
        };

        loop {
            if let Some(outcome) = receiver.borrow().clone() {
                return outcome.public_result();
            }
            receiver
                .changed()
                .await
                .map_err(|_| owner_failure("runtime task drain channel closed"))?;
        }
    }

    /// Drain finite local work, then run one shared ordered shutdown tail.
    pub(crate) async fn shutdown_owned_then<F, Fut>(&self, tail: F) -> Result<()>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let mut tail = Some(tail);
        let mut receiver = {
            let mut state = self.inner.lock().expect("runtime task owner poisoned");
            state.closed = true;
            for handle in state.background.values() {
                handle.abort();
            }
            let receiver = state.tail_tx.subscribe();
            if !state.tail_started {
                state.tail_started = true;
                let function = tail.take().expect("first shutdown tail must be present");
                let owner = self.clone();
                let inner = Arc::clone(&self.inner);
                let coordinator = self.handle.spawn(async move {
                    let local = owner.shutdown_owned().await;
                    let tail = AssertUnwindSafe(async move { function().await })
                        .catch_unwind()
                        .await;
                    let mut failures = Vec::new();
                    if let Err(error) = local {
                        failures.push(format!("local runtime drain failed: {error}"));
                    }
                    match tail {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            failures.push(format!("ordered shutdown tail failed: {error}"));
                        }
                        Err(payload) => failures.push(format!(
                            "ordered shutdown tail panicked: {}",
                            panic_message(payload)
                        )),
                    }
                    publish_tail_outcome(&inner, DrainOutcome { failures });
                });
                let inner = Arc::clone(&self.inner);
                self.handle.spawn(async move {
                    if let Err(error) = coordinator.await {
                        publish_tail_outcome(
                            &inner,
                            DrainOutcome {
                                failures: vec![format!(
                                    "ordered shutdown coordinator join failed: {error}"
                                )],
                            },
                        );
                    }
                });
            }
            receiver
        };
        drop(tail);

        loop {
            if let Some(outcome) = receiver.borrow().clone() {
                return outcome.public_result();
            }
            receiver
                .changed()
                .await
                .map_err(|_| owner_failure("ordered shutdown tail channel closed"))?;
        }
    }

    /// Close admission and abort background work without aborting finite work.
    pub(crate) fn shutdown(&self) {
        let mut state = self.inner.lock().expect("runtime task owner poisoned");
        state.closed = true;
        for handle in state.background.values() {
            handle.abort();
        }
    }

    #[cfg(test)]
    fn tracked_count(&self) -> usize {
        self.inner
            .lock()
            .expect("runtime task owner poisoned")
            .background
            .len()
    }
}

impl RuntimeTaskContext {
    /// Stop further nested admission and wait for registered blocking effects.
    pub(crate) async fn drain_nested(&self) -> Result<()> {
        let inner = self.owner.upgrade().ok_or_else(stale_context)?;
        let mut progress = {
            let mut state = inner.lock().expect("runtime task owner poisoned");
            let entry = state
                .operations
                .get_mut(&self.operation_id)
                .ok_or_else(stale_context)?;
            entry.accepting_nested = false;
            state.progress_tx.subscribe()
        };
        loop {
            let complete = inner
                .lock()
                .expect("runtime task owner poisoned")
                .operations
                .get(&self.operation_id)
                .is_none_or(|entry| entry.nested_active == 0);
            if complete {
                return Ok(());
            }
            progress
                .changed()
                .await
                .map_err(|_| owner_failure("runtime task progress channel closed"))?;
        }
    }

    pub(crate) async fn run_blocking<T, F>(&self, operation: &'static str, function: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let result_rx = {
            let inner = self.owner.upgrade().ok_or_else(stale_context)?;
            let mut state = inner.lock().expect("runtime task owner poisoned");
            let entry = state
                .operations
                .get_mut(&self.operation_id)
                .filter(|entry| entry.accepting_nested)
                .ok_or_else(stale_context)?;
            let owner_operation = entry.operation;
            entry.nested_active = entry
                .nested_active
                .checked_add(1)
                .ok_or_else(|| owner_failure("runtime nested effect count exhausted"))?;
            let owner = Arc::clone(&inner);
            let (start_tx, start_rx) = oneshot::channel();
            let (result_tx, result_rx) = oneshot::channel();
            let blocking_handle = self.handle.clone();
            let nested = self.handle.spawn(async move {
                if start_rx.await.is_err() {
                    return;
                }
                let result = match blocking_handle.spawn_blocking(function).await {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        let message = format!(
                            "owned operation `{owner_operation}` blocking effect `{operation}` failed: {error}"
                        );
                        archive_failure(&owner, message.clone());
                        Err(owner_failure(&message))
                    }
                };
                let _ = result_tx.send(result);
            });
            let operation_id = self.operation_id;
            let observer_owner = Arc::clone(&inner);
            self.handle.spawn(async move {
                let result = nested.await;
                let mut state = observer_owner.lock().expect("runtime task owner poisoned");
                if let Err(error) = result {
                    state.failures.push(format!(
                        "blocking effect `{operation}` join failed: {error}"
                    ));
                }
                if let Some(entry) = state.operations.get_mut(&operation_id) {
                    entry.nested_active = entry.nested_active.saturating_sub(1);
                }
                state.progress_version = state.progress_version.wrapping_add(1);
                let progress_version = state.progress_version;
                state.progress_tx.send_replace(progress_version);
                settle_operation_if_ready(&mut state, operation_id);
                maybe_finish_drain(&mut state);
            });
            let _ = start_tx.send(());
            result_rx
        };
        result_rx
            .await
            .map_err(|_| owner_failure("owned blocking effect result channel closed"))?
    }
}

impl Default for RuntimeTasks {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RuntimeTasks {
    fn drop(&mut self) {
        if Arc::strong_count(&self.owner_refs) == 1 {
            self.shutdown();
        }
    }
}

fn settle_operation_if_ready(state: &mut OwnerState, operation_id: u64) {
    let ready = state
        .operations
        .get(&operation_id)
        .is_some_and(|entry| entry.outer_observed && entry.nested_active == 0);
    if ready {
        state.operations.remove(&operation_id);
    }
}

fn maybe_finish_drain(state: &mut OwnerState) {
    if state.drain_started
        && state.background.is_empty()
        && state.operations.is_empty()
        && state.drain_tx.borrow().is_none()
    {
        let outcome = DrainOutcome {
            failures: state.failures.clone(),
        };
        state.drain_tx.send_replace(Some(outcome));
    }
}

fn publish_tail_outcome(inner: &Arc<Mutex<OwnerState>>, outcome: DrainOutcome) {
    let state = inner.lock().expect("runtime task owner poisoned");
    if state.tail_tx.borrow().is_none() {
        state.tail_tx.send_replace(Some(outcome));
    }
}

fn archive_failure(inner: &Arc<Mutex<OwnerState>>, message: String) {
    tracing::error!(error = %message, "owned runtime task failed");
    match inner.lock() {
        Ok(mut state) => state.failures.push(message),
        Err(poisoned) => poisoned.into_inner().failures.push(message),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_string())
}

fn owner_closed() -> PumasError {
    owner_failure("runtime task owner is closed")
}

fn stale_context() -> PumasError {
    owner_failure("runtime task context is stale")
}

fn owner_failure(message: &str) -> PumasError {
    PumasError::Other(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::RuntimeTasks;
    use crate::PumasError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Barrier};
    use tokio::sync::{oneshot, Notify};
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn shutdown_aborts_tracked_background_tasks() {
        let tasks = RuntimeTasks::default();
        let (started_tx, started_rx) = oneshot::channel();
        let (aborted_tx, aborted_rx) = oneshot::channel();
        tasks.spawn(async move {
            let _guard = DropSignal(Some(aborted_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        started_rx.await.unwrap();
        assert_eq!(tasks.tracked_count(), 1);
        tasks.shutdown();
        aborted_rx.await.unwrap();
    }

    #[tokio::test]
    async fn close_rejects_roots_without_polling_their_effects() {
        let tasks = RuntimeTasks::default();
        tasks.close();
        let effects = Arc::new(AtomicUsize::new(0));
        let background_effect = Arc::clone(&effects);
        tasks.spawn(async move {
            background_effect.fetch_add(1, Ordering::SeqCst);
        });
        let owned_effect = Arc::clone(&effects);
        assert!(tasks
            .run_owned("closed", move |_| async move {
                owned_effect.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .is_err());
        assert_eq!(effects.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn registration_racing_close_is_rejected_or_fully_admitted() {
        let tasks = RuntimeTasks::default();
        let barrier = Arc::new(Barrier::new(3));
        let effects = Arc::new(AtomicUsize::new(0));
        let closing = std::thread::spawn({
            let tasks = tasks.clone();
            let barrier = Arc::clone(&barrier);
            move || {
                barrier.wait();
                tasks.close();
            }
        });
        let registering = std::thread::spawn({
            let tasks = tasks.clone();
            let barrier = Arc::clone(&barrier);
            let effects = Arc::clone(&effects);
            move || {
                barrier.wait();
                tasks.start_owned("racing", move |_| async move {
                    effects.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
            }
        });
        barrier.wait();
        closing.join().unwrap();
        let registration = registering.join().unwrap();
        match registration {
            Ok(receiver) => receiver.await.unwrap().unwrap(),
            Err(_) => assert_eq!(effects.load(Ordering::SeqCst), 0),
        }
        tasks.shutdown_owned().await.unwrap();
        assert!(effects.load(Ordering::SeqCst) <= 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropped_requester_does_not_cancel_blocking_effect_and_drain_waits() {
        let tasks = RuntimeTasks::default();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let commits = Arc::new(AtomicUsize::new(0));
        let committed = Arc::clone(&commits);
        let receiver = tasks
            .start_owned("ensure", move |context| async move {
                context
                    .run_blocking("commit", move || {
                        let _ = started_tx.send(());
                        release_rx.recv().unwrap();
                        committed.fetch_add(1, Ordering::SeqCst);
                    })
                    .await?;
                Ok(())
            })
            .unwrap();
        started_rx.await.unwrap();
        drop(receiver);
        let mut drain = tokio::spawn({
            let tasks = tasks.clone();
            async move { tasks.shutdown_owned().await }
        });
        assert!(timeout(Duration::from_millis(25), &mut drain)
            .await
            .is_err());
        release_tx.send(()).unwrap();
        drain.await.unwrap().unwrap();
        assert_eq!(commits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn outer_panic_retains_nested_join_until_drain() {
        let tasks = RuntimeTasks::default();
        let entered = Arc::new(Notify::new());
        let (release_tx, release_rx) = mpsc::channel();
        let (panicked_tx, panicked_rx) = oneshot::channel();
        let receiver = tasks
            .start_owned("panicking", {
                let entered = Arc::clone(&entered);
                move |context| async move {
                    let nested_context = context.clone();
                    let nested_entered = Arc::clone(&entered);
                    tokio::spawn(async move {
                        let _ = nested_context
                            .run_blocking("held", move || {
                                nested_entered.notify_one();
                                release_rx.recv().unwrap();
                            })
                            .await;
                    });
                    entered.notified().await;
                    let _ = panicked_tx.send(());
                    panic!("outer failed");
                    #[allow(unreachable_code)]
                    Ok::<(), PumasError>(())
                }
            })
            .unwrap();
        panicked_rx.await.unwrap();
        drop(receiver);
        let mut drain = tokio::spawn({
            let tasks = tasks.clone();
            async move { tasks.shutdown_owned().await }
        });
        assert!(timeout(Duration::from_millis(25), &mut drain)
            .await
            .is_err());
        release_tx.send(()).unwrap();
        let error = drain.await.unwrap().unwrap_err().to_string();
        assert!(error.contains("outer failed"));
        assert_eq!(tasks.shutdown_owned().await.unwrap_err().to_string(), error);
    }

    #[tokio::test]
    async fn cancelled_shutdown_waiter_does_not_cancel_shared_drain() {
        let tasks = RuntimeTasks::default();
        let release = Arc::new(Notify::new());
        drop(
            tasks
                .start_owned("held", {
                    let release = Arc::clone(&release);
                    move |_| async move {
                        release.notified().await;
                        Ok(())
                    }
                })
                .unwrap(),
        );
        let abandoned = tokio::spawn({
            let tasks = tasks.clone();
            async move { tasks.shutdown_owned().await }
        });
        tokio::task::yield_now().await;
        abandoned.abort();
        let waiting = tokio::spawn({
            let tasks = tasks.clone();
            async move { tasks.shutdown_owned().await }
        });
        release.notify_one();
        waiting.await.unwrap().unwrap();
        tasks.shutdown_owned().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_ordered_shutdown_still_runs_tail_once_after_local_drain() {
        let tasks = RuntimeTasks::default();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let settled = Arc::new(AtomicUsize::new(0));
        let settled_effect = Arc::clone(&settled);
        drop(
            tasks
                .start_owned("held", move |context| async move {
                    context
                        .run_blocking("effect", move || {
                            let _ = started_tx.send(());
                            release_rx.recv().unwrap();
                            settled_effect.store(1, Ordering::SeqCst);
                        })
                        .await?;
                    Ok(())
                })
                .unwrap(),
        );
        started_rx.await.unwrap();
        let tails = Arc::new(AtomicUsize::new(0));
        let abandoned = tokio::spawn({
            let tasks = tasks.clone();
            let tails = Arc::clone(&tails);
            let settled = Arc::clone(&settled);
            async move {
                tasks
                    .shutdown_owned_then(move || async move {
                        assert_eq!(settled.load(Ordering::SeqCst), 1);
                        tails.fetch_add(1, Ordering::SeqCst);
                        Err(PumasError::Other("tail failed".to_string()))
                    })
                    .await
            }
        });
        loop {
            let started = tasks
                .inner
                .lock()
                .expect("runtime task owner poisoned")
                .tail_started;
            if started {
                break;
            }
            tokio::task::yield_now().await;
        }
        abandoned.abort();
        release_tx.send(()).unwrap();
        let unused_tails = Arc::clone(&tails);
        let first = tasks
            .shutdown_owned_then(move || async move {
                unused_tails.fetch_add(100, Ordering::SeqCst);
                Ok(())
            })
            .await
            .unwrap_err()
            .to_string();
        let second = tasks
            .shutdown_owned_then(|| async { Ok(()) })
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(first, second);
        assert!(first.contains("tail failed"));
        assert_eq!(tails.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn context_nests_after_root_close_then_drain_makes_it_stale() {
        let tasks = RuntimeTasks::default();
        let (context_tx, context_rx) = oneshot::channel();
        let proceed = Arc::new(Notify::new());
        let receiver = tasks
            .start_owned("root", {
                let proceed = Arc::clone(&proceed);
                move |context| async move {
                    assert!(context_tx.send(context.clone()).is_ok());
                    proceed.notified().await;
                    let value = context.run_blocking("nested", || 7usize).await?;
                    context.drain_nested().await?;
                    Ok(value)
                }
            })
            .unwrap();
        let stale = context_rx.await.unwrap();
        tasks.close();
        proceed.notify_one();
        assert_eq!(receiver.await.unwrap().unwrap(), 7);
        assert!(stale.run_blocking("late", || ()).await.is_err());
        tasks.shutdown_owned().await.unwrap();
    }

    #[tokio::test]
    async fn completed_background_panic_is_observed_by_shared_drain() {
        let tasks = RuntimeTasks::default();
        let (about_to_panic_tx, about_to_panic_rx) = oneshot::channel();
        tasks.spawn(async move {
            let _ = about_to_panic_tx.send(());
            panic!("background failed");
        });
        about_to_panic_rx.await.unwrap();
        tokio::task::yield_now().await;
        let error = tasks.shutdown_owned().await.unwrap_err().to_string();
        assert!(error.contains("background failed"));
        assert_eq!(tasks.shutdown_owned().await.unwrap_err().to_string(), error);
    }

    #[tokio::test]
    async fn owner_errors_are_cached_while_nested_domain_results_are_transport() {
        let tasks = RuntimeTasks::default();
        let domain = tasks
            .run_owned("domain", |context| async move {
                let refusal = context
                    .run_blocking("domain-refusal", || {
                        Err::<(), _>("retained by another consumer")
                    })
                    .await?;
                Ok(refusal)
            })
            .await
            .unwrap();
        assert_eq!(domain, Err("retained by another consumer"));
        let error = tasks
            .run_owned("io", |_| async move {
                Err::<(), _>(PumasError::Other("disk failed".to_string()))
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("disk failed"));
        let drain = tasks.shutdown_owned().await.unwrap_err().to_string();
        assert!(drain.contains("owned operation `io` failed"));
        assert!(!drain.contains("retained by another consumer"));
    }

    #[tokio::test]
    async fn spawn_uses_captured_runtime_from_non_runtime_thread() {
        let tasks = RuntimeTasks::default();
        let thread_tasks = tasks.clone();
        let (tx, rx) = oneshot::channel();
        std::thread::spawn(move || {
            thread_tasks.spawn(async move {
                let _ = tx.send(());
            })
        })
        .join()
        .unwrap();
        timeout(Duration::from_secs(1), rx).await.unwrap().unwrap();
    }

    struct DropSignal(Option<oneshot::Sender<()>>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }
}
