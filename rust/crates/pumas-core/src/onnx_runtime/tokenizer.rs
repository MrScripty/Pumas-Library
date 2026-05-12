use std::path::PathBuf;

use tokenizers::Tokenizer;

use super::{package::resolve_package_file, OnnxEmbeddingRequest, OnnxModelPath, OnnxRuntimeError};

const TOKENIZER_FILE_NAME: &str = "tokenizer.json";
const MAX_EMBEDDING_TOKENS_PER_INPUT: usize = 8_192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxTokenizedInput {
    pub input_ids: Vec<i64>,
    pub attention_mask: Vec<i64>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxTokenizedBatch {
    pub inputs: Vec<OnnxTokenizedInput>,
    pub total_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct OnnxTokenizer {
    tokenizer: Tokenizer,
    tokenizer_path: PathBuf,
}

impl OnnxTokenizer {
    pub fn from_model_path(model_path: &OnnxModelPath) -> Result<Self, OnnxRuntimeError> {
        let tokenizer_path = resolve_package_file(model_path, TOKENIZER_FILE_NAME, "tokenizer")?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|_| {
            OnnxRuntimeError::validation("tokenizer", "tokenizer.json could not be loaded")
        })?;

        Ok(Self {
            tokenizer,
            tokenizer_path,
        })
    }

    pub fn tokenizer_path(&self) -> &std::path::Path {
        &self.tokenizer_path
    }

    pub fn tokenize_request(
        &self,
        request: &OnnxEmbeddingRequest,
    ) -> Result<OnnxTokenizedBatch, OnnxRuntimeError> {
        let mut inputs = Vec::with_capacity(request.input.len());
        let mut total_tokens = 0usize;

        for text in &request.input {
            let encoding = self.tokenizer.encode(text.as_str(), true).map_err(|_| {
                OnnxRuntimeError::validation("input", "embedding input could not be tokenized")
            })?;
            let token_count = encoding.get_ids().len();
            validate_token_count(token_count)?;
            total_tokens = total_tokens.checked_add(token_count).ok_or_else(|| {
                OnnxRuntimeError::backend("ONNX token count exceeds supported size")
            })?;

            inputs.push(OnnxTokenizedInput {
                input_ids: token_values(encoding.get_ids()),
                attention_mask: attention_mask_values(encoding.get_attention_mask(), token_count)?,
                token_count,
            });
        }

        Ok(OnnxTokenizedBatch {
            inputs,
            total_tokens,
        })
    }
}

fn validate_token_count(token_count: usize) -> Result<(), OnnxRuntimeError> {
    if token_count == 0 {
        return Err(OnnxRuntimeError::validation(
            "input",
            "tokenized embedding input must contain at least one token",
        ));
    }
    if token_count > MAX_EMBEDDING_TOKENS_PER_INPUT {
        return Err(OnnxRuntimeError::validation(
            "input",
            format!(
                "tokenized embedding input must contain at most {MAX_EMBEDDING_TOKENS_PER_INPUT} tokens"
            ),
        ));
    }
    Ok(())
}

fn token_values(values: &[u32]) -> Vec<i64> {
    values.iter().map(|value| i64::from(*value)).collect()
}

fn attention_mask_values(values: &[u32], token_count: usize) -> Result<Vec<i64>, OnnxRuntimeError> {
    if values.is_empty() {
        return Ok(vec![1; token_count]);
    }
    if values.len() != token_count {
        return Err(OnnxRuntimeError::backend(
            "ONNX tokenizer returned mismatched attention mask length",
        ));
    }
    Ok(token_values(values))
}
