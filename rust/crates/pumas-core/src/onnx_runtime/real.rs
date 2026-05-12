use std::path::PathBuf;

use half::{bf16, f16};
use ort::{
    ep::CPU,
    inputs,
    session::{builder::GraphOptimizationLevel, Session},
    value::{Shape, TensorElementType, TensorRef, ValueRef, ValueType},
};

use super::{
    OnnxEmbedding, OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor,
    OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxEmbeddingUsage, OnnxExecutionProvider,
    OnnxLoadOptions, OnnxLoadRequest, OnnxModelConfig, OnnxModelId, OnnxOutputTensorSelection,
    OnnxRuntimeError, OnnxSessionState, OnnxSessionStatus, OnnxTokenizedBatch, OnnxTokenizer,
};

#[derive(Debug)]
pub struct OnnxRuntimeSession {
    model_id: OnnxModelId,
    model_path: PathBuf,
    execution_provider: OnnxExecutionProvider,
    embedding_dimensions: usize,
    model_config: OnnxModelConfig,
    tokenizer: OnnxTokenizer,
    input_names: Vec<String>,
    output_names: Vec<String>,
    session: Session,
}

impl OnnxRuntimeSession {
    pub fn load(request: OnnxLoadRequest) -> Result<Self, OnnxRuntimeError> {
        let tokenizer = OnnxTokenizer::from_model_path(&request.model_path)?;
        let model_config = OnnxModelConfig::from_model_path(&request.model_path)?;
        let embedding_dimensions = resolve_embedding_dimensions(&request.options, &model_config)?;
        let mut builder = Session::builder()
            .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime session builder failed"))?;
        builder = match request.options.execution_provider {
            OnnxExecutionProvider::Cpu => builder
                .with_execution_providers([CPU::default().build()])
                .map_err(|_| {
                    OnnxRuntimeError::backend("ONNX Runtime CPU execution provider setup failed")
                })?,
        };
        builder = apply_session_options(builder, &request.options)?;

        let session = builder
            .commit_from_file(request.model_path.path())
            .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime model load failed"))?;
        let input_names = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();
        let output_names = session
            .outputs()
            .iter()
            .map(|output| output.name().to_string())
            .collect();

        Ok(Self {
            model_id: request.model_id,
            model_path: request.model_path.path().to_path_buf(),
            execution_provider: request.options.execution_provider,
            embedding_dimensions,
            model_config,
            tokenizer,
            input_names,
            output_names,
            session,
        })
    }

    pub fn status(&self) -> OnnxSessionStatus {
        OnnxSessionStatus {
            model_id: self.model_id.clone(),
            model_path: self.model_path.clone(),
            execution_provider: self.execution_provider,
            embedding_dimensions: self.embedding_dimensions,
            state: OnnxSessionState::Loaded,
        }
    }

    pub fn tokenizer(&self) -> &OnnxTokenizer {
        &self.tokenizer
    }

    pub fn model_config(&self) -> &OnnxModelConfig {
        &self.model_config
    }

    pub fn input_names(&self) -> &[String] {
        &self.input_names
    }

