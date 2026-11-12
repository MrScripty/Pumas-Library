use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use async_trait::async_trait;

use super::{
    FakeOnnxEmbeddingBackend, OnnxEmbeddingBackend, OnnxEmbeddingRequest, OnnxEmbeddingResponse,
    OnnxExecutionProvider, OnnxHostEndpoint, OnnxLoadRequest, OnnxModelId, OnnxOperationLease,
    OnnxRuntimeError, OnnxRuntimeLibrary, OnnxRuntimeSession, OnnxSessionStatus,
    RemoteOnnxEmbeddingBackend,
};

#[derive(Debug)]
pub enum OnnxEmbeddingBackendKind {
    Fake(FakeOnnxEmbeddingBackend),
    Real(RealOnnxEmbeddingBackend),
    /// Sessions held by runtime-profile ONNX hosts (the Pumas backend).
    Remote(RemoteOnnxEmbeddingBackend),
}

impl OnnxEmbeddingBackendKind {
    pub fn fake() -> Self {
        Self::Fake(FakeOnnxEmbeddingBackend::new())
    }

    pub fn real() -> Self {
        Self::Real(RealOnnxEmbeddingBackend::new())
    }

    pub fn remote() -> Result<Self, OnnxRuntimeError> {
        Ok(Self::Remote(RemoteOnnxEmbeddingBackend::new()?))
    }
}

macro_rules! dispatch {
    ($self:ident, $backend:ident => $call:expr) => {
        match $self {
            Self::Fake($backend) => $call,
            Self::Real($backend) => $call,
            Self::Remote($backend) => $call,
        }
    };
}

#[async_trait]
impl OnnxEmbeddingBackend for OnnxEmbeddingBackendKind {
    async fn load(&self, request: OnnxLoadRequest) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        dispatch!(self, backend => backend.load(request).await)
    }

    async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError> {
        dispatch!(self, backend => backend.unload(model_id).await)
    }

    async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        dispatch!(self, backend => backend.list().await)
    }

    async fn embed(
        &self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        self.embed_with_admission(request, None).await
    }

    async fn embed_with_admission(
        &self,
        request: OnnxEmbeddingRequest,
        admission: Option<&AtomicBool>,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        dispatch!(self, backend => backend.embed_with_admission(request, admission).await)
    }

    async fn load_leased(
        &self,
        request: OnnxLoadRequest,
        lease: OnnxOperationLease,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        dispatch!(self, backend => backend.load_leased(request, lease).await)
    }

    async fn load_leased_on_host(
        &self,
        request: OnnxLoadRequest,
        host: OnnxHostEndpoint,
        lease: OnnxOperationLease,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        dispatch!(self, backend => backend.load_leased_on_host(request, host, lease).await)
    }

    async fn embed_leased(
        &self,
        request: OnnxEmbeddingRequest,
        admission: Option<&AtomicBool>,
        lease: OnnxOperationLease,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        dispatch!(self, backend => backend.embed_leased(request, admission, lease).await)
    }
}

/// ONNX Runtime sessions keyed by model id.
///
/// Model loading and inference are CPU-bound native calls, so they run on
/// Tokio's blocking pool, never on an async worker. The map lock is held only
/// to look up or replace an entry; each session has its own lock, so one
/// model's inference never blocks listing, unloading or another model.
///
/// An ONNX host builds it [`Self::for_runtime`]: every session then uses that
/// installed version's library and the profile's execution provider.
#[derive(Debug, Default)]
pub struct RealOnnxEmbeddingBackend {
    sessions: Mutex<HashMap<OnnxModelId, LoadedSession>>,
    runtime_library: Option<OnnxRuntimeLibrary>,
    execution_provider: Option<OnnxExecutionProvider>,
}

#[derive(Debug)]
struct LoadedSession {
    status: OnnxSessionStatus,
    session: Arc<Mutex<OnnxRuntimeSession>>,
}

impl RealOnnxEmbeddingBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sessions that load `runtime_library` and run on `execution_provider`,
    /// whatever a load request names.
    pub fn for_runtime(
        runtime_library: OnnxRuntimeLibrary,
        execution_provider: OnnxExecutionProvider,
    ) -> Self {
        Self {
            sessions: Mutex::default(),
            runtime_library: Some(runtime_library),
            execution_provider: Some(execution_provider),
        }
    }

    fn sessions(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<OnnxModelId, LoadedSession>>, OnnxRuntimeError>
    {
        self.sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX real backend lock poisoned"))
    }

    async fn load_holding(
        &self,
        mut request: OnnxLoadRequest,
        lease: Option<OnnxOperationLease>,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        if let Some(provider) = self.execution_provider {
            request.options.execution_provider = provider;
        }
        let runtime_library = self.runtime_library.clone();
        let session = run_blocking(lease, move || {
            OnnxRuntimeSession::load_with_runtime_library(request, runtime_library.as_ref())
        })
        .await?;
        let status = session.status();
        self.sessions()?.insert(
            status.model_id.clone(),
            LoadedSession {
                status: status.clone(),
                session: Arc::new(Mutex::new(session)),
            },
        );
        Ok(status)
    }

    async fn embed_holding(
        &self,
        request: OnnxEmbeddingRequest,
        admission: Option<&AtomicBool>,
        lease: Option<OnnxOperationLease>,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        let session = self
            .sessions()?
            .get(&request.model_id)
            .map(|loaded| loaded.session.clone())
            .ok_or_else(|| OnnxRuntimeError::not_loaded(&request.model_id))?;
        if let Some(marker) = admission {
            marker.store(true, Ordering::Release);
        }
        run_blocking(lease, move || {
            session
                .lock()
                .map_err(|_| OnnxRuntimeError::backend("ONNX session lock poisoned"))?
                .embed(request)
        })
        .await
    }
}

/// Run blocking ONNX work off the async executor.
///
/// Dropping the returned future does not stop native work already running, so
/// `lease` moves into the work and is released only when the work ends.
async fn run_blocking<T, F>(
    lease: Option<OnnxOperationLease>,
    work: F,
) -> Result<T, OnnxRuntimeError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, OnnxRuntimeError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _lease = lease;
        work()
    })
    .await
    .map_err(|error| {
        OnnxRuntimeError::backend(if error.is_panic() {
            "ONNX Runtime work panicked"
        } else {
            "ONNX Runtime work was cancelled"
        })
    })?
}

#[async_trait]
impl OnnxEmbeddingBackend for RealOnnxEmbeddingBackend {
    async fn load(&self, request: OnnxLoadRequest) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        self.load_holding(request, None).await
    }

    async fn load_leased(
        &self,
        request: OnnxLoadRequest,
        lease: OnnxOperationLease,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        self.load_holding(request, Some(lease)).await
    }

    async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError> {
        // An in-flight embed keeps its own handle and finishes first; the
        // session is dropped when that handle is released.
        Ok(self
            .sessions()?
            .remove(model_id)
            .map(|loaded| loaded.status))
    }

    async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        let mut loaded = self
            .sessions()?
            .values()
            .map(|loaded| loaded.status.clone())
            .collect::<Vec<_>>();
        loaded.sort_by(|left, right| left.model_id.as_str().cmp(right.model_id.as_str()));
        Ok(loaded)
    }

    async fn embed(
        &self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        self.embed_with_admission(request, None).await
    }

    async fn embed_with_admission(
        &self,
        request: OnnxEmbeddingRequest,
        admission: Option<&AtomicBool>,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        self.embed_holding(request, admission, None).await
    }

    async fn embed_leased(
        &self,
        request: OnnxEmbeddingRequest,
        admission: Option<&AtomicBool>,
        lease: OnnxOperationLease,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        self.embed_holding(request, admission, Some(lease)).await
    }
}

#[cfg(test)]
mod lease_tests {
    use std::{sync::mpsc, time::Duration};

    use tokio::sync::Semaphore;

    use super::*;

    /// A disconnect drops the awaiting future while native work still runs; the
    /// work keeps its share of the limit until it ends, so neither new work nor
    /// shutdown can claim that capacity early.
    #[tokio::test]
    async fn cancelled_blocking_work_keeps_its_lease_until_it_ends() {
        let semaphore = Arc::new(Semaphore::new(1));
        let lease = OnnxOperationLease::new(semaphore.clone().acquire_owned().await.unwrap());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let work = run_blocking(Some(lease), move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        });
        let mut work = Box::pin(work);
        tokio::select! {
            _ = &mut work => panic!("work must still be blocked"),
            _ = tokio::task::spawn_blocking(move || started_rx.recv().unwrap()) => {}
        }
        drop(work);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            semaphore.available_permits(),
            0,
            "running work keeps its lease"
        );

        release_tx.send(()).unwrap();
        let _returned = tokio::time::timeout(Duration::from_secs(5), semaphore.acquire())
            .await
            .expect("the lease returns when the work ends")
            .unwrap();
    }
}
