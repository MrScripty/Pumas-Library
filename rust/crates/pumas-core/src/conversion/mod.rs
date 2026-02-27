//! Model format conversion and quantization.
//!
//! This module provides the `ConversionManager` for converting models between
//! GGUF (used by llama.cpp/Ollama) and Safetensors (used by transformers/diffusers)
//! formats, and for quantizing models using pluggable backends.
//!
//! Conversion is performed by Python scripts running in a dedicated virtual
//! environment. Quantization uses the `QuantizationBackend` trait with backend-
//! specific implementations (e.g. `LlamaCppBackend` for GGUF quantization).

pub mod llama_cpp;
mod manager;
pub mod nvfp4;
pub(crate) mod pipeline;
pub(crate) mod progress;
mod scripts;
pub mod sherry;
mod types;

pub use llama_cpp::LlamaCppBackend;
pub use manager::ConversionManager;
pub use nvfp4::Nvfp4Backend;
pub use sherry::SherryBackend;
pub use types::{
    BackendStatus, ConversionDirection, ConversionProgress, ConversionRequest, ConversionSource,
    ConversionStatus, QuantBackend, QuantOption, QuantizationBackend, QuantizeParams,
    ScriptProgressLine,
};
