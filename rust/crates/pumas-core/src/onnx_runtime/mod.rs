//! Rust ONNX Runtime provider/session contracts.
//!
//! ONNX Runtime executes in an ONNX host process per runtime profile (see
//! [`host_protocol`]). The host runs [`RealOnnxEmbeddingBackend`] against the
//! library of the installed version its profile pins; Pumas reaches it through
//! [`RemoteOnnxEmbeddingBackend`]. The fake backend validates serving and
//! gateway contracts without a native runtime.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

mod config;
mod cuda_driver;
mod fake;
pub mod host_protocol;
mod manager;
mod native_library;
mod output;
mod package;
mod postprocess;
mod real;
mod real_backend;
mod remote;
mod tensors;
mod tokenizer;

pub use config::{OnnxInputTokenLimit, OnnxModelConfig};
pub use fake::FakeOnnxEmbeddingBackend;
pub use manager::{OnnxEmbeddingBackend, OnnxOperationLease, OnnxSessionManager};
pub use output::LOGITS_OUTPUT;
pub use postprocess::{
    OnnxEmbeddingPooling, OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor,
    OnnxOutputTensorSelection,
};
pub use real::{probe_execution_provider, OnnxRuntimeSession};
pub use real_backend::{OnnxEmbeddingBackendKind, RealOnnxEmbeddingBackend};
pub use remote::{OnnxHostEndpoint, RemoteOnnxEmbeddingBackend};
pub use tokenizer::{OnnxTokenizedBatch, OnnxTokenizedInput, OnnxTokenizer};

/// Pumas library model IDs are artifact-qualified paths (category, family,
/// repository, file selection, revision) that routinely exceed 128 bytes;
/// the bound matches Pumas's own identifier bound.
const MAX_MODEL_ID_LEN: usize = 4096;
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

/// The ONNX Runtime library of one installed, host-selected runtime version.
///
/// The host resolves it from the version manager's selection. It is never
/// built from a request or model, because loading it executes native code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxRuntimeLibrary {
    version_tag: String,
    path: PathBuf,
}

impl OnnxRuntimeLibrary {
    /// The library inside an installed version directory.
    pub fn installed_version(version_tag: impl Into<String>, version_dir: &Path) -> Self {
        Self {
            version_tag: version_tag.into(),
            path: version_dir.join(crate::onnx_runtime_distribution::library_relative_path()),
        }
    }

    pub fn version_tag(&self) -> &str {
        &self.version_tag
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnnxExecutionProvider {
    Cpu,
    /// NVIDIA CUDA, from an installed `+cuda12` or `+cuda13` build.
    Cuda,
    /// Apple CoreML, part of the macOS build.
    #[serde(rename = "coreml")]
    CoreMl,
}

impl OnnxExecutionProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::CoreMl => "coreml",
        }
    }
}

