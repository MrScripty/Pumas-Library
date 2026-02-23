//! Model format conversion handlers.

use super::require_str_param;
use crate::server::AppState;
use serde_json::{json, Value};

use super::{get_bool_param, get_str_param};

pub async fn start_model_conversion(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let direction = require_str_param(params, "direction", "direction")?;
    let target_quant = get_str_param(params, "target_quant", "targetQuant").map(String::from);
    let output_name = get_str_param(params, "output_name", "outputName").map(String::from);
    let imatrix_calibration_file =
        get_str_param(params, "imatrix_calibration_file", "imatrixCalibrationFile")
            .map(String::from);
    let force_imatrix = get_bool_param(params, "force_imatrix", "forceImatrix");

    let direction = match direction.as_str() {
        "gguf_to_safetensors" | "GgufToSafetensors" => {
            pumas_library::conversion::ConversionDirection::GgufToSafetensors
        }
        "safetensors_to_gguf" | "SafetensorsToGguf" => {
            pumas_library::conversion::ConversionDirection::SafetensorsToGguf
        }
        "safetensors_to_quantized_gguf" | "SafetensorsToQuantizedGguf" => {
            pumas_library::conversion::ConversionDirection::SafetensorsToQuantizedGguf
        }
        "gguf_to_quantized_gguf" | "GgufToQuantizedGguf" => {
            pumas_library::conversion::ConversionDirection::GgufToQuantizedGguf
        }
        "safetensors_to_nvfp4" | "SafetensorsToNvfp4" => {
            pumas_library::conversion::ConversionDirection::SafetensorsToNvfp4
        }
        "safetensors_to_sherry_qat" | "SafetensorsToSherryQat" => {
            pumas_library::conversion::ConversionDirection::SafetensorsToSherryQat
        }
        _ => {
            return Err(pumas_library::PumasError::InvalidParams {
                message: format!("Invalid conversion direction: {}", direction),
            });
        }
    };

    let request = pumas_library::conversion::ConversionRequest {
        model_id,
        direction,
        target_quant,
        output_name,
        imatrix_calibration_file,
        force_imatrix,
    };

    let conversion_id = state.api.start_conversion(request).await?;
    Ok(json!({
        "success": true,
        "conversion_id": conversion_id
    }))
}

pub async fn get_conversion_progress(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let conversion_id = require_str_param(params, "conversion_id", "conversionId")?;
    let progress = state.api.get_conversion_progress(&conversion_id);
    Ok(json!({
        "success": true,
        "progress": progress
    }))
}

pub async fn cancel_model_conversion(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let conversion_id = require_str_param(params, "conversion_id", "conversionId")?;
    let cancelled = state.api.cancel_conversion(&conversion_id).await?;
    Ok(json!({
        "success": true,
        "cancelled": cancelled
    }))
}

pub async fn list_model_conversions(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let conversions = state.api.list_conversions();
    Ok(json!({
        "success": true,
        "conversions": conversions
    }))
}

pub async fn check_conversion_environment(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let ready = state.api.is_conversion_environment_ready();
    Ok(json!({
        "success": true,
        "ready": ready
    }))
}

pub async fn setup_conversion_environment(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    state.api.ensure_conversion_environment().await?;
    Ok(json!({
        "success": true
    }))
}

pub async fn get_supported_quant_types(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let types = state.api.supported_quant_types();
    Ok(json!({
        "success": true,
        "quant_types": types
    }))
}

pub async fn get_backend_status(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let status = state.api.backend_status();
    Ok(json!({
        "success": true,
        "backends": status
    }))
}

pub async fn setup_quantization_backend(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let backend = require_str_param(params, "backend", "backend")?;
    let backend = match backend.as_str() {
        "llama_cpp" | "LlamaCpp" => pumas_library::conversion::QuantBackend::LlamaCpp,
        "nvfp4" | "Nvfp4" => pumas_library::conversion::QuantBackend::Nvfp4,
        "sherry" | "Sherry" => pumas_library::conversion::QuantBackend::Sherry,
        "python_conversion" | "PythonConversion" => {
            pumas_library::conversion::QuantBackend::PythonConversion
        }
        _ => {
            return Err(pumas_library::PumasError::InvalidParams {
                message: format!("Unknown quantization backend: {}", backend),
            });
        }
    };

    state.api.ensure_backend_environment(backend).await?;
    Ok(json!({
        "success": true
    }))
}
