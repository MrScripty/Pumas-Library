use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{package::resolve_package_file, validate_dimensions, OnnxModelPath, OnnxRuntimeError};

const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxModelConfig {
    config_path: PathBuf,
    embedding_dimensions: usize,
    input_token_limit: Option<OnnxInputTokenLimit>,
}

/// The longest tokenized input a model accepts, as its config declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxInputTokenLimit {
    /// Maximum tokens per input, including special tokens.
    pub max_tokens: usize,
    /// The `config.json` field the limit was derived from.
    pub source: &'static str,
}

/// Model families whose learned position table starts after the padding
/// index (`padding_idx + 1`), so `max_position_embeddings` overstates the
/// usable sequence length by `pad_token_id + 1`.
const PADDING_OFFSET_POSITION_MODEL_TYPES: &[&str] =
    &["roberta", "xlm-roberta", "camembert", "mpnet"];

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
        let input_token_limit = parse_input_token_limit(&config)?;

        Ok(Self {
            config_path,
            embedding_dimensions,
            input_token_limit,
        })
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn embedding_dimensions(&self) -> usize {
        self.embedding_dimensions
    }

    /// The per-input token limit derived from the model's positional table
    /// (`max_position_embeddings`, or `n_positions` for GPT-style configs).
    /// `None` when the config declares no positional limit.
    pub fn input_token_limit(&self) -> Option<&OnnxInputTokenLimit> {
        self.input_token_limit.as_ref()
    }
}

fn parse_input_token_limit(
    config: &Value,
) -> Result<Option<OnnxInputTokenLimit>, OnnxRuntimeError> {
    let max_position_embeddings = optional_positive(config, "max_position_embeddings")?;
    let n_positions = optional_positive(config, "n_positions")?;
    let (positions, source) = match (max_position_embeddings, n_positions) {
        (Some(left), Some(right)) if left != right => {
            return Err(OnnxRuntimeError::validation(
                "config",
                "config.json max_position_embeddings and n_positions must agree",
            ))
        }
        (Some(positions), _) => (positions, "max_position_embeddings"),
        (None, Some(positions)) => (positions, "n_positions"),
        (None, None) => return Ok(None),
    };
    let padding_offset = match config.get("model_type").and_then(Value::as_str) {
        Some(model_type) if PADDING_OFFSET_POSITION_MODEL_TYPES.contains(&model_type) => {
            optional_positive(config, "pad_token_id")?.unwrap_or(1) + 1
        }
        _ => 0,
    };
    let max_tokens = positions
        .checked_sub(padding_offset)
        .filter(|max| *max > 0)
        .ok_or_else(|| {
            OnnxRuntimeError::validation(
                "config",
                format!("config.json {source} leaves no usable input positions"),
            )
        })?;
    Ok(Some(OnnxInputTokenLimit { max_tokens, source }))
}

fn optional_positive(config: &Value, key: &'static str) -> Result<Option<usize>, OnnxRuntimeError> {
    let Some(value) = config.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .map(Some)
        .ok_or_else(|| {
            OnnxRuntimeError::validation(
                "config",
                format!("config.json {key} must be a non-negative integer"),
            )
        })
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
