//! Resolve a local image projector shared by router and dedicated launches.

use std::path::{Path, PathBuf};

use crate::{PumasError, Result};

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
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.contains("mmproj") || !name.ends_with(".gguf") {
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
