//! Rust ONNX Runtime provider/session contracts.
//!
//! This module intentionally starts with a fake backend so serving and gateway
//! slices can validate the public contract before real ONNX Runtime packages are
//! added.

use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

const MAX_MODEL_ID_LEN: usize = 128;
const MAX_EMBEDDING_INPUTS: usize = 128;
const MAX_EMBEDDING_INPUT_CHARS: usize = 65_536;
const MAX_EMBEDDING_DIMENSIONS: usize = 8_192;
const DEFAULT_FAKE_EMBEDDING_DIMENSIONS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OnnxModelId(String);

impl OnnxModelId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, OnnxRuntimeError> {
        let trimmed = value.as_ref().trim();
        if trimmed.is_empty() {
            return Err(OnnxRuntimeError::validation(
                "model_id",
                "model id is required",
            ));
        }
        if trimmed.len() > MAX_MODEL_ID_LEN {
            return Err(OnnxRuntimeError::validation(
                "model_id",
                format!("model id must be at most {MAX_MODEL_ID_LEN} bytes"),
            ));
        }
        if !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            return Err(OnnxRuntimeError::validation(
                "model_id",
                "model id may only contain ASCII letters, numbers, '.', '-', or '_'",
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxModelPath {
    root: PathBuf,
    path: PathBuf,
}

impl OnnxModelPath {
    pub fn parse(root: impl AsRef<Path>, path: impl AsRef<Path>) -> Result<Self, OnnxRuntimeError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|err| OnnxRuntimeError::path("root", "model root is invalid", err))?;
        if !root.is_dir() {
            return Err(OnnxRuntimeError::validation(
                "root",
                "model root must be a directory",
            ));
        }

        let candidate = path.as_ref();
        let candidate = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.join(candidate)
        };
        let canonical = candidate
            .canonicalize()
            .map_err(|err| OnnxRuntimeError::path("path", "model path is invalid", err))?;
        if !canonical.starts_with(&root) {
            return Err(OnnxRuntimeError::validation(
                "path",
                "model path must stay inside the configured model root",
            ));
        }
        if !canonical.is_file() {
            return Err(OnnxRuntimeError::validation(
                "path",
                "model path must point to an ONNX file",
            ));
        }
        let is_onnx = canonical
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("onnx"));
        if !is_onnx {
            return Err(OnnxRuntimeError::validation(
                "path",
                "model path must use the .onnx extension",
            ));
        }

