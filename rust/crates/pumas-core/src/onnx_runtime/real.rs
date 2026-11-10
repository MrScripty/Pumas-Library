use std::path::PathBuf;

use ort::{
    ep::CPU,
    inputs,
    session::{builder::GraphOptimizationLevel, Session},
    value::TensorRef,
};
use tracing::info;

use super::{
    output::{extract_hidden_states, extract_scores},
    tensors::TokenTensors,
    OnnxEmbedding, OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor,
    OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxEmbeddingUsage, OnnxExecutionProvider,
    OnnxLoadOptions, OnnxLoadRequest, OnnxModelConfig, OnnxModelId, OnnxRerankRequest,
    OnnxRerankResponse, OnnxRerankScore, OnnxRuntimeError, OnnxSessionState, OnnxSessionStatus,
    OnnxSessionTask, OnnxTokenizedBatch, OnnxTokenizer,
};

/// Graph inputs this adapter can supply; a graph that requires any other
/// input (for example a key/value cache) is refused at load.
const SUPPORTED_INPUTS: &[&str] = &[
    "input_ids",
    "attention_mask",
    "token_type_ids",
    "position_ids",
];

const ORT_NATIVE_LIBRARY_STRATEGY: &str = "explicit-runtime-library-cpu";
const ONNX_RUNTIME_GRAPH_OPTIMIZATION: GraphOptimizationLevel = GraphOptimizationLevel::Level3;
const ONNX_RUNTIME_INTRA_THREADS: usize = 1;
const ONNX_RUNTIME_INTER_THREADS: usize = 1;

#[derive(Debug)]
pub struct OnnxRuntimeSession {
    model_id: OnnxModelId,
    model_path: PathBuf,
    execution_provider: OnnxExecutionProvider,
    embedding_dimensions: usize,
    task: OnnxSessionTask,
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
        super::native_library::ensure_runtime_library()?;
        info!(
            provider = "onnx_runtime",
            model_id = %request.model_id.as_str(),
            execution_provider = %request.options.execution_provider.as_str(),
            native_library_strategy = ORT_NATIVE_LIBRARY_STRATEGY,
            graph_optimization = "level3",
            intra_threads = ONNX_RUNTIME_INTRA_THREADS,
            inter_threads = ONNX_RUNTIME_INTER_THREADS,
            "initializing ONNX Runtime embedding session"
        );
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
        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();
        let unsupported: Vec<&str> = input_names
            .iter()
            .map(String::as_str)
            .filter(|name| !SUPPORTED_INPUTS.contains(name))
            .collect();
        if !unsupported.is_empty() || !input_names.iter().any(|name| name == "input_ids") {
            return Err(OnnxRuntimeError::validation(
                "model",
                format!(
                    "the ONNX graph requires inputs this adapter cannot supply ({}); supported \
                     inputs are {}",
                    if unsupported.is_empty() {
                        "no input_ids".to_string()
                    } else {
                        unsupported.join(", ")
                    },
                    SUPPORTED_INPUTS.join(", ")
                ),
            ));
        }
        let output_names: Vec<String> = session
            .outputs()
            .iter()
            .map(|output| output.name().to_string())
            .collect();
        info!(
            provider = "onnx_runtime",
            model_id = %request.model_id.as_str(),
            execution_provider = %request.options.execution_provider.as_str(),
            native_library_strategy = ORT_NATIVE_LIBRARY_STRATEGY,
            input_count = input_names.len(),
            output_count = output_names.len(),
            embedding_dimensions,
            "loaded ONNX Runtime embedding session"
        );

        Ok(Self {
            model_id: request.model_id,
            model_path: request.model_path.path().to_path_buf(),
            execution_provider: request.options.execution_provider,
            embedding_dimensions,
            task: request.options.task,
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
            task: self.task,
            max_input_tokens: self.max_input_tokens(),
        }
    }

