use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{package::resolve_package_file, validate_dimensions, OnnxModelPath, OnnxRuntimeError};

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxModelConfig {
    config_path: PathBuf,
    embedding_dimensions: usize,
}

impl OnnxModelConfig {
    pub fn from_model_path(model_path: &OnnxModelPath) -> Result<Self, OnnxRuntimeError> {
        let config_path = resolve_package_file(model_path, CONFIG_FILE_NAME, "config")?;
        let config_text = std::fs::read_to_string(&config_path).map_err(|err| {
            OnnxRuntimeError::path("config", "config.json could not be read", err)
        })?;
        let config: Value = serde_json::from_str(&config_text).map_err(|_| {
            OnnxRuntimeError::validation("config", "config.json could not be parsed")
        })?;
        let embedding_dimensions = parse_embedding_dimensions(&config)?;

        Ok(Self {
            config_path,
            embedding_dimensions,
        })
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn embedding_dimensions(&self) -> usize {
        self.embedding_dimensions
    }
}

fn parse_embedding_dimensions(config: &Value) -> Result<usize, OnnxRuntimeError> {
    let hidden_size = optional_dimension(config, "hidden_size")?;
    let n_embd = optional_dimension(config, "n_embd")?;
    match (hidden_size, n_embd) {
        (Some(left), Some(right)) if left != right => Err(OnnxRuntimeError::validation(
            "config",
            "config.json hidden_size and n_embd dimensions must agree",
        )),
        (Some(dimensions), _) | (_, Some(dimensions)) => Ok(dimensions),
        (None, None) => Err(OnnxRuntimeError::validation(
            "config",
            "config.json must define hidden_size or n_embd",
        )),
    }
}

fn optional_dimension(
    config: &Value,
    key: &'static str,
) -> Result<Option<usize>, OnnxRuntimeError> {
    let Some(value) = config.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_u64() else {
        return Err(OnnxRuntimeError::validation(
            "config",
            format!("config.json {key} must be a positive integer"),
        ));
    };
    let dimensions = usize::try_from(value).map_err(|_| {
        OnnxRuntimeError::validation("config", format!("config.json {key} is too large"))
    })?;
    validate_dimensions(dimensions)?;
    Ok(Some(dimensions))
}
