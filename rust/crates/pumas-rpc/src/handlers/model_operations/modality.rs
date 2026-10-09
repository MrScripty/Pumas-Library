//! Modality-first admission resolves only the selected model's declared adapters.
//! Conversion into a legacy operation does not grant runtime or model authority.
use super::{projection, types::*};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModalityRequest {
    pub contract_version: u32,
    pub request_id: String,
    pub model: String,
    #[serde(default)]
    pub profile: Option<String>,
    pub input: Input,
    pub output: Output,
    #[serde(default)]
    pub semantic_task: Option<SemanticTask>,
    #[serde(default)]
    pub options: Option<OperationOptions>,
    #[serde(default)]
    pub stream: bool,
}

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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModalityMessage {
    role: Role,
    content: Content,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Parts(Vec<Part>),
}
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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageEncoding {
    Png,
    Jpeg,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    Text,
    EmbeddingsFloat32,
    PngBase64,
    Labels,
    PcmS16le,
    PcmF32le,
}
impl Output {
    fn adapter_format(self) -> Option<OutputFormat> {
        Some(match self {
            Self::Text => OutputFormat::Text,
            Self::EmbeddingsFloat32 => OutputFormat::EmbeddingsFloat32,
            Self::PngBase64 => OutputFormat::PngBase64,
            Self::Labels => OutputFormat::Labels,
            Self::PcmS16le | Self::PcmF32le => return None,
        })
    }
}
impl ModalityRequest {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.contract_version != CONTRACT_VERSION {
            return Err(ErrorCode::UnsupportedContract);
        }
        if !projection::valid_identifier(&self.request_id, 128)
            || !projection::valid_model(&self.model)
            || self.profile.as_ref().is_some_and(|profile| {
                profile.len() > 128
                    || profile.trim() != profile
                    || pumas_library::models::RuntimeProfileId::parse(profile).is_err()
            })
        {
            return Err(ErrorCode::InvalidRequest);
        }
        match &self.input {
            Input::Text { text } => valid_text(text)?,
            Input::TextBatch { texts } => {
                if texts.is_empty() || texts.len() > 128 {
                    return Err(ErrorCode::InvalidRequest);
                }
                for text in texts {
                    valid_text(text)?;
                }
            }
            Input::Messages { messages } => {
                if messages.is_empty() || messages.len() > 128 {
                    return Err(ErrorCode::InvalidRequest);
                }
                for message in messages {
                    match &message.content {
                        Content::Text(text) => valid_text(text)?,
                        Content::Parts(parts) => {
                            if parts.is_empty() || parts.len() > 128 {
                                return Err(ErrorCode::InvalidRequest);
                            }
                            for part in parts {
                                match part {
                                    Part::Text { text } => valid_text(text)?,
                                    Part::Image {
                                        encoding,
                                        data_base64,
                                    } => valid_image(encoding, data_base64)?,
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
            Input::Image {
                encoding,
                data_base64,
            } => valid_image(encoding, data_base64)?,
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
    fn validate_audio(
        &self,
        encoding: AudioEncoding,
        sample_rate_hz: u32,
        channels: u16,
        sample_count: u64,
        data_base64: &str,
    ) -> Result<(), ErrorCode> {
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
    ) -> Result<OperationRequest, ErrorCode> {
        let output = self
            .output
            .adapter_format()
            .ok_or(ErrorCode::UnsupportedModality)?;
        let input = self.adapter_input()?;
        let candidates: Vec<_> = declarations
            .iter()
            .filter(|descriptor| {
                descriptor.output_formats.contains(&output)
                    && self
                        .semantic_task
                        .is_none_or(|task| task == descriptor.semantic_task)
                    && accepts(descriptor, &input)
                    // streaming includes readiness, so false alone cannot
                    // erase a declared but unavailable text-stream operation.
                    // Nontext adapters still have no structural stream contract.
                    && (!self.stream
                        || descriptor.streaming
                        || (descriptor.capability.text_generation()
                            && !descriptor.availability.available()))
            })
            .collect();
        let available: Vec<_> = candidates
            .iter()
            .filter(|descriptor| descriptor.availability.available())
            .collect();
        let capability = match available.as_slice() {
            [] if candidates.is_empty() => return Err(ErrorCode::UnsupportedModality),
            [] => return Err(ErrorCode::CapabilityUnavailable),
            [descriptor] => descriptor.capability,
            _ => return Err(ErrorCode::AmbiguousOperation),
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
                Capability::TextEmbedding => OperationOptions::Embeddings { dimensions: None },
                Capability::AudioTranscription | Capability::AudioClassification => {
                    OperationOptions::Audio {
                        language: None,
                        max_output_tokens: None,
                    }
                }
                Capability::ImageGeneration => return Err(ErrorCode::InvalidRequest),
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
        projection::provider_request(&operation)?;
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
    fn adapter_input(&self) -> Result<OperationInput, ErrorCode> {
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
            Input::Image { .. } => return Err(ErrorCode::UnsupportedModality),
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
                                        _ => return Err(ErrorCode::UnsupportedModality),
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
                    .collect::<Result<Vec<_>, ErrorCode>>()?,
            },
        })
    }
}
fn valid_text(text: &str) -> Result<(), ErrorCode> {
    if text.trim().is_empty() {
        Err(ErrorCode::InvalidRequest)
    } else {
        Ok(())
    }
}
fn valid_image(_encoding: &ImageEncoding, data: &str) -> Result<(), ErrorCode> {
    // Bound and validate the transport grammar only. No image decoder or vision
    // adapter has been qualified, so this can never authorize provider input.
    let padding = data.bytes().rev().take_while(|byte| *byte == b'=').count();
    if data.is_empty()
        || data.len() > MAX_BYTES
        || !data.len().is_multiple_of(4)
        || padding > 2
        || !data.as_bytes()[..data.len() - padding]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/'))
    {
        Err(ErrorCode::InvalidRequest)
    } else {
        Ok(())
    }
}
fn accepts(descriptor: &CapabilityDescriptor, input: &OperationInput) -> bool {
    let format = match input {
        OperationInput::Text { .. } if descriptor.capability == Capability::ChatGeneration => {
            InputFormat::MessagesText
        }
        OperationInput::Text { .. } => InputFormat::Text,
        OperationInput::TextBatch { .. } => InputFormat::TextBatch,
        OperationInput::Messages { .. } => InputFormat::MessagesText,
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
