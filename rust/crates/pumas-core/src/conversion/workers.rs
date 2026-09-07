//! Conversion Rust-worker custody. This owns join observation, not native
//! process-tree cleanup. The embedding owner must await shutdown before runtime
//! shutdown; cancellation requests alone do not establish terminal state.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use tokio::task::{JoinError, JoinHandle};

use super::progress::ConversionProgressTracker;
use super::{ConversionProgress, ConversionStatus};
use crate::cancel::CancellationToken;
use crate::{PumasError, Result};

struct Receipt {
    handle: Option<JoinHandle<Result<()>>>,
    outcome: Option<std::result::Result<(), String>>,
}

struct Worker {
    id: String,
    cancel: CancellationToken,
    receipt: tokio::sync::Mutex<Receipt>,
}

impl Worker {
    fn record(
        &self,
        receipt: &mut Receipt,
        result: std::result::Result<Result<()>, JoinError>,
        progress: &ConversionProgressTracker,
    ) {
        let outcome = match result {
            Ok(Ok(())) => {
                progress.set_status(&self.id, ConversionStatus::Completed);
                Ok(())
            }
            Ok(Err(PumasError::ConversionCancelled)) => {
                progress.set_status(&self.id, ConversionStatus::Cancelled);
                Ok(())
            }
            Ok(Err(error)) => Err(error.to_string()),
            Err(error) => Err(if error.is_panic() {
                "Conversion worker panicked".into()
            } else {
                "Conversion worker was cancelled before its result was observed".into()
            }),
        };
        if let Err(message) = &outcome {
            progress.set_error(&self.id, message.clone());
        }
        receipt.outcome = Some(outcome);
        receipt.handle = None;
    }

    fn observe_finished(
        &self,
        progress: &ConversionProgressTracker,
    ) -> Option<std::result::Result<(), String>> {
        let mut receipt = self.receipt.try_lock().ok()?;
        if let Some(handle) = receipt.handle.as_mut() {
            if !handle.is_finished() {
                return None;
            }
            let result = handle.now_or_never()?;
            self.record(&mut receipt, result, progress);
        }
        receipt.outcome.clone()
    }

    async fn observe(
        &self,
        progress: &ConversionProgressTracker,
    ) -> std::result::Result<(), String> {
        let mut receipt = self.receipt.lock().await;
        if let Some(handle) = receipt.handle.as_mut() {
            // Cancellation of this waiter leaves the handle inside its owner.
            let result = handle.await;
            self.record(&mut receipt, result, progress);
        }
        receipt
            .outcome
            .clone()
            .expect("observed conversion receipt")
    }
}

#[derive(Default)]
struct State {
    closed: bool,
    workers: HashMap<String, Arc<Worker>>,
    failure: Option<String>,
}

pub(super) struct WorkerOwner {
    state: Mutex<State>,
    progress: Arc<ConversionProgressTracker>,
    capacity: usize,
}

impl WorkerOwner {
    pub(super) fn new(progress: Arc<ConversionProgressTracker>, capacity: usize) -> Self {
        Self {
            state: Mutex::new(State::default()),
            progress,
            capacity,
        }
    }