    /// The per-input token limit derived from the model config, if declared.
    pub fn max_input_tokens(&self) -> Option<usize> {
        self.model_config
            .input_token_limit()
            .map(|limit| limit.max_tokens)
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
        let OnnxSessionTask::Embedding { pooling, normalize } = self.task else {
            return Err(OnnxRuntimeError::validation(
                "model",
                "this ONNX session is a cross-encoder; it scores rerank pairs, not embeddings",
            ));
        };
        let tokenized = self
            .tokenizer
            .tokenize_request_within(&request, self.model_config.input_token_limit())?;
        let tensors =
            TokenTensors::from_tokenized_batch(&tokenized, self.model_config.pad_token_id())?;
        let postprocess_config = match pooling {
            Some(pooling) => OnnxEmbeddingPostprocessConfig::declared(pooling, normalize),
            None => OnnxEmbeddingPostprocessConfig::mean_pool_l2(),
        };
        let embedding_dimensions = self.embedding_dimensions;
        let embeddings = self.run(&tensors, |outputs| {
            let hidden_states = extract_hidden_states(
                outputs,
                &postprocess_config.output_tensor,
                tensors.batch_size,
                tensors.sequence_len,
                embedding_dimensions,
            )?;
            OnnxEmbeddingPostprocessor::new(postprocess_config.clone(), embedding_dimensions)?
                .postprocess_hidden_states(&hidden_states, &tokenized, request.dimensions)
        })?;
        let data = embeddings
            .into_iter()
            .enumerate()
            .map(|(index, embedding)| OnnxEmbedding { index, embedding })
            .collect();

        Ok(OnnxEmbeddingResponse {
            model: self.model_id.as_str().to_string(),
            data,
            usage: usage(&tokenized),
        })
    }

    /// Score every `(query, document)` pair with the cross-encoder's logit.
    pub fn rerank(
        &mut self,
        request: OnnxRerankRequest,
    ) -> Result<OnnxRerankResponse, OnnxRuntimeError> {
        if request.model_id != self.model_id {
            return Err(OnnxRuntimeError::not_loaded(&request.model_id));
        }
        if self.task != OnnxSessionTask::Rerank {
            return Err(OnnxRuntimeError::validation(
                "model",
                "this ONNX session produces embeddings, not rerank scores",
            ));
        }
        let tokenized = self.tokenizer.tokenize_pairs_within(
            request.model_id.as_str(),
            &request.query,
            &request.documents,
            self.model_config.input_token_limit(),
        )?;
        let tensors =
            TokenTensors::from_tokenized_batch(&tokenized, self.model_config.pad_token_id())?;
        let scores = self.run(&tensors, |outputs| {
            extract_scores(outputs, tensors.batch_size)
        })?;
        Ok(OnnxRerankResponse {
            model: self.model_id.as_str().to_string(),
            results: scores
                .into_iter()
                .enumerate()
                .map(|(index, relevance_score)| OnnxRerankScore {
                    index,
                    relevance_score,
                })
                .collect(),
            usage: usage(&tokenized),
        })
    }

    /// Run the graph on the batch, supplying each input the graph declares.
    fn run<T>(
        &mut self,
        tensors: &TokenTensors,
        read: impl FnOnce(&ort::session::SessionOutputs<'_>) -> Result<T, OnnxRuntimeError>,
    ) -> Result<T, OnnxRuntimeError> {
        let shape = [tensors.batch_size, tensors.sequence_len];
        let mut inputs = inputs! {
            "input_ids" => tensor(shape, "input_ids", &tensors.input_ids)?,
        };
        for (name, values) in [
            ("attention_mask", &tensors.attention_mask),
            ("token_type_ids", &tensors.token_type_ids),
            ("position_ids", &tensors.position_ids),
        ] {
            if self.has_input(name) {
                inputs.push((name.into(), tensor(shape, name, values)?.into()));
            }
        }
        let outputs = self.session.run(inputs).map_err(|_| {
            OnnxRuntimeError::backend(format!(
                "ONNX Runtime inference failed for {} inputs of up to {} tokens",
                tensors.batch_size, tensors.sequence_len
            ))
        })?;
        read(&outputs)
    }

    fn has_input(&self, name: &str) -> bool {
        self.input_names
            .iter()
            .any(|input_name| input_name.as_str() == name)
    }
}

fn tensor<'a>(
    shape: [usize; 2],
    name: &str,
    values: &'a [i64],
) -> Result<TensorRef<'a, i64>, OnnxRuntimeError> {
    TensorRef::from_array_view((shape, values))
        .map_err(|_| OnnxRuntimeError::backend(format!("ONNX {name} tensor construction failed")))
}

fn usage(tokenized: &OnnxTokenizedBatch) -> OnnxEmbeddingUsage {
    OnnxEmbeddingUsage {
        prompt_tokens: tokenized.total_tokens,
        total_tokens: tokenized.total_tokens,
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
        .with_optimization_level(ONNX_RUNTIME_GRAPH_OPTIMIZATION)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime optimization setup failed"))?;
    let builder = builder
        .with_intra_threads(ONNX_RUNTIME_INTRA_THREADS)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime intra-thread setup failed"))?;
    builder
        .with_inter_threads(ONNX_RUNTIME_INTER_THREADS)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime inter-thread setup failed"))
}
