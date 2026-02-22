//! Sherry / AngelSlim quantization-aware training (QAT) backend.
//!
//! Produces 1.25-bit ternary quantized models via Tencent's AngelSlim framework.
//! Requires: GPU with sufficient VRAM, Python 3.10+, angelslim package.
//!
//! Unlike post-training quantization, Sherry performs actual QAT which takes
//! significantly longer but produces higher quality at extreme compression ratios.

use std::path::{Path, PathBuf};

use tokio::process::Command;
use tracing::{debug, info, warn};

use super::pipeline;
use super::progress::ConversionProgressTracker;
use super::types::{
    ConversionStatus, QuantBackend, QuantOption, QuantizationBackend, QuantizeParams,
};
use crate::cancel::CancellationToken;
use crate::{PumasError, Result};

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// Sherry QAT backend using Tencent AngelSlim.
pub struct SherryBackend {
    base_dir: PathBuf,
}

impl SherryBackend {
    pub fn new(launcher_root: &Path) -> Self {
        Self {
            base_dir: launcher_root.join("launcher-data").join("sherry"),
        }
    }

    fn venv_dir(&self) -> PathBuf {
        self.base_dir.join("venv")
    }

    fn venv_python(&self) -> PathBuf {
        self.venv_dir().join("bin").join("python")
    }

    fn train_script(&self) -> PathBuf {
        self.base_dir.join("sherry_qat.py")
    }

    /// Deploy the Sherry QAT script to the backend directory.
    fn deploy_script(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)
            .map_err(|e| PumasError::io("creating sherry dir", &self.base_dir, e))?;

