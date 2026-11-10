use half::{bf16, f16};
use ort::value::{Shape, TensorElementType, ValueRef, ValueType};

use super::{OnnxOutputTensorSelection, OnnxRuntimeError};

pub(super) fn extract_hidden_states(
    outputs: &ort::session::SessionOutputs<'_>,
    selection: &OnnxOutputTensorSelection,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<Vec<Vec<Vec<f32>>>, OnnxRuntimeError> {
    match selection {
        OnnxOutputTensorSelection::Named(name) => {
            let value = outputs.get(name).ok_or_else(|| {
                OnnxRuntimeError::validation(
                    "output",
                    "configured ONNX output tensor was not found",
                )
            })?;
            extract_hidden_states_value(
                value.view(),
                name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        OnnxOutputTensorSelection::FirstFloatingTensor => {
            for (name, value) in outputs {
                if is_floating_tensor(value.dtype()) {
                    return extract_hidden_states_value(
                        value,
                        name,
                        expected_batch,
                        expected_tokens,
                        expected_dimensions,
                    );
                }
            }
            Err(OnnxRuntimeError::validation(
                "output",
                "ONNX Runtime did not return a floating tensor output",
            ))
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

fn extract_hidden_states_value(
    value: ValueRef<'_>,
    output_name: &str,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<Vec<Vec<Vec<f32>>>, OnnxRuntimeError> {
    match value.dtype() {
        ValueType::Tensor {
            ty: TensorElementType::Float32,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f32>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX f32 output tensor extraction failed")
            })?;
            tensor_values_to_hidden_states(
                shape,
                values.to_vec(),
                output_name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        ValueType::Tensor {
            ty: TensorElementType::Float16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<f16>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX f16 output tensor extraction failed")
            })?;
            tensor_values_to_hidden_states(
                shape,
                values.iter().map(|value| value.to_f32()).collect(),
                output_name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        ValueType::Tensor {
            ty: TensorElementType::Bfloat16,
            ..
        } => {
            let (shape, values) = value.try_extract_tensor::<bf16>().map_err(|_| {
                OnnxRuntimeError::validation("output", "ONNX bf16 output tensor extraction failed")
            })?;
            tensor_values_to_hidden_states(
                shape,
                values.iter().map(|value| value.to_f32()).collect(),
                output_name,
                expected_batch,
                expected_tokens,
                expected_dimensions,
            )
        }
        _ => Err(OnnxRuntimeError::validation(
            "output",
            format!("ONNX output tensor '{output_name}' is not a supported floating tensor"),
        )),
    }
}

fn tensor_values_to_hidden_states(
    shape: &Shape,
    values: Vec<f32>,
    output_name: &str,
    expected_batch: usize,
    expected_tokens: usize,
    expected_dimensions: usize,
) -> Result<Vec<Vec<Vec<f32>>>, OnnxRuntimeError> {
    if shape.len() != 3 {
        return Err(OnnxRuntimeError::validation(
            "output",
            format!(
                "ONNX output tensor '{output_name}' must have shape [batch, tokens, dimensions]"
            ),
        ));
    }
    let batch = shape_dimension(shape[0], "batch")?;
    let tokens = shape_dimension(shape[1], "tokens")?;
    let dimensions = shape_dimension(shape[2], "dimensions")?;
    if batch != expected_batch || tokens != expected_tokens || dimensions != expected_dimensions {
        return Err(OnnxRuntimeError::validation(
            "output",
            "ONNX output tensor shape does not match tokenized batch and configured dimensions",
        ));
    }
    let expected_values = expected_batch
        .checked_mul(expected_tokens)
        .and_then(|value| value.checked_mul(expected_dimensions))
        .ok_or_else(|| OnnxRuntimeError::backend("ONNX output tensor size overflow"))?;
    if values.len() != expected_values {
        return Err(OnnxRuntimeError::validation(
            "output",
            "ONNX output tensor value count does not match tensor shape",
        ));
    }

    let mut offset = 0usize;
    let mut hidden_states = Vec::with_capacity(expected_batch);
    for _ in 0..expected_batch {
        let mut token_embeddings = Vec::with_capacity(expected_tokens);
        for _ in 0..expected_tokens {
            let next = offset + expected_dimensions;
            token_embeddings.push(values[offset..next].to_vec());
            offset = next;
        }
        hidden_states.push(token_embeddings);
    }
    Ok(hidden_states)
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
