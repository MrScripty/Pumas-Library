use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use async_trait::async_trait;
use tokio::{sync::Semaphore, time::timeout};

use super::{
    OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxLoadRequest, OnnxModelId, OnnxRuntimeError,
    OnnxSessionStatus,
};

#[async_trait]
pub trait OnnxEmbeddingBackend: Send + Sync {
    async fn load(&self, request: OnnxLoadRequest) -> Result<OnnxSessionStatus, OnnxRuntimeError>;
    async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError>;
    async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError>;
    async fn embed(
        &self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError>;
}

#[derive(Debug)]
pub struct OnnxSessionManager<B> {
    backend: B,
    semaphore: Arc<Semaphore>,
    max_concurrent_operations: u32,
    closed: AtomicBool,
}

impl<B> OnnxSessionManager<B>
where
    B: OnnxEmbeddingBackend,
{
    pub fn new(backend: B, max_concurrent_operations: usize) -> Result<Self, OnnxRuntimeError> {
        if max_concurrent_operations == 0 {
            return Err(OnnxRuntimeError::validation(
                "max_concurrent_operations",
                "max concurrent operations must be greater than zero",
            ));
        }
        let max_concurrent_operations = u32::try_from(max_concurrent_operations).map_err(|_| {
            OnnxRuntimeError::validation(
                "max_concurrent_operations",
                "max concurrent operations exceeds supported semaphore permits",
            )
        })?;
        Ok(Self {
            backend,
            semaphore: Arc::new(Semaphore::new(max_concurrent_operations as usize)),
            max_concurrent_operations,
            closed: AtomicBool::new(false),
        })
    }

    pub async fn load(
        &self,
        request: OnnxLoadRequest,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        let _permit = self.operation_permit().await?;
        self.backend.load(request).await
    }

    pub async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError> {
        let _permit = self.operation_permit().await?;
        self.backend.unload(model_id).await
    }

    pub async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        let _permit = self.operation_permit().await?;
        self.backend.list().await
    }

    pub async fn embed(
        &self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        let _permit = self.operation_permit().await?;
        self.backend.embed(request).await
    }

    pub async fn shutdown(
        &self,
        drain_timeout: Duration,
    ) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        self.closed.store(true, Ordering::SeqCst);
        let permits = timeout(
            drain_timeout,
            self.semaphore
                .clone()
                .acquire_many_owned(self.max_concurrent_operations),
        )
        .await
        .map_err(|_| OnnxRuntimeError::backend("ONNX session manager shutdown timed out"))?
        .map_err(|_| OnnxRuntimeError::backend("ONNX session manager is closed"))?;

        let sessions = self.backend.list().await?;
        let mut unloaded = Vec::with_capacity(sessions.len());
        for session in sessions {
            if let Some(removed) = self.backend.unload(&session.model_id).await? {
                unloaded.push(removed);
            }
        }
        drop(permits);
        Ok(unloaded)
    }

    async fn operation_permit(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, OnnxRuntimeError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(OnnxRuntimeError::backend("ONNX session manager is closed"));
        }
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| OnnxRuntimeError::backend("ONNX session manager is closed"))?;
        if self.closed.load(Ordering::SeqCst) {
            return Err(OnnxRuntimeError::backend("ONNX session manager is closed"));
        }
        Ok(permit)
    }
}
