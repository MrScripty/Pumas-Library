//! FP8 quantization backend via Transformers.
//!
//! Produces Transformers block-scaled FP8 Safetensors using CPU conversion.
//! Requires: CPU PyTorch and Transformers; conversion does not occupy the inference GPU.

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

/// FP8 quantization backend using Transformers.
pub struct Fp8Backend {
    base_dir: PathBuf,
    pub(super) setup: std::sync::Arc<super::setup::SetupOwner>,
    pub(super) readiness: std::sync::Arc<super::readiness::ProbeOwner>,
}

impl Fp8Backend {
    /// Create a new backend rooted under `{launcher_root}/launcher-data/fp8/`.
    pub fn new(launcher_root: &Path) -> Self {
        let base_dir = launcher_root.join("launcher-data").join("fp8");
        let python = base_dir.join("venv/bin/python");
        Self {
            readiness: std::sync::Arc::new(super::readiness::ProbeOwner::new(
                python.clone(),
                "fp8",
                super::backend_setup::FP8_IMPORTS,
                vec![(base_dir.join("quantize_fp8.py"), false), (python, true)],
            )),
            setup: std::sync::Arc::new(super::setup::SetupOwner::for_backend(
                launcher_root.to_path_buf(),
                QuantBackend::Fp8,
            )),
            base_dir,
        }
    }

    fn venv_dir(&self) -> PathBuf {
        self.base_dir.join("venv")
    }

    fn venv_python(&self) -> PathBuf {
        self.venv_dir().join("bin").join("python")
    }

    fn quantize_script(&self) -> PathBuf {
        self.base_dir.join("quantize_fp8.py")
    }

    /// Close installer/probe admission and drain retained async work before
    /// stopping the host runtime. Finish caller-owned synchronous reads first.
    pub async fn shutdown_setup(&self) -> Result<()> {
        super::readiness::shutdown_backend(&self.setup, &self.readiness).await
    }
}

#[async_trait::async_trait]
impl QuantizationBackend for Fp8Backend {
    fn name(&self) -> &str {
        "FP8 (Transformers)"
    }

    fn backend_id(&self) -> QuantBackend {
        QuantBackend::Fp8
    }

    fn is_ready(&self) -> bool {
        self.readiness.check_blocking().unwrap_or(false)
    }

    async fn is_ready_async(&self) -> Result<bool> {
        self.readiness.check().await
    }

    async fn ensure_environment(&self) -> Result<()> {
        self.setup.ensure().await
    }

    fn supported_quant_types(&self) -> Vec<QuantOption> {
        vec![QuantOption {
            name: "FP8".to_string(),
            description: "Transformers FP8 Safetensors (8-bit block-scaled weights)".to_string(),
            bits_per_weight: 8.0,
            recommended: false,
            backend: Some(QuantBackend::Fp8),
            imatrix_recommended: false,
        }]
    }

    async fn quantize(
        &self,
        params: &QuantizeParams,
        progress: &ConversionProgressTracker,
        cancel_token: &CancellationToken,
    ) -> Result<PathBuf> {
        super::options::validate_options(self, &params.target_quant, params.force_imatrix)?;
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
        cancel_token
            .check()
            .map_err(|_| PumasError::ConversionCancelled)?;
        let ready = self.is_ready_async().await?;
        cancel_token
            .check()
            .map_err(|_| PumasError::ConversionCancelled)?;
        if !ready {
            return Err(PumasError::QuantizationEnvNotReady {
                backend: "fp8".to_string(),
                message: "FP8 environment not set up. Call setup_quantization_backend first."
                    .to_string(),
            });
        }

        progress.update_pipeline(conversion_id, 1, 2, "Quantizing");
        progress.set_status(conversion_id, ConversionStatus::Quantizing);

        // -- PHASE 3: CREATE --
        let output_dir_name = format!(
            "{}-fp8",
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

        let args = vec![
            self.quantize_script().to_string_lossy().to_string(),
            "--model-dir".to_string(),
            params.model_path.to_string_lossy().to_string(),
            "--output-dir".to_string(),
            temp_dir.to_string_lossy().to_string(),
        ];

        let mut command = Command::new(self.venv_python());
        command.args(&args);
        script_process::run(
            &mut command,
            "fp8-quantize",
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
