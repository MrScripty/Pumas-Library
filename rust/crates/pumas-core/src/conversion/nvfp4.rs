//! NVFP4 quantization backend via nvidia-modelopt / TensorRT-LLM.
//!
//! Produces FP4-quantized safetensors for NVIDIA Blackwell GPUs.
//! Requires: NVIDIA GPU with Blackwell architecture, Python 3.10+,
//! nvidia-modelopt, tensorrt-llm, and calibration data.

use std::path::{Path, PathBuf};

use tokio::process::Command;

use super::outputs::OutputWorkspace;
use super::pipeline;
use super::progress::ConversionProgressTracker;
use super::script_process;
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
    pub(super) setup: std::sync::Arc<super::setup::SetupOwner>,
}

impl Nvfp4Backend {
    /// Create a new backend rooted under `{launcher_root}/launcher-data/nvfp4/`.
    pub fn new(launcher_root: &Path) -> Self {
        Self {
            setup: std::sync::Arc::new(super::setup::SetupOwner::for_backend(
                launcher_root.to_path_buf(),
                QuantBackend::Nvfp4,
            )),
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

    /// Close installer admission and observe retained setup cleanup before
    /// stopping the host runtime. Dropping an ensure waiter does not stop setup.
    pub async fn shutdown_setup(&self) -> Result<()> {
        self.setup.shutdown().await
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
        self.setup.ensure().await
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
                message: format!(
                    "Source model path is not a directory: {}",
                    params.model_path.display()
                ),
            });
        }

        // Verify safetensors files exist
        if pipeline::list_files_with_extension(&params.model_path, "safetensors")
            .await?
            .is_empty()
        {
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
        let workspace = OutputWorkspace::prepare(&output_dir).await?;
        let temp_dir = workspace.staging_path().to_path_buf();

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

        let mut command = Command::new(self.venv_python());
        command.args(&args);
        script_process::run(
            &mut command,
            "nvfp4-quantize",
            conversion_id,
            progress,
            cancel_token,
        )
        .await?;

        // -- PHASE 4: CLEANUP --
        progress.update_pipeline(conversion_id, 2, 2, "Finalizing output");
        workspace.publish().await
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
        assert_eq!(backend.base_dir, PathBuf::from("/app/launcher-data/nvfp4"));
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
