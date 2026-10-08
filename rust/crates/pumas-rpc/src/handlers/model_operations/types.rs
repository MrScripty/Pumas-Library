//! Closed public types. Provider envelopes never cross this boundary.
use serde::{Deserialize, Serialize};

pub const CONTRACT_VERSION: u32 = 1;
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ChatGeneration,
    TextGeneration,
    TextEmbedding,
    ImageGeneration,
    AudioTranscription,
    AudioClassification,
}
impl Capability {
    pub fn path(self) -> Option<&'static str> {
        Some(match self {
            Self::ChatGeneration => "/v1/chat/completions",
            Self::TextGeneration => "/v1/completions",
            Self::TextEmbedding => "/v1/embeddings",
            Self::ImageGeneration => "/v1/images/generations",
            Self::AudioTranscription | Self::AudioClassification => return None,
        })
    }
    pub fn text_generation(self) -> bool {
        matches!(self, Self::ChatGeneration | Self::TextGeneration)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationRequest {
    pub contract_version: u32,
    pub request_id: String,
    pub model: String,
    #[serde(default)]
    pub profile: Option<String>,
    pub capability: Capability,
    pub input: OperationInput,
    pub output: OutputFormat,
    pub options: OperationOptions,
    #[serde(default)]
    pub stream: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationInput {
    Text {
        text: String,
    },
    Messages {
        messages: Vec<Message>,
    },
    TextBatch {
        texts: Vec<String>,
    },
    Audio {
        encoding: AudioEncoding,
        sample_rate_hz: u32,
        channels: u16,
        sample_count: u64,
        data_base64: String,
    },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEncoding {
    PcmS16le,
    PcmF32le,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub content: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Text,
    EmbeddingsFloat32,
    PngBase64,
    Labels,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationOptions {
    TextGeneration {
        #[serde(default)]
        max_tokens: Option<u32>,
        #[serde(default)]
        temperature: Option<f64>,
        #[serde(default)]
        top_p: Option<f64>,
    },
    Embeddings {
        #[serde(default)]
        dimensions: Option<usize>,
    },
    ImageGeneration {
        width: u32,
        height: u32,
        #[serde(default)]
        seed: Option<u32>,
    },
    Audio {
        #[serde(default)]
        language: Option<AudioLanguage>,
        #[serde(default)]
        max_output_tokens: Option<u32>,
    },
}
/// The existing private speech owner's closed language vocabulary.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioLanguage {
    En,
    De,
    Fr,
    It,
    Es,
    Pt,
    El,
    Nl,
    Pl,
    Vi,
    Zh,
    Ar,
    Ja,
    Ko,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
}
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationResult {
    Text {
        text: String,
        finish_reason: FinishReason,
    },
    Embeddings {
        vectors: Vec<Vec<f32>>,
    },
    Image {
        png_base64: String,
        seed: u32,
    },
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedContract,
    ModelNotFound,
    AmbiguousModel,
    CapabilityUnavailable,
    ProviderFailure,
    InvalidProviderResult,
    RequestLimit,
    ResponseLimit,
    TransportLost,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    NotAdmitted,
    Unknown,
}
#[derive(Debug, Clone, Serialize)]
pub struct OperationError {
    pub code: ErrorCode,
    pub outcome: Outcome,
}
#[derive(Debug, Serialize)]
pub struct OperationResponse {
    pub contract_version: u32,
    pub request_id: String,
    pub result: OperationResult,
}
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub contract_version: u32,
    pub request_id: Option<String>,
    pub error: OperationError,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    Started {
        contract_version: u32,
        request_id: String,
        capability: Capability,
        model: String,
        profile: String,
    },
    Delta {
        request_id: String,
        text: String,
    },
    Completed {
        request_id: String,
        finish_reason: FinishReason,
    },
    Failed {
        request_id: String,
        error: OperationError,
    },
}
impl StreamEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Started { .. } => "started",
            Self::Delta { .. } => "delta",
            Self::Completed { .. } => "completed",
            Self::Failed { .. } => "failed",
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityQuery {
    pub model: String,
    #[serde(default)]
    pub profile: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct CapabilitiesResponse {
    pub supported_contract_versions: Vec<u32>,
    pub model: String,
    pub profile: String,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_stream_event_bytes: usize,
    pub capabilities: Vec<CapabilityDescriptor>,
}
#[derive(Debug, Serialize)]
pub struct CapabilityDescriptor {
    pub capability: Capability,
    pub semantic_task: SemanticTask,
    pub input_formats: Vec<InputFormat>,
    pub output_formats: Vec<OutputFormat>,
    pub streaming: bool,
    pub availability: Availability,
    pub option_bounds: Vec<OptionBound>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticTask {
    ChatGeneration,
    TextGeneration,
    TextEmbedding,
    TextToImage,
    SpeechToText,
    AudioClassification,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputFormat {
    MessagesText,
    Text,
    TextBatch,
    PcmS16le,
    PcmF32le,
}
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability {
    Available,
    Unavailable { reason: AvailabilityReason },
}
impl Availability {
    pub fn available(&self) -> bool {
        matches!(self, Self::Available)
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityReason {
    UnsupportedAdapter,
    UnqualifiedAudioRuntime,
    UnknownModelTask,
    ModelTaskMismatch,
    RuntimeUnavailable,
}
#[derive(Debug, Serialize)]
pub struct OptionBound {
    pub option: OptionName,
    pub minimum: f64,
    pub maximum: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionName {
    MaxTokens,
    Temperature,
    TopP,
    Dimensions,
    InputCount,
    InputCharacters,
    Width,
    Height,
    Seed,
    MaxOutputTokens,
}
