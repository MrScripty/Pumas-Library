//! Model format conversion between GGUF and Safetensors.
//!
//! This module provides the `ConversionManager` for converting models between
//! GGUF (used by llama.cpp/Ollama) and Safetensors (used by transformers/diffusers)
//! formats. Conversion is performed by Python scripts running in a dedicated
//! virtual environment, with per-tensor progress tracking.

mod manager;
mod progress;
mod scripts;
mod types;

pub use manager::ConversionManager;
pub use types::{
    ConversionDirection, ConversionProgress, ConversionRequest, ConversionSource,
    ConversionStatus, QuantOption, ScriptProgressLine,
};