    pub(super) fn spawn<F>(
        &self,
        initial: ConversionProgress,
        cancel: CancellationToken,
        work: F,
    ) -> Result<()>
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.observe_finished();
        let mut state = self.state.lock().expect("conversion worker owner poisoned");
        if state.closed {
            return Err(PumasError::ConversionCancelled);
        }
        if state.workers.len() >= self.capacity
            || state.workers.contains_key(&initial.conversion_id)
        {
            return Err(PumasError::ConversionFailed {
                message: "Maximum concurrent conversions reached; wait for current work to finish"
                    .into(),
            });
        }
        let id = initial.conversion_id.clone();
        self.progress.insert(initial);
        let handle = tokio::spawn(work);
        state.workers.insert(
            id.clone(),
            Arc::new(Worker {
                id,
                cancel,
                receipt: tokio::sync::Mutex::new(Receipt {
                    handle: Some(handle),
                    outcome: None,
                }),
            }),
        );
        Ok(())
    }

    pub(super) fn observe_finished(&self) {
        let workers: Vec<_> = self
            .state
            .lock()
            .expect("conversion worker owner poisoned")
            .workers
            .values()
            .cloned()
            .collect();
        for worker in workers {
            if let Some(result) = worker.observe_finished(&self.progress) {
                self.reclaim(&worker, result);
            }
        }
    }

    fn reclaim(&self, worker: &Arc<Worker>, result: std::result::Result<(), String>) {
        let mut state = self.state.lock().expect("conversion worker owner poisoned");
        if let Err(message) = result {
            state.failure.get_or_insert(message);
        }
        if state
            .workers
            .get(&worker.id)
            .is_some_and(|current| Arc::ptr_eq(current, worker))
        {
            state.workers.remove(&worker.id);
        }
    }

    pub(super) fn cancel(&self, id: &str) -> bool {
        self.observe_finished();
        let state = self.state.lock().expect("conversion worker owner poisoned");
        let Some(worker) = state.workers.get(id) else {
            return false;
        };
        if let Ok(receipt) = worker.receipt.try_lock() {
            if receipt.handle.as_ref().is_none_or(JoinHandle::is_finished) {
                return false;
            }
        }
        // A shutdown observer can hold the receipt while this worker is still
        // active. Lock contention is not evidence that cancellation is invalid.
        worker.cancel.cancel();
        true
    }

    pub(super) async fn shutdown(&self) -> Result<()> {
        let workers: Vec<_> = {
            let mut state = self.state.lock().expect("conversion worker owner poisoned");
            state.closed = true;
            for worker in state.workers.values() {
                worker.cancel.cancel();
            }
            state.workers.values().cloned().collect()
        };
        for worker in workers {
            let result = worker.observe(&self.progress).await;
            self.reclaim(&worker, result);
        }
        match self
            .state
            .lock()
            .expect("conversion worker owner poisoned")
            .failure
            .as_ref()
        {
            Some(message) => Err(PumasError::ConversionFailed {
                message: message.clone(),
            }),
            None => Ok(()),
        }
    }
}

