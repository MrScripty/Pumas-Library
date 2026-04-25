//! Shared subprocess utilities for conversion and quantization pipelines.
//!
//! Provides reusable helpers for streaming subprocess output, waiting for
//! process exit, finalizing output directories, and writing quantized model
//! metadata. Used by both the existing Python-based conversion pipeline and
//! the llama.cpp quantization backend.

use std::path::Path;
use std::path::PathBuf;

use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, warn};

use super::progress::ConversionProgressTracker;
use super::types::{ConversionSource, ConversionStatus};
use crate::cancel::CancellationToken;
use crate::model_library::ModelLibrary;
use crate::models::ModelMetadata;
use crate::{PumasError, Result};

// ---------------------------------------------------------------------------
// Subprocess output streaming
// ---------------------------------------------------------------------------

/// Stream a subprocess's stderr, logging each line at debug level.
///
/// Checks cancellation between lines. Does not parse the output — use this
/// when you only need to drain stderr and watch for cancellation.
pub async fn stream_subprocess_stderr_lines(
    conversion_id: &str,
    child: &mut tokio::process::Child,
    _progress: &ConversionProgressTracker,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let stderr = child.stderr.take().expect("stderr was piped");
    let mut reader = BufReader::new(stderr).lines();

    loop {
        if cancel_token.is_cancelled() {
            child.kill().await.ok();
            return Err(PumasError::ConversionCancelled);
        }

        match reader.next_line().await {
            Ok(Some(line)) => {
                debug!("[{}] stderr: {}", conversion_id, line);
            }
            Ok(None) => break,
            Err(e) => {
                warn!("Error reading subprocess stderr: {}", e);
                break;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Process exit handling
// ---------------------------------------------------------------------------

/// Wait for a child process to exit and return an error on non-zero status.
pub async fn wait_and_check_exit(
    child: &mut tokio::process::Child,
    process_name: &str,
) -> Result<()> {
    let status = child
        .wait()
        .await
        .map_err(|e| PumasError::ConversionFailed {
            message: format!("{process_name} process error: {e}"),
        })?;

    if !status.success() {
        return Err(PumasError::ConversionFailed {
            message: format!(
                "{process_name} exited with status: {}",
                status.code().unwrap_or(-1)
            ),
        });
    }
    Ok(())
}

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

/// Remove any stale temp directory and recreate it for a fresh conversion run.
pub async fn prepare_temp_output_dir(temp_dir: &Path, context: &str) -> Result<()> {
    if fs::try_exists(temp_dir)
        .await
        .map_err(|e| PumasError::io("checking conversion temp dir", temp_dir, e))?
    {
        let _ = fs::remove_dir_all(temp_dir).await;
    }

    fs::create_dir_all(temp_dir)
        .await
        .map_err(|e| PumasError::io(context, temp_dir, e))?;
    Ok(())
}

/// Best-effort removal for temp output directories after cancellation or failure.
pub async fn cleanup_temp_output_dir(temp_dir: &Path) {
    let _ = fs::remove_dir_all(temp_dir).await;
}

/// Atomically rename `temp_dir` to `output_dir`.
///
/// If `output_dir` already exists, appends a `-v{N}` suffix to avoid collision.
pub async fn finalize_output_dir(temp_dir: &Path, output_dir: &Path) -> Result<()> {
    if fs::try_exists(output_dir)
        .await
        .map_err(|e| PumasError::io("checking quantization output dir", output_dir, e))?
    {
        let mut suffix = 2u32;
        let base = output_dir.to_path_buf();
        let mut final_dir = base.clone();
        while fs::try_exists(&final_dir)
            .await
            .map_err(|e| PumasError::io("checking quantization output dir", &final_dir, e))?
        {
            final_dir = base.with_file_name(format!(
                "{}-v{}",
                base.file_name().unwrap_or_default().to_string_lossy(),
                suffix
            ));
            suffix += 1;
        }
        fs::rename(temp_dir, &final_dir)
            .await
            .map_err(|e| PumasError::io("renaming quantization output", temp_dir, e))?;
    } else {
        fs::rename(temp_dir, output_dir)
            .await
            .map_err(|e| PumasError::io("renaming quantization output", temp_dir, e))?;
    }
    Ok(())
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
