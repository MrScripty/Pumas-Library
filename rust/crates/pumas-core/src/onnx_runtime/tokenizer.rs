use std::path::PathBuf;

use tokenizers::Tokenizer;

use super::{
    package::resolve_package_file, OnnxEmbeddingRequest, OnnxInputTokenLimit, OnnxModelPath,
    OnnxRuntimeError,
};

const TOKENIZER_FILE_NAME: &str = "tokenizer.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxTokenizedInput {
    pub input_ids: Vec<i64>,
    pub attention_mask: Vec<i64>,
    /// Segment ids from the tokenizer: 0 for a single text, 0/1 for a pair.
    pub type_ids: Vec<i64>,
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

    /// Tokenize every input without a model token limit.
    pub fn tokenize_request(
        &self,
        request: &OnnxEmbeddingRequest,
    ) -> Result<OnnxTokenizedBatch, OnnxRuntimeError> {
        self.tokenize_request_within(request, None)
    }

    /// Tokenize every input and refuse, with a reason naming the input, its
    /// token count and the model's limit, any input longer than `limit`.
    pub fn tokenize_request_within(
        &self,
        request: &OnnxEmbeddingRequest,
        limit: Option<&OnnxInputTokenLimit>,
    ) -> Result<OnnxTokenizedBatch, OnnxRuntimeError> {
        let mut inputs = Vec::with_capacity(request.input.len());
        let mut total_tokens = 0usize;

        for (index, text) in request.input.iter().enumerate() {
            let encoding = self.tokenizer.encode(text.as_str(), true).map_err(|_| {
                OnnxRuntimeError::validation("input", "embedding input could not be tokenized")
            })?;
            let token_count = encoding.get_ids().len();
            validate_token_count(token_count)?;
            if let Some(limit) = limit {
                if token_count > limit.max_tokens {
                    return Err(OnnxRuntimeError::validation(
                        "input",
                        format!(
                            "embedding input {index} has {token_count} tokens, but model {} \
                             accepts at most {} tokens per input (config.json {})",
                            request.model_id.as_str(),
                            limit.max_tokens,
                            limit.source
                        ),
                    ));
                }
            }
            total_tokens = total_tokens.checked_add(token_count).ok_or_else(|| {
                OnnxRuntimeError::backend("ONNX token count exceeds supported size")
            })?;

            inputs.push(tokenized_input(&encoding, token_count)?);
        }

        Ok(OnnxTokenizedBatch {
            inputs,
            total_tokens,
        })
    }

    /// Tokenize `(query, document)` pairs as the tokenizer's pair template
    /// joins them, refusing, with the document named, any pair longer than
    /// `limit`.
    pub fn tokenize_pairs_within(
        &self,
        model_id: &str,
        query: &str,
        documents: &[String],
        limit: Option<&OnnxInputTokenLimit>,
    ) -> Result<OnnxTokenizedBatch, OnnxRuntimeError> {
        let mut inputs = Vec::with_capacity(documents.len());
        let mut total_tokens = 0usize;
        for (index, document) in documents.iter().enumerate() {
            let encoding = self
                .tokenizer
                .encode((query, document.as_str()), true)
                .map_err(|_| {
                    OnnxRuntimeError::validation(
                        "documents",
                        "a rerank pair could not be tokenized",
                    )
                })?;
            let token_count = encoding.get_ids().len();
            validate_token_count(token_count)?;
            if let Some(limit) = limit {
                if token_count > limit.max_tokens {
                    return Err(OnnxRuntimeError::validation(
                        "documents",
                        format!(
                            "query and document {index} have {token_count} tokens, but model \
                             {model_id} accepts at most {} tokens per pair (config.json {})",
                            limit.max_tokens, limit.source
                        ),
                    ));
                }
            }
            total_tokens = total_tokens.checked_add(token_count).ok_or_else(|| {
                OnnxRuntimeError::backend("ONNX token count exceeds supported size")
            })?;
            inputs.push(tokenized_input(&encoding, token_count)?);
        }
        Ok(OnnxTokenizedBatch {
            inputs,
            total_tokens,
        })
    }
}

fn tokenized_input(
    encoding: &tokenizers::Encoding,
    token_count: usize,
) -> Result<OnnxTokenizedInput, OnnxRuntimeError> {
    let type_ids = encoding.get_type_ids();
    if !type_ids.is_empty() && type_ids.len() != token_count {
        return Err(OnnxRuntimeError::backend(
            "ONNX tokenizer returned mismatched segment id length",
        ));
    }
    Ok(OnnxTokenizedInput {
        input_ids: token_values(encoding.get_ids()),
        attention_mask: attention_mask_values(encoding.get_attention_mask(), token_count)?,
        type_ids: if type_ids.is_empty() {
            vec![0; token_count]
        } else {
            token_values(type_ids)
        },
        token_count,
    })
}

fn validate_token_count(token_count: usize) -> Result<(), OnnxRuntimeError> {
    if token_count == 0 {
        return Err(OnnxRuntimeError::validation(
            "input",
            "tokenized embedding input must contain at least one token",
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
