//! Conversion manager for orchestrating model format conversions.
//!
//! Manages the Python virtual environment, spawns conversion subprocesses,
//! tracks progress, and registers converted models in the library.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use super::progress::ConversionProgressTracker;
use super::scripts;
use super::types::{
    ConversionDirection, ConversionProgress, ConversionRequest, ConversionSource,
    ConversionStatus, QuantOption, ScriptProgressLine,
};
use crate::cancel::CancellationToken;
use crate::model_library::{ModelImporter, ModelLibrary};
use crate::models::ModelMetadata;
use crate::{PumasError, Result};

/// Maximum number of concurrent conversions (to avoid OOM on large models).
const MAX_CONCURRENT: usize = 1;

/// Orchestrates model format conversions between GGUF and Safetensors.
pub struct ConversionManager {
    launcher_root: PathBuf,
    model_library: Arc<ModelLibrary>,
    model_importer: Arc<ModelImporter>,
    progress: ConversionProgressTracker,
    cancel_tokens: Mutex<HashMap<String, CancellationToken>>,
    /// Counter for generating unique conversion IDs.
    id_counter: Mutex<u64>,
}

impl ConversionManager {
    /// Create a new ConversionManager.
    pub fn new(
        launcher_root: PathBuf,
        model_library: Arc<ModelLibrary>,
        model_importer: Arc<ModelImporter>,
    ) -> Self {
        Self {
            launcher_root,
            model_library,
            model_importer,
            progress: ConversionProgressTracker::new(),
            cancel_tokens: Mutex::new(HashMap::new()),
            id_counter: Mutex::new(0),
        }
    }

    /// Check if the Python conversion environment is ready.
    pub fn is_environment_ready(&self) -> bool {
        scripts::venv_python(&self.launcher_root).exists()
    }

