//! Runtime task ownership for primary API background work.

use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

#[derive(Clone, Default)]
pub(crate) struct RuntimeTasks {
    inner: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl RuntimeTasks {
    pub(crate) fn spawn<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let handle = tokio::spawn(task);
        let mut handles = self.inner.lock().expect("runtime task owner poisoned");
        handles.push(handle);
    }

    pub(crate) fn shutdown(&self) {
        let mut handles = self.inner.lock().expect("runtime task owner poisoned");
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

    struct AbortGuard(Option<oneshot::Sender<()>>);

    impl Drop for AbortGuard {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }
}
