//! Shared file discovery and metadata helpers for conversion pipelines.
//! Native execution and output publication have separate private owners.

use std::path::Path;
use std::path::PathBuf;

use tokio::fs;

use super::progress::ConversionProgressTracker;
use super::types::{ConversionSource, ConversionStatus};
use crate::model_library::ModelLibrary;
use crate::models::ModelMetadata;
use crate::{PumasError, Result};

// ---------------------------------------------------------------------------
// Output directory management
// ---------------------------------------------------------------------------

/// List files with a matching extension from a model directory.
pub async fn list_files_with_extension(model_path: &Path, ext: &str) -> Result<Vec<PathBuf>> {
    let mut entries = fs::read_dir(model_path)
        .await
        .map_err(|e| PumasError::io("reading model directory", model_path, e))?;
    let mut files = Vec::new();

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| PumasError::io("reading directory entry", model_path, e))?
    {
        let path = entry.path();
        if path.extension().and_then(|entry_ext| entry_ext.to_str()) == Some(ext) {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// Metadata helpers
// ---------------------------------------------------------------------------

/// Write `ConversionSource` metadata for a quantized model and index it.
///
/// # Preconditions
/// - `output_dir` must contain the quantized model file(s).
/// - `source_metadata` should be the metadata of the source model.
///
/// # Postconditions
/// - `metadata.json` written in `output_dir`.
/// - Model indexed in the library.
#[allow(clippy::too_many_arguments)]
pub async fn write_quantized_metadata(
    conversion_id: &str,
    source_model_id: &str,
    source_format: &str,
    target_format: &str,
    target_quant: &str,
    source_metadata: &ModelMetadata,
    output_dir: &Path,
    progress: &ConversionProgressTracker,
    library: &ModelLibrary,
) -> Result<String> {
    progress.set_status(conversion_id, ConversionStatus::Importing);

    let conversion_source = ConversionSource {
        source_model_id: source_model_id.to_string(),
        source_format: source_format.to_string(),
        source_quant: None,
        target_format: target_format.to_string(),
        target_quant: Some(target_quant.to_string()),
        was_dequantized: false,
        conversion_date: chrono::Utc::now().to_rfc3339(),
    };

    let converted_metadata = ModelMetadata {
        model_id: source_metadata.model_id.clone(),
        family: source_metadata.family.clone(),
        model_type: source_metadata.model_type.clone(),
        official_name: source_metadata
            .official_name
            .as_ref()
            .map(|name| format!("{} (GGUF {})", name, target_quant)),
        tags: Some(
            source_metadata
                .tags
                .clone()
                .unwrap_or_default()
                .into_iter()
                .chain(["quantized".to_string()])
                .collect(),
        ),
        match_source: Some("quantization".to_string()),
        conversion_source: Some(conversion_source),
        ..Default::default()
    };

    library
        .save_metadata(output_dir, &converted_metadata)
        .await?;
    library.index_model_dir(output_dir).await?;

    let output_model_id = library
        .get_relative_path(output_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| output_dir.to_string_lossy().to_string());

    progress.set_output_model_id(conversion_id, output_model_id.clone());
    progress.set_status(conversion_id, ConversionStatus::Completed);

    Ok(output_model_id)
}
