use std::path::PathBuf;

use super::{OnnxModelPath, OnnxRuntimeError};

pub(super) fn resolve_package_file(
    model_path: &OnnxModelPath,
    file_name: &'static str,
    field: &'static str,
) -> Result<PathBuf, OnnxRuntimeError> {
    let mut search_dir = model_path.path().parent().ok_or_else(|| {
        OnnxRuntimeError::validation("path", "model path must have a parent directory")
    })?;

    loop {
        let candidate = search_dir.join(file_name);
        if candidate
            .try_exists()
            .map_err(|err| OnnxRuntimeError::path(field, format!("{file_name} is invalid"), err))?
        {
            let resolved = candidate.canonicalize().map_err(|err| {
                OnnxRuntimeError::path(field, format!("{file_name} is invalid"), err)
            })?;
            if !resolved.starts_with(model_path.root()) {
                return Err(OnnxRuntimeError::validation(
                    field,
                    format!("{file_name} must stay inside the configured model root"),
                ));
            }
            if !resolved.is_file() {
                return Err(OnnxRuntimeError::validation(
                    field,
                    format!("{file_name} must be a file"),
                ));
            }
            return Ok(resolved);
        }

        if search_dir == model_path.root() {
            break;
        }
        search_dir = search_dir.parent().ok_or_else(|| {
            OnnxRuntimeError::validation(
                field,
                format!(
                    "{file_name} must be in the model directory or an ancestor under the model root"
                ),
            )
        })?;
    }

    Err(OnnxRuntimeError::validation(
        field,
        format!("{file_name} must be in the model directory or an ancestor under the model root"),
    ))
}
