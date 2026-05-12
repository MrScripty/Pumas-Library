use super::{OnnxRuntimeError, OnnxTokenizedBatch};

pub(super) struct TokenTensors {
    pub input_ids: Vec<i64>,
    pub attention_mask: Vec<i64>,
    pub token_type_ids: Vec<i64>,
    pub batch_size: usize,
    pub sequence_len: usize,
}

impl TokenTensors {
    pub fn from_tokenized_batch(tokenized: &OnnxTokenizedBatch) -> Result<Self, OnnxRuntimeError> {
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
