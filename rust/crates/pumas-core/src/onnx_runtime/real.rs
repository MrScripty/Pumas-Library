use std::path::PathBuf;

use ort::{
    ep::CPU,
    inputs,
    session::{builder::GraphOptimizationLevel, Session},
    value::TensorRef,
};

use super::{
    output::extract_hidden_states, tensors::TokenTensors, OnnxEmbedding,
    OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor, OnnxEmbeddingRequest,
    OnnxEmbeddingResponse, OnnxEmbeddingUsage, OnnxExecutionProvider, OnnxLoadOptions,
    OnnxLoadRequest, OnnxModelConfig, OnnxModelId, OnnxRuntimeError, OnnxSessionState,
    OnnxSessionStatus, OnnxTokenizer,
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
