use super::types::*;
use serde_json::{json, Value};

/// Closed typed admission precedes every provider-facing effect.
pub fn provider_request(request: &OperationRequest) -> Result<Value, Rejection> {
    if request.contract_version != CONTRACT_VERSION {
        return Err(ErrorCode::UnsupportedContract.into());
    }
    if !valid_identifier(&request.request_id, 128) {
        return Err(Rejection::invalid(
            "request_id must be 1-128 ASCII letters, digits or '-', '_', '.', ':'",
        ));
    }
    if !valid_model(&request.model) {
        return Err(Rejection::invalid("model is not a valid served alias"));
    }
    if let Some(profile) = &request.profile {
        valid_profile(profile)?;
    }
    if request.stream && !request.capability.text_generation() {
        return Err(Rejection::invalid(
            "stream is only supported for chat_generation and text_generation",
        ));
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
            if messages.is_empty() {
                return Err(Rejection::invalid("input.messages must not be empty"));
            }
            if let Some(index) = messages.iter().position(|m| m.content.trim().is_empty()) {
                return Err(Rejection::invalid(format!(
                    "input.messages[{index}].content must not be blank"
                )));
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
            Capability::Decision,
            OperationInput::Decision {
                state,
                questions,
                images,
            },
            OperationOptions::Decision {},
            OutputFormat::Decisions,
        ) => {
            body["state"] = state.clone();
            body["questions"] = json!(questions);
            if !images.is_empty() {
                body["images"] = json!(super::image_input::decision_images(images)?);
            }
        }
        (
            Capability::Rerank,
            OperationInput::Rerank { query, documents },
            OperationOptions::Rerank { top_n },
            OutputFormat::RelevanceScores,
        ) => {
            if query.trim().is_empty() {
                return Err(Rejection::invalid("input.query must not be blank"));
            }
            if documents.is_empty() {
                return Err(Rejection::invalid("input.documents must not be empty"));
            }
            if let Some(index) = documents.iter().position(|d| d.trim().is_empty()) {
                return Err(Rejection::invalid(format!(
                    "input.documents[{index}] must not be blank"
                )));
            }
            if *top_n == Some(0) {
                return Err(Rejection::invalid("options.top_n must be at least 1"));
            }
            body["query"] = json!(query);
            body["documents"] = json!(documents);
            // Always ask for every score; `top_n` is applied to the checked result.
            body["top_n"] = json!(documents.len());
        }
        (
            Capability::MultimodalEmbedding,
            OperationInput::Image { .. } | OperationInput::ContentBatch { .. },
            OperationOptions::Embeddings { dimensions },
            OutputFormat::EmbeddingsFloat32,
        ) => {
            if *dimensions == Some(0) {
                return Err(Rejection::invalid("options.dimensions must be at least 1"));
            }
            body["input"] = super::image_input::embedding_items(&request.input)?;
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
            super::super::openai_gateway_images::parse(&body).map_err(Rejection::invalid)?;
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
                return Err(Rejection::invalid(
                    "audio_transcription produces text and audio_classification produces labels",
                ));
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
            {
                return Err(Rejection::invalid(
                    "audio input requires 8000-192000 Hz, 1 or 2 channels, 1 to 30 seconds of samples and nonempty data",
                ));
            }
            if max_output_tokens.is_some_and(|tokens| tokens != super::AUDIO_MAX_OUTPUT_TOKENS) {
                return Err(Rejection::invalid(format!(
                    "options.max_output_tokens must be omitted or {}: the transcription adapter generates up to that fixed bound",
                    super::AUDIO_MAX_OUTPUT_TOKENS
                )));
            }
            // Byte decoding and normalization belong to the owned native bridge.
            // Returning this closed shape grants no runtime or model authority.
            let _declared_language = language;
            return serde_json::to_value(request)
                .map_err(|_| Rejection::invalid("audio request could not be encoded"));
        }
        (Capability::TextGeneration, OperationInput::Text { .. }, _, OutputFormat::Text) => {
            return Err(Rejection::invalid("input.text must not be blank"));
        }
        (capability, input, options, output) => {
            return Err(Rejection::invalid(format!(
                "{} does not accept input kind {:?}, options kind {:?} and output {:?}",
                serde_json::to_value(capability).unwrap_or_default(),
                input_kind(input),
                options_kind(options),
                serde_json::to_value(output).unwrap_or_default(),
            )));
        }
    }
    match request.options {
        OperationOptions::TextGeneration {
            max_tokens,
            temperature,
            top_p,
        } => {
            if max_tokens == Some(0) {
                return Err(Rejection::invalid("options.max_tokens must be at least 1"));
            }
            if temperature.is_some_and(|v| !v.is_finite() || !(0.0..=2.0).contains(&v)) {
                return Err(Rejection::invalid(
                    "options.temperature must be within 0 to 2",
                ));
            }
            if top_p.is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v)) {
                return Err(Rejection::invalid("options.top_p must be within 0 to 1"));
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
pub fn validate_image_to_text_request(request: &OperationRequest) -> Result<(), Rejection> {
    if request.capability != Capability::ImageToText
        || request.stream
        || request.output != OutputFormat::Text
        || !matches!(
            request.input,
            OperationInput::Image { .. } | OperationInput::ImageMessages { .. }
        )
    {
        return Err(Rejection::invalid(
            "image_to_text accepts image or image_messages input, text output and no stream",
        ));
    }
    match request.options {
        OperationOptions::TextGeneration {
            max_tokens,
            temperature,
            top_p,
        } if max_tokens.is_none_or(|v| v >= 1)
            && temperature.is_none_or(|v| v.is_finite() && (0.0..=2.0).contains(&v))
            && top_p.is_none_or(|v| v.is_finite() && (0.0..=1.0).contains(&v)) =>
        {
            Ok(())
        }
        _ => Err(Rejection::invalid(
            "image_to_text options must be text_generation with max_tokens of at least 1, temperature 0-2 and top_p 0-1",
        )),
    }
}
pub fn valid_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}
pub fn valid_profile(profile: &str) -> Result<(), Rejection> {
    if profile.len() > 128
        || profile.trim() != profile
        || pumas_library::models::RuntimeProfileId::parse(profile).is_err()
    {
        return Err(Rejection::invalid(
            "profile must be an exact runtime profile id of at most 128 bytes",
        ));
    }
    Ok(())
}
fn input_kind(input: &OperationInput) -> &'static str {
    match input {
        OperationInput::Text { .. } => "text",
        OperationInput::Messages { .. } => "messages",
        OperationInput::TextBatch { .. } => "text_batch",
        OperationInput::Image { .. } => "image",
        OperationInput::ImageMessages { .. } => "image_messages",
        OperationInput::Audio { .. } => "audio",
        OperationInput::ContentBatch { .. } => "content_batch",
        OperationInput::Decision { .. } => "decision",
        OperationInput::Rerank { .. } => "rerank",
    }
}
fn options_kind(options: &OperationOptions) -> &'static str {
    match options {
        OperationOptions::TextGeneration { .. } => "text_generation",
        OperationOptions::Embeddings { .. } => "embeddings",
        OperationOptions::ImageGeneration { .. } => "image_generation",
        OperationOptions::Audio { .. } => "audio",
        OperationOptions::Decision { .. } => "decision",
        OperationOptions::Rerank { .. } => "rerank",
    }
}
pub fn valid_model(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= pumas_library::serving::MAX_GATEWAY_ALIAS_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
/// Provider-independent embedding checks. Batch, length and dimension limits
/// belong to the serving provider (the ONNX adapter checks its own).
fn validate_embedding(input: &[&str], dimensions: Option<usize>) -> Result<(), Rejection> {
    if input.is_empty() {
        return Err(Rejection::invalid("input.texts must not be empty"));
    }
    if let Some(index) = input.iter().position(|text| text.trim().is_empty()) {
        return Err(Rejection::invalid(if input.len() == 1 {
            "input.text must not be blank".to_owned()
        } else {
            format!("input.texts[{index}] must not be blank")
        }));
    }
    if dimensions == Some(0) {
        return Err(Rejection::invalid("options.dimensions must be at least 1"));
    }
    Ok(())
}

/// Validate a `/v1/systemone` response: one answer of the asked type for every
/// question, with finite probabilities. Provider extras are not forwarded.
fn decision_result(request: &OperationRequest, value: &Value) -> Option<OperationResult> {
    let OperationInput::Decision { questions, .. } = &request.input else {
        return None;
    };
    let answers = value.get("answers")?.as_object()?;
    if answers.len() != questions.len() {
        return None;
    }
    let probability = |value: &Value| {
        value
            .as_f64()
            .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
    };
    let probabilities = |value: &Value| -> Option<std::collections::BTreeMap<String, f64>> {
        value
            .as_object()?
            .iter()
            .map(|(key, p)| probability(p).map(|p| (key.clone(), p)))
            .collect()
    };
    let mut typed = std::collections::BTreeMap::new();
    for (id, question) in questions {
        let answer = answers.get(id)?;
        let answer = match question.kind {
            DecisionType::Choice => {
                if answer.get("type").and_then(Value::as_str) != Some("choice") {
                    return None;
                }
                DecisionAnswer::Choice {
                    choice: answer.get("choice")?.as_str()?.to_owned(),
                    probabilities: probabilities(answer.get("probabilities")?)?,
                    confidence: probability(answer.get("confidence")?)?,
                }
            }
            DecisionType::Score => {
                if answer.get("type").and_then(Value::as_str) != Some("score") {
                    return None;
                }
                DecisionAnswer::Score {
                    score: answer.get("score")?.as_f64().filter(|s| s.is_finite())?,
                    legend: answer
                        .get("legend")?
                        .as_object()?
                        .iter()
                        .map(|(level, text)| text.as_str().map(|t| (level.clone(), t.to_owned())))
                        .collect::<Option<_>>()?,
                    probabilities: probabilities(answer.get("probabilities")?)?,
                    confidence: probability(answer.get("confidence")?)?,
                }
            }
            DecisionType::Noul => {
                if answer.get("type").and_then(Value::as_str) != Some("noul") {
                    return None;
                }
                DecisionAnswer::Noul {
                    noul: probability(answer.get("noul")?)?,
                }
            }
        };
        typed.insert(id.clone(), answer);
    }
    Some(OperationResult::Decisions {
        answers: typed,
        input_tokens: value.pointer("/usage/input_tokens").and_then(Value::as_u64),
    })
}

/// Validate a `/v1/rerank` response: one finite score for every document,
/// each document exactly once. Results are ordered most relevant first and
/// cut to `top_n`; provider extras such as document text are not forwarded.
fn rerank_result(request: &OperationRequest, value: &Value) -> Option<OperationResult> {
    let (OperationInput::Rerank { documents, .. }, OperationOptions::Rerank { top_n }) =
        (&request.input, &request.options)
    else {
        return None;
    };
    let rows = value.get("results")?.as_array()?;
    if rows.len() != documents.len() {
        return None;
    }
    let mut seen = vec![false; documents.len()];
    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let index = usize::try_from(row.get("index")?.as_u64()?).ok()?;
        if std::mem::replace(seen.get_mut(index)?, true) {
            return None;
        }
        let relevance_score = row
            .get("relevance_score")?
            .as_f64()
            .filter(|score| score.is_finite())?;
        results.push(RelevanceScore {
            index,
            relevance_score,
        });
    }
    // Stable: equal scores keep document order.
    results.sort_by(|a, b| {
        b.relevance_score
            .total_cmp(&a.relevance_score)
            .then(a.index.cmp(&b.index))
    });
    if let Some(top_n) = top_n {
        results.truncate(*top_n);
    }
    Some(OperationResult::RelevanceScores {
        results,
        input_tokens: value
            .pointer("/usage/prompt_tokens")
            .and_then(Value::as_u64),
    })
}

