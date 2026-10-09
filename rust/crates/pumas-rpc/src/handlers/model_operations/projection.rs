use super::types::*;
use serde_json::{json, Value};

/// Closed typed admission precedes every provider-facing effect.
pub fn provider_request(request: &OperationRequest) -> Result<Value, ErrorCode> {
    if request.contract_version != CONTRACT_VERSION {
        return Err(ErrorCode::UnsupportedContract);
    }
    if !valid_identifier(&request.request_id, 128)
        || !valid_model(&request.model)
        || request.profile.as_ref().is_some_and(|p| {
            p.len() > 128
                || pumas_library::models::RuntimeProfileId::parse(p).is_err()
                || p.trim() != p
        })
    {
        return Err(ErrorCode::InvalidRequest);
    }
    if request.stream && !request.capability.text_generation() {
        return Err(ErrorCode::InvalidRequest);
    }
    if request.capability == Capability::ImageToText {
        validate_image_to_text_request(request)?;
    }
    let mut body = json!({"model": request.model});
    match (
        &request.capability,
        &request.input,
        &request.options,
        request.output,
    ) {
        (
            Capability::ChatGeneration,
            OperationInput::Messages { messages },
            OperationOptions::TextGeneration { .. },
            OutputFormat::Text,
        ) => {
            if messages.is_empty() || messages.iter().any(|m| m.content.trim().is_empty()) {
                return Err(ErrorCode::InvalidRequest);
            }
            body["messages"] = json!(messages
                .iter()
                .map(|m| json!({"role":m.role,"content":m.content}))
                .collect::<Vec<_>>());
        }
        (
            Capability::ImageToText,
            OperationInput::Image { .. } | OperationInput::ImageMessages { .. },
            OperationOptions::TextGeneration { .. },
            OutputFormat::Text,
        ) => {
            body["messages"] = super::image_input::provider_messages(&request.input)?;
        }
        (
            Capability::TextGeneration,
            OperationInput::Text { text },
            OperationOptions::TextGeneration { .. },
            OutputFormat::Text,
        ) if !text.trim().is_empty() => body["prompt"] = json!(text),
        (
            Capability::TextEmbedding,
            OperationInput::Text { text },
            OperationOptions::Embeddings { dimensions },
            OutputFormat::EmbeddingsFloat32,
        ) => {
            validate_embedding(&[text.as_str()], *dimensions)?;
            body["input"] = json!(text);
        }
        (
            Capability::TextEmbedding,
            OperationInput::TextBatch { texts },
            OperationOptions::Embeddings { dimensions },
            OutputFormat::EmbeddingsFloat32,
        ) => {
            validate_embedding(
                &texts.iter().map(String::as_str).collect::<Vec<_>>(),
                *dimensions,
            )?;
            body["input"] = json!(texts);
        }
        (
            Capability::ImageGeneration,
            OperationInput::Text { text },
            OperationOptions::ImageGeneration {
                width,
                height,
                seed,
            },
            OutputFormat::PngBase64,
        ) => {
            body["prompt"] = json!(text);
            body["width"] = json!(width);
            body["height"] = json!(height);
            body["seed"] = json!(seed);
            super::super::openai_gateway_images::parse(&body)
                .map_err(|_| ErrorCode::InvalidRequest)?;
        }
        (
            Capability::AudioTranscription | Capability::AudioClassification,
            OperationInput::Audio {
                encoding,
                sample_rate_hz,
                channels,
                sample_count,
                data_base64,
            },
            OperationOptions::Audio {
                language,
                max_output_tokens,
            },
            output,
        ) => {
            if !matches!(
                (request.capability, output),
                (Capability::AudioTranscription, OutputFormat::Text)
                    | (Capability::AudioClassification, OutputFormat::Labels)
            ) {
                return Err(ErrorCode::InvalidRequest);
            }
            let bytes_per_sample = match encoding {
                AudioEncoding::PcmS16le => 2,
                AudioEncoding::PcmF32le => 4,
            };
            let declared_bytes = sample_count
                .checked_mul(u64::from(*channels))
                .and_then(|n| n.checked_mul(bytes_per_sample));
            if !(8000..=192000).contains(sample_rate_hz)
                || !matches!(channels, 1 | 2)
                || *sample_count == 0
                || *sample_count > u64::from(*sample_rate_hz) * 30
                || data_base64.is_empty()
                || declared_bytes.is_none_or(|n| n > MAX_BYTES as u64)
                || *max_output_tokens == Some(0)
            {
                return Err(ErrorCode::InvalidRequest);
            }
            // Byte decoding and normalization belong to the owned native bridge.
            // Returning this closed shape grants no runtime or model authority.
            let _declared_language = language;
            return serde_json::to_value(request).map_err(|_| ErrorCode::InvalidRequest);
        }
        _ => return Err(ErrorCode::InvalidRequest),
    }
    match request.options {
        OperationOptions::TextGeneration {
            max_tokens,
            temperature,
            top_p,
        } => {
            if max_tokens == Some(0)
                || temperature.is_some_and(|v| !v.is_finite() || !(0.0..=2.0).contains(&v))
                || top_p.is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            {
                return Err(ErrorCode::InvalidRequest);
            }
            body["stream"] = json!(request.stream);
            if let Some(v) = max_tokens
                .or_else(|| (request.capability == Capability::ImageToText).then_some(512))
            {
                body["max_tokens"] = json!(v);
            }
            if let Some(v) = temperature {
                body["temperature"] = json!(v);
            }
            if let Some(v) = top_p {
                body["top_p"] = json!(v);
            }
            body["n"] = json!(1);
        }
        OperationOptions::Embeddings { dimensions } => {
            body["encoding_format"] = json!("float");
            if let Some(v) = dimensions {
                body["dimensions"] = json!(v);
            }
        }
        _ => {}
    }
    Ok(body)
}
/// Validate the finite vision contract without decoding image bytes. Resolver
/// selection uses this; the handler performs the bounded image projection once.
pub fn validate_image_to_text_request(request: &OperationRequest) -> Result<(), ErrorCode> {
    if request.capability != Capability::ImageToText
        || request.stream
        || request.output != OutputFormat::Text
        || !matches!(
            request.input,
            OperationInput::Image { .. } | OperationInput::ImageMessages { .. }
        )
    {
        return Err(ErrorCode::InvalidRequest);
    }
    match request.options {
        OperationOptions::TextGeneration {
            max_tokens,
            temperature,
            top_p,
        } if max_tokens.is_none_or(|v| (1..=2048).contains(&v))
            && temperature.is_none_or(|v| v.is_finite() && (0.0..=2.0).contains(&v))
            && top_p.is_none_or(|v| v.is_finite() && (0.0..=1.0).contains(&v)) =>
        {
            Ok(())
        }
        _ => Err(ErrorCode::InvalidRequest),
    }
}
pub fn valid_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}
pub fn valid_model(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn validate_embedding(input: &[&str], dimensions: Option<usize>) -> Result<(), ErrorCode> {
    // Reuse the native adapter's exact input and dimension bounds for every typed embedding provider.
    pumas_library::OnnxEmbeddingRequest::parse(
        "typed-validation",
        input.iter().map(|v| v.to_string()).collect(),
        dimensions,
    )
    .map(|_| ())
    .map_err(|_| ErrorCode::InvalidRequest)
}

/// Extract only the public typed fields, validating coherence instead of forwarding provider JSON.
pub fn result(request: &OperationRequest, value: Value) -> Result<OperationResult, ErrorCode> {
    let invalid = ErrorCode::InvalidProviderResult;
    if value.get("error").is_some() {
        return Err(ErrorCode::ProviderFailure);
    }
    match request.capability {
        Capability::ChatGeneration | Capability::TextGeneration | Capability::ImageToText => {
            let choices = value
                .get("choices")
                .and_then(Value::as_array)
                .ok_or(invalid)?;
            if choices.len() != 1 || choices[0].get("index").and_then(Value::as_u64) != Some(0) {
                return Err(invalid);
            }
            let choice = &choices[0];
            let text = if matches!(
                request.capability,
                Capability::ChatGeneration | Capability::ImageToText
            ) {
                let message = choice.get("message").ok_or(invalid)?;
                if message.get("role").and_then(Value::as_str) != Some("assistant")
                    || message
                        .get("tool_calls")
                        .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
                    || message.get("function_call").is_some_and(|v| !v.is_null())
                    || message.get("refusal").is_some_and(|v| !v.is_null())
                {
                    return Err(invalid);
                }
                message
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or(invalid)?
            } else {
                choice.get("text").and_then(Value::as_str).ok_or(invalid)?
            };
            let finish_reason: FinishReason =
                serde_json::from_value(choice.get("finish_reason").cloned().ok_or(invalid)?)
                    .map_err(|_| invalid)?;
            Ok(OperationResult::Text {
                text: text.to_owned(),
                finish_reason,
            })
        }
        Capability::TextEmbedding => {
            let data = value.get("data").and_then(Value::as_array).ok_or(invalid)?;
            let count = match &request.input {
                OperationInput::TextBatch { texts } => texts.len(),
                _ => 1,
            };
            if data.len() != count {
                return Err(invalid);
            }
            let mut vectors = Vec::new();
            let mut width = None;
            for (i, item) in data.iter().enumerate() {
                if item.get("index").and_then(Value::as_u64) != Some(i as u64) {
                    return Err(invalid);
                }
                let vector = item
                    .get("embedding")
                    .and_then(Value::as_array)
                    .ok_or(invalid)?
                    .iter()
                    .map(|v| {
                        v.as_f64()
                            .filter(|v| v.is_finite() && (*v as f32).is_finite())
                            .map(|v| v as f32)
                            .ok_or(invalid)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if vector.is_empty()
                    || vector.len() > 8192
                    || width.is_some_and(|w| w != vector.len())
                {
                    return Err(invalid);
                }
                if let OperationOptions::Embeddings {
                    dimensions: Some(d),
                } = request.options
                {
                    if vector.len() != d {
                        return Err(invalid);
                    }
                }
                width = Some(vector.len());
                vectors.push(vector);
            }
            Ok(OperationResult::Embeddings { vectors })
        }
        Capability::ImageGeneration => {
            let data = value.get("data").and_then(Value::as_array).ok_or(invalid)?;
            if data.len() != 1 {
                return Err(invalid);
            }
            // This envelope is produced solely by the existing Torch adapter, which validates PNG bytes and dimensions.
            let png_base64 = data[0]
                .get("b64_json")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or(invalid)?
                .to_owned();
            let seed = value
                .get("metadata")
                .and_then(|v| v.get("seed"))
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .ok_or(invalid)?;
            Ok(OperationResult::Image { png_base64, seed })
        }
        _ => Err(invalid),
    }
}

#[cfg(test)]
mod image_to_text_projection_tests {
    use super::*;

    fn request() -> OperationRequest {
        OperationRequest {
            contract_version: 1,
            request_id: "vision-1".into(),
            model: "selected-vision".into(),
            profile: None,
            capability: Capability::ImageToText,
            input: OperationInput::Image {
                encoding: ImageEncoding::Png,
                data_base64: "AA==".into(),
            },
            output: OutputFormat::Text,
            options: OperationOptions::TextGeneration {
                max_tokens: None,
                temperature: None,
                top_p: None,
            },
            stream: false,
        }
    }

    #[test]
    fn lightweight_vision_validation_enforces_finite_options_before_decoding() {
        let mut r = request();
        validate_image_to_text_request(&r).unwrap();
        for tokens in [0, 2049, u32::MAX] {
            r.options = OperationOptions::TextGeneration {
                max_tokens: Some(tokens),
                temperature: None,
                top_p: None,
            };
            assert_eq!(provider_request(&r), Err(ErrorCode::InvalidRequest));
        }
        r = request();
        r.stream = true;
        assert_eq!(provider_request(&r), Err(ErrorCode::InvalidRequest));
        r = request();
        r.input = OperationInput::Text {
            text: "pretend image".into(),
        };
        assert_eq!(provider_request(&r), Err(ErrorCode::InvalidRequest));
        // The new vision token cap does not narrow existing completion options.
        r.capability = Capability::TextGeneration;
        r.options = OperationOptions::TextGeneration {
            max_tokens: Some(2049),
            temperature: None,
            top_p: None,
        };
        assert_eq!(provider_request(&r).unwrap()["max_tokens"], 2049);
    }

    #[test]
    fn vision_result_projects_finite_assistant_text_and_discards_private_fields() {
        let payload = json!({"choices":[{"index":0,"message":{"role":"assistant","content":"a small red square","private":"hidden"},"finish_reason":"stop"}],"model":"private-provider-id","usage":{"private":true}});
        assert_eq!(
            result(&request(), payload).unwrap(),
            OperationResult::Text {
                text: "a small red square".into(),
                finish_reason: FinishReason::Stop
            }
        );
    }

    #[test]
    fn vision_result_rejects_tools_refusal_wrong_roles_and_open_finish_reasons() {
        for message in [
            json!({"role":"user","content":"caption"}),
            json!({"role":"assistant","content":"caption","tool_calls":[{"id":"tool"}]}),
            json!({"role":"assistant","content":"caption","function_call":{}}),
            json!({"role":"assistant","content":"caption","refusal":"refused"}),
            json!({"role":"assistant","content":[{"type":"text","text":"caption"}]}),
        ] {
            assert_eq!(
                result(
                    &request(),
                    json!({"choices":[{"index":0,"message":message,"finish_reason":"stop"}]})
                ),
                Err(ErrorCode::InvalidProviderResult)
            );
        }
        assert_eq!(
            result(
                &request(),
                json!({"choices":[{"index":0,"message":{"role":"assistant","content":"caption"},"finish_reason":"tool_calls"}]})
            ),
            Err(ErrorCode::InvalidProviderResult)
        );
        assert_eq!(
            result(&request(), json!({"error":{},"choices":[]})),
            Err(ErrorCode::ProviderFailure)
        );
    }
}
