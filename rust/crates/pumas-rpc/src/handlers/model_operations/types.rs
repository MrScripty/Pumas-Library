//! Closed public types. Provider envelopes never cross this boundary.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CONTRACT_VERSION: u32 = 1;
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 256 * 1024;

#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ChatGeneration,
    TextGeneration,
    TextEmbedding,
    ImageGeneration,
    ImageToText,
    AudioTranscription,
    AudioClassification,
    /// One embedding per image or per mixed text/image item, in the same
    /// vector space as the model's text embeddings.
    MultimodalEmbedding,
    /// Typed questions about a state answered with probabilities by a native
    /// decision model (llama.cpp `/v1/systemone`, e.g. Kev, OpenJev).
    Decision,
    /// Relevance of each document to a query, scored by a reranker
    /// (llama.cpp `/v1/rerank`).
    Rerank,
}
impl Capability {
    pub fn path(self) -> Option<&'static str> {
        Some(match self {
            Self::ChatGeneration | Self::ImageToText => "/v1/chat/completions",
            Self::TextGeneration => "/v1/completions",
            Self::TextEmbedding | Self::MultimodalEmbedding => "/v1/embeddings",
            Self::Decision => "/v1/systemone",
            Self::Rerank => "/v1/rerank",
            Self::ImageGeneration => "/v1/images/generations",
            Self::AudioTranscription | Self::AudioClassification => return None,
        })
    }
    pub fn text_generation(self) -> bool {
        matches!(self, Self::ChatGeneration | Self::TextGeneration)
    }
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
    Image {
        encoding: ImageEncoding,
        data_base64: String,
    },
    ImageMessages {
        messages: Vec<ImageMessage>,
    },
    Audio {
        encoding: AudioEncoding,
        sample_rate_hz: u32,
        channels: u16,
        sample_count: u64,
        data_base64: String,
    },
    /// Items to embed; each item's text and image parts produce one vector.
    ContentBatch {
        items: Vec<ContentItem>,
    },
    /// A state and typed questions for a decision model.
    Decision {
        /// Content to evaluate: a string, or JSON given to the model as text.
        state: serde_json::Value,
        /// Question id to question. Each question is answered independently.
        questions: BTreeMap<String, DecisionQuestion>,
        /// Optional images, for decision models with a loaded projector.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<DecisionImage>,
        /// Optional audio clips, for decision models that declare audio input.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        audio: Vec<AudioClip>,
    },
    /// A query and the documents a reranker scores against it.
    Rerank {
        query: String,
        documents: Vec<String>,
    },
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    /// Pick one option; `criteria` maps option to description (or null).
    Choice,
    /// Rate on a scale; `criteria` lists level descriptions, lowest first.
    Score,
    /// Probability that the answer is true; optional `criteria` describes
    /// `true` and `false`.
    Noul,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionQuestion {
    #[serde(rename = "type")]
    pub kind: DecisionType,
    pub instructions: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria: Option<serde_json::Value>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionImage {
    pub encoding: ImageEncoding,
    pub data_base64: String,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentItem {
    pub parts: Vec<ContentPart>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageEncoding {
    Png,
    Jpeg,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageMessage {
    pub role: Role,
    pub content: Vec<ContentPart>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContentPart {
    Text {
        text: String,
    },
    Image {
        encoding: ImageEncoding,
        data_base64: String,
    },
    /// The audio primitive: raw PCM with its declared format.
    Audio {
        encoding: AudioEncoding,
        sample_rate_hz: u32,
        channels: u16,
        sample_count: u64,
        data_base64: String,
    },
}
impl ContentPart {
    pub fn modality(&self) -> Modality {
        match self {
            Self::Text { .. } => Modality::Text,
            Self::Image { .. } => Modality::Image,
            Self::Audio { .. } => Modality::Audio,
        }
    }
}
/// A modality primitive: one kind of data, the same whether it is a model's
/// input or its output. `json` and `vector` are output-only result kinds.
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
    /// Structured data checked against the capability's schema
    /// (decision answers, relevance scores).
    Json,
    /// Embedding vectors.
    Vector,
}
/// What a served model accepts and produces, as its runtime declares it.
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ModalitySet {
    pub input: Vec<Modality>,
    pub output: Vec<Modality>,
}
/// An audio clip for inputs that list clips instead of message parts.
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioClip {
    pub encoding: AudioEncoding,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub sample_count: u64,
    pub data_base64: String,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEncoding {
    PcmS16le,
    PcmF32le,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub content: String,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Text,
    EmbeddingsFloat32,
    PngBase64,
    Labels,
    Decisions,
    RelevanceScores,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
    Decision {},
    Rerank {
        /// Return only the `top_n` most relevant documents; all by default.
        #[serde(default)]
        top_n: Option<usize>,
    },
}
/// Languages the Cohere ASR transcription adapter accepts.
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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

#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
    Decisions {
        answers: BTreeMap<String, DecisionAnswer>,
        /// Prompt tokens evaluated across all questions, when reported.
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
    },
    /// Documents ordered from most to least relevant.
    RelevanceScores {
        results: Vec<RelevanceScore>,
        /// Prompt tokens evaluated across all query-document pairs, when reported.
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
    },
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RelevanceScore {
    /// Position of the document in `input.documents`.
    pub index: usize,
    /// The reranker's score; higher is more relevant. Its scale is the
    /// model's own and is comparable only within one model.
    pub relevance_score: f64,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionAnswer {
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Score {
        /// Probability-weighted level index; may fall between levels.
        score: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Noul {
        /// Probability that the answer is true.
        noul: f64,
    },
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedContract,
    ModelNotFound,
    AmbiguousModel,
    AmbiguousOperation,
    UnsupportedModality,
    CapabilityUnavailable,
    ProviderFailure,
    InvalidProviderResult,
    RequestLimit,
    ResponseLimit,
    TransportLost,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    NotAdmitted,
    Unknown,
}
impl ErrorCode {
    /// Generic reason used when the rejecting site has no more specific fact.
    pub fn default_message(self) -> &'static str {
        match self {
            Self::InvalidRequest => "the request is not valid for this operation",
            Self::UnsupportedContract => "the requested contract_version is not supported",
            Self::ModelNotFound => "no served model matches the requested model and profile",
            Self::AmbiguousModel => {
                "the model alias is served by more than one profile; supply profile"
            }
            Self::AmbiguousOperation => {
                "more than one capability accepts this input and output; supply capability"
            }
            Self::UnsupportedModality => {
                "the selected model has no capability for this input and output"
            }
            Self::CapabilityUnavailable => "the capability is not currently available",
            Self::ProviderFailure => "the inference provider reported a failure",
            Self::InvalidProviderResult => "the inference provider returned an invalid result",
            Self::RequestLimit => "the request exceeds the accepted size",
            Self::ResponseLimit => "the provider response exceeds the accepted size",
            Self::TransportLost => "the connection to the caller or provider was lost",
        }
    }
}
/// A rejected or failed operation and the reason reported to the caller.
///
/// Messages describe the caller's request or the provider's reported failure;
/// they never carry credentials or provider envelopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    pub code: ErrorCode,
    pub message: String,
}
impl Rejection {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: bounded_message(message.into()),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }
}
impl From<ErrorCode> for Rejection {
    fn from(code: ErrorCode) -> Self {
        Self {
            code,
            message: code.default_message().to_owned(),
        }
    }
}
/// Upper bound on a reported reason, in bytes.
pub const MAX_MESSAGE_BYTES: usize = 1024;
fn bounded_message(mut message: String) -> String {
    if message.len() > MAX_MESSAGE_BYTES {
        let mut end = MAX_MESSAGE_BYTES;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message.truncate(end);
    }
    message
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct OperationError {
    pub code: ErrorCode,
    pub outcome: Outcome,
    /// Human-readable reason. Consumers display or log it; they branch on `code`.
    pub message: String,
}
impl OperationError {
    pub fn new(rejection: Rejection, admitted: bool) -> Self {
        Self {
            code: rejection.code,
            outcome: if admitted {
                Outcome::Unknown
            } else {
                Outcome::NotAdmitted
            },
            message: rejection.message,
        }
    }
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
pub struct OperationResponse {
    pub contract_version: u32,
    pub request_id: String,
    pub result: OperationResult,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub contract_version: u32,
    pub request_id: Option<String>,
    pub error: OperationError,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityQuery {
    pub model: String,
    #[serde(default)]
    pub profile: Option<String>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
pub struct CapabilitiesResponse {
    pub supported_contract_versions: Vec<u32>,
    pub model: String,
    pub profile: String,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_stream_event_bytes: usize,
    /// Context window configured for this serving, when the runtime reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u32>,
    /// Modalities the served model accepts and produces, from its runtime.
    pub modalities: ModalitySet,
    pub capabilities: Vec<CapabilityDescriptor>,
}
/// `GET /v1/served-models`: every served (model, profile) pair exactly once.
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
pub struct ServedModelsResponse {
    pub supported_contract_versions: Vec<u32>,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_stream_event_bytes: usize,
    pub models: Vec<ServedModelEntry>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
pub struct ServedModelEntry {
    /// The served alias callers put in `model`.
    pub model: String,
    /// The exact profile callers put in `profile`.
    pub profile: String,
    /// The library model identifier behind the alias.
    pub model_id: String,
    /// Serving provider, for example `llama_cpp`, `ollama`, `onnx_runtime`, `torch`.
    pub provider: String,
    /// Serving load state, for example `loaded` or `loading`.
    pub load_state: String,
    /// Context window configured for this serving, when the runtime reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u32>,
    /// True when the alias is also served by another profile; callers must send `profile`.
    pub profile_required: bool,
    /// Modalities the served model accepts and produces, from its runtime.
    pub modalities: ModalitySet,
    pub capabilities: Vec<CapabilityDescriptor>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
pub struct CapabilityDescriptor {
    pub capability: Capability,
    pub semantic_task: SemanticTask,
    pub input_formats: Vec<InputFormat>,
    pub output_formats: Vec<OutputFormat>,
    /// Content modalities this capability accepts for this served model.
    pub input_modalities: Vec<Modality>,
    /// Modalities this capability produces.
    pub output_modalities: Vec<Modality>,
    pub streaming: bool,
    pub availability: Availability,
    pub option_bounds: Vec<OptionBound>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticTask {
    ChatGeneration,
    TextGeneration,
    TextEmbedding,
    TextToImage,
    SpeechToText,
    AudioClassification,
    ImageToText,
    MultimodalEmbedding,
    Decision,
    Rerank,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputFormat {
    MessagesText,
    PngBase64,
    JpegBase64,
    MessagesImage,
    Text,
    TextBatch,
    PcmS16le,
    PcmF32le,
    ContentBatch,
    Decision,
    Rerank,
    /// Messages whose parts include audio.
    MessagesMultimodal,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
    pub fn reason_text(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable { reason } => reason.description(),
        }
    }
}
impl AvailabilityReason {
    pub fn description(&self) -> &'static str {
        match self {
            Self::UnsupportedAdapter => {
                "unsupported_adapter: the serving provider has no adapter for this capability"
            }
            Self::UnknownModelTask => {
                "unknown_model_task: the model's library record does not state its task"
            }
            Self::ModelTaskMismatch => {
                "model_task_mismatch: the model's recorded task does not support this capability"
            }
            Self::RuntimeUnavailable => {
                "runtime_unavailable: the serving runtime is not ready for this model"
            }
        }
    }
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityReason {
    UnsupportedAdapter,
    UnknownModelTask,
    ModelTaskMismatch,
    RuntimeUnavailable,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
pub struct OptionBound {
    pub option: OptionName,
    pub minimum: f64,
    pub maximum: f64,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
    ImageBytes,
    ImagePixels,
    ImageCount,
    /// Decoded bytes of all images in one request.
    TotalImageBytes,
    /// Messages in one `image_messages` input.
    MessageCount,
    /// Content parts across one request.
    ContentParts,
    /// Tokens per input, including special tokens, from the served model's
    /// own positional limit.
    InputTokens,
    /// Documents returned by a `rerank` operation.
    TopN,
    /// Audio clips in one request.
    AudioCount,
    /// Decoded PCM bytes of all audio in one request.
    TotalAudioBytes,
    /// Seconds per audio clip.
    AudioSeconds,
    /// Sample rate of an audio clip, in Hz.
    SampleRate,
}

#[cfg(test)]
mod image_to_text_grammar_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn vision_named_dto_is_closed_and_preserves_image_parts() {
        let input = json!({"kind":"image_messages","messages":[{"role":"user","content":[{"kind":"text","text":"caption"},{"kind":"image","encoding":"jpeg","data_base64":"AA=="}]}]});
        let parsed: OperationInput = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), input);
        for bad in [
            json!({"kind":"image","encoding":"webp","data_base64":"AA=="}),
            json!({"kind":"image","encoding":"png","data_base64":"AA==","url":"file:///private"}),
            json!({"kind":"image_messages","messages":[{"role":"user","content":[{"kind":"audio","data_base64":"AA=="}]}]}),
            json!({"kind":"image_messages","messages":[{"role":"user","content":[{"kind":"text","text":"caption","provider_options":{}}]}]}),
        ] {
            assert!(serde_json::from_value::<OperationInput>(bad).is_err());
        }
    }

    #[test]
    fn vision_is_finite_and_legacy_messages_still_require_string_content() {
        assert_eq!(Capability::ImageToText.path(), Some("/v1/chat/completions"));
        assert!(!Capability::ImageToText.text_generation());
        assert_eq!(
            serde_json::to_value(Capability::ImageToText).unwrap(),
            "image_to_text"
        );
        assert!(serde_json::from_value::<OperationInput>(json!({"kind":"messages","messages":[{"role":"user","content":[{"kind":"image","encoding":"png","data_base64":"AA=="}]}]})).is_err());
        let raw = br#"{"kind":"image","encoding":"png","encoding":"jpeg","data_base64":"AA=="}"#;
        assert!(serde_json::from_slice::<OperationInput>(raw).is_err());
    }
}

#[cfg(all(test, feature = "export-contract"))]
mod image_to_text_schema_tests {
    use super::super::{modality::ModalityRequest, projection};
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn image_to_text_actual_dto_schema_export() {
        // These are the actual DTO schemas. Cross-field admission, image-byte
        // validity and numeric policy still belong to the Rust validators.
        let schemas: [(&str, Value); 6] = [
            (
                "operation-request.schema.json",
                serde_json::to_value(schemars::schema_for!(OperationRequest)).unwrap(),
            ),
            (
                "modality-request.schema.json",
                serde_json::to_value(schemars::schema_for!(ModalityRequest)).unwrap(),
            ),
            (
                "capabilities-response.schema.json",
                serde_json::to_value(schemars::schema_for!(CapabilitiesResponse)).unwrap(),
            ),
            (
                "capability-descriptor.schema.json",
                serde_json::to_value(schemars::schema_for!(CapabilityDescriptor)).unwrap(),
            ),
            (
                "operation-response.schema.json",
                serde_json::to_value(schemars::schema_for!(OperationResponse)).unwrap(),
            ),
            (
                "error-response.schema.json",
                serde_json::to_value(schemars::schema_for!(ErrorResponse)).unwrap(),
            ),
        ];
        // Documented variants are emitted as `oneOf` entries instead of `enum`.
        let enum_values = |schema: Value| -> Vec<Value> {
            match schema["enum"].as_array() {
                Some(values) => values.clone(),
                None => schema["oneOf"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|variant| {
                        variant["enum"]
                            .as_array()
                            .cloned()
                            .unwrap_or_else(|| vec![variant["const"].clone()])
                    })
                    .collect(),
            }
        };
        let capabilities =
            enum_values(serde_json::to_value(schemars::schema_for!(Capability)).unwrap());
        assert_eq!(
            &capabilities,
            &vec![
                json!("chat_generation"),
                json!("text_generation"),
                json!("text_embedding"),
                json!("image_generation"),
                json!("image_to_text"),
                json!("audio_transcription"),
                json!("audio_classification"),
                json!("multimodal_embedding"),
                json!("decision"),
                json!("rerank"),
            ],
            "new capabilities are appended; consumers ignore ones they do not use"
        );
        let formats =
            enum_values(serde_json::to_value(schemars::schema_for!(InputFormat)).unwrap());
        for format in [
            "png_base64",
            "jpeg_base64",
            "messages_image",
            "messages_text",
            "messages_multimodal",
        ] {
            assert!(formats.contains(&json!(format)));
        }

        let named = json!({"contract_version":1,"request_id":"schema-vision","model":"selected-vision","capability":"image_to_text","input":{"kind":"image","encoding":"png","data_base64":"AA=="},"output":"text","options":{"kind":"text_generation"}});
        let valid_shape: OperationRequest = serde_json::from_value(named.clone()).unwrap();
        projection::validate_image_to_text_request(&valid_shape).unwrap();
        let mut malformed = named.clone();
        malformed["input"]["url"] = json!("https://unselected.example/image");
        assert!(serde_json::from_value::<OperationRequest>(malformed).is_err());
        for field in ["stream", "options"] {
            let mut bad = named.clone();
            if field == "stream" {
                bad["stream"] = json!(true);
            } else {
                bad["options"]["max_tokens"] = json!(0);
            }
            let parsed: OperationRequest = serde_json::from_value(bad).unwrap();
            assert_eq!(
                projection::validate_image_to_text_request(&parsed)
                    .map_err(|rejection| rejection.code),
                Err(ErrorCode::InvalidRequest)
            );
        }
        let facade = json!({"contract_version":1,"request_id":"schema-facade","model":"selected-vision","input":{"kind":"image","encoding":"jpeg","data_base64":"AA=="},"output":"text","semantic_task":"image_to_text"});
        serde_json::from_value::<ModalityRequest>(facade.clone())
            .unwrap()
            .validate()
            .unwrap();
        let mut bad_facade = facade;
        bad_facade["input"]["encoding"] = json!("webp");
        assert!(serde_json::from_value::<ModalityRequest>(bad_facade).is_err());

        if let Some(directory) = std::env::var_os("PUMAS_IMAGE_CONTRACT_EXPORT_DIR") {
            let directory = std::path::PathBuf::from(directory);
            assert!(
                directory.is_dir(),
                "schema export requires the provided existing directory"
            );
            for (filename, schema) in schemas {
                let mut bytes = serde_json::to_vec_pretty(&schema).unwrap();
                bytes.push(b'\n');
                std::fs::write(directory.join(filename), bytes).unwrap();
            }
        }
    }
}