/// Matryoshka truncation for providers that ignore a requested dimension
/// (llama.cpp, Ollama): keep the leading `dimensions` values and renormalize.
/// A provider that already returned `dimensions` values is passed through; a
/// shorter vector than requested cannot be extended.
fn truncate_embedding(mut vector: Vec<f32>, dimensions: usize) -> Option<Vec<f32>> {
    if vector.len() == dimensions {
        return Some(vector);
    }
    if vector.len() < dimensions {
        return None;
    }
    vector.truncate(dimensions);
    let norm = vector
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm == 0.0 {
        return None;
    }
    for value in &mut vector {
        *value = (f64::from(*value) / norm) as f32;
    }
    Some(vector)
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
        Capability::TextEmbedding | Capability::MultimodalEmbedding => {
            let data = value.get("data").and_then(Value::as_array).ok_or(invalid)?;
            let count = match &request.input {
                OperationInput::TextBatch { texts } => texts.len(),
                OperationInput::ContentBatch { items } => items.len(),
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
                // Coherence compares the provider's own widths, before any
                // requested truncation changes them.
                if vector.is_empty() || width.is_some_and(|w| w != vector.len()) {
                    return Err(invalid);
                }
                width = Some(vector.len());
                let vector = match request.options {
                    OperationOptions::Embeddings {
                        dimensions: Some(d),
                    } => truncate_embedding(vector, d).ok_or(invalid)?,
                    _ => vector,
                };
                vectors.push(vector);
            }
            Ok(OperationResult::Embeddings { vectors })
        }
        Capability::Decision => decision_result(request, &value).ok_or(invalid),
        Capability::Rerank => rerank_result(request, &value).ok_or(invalid),
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
        // Only a zero length is invalid; the provider enforces its context window.
        r.options = OperationOptions::TextGeneration {
            max_tokens: Some(0),
            temperature: None,
            top_p: None,
        };
        assert_eq!(
            provider_request(&r).map_err(|rejection| rejection.code),
            Err(ErrorCode::InvalidRequest)
        );
        r.options = OperationOptions::TextGeneration {
            max_tokens: Some(u32::MAX),
            temperature: None,
            top_p: None,
        };
        validate_image_to_text_request(&r).unwrap();
        r = request();
        r.stream = true;
        assert_eq!(
            provider_request(&r).map_err(|rejection| rejection.code),
            Err(ErrorCode::InvalidRequest)
        );
        r = request();
        r.input = OperationInput::Text {
            text: "pretend image".into(),
        };
        assert_eq!(
            provider_request(&r).map_err(|rejection| rejection.code),
            Err(ErrorCode::InvalidRequest)
        );
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
