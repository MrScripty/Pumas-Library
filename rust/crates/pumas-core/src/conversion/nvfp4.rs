//! NVFP4 quantization backend via nvidia-modelopt / TensorRT-LLM.
//!
//! Produces FP4-quantized safetensors for NVIDIA Blackwell GPUs.
//! Requires: NVIDIA GPU with Blackwell architecture, Python 3.10+,
//! nvidia-modelopt, tensorrt-llm, and calibration data.

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

/// NVFP4 quantization backend using nvidia-modelopt.
pub struct Nvfp4Backend {
    base_dir: PathBuf,
}

impl Nvfp4Backend {
    /// Create a new backend rooted under `{launcher_root}/launcher-data/nvfp4/`.
    pub fn new(launcher_root: &Path) -> Self {
        Self {
            base_dir: launcher_root.join("launcher-data").join("nvfp4"),
        }
    }

    fn venv_dir(&self) -> PathBuf {
        self.base_dir.join("venv")
    }

    fn venv_python(&self) -> PathBuf {
        self.venv_dir().join("bin").join("python")
    }

    fn quantize_script(&self) -> PathBuf {
        self.base_dir.join("quantize_nvfp4.py")
    }

    /// Deploy the NVFP4 quantization script to the backend directory.
    fn deploy_script(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)
            .map_err(|e| PumasError::io("creating nvfp4 dir", &self.base_dir, e))?;

        let script = include_str!("nvfp4_script.py");
        std::fs::write(self.quantize_script(), script)
            .map_err(|e| PumasError::io("writing nvfp4 script", &self.quantize_script(), e))?;
        Ok(())
    }

    /// Create the Python venv and install nvidia-modelopt dependencies.
    async fn setup_venv(&self) -> Result<()> {
        let venv_dir = self.venv_dir();
        let python = self.venv_python();

        if python.exists() {
            debug!("NVFP4 venv already exists at {}", venv_dir.display());
            return Ok(());
        }

        info!("Creating NVFP4 virtual environment at {}", venv_dir.display());

        let output = Command::new("python3")
            .args(["-m", "venv", &venv_dir.to_string_lossy()])
            .output()
            .await
            .map_err(|e| PumasError::QuantizationEnvNotReady {
                backend: "nvfp4".to_string(),
                message: format!("Failed to create venv: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::QuantizationEnvNotReady {
                backend: "nvfp4".to_string(),
                message: format!("Failed to create venv: {stderr}"),
            });
        }

        // Upgrade pip
        let output = Command::new(&python)
            .args(["-m", "pip", "install", "--upgrade", "pip"])
            .output()
            .await
            .map_err(|e| PumasError::QuantizationEnvNotReady {
                backend: "nvfp4".to_string(),
                message: format!("Failed to upgrade pip: {e}"),
            })?;

        if !output.status.success() {
            warn!(
                "NVFP4 pip upgrade failed (non-fatal): {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Install nvidia-modelopt and dependencies
        info!("Installing nvidia-modelopt dependencies...");
        let output = Command::new(&python)
            .args([
                "-m", "pip", "install",
                "nvidia-modelopt[all]",
                "transformers",
                "torch",
                "safetensors",
                "datasets",
                "accelerate",
            ])
            .output()
            .await
            .map_err(|e| PumasError::QuantizationEnvNotReady {
                backend: "nvfp4".to_string(),
                message: format!("Failed to install dependencies: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::QuantizationEnvNotReady {
                backend: "nvfp4".to_string(),
                message: format!("Failed to install nvidia-modelopt: {stderr}"),
            });
        }

        info!("NVFP4 environment ready");
        Ok(())
    }
}

#[async_trait::async_trait]
impl QuantizationBackend for Nvfp4Backend {
    fn name(&self) -> &str {
        "NVFP4 (nvidia-modelopt)"
    }

    fn backend_id(&self) -> QuantBackend {
        QuantBackend::Nvfp4
    }

    fn is_ready(&self) -> bool {
        self.venv_python().exists() && self.quantize_script().exists()
    }

    async fn ensure_environment(&self) -> Result<()> {
        self.deploy_script()?;
        self.setup_venv().await
    }

    fn supported_quant_types(&self) -> Vec<QuantOption> {
        vec![QuantOption {
            name: "NVFP4".to_string(),
            description: "NVIDIA FP4 for Blackwell GPUs (4-bit, calibration required)".to_string(),
            bits_per_weight: 4.0,
            recommended: false,
            backend: Some(QuantBackend::Nvfp4),
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
                message: format!("Source model path is not a directory: {}", params.model_path.display()),
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
                backend: "nvfp4".to_string(),
                message: "NVFP4 environment not set up. Call setup_quantization_backend first."
                    .to_string(),
            });
        }

        progress.update_pipeline(conversion_id, 1, 2, "Calibrating & quantizing");
        progress.set_status(conversion_id, ConversionStatus::Calibrating);

        // -- PHASE 3: CREATE --
        let output_dir_name = format!(
            "{}-nvfp4",
            params.model_path.file_name().unwrap_or_default().to_string_lossy()
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
            .map_err(|e| PumasError::io("creating nvfp4 temp dir", &temp_dir, e))?;

        // Build calibration dataset arg if provided
        let mut args = vec![
            self.quantize_script().to_string_lossy().to_string(),
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
                message: format!("Failed to spawn NVFP4 quantization process: {e}"),
            })?;

        // Stream stderr for progress
        pipeline::stream_subprocess_stderr_lines(
            conversion_id,
            &mut child,
            progress,
            cancel_token,
        )
        .await?;

        pipeline::wait_and_check_exit(&mut child, "nvfp4-quantize").await?;

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
        let backend = Nvfp4Backend::new(Path::new("/app"));
        assert_eq!(
            backend.base_dir,
            PathBuf::from("/app/launcher-data/nvfp4")
        );
        assert_eq!(
            backend.venv_python(),
            PathBuf::from("/app/launcher-data/nvfp4/venv/bin/python")
        );
    }

    #[test]
    fn test_not_ready_without_setup() {
        let backend = Nvfp4Backend::new(Path::new("/nonexistent"));
        assert!(!backend.is_ready());
    }

    #[test]
    fn test_quant_options() {
        let backend = Nvfp4Backend::new(Path::new("/app"));
        let options = backend.supported_quant_types();
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].name, "NVFP4");
        assert_eq!(options[0].bits_per_weight, 4.0);
        assert_eq!(options[0].backend, Some(QuantBackend::Nvfp4));
    }

    #[test]
    fn test_backend_status() {
        let backend = Nvfp4Backend::new(Path::new("/app"));
        assert_eq!(backend.name(), "NVFP4 (nvidia-modelopt)");
        assert_eq!(backend.backend_id(), QuantBackend::Nvfp4);
    }
}