        Ok(Self {
            root,
            path: canonical,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnnxExecutionProvider {
    Cpu,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxLoadOptions {
    pub execution_provider: OnnxExecutionProvider,
    pub embedding_dimensions: usize,
}

impl OnnxLoadOptions {
    pub fn cpu(embedding_dimensions: usize) -> Result<Self, OnnxRuntimeError> {
        validate_dimensions(embedding_dimensions)?;
        Ok(Self {
            execution_provider: OnnxExecutionProvider::Cpu,
            embedding_dimensions,
        })
    }
}

impl Default for OnnxLoadOptions {
    fn default() -> Self {
        Self {
            execution_provider: OnnxExecutionProvider::Cpu,
            embedding_dimensions: DEFAULT_FAKE_EMBEDDING_DIMENSIONS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxLoadRequest {
    pub model_id: OnnxModelId,
    pub model_path: OnnxModelPath,
    pub options: OnnxLoadOptions,
}

impl OnnxLoadRequest {
    pub fn parse(
        root: impl AsRef<Path>,
        path: impl AsRef<Path>,
        model_id: impl AsRef<str>,
        options: OnnxLoadOptions,
    ) -> Result<Self, OnnxRuntimeError> {
        Ok(Self {
            model_id: OnnxModelId::parse(model_id)?,
            model_path: OnnxModelPath::parse(root, path)?,
            options,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxEmbeddingRequest {
    pub model_id: OnnxModelId,
    pub input: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl OnnxEmbeddingRequest {
    pub fn parse(
        model_id: impl AsRef<str>,
        input: Vec<String>,
        dimensions: Option<usize>,
    ) -> Result<Self, OnnxRuntimeError> {
        let model_id = OnnxModelId::parse(model_id)?;
        validate_embedding_input(&input)?;
        if let Some(dimensions) = dimensions {
            validate_dimensions(dimensions)?;
        }
        Ok(Self {
            model_id,
            input,
            dimensions,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnnxSessionState {
    Loaded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxSessionStatus {
    pub model_id: OnnxModelId,
    pub model_path: PathBuf,
    pub execution_provider: OnnxExecutionProvider,
    pub embedding_dimensions: usize,
    pub state: OnnxSessionState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxEmbedding {
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxEmbeddingUsage {
    pub prompt_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxEmbeddingResponse {
    pub model: String,
    pub data: Vec<OnnxEmbedding>,
    pub usage: OnnxEmbeddingUsage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnnxRuntimeErrorCode {
    Validation,
    NotLoaded,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxRuntimeError {
    pub code: OnnxRuntimeErrorCode,
    pub field: Option<String>,
    pub message: String,
}

impl OnnxRuntimeError {
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: OnnxRuntimeErrorCode::Validation,
            field: Some(field.into()),
            message: message.into(),
        }
    }

    pub fn not_loaded(model_id: &OnnxModelId) -> Self {
        Self {
            code: OnnxRuntimeErrorCode::NotLoaded,
            field: Some("model".to_string()),
            message: format!("ONNX model '{}' is not loaded", model_id.as_str()),
        }
    }

    pub fn backend(message: impl Into<String>) -> Self {
        Self {
            code: OnnxRuntimeErrorCode::Backend,
            field: None,
            message: message.into(),
        }
    }

    fn path(field: impl Into<String>, message: impl Into<String>, source: std::io::Error) -> Self {
        Self {
            code: OnnxRuntimeErrorCode::Validation,
            field: Some(field.into()),
            message: format!("{}: {source}", message.into()),
        }
    }
}

impl fmt::Display for OnnxRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.field {
            Some(field) => write!(formatter, "{}: {}", field, self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for OnnxRuntimeError {}

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
        Ok(Self {
            backend,
            semaphore: Arc::new(Semaphore::new(max_concurrent_operations)),
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

    async fn operation_permit(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, OnnxRuntimeError> {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| OnnxRuntimeError::backend("ONNX session manager is closed"))
    }
}

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
            embedding_dimensions: request.options.embedding_dimensions,
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

fn validate_embedding_input(input: &[String]) -> Result<(), OnnxRuntimeError> {
    if input.is_empty() {
        return Err(OnnxRuntimeError::validation(
            "input",
            "embedding input must contain at least one item",
        ));
    }
    if input.len() > MAX_EMBEDDING_INPUTS {
        return Err(OnnxRuntimeError::validation(
            "input",
            format!("embedding input must contain at most {MAX_EMBEDDING_INPUTS} items"),
        ));
    }
    let total_chars = input
        .iter()
        .map(|value| value.chars().count())
        .sum::<usize>();
    if total_chars > MAX_EMBEDDING_INPUT_CHARS {
        return Err(OnnxRuntimeError::validation(
            "input",
            format!("embedding input must contain at most {MAX_EMBEDDING_INPUT_CHARS} characters"),
        ));
    }
    Ok(())
}

fn validate_dimensions(dimensions: usize) -> Result<(), OnnxRuntimeError> {
    if dimensions == 0 {
        return Err(OnnxRuntimeError::validation(
            "dimensions",
            "embedding dimensions must be greater than zero",
        ));
    }
    if dimensions > MAX_EMBEDDING_DIMENSIONS {
        return Err(OnnxRuntimeError::validation(
            "dimensions",
            format!("embedding dimensions must be at most {MAX_EMBEDDING_DIMENSIONS}"),
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn model_fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("model.onnx"), b"fake").unwrap();
        temp
    }

    #[test]
    fn model_path_rejects_root_escape() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();

        let err = OnnxModelPath::parse(root.path(), outside.path()).unwrap_err();

        assert_eq!(err.code, OnnxRuntimeErrorCode::Validation);
        assert_eq!(err.field.as_deref(), Some("path"));
    }

    #[test]
    fn model_path_requires_onnx_extension() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("model.bin"), b"fake").unwrap();

        let err = OnnxModelPath::parse(root.path(), "model.bin").unwrap_err();

        assert_eq!(err.field.as_deref(), Some("path"));
        assert!(err.message.contains(".onnx"));
    }

    #[test]
    fn embedding_request_validates_model_id_and_shape() {
        let err =
            OnnxEmbeddingRequest::parse("bad/id", vec!["hello".to_string()], None).unwrap_err();
        assert_eq!(err.field.as_deref(), Some("model_id"));

        let err = OnnxEmbeddingRequest::parse("model", Vec::new(), None).unwrap_err();
        assert_eq!(err.field.as_deref(), Some("input"));

        let err =
            OnnxEmbeddingRequest::parse("model", vec!["hello".to_string()], Some(0)).unwrap_err();
        assert_eq!(err.field.as_deref(), Some("dimensions"));
    }

    #[test]
    fn embedding_request_rejects_oversized_payloads() {
        let too_many_inputs = vec!["hello".to_string(); MAX_EMBEDDING_INPUTS + 1];
        let err = OnnxEmbeddingRequest::parse("model", too_many_inputs, None).unwrap_err();
        assert_eq!(err.field.as_deref(), Some("input"));

        let too_many_chars = vec!["x".repeat(MAX_EMBEDDING_INPUT_CHARS + 1)];
        let err = OnnxEmbeddingRequest::parse("model", too_many_chars, None).unwrap_err();
        assert_eq!(err.field.as_deref(), Some("input"));
    }

    #[tokio::test]
    async fn fake_backend_loads_embeds_lists_and_unloads() {
        let fixture = model_fixture();
        let manager = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 2).unwrap();
        let load = OnnxLoadRequest::parse(
            fixture.path(),
            "model.onnx",
            "nomic-embed-text-v1.5",
            OnnxLoadOptions::cpu(4).unwrap(),
        )
        .unwrap();

        let loaded = manager.load(load).await.unwrap();
        assert_eq!(loaded.embedding_dimensions, 4);

        let listed = manager.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].model_id.as_str(), "nomic-embed-text-v1.5");

        let response = manager
            .embed(
                OnnxEmbeddingRequest::parse(
                    "nomic-embed-text-v1.5",
                    vec!["hello world".to_string()],
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.model, "nomic-embed-text-v1.5");
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].embedding.len(), 4);
        assert_eq!(response.usage.total_tokens, 2);

        let removed = manager
            .unload(&OnnxModelId::parse("nomic-embed-text-v1.5").unwrap())
            .await
            .unwrap();
        assert!(removed.is_some());
        assert!(manager.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn fake_backend_rejects_embedding_before_load() {
        let manager = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 1).unwrap();

        let err = manager
            .embed(OnnxEmbeddingRequest::parse("model", vec!["hello".to_string()], None).unwrap())
            .await
            .unwrap_err();

        assert_eq!(err.code, OnnxRuntimeErrorCode::NotLoaded);
    }

    #[test]
    fn session_manager_requires_positive_concurrency_limit() {
        let err = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 0).unwrap_err();

        assert_eq!(err.field.as_deref(), Some("max_concurrent_operations"));
    }
}