impl From<crate::onnx_runtime_distribution::OnnxRuntimeExecutionProvider>
    for OnnxExecutionProvider
{
    fn from(provider: crate::onnx_runtime_distribution::OnnxRuntimeExecutionProvider) -> Self {
        use crate::onnx_runtime_distribution::OnnxRuntimeExecutionProvider as Distribution;
        match provider {
            Distribution::Cpu => Self::Cpu,
            Distribution::Cuda => Self::Cuda,
            Distribution::CoreMl => Self::CoreMl,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxLoadOptions {
    pub execution_provider: OnnxExecutionProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_dimensions: Option<usize>,
    /// What the session computes; embeddings unless the model is a
    /// cross-encoder.
    #[serde(default)]
    pub task: OnnxSessionTask,
}

/// What an ONNX session computes from its graph's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OnnxSessionTask {
    /// Pooled token vectors. Without a declared pooling, mean pooling and
    /// L2 normalization apply.
    Embedding {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pooling: Option<OnnxEmbeddingPooling>,
        normalize: bool,
        /// The declared pipeline continues past pooling (Dense, LayerNorm).
        /// Only an output the graph pooled itself (`sentence_embedding`)
        /// carries those stages, so token states are not read.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        requires_pooled_output: bool,
    },
    /// One relevance score per `(query, document)` pair (cross-encoder):
    /// the graph's logit under the model's declared activation.
    Rerank {
        #[serde(default, skip_serializing_if = "OnnxScoreActivation::is_identity")]
        activation: OnnxScoreActivation,
    },
}

/// The monotone activation a cross-encoder declares for its scores
/// (`activation_fn`); without one, scores are raw logits, as on Torch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnnxScoreActivation {
    #[default]
    Identity,
    Sigmoid,
}

impl OnnxScoreActivation {
    fn is_identity(&self) -> bool {
        *self == Self::Identity
    }

    /// The activation named by a declared `activation_fn` class path.
    pub fn from_declared(declared: Option<&str>) -> Result<Self, OnnxRuntimeError> {
        match declared {
            None | Some("torch.nn.modules.linear.Identity") => Ok(Self::Identity),
            Some("torch.nn.modules.activation.Sigmoid") => Ok(Self::Sigmoid),
            Some(other) => Err(OnnxRuntimeError::validation(
                "model",
                format!("the model declares score activation {other}, which the ONNX adapter does not implement"),
            )),
        }
    }

    pub fn apply(self, logit: f32) -> f32 {
        match self {
            Self::Identity => logit,
            Self::Sigmoid => 1.0 / (1.0 + (-logit).exp()),
        }
    }
}

impl OnnxSessionTask {
    /// The task a library model's ONNX graph serves: a cross-encoder for
    /// rerankers, otherwise embeddings pooled and normalized as the model's
    /// sentence-transformers declarations say (ADR 0007).
    pub fn for_model(
        model_type: &str,
        declarations: Option<&crate::model_library::declarations::SentenceTransformersDeclarations>,
    ) -> Result<Self, OnnxRuntimeError> {
        use crate::model_library::declarations::{
            module_role, ModuleRole, Pooling, SentenceTransformersKind,
        };
        let cross_encoder = model_type == "reranker"
            || declarations.is_some_and(|declared| {
                declared.model_kind == SentenceTransformersKind::CrossEncoder
            });
        if cross_encoder {
            if declarations.is_some_and(|declared| declared.logit_score.is_some()) {
                return Err(OnnxRuntimeError::validation(
                    "model",
                    "this reranker scores from language-model logits (LogitScore); its ONNX graph \
                     has no relevance output, so serve it through llama.cpp or Torch",
                ));
            }
            return Ok(Self::Rerank {
                activation: OnnxScoreActivation::from_declared(
                    declarations.and_then(|declared| declared.activation.as_deref()),
                )?,
            });
        }
        let Some(declared) = declarations else {
            return Ok(Self::default());
        };
        let mut requires_pooled_output = false;
        for module in &declared.modules {
            match module_role(module) {
                ModuleRole::Transformer | ModuleRole::Pooling | ModuleRole::Normalize => {}
                // Stages the graph applies before a pooled output, or nowhere.
                ModuleRole::Dense | ModuleRole::LayerNorm => requires_pooled_output = true,
                ModuleRole::LogitScore | ModuleRole::Other => {
                    return Err(OnnxRuntimeError::validation(
                        "model",
                        format!(
                            "the model declares a {module} module, which the ONNX adapter \
                             cannot apply to embeddings"
                        ),
                    ))
                }
            }
        }
        let pooling = match declared.pooling {
            None => None,
            Some(Pooling::Mean) => Some(OnnxEmbeddingPooling::Mean),
            Some(Pooling::Cls) => Some(OnnxEmbeddingPooling::Cls),
            Some(Pooling::LastToken) => Some(OnnxEmbeddingPooling::LastToken),
            Some(Pooling::Max) => Some(OnnxEmbeddingPooling::Max),
            Some(other) => {
                return Err(OnnxRuntimeError::validation(
                    "model",
                    format!("the model declares {other:?} pooling, which the ONNX adapter does not implement"),
                ))
            }
        };
        Ok(Self::Embedding {
            pooling: pooling.or(Some(OnnxEmbeddingPooling::Mean)),
            normalize: declared.normalize,
            requires_pooled_output,
        })
    }
}

impl Default for OnnxSessionTask {
    fn default() -> Self {
        Self::Embedding {
            pooling: None,
            normalize: true,
            requires_pooled_output: false,
        }
    }
}

impl OnnxLoadOptions {
    pub fn cpu(embedding_dimensions: usize) -> Result<Self, OnnxRuntimeError> {
        validate_dimensions(embedding_dimensions)?;
        Ok(Self {
            execution_provider: OnnxExecutionProvider::Cpu,
            embedding_dimensions: Some(embedding_dimensions),
            task: OnnxSessionTask::default(),
        })
    }
}

impl Default for OnnxLoadOptions {
    fn default() -> Self {
        Self {
            execution_provider: OnnxExecutionProvider::Cpu,
            embedding_dimensions: None,
            task: OnnxSessionTask::default(),
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

/// Score each document's relevance to the query with a cross-encoder session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxRerankRequest {
    pub model_id: OnnxModelId,
    pub query: String,
    pub documents: Vec<String>,
}

impl OnnxRerankRequest {
    /// Check the request against the adapter's fixed limits: every pair
    /// repeats the query, so the bound covers the query once per document.
    pub fn parse(
        model_id: impl AsRef<str>,
        query: String,
        documents: Vec<String>,
    ) -> Result<Self, OnnxRuntimeError> {
        let model_id = OnnxModelId::parse(model_id)?;
        if query.trim().is_empty() || documents.iter().any(|document| document.trim().is_empty()) {
            return Err(OnnxRuntimeError::validation(
                "documents",
                "the query and every document must be nonblank",
            ));
        }
        if documents.is_empty() || documents.len() > MAX_EMBEDDING_INPUTS {
            return Err(OnnxRuntimeError::validation(
                "documents",
                format!("rerank accepts 1 to {MAX_EMBEDDING_INPUTS} documents"),
            ));
        }
        let query_chars = query.chars().count();
        let total = documents.iter().try_fold(0usize, |total, document| {
            total
                .checked_add(query_chars)
                .and_then(|total| total.checked_add(document.chars().count()))
        });
        if total.is_none_or(|total| total > MAX_EMBEDDING_INPUT_CHARS) {
            return Err(OnnxRuntimeError::validation(
                "documents",
                format!(
                    "rerank pairs may total at most {MAX_EMBEDDING_INPUT_CHARS} characters \
                     (the query counts once per document)"
                ),
            ));
        }
        Ok(Self {
            model_id,
            query,
            documents,
        })
    }
}

/// One document's relevance score (its logit under the declared activation),
/// by its position in the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxRerankScore {
    pub index: usize,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OnnxRerankResponse {
    pub model: String,
    /// In document order; ordering by relevance is the caller's.
    pub results: Vec<OnnxRerankScore>,
    pub usage: OnnxEmbeddingUsage,
}

/// Fixed limits of the in-process ONNX embedding adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnnxEmbeddingLimits {
    pub max_inputs: usize,
    pub max_input_chars: usize,
    pub max_dimensions: usize,
}

impl OnnxEmbeddingRequest {
    pub const LIMITS: OnnxEmbeddingLimits = OnnxEmbeddingLimits {
        max_inputs: MAX_EMBEDDING_INPUTS,
        max_input_chars: MAX_EMBEDDING_INPUT_CHARS,
        max_dimensions: MAX_EMBEDDING_DIMENSIONS,
    };

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
    #[serde(default)]
    pub task: OnnxSessionTask,
    /// The per-input token limit derived from the model config
    /// (`max_position_embeddings` or `n_positions`), when it declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<usize>,
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