    pub fn output_names(&self) -> &[String] {
        &self.output_names
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn embed(
        &mut self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        if request.model_id != self.model_id {
            return Err(OnnxRuntimeError::not_loaded(&request.model_id));
        }
        let tokenized = self.tokenizer.tokenize_request(&request)?;
        let tensors = TokenTensors::from_tokenized_batch(&tokenized)?;
        let input_ids = TensorRef::from_array_view((
            [tensors.batch_size, tensors.sequence_len],
            tensors.input_ids.as_slice(),
        ))
        .map_err(|_| OnnxRuntimeError::backend("ONNX input_ids tensor construction failed"))?;
        let attention_mask = TensorRef::from_array_view((
            [tensors.batch_size, tensors.sequence_len],
            tensors.attention_mask.as_slice(),
        ))
        .map_err(|_| OnnxRuntimeError::backend("ONNX attention_mask tensor construction failed"))?;
        let token_type_ids = if self.has_input("token_type_ids") {
            Some(
                TensorRef::from_array_view((
                    [tensors.batch_size, tensors.sequence_len],
                    tensors.token_type_ids.as_slice(),
                ))
                .map_err(|_| {
                    OnnxRuntimeError::backend("ONNX token_type_ids tensor construction failed")
                })?,
            )
        } else {
            None
        };

        let mut inputs = inputs! {
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
        };
        if let Some(token_type_ids) = token_type_ids {
            inputs.push(("token_type_ids".into(), token_type_ids.into()));
        }

        let outputs = self
            .session
            .run(inputs)
            .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime inference failed"))?;
        let postprocess_config = OnnxEmbeddingPostprocessConfig::mean_pool_l2();
        let hidden_states = extract_hidden_states(
            &outputs,
            &postprocess_config.output_tensor,
            tensors.batch_size,
            tensors.sequence_len,
            self.embedding_dimensions,
        )?;
        let postprocessor =
            OnnxEmbeddingPostprocessor::new(postprocess_config, self.embedding_dimensions)?;
        let embeddings = postprocessor.postprocess_hidden_states(
            &hidden_states,
            &tokenized,
            request.dimensions,
        )?;
        let data = embeddings
            .into_iter()
            .enumerate()
            .map(|(index, embedding)| OnnxEmbedding { index, embedding })
            .collect();

        Ok(OnnxEmbeddingResponse {
            model: self.model_id.as_str().to_string(),
            data,
            usage: OnnxEmbeddingUsage {
                prompt_tokens: tokenized.total_tokens,
                total_tokens: tokenized.total_tokens,
            },
        })
    }

    fn has_input(&self, name: &str) -> bool {
        self.input_names
            .iter()
            .any(|input_name| input_name.as_str() == name)
    }
}

fn resolve_embedding_dimensions(
    options: &OnnxLoadOptions,
    model_config: &OnnxModelConfig,
) -> Result<usize, OnnxRuntimeError> {
    let config_dimensions = model_config.embedding_dimensions();
    let Some(requested_dimensions) = options.embedding_dimensions else {
        return Ok(config_dimensions);
    };
    if requested_dimensions != config_dimensions {
        return Err(OnnxRuntimeError::validation(
            "embedding_dimensions",
            format!(
                "embedding dimensions must match config.json value {config_dimensions} for real ONNX sessions"
            ),
        ));
    }
    Ok(requested_dimensions)
}

struct TokenTensors {
    input_ids: Vec<i64>,
    attention_mask: Vec<i64>,
    token_type_ids: Vec<i64>,
    batch_size: usize,
    sequence_len: usize,
}

impl TokenTensors {
    fn from_tokenized_batch(tokenized: &OnnxTokenizedBatch) -> Result<Self, OnnxRuntimeError> {
        let batch_size = tokenized.inputs.len();
        if batch_size == 0 {
            return Err(OnnxRuntimeError::validation(
                "input",
                "tokenized embedding batch must contain at least one input",
            ));
        }
        let sequence_len = tokenized
            .inputs
            .iter()
            .map(|input| input.token_count)
            .max()
            .ok_or_else(|| OnnxRuntimeError::backend("ONNX tokenized batch has no inputs"))?;
        let value_count = batch_size
            .checked_mul(sequence_len)
            .ok_or_else(|| OnnxRuntimeError::backend("ONNX input tensor size overflow"))?;
        let mut input_ids = Vec::with_capacity(value_count);
        let mut attention_mask = Vec::with_capacity(value_count);
        let mut token_type_ids = Vec::with_capacity(value_count);

        for input in &tokenized.inputs {
            if input.input_ids.len() != input.token_count
                || input.attention_mask.len() != input.token_count
            {
                return Err(OnnxRuntimeError::backend(
                    "ONNX tokenized input length mismatch",
                ));
            }
            input_ids.extend_from_slice(&input.input_ids);
            attention_mask.extend_from_slice(&input.attention_mask);
            token_type_ids.extend(std::iter::repeat_n(0, input.token_count));
            let padding = sequence_len.checked_sub(input.token_count).ok_or_else(|| {
                OnnxRuntimeError::backend("ONNX tokenized input padding underflow")
            })?;
            input_ids.extend(std::iter::repeat_n(0, padding));
            attention_mask.extend(std::iter::repeat_n(0, padding));
            token_type_ids.extend(std::iter::repeat_n(0, padding));
        }

        Ok(Self {
            input_ids,
            attention_mask,
            token_type_ids,
            batch_size,
            sequence_len,
        })
    }
}

fn extract_hidden_states(
    outputs: &ort::session::SessionOutputs<'_>,
    selection: &OnnxOutputTensorSelection,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<Vec<Vec<Vec<f32>>>, OnnxRuntimeError> {
    match selection {
        OnnxOutputTensorSelection::Named(name) => {
            let value = outputs.get(name).ok_or_else(|| {
                OnnxRuntimeError::validation(
                    "output",
                    "configured ONNX output tensor was not found",
                )
            })?;
            extract_hidden_states_value(
                value.view(),
                name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        OnnxOutputTensorSelection::FirstFloatingTensor => {
            for (name, value) in outputs {
                if is_floating_tensor(value.dtype()) {
                    return extract_hidden_states_value(
                        value,
                        name,
                        expected_batch,
                        expected_tokens,
                        expected_dimensions,
                    );
                }
            }
            Err(OnnxRuntimeError::validation(
                "output",
                "ONNX Runtime did not return a floating tensor output",
            ))
        }
    }
}

fn is_floating_tensor(value_type: &ValueType) -> bool {
    matches!(
        value_type,
        ValueType::Tensor {
            ty: TensorElementType::Float32
                | TensorElementType::Float16
                | TensorElementType::Bfloat16,
            ..
        }
    )
}

fn extract_hidden_states_value(
    value: ValueRef<'_>,
    output_name: &str,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<Vec<Vec<Vec<f32>>>, OnnxRuntimeError> {
    match value.dtype() {
        ValueType::Tensor {
            ty: TensorElementType::Float32,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f32>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX f32 output tensor extraction failed")
            })?;
            tensor_values_to_hidden_states(
                shape,
                values.iter().copied().collect(),
                output_name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        ValueType::Tensor {
            ty: TensorElementType::Float16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f16>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX f16 output tensor extraction failed")
            })?;
            tensor_values_to_hidden_states(
                shape,
                values.iter().map(|value| value.to_f32()).collect(),
                output_name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        ValueType::Tensor {
            ty: TensorElementType::Bfloat16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<bf16>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX bf16 output tensor extraction failed")
            })?;
            tensor_values_to_hidden_states(
                shape,
                values.iter().map(|value| value.to_f32()).collect(),
                output_name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        _ => Err(OnnxRuntimeError::validation(
            "output",
            format!("ONNX output tensor '{output_name}' is not a supported floating tensor"),
        )),
    }
}

fn tensor_values_to_hidden_states(
    shape: &Shape,
    values: Vec<f32>,
    output_name: &str,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<Vec<Vec<Vec<f32>>>, OnnxRuntimeError> {
    if shape.len() != 3 {
        return Err(OnnxRuntimeError::validation(
            "output",
            format!(
                "ONNX output tensor '{output_name}' must have shape [batch, tokens, dimensions]"
            ),
        ));
    }
    let batch = shape_dimension(shape[0], "batch")?;
    let tokens = shape_dimension(shape[1], "tokens")?;
    let dimensions = shape_dimension(shape[2], "dimensions")?;
    if batch != expected_batch || tokens != expected_tokens || dimensions != expected_dimensions {
        return Err(OnnxRuntimeError::validation(
            "output",
            "ONNX output tensor shape does not match tokenized batch and configured dimensions",
        ));
    }
    let expected_values = expected_batch
        .checked_mul(expected_tokens)
        .and_then(|value| value.checked_mul(expected_dimensions))
        .ok_or_else(|| OnnxRuntimeError::backend("ONNX output tensor size overflow"))?;
    if values.len() != expected_values {
        return Err(OnnxRuntimeError::validation(
            "output",
            "ONNX output tensor value count does not match tensor shape",
        ));
    }

    let mut offset = 0usize;
    let mut hidden_states = Vec::with_capacity(expected_batch);
    for _ in 0..expected_batch {
        let mut token_embeddings = Vec::with_capacity(expected_tokens);
        for _ in 0..expected_tokens {
            let next = offset + expected_dimensions;
            token_embeddings.push(values[offset..next].to_vec());
            offset = next;
        }
        hidden_states.push(token_embeddings);
    }
    Ok(hidden_states)
}

fn shape_dimension(value: i64, name: &'static str) -> Result<usize, OnnxRuntimeError> {
    usize::try_from(value).map_err(|_| {
        OnnxRuntimeError::validation("output", format!("ONNX output {name} dimension is invalid"))
    })
}

fn apply_session_options(
    builder: ort::session::builder::SessionBuilder,
    _options: &OnnxLoadOptions,
) -> Result<ort::session::builder::SessionBuilder, OnnxRuntimeError> {
    let builder = builder
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime optimization setup failed"))?;
    let builder = builder
        .with_intra_threads(1)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime intra-thread setup failed"))?;
    builder
        .with_inter_threads(1)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime inter-thread setup failed"))
}