    /// Ensure the Python conversion environment is set up.
    ///
    /// Creates the virtual environment and installs required packages if needed.
    pub async fn ensure_environment(&self) -> Result<()> {
        // Deploy scripts to disk
        scripts::ensure_scripts_deployed(&self.launcher_root)?;

        let venv_path = scripts::venv_dir(&self.launcher_root);
        let python_path = scripts::venv_python(&self.launcher_root);

        if python_path.exists() {
            debug!("Conversion venv already exists at {}", venv_path.display());
            return Ok(());
        }

        info!("Creating conversion virtual environment at {}", venv_path.display());

        // Create venv
        let output = Command::new("python3")
            .args(["-m", "venv", &venv_path.to_string_lossy()])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to create venv: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::ConversionFailed {
                message: format!(
                    "Failed to create Python venv. Ensure python3 is installed. Error: {stderr}"
                ),
            });
        }

        // Upgrade pip
        let output = Command::new(&python_path)
            .args(["-m", "pip", "install", "--upgrade", "pip"])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to upgrade pip: {e}")))?;

        if !output.status.success() {
            warn!(
                "pip upgrade failed (non-fatal): {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Install requirements
        let requirements_path = scripts::scripts_dir(&self.launcher_root).join("requirements.txt");
        info!("Installing conversion dependencies...");

        let output = Command::new(&python_path)
            .args([
                "-m",
                "pip",
                "install",
                "-r",
                &requirements_path.to_string_lossy(),
            ])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to install dependencies: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::ConversionFailed {
                message: format!("Failed to install conversion dependencies: {stderr}"),
            });
        }

        info!("Conversion environment ready");
        Ok(())
    }

    /// Get the list of supported quantization types.
    pub fn supported_quant_types(&self) -> Vec<QuantOption> {
        vec![QuantOption {
            name: "F16".to_string(),
            description: "Half-precision float, no quality loss".to_string(),
            bits_per_weight: 16.0,
            recommended: true,
        }]
    }

    /// Start a model format conversion.
    ///
    /// Returns a conversion ID that can be used to track progress.
    pub async fn start_conversion(&self, request: ConversionRequest) -> Result<String> {
        // Check concurrent conversion limit
        let active_count = self
            .progress
            .list_all()
            .iter()
            .filter(|p| {
                matches!(
                    p.status,
                    ConversionStatus::SettingUp
                        | ConversionStatus::Validating
                        | ConversionStatus::Converting
                        | ConversionStatus::Writing
                        | ConversionStatus::Importing
                )
            })
            .count();

        if active_count >= MAX_CONCURRENT {
            return Err(PumasError::ConversionFailed {
                message: format!(
                    "Maximum concurrent conversions ({MAX_CONCURRENT}) reached. \
                     Wait for the current conversion to finish."
                ),
            });
        }

        // Validate source model exists
        let model = self
            .model_library
            .get_model(&request.model_id)
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: request.model_id.clone(),
            })?;

        // Validate format matches direction
        let metadata: ModelMetadata = serde_json::from_value(model.metadata.clone())
            .unwrap_or_default();

        // Generate conversion ID
        let conversion_id = {
            let mut counter = self.id_counter.lock().expect("id_counter lock poisoned");
            *counter += 1;
            format!("conv-{}", *counter)
        };

        // Create initial progress entry
        let progress = ConversionProgress {
            conversion_id: conversion_id.clone(),
            source_model_id: request.model_id.clone(),
            direction: request.direction,
            status: ConversionStatus::SettingUp,
            progress: Some(0.0),
            current_tensor: None,
            tensors_completed: None,
            tensors_total: None,
            bytes_written: None,
            estimated_output_size: None,
            target_quant: request.target_quant.clone(),
            error: None,
            output_model_id: None,
        };
        self.progress.insert(progress);

        // Create cancellation token
        let cancel_token = CancellationToken::new();
        {
            let mut tokens = self.cancel_tokens.lock().expect("cancel_tokens lock poisoned");
            tokens.insert(conversion_id.clone(), cancel_token.clone());
        }

        // Spawn the conversion task
        let conv_id = conversion_id.clone();
        let launcher_root = self.launcher_root.clone();
        let model_path = PathBuf::from(&model.path);
        let library = self.model_library.clone();
        let importer = self.model_importer.clone();
        let progress_tracker = &self.progress as *const ConversionProgressTracker;
        // SAFETY: ConversionManager is held in an Arc and lives for the duration of
        // the application. The progress tracker reference is valid as long as the manager exists.
        let progress_ref = unsafe { &*progress_tracker };
        let direction = request.direction;
        let target_quant = request.target_quant.clone();
        let source_model_id = request.model_id.clone();

        tokio::spawn(async move {
            let result = run_conversion(
                &conv_id,
                direction,
                &launcher_root,
                &model_path,
                &source_model_id,
                target_quant.as_deref(),
                metadata,
                progress_ref,
                &cancel_token,
                &library,
                &importer,
            )
            .await;

            if let Err(e) = result {
                error!("Conversion {} failed: {}", conv_id, e);
                progress_ref.set_error(&conv_id, e.to_string());
            }

            // Clean up cancel token
            // Note: We can't access self here, but the token will be dropped when the
            // HashMap entry is removed on the next cancel_conversion or list_conversions call
        });

        Ok(conversion_id)
    }

    /// Get progress for a specific conversion.
    pub fn get_progress(&self, conversion_id: &str) -> Option<ConversionProgress> {
        self.progress.get(conversion_id)
    }

    /// Cancel a running conversion.
    pub async fn cancel_conversion(&self, conversion_id: &str) -> Result<bool> {
        let token = {
            let tokens = self.cancel_tokens.lock().expect("cancel_tokens lock poisoned");
            tokens.get(conversion_id).cloned()
        };

        if let Some(token) = token {
            token.cancel();
            self.progress.set_status(conversion_id, ConversionStatus::Cancelled);
            info!("Cancelled conversion {}", conversion_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all tracked conversions (active and recently completed).
    pub fn list_conversions(&self) -> Vec<ConversionProgress> {
        self.progress.list_all()
    }

    /// Graceful shutdown: cancel all active conversions.
    pub async fn shutdown(&self) {
        let tokens: Vec<(String, CancellationToken)> = {
            let tokens = self.cancel_tokens.lock().expect("cancel_tokens lock poisoned");
            tokens
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        for (id, token) in tokens {
            token.cancel();
            self.progress.set_status(&id, ConversionStatus::Cancelled);
        }
    }
}

/// Execute the actual conversion in a spawned task.
async fn run_conversion(
    conversion_id: &str,
    direction: ConversionDirection,
    launcher_root: &Path,
    model_path: &Path,
    source_model_id: &str,
    target_quant: Option<&str>,
    source_metadata: ModelMetadata,
    progress: &ConversionProgressTracker,
    cancel_token: &CancellationToken,
    library: &ModelLibrary,
    _importer: &ModelImporter,
) -> Result<()> {
    // Ensure environment is ready
    let python_path = scripts::venv_python(launcher_root);
    if !python_path.exists() {
        return Err(PumasError::ConversionFailed {
            message: "Conversion environment not set up. Call setup_conversion_environment first."
                .to_string(),
        });
    }

    // Deploy scripts (in case they were updated)
    scripts::ensure_scripts_deployed(launcher_root)?;

    progress.set_status(conversion_id, ConversionStatus::Validating);

    // Find model files in the source directory
    let model_files = find_model_files(model_path, direction)?;
    if model_files.is_empty() {
        return Err(PumasError::ConversionFailed {
            message: format!("No {} files found in {}", source_extension(direction), model_path.display()),
        });
    }

    // Determine output location
    let output_dir = determine_output_dir(model_path, direction)?;
    let temp_dir = output_dir.with_extension("converting");

    // Clean up any previous temp dir
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| PumasError::io("creating temp conversion dir", &temp_dir, e))?;

    // Build script command
    let scripts_dir = scripts::scripts_dir(launcher_root);
    let (script_name, args) = match direction {
        ConversionDirection::GgufToSafetensors => {
            let mut args = vec!["--output-dir".to_string(), temp_dir.to_string_lossy().to_string()];
            args.push("--input".to_string());
            for f in &model_files {
                args.push(f.to_string_lossy().to_string());
            }
            ("convert_gguf_to_safetensors.py", args)
        }
        ConversionDirection::SafetensorsToGguf => {
            let output_file = temp_dir.join("model.gguf");
            let mut args = vec![
                "--output".to_string(),
                output_file.to_string_lossy().to_string(),
            ];

            // Add config.json path if available
            let config_path = model_path.join("config.json");
            if config_path.exists() {
                args.push("--config".to_string());
                args.push(config_path.to_string_lossy().to_string());
            }

            if let Some(quant) = target_quant {
                args.push("--quant".to_string());
                args.push(quant.to_string());
            }

            args.push("--input".to_string());
            for f in &model_files {
                args.push(f.to_string_lossy().to_string());
            }
            ("convert_safetensors_to_gguf.py", args)
        }
    };

    let script_path = scripts_dir.join(script_name);

    info!(
        "Starting conversion {}: {} with {} input file(s)",
        conversion_id,
        script_name,
        model_files.len()
    );

    // Spawn Python subprocess
    let mut child = Command::new(&python_path)
        .arg(&script_path)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| PumasError::ConversionFailed {
            message: format!("Failed to spawn conversion process: {e}"),
        })?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let mut reader = BufReader::new(stdout).lines();

    // Read progress lines from stdout, checking cancellation between lines
    loop {
        if cancel_token.is_cancelled() {
            child.kill().await.ok();
            std::fs::remove_dir_all(&temp_dir).ok();
            return Err(PumasError::ConversionCancelled);
        }

        match reader.next_line().await {
            Ok(Some(line)) => {
                if let Ok(script_progress) = serde_json::from_str::<ScriptProgressLine>(&line) {
                    progress.update_from_script(conversion_id, &script_progress);
                } else {
                    debug!("Non-JSON output from conversion script: {}", line);
                }
            }
            Ok(None) => break, // EOF
            Err(e) => {
                warn!("Error reading conversion output: {}", e);
                break;
            }
        }
    }

    // Wait for process exit
    let status = child.wait().await.map_err(|e| PumasError::ConversionFailed {
        message: format!("Conversion process error: {e}"),
    })?;

    if !status.success() {
        // Clean up temp dir
        std::fs::remove_dir_all(&temp_dir).ok();
        // Check if we already have an error from the script
        if let Some(p) = progress.get(conversion_id) {
            if p.status == ConversionStatus::Error {
                return Err(PumasError::ConversionFailed {
                    message: p.error.unwrap_or_else(|| "Conversion script failed".to_string()),
                });
            }
        }
        return Err(PumasError::ConversionFailed {
            message: format!("Conversion process exited with status: {status}"),
        });
    }

    // Rename temp dir to final output dir
    if output_dir.exists() {
        // Add suffix to avoid collision
        let mut suffix = 2u32;
        let base = output_dir.clone();
        let mut final_dir = base.clone();
        while final_dir.exists() {
            final_dir = base.with_file_name(format!(
                "{}-v{}",
                base.file_name().unwrap_or_default().to_string_lossy(),
                suffix
            ));
            suffix += 1;
        }
        std::fs::rename(&temp_dir, &final_dir)
            .map_err(|e| PumasError::io("renaming conversion output", &temp_dir, e))?;
    } else {
        std::fs::rename(&temp_dir, &output_dir)
            .map_err(|e| PumasError::io("renaming conversion output", &temp_dir, e))?;
    }

    // Build conversion source metadata
    let is_dequantized = direction == ConversionDirection::GgufToSafetensors
        && target_quant.map_or(true, |q| q != "F16" && q != "F32");
    let conversion_source = ConversionSource {
        source_model_id: source_model_id.to_string(),
        source_format: source_extension(direction).to_string(),
        source_quant: None, // TODO: detect from GGUF metadata
        target_format: target_extension(direction).to_string(),
        target_quant: target_quant.map(|s| s.to_string()),
        was_dequantized: is_dequantized,
        conversion_date: chrono::Utc::now().to_rfc3339(),
    };

    // Write metadata.json for the converted model
    progress.set_status(conversion_id, ConversionStatus::Importing);

    let final_output_dir = if output_dir.exists() {
        output_dir.clone()
    } else {
        // Find the actual dir (may have been renamed with suffix)
        output_dir.clone()
    };

    let converted_metadata = ModelMetadata {
        model_id: source_metadata.model_id.clone(),
        family: source_metadata.family.clone(),
        model_type: source_metadata.model_type.clone(),
        official_name: source_metadata.official_name.as_ref().map(|name| {
            format!(
                "{} ({})",
                name,
                target_extension(direction).to_uppercase()
            )
        }),
        tags: Some(
            source_metadata
                .tags
                .unwrap_or_default()
                .into_iter()
                .chain(["converted".to_string()])
                .collect(),
        ),
        match_source: Some("conversion".to_string()),
        conversion_source: Some(conversion_source),
        ..Default::default()
    };

    // Save metadata and index
    library
        .save_metadata(&final_output_dir, &converted_metadata)
        .await?;
    library.index_model_dir(&final_output_dir).await?;

    let output_model_id = library
        .get_relative_path(&final_output_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| final_output_dir.to_string_lossy().to_string());

    progress.set_output_model_id(conversion_id, output_model_id);
    progress.set_status(conversion_id, ConversionStatus::Completed);

    info!("Conversion {} completed successfully", conversion_id);
    Ok(())
}

/// Find model files of the appropriate format in the model directory.
fn find_model_files(model_path: &Path, direction: ConversionDirection) -> Result<Vec<PathBuf>> {
    let ext = source_extension(direction);
    let mut files = Vec::new();

    if model_path.is_dir() {
        for entry in std::fs::read_dir(model_path)
            .map_err(|e| PumasError::io("reading model directory", model_path, e))?
        {
            let entry =
                entry.map_err(|e| PumasError::io("reading directory entry", model_path, e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                files.push(path);
            }
        }
        files.sort();
    }

    Ok(files)
}

/// Get the source file extension for a conversion direction.
fn source_extension(direction: ConversionDirection) -> &'static str {
    match direction {
        ConversionDirection::GgufToSafetensors => "gguf",
        ConversionDirection::SafetensorsToGguf => "safetensors",
    }
}

/// Get the target file extension for a conversion direction.
fn target_extension(direction: ConversionDirection) -> &'static str {
    match direction {
        ConversionDirection::GgufToSafetensors => "safetensors",
        ConversionDirection::SafetensorsToGguf => "gguf",
    }
}

/// Determine the output directory for a conversion.
fn determine_output_dir(model_path: &Path, direction: ConversionDirection) -> Result<PathBuf> {
    let dir_name = model_path
        .file_name()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: format!("Invalid model path: {}", model_path.display()),
        })?
        .to_string_lossy();

    let suffix = match direction {
        ConversionDirection::GgufToSafetensors => "safetensors",
        ConversionDirection::SafetensorsToGguf => "gguf-f16",
    };

    let output_name = format!("{}-{}", dir_name, suffix);
    let parent = model_path.parent().ok_or_else(|| PumasError::ConversionFailed {
        message: format!("Model path has no parent: {}", model_path.display()),
    })?;

    Ok(parent.join(output_name))
}
