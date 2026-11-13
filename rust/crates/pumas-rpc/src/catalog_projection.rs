//! Bounded, server-owned filesystem projection for catalog responses.

use crate::contract::{CatalogSearchOutcome, ModelsOutcome};
use pumas_library::{index::SearchResult, ModelRecord, PumasError, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

enum Job {
    Models(
        Vec<ModelRecord>,
        PathBuf,
        oneshot::Sender<Result<ModelsOutcome>>,
    ),
    Search(
        SearchResult,
        PathBuf,
        oneshot::Sender<Result<CatalogSearchOutcome>>,
    ),
    #[cfg(test)]
    Gate(oneshot::Sender<()>, std::sync::mpsc::Receiver<()>),
}

/// Request-scoped cancellation drops only the response, never the worker owner.
#[derive(Clone)]
pub(crate) struct CatalogProjection {
    admission: Arc<Mutex<Option<mpsc::Sender<Job>>>>,
}

pub(crate) struct CatalogProjectionWorker {
    client: CatalogProjection,
    completion: JoinHandle<()>,
}

fn unavailable() -> PumasError {
    PumasError::Config {
        message: "Catalog projection is unavailable".into(),
    }
}

impl CatalogProjection {
    pub(crate) fn start(queue_capacity: usize) -> (Self, CatalogProjectionWorker) {
        let (sender, mut receiver) = mpsc::channel(queue_capacity);
        let client = Self {
            admission: Arc::new(Mutex::new(Some(sender))),
        };
        let completion = tokio::task::spawn_blocking(move || {
            while let Some(job) = receiver.blocking_recv() {
                match job {
                    Job::Models(records, root, reply) => {
                        if !reply.is_closed() {
                            let result = ModelsOutcome::from_records(records, &root);
                            let _ = reply.send(result);
                        }
                    }
                    Job::Search(search, root, reply) => {
                        if !reply.is_closed() {
                            let result = CatalogSearchOutcome::from_search(search, &root);
                            let _ = reply.send(result);
                        }
                    }
                    #[cfg(test)]
                    Job::Gate(started, release) => {
                        let _ = started.send(());
                        release.recv().expect("test gate must release the worker");
                    }
                }
            }
        });
        let worker = CatalogProjectionWorker {
            client: client.clone(),
            completion,
        };
        (client, worker)
    }

    // Plugin-only handler fixtures deliberately have no catalog executor.
    #[cfg(test)]
    pub(crate) fn unavailable() -> Self {
        Self {
            admission: Arc::new(Mutex::new(None)),
        }
    }

    /// Hold the real worker inside an admitted blocking job for shutdown tests.
    #[cfg(test)]
    pub(crate) fn hold_for_test(
        &self,
    ) -> Result<(oneshot::Receiver<()>, std::sync::mpsc::Sender<()>)> {
        let (entered, ready) = oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        self.admit(Job::Gate(entered, blocked))?;
        Ok((ready, release))
    }

    fn admit(&self, job: Job) -> Result<()> {
        self.admission
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
            .ok_or_else(unavailable)?
            .try_send(job)
            .map_err(|_| unavailable())
    }

    pub(crate) async fn models(
        &self,
        records: Vec<ModelRecord>,
        root: PathBuf,
    ) -> Result<ModelsOutcome> {
        let (reply, result) = oneshot::channel();
        self.admit(Job::Models(records, root, reply))?;
        result.await.map_err(|_| unavailable())?
    }

    pub(crate) async fn search(
        &self,
        search: SearchResult,
        root: PathBuf,
    ) -> Result<CatalogSearchOutcome> {
        let (reply, result) = oneshot::channel();
        self.admit(Job::Search(search, root, reply))?;
        result.await.map_err(|_| unavailable())?
    }
}

impl CatalogProjectionWorker {
    /// Close admission atomically, then observe every accepted filesystem read.
    pub(crate) async fn shutdown(self) -> anyhow::Result<()> {
        self.client
            .admission
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog admission poisoned"))?
            .take();
        self.completion
            .await
            .map_err(|error| anyhow::anyhow!("Catalog worker failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_drains_admitted_projection_after_request_cancellation() {
        let (client, worker) = CatalogProjection::start(1);
        let (started, ready) = oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        client.admit(Job::Gate(started, blocked)).unwrap();
        ready.await.unwrap();
        let (reply, response) = oneshot::channel();
        client
            .admit(Job::Models(Vec::new(), PathBuf::new(), reply))
            .unwrap();
        drop(response);
        assert!(client.models(Vec::new(), PathBuf::new()).await.is_err());
        let shutdown = tokio::spawn(worker.shutdown());
        tokio::task::yield_now().await;
        assert!(!shutdown.is_finished());
        release.send(()).unwrap();
        shutdown.await.unwrap().unwrap();
        assert!(client.models(Vec::new(), PathBuf::new()).await.is_err());
    }

    #[tokio::test]
    async fn projection_returns_owned_result_before_shutdown() {
        let (client, worker) = CatalogProjection::start(1);
        let result = client.models(Vec::new(), PathBuf::new()).await.unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({"success":true,"models":{},"unavailable_records":[]})
        );
        worker.shutdown().await.unwrap();
    }
}
