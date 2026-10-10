//! Closed public types. Provider envelopes never cross this boundary.
use serde::{Deserialize, Serialize};

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
}
impl Capability {
    pub fn path(self) -> Option<&'static str> {
        Some(match self {
            Self::ChatGeneration | Self::ImageToText => "/v1/chat/completions",
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
    pub content: Vec<ImagePart>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImagePart {
    Text {
        text: String,
    },
    Image {
        encoding: ImageEncoding,
        data_base64: String,
    },
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
}
/// The existing private speech owner's closed language vocabulary.
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
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct OperationError {
    pub code: ErrorCode,
    pub outcome: Outcome,
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
    pub capabilities: Vec<CapabilityDescriptor>,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
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
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityReason {
    UnsupportedAdapter,
    UnqualifiedAudioRuntime,
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
        let capability_schema = serde_json::to_value(schemars::schema_for!(Capability)).unwrap();
        let capabilities = capability_schema["enum"].as_array().unwrap();
        assert_eq!(
            capabilities,
            &vec![
                json!("chat_generation"),
                json!("text_generation"),
                json!("text_embedding"),
                json!("image_generation"),
                json!("image_to_text"),
                json!("audio_transcription"),
                json!("audio_classification"),
            ],
            "closed consumers must adopt the complete producer capability vocabulary"
        );
        let input_schema = serde_json::to_value(schemars::schema_for!(InputFormat)).unwrap();
        let formats = input_schema["enum"].as_array().unwrap();
        for format in [
            "png_base64",
            "jpeg_base64",
            "messages_image",
            "messages_text",
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
                bad["options"]["max_tokens"] = json!(2049);
            }
            let parsed: OperationRequest = serde_json::from_value(bad).unwrap();
            assert_eq!(
                projection::validate_image_to_text_request(&parsed),
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
