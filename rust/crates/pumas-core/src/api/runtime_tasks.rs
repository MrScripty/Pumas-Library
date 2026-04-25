//! Runtime task ownership for primary API background work.

use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub(crate) struct RuntimeTasks {
    handle: Handle,
    inner: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl RuntimeTasks {
    pub(crate) fn new() -> Self {
        Self {
            handle: Handle::current(),
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn prune_finished(handles: &mut Vec<JoinHandle<()>>) {
        handles.retain(|handle| !handle.is_finished());
    }

    pub(crate) fn spawn<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let handle = self.handle.spawn(task);
        let mut handles = self.inner.lock().expect("runtime task owner poisoned");
        Self::prune_finished(&mut handles);
        handles.push(handle);
    }

    pub(crate) fn shutdown(&self) {
        let mut handles = self.inner.lock().expect("runtime task owner poisoned");
        Self::prune_finished(&mut handles);
        for handle in handles.drain(..) {
            handle.abort();
        }
    }

    #[cfg(test)]
    fn tracked_count(&self) -> usize {
        self.inner
            .lock()
            .expect("runtime task owner poisoned")
            .len()
    }
}

impl Default for RuntimeTasks {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RuntimeTasks {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeTasks;
    use tokio::sync::oneshot;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn shutdown_aborts_tracked_tasks() {
        let tasks = RuntimeTasks::default();
        let (started_tx, started_rx) = oneshot::channel();
        let (aborted_tx, aborted_rx) = oneshot::channel();

        tasks.spawn(async move {
            let _guard = AbortGuard(Some(aborted_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });

        started_rx.await.expect("tracked task should start");
        assert_eq!(tasks.tracked_count(), 1);
        tasks.shutdown();

        aborted_rx.await.expect("tracked task should be aborted");
        assert_eq!(tasks.tracked_count(), 0);
    }

    #[tokio::test]
    async fn spawn_prunes_finished_handles_before_tracking_new_tasks() {
        let tasks = RuntimeTasks::default();
        let (completed_tx, completed_rx) = oneshot::channel();
        let (started_tx, started_rx) = oneshot::channel();
        let (aborted_tx, aborted_rx) = oneshot::channel();

        tasks.spawn(async move {
            let _ = completed_tx.send(());
        });
        completed_rx.await.expect("short task should complete");

        tasks.spawn(async move {
            let _guard = AbortGuard(Some(aborted_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });

        started_rx.await.expect("tracked task should start");
        assert_eq!(tasks.tracked_count(), 1);

        tasks.shutdown();
        aborted_rx.await.expect("tracked task should be aborted");
        assert_eq!(tasks.tracked_count(), 0);
    }

    #[tokio::test]
    async fn spawn_uses_captured_runtime_handle_from_non_runtime_thread() {
        let tasks = RuntimeTasks::default();
        let tasks_for_thread = tasks.clone();
        let (tx, rx) = oneshot::channel();
        let join = std::thread::spawn(move || {
            tasks_for_thread.spawn(async move {
                let _ = tx.send(());
            });
        });

        join.join()
            .expect("non-runtime thread should enqueue task successfully");
        timeout(Duration::from_secs(1), rx)
            .await
            .expect("spawned task should run on captured runtime")
            .expect("spawned task should signal completion");
    }

    struct AbortGuard(Option<oneshot::Sender<()>>);

    impl Drop for AbortGuard {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }
}
