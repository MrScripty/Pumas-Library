//! Rust ONNX Runtime provider/session contracts.
//!
//! This module intentionally starts with a fake backend so serving and gateway
//! slices can validate the public contract before real ONNX Runtime packages are
//! added.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

mod config;
mod fake;
mod manager;
mod package;
mod postprocess;
mod real;
mod tokenizer;

pub use config::OnnxModelConfig;
pub use fake::FakeOnnxEmbeddingBackend;
pub use manager::{OnnxEmbeddingBackend, OnnxSessionManager};
pub use postprocess::{
    OnnxEmbeddingPooling, OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor,
    OnnxOutputTensorSelection,
};
pub use real::OnnxRuntimeSession;
pub use tokenizer::{OnnxTokenizedBatch, OnnxTokenizedInput, OnnxTokenizer};

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
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
        {
            return Err(OnnxRuntimeError::validation(
                "model_id",
                "model id may only contain ASCII letters, numbers, '.', '-', '_', or '/'",
            ));
        }
        if trimmed.starts_with('/')
            || trimmed
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(OnnxRuntimeError::validation(
                "model_id",
                "model id path segments must be non-empty and may not be '.' or '..'",
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_dimensions: Option<usize>,
}

impl OnnxLoadOptions {
    pub fn cpu(embedding_dimensions: usize) -> Result<Self, OnnxRuntimeError> {
        validate_dimensions(embedding_dimensions)?;
        Ok(Self {
            execution_provider: OnnxExecutionProvider::Cpu,
            embedding_dimensions: Some(embedding_dimensions),
        })
    }
}

impl Default for OnnxLoadOptions {
    fn default() -> Self {
        Self {
            execution_provider: OnnxExecutionProvider::Cpu,
            embedding_dimensions: None,
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

#[cfg(test)]
mod tests;
