use half::{bf16, f16};
use ort::value::{TensorElementType, ValueRef, ValueType};

use super::{OnnxOutputTensorSelection, OnnxRuntimeError};

/// What an embedding graph returned: token-level states to pool as declared,
/// or vectors the graph already pooled (exports that end in a
/// `sentence_embedding` output, for example).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum EmbeddingOutput {
    Tokens(Vec<Vec<Vec<f32>>>),
    Pooled(Vec<Vec<f32>>),
}

/// Select and read the embedding output. A named output may be token-level or
/// pooled. Otherwise the first token-level floating output wins, so declared
/// pooling applies when the graph exposes token states; a graph with only a
/// pooled output is read as already pooled.
pub(super) fn extract_embedding_output(
    outputs: &ort::session::SessionOutputs<'_>,
    selection: &OnnxOutputTensorSelection,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<EmbeddingOutput, OnnxRuntimeError> {
    let read = |value: ValueRef<'_>, name: &str| {
        let (shape, values) = floating_values(value, name)?;
        embedding_output_from_values(
            &shape,
            values,
            name,
            expected_batch,
            expected_tokens,
            expected_dimensions,
        )
    };
    match selection {
        OnnxOutputTensorSelection::Named(name) => {
            let value = outputs.get(name).ok_or_else(|| {
                OnnxRuntimeError::validation(
                    "output",
                    "configured ONNX output tensor was not found",
                )
            })?;
            read(value.view(), name)
        }
        OnnxOutputTensorSelection::FirstFloatingTensor => {
            let floating = || {
                outputs
                    .iter()
                    .filter(|(_, value)| is_floating_tensor(value.dtype()))
            };
            let rank = |value: &ValueRef<'_>| match value.dtype() {
                ValueType::Tensor { shape, .. } => shape.len(),
                _ => 0,
            };
            if let Some((name, value)) = floating().find(|(_, value)| rank(value) == 3) {
                return read(value, name);
            }
            if let Some((name, value)) = floating().find(|(_, value)| rank(value) == 2) {
                return read(value, name);
            }
            match floating().next() {
                Some((name, value)) => read(value, name),
                None => Err(OnnxRuntimeError::validation(
                    "output",
                    "ONNX Runtime did not return a floating tensor output",
                )),
            }
        }
    }
}

fn is_floating_tensor(value_type: &ValueType) -> bool {
    matches!(
        value_type,
        ValueType::Tensor {
            ty: TensorElementType::Float32
                | TensorElementType::Float16
                | TensorElementType::Bfloat16,
            ..
        }
    )
}

/// The tensor's shape and its values as f32.
fn floating_values(
    value: ValueRef<'_>,
    output_name: &str,
) -> Result<(Vec<i64>, Vec<f32>), OnnxRuntimeError> {
    match value.dtype() {
        ValueType::Tensor {
            ty: TensorElementType::Float32,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f32>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX f32 output tensor extraction failed")
            })?;
            Ok((shape.to_vec(), values.to_vec()))
        }
        ValueType::Tensor {
            ty: TensorElementType::Float16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f16>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX f16 output tensor extraction failed")
            })?;
            Ok((
                shape.to_vec(),
                values.iter().map(|value| value.to_f32()).collect(),
            ))
        }
        ValueType::Tensor {
            ty: TensorElementType::Bfloat16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<bf16>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX bf16 output tensor extraction failed")
            })?;
            Ok((
                shape.to_vec(),
                values.iter().map(|value| value.to_f32()).collect(),
            ))
        }
        _ => Err(OnnxRuntimeError::validation(
            "output",
            format!("ONNX output tensor '{output_name}' is not a supported floating tensor"),
        )),
    }
}

