//! Resolve a local image projector shared by router and dedicated launches.

use std::path::{Path, PathBuf};

use crate::{PumasError, Result};

/// llama.cpp names multimodal projector files `*mmproj*.gguf`. A projector is
/// a companion of model weights and is never itself the model to serve.
pub(crate) fn is_projector_file_name(file_name: &str) -> bool {
    let name = file_name.to_ascii_lowercase();
    name.contains("mmproj") && name.ends_with(".gguf")
}

/// Only associate a projector in the selected weights' directory. Never borrow
/// a projector from another model package or guess between quantizations.
pub(crate) fn resolve_sibling_mmproj(model_path: &Path) -> Result<Option<PathBuf>> {
    let directory = model_path
        .parent()
        .ok_or_else(|| PumasError::InvalidParams {
            message: "llama.cpp model path has no parent directory".into(),
        })?;
    let mut projectors = Vec::new();
    for entry in
        std::fs::read_dir(directory).map_err(|error| PumasError::io_with_path(error, directory))?
    {
        let entry = entry.map_err(|error| PumasError::io_with_path(error, directory))?;
        if !is_projector_file_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        if path == model_path {
            return Err(PumasError::InvalidParams {
                message: "llama.cpp model weights resolve to an image projector".into(),
            });
        }
        let metadata =
            std::fs::metadata(&path).map_err(|error| PumasError::io_with_path(error, &path))?;
        if metadata.is_file() {
            if path.to_string_lossy().contains(['\r', '\n']) {
                return Err(PumasError::InvalidParams {
                    message: "llama.cpp projector path contains a line break".into(),
                });
            }
            projectors.push(path);
        }
    }
    if projectors.len() > 1 {
        return Err(PumasError::InvalidParams {
            message: format!(
                "multiple mmproj GGUF files beside {}; keep one matching image projector in the model directory",
                model_path.display()
            ),
        });
    }
    Ok(projectors.pop())
}
