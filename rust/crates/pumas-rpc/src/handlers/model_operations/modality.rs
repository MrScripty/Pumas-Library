//! The single public request grammar for `/v1/model-operations`.
//!
//! A request names a served model, its input and the output it wants. Pumas
//! resolves the operation from the selected model's current capability
//! descriptors. `capability` and `semantic_task` are optional selectors that
//! narrow resolution; they never grant runtime or model authority.
use super::{projection, types::*};
use serde::Deserialize;

#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModalityRequest {
    pub contract_version: u32,
    pub request_id: String,
    pub model: String,
    #[serde(default)]
    pub profile: Option<String>,
    /// Optional explicit capability. When present, resolution only considers it.
    #[serde(default)]
    pub capability: Option<Capability>,
    pub input: Input,
    pub output: Output,
    #[serde(default)]
    pub semantic_task: Option<SemanticTask>,
    #[serde(default)]
    pub options: Option<OperationOptions>,
    #[serde(default)]
    pub stream: bool,
}

#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    Text {
        text: String,
    },
    TextBatch {
        texts: Vec<String>,
    },
    Messages {
        messages: Vec<ModalityMessage>,
    },
    /// Messages whose content is a list of text and image parts.
    ImageMessages {
        messages: Vec<ImageMessage>,
    },
    Image {
        encoding: ImageEncoding,
        data_base64: String,
    },
    Audio {
        encoding: AudioEncoding,
        sample_rate_hz: u32,
        channels: u16,
        sample_count: u64,
        data_base64: String,
    },
    /// Items to embed together; each item produces one vector.
    ContentBatch {
        items: Vec<ContentItem>,
    },
    /// A state and typed questions for a native decision model.
    Decision {
        state: serde_json::Value,
        questions: std::collections::BTreeMap<String, DecisionQuestion>,
        #[serde(default)]
        images: Vec<DecisionImage>,
        #[serde(default)]
        audio: Vec<AudioClip>,
    },
    /// A query and the documents a reranker scores against it.
    Rerank {
        query: String,
        documents: Vec<String>,
    },
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModalityMessage {
    role: Role,
    content: Content,
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Parts(Vec<Part>),
}
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Part {
    Text {
        text: String,
    },
    Image {
        encoding: ImageEncoding,
        data_base64: String,
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
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    Text,
    EmbeddingsFloat32,
    PngBase64,
    Labels,
    PcmS16le,
    PcmF32le,
    Decisions,
    RelevanceScores,
}
impl Output {
    fn adapter_format(self) -> Option<OutputFormat> {
        Some(match self {
            Self::Text => OutputFormat::Text,
            Self::EmbeddingsFloat32 => OutputFormat::EmbeddingsFloat32,
            Self::PngBase64 => OutputFormat::PngBase64,
            Self::Labels => OutputFormat::Labels,
            Self::Decisions => OutputFormat::Decisions,
            Self::RelevanceScores => OutputFormat::RelevanceScores,
            Self::PcmS16le | Self::PcmF32le => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::EmbeddingsFloat32 => "embeddings_float32",
            Self::PngBase64 => "png_base64",
            Self::Labels => "labels",
            Self::Decisions => "decisions",
            Self::RelevanceScores => "relevance_scores",
            Self::PcmS16le => "pcm_s16le",
            Self::PcmF32le => "pcm_f32le",
        }
    }
}
impl Input {
    fn kind(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::TextBatch { .. } => "text_batch",
            Self::Messages { .. } => "messages",
            Self::ImageMessages { .. } => "image_messages",
            Self::Image { .. } => "image",
            Self::Audio { .. } => "audio",
            Self::ContentBatch { .. } => "content_batch",
            Self::Decision { .. } => "decision",
            Self::Rerank { .. } => "rerank",
        }
    }
    fn has_image(&self) -> bool {
        match self {
            Self::Image { .. } | Self::ImageMessages { .. } => true,
            Self::ContentBatch { items } => items.iter().any(|item| {
                item.parts
                    .iter()
                    .any(|part| matches!(part, ContentPart::Image { .. }))
            }),
            Self::Messages { messages } => messages.iter().any(|message| {
                matches!(&message.content, Content::Parts(parts)
                    if parts.iter().any(|part| matches!(part, Part::Image { .. })))
            }),
            _ => false,
        }
    }
    /// The input carries an audio part (not the bare `audio` input kind,
    /// which is the transcription lane).
    fn has_audio_part(&self) -> bool {
        match self {
            Self::ImageMessages { messages } => messages.iter().any(|message| {
                message
                    .content
                    .iter()
                    .any(|part| matches!(part, ContentPart::Audio { .. }))
            }),
            Self::ContentBatch { items } => items.iter().any(|item| {
                item.parts
                    .iter()
                    .any(|part| matches!(part, ContentPart::Audio { .. }))
            }),
            Self::Messages { messages } => messages.iter().any(|message| {
                matches!(&message.content, Content::Parts(parts)
                    if parts.iter().any(|part| matches!(part, Part::Audio { .. })))
            }),
            _ => false,
        }
    }
}
impl ModalityRequest {
    pub fn validate(&self) -> Result<(), Rejection> {
        if self.contract_version != CONTRACT_VERSION {
            return Err(Rejection::new(
                ErrorCode::UnsupportedContract,
                format!(
                    "contract_version {} is not supported; supported versions: [{}]",
                    self.contract_version, CONTRACT_VERSION
                ),
            ));
        }
        if !projection::valid_identifier(&self.request_id, 128) {
            return Err(Rejection::invalid(
                "request_id must be 1-128 ASCII letters, digits or '-', '_', '.', ':'",
            ));
        }
        if !projection::valid_model(&self.model) {
            return Err(Rejection::invalid(
                "model must be a nonempty served alias of at most 256 bytes without surrounding whitespace or control characters",
            ));
        }
        if let Some(profile) = &self.profile {
            projection::valid_profile(profile)?;
        }
        if self.stream
            && self
                .capability
                .is_some_and(|capability| !capability.text_generation())
        {
            return Err(Rejection::invalid(
                "stream is only supported for chat_generation and text_generation",
            ));
        }
        match &self.input {
            Input::Text { text } => valid_text("input.text", text)?,
            Input::TextBatch { texts } => {
                if texts.is_empty() {
                    return Err(Rejection::invalid("input.texts must not be empty"));
                }
                for (index, text) in texts.iter().enumerate() {
                    valid_text(&format!("input.texts[{index}]"), text)?;
                }
            }
            Input::Messages { messages } => {
                if messages.is_empty() {
                    return Err(Rejection::invalid("input.messages must not be empty"));
                }
                for (index, message) in messages.iter().enumerate() {
                    let field = format!("input.messages[{index}].content");
                    match &message.content {
                        Content::Text(text) => valid_text(&field, text)?,
                        Content::Parts(parts) => {
                            if parts.is_empty() {
                                return Err(Rejection::invalid(format!(
                                    "{field} must not be empty"
                                )));
                            }
                            for (part_index, part) in parts.iter().enumerate() {
                                let field = format!("{field}[{part_index}]");
                                match part {
                                    Part::Text { text } => valid_text(&field, text)?,
                                    Part::Image {
                                        encoding,
                                        data_base64,
                                    } => valid_image(&field, encoding, data_base64)?,
                                    Part::Audio {
                                        encoding,
                                        sample_rate_hz,
                                        channels,
                                        sample_count,
                                        data_base64,
                                    } => self.validate_audio(
                                        *encoding,
                                        *sample_rate_hz,
                                        *channels,
                                        *sample_count,
                                        data_base64,
                                    )?,
                                }
                            }
                        }
                    }
                }
            }
            Input::ImageMessages { messages } => {
                if messages.is_empty() {
                    return Err(Rejection::invalid("input.messages must not be empty"));
                }
                for (index, message) in messages.iter().enumerate() {
                    let field = format!("input.messages[{index}].content");
                    if message.content.is_empty() {
                        return Err(Rejection::invalid(format!("{field} must not be empty")));
                    }
                    for (part_index, part) in message.content.iter().enumerate() {
                        self.validate_part(&format!("{field}[{part_index}]"), part)?;
                    }
                }
            }
            Input::Image {
                encoding,
                data_base64,
            } => valid_image("input", encoding, data_base64)?,
            Input::Decision {
                questions,
                images,
                audio,
                ..
            } => {
                validate_decision_questions(questions)?;
                for clip in audio {
                    self.validate_audio(
                        clip.encoding,
                        clip.sample_rate_hz,
                        clip.channels,
                        clip.sample_count,
                        &clip.data_base64,
                    )?;
                }
                for (index, image) in images.iter().enumerate() {
                    valid_image(
                        &format!("input.images[{index}]"),
                        &image.encoding,
                        &image.data_base64,
                    )?;
                }
            }
            Input::Rerank { query, documents } => {
                valid_text("input.query", query)?;
                if documents.is_empty() {
                    return Err(Rejection::invalid("input.documents must not be empty"));
                }
                for (index, document) in documents.iter().enumerate() {
                    valid_text(&format!("input.documents[{index}]"), document)?;
                }
            }
            Input::ContentBatch { items } => {
                if items.is_empty() {
                    return Err(Rejection::invalid("input.items must not be empty"));
                }
                for (index, item) in items.iter().enumerate() {
                    let field = format!("input.items[{index}].parts");
                    if item.parts.is_empty() {
                        return Err(Rejection::invalid(format!("{field} must not be empty")));
                    }
                    for (part_index, part) in item.parts.iter().enumerate() {
                        self.validate_part(&format!("{field}[{part_index}]"), part)?;
                    }
                }
            }
            Input::Audio {
                encoding,
                sample_rate_hz,
                channels,
                sample_count,
                data_base64,
            } => self.validate_audio(
                *encoding,
                *sample_rate_hz,
                *channels,
                *sample_count,
                data_base64,
            )?,
        }
        Ok(())
    }
    /// Prepare only image-bearing input for bounded projection before model
    /// lookup. This validation value grants no selection or admission authority;
    /// resolve against the actual selected descriptors must still authorize it.
    pub fn image_preflight(&self) -> Result<Option<OperationRequest>, Rejection> {
        if !self.input.has_image()
            && !self.input.has_audio_part()
            && !matches!(self.input, Input::ContentBatch { .. })
        {
            return Ok(None);
        }
        if matches!(self.output, Output::EmbeddingsFloat32) {
            return self.multimodal_embedding_preflight().map(Some);
        }
        // Messages with audio, or an explicit chat request with any non-text
        // part, are chat over the declared primitives. Image-only messages
        // keep resolving to image_to_text, as contract v1 defines.
        let chat = self.input.has_audio_part()
            || self.capability == Some(Capability::ChatGeneration)
            || self.semantic_task == Some(SemanticTask::ChatGeneration);
        if chat && matches!(self.input, Input::Messages { .. } | Input::ImageMessages { .. }) {
            return self.chat_preflight().map(Some);
        }
        if !matches!(self.output, Output::Text) {
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                format!(
                    "image input supports output \"text\", not \"{}\"",
                    self.output.name()
                ),
            ));
        }
        if self
            .semantic_task
            .is_some_and(|task| task != SemanticTask::ImageToText)
            || self
                .capability
                .is_some_and(|capability| capability != Capability::ImageToText)
        {
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                "image input is only accepted by image_to_text",
            ));
        }
        if self.stream {
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                "stream is not supported for image_to_text",
            ));
        }
        let input = self.adapter_input()?;
        let options = self
            .options
            .clone()
            .unwrap_or(OperationOptions::TextGeneration {
                max_tokens: Some(512),
                temperature: None,
                top_p: None,
            });
        let operation = self.operation(Capability::ImageToText, input, OutputFormat::Text, options);
        projection::validate_image_to_text_request(&operation)?;
        Ok(Some(operation))
    }
    fn chat_preflight(&self) -> Result<OperationRequest, Rejection> {
        if !matches!(self.output, Output::Text) {
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                format!(
                    "chat over image or audio parts produces output \"text\", not \"{}\"",
                    self.output.name()
                ),
            ));
        }
        if self
            .capability
            .is_some_and(|capability| capability != Capability::ChatGeneration)
            || self
                .semantic_task
                .is_some_and(|task| task != SemanticTask::ChatGeneration)
        {
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                "messages with audio parts are accepted by chat_generation",
            ));
        }
        if self.stream {
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                "stream is not supported for messages with image or audio parts",
            ));
        }
        let input = self.adapter_input()?;
        let options = self
            .options
            .clone()
            .unwrap_or(OperationOptions::TextGeneration {
                max_tokens: None,
                temperature: None,
                top_p: None,
            });
        let operation =
            self.operation(Capability::ChatGeneration, input, OutputFormat::Text, options);
        projection::validate_image_to_text_request(&operation)?;
        Ok(operation)
    }
    fn multimodal_embedding_preflight(&self) -> Result<OperationRequest, Rejection> {
        if self
            .semantic_task
            .is_some_and(|task| task != SemanticTask::MultimodalEmbedding)
            || self
                .capability
                .is_some_and(|capability| capability != Capability::MultimodalEmbedding)
        {
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                "image embeddings use capability multimodal_embedding",
            ));
        }
        if self.stream {
            return Err(Rejection::invalid("stream is not supported for embeddings"));
        }
        if !matches!(self.input, Input::Image { .. } | Input::ContentBatch { .. }) {
            return Err(Rejection::invalid(
                "multimodal_embedding accepts input kind \"image\" or \"content_batch\"",
            ));
        }
        let options = self
            .options
            .clone()
            .unwrap_or(OperationOptions::Embeddings { dimensions: None });
        let input = self.adapter_input()?;
        Ok(self.operation(
            Capability::MultimodalEmbedding,
            input,
            OutputFormat::EmbeddingsFloat32,
            options,
        ))
    }
    /// The transport grammar of one content part; decoding happens later.
    fn validate_part(&self, field: &str, part: &ContentPart) -> Result<(), Rejection> {
        match part {
            ContentPart::Text { text } => valid_text(field, text),
            ContentPart::Image {
                encoding,
                data_base64,
            } => valid_image(field, encoding, data_base64),
            ContentPart::Audio {
                encoding,
                sample_rate_hz,
                channels,
                sample_count,
                data_base64,
            } => self.validate_audio(
                *encoding,
                *sample_rate_hz,
                *channels,
                *sample_count,
                data_base64,
            ),
        }
    }
    fn validate_audio(
        &self,
        encoding: AudioEncoding,
        sample_rate_hz: u32,
        channels: u16,
        sample_count: u64,
        data_base64: &str,
    ) -> Result<(), Rejection> {
        // Reuse the established PCM envelope bounds. Byte decoding remains with
        // the owned bridge; even a valid envelope is not runtime qualification.
        let operation = self.operation(
            Capability::AudioTranscription,
            OperationInput::Audio {
                encoding,
                sample_rate_hz,
                channels,
                sample_count,
                data_base64: data_base64.to_owned(),
            },
            OutputFormat::Text,
            OperationOptions::Audio {
                language: None,
                max_output_tokens: None,
            },
        );
        projection::provider_request(&operation).map(|_| ())
    }
    pub fn resolve(
        self,
        declarations: &[CapabilityDescriptor],
    ) -> Result<OperationRequest, Rejection> {
        let output = self.output.adapter_format().ok_or_else(|| {
            Rejection::new(
                ErrorCode::UnsupportedModality,
                format!(
                    "output \"{}\" is not produced by any capability",
                    self.output.name()
                ),
            )
        })?;
        let input = self.adapter_input()?;
        let selected: Vec<_> = declarations
            .iter()
            .filter(|descriptor| {
                self.capability
                    .is_none_or(|capability| capability == descriptor.capability)
                    && self
                        .semantic_task
                        .is_none_or(|task| task == descriptor.semantic_task)
            })
            .collect();
        let needs = input_modalities(&input);
        let explicit_chat = self.capability == Some(Capability::ChatGeneration)
            || self.semantic_task == Some(SemanticTask::ChatGeneration);
        let shaped: Vec<_> = selected
            .iter()
            .filter(|descriptor| {
                descriptor.output_formats.contains(&output) && accepts(descriptor, &input, explicit_chat)
            })
            .collect();
        let covers = |descriptor: &CapabilityDescriptor| {
            needs.iter().all(|m| descriptor.input_modalities.contains(m))
        };
        // A part in a modality the served model does not declare is refused
        // here, naming what it does accept. An unavailable capability keeps
        // its availability reason instead: its runtime may not have answered.
        if !shaped.is_empty()
            && shaped
                .iter()
                .all(|descriptor| descriptor.availability.available() && !covers(descriptor))
        {
            let accepted = shaped
                .iter()
                .map(|descriptor| {
                    format!(
                        "{} accepts {}",
                        capability_name(descriptor.capability),
                        modality_list(&descriptor.input_modalities)
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            return Err(Rejection::new(
                ErrorCode::UnsupportedModality,
                format!(
                    "model \"{}\" cannot take this request's {} input ({accepted})",
                    self.model,
                    modality_list(&needs.iter().copied().collect::<Vec<_>>())
                ),
            ));
        }
        let candidates: Vec<_> = selected
            .iter()
            .filter(|descriptor| {
                descriptor.output_formats.contains(&output)
                    && accepts(descriptor, &input, explicit_chat)
                    && (!descriptor.availability.available() || covers(descriptor))
                    // streaming includes readiness, so false alone cannot
                    // erase a declared but unavailable text-stream operation.
                    // Nontext adapters still have no structural stream contract.
                    && (!self.stream
                        || (descriptor.capability != Capability::ImageToText
                            && (descriptor.streaming
                                || (descriptor.capability.text_generation()
                                    && !descriptor.availability.available()))))
            })
            .collect();
        let available: Vec<_> = candidates
            .iter()
            .filter(|descriptor| descriptor.availability.available())
            .collect();
        let capability = match (available.as_slice(), candidates.as_slice()) {
            ([descriptor], _) => descriptor.capability,
            ([], []) => {
                let what = format!(
                    "input \"{}\" with output \"{}\"{}",
                    self.input.kind(),
                    self.output.name(),
                    if self.stream { " and stream" } else { "" }
                );
                return Err(match self.capability {
                    // An explicit capability with a mismatched shape is a request error.
                    Some(capability) => Rejection::invalid(format!(
                        "{} does not accept {what}",
                        capability_name(capability)
                    )),
                    None => Rejection::new(
                        ErrorCode::UnsupportedModality,
                        format!("model \"{}\" has no capability for {what}", self.model),
                    ),
                });
            }
            ([], unavailable) => {
                let reasons = unavailable
                    .iter()
                    .map(|descriptor| {
                        format!(
                            "{}: {}",
                            capability_name(descriptor.capability),
                            descriptor.availability.reason_text()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(Rejection::new(
                    ErrorCode::CapabilityUnavailable,
                    format!(
                        "model \"{}\" cannot run this request now ({reasons})",
                        self.model
                    ),
                ));
            }
            (several, _) => {
                let names = several
                    .iter()
                    .map(|descriptor| capability_name(descriptor.capability))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Rejection::new(
                    ErrorCode::AmbiguousOperation,
                    format!(
                        "several capabilities accept this request ({names}); supply capability"
                    ),
                ));
            }
        };
        // Option shape never participates in task selection. Image size must be
        // explicit; other named adapters already define their default options.
        let options = match self.options.clone() {
            Some(options) => options,
            None => match capability {
                Capability::ChatGeneration | Capability::TextGeneration => {
                    OperationOptions::TextGeneration {
                        max_tokens: None,
                        temperature: None,
                        top_p: None,
                    }
                }
                Capability::ImageToText => OperationOptions::TextGeneration {
                    max_tokens: Some(512),
                    temperature: None,
                    top_p: None,
                },
                Capability::TextEmbedding | Capability::MultimodalEmbedding => {
                    OperationOptions::Embeddings { dimensions: None }
                }
                Capability::AudioTranscription | Capability::AudioClassification => {
                    OperationOptions::Audio {
                        language: None,
                        max_output_tokens: None,
                    }
                }
                Capability::Decision => OperationOptions::Decision {},
                Capability::Rerank => OperationOptions::Rerank { top_n: None },
                Capability::ImageGeneration => {
                    return Err(Rejection::invalid(
                        "image_generation requires options {\"kind\":\"image_generation\",\"width\":...,\"height\":...}",
                    ))
                }
            },
        };
        let input = match (capability, input) {
            (Capability::ChatGeneration, OperationInput::Text { text }) => {
                OperationInput::Messages {
                    messages: vec![Message {
                        role: Role::User,
                        content: text,
                    }],
                }
            }
            (_, input) => input,
        };
        let operation = self.operation(capability, input, output, options);
        match capability {
            Capability::ImageToText => projection::validate_image_to_text_request(&operation)?,
            // Image bytes were already decoded and projected by the preflight;
            // decision images are decoded once, off the async runtime, by the handler.
            Capability::MultimodalEmbedding | Capability::Decision => {}
            _ => {
                projection::provider_request(&operation)?;
            }
        }
        Ok(operation)
    }
    fn operation(
        &self,
        capability: Capability,
        input: OperationInput,
        output: OutputFormat,
        options: OperationOptions,
    ) -> OperationRequest {
        OperationRequest {
            contract_version: self.contract_version,
            request_id: self.request_id.clone(),
            model: self.model.clone(),
            profile: self.profile.clone(),
            capability,
            input,
            output,
            options,
            stream: self.stream,
        }
    }
    fn adapter_input(&self) -> Result<OperationInput, Rejection> {
        let audio_in_messages = || {
            Rejection::new(
                ErrorCode::UnsupportedModality,
                "audio parts inside messages are not supported; send audio as input kind \"audio\"",
            )
        };
        Ok(match &self.input {
            Input::Text { text } => OperationInput::Text { text: text.clone() },
            Input::TextBatch { texts } => OperationInput::TextBatch {
                texts: texts.clone(),
            },
            Input::Audio {
                encoding,
                sample_rate_hz,
                channels,
                sample_count,
                data_base64,
            } => OperationInput::Audio {
                encoding: *encoding,
                sample_rate_hz: *sample_rate_hz,
                channels: *channels,
                sample_count: *sample_count,
                data_base64: data_base64.clone(),
            },
            Input::Image {
                encoding,
                data_base64,
            } => OperationInput::Image {
                encoding: *encoding,
                data_base64: data_base64.clone(),
            },
            Input::ImageMessages { messages } => OperationInput::ImageMessages {
                messages: messages.clone(),
            },
            Input::ContentBatch { items } => OperationInput::ContentBatch {
                items: items.clone(),
            },
            Input::Decision {
                state,
                questions,
                images,
                audio,
            } => OperationInput::Decision {
                state: state.clone(),
                questions: questions.clone(),
                images: images.clone(),
                audio: audio.clone(),
            },
            Input::Rerank { query, documents } => OperationInput::Rerank {
                query: query.clone(),
                documents: documents.clone(),
            },
            Input::Messages { messages }
                if self.input.has_image() || self.input.has_audio_part() =>
            {
                OperationInput::ImageMessages {
                    messages: messages
                        .iter()
                        .map(|message| {
                            let content = match &message.content {
                                Content::Text(text) => vec![ContentPart::Text { text: text.clone() }],
                                Content::Parts(parts) => parts
                                    .iter()
                                    .map(|part| match part {
                                        Part::Text { text } => {
                                            Ok(ContentPart::Text { text: text.clone() })
                                        }
                                        Part::Image {
                                            encoding,
                                            data_base64,
                                        } => Ok(ContentPart::Image {
                                            encoding: *encoding,
                                            data_base64: data_base64.clone(),
                                        }),
                                        Part::Audio {
                                            encoding,
                                            sample_rate_hz,
                                            channels,
                                            sample_count,
                                            data_base64,
                                        } => Ok(ContentPart::Audio {
                                            encoding: *encoding,
                                            sample_rate_hz: *sample_rate_hz,
                                            channels: *channels,
                                            sample_count: *sample_count,
                                            data_base64: data_base64.clone(),
                                        }),
                                    })
                                    .collect::<Result<Vec<_>, Rejection>>()?,
                            };
                            Ok(ImageMessage {
                                role: message.role,
                                content,
                            })
                        })
                        .collect::<Result<Vec<_>, Rejection>>()?,
                }
            }
            Input::Messages { messages } => OperationInput::Messages {
                messages: messages
                    .iter()
                    .map(|message| {
                        let content = match &message.content {
                            Content::Text(text) => text.clone(),
                            Content::Parts(parts) => {
                                let mut text = String::new();
                                for part in parts {
                                    match part {
                                        Part::Text { text: part } => text.push_str(part),
                                        _ => return Err(audio_in_messages()),
                                    }
                                }
                                text
                            }
                        };
                        Ok(Message {
                            role: message.role,
                            content,
                        })
                    })
                    .collect::<Result<Vec<_>, Rejection>>()?,
            },
        })
    }
}
/// Shape checks for decision questions. Option counts and score levels are
/// model limits enforced and reported by the decision runtime.
fn validate_decision_questions(
    questions: &std::collections::BTreeMap<String, DecisionQuestion>,
) -> Result<(), Rejection> {
    if questions.is_empty() {
        return Err(Rejection::invalid("input.questions must not be empty"));
    }
    for (id, question) in questions {
        let field = format!("input.questions.{id}");
        if id.trim().is_empty() {
            return Err(Rejection::invalid("question ids must not be blank"));
        }
        if question.instructions.is_null()
            || question
                .instructions
                .as_str()
                .is_some_and(|text| text.trim().is_empty())
        {
            return Err(Rejection::invalid(format!(
                "{field}.instructions must not be empty"
            )));
        }
        let criteria = question.criteria.as_ref();
        let valid = match question.kind {
            DecisionType::Choice => {
                criteria
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|options| {
                        !options.is_empty()
                            && options.values().all(|v| v.is_string() || v.is_null())
                    })
            }
            DecisionType::Score => criteria
                .and_then(serde_json::Value::as_array)
                .is_some_and(|levels| !levels.is_empty() && levels.iter().all(|v| v.is_string())),
            DecisionType::Noul => criteria.is_none_or(|criteria| {
                criteria.as_object().is_some_and(|object| {
                    object.iter().all(|(key, value)| {
                        matches!(key.as_str(), "true" | "false") && value.is_string()
                    })
                })
            }),
        };
        if !valid {
            return Err(Rejection::invalid(format!(
                "{field}.criteria is invalid: choice needs an object of option to description or null, score needs a list of level descriptions, noul accepts an optional object with \"true\"/\"false\" descriptions"
            )));
        }
    }
    Ok(())
}
/// The content modalities a request's input carries.
pub(super) fn input_modalities(input: &OperationInput) -> std::collections::BTreeSet<Modality> {
    let mut needs = std::collections::BTreeSet::new();
    let mut parts = |parts: &[ContentPart]| needs.extend(parts.iter().map(ContentPart::modality));
    match input {
        OperationInput::Text { .. }
        | OperationInput::TextBatch { .. }
        | OperationInput::Messages { .. }
        | OperationInput::Rerank { .. } => {
            needs.insert(Modality::Text);
        }
        OperationInput::Image { .. } => {
            needs.insert(Modality::Image);
        }
        OperationInput::Audio { .. } => {
            needs.insert(Modality::Audio);
        }
        OperationInput::ImageMessages { messages } => {
            for message in messages {
                parts(&message.content);
            }
        }
        OperationInput::ContentBatch { items } => {
            for item in items {
                parts(&item.parts);
            }
        }
        OperationInput::Decision { images, audio, .. } => {
            needs.insert(Modality::Text);
            if !images.is_empty() {
                needs.insert(Modality::Image);
            }
            if !audio.is_empty() {
                needs.insert(Modality::Audio);
            }
        }
    }
    needs
}
fn modality_list(modalities: &[Modality]) -> String {
    modalities
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok()?.as_str().map(str::to_owned))
        .collect::<Vec<_>>()
        .join(", ")
}
fn capability_name(capability: Capability) -> String {
    serde_json::to_value(capability)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}
fn valid_text(field: &str, text: &str) -> Result<(), Rejection> {
    if text.trim().is_empty() {
        Err(Rejection::invalid(format!("{field} must not be blank")))
    } else {
        Ok(())
    }
}
fn valid_image(field: &str, _encoding: &ImageEncoding, data: &str) -> Result<(), Rejection> {
    // Bound the transport grammar here. Decoded image validation belongs to
    // parts before projection; this check alone grants no runtime authority.
    let padding = data.bytes().rev().take_while(|byte| *byte == b'=').count();
    if data.is_empty()
        || data.len() > MAX_BYTES
        || !data.len().is_multiple_of(4)
        || padding > 2
        || !data.as_bytes()[..data.len() - padding]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/'))
    {
        Err(Rejection::invalid(format!(
            "{field}.data_base64 must be nonempty standard padded base64"
        )))
    } else {
        Ok(())
    }
}
fn accepts(descriptor: &CapabilityDescriptor, input: &OperationInput, explicit_chat: bool) -> bool {
    let format = match input {
        OperationInput::Text { .. } if descriptor.capability == Capability::ChatGeneration => {
            InputFormat::MessagesText
        }
        OperationInput::Text { .. } => InputFormat::Text,
        OperationInput::TextBatch { .. } => InputFormat::TextBatch,
        OperationInput::Messages { .. } => InputFormat::MessagesText,
        OperationInput::Image {
            encoding: ImageEncoding::Png,
            ..
        } => InputFormat::PngBase64,
        OperationInput::Image {
            encoding: ImageEncoding::Jpeg,
            ..
        } => InputFormat::JpegBase64,
        OperationInput::ImageMessages { messages }
            if messages.iter().any(|message| {
                message
                    .content
                    .iter()
                    .any(|part| matches!(part, ContentPart::Audio { .. }))
            }) =>
        {
            InputFormat::MessagesMultimodal
        }
        // Chat over image parts only when the caller asks for chat; v1
        // resolves image-only messages to image_to_text.
        OperationInput::ImageMessages { .. }
            if explicit_chat && descriptor.capability == Capability::ChatGeneration =>
        {
            InputFormat::MessagesMultimodal
        }
        OperationInput::ImageMessages { .. } => InputFormat::MessagesImage,
        OperationInput::ContentBatch { .. } => InputFormat::ContentBatch,
        OperationInput::Decision { .. } => InputFormat::Decision,
        OperationInput::Rerank { .. } => InputFormat::Rerank,
        OperationInput::Audio {
            encoding: AudioEncoding::PcmS16le,
            ..
        } => InputFormat::PcmS16le,
        OperationInput::Audio {
            encoding: AudioEncoding::PcmF32le,
            ..
        } => InputFormat::PcmF32le,
    };
    descriptor.input_formats.contains(&format)
}

#[cfg(test)]
mod image_to_text_resolver_tests {
    use super::*;
    use serde_json::json;

    fn request(input: serde_json::Value) -> ModalityRequest {
        serde_json::from_value(json!({"contract_version":1,"request_id":"vision-1","model":"selected-vision","input":input,"output":"text"})).unwrap()
    }
    fn image() -> serde_json::Value {
        // Transport grammar fixture only; resolution must not decode pixels.
        json!({"kind":"image","encoding":"png","data_base64":"AA=="})
    }
    fn declaration(available: bool) -> CapabilityDescriptor {
        CapabilityDescriptor {
            capability: Capability::ImageToText,
            semantic_task: SemanticTask::ImageToText,
            input_formats: vec![
                InputFormat::PngBase64,
                InputFormat::JpegBase64,
                InputFormat::MessagesImage,
            ],
            output_formats: vec![OutputFormat::Text],
            input_modalities: vec![Modality::Text, Modality::Image],
            output_modalities: vec![Modality::Text],
            streaming: false,
            availability: if available {
                Availability::Available
            } else {
                Availability::Unavailable {
                    reason: AvailabilityReason::RuntimeUnavailable,
                }
            },
            option_bounds: vec![],
        }
    }

    #[test]
    fn image_preflight_prepares_validation_without_decoding_or_authorizing() {
        // AA== has valid transport grammar but is not an image. The parent
        // blocking projection still must validate decoded bytes before lookup.
        let prepared = request(image()).image_preflight().unwrap().unwrap();
        assert_eq!(prepared.capability, Capability::ImageToText);
        assert!(matches!(
            prepared.options,
            OperationOptions::TextGeneration {
                max_tokens: Some(512),
                ..
            }
        ));
        assert_eq!(
            request(image()).resolve(&[]).unwrap_err().code,
            ErrorCode::UnsupportedModality
        );
        for input in [
            json!({"kind":"text","text":"hello"}),
            json!({"kind":"messages","messages":[{"role":"user","content":"hello"}]}),
            json!({"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="}),
        ] {
            assert!(request(input).image_preflight().unwrap().is_none());
        }
        let mixed = request(
            json!({"kind":"messages","messages":[{"role":"user","content":[{"kind":"text","text":"caption"},{"kind":"image","encoding":"jpeg","data_base64":"AA=="}]}]}),
        );
        assert!(matches!(
            mixed.image_preflight().unwrap().unwrap().input,
            OperationInput::ImageMessages { .. }
        ));
    }

    #[test]
    fn image_preflight_rejects_wrong_semantics_and_options_and_routes_audio_to_chat() {
        for (output, semantic_task, stream) in [
            (Output::PngBase64, None, false),
            (Output::Text, Some(SemanticTask::ChatGeneration), false),
            (Output::Text, Some(SemanticTask::ImageToText), true),
        ] {
            let mut r = request(image());
            r.output = output;
            r.semantic_task = semantic_task;
            r.stream = stream;
            assert_eq!(
                r.image_preflight().unwrap_err().code,
                ErrorCode::UnsupportedModality
            );
        }
        for options in [
            OperationOptions::Embeddings { dimensions: None },
            OperationOptions::TextGeneration {
                max_tokens: Some(0),
                temperature: None,
                top_p: None,
            },
        ] {
            let mut r = request(image());
            r.options = Some(options);
            assert_eq!(
                r.image_preflight().unwrap_err().code,
                ErrorCode::InvalidRequest
            );
        }
        let mixed = request(
            json!({"kind":"messages","messages":[{"role":"user","content":[{"kind":"image","encoding":"png","data_base64":"AA=="},{"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="}]}]}),
        );
        // Audio-bearing messages are chat (ADR 0005), never image_to_text.
        assert_eq!(
            mixed.image_preflight().unwrap().unwrap().capability,
            Capability::ChatGeneration
        );
    }

    #[test]
    fn image_resolution_requires_an_available_declared_adapter_and_defaults_tokens() {
        let r = request(image());
        r.validate().unwrap();
        let resolved = r.resolve(&[declaration(true)]).unwrap();
        assert_eq!(resolved.capability, Capability::ImageToText);
        assert!(matches!(
            resolved.options,
            OperationOptions::TextGeneration {
                max_tokens: Some(512),
                ..
            }
        ));
        assert_eq!(
            request(image()).resolve(&[]).unwrap_err().code,
            ErrorCode::UnsupportedModality
        );
        assert_eq!(
            request(image())
                .resolve(&[declaration(false)])
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnavailable
        );
        let mut descriptor = declaration(true);
        descriptor.input_formats = vec![InputFormat::JpegBase64];
        assert_eq!(
            request(image()).resolve(&[descriptor]).unwrap_err().code,
            ErrorCode::UnsupportedModality
        );
    }

    #[test]
    fn image_messages_preserve_roles_and_order_while_all_text_keeps_legacy_conversion() {
        let input = json!({"kind":"messages","messages":[{"role":"system","content":"Be concise"},{"role":"user","content":[{"kind":"text","text":"before"},{"kind":"image","encoding":"jpeg","data_base64":"AA=="},{"kind":"text","text":"after"}]}]});
        let resolved = request(input).resolve(&[declaration(true)]).unwrap();
        assert_eq!(
            serde_json::to_value(resolved.input).unwrap(),
            json!({"kind":"image_messages","messages":[{"role":"system","content":[{"kind":"text","text":"Be concise"}]},{"role":"user","content":[{"kind":"text","text":"before"},{"kind":"image","encoding":"jpeg","data_base64":"AA=="},{"kind":"text","text":"after"}]}]})
        );
        let text = request(
            json!({"kind":"messages","messages":[{"role":"user","content":[{"kind":"text","text":"one"},{"kind":"text","text":"two"}]}]}),
        );
        assert_eq!(
            serde_json::to_value(text.adapter_input().unwrap()).unwrap(),
            json!({"kind":"messages","messages":[{"role":"user","content":"onetwo"}]})
        );
    }

    #[test]
    fn mixed_audio_does_not_acquire_a_vision_adapter() {
        for include_image in [false, true] {
            let mut parts = vec![
                json!({"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="}),
            ];
            if include_image {
                parts.push(image());
            }
            let r =
                request(json!({"kind":"messages","messages":[{"role":"user","content":parts}]}));
            r.validate().unwrap();
            assert_eq!(
                r.resolve(&[declaration(true)]).unwrap_err().code,
                ErrorCode::UnsupportedModality
            );
        }
    }

    #[test]
    fn vision_stream_wrong_hint_output_and_options_are_not_admitted_by_resolution() {
        let mut descriptor = declaration(true);
        descriptor.streaming = true; // Even an incorrect declaration cannot expand the finite contract.
        let mut streaming = request(image());
        streaming.stream = true;
        assert_eq!(
            streaming.resolve(&[descriptor]).unwrap_err().code,
            ErrorCode::UnsupportedModality
        );
        let mut hint = request(image());
        hint.semantic_task = Some(SemanticTask::ChatGeneration);
        assert_eq!(
            hint.resolve(&[declaration(true)]).unwrap_err().code,
            ErrorCode::UnsupportedModality
        );
        let mut output = request(image());
        output.output = Output::PngBase64;
        assert_eq!(
            output.resolve(&[declaration(true)]).unwrap_err().code,
            ErrorCode::UnsupportedModality
        );
        for options in [
            OperationOptions::Embeddings {
                dimensions: Some(4),
            },
            OperationOptions::TextGeneration {
                max_tokens: Some(0),
                temperature: None,
                top_p: None,
            },
            OperationOptions::TextGeneration {
                max_tokens: None,
                temperature: Some(f64::NAN),
                top_p: None,
            },
        ] {
            let mut r = request(image());
            r.options = Some(options);
            assert_eq!(
                r.resolve(&[declaration(true)]).unwrap_err().code,
                ErrorCode::InvalidRequest
            );
        }
    }
}
