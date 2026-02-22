//! Types for model format conversion and quantization operations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cancel::CancellationToken;
use crate::Result;

use super::progress::ConversionProgressTracker;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Direction of model format conversion or quantization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionDirection {
    /// Dequantize GGUF to Safetensors (float16/float32).
    GgufToSafetensors,
    /// Convert Safetensors to GGUF (F16 only, no quantization).
    SafetensorsToGguf,
    /// Convert Safetensors to a quantized GGUF via llama.cpp pipeline.
    /// 3-step: convert_hf_to_gguf.py → [llama-imatrix] → llama-quantize
    SafetensorsToQuantizedGguf,
    /// Re-quantize an existing GGUF to a different quantization type.
    /// 1-step (or 2 with imatrix): [llama-imatrix] → llama-quantize
    GgufToQuantizedGguf,
    /// Quantize Safetensors to NVFP4 format via nvidia-modelopt / TensorRT-LLM.
    /// Requires NVIDIA Blackwell GPU.
    SafetensorsToNvfp4,
    /// Quantize Safetensors to 1.25-bit ternary via Sherry / AngelSlim QAT.
    /// Requires GPU with sufficient VRAM for quantization-aware training.
    SafetensorsToSherryQat,
}

/// Status of a conversion or quantization operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStatus {
    // -- Shared statuses (existing) --
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

    // -- Quantization pipeline statuses --
    /// Building the native toolchain (e.g. git clone + cmake for llama.cpp)
    BuildingToolchain,
    /// Generating an intermediate F16 GGUF from safetensors
    GeneratingF16Gguf,
    /// Computing an importance matrix for quality-guided quantization
    ComputingImatrix,
    /// Running the quantization step (e.g. llama-quantize)
    Quantizing,
    /// Running calibration pass for NVFP4 quantization
    Calibrating,
    /// Running quantization-aware training (Sherry QAT)
    Training,
}

/// Identifies which quantization backend provides a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantBackend {
    /// Existing Python-based safetensors ↔ GGUF F16 conversion.
    PythonConversion,
    /// llama.cpp native quantization (llama-quantize, llama-imatrix).
    LlamaCpp,
    /// NVIDIA NVFP4 via TensorRT-LLM / nvidia-modelopt (Phase 2).
    Nvfp4,
    /// Sherry / AngelSlim quantization-aware training (Phase 3).
    Sherry,
}

// ---------------------------------------------------------------------------
// Request / Progress
// ---------------------------------------------------------------------------

/// Request to convert a model between formats or quantize it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConversionRequest {
    /// Source model ID in the library (relative path from library root).
    pub model_id: String,
    /// Conversion direction.
    pub direction: ConversionDirection,
    /// Target quantization type (e.g. "Q4_K_M", "IQ3_XXS", "F16").
    #[serde(default)]
    pub target_quant: Option<String>,
    /// Custom output name (auto-generated if omitted).
    #[serde(default)]
    pub output_name: Option<String>,
    /// Path to a calibration text file for importance matrix generation.
    /// Required for IQ* quant types unless `force_imatrix` is false.
    #[serde(default)]
    pub imatrix_calibration_file: Option<String>,
    /// Use an importance matrix even for non-IQ quant types.
    #[serde(default)]
    pub force_imatrix: Option<bool>,
}

/// Progress of a conversion or quantization operation.
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

    // -- Multi-step pipeline tracking --
    /// Which pipeline step is active (1-indexed). None for single-step operations.
    #[serde(default)]
    pub pipeline_step: Option<u32>,
    /// Total number of pipeline steps.
    #[serde(default)]
    pub pipeline_steps_total: Option<u32>,
    /// Human-readable label for the current pipeline step.
    #[serde(default)]
    pub pipeline_step_label: Option<String>,
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Quantization options & backend metadata
// ---------------------------------------------------------------------------

/// A supported quantization option.
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
    /// Which backend provides this quantization type.
    #[serde(default)]
    pub backend: Option<QuantBackend>,
    /// Whether an importance matrix is highly recommended for quality.
    #[serde(default)]
    pub imatrix_recommended: bool,
}

/// Readiness status of a quantization backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    /// Backend identifier.
    pub backend: QuantBackend,
    /// Human-readable backend name.
    pub name: String,
    /// Whether the backend environment is fully set up and ready.
    pub ready: bool,
}

// ---------------------------------------------------------------------------
// Script progress parsing
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Quantization backend trait & params
// ---------------------------------------------------------------------------

/// Parameters passed to a quantization backend's `quantize()` method.
#[derive(Debug, Clone)]
pub struct QuantizeParams {
    /// Unique conversion/quantization operation ID for progress tracking.
    pub conversion_id: String,
    /// Path to the source model directory.
    pub model_path: PathBuf,
    /// Source model ID in the library (for provenance metadata).
    pub source_model_id: String,
    /// Target quantization type name (e.g. "Q4_K_M", "IQ3_XXS").
    pub target_quant: String,
    /// Path to calibration text file for importance matrix generation.
    pub calibration_file: Option<PathBuf>,
    /// Force importance matrix even for non-IQ quant types.
    pub force_imatrix: bool,
}

/// Trait for pluggable quantization backends.
///
/// Each backend manages its own environment (binaries, venvs, etc.)
/// and implements the quantization pipeline as a series of subprocess steps.
///
/// # Invariants
/// - `quantize()` must not modify the source model directory.
/// - Output is written to a temp directory, then atomically renamed.
/// - Progress updates flow through the provided `ConversionProgressTracker`.
#[async_trait::async_trait]
pub trait QuantizationBackend: Send + Sync {
    /// Human-readable backend name for logging and UI display.
    fn name(&self) -> &str;

    /// Which `QuantBackend` variant this backend corresponds to.
    fn backend_id(&self) -> QuantBackend;

    /// Check if the backend's environment is fully set up (binaries exist, etc.).
    fn is_ready(&self) -> bool;

    /// Set up the backend environment (clone repos, build, install deps).
    ///
    /// # Postconditions
    /// - `is_ready()` returns true on success.
    async fn ensure_environment(&self) -> Result<()>;

    /// Return the quantization types this backend supports.
    fn supported_quant_types(&self) -> Vec<QuantOption>;

    /// Execute the quantization pipeline.
    ///
    /// # Preconditions
    /// - `is_ready()` must be true.
    /// - Source model files must exist at `params.model_path`.
    /// - For IQ types: `params.calibration_file` should be provided.
    ///
    /// # Postconditions
    /// - Quantized model files written to the returned `PathBuf` (output directory).
    /// - Source model directory unchanged.
    /// - Temp files cleaned up on success or failure.
    async fn quantize(
        &self,
        params: &QuantizeParams,
        progress: &ConversionProgressTracker,
        cancel_token: &CancellationToken,
    ) -> Result<PathBuf>;
}
