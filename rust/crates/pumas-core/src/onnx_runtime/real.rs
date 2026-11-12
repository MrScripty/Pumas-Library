use std::path::PathBuf;

use ort::{
    ep::{CoreML, CPU, CUDA},
    inputs,
    session::{
        builder::{GraphOptimizationLevel, SessionBuilder},
        Session,
    },
    value::TensorRef,
};
use tracing::info;

use super::{
    output::extract_hidden_states, tensors::TokenTensors, OnnxEmbedding,
    OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor, OnnxEmbeddingRequest,
    OnnxEmbeddingResponse, OnnxEmbeddingUsage, OnnxExecutionProvider, OnnxLoadOptions,
    OnnxLoadRequest, OnnxModelConfig, OnnxModelId, OnnxRuntimeError, OnnxRuntimeLibrary,
    OnnxSessionState, OnnxSessionStatus, OnnxTokenizer,
};

const ORT_NATIVE_LIBRARY_STRATEGY: &str = "installed-runtime-library";
const ONNX_RUNTIME_GRAPH_OPTIMIZATION: GraphOptimizationLevel = GraphOptimizationLevel::Level3;
const ONNX_RUNTIME_INTRA_THREADS: usize = 1;
const ONNX_RUNTIME_INTER_THREADS: usize = 1;

/// Register exactly the requested execution provider. A GPU provider that
/// cannot initialize (its provider library or the CUDA/cuDNN libraries it
/// needs are missing) is an error with the reason, never a silent CPU fallback.
fn register_execution_provider(
    builder: SessionBuilder,
    provider: OnnxExecutionProvider,
) -> Result<SessionBuilder, OnnxRuntimeError> {
    let dispatch = match provider {
        OnnxExecutionProvider::Cpu => CPU::default().build(),
        OnnxExecutionProvider::Cuda => CUDA::default().build(),
        OnnxExecutionProvider::CoreMl => CoreML::default().build(),
    }
    .error_on_failure();
    builder
        .with_execution_providers([dispatch])
        .map_err(|error| {
            let hint = match provider {
                OnnxExecutionProvider::Cpu => "",
                OnnxExecutionProvider::Cuda => {
                    "; the CUDA build needs an NVIDIA driver plus the CUDA runtime and cuDNN \
                     libraries of its CUDA major version on the library path"
                }
                OnnxExecutionProvider::CoreMl => "; CoreML needs macOS on Apple hardware",
            };
            OnnxRuntimeError {
                code: super::OnnxRuntimeErrorCode::Backend,
                field: Some("execution_provider".into()),
                message: format!(
                    "ONNX Runtime {} execution provider could not initialize: {error}{hint}",
                    provider.as_str()
                ),
            }
        })
}

/// Load `runtime_library` and initialize `provider` without a model, so an
/// ONNX host can report at startup why it cannot run instead of failing on
/// the first model load.
pub fn probe_execution_provider(
    runtime_library: &OnnxRuntimeLibrary,
    provider: OnnxExecutionProvider,
) -> Result<(), OnnxRuntimeError> {
    super::native_library::ensure_runtime_library(Some(runtime_library))?;
    let builder = Session::builder()
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime session builder failed"))?;
    register_execution_provider(builder, provider).map(|_| ())
}

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
    /// Load without a selected installed version. Only the developer
    /// override `ORT_DYLIB_PATH` can serve.
    pub fn load(request: OnnxLoadRequest) -> Result<Self, OnnxRuntimeError> {
        Self::load_with_runtime_library(request, None)
    }

    /// Load with the library of the host-selected installed ONNX Runtime
    /// version. The selection is host authority, never request-supplied.
    pub fn load_with_runtime_library(
        request: OnnxLoadRequest,
        runtime_library: Option<&OnnxRuntimeLibrary>,
    ) -> Result<Self, OnnxRuntimeError> {
        let tokenizer = OnnxTokenizer::from_model_path(&request.model_path)?;
        let model_config = OnnxModelConfig::from_model_path(&request.model_path)?;
        let embedding_dimensions = resolve_embedding_dimensions(&request.options, &model_config)?;
        super::native_library::ensure_runtime_library(runtime_library)?;
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
        let builder = Session::builder()
            .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime session builder failed"))?;
        let builder = register_execution_provider(builder, request.options.execution_provider)?;
        let mut builder = apply_session_options(builder, &request.options)?;

        let session = builder
            .commit_from_file(request.model_path.path())
            .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime model load failed"))?;
        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();
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
        let tokenized = self
            .tokenizer
            .tokenize_request_within(&request, self.model_config.input_token_limit())?;
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

        let outputs = self.session.run(inputs).map_err(|_| {
            OnnxRuntimeError::backend(format!(
                "ONNX Runtime inference failed for {} inputs of up to {} tokens",
                tensors.batch_size, tensors.sequence_len
            ))
        })?;
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
        .with_optimization_level(ONNX_RUNTIME_GRAPH_OPTIMIZATION)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime optimization setup failed"))?;
    let builder = builder
        .with_intra_threads(ONNX_RUNTIME_INTRA_THREADS)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime intra-thread setup failed"))?;
    builder
        .with_inter_threads(ONNX_RUNTIME_INTER_THREADS)
        .map_err(|_| OnnxRuntimeError::backend("ONNX Runtime inter-thread setup failed"))
}