        let script = include_str!("sherry_script.py");
        std::fs::write(self.train_script(), script)
            .map_err(|e| PumasError::io("writing sherry script", &self.train_script(), e))?;
        Ok(())
    }

    /// Create the Python venv and install AngelSlim dependencies.
    async fn setup_venv(&self) -> Result<()> {
        let venv_dir = self.venv_dir();
        let python = self.venv_python();

        if python.exists() {
            debug!("Sherry venv already exists at {}", venv_dir.display());
            return Ok(());
        }

        info!(
            "Creating Sherry QAT virtual environment at {}",
            venv_dir.display()
        );

        let output = Command::new("python3")
            .args(["-m", "venv", &venv_dir.to_string_lossy()])
            .output()
            .await
            .map_err(|e| PumasError::QuantizationEnvNotReady {
                backend: "sherry".to_string(),
                message: format!("Failed to create venv: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::QuantizationEnvNotReady {
                backend: "sherry".to_string(),
                message: format!("Failed to create venv: {stderr}"),
            });
        }

        // Upgrade pip
        let output = Command::new(&python)
            .args(["-m", "pip", "install", "--upgrade", "pip"])
            .output()
            .await
            .map_err(|e| PumasError::QuantizationEnvNotReady {
                backend: "sherry".to_string(),
                message: format!("Failed to upgrade pip: {e}"),
            })?;

        if !output.status.success() {
            warn!(
                "Sherry pip upgrade failed (non-fatal): {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Install AngelSlim and dependencies
        info!("Installing AngelSlim dependencies...");
        let output = Command::new(&python)
            .args([
                "-m", "pip", "install",
                "angelslim",
                "transformers",
                "torch",
                "safetensors",
                "datasets",
                "accelerate",
                "bitsandbytes",
            ])
            .output()
            .await
            .map_err(|e| PumasError::QuantizationEnvNotReady {
                backend: "sherry".to_string(),
                message: format!("Failed to install dependencies: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::QuantizationEnvNotReady {
                backend: "sherry".to_string(),
                message: format!("Failed to install angelslim: {stderr}"),
            });
        }

        info!("Sherry QAT environment ready");
        Ok(())
    }
}

#[async_trait::async_trait]
impl QuantizationBackend for SherryBackend {
    fn name(&self) -> &str {
        "Sherry QAT (AngelSlim)"
    }

    fn backend_id(&self) -> QuantBackend {
        QuantBackend::Sherry
    }

    fn is_ready(&self) -> bool {
        self.venv_python().exists() && self.train_script().exists()
    }

    async fn ensure_environment(&self) -> Result<()> {
        self.deploy_script()?;
        self.setup_venv().await
    }

    fn supported_quant_types(&self) -> Vec<QuantOption> {
        vec![QuantOption {
            name: "Sherry-1.25bit".to_string(),
            description: "1.25-bit ternary QAT via AngelSlim (long training, best extreme compression)".to_string(),
            bits_per_weight: 1.25,
            recommended: false,
            backend: Some(QuantBackend::Sherry),
            imatrix_recommended: false,
        }]
    }

    async fn quantize(
        &self,
        params: &QuantizeParams,
        progress: &ConversionProgressTracker,
        cancel_token: &CancellationToken,
    ) -> Result<PathBuf> {
        let conversion_id = &params.conversion_id;

        // -- PHASE 1: GATHER --
        if !params.model_path.is_dir() {
            return Err(PumasError::ConversionFailed {
                message: format!(
                    "Source model path is not a directory: {}",
                    params.model_path.display()
                ),
            });
        }

        // Verify safetensors files exist
        let has_safetensors = std::fs::read_dir(&params.model_path)
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext == "safetensors")
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if !has_safetensors {
            return Err(PumasError::ConversionFailed {
                message: "No safetensors files found in source model directory".to_string(),
            });
        }

        // -- PHASE 2: VALIDATE --
        if !self.is_ready() {
            return Err(PumasError::QuantizationEnvNotReady {
                backend: "sherry".to_string(),
                message: "Sherry QAT environment not set up. Call setup_quantization_backend first."
                    .to_string(),
            });
        }

        progress.update_pipeline(conversion_id, 1, 2, "Quantization-aware training");
        progress.set_status(conversion_id, ConversionStatus::Training);

        // -- PHASE 3: CREATE --
        let output_dir_name = format!(
            "{}-sherry-1.25bit",
            params
                .model_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let output_dir = params
            .model_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&output_dir_name);
        let temp_dir = output_dir.with_extension("converting");

        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir).ok();
        }
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| PumasError::io("creating sherry temp dir", &temp_dir, e))?;

        let mut args = vec![
            self.train_script().to_string_lossy().to_string(),
            "--model-dir".to_string(),
            params.model_path.to_string_lossy().to_string(),
            "--output-dir".to_string(),
            temp_dir.to_string_lossy().to_string(),
        ];

        if let Some(ref cal_file) = params.calibration_file {
            args.push("--calibration-file".to_string());
            args.push(cal_file.to_string_lossy().to_string());
        }

        let mut child = Command::new(self.venv_python())
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| PumasError::ConversionFailed {
                message: format!("Failed to spawn Sherry QAT process: {e}"),
            })?;

        // Stream stderr for progress
        pipeline::stream_subprocess_stderr_lines(
            conversion_id,
            &mut child,
            progress,
            cancel_token,
        )
        .await?;

        pipeline::wait_and_check_exit(&mut child, "sherry-qat").await?;

        // -- PHASE 4: CLEANUP --
        progress.update_pipeline(conversion_id, 2, 2, "Finalizing output");
        pipeline::finalize_output_dir(&temp_dir, &output_dir)?;

        Ok(output_dir)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_construction() {
        let backend = SherryBackend::new(Path::new("/app"));
        assert_eq!(
            backend.base_dir,
            PathBuf::from("/app/launcher-data/sherry")
        );
        assert_eq!(
            backend.venv_python(),
            PathBuf::from("/app/launcher-data/sherry/venv/bin/python")
        );
    }

    #[test]
    fn test_not_ready_without_setup() {
        let backend = SherryBackend::new(Path::new("/nonexistent"));
        assert!(!backend.is_ready());
    }

    #[test]
    fn test_quant_options() {
        let backend = SherryBackend::new(Path::new("/app"));
        let options = backend.supported_quant_types();
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].name, "Sherry-1.25bit");
        assert_eq!(options[0].bits_per_weight, 1.25);
        assert_eq!(options[0].backend, Some(QuantBackend::Sherry));
    }

    #[test]
    fn test_backend_status() {
        let backend = SherryBackend::new(Path::new("/app"));
        assert_eq!(backend.name(), "Sherry QAT (AngelSlim)");
        assert_eq!(backend.backend_id(), QuantBackend::Sherry);
    }
}
