use std::{collections::HashMap, sync::Mutex};

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
        match self {
            Self::Fake(backend) => backend.embed(request).await,
            Self::Real(backend) => backend.embed(request).await,
        }
    }
}

#[derive(Debug, Default)]
pub struct RealOnnxEmbeddingBackend {
    sessions: Mutex<HashMap<OnnxModelId, OnnxRuntimeSession>>,
}

impl RealOnnxEmbeddingBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OnnxEmbeddingBackend for RealOnnxEmbeddingBackend {
    async fn load(&self, request: OnnxLoadRequest) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        let session = OnnxRuntimeSession::load(request)?;
        let status = session.status();
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX real backend lock poisoned"))?;
        sessions.insert(status.model_id.clone(), session);
        Ok(status)
    }

    async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX real backend lock poisoned"))?;
        Ok(sessions.remove(model_id).map(|session| session.status()))
    }

    async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX real backend lock poisoned"))?;
        let mut loaded = sessions
            .values()
            .map(OnnxRuntimeSession::status)
            .collect::<Vec<_>>();
        loaded.sort_by(|left, right| left.model_id.as_str().cmp(right.model_id.as_str()));
        Ok(loaded)
    }

    async fn embed(
        &self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX real backend lock poisoned"))?;
        let session = sessions
            .get_mut(&request.model_id)
            .ok_or_else(|| OnnxRuntimeError::not_loaded(&request.model_id))?;
        session.embed(request)
    }
}