pub(super) fn embedding_output_from_values(
    shape: &[i64],
    values: Vec<f32>,
    output_name: &str,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<EmbeddingOutput, OnnxRuntimeError> {
    let dims = shape
        .iter()
        .map(|value| shape_dimension(*value, "dimension"))
        .collect::<Result<Vec<_>, _>>()?;
    let (rows, row_len) = match dims.as_slice() {
        [batch, tokens, dimensions] => {
            if *batch != expected_batch
                || *tokens != expected_tokens
                || *dimensions != expected_dimensions
            {
                return Err(OnnxRuntimeError::validation(
                    "output",
                    "ONNX output tensor shape does not match tokenized batch and configured dimensions",
                ));
            }
            (expected_batch * expected_tokens, expected_dimensions)
        }
        [batch, dimensions] => {
            if *batch != expected_batch || *dimensions != expected_dimensions {
                return Err(OnnxRuntimeError::validation(
                    "output",
                    format!(
                        "ONNX output tensor '{output_name}' is pooled [batch, dimensions] but its shape does not match the batch and configured dimensions"
                    ),
                ));
            }
            (expected_batch, expected_dimensions)
        }
        _ => {
            return Err(OnnxRuntimeError::validation(
                "output",
                format!(
                    "ONNX output tensor '{output_name}' must have shape [batch, tokens, dimensions] or, already pooled, [batch, dimensions]"
                ),
            ))
        }
    };
    let expected_values = rows
        .checked_mul(row_len)
        .ok_or_else(|| OnnxRuntimeError::backend("ONNX output tensor size overflow"))?;
    if values.len() != expected_values {
        return Err(OnnxRuntimeError::validation(
            "output",
            "ONNX output tensor value count does not match tensor shape",
        ));
    }
    let vectors: Vec<Vec<f32>> = values.chunks(row_len).map(<[f32]>::to_vec).collect();
    if dims.len() == 2 {
        return Ok(EmbeddingOutput::Pooled(vectors));
    }
    let mut vectors = vectors.into_iter();
    Ok(EmbeddingOutput::Tokens(
        (0..expected_batch)
            .map(|_| vectors.by_ref().take(expected_tokens).collect())
            .collect(),
    ))
}

fn shape_dimension(value: i64, name: &'static str) -> Result<usize, OnnxRuntimeError> {
    usize::try_from(value).map_err(|_| {
        OnnxRuntimeError::validation("output", format!("ONNX output {name} dimension is invalid"))
    })
}

/// Name of the cross-encoder score output exported by Transformers.
pub const LOGITS_OUTPUT: &str = "logits";

/// One relevance logit per input pair, from a `[batch, 1]` (or `[batch]`)
/// `logits` output, or the first floating output when none is named so.
pub(super) fn extract_scores(
    outputs: &ort::session::SessionOutputs<'_>,
    expected_batch: usize,
) -> Result<Vec<f32>, OnnxRuntimeError> {
    let (name, value) = match outputs.get(LOGITS_OUTPUT) {
        Some(value) => (LOGITS_OUTPUT.to_string(), value.view()),
        None => outputs
            .iter()
            .find(|(_, value)| is_floating_tensor(value.dtype()))
            .map(|(name, value)| (name.to_string(), value))
            .ok_or_else(|| {
                OnnxRuntimeError::validation(
                    "output",
                    "ONNX Runtime did not return a floating score output",
                )
            })?,
    };
    let (shape, values) = float_values(value, &name)?;
    let single_label = match shape.len() {
        1 => true,
        2 => shape_dimension(shape[1], "labels")? == 1,
        _ => false,
    };
    if !single_label || shape_dimension(shape[0], "batch")? != expected_batch {
        return Err(OnnxRuntimeError::validation(
            "output",
            format!(
                "ONNX output '{name}' must hold one relevance logit per pair, shape [batch, 1]"
            ),
        ));
    }
    if values.len() != expected_batch || values.iter().any(|value| !value.is_finite()) {
        return Err(OnnxRuntimeError::validation(
            "output",
            format!("ONNX output '{name}' must hold one finite logit per pair"),
        ));
    }
    Ok(values)
}

fn float_values(value: ValueRef<'_>, name: &str) -> Result<(Vec<i64>, Vec<f32>), OnnxRuntimeError> {
    let failed = || {
        OnnxRuntimeError::validation("output", format!("ONNX output '{name}' extraction failed"))
    };
    match value.dtype() {
        ValueType::Tensor {
            ty: TensorElementType::Float32,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f32>().map_err(|_| failed())?;
            Ok((shape.to_vec(), values.to_vec()))
        }
        ValueType::Tensor {
            ty: TensorElementType::Float16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f16>().map_err(|_| failed())?;
            Ok((
                shape.to_vec(),
                values.iter().map(|value| value.to_f32()).collect(),
            ))
        }
        ValueType::Tensor {
            ty: TensorElementType::Bfloat16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<bf16>().map_err(|_| failed())?;
            Ok((
                shape.to_vec(),
                values.iter().map(|value| value.to_f32()).collect(),
            ))
        }
        _ => Err(OnnxRuntimeError::validation(
            "output",
            format!("ONNX output '{name}' is not a supported floating tensor"),
        )),
    }
}