impl Drop for WorkerOwner {
    fn drop(&mut self) {
        // This signals intent only. Explicit shutdown owns the drain receipt.
        for worker in self
            .state
            .get_mut()
            .expect("conversion worker owner poisoned")
            .workers
            .values()
        {
            worker.cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Poll;
    use std::time::Duration;
    use tokio::sync::oneshot;

    fn initial(id: &str) -> ConversionProgress {
        serde_json::from_value(serde_json::json!({
            "conversionId": id, "sourceModelId": "fixture/model", "direction": "gguf_to_safetensors",
            "status": "converting"
        })).expect("fixture progress")
    }

    fn owner() -> WorkerOwner {
        WorkerOwner::new(Arc::new(ConversionProgressTracker::new()), 1)
    }

    async fn reclaim(owner: &WorkerOwner) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                owner.observe_finished();
                if owner.state.lock().expect("state").workers.is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker result observed");
    }

    #[tokio::test]
    async fn cancellation_waits_for_worker_result_and_interrupted_shutdown_retains_custody() {
        let owner = owner();
        let cancel = CancellationToken::new();
        let worker_cancel = cancel.clone();
        let (release, held) = oneshot::channel();
        owner
            .spawn(initial("held"), cancel.clone(), async move {
                held.await.expect("fixture release");
                assert!(worker_cancel.is_cancelled());
                Err(PumasError::ConversionCancelled)
            })
            .expect("admit held worker");
        assert!(owner.cancel("held"));
        assert!(cancel.is_cancelled());
        assert_eq!(
            owner.progress.get("held").expect("progress").status,
            ConversionStatus::Converting
        );
        let mut interrupted = Box::pin(owner.shutdown());
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(interrupted.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        assert!(
            owner.cancel("held"),
            "a pending drain still owns active work"
        );
        drop(interrupted);
        let worker = owner
            .state
            .lock()
            .expect("state")
            .workers
            .get("held")
            .cloned()
            .expect("retained worker");
        assert!(worker
            .receipt
            .try_lock()
            .expect("interrupted waiter releases receipt lock")
            .handle
            .is_some());
        assert!(owner
            .spawn(initial("closed"), CancellationToken::new(), async {
                panic!("a rejected worker must never be polled");
            })
            .is_err());
        assert!(
            owner.progress.get("closed").is_none(),
            "no ghost progress after shutdown"
        );
        let first = owner.shutdown();
        let second = owner.shutdown();
        release.send(()).expect("worker still owned");
        let (first, second) = tokio::join!(first, second);
        first.expect("first drain");
        second.expect("second drain");
        assert_eq!(
            owner
                .progress
                .get("held")
                .expect("terminal progress")
                .status,
            ConversionStatus::Cancelled
        );
        assert!(!owner.cancel("held"));
        assert!(!owner.cancel("unknown"));
        owner.shutdown().await.expect("idempotent drain");
    }

    #[tokio::test]
    async fn progress_terminal_is_not_worker_completion_and_success_wins_late_cancel() {
        let owner = owner();
        let (release, held) = oneshot::channel();
        owner
            .spawn(initial("success"), CancellationToken::new(), async move {
                held.await.expect("fixture release");
                Ok(())
            })
            .expect("admit worker");
        owner
            .progress
            .set_status("success", ConversionStatus::Completed);
        assert!(
            owner.cancel("success"),
            "raw script status is not a joined worker"
        );
        assert!(owner
            .spawn(initial("excess"), CancellationToken::new(), async {
                Ok(())
            })
            .is_err());
        assert!(owner.progress.get("excess").is_none());
        release.send(()).expect("release success");
        reclaim(&owner).await;
        assert_eq!(
            owner.progress.get("success").expect("progress").status,
            ConversionStatus::Completed
        );
        assert!(!owner.cancel("success"));
        owner
            .shutdown()
            .await
            .expect("successful worker despite cancellation intent");
    }

    #[tokio::test]
    async fn observed_panics_and_operation_errors_survive_pruning_and_repeated_shutdown() {
        let owner = owner();
        owner
            .spawn(initial("panic"), CancellationToken::new(), async {
                panic!("controlled worker panic");
            })
            .expect("admit panic worker");
        reclaim(&owner).await;
        assert_eq!(
            owner
                .progress
                .get("panic")
                .expect("panic receipt")
                .error
                .as_deref(),
            Some("Conversion worker panicked")
        );
        owner
            .spawn(initial("error"), CancellationToken::new(), async {
                Err(PumasError::ConversionFailed {
                    message: "controlled operation failure".into(),
                })
            })
            .expect("admit later worker");
        reclaim(&owner).await;
        assert_eq!(
            owner.progress.get("error").expect("failure receipt").status,
            ConversionStatus::Error
        );
        for _ in 0..2 {
            assert!(
                matches!(owner.shutdown().await, Err(PumasError::ConversionFailed { message }) if message == "Conversion worker panicked")
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_registration_enforces_capacity_without_rejected_progress() {
        let owner = Arc::new(owner());
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let mut attempts = Vec::new();
        let mut releases = Vec::new();
        for id in ["first", "second"] {
            let owner = Arc::clone(&owner);
            let barrier = Arc::clone(&barrier);
            let (release, held) = oneshot::channel();
            releases.push(release);
            attempts.push(tokio::spawn(async move {
                barrier.wait().await;
                owner.spawn(initial(id), CancellationToken::new(), async move {
                    held.await.expect("accepted worker release");
                    Ok(())
                })
            }));
        }
        let mut accepted = 0;
        for attempt in attempts {
            if attempt.await.expect("registration task").is_ok() {
                accepted += 1;
            }
        }
        assert_eq!(accepted, 1);
        assert_eq!(owner.progress.list_all().len(), 1);
        let released = releases
            .into_iter()
            .filter_map(|release| release.send(()).ok())
            .count();
        assert_eq!(released, 1);
        owner.shutdown().await.expect("drain admitted worker");
    }
}
