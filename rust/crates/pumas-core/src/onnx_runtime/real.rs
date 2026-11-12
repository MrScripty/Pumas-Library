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

use crate::onnx_runtime_distribution::OnnxRuntimeVariant;

use super::{
    output::{extract_embedding_output, extract_scores, EmbeddingOutput},
    tensors::TokenTensors,
    OnnxEmbedding, OnnxEmbeddingPostprocessConfig, OnnxEmbeddingPostprocessor,
    OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxEmbeddingUsage, OnnxExecutionProvider,
    OnnxLoadOptions, OnnxLoadRequest, OnnxModelConfig, OnnxModelId, OnnxRerankRequest,
    OnnxRerankResponse, OnnxRerankScore, OnnxRuntimeError, OnnxRuntimeLibrary, OnnxSessionState,
    OnnxSessionStatus, OnnxSessionTask, OnnxTokenizedBatch, OnnxTokenizer,
};

/// Graph inputs this adapter can supply; a graph that requires any other
/// input (for example a key/value cache) is refused at load.
const SUPPORTED_INPUTS: &[&str] = &[
    "input_ids",
    "attention_mask",
    "token_type_ids",
    "position_ids",
];

const ORT_NATIVE_LIBRARY_STRATEGY: &str = "installed-runtime-library";
const ONNX_RUNTIME_GRAPH_OPTIMIZATION: GraphOptimizationLevel = GraphOptimizationLevel::Level3;
const ONNX_RUNTIME_INTRA_THREADS: usize = 1;
const ONNX_RUNTIME_INTER_THREADS: usize = 1;

/// Where the CUDA libraries of the library being loaded come from: the
/// installed version's own, unless a developer's `ORT_DYLIB_PATH` library
/// replaces it.
fn cuda_libraries(
    runtime_library: Option<&OnnxRuntimeLibrary>,
) -> super::cuda_driver::CudaLibraries {
    use super::cuda_driver::CudaLibraries;
    let version_dir = runtime_library.and_then(|library| library.path().parent()?.parent());
    match version_dir {
        _ if std::env::var_os("ORT_DYLIB_PATH").is_some_and(|path| !path.is_empty()) => {
            CudaLibraries::External
        }
        None => CudaLibraries::External,
        Some(version_dir) => {
            if crate::onnx_runtime_distribution::installed_cuda_library_dir(version_dir).is_some() {
                CudaLibraries::Installed
            } else {
                CudaLibraries::NotInstalled
            }
        }
    }
}

/// Register exactly the requested execution provider. A GPU provider that
/// cannot initialize is an error with the reason, never a silent CPU
/// fallback. For CUDA the reason names the missing piece: the NVIDIA driver
/// (missing or too old), or the build's CUDA runtime and cuDNN libraries.
fn register_execution_provider(
    builder: SessionBuilder,
    provider: OnnxExecutionProvider,
    runtime_library: Option<&OnnxRuntimeLibrary>,
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
                OnnxExecutionProvider::Cpu => String::new(),
                OnnxExecutionProvider::Cuda => super::cuda_driver::cuda_failure_hint(
                    super::cuda_driver::probe_driver(),
                    runtime_library
                        .and_then(|library| OnnxRuntimeVariant::from_tag(library.version_tag()))
                        .and_then(OnnxRuntimeVariant::cuda_major),
                    cuda_libraries(runtime_library),
                ),
                OnnxExecutionProvider::CoreMl => "; CoreML needs macOS on Apple hardware".into(),
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
    register_execution_provider(builder, provider, Some(runtime_library)).map(|_| ())
}

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
        let builder = register_execution_provider(
            builder,
            request.options.execution_provider,
            runtime_library,
        )?;
        let mut builder = apply_session_options(builder, &request.options)?;

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
        let OnnxSessionTask::Embedding {
            pooling,
            normalize,
            requires_pooled_output,
        } = self.task
        else {
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
            let output = extract_embedding_output(
                outputs,
                &postprocess_config.output_tensor,
                requires_pooled_output,
                tensors.batch_size,
                tensors.sequence_len,
                embedding_dimensions,
            )?;
            let postprocessor =
                OnnxEmbeddingPostprocessor::new(postprocess_config.clone(), embedding_dimensions)?;
            match output {
                EmbeddingOutput::Tokens(hidden_states) => postprocessor.postprocess_hidden_states(
                    &hidden_states,
                    &tokenized,
                    request.dimensions,
                ),
                EmbeddingOutput::Pooled(pooled) => {
                    postprocessor.postprocess_pooled(pooled, tensors.batch_size, request.dimensions)
                }
            }
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
        let OnnxSessionTask::Rerank { activation } = self.task else {
            return Err(OnnxRuntimeError::validation(
                "model",
                "this ONNX session produces embeddings, not rerank scores",
            ));
        };
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
        let scores = activate_scores(scores, activation)?;
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

/// The graph's logits under the declared activation, as the Torch adapter
/// scores the same model.
fn activate_scores(
    logits: Vec<f32>,
    activation: super::OnnxScoreActivation,
) -> Result<Vec<f32>, OnnxRuntimeError> {
    logits
        .into_iter()
        .map(|logit| {
            let score = activation.apply(logit);
            if score.is_finite() {
                Ok(score)
            } else {
                Err(OnnxRuntimeError::validation(
                    "output",
                    "the model produced a non-finite relevance score",
                ))
            }
        })
        .collect()
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

#[cfg(test)]
mod activation_tests {
    use super::*;
    use crate::onnx_runtime::OnnxScoreActivation;

    #[test]
    fn declared_sigmoid_activates_logits_and_identity_keeps_them() {
        let logits = vec![0.0, 2.0, -2.0];
        assert_eq!(
            activate_scores(logits.clone(), OnnxScoreActivation::Identity).unwrap(),
            logits
        );
        let activated = activate_scores(logits, OnnxScoreActivation::Sigmoid).unwrap();
        assert!((activated[0] - 0.5).abs() < 1e-6);
        assert!((activated[1] - 0.880_797).abs() < 1e-5);
        assert!((activated[2] - 0.119_203).abs() < 1e-5);
    }
}
