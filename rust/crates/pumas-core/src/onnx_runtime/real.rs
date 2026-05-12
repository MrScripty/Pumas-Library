use std::path::PathBuf;

use ort::{
    ep::CPU,
    session::{builder::GraphOptimizationLevel, Session},
};

use super::{
    OnnxExecutionProvider, OnnxLoadOptions, OnnxLoadRequest, OnnxModelConfig, OnnxModelId,
    OnnxRuntimeError, OnnxSessionState, OnnxSessionStatus, OnnxTokenizer,
};

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
