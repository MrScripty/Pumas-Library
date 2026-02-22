//! Types for model format conversion operations.

use serde::{Deserialize, Serialize};

/// Direction of model format conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionDirection {
    GgufToSafetensors,
    SafetensorsToGguf,
}

/// Status of a conversion operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStatus {
    /// Python environment is being set up (venv creation, pip install)
    SettingUp,
    /// Validating source files and format
    Validating,
    /// Actively converting tensors
    Converting,
    /// Writing output file(s)
    Writing,
    /// Registering converted model in the library
    Importing,
    /// Conversion completed successfully
    Completed,
    /// Conversion was cancelled by the user
    Cancelled,
    /// Conversion failed
    Error,
}

/// Request to convert a model between formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversionRequest {
    /// Source model ID in the library (relative path from library root)
    pub model_id: String,
    /// Conversion direction
    pub direction: ConversionDirection,
    /// Target quantization type (only for safetensors-to-GGUF, e.g. "F16")
    #[serde(default)]
    pub target_quant: Option<String>,
    /// Custom output name (auto-generated if omitted)
    #[serde(default)]
    pub output_name: Option<String>,
}

/// Progress of a conversion operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionProgress {
    pub conversion_id: String,
    pub source_model_id: String,
    pub direction: ConversionDirection,
    pub status: ConversionStatus,
    /// Overall progress from 0.0 to 1.0
    #[serde(default)]
    pub progress: Option<f32>,
    /// Name of the tensor currently being processed
    #[serde(default)]
    pub current_tensor: Option<String>,
    /// Number of tensors processed so far
    #[serde(default)]
    pub tensors_completed: Option<u32>,
    /// Total number of tensors
    #[serde(default)]
    pub tensors_total: Option<u32>,
    /// Bytes written to output so far
    #[serde(default)]
    pub bytes_written: Option<u64>,
    /// Estimated total output size in bytes
    #[serde(default)]
    pub estimated_output_size: Option<u64>,
    /// Target quantization type (for safetensors-to-GGUF)
    #[serde(default)]
    pub target_quant: Option<String>,
    /// Error message if status is Error
    #[serde(default)]
    pub error: Option<String>,
    /// Output model ID after successful conversion and library import
    #[serde(default)]
    pub output_model_id: Option<String>,
}

/// Provenance tracking for a model created by format conversion.
///
/// Stored in `metadata.json` so we always know where a converted model came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversionSource {
    /// Model ID of the source model in the library
    pub source_model_id: String,
    /// Original format (e.g. "gguf", "safetensors")
    pub source_format: String,
    /// Original quantization type if applicable (e.g. "Q4_K_M", "Q8_0")
    #[serde(default)]
    pub source_quant: Option<String>,
    /// Target format produced (e.g. "safetensors", "gguf")
    pub target_format: String,
    /// Target quantization if applicable (e.g. "F16")
    #[serde(default)]
    pub target_quant: Option<String>,
    /// Whether quality loss occurred (true when dequantizing from a quantized format)
    pub was_dequantized: bool,
    /// ISO 8601 timestamp of when conversion was performed
    pub conversion_date: String,
}

/// A supported quantization option for safetensors-to-GGUF conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuantOption {
    /// Quantization type name (e.g. "Q4_K_M", "F16")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Approximate bits per weight
    pub bits_per_weight: f32,
    /// Whether this is a recommended default option
    pub recommended: bool,
}

/// JSON progress line emitted by the Python conversion scripts on stdout.
#[derive(Debug, Clone, Deserialize)]
pub struct ScriptProgressLine {
    pub stage: String,
    #[serde(default)]
    pub tensor_index: Option<u32>,
    #[serde(default)]
    pub tensor_count: Option<u32>,
    #[serde(default)]
    pub tensor_name: Option<String>,
    #[serde(default)]
    pub bytes_written: Option<u64>,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub output_size: Option<u64>,
    #[serde(default)]
    pub message: Option<String>,
}
