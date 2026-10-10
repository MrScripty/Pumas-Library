//! Modality-first admission resolves only the selected model's declared adapters.
//! Conversion into a legacy operation does not grant runtime or model authority.
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
    /// Prepare only image-bearing input for bounded projection before model
    /// lookup. This validation value grants no selection or admission authority;
    /// resolve against the actual selected descriptors must still authorize it.
    pub fn image_preflight(&self) -> Result<Option<OperationRequest>, ErrorCode> {
        let has_image = matches!(self.input, Input::Image { .. })
            || matches!(&self.input, Input::Messages { messages } if messages.iter().any(|message| {
                matches!(&message.content, Content::Parts(parts)
                    if parts.iter().any(|part| matches!(part, Part::Image { .. })))
            }));
        if !has_image {
            return Ok(None);
        }
        if !matches!(self.output, Output::Text)
            || self
                .semantic_task
                .is_some_and(|task| task != SemanticTask::ImageToText)
            || self.stream
        {
            return Err(ErrorCode::UnsupportedModality);
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
                Capability::ImageToText => OperationOptions::TextGeneration {
                    max_tokens: Some(512),
                    temperature: None,
                    top_p: None,
                },
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
        if capability == Capability::ImageToText {
            projection::validate_image_to_text_request(&operation)?;
        } else {
            projection::provider_request(&operation)?;
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
            Input::Image {
                encoding,
                data_base64,
            } => OperationInput::Image {
                encoding: *encoding,
                data_base64: data_base64.clone(),
            },
            Input::Messages { messages }
                if messages.iter().any(|message| {
                    matches!(&message.content, Content::Parts(parts)
                        if parts.iter().any(|part| matches!(part, Part::Image { .. })))
                }) =>
            {
                OperationInput::ImageMessages {
                    messages: messages
                        .iter()
                        .map(|message| {
                            let content = match &message.content {
                                Content::Text(text) => vec![ImagePart::Text { text: text.clone() }],
                                Content::Parts(parts) => parts
                                    .iter()
                                    .map(|part| match part {
                                        Part::Text { text } => {
                                            Ok(ImagePart::Text { text: text.clone() })
                                        }
                                        Part::Image {
                                            encoding,
                                            data_base64,
                                        } => Ok(ImagePart::Image {
                                            encoding: *encoding,
                                            data_base64: data_base64.clone(),
                                        }),
                                        Part::Audio { .. } => Err(ErrorCode::UnsupportedModality),
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                            };
                            Ok(ImageMessage {
                                role: message.role,
                                content,
                            })
                        })
                        .collect::<Result<Vec<_>, ErrorCode>>()?,
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
    // Bound the transport grammar here. Decoded image validation belongs to
    // image_input before projection; this check alone grants no runtime authority.
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
        OperationInput::Image {
            encoding: ImageEncoding::Png,
            ..
        } => InputFormat::PngBase64,
        OperationInput::Image {
            encoding: ImageEncoding::Jpeg,
            ..
        } => InputFormat::JpegBase64,
        OperationInput::ImageMessages { .. } => InputFormat::MessagesImage,
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
            request(image()).resolve(&[]).unwrap_err(),
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
    fn image_preflight_rejects_wrong_semantics_stream_options_and_mixed_audio() {
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
                r.image_preflight().unwrap_err(),
                ErrorCode::UnsupportedModality
            );
        }
        for options in [
            OperationOptions::Embeddings { dimensions: None },
            OperationOptions::TextGeneration {
                max_tokens: Some(2049),
                temperature: None,
                top_p: None,
            },
        ] {
            let mut r = request(image());
            r.options = Some(options);
            assert_eq!(r.image_preflight().unwrap_err(), ErrorCode::InvalidRequest);
        }
        let mixed = request(
            json!({"kind":"messages","messages":[{"role":"user","content":[{"kind":"image","encoding":"png","data_base64":"AA=="},{"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="}]}]}),
        );
        assert_eq!(
            mixed.image_preflight().unwrap_err(),
            ErrorCode::UnsupportedModality
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
            request(image()).resolve(&[]).unwrap_err(),
            ErrorCode::UnsupportedModality
        );
        assert_eq!(
            request(image()).resolve(&[declaration(false)]).unwrap_err(),
            ErrorCode::CapabilityUnavailable
        );
        let mut descriptor = declaration(true);
        descriptor.input_formats = vec![InputFormat::JpegBase64];
        assert_eq!(
            request(image()).resolve(&[descriptor]).unwrap_err(),
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
                r.resolve(&[declaration(true)]).unwrap_err(),
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
            streaming.resolve(&[descriptor]).unwrap_err(),
            ErrorCode::UnsupportedModality
        );
        let mut hint = request(image());
        hint.semantic_task = Some(SemanticTask::ChatGeneration);
        assert_eq!(
            hint.resolve(&[declaration(true)]).unwrap_err(),
            ErrorCode::UnsupportedModality
        );
        let mut output = request(image());
        output.output = Output::PngBase64;
        assert_eq!(
            output.resolve(&[declaration(true)]).unwrap_err(),
            ErrorCode::UnsupportedModality
        );
        for options in [
            OperationOptions::Embeddings {
                dimensions: Some(4),
            },
            OperationOptions::TextGeneration {
                max_tokens: Some(2049),
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
                r.resolve(&[declaration(true)]).unwrap_err(),
                ErrorCode::InvalidRequest
            );
        }
    }
}
