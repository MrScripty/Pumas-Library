use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;

use super::{
    validate_dimensions, OnnxEmbedding, OnnxEmbeddingBackend, OnnxEmbeddingRequest,
    OnnxEmbeddingResponse, OnnxEmbeddingUsage, OnnxLoadRequest, OnnxModelId, OnnxRuntimeError,
    OnnxSessionState, OnnxSessionStatus, DEFAULT_FAKE_EMBEDDING_DIMENSIONS,
};

#[derive(Debug, Default)]
pub struct FakeOnnxEmbeddingBackend {
    sessions: Mutex<HashMap<OnnxModelId, OnnxSessionStatus>>,
}

impl FakeOnnxEmbeddingBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OnnxEmbeddingBackend for FakeOnnxEmbeddingBackend {
    async fn load(&self, request: OnnxLoadRequest) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        let status = OnnxSessionStatus {
            model_id: request.model_id.clone(),
            model_path: request.model_path.path().to_path_buf(),
            execution_provider: request.options.execution_provider,
            embedding_dimensions: request
                .options
                .embedding_dimensions
                .unwrap_or(DEFAULT_FAKE_EMBEDDING_DIMENSIONS),
            state: OnnxSessionState::Loaded,
        };
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX fake backend lock poisoned"))?;
        sessions.insert(request.model_id, status.clone());
        Ok(status)
    }

    async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX fake backend lock poisoned"))?;
        Ok(sessions.remove(model_id))
    }

    async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX fake backend lock poisoned"))?;
        let mut loaded = sessions.values().cloned().collect::<Vec<_>>();
        loaded.sort_by(|left, right| left.model_id.as_str().cmp(right.model_id.as_str()));
        Ok(loaded)
    }

    async fn embed(
        &self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        let status = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| OnnxRuntimeError::backend("ONNX fake backend lock poisoned"))?;
            sessions
                .get(&request.model_id)
                .cloned()
                .ok_or_else(|| OnnxRuntimeError::not_loaded(&request.model_id))?
        };
        let dimensions = request.dimensions.unwrap_or(status.embedding_dimensions);
        validate_dimensions(dimensions)?;

        let mut prompt_tokens = 0usize;
        let data = request
            .input
            .iter()
            .enumerate()
            .map(|(index, text)| {
                prompt_tokens += fake_token_count(text);
                OnnxEmbedding {
                    index,
                    embedding: fake_embedding(text, dimensions),
                }
            })
            .collect::<Vec<_>>();

        Ok(OnnxEmbeddingResponse {
            model: request.model_id.as_str().to_string(),
            data,
            usage: OnnxEmbeddingUsage {
                prompt_tokens,
                total_tokens: prompt_tokens,
            },
        })
    }
}

fn fake_token_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| !token.is_empty())
        .count()
}

fn fake_embedding(text: &str, dimensions: usize) -> Vec<f32> {
    let seed = text.bytes().fold(0u32, |accumulator, byte| {
        accumulator.wrapping_add(byte as u32)
    });
    (0..dimensions)
        .map(|index| ((seed.wrapping_add(index as u32) % 997) as f32) / 997.0)
        .collect()
}
