//! Pure option admission shared by managed and direct quantization.

use super::types::{QuantBackend, QuantizationBackend};
use crate::{PumasError, Result};

pub(super) fn validate_options(
    backend: &dyn QuantizationBackend,
    target: &str,
    force_imatrix: bool,
) -> Result<()> {
    let id = backend.backend_id();
    if backend
        .supported_quant_types()
        .iter()
        .any(|option| option.backend == Some(id) && option.name == target)
    {
        if force_imatrix && id != QuantBackend::LlamaCpp {
            return Err(PumasError::InvalidParams {
                message: "force_imatrix is only supported by llama.cpp".into(),
            });
        }
        return Ok(());
    }
    // Preserve managed error labels, which differ from human-readable names.
    let name = match id {
        QuantBackend::LlamaCpp => "llama.cpp",
        QuantBackend::Nvfp4 => "nvfp4",
        QuantBackend::Sherry => "sherry",
        QuantBackend::PythonConversion => "python conversion",
    };
    Err(PumasError::InvalidParams {
        message: format!("Unsupported target quantization for {name}"),
    })
}
