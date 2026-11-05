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
    OnnxLoadRequest, OnnxModelId, OnnxRuntimeError, OnnxRuntimeSession, OnnxSessionStatus,
};

#[derive(Debug)]
pub enum OnnxEmbeddingBackendKind {
    Fake(FakeOnnxEmbeddingBackend),
    Real(RealOnnxEmbeddingBackend),
}

impl OnnxEmbeddingBackendKind {
    pub fn fake() -> Self {
        Self::Fake(FakeOnnxEmbeddingBackend::new())
    }

    pub fn real() -> Self {
        Self::Real(RealOnnxEmbeddingBackend::new())
    }
}

#[async_trait]
impl OnnxEmbeddingBackend for OnnxEmbeddingBackendKind {
    async fn load(&self, request: OnnxLoadRequest) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        match self {
            Self::Fake(backend) => backend.load(request).await,
            Self::Real(backend) => backend.load(request).await,
        }
    }

    async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError> {
        match self {
            Self::Fake(backend) => backend.unload(model_id).await,
            Self::Real(backend) => backend.unload(model_id).await,
        }
    }

    async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        match self {
            Self::Fake(backend) => backend.list().await,
            Self::Real(backend) => backend.list().await,
        }
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
        match self {
            Self::Fake(backend) => backend.embed_with_admission(request, admission).await,
            Self::Real(backend) => backend.embed_with_admission(request, admission).await,
        }
    }
}

/// ONNX Runtime sessions keyed by model id.
///
/// Model loading and inference are CPU-bound native calls, so they run on
/// Tokio's blocking pool, never on an async worker. The map lock is held only
/// to look up or replace an entry; each session has its own lock, so one
/// model's inference never blocks listing, unloading or another model.
#[derive(Debug, Default)]
pub struct RealOnnxEmbeddingBackend {
    sessions: Mutex<HashMap<OnnxModelId, LoadedSession>>,
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

    fn sessions(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<OnnxModelId, LoadedSession>>, OnnxRuntimeError>
    {
        self.sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX real backend lock poisoned"))
    }
}

/// Run blocking ONNX work off the async executor.
async fn run_blocking<T, F>(work: F) -> Result<T, OnnxRuntimeError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, OnnxRuntimeError> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|error| {
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
        let session = run_blocking(move || OnnxRuntimeSession::load(request)).await?;
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
        let session = self
            .sessions()?
            .get(&request.model_id)
            .map(|loaded| loaded.session.clone())
            .ok_or_else(|| OnnxRuntimeError::not_loaded(&request.model_id))?;
        if let Some(marker) = admission {
            marker.store(true, Ordering::Release);
        }
        run_blocking(move || {
            session
                .lock()
                .map_err(|_| OnnxRuntimeError::backend("ONNX session lock poisoned"))?
                .embed(request)
        })
        .await
    }
}
