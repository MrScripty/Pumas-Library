use super::{validate_dimensions, OnnxRuntimeError, OnnxTokenizedBatch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnnxOutputTensorSelection {
    FirstFloatingTensor,
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnnxEmbeddingPooling {
    Mean,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxEmbeddingPostprocessConfig {
    pub output_tensor: OnnxOutputTensorSelection,
    pub pooling: OnnxEmbeddingPooling,
    pub apply_layer_norm: bool,
    pub l2_normalize: bool,
}

impl OnnxEmbeddingPostprocessConfig {
    pub fn mean_pool_l2() -> Self {
        Self {
            output_tensor: OnnxOutputTensorSelection::FirstFloatingTensor,
            pooling: OnnxEmbeddingPooling::Mean,
            apply_layer_norm: false,
            l2_normalize: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OnnxEmbeddingPostprocessor {
    config: OnnxEmbeddingPostprocessConfig,
    source_dimensions: usize,
}

impl OnnxEmbeddingPostprocessor {
    pub fn new(
        config: OnnxEmbeddingPostprocessConfig,
        source_dimensions: usize,
    ) -> Result<Self, OnnxRuntimeError> {
        validate_dimensions(source_dimensions)?;
        Ok(Self {
            config,
            source_dimensions,
        })
    }

    pub fn postprocess_hidden_states(
        &self,
        hidden_states: &[Vec<Vec<f32>>],
        tokenized: &OnnxTokenizedBatch,
        dimensions: Option<usize>,
    ) -> Result<Vec<Vec<f32>>, OnnxRuntimeError> {
        if hidden_states.len() != tokenized.inputs.len() {
            return Err(OnnxRuntimeError::validation(
                "output",
                "ONNX output batch size must match tokenized input batch size",
            ));
        }
        let dimensions = dimensions.unwrap_or(self.source_dimensions);
        validate_dimensions(dimensions)?;
        if dimensions > self.source_dimensions {
            return Err(OnnxRuntimeError::validation(
                "dimensions",
                "requested embedding dimensions exceed ONNX output dimensions",
            ));
        }
        let _response_values = hidden_states
            .len()
            .checked_mul(dimensions)
            .ok_or_else(|| OnnxRuntimeError::backend("ONNX embedding response size overflow"))?;

        hidden_states
            .iter()
            .zip(&tokenized.inputs)
            .map(|(token_embeddings, tokenized_input)| {
                let mut pooled = match self.config.pooling {
                    OnnxEmbeddingPooling::Mean => mean_pool(
                        token_embeddings,
                        &tokenized_input.attention_mask,
                        self.source_dimensions,
                    )?,
                };
                if self.config.apply_layer_norm {
                    apply_layer_norm(&mut pooled);
                }
                pooled.truncate(dimensions);
                if self.config.l2_normalize {
                    l2_normalize(&mut pooled);
                }
                Ok(pooled)
            })
            .collect()
    }
}

fn mean_pool(
    token_embeddings: &[Vec<f32>],
    attention_mask: &[i64],
    source_dimensions: usize,
) -> Result<Vec<f32>, OnnxRuntimeError> {
    if token_embeddings.len() != attention_mask.len() {
        return Err(OnnxRuntimeError::validation(
            "output",
            "ONNX output token count must match tokenizer attention mask length",
        ));
    }

    let mut pooled = vec![0.0f32; source_dimensions];
    let mut included = 0usize;
    for (embedding, mask) in token_embeddings.iter().zip(attention_mask) {
        if embedding.len() != source_dimensions {
            return Err(OnnxRuntimeError::validation(
                "output",
                "ONNX output token vector dimensions do not match session dimensions",
            ));
        }
        if *mask == 0 {
            continue;
        }
        included = included
            .checked_add(1)
            .ok_or_else(|| OnnxRuntimeError::backend("ONNX postprocess token count overflow"))?;
        for (accumulator, value) in pooled.iter_mut().zip(embedding) {
            *accumulator += *value;
        }
    }
    if included == 0 {
        return Err(OnnxRuntimeError::validation(
            "input",
            "tokenized embedding input must contain at least one attended token",
        ));
    }
    let divisor = included as f32;
    for value in &mut pooled {
        *value /= divisor;
    }
    Ok(pooled)
}

fn apply_layer_norm(values: &mut [f32]) {
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / values.len() as f32;
    let denominator = (variance + 1e-12).sqrt();
    for value in values {
        *value = (*value - mean) / denominator;
    }
}

fn l2_normalize(values: &mut [f32]) {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return;
    }
    for value in values {
        *value /= norm;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onnx_runtime::OnnxTokenizedInput;

    fn tokenized_batch(masks: Vec<Vec<i64>>) -> OnnxTokenizedBatch {
        let total_tokens = masks.iter().map(Vec::len).sum();
        let inputs = masks
            .into_iter()
            .map(|attention_mask| OnnxTokenizedInput {
                input_ids: vec![1; attention_mask.len()],
                token_count: attention_mask.len(),
                attention_mask,
            })
            .collect();
        OnnxTokenizedBatch {
            inputs,
            total_tokens,
        }
    }

    #[test]
    fn mean_pooling_preserves_batch_order_and_attention_mask() {
        let postprocessor =
            OnnxEmbeddingPostprocessor::new(OnnxEmbeddingPostprocessConfig::mean_pool_l2(), 2)
                .unwrap();
        let hidden_states = vec![
            vec![vec![1.0, 1.0], vec![3.0, 3.0]],
            vec![vec![2.0, 4.0], vec![100.0, 100.0]],
        ];
        let tokenized = tokenized_batch(vec![vec![1, 1], vec![1, 0]]);

        let embeddings = postprocessor
            .postprocess_hidden_states(&hidden_states, &tokenized, None)
            .unwrap();

        assert_eq!(embeddings.len(), 2);
        assert_close(&embeddings[0], &[0.70710677, 0.70710677]);
        assert_close(&embeddings[1], &[0.4472136, 0.8944272]);
    }

    #[test]
    fn truncation_runs_before_l2_normalization() {
        let postprocessor =
            OnnxEmbeddingPostprocessor::new(OnnxEmbeddingPostprocessConfig::mean_pool_l2(), 3)
                .unwrap();
        let hidden_states = vec![vec![vec![3.0, 4.0, 12.0]]];
        let tokenized = tokenized_batch(vec![vec![1]]);

        let embeddings = postprocessor
            .postprocess_hidden_states(&hidden_states, &tokenized, Some(2))
            .unwrap();

        assert_close(&embeddings[0], &[0.6, 0.8]);
    }

    #[test]
    fn optional_layer_norm_is_applied_before_truncation() {
        let postprocessor = OnnxEmbeddingPostprocessor::new(
            OnnxEmbeddingPostprocessConfig {
                output_tensor: OnnxOutputTensorSelection::FirstFloatingTensor,
                pooling: OnnxEmbeddingPooling::Mean,
                apply_layer_norm: true,
                l2_normalize: false,
            },
            3,
        )
        .unwrap();
        let hidden_states = vec![vec![vec![1.0, 2.0, 3.0]]];
        let tokenized = tokenized_batch(vec![vec![1]]);

        let embeddings = postprocessor
            .postprocess_hidden_states(&hidden_states, &tokenized, None)
            .unwrap();

        assert_close(&embeddings[0], &[-1.2247448, 0.0, 1.2247448]);
    }

    #[test]
    fn invalid_dimensions_and_shape_are_rejected() {
        let postprocessor =
            OnnxEmbeddingPostprocessor::new(OnnxEmbeddingPostprocessConfig::mean_pool_l2(), 2)
                .unwrap();
        let tokenized = tokenized_batch(vec![vec![1]]);

        let err = postprocessor
            .postprocess_hidden_states(&[vec![vec![1.0, 2.0]]], &tokenized, Some(3))
            .unwrap_err();
        assert_eq!(err.field.as_deref(), Some("dimensions"));

        let err = postprocessor
            .postprocess_hidden_states(&[vec![vec![1.0]]], &tokenized, None)
            .unwrap_err();
        assert_eq!(err.field.as_deref(), Some("output"));
    }

    fn assert_close(actual: &[f32], expected: &[f32]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (*actual - *expected).abs() < 0.00001,
                "expected {actual} to be close to {expected}",
            );
        }
    }
}
