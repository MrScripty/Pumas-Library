//! Sherry / AngelSlim quantization-aware training (QAT) backend.
//!
//! Produces 1.25-bit ternary quantized models via Tencent's AngelSlim framework.
//! Requires: GPU with sufficient VRAM, Python 3.10+, angelslim package.
//!
//! Unlike post-training quantization, Sherry performs actual QAT which takes
//! significantly longer but produces higher quality at extreme compression ratios.

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

/// Sherry QAT backend using Tencent AngelSlim.
pub struct SherryBackend {
    base_dir: PathBuf,
    pub(super) setup: std::sync::Arc<super::setup::SetupOwner>,
    pub(super) readiness: std::sync::Arc<super::readiness::ProbeOwner>,
}

impl SherryBackend {
    /// Create a new backend rooted under `{launcher_root}/launcher-data/sherry/`.
    pub fn new(launcher_root: &Path) -> Self {
        let base_dir = launcher_root.join("launcher-data").join("sherry");
        let python = base_dir.join("venv/bin/python");
        Self {
            readiness: std::sync::Arc::new(super::readiness::ProbeOwner::new(
                python.clone(),
                "sherry",
                super::backend_setup::SHERRY_IMPORTS,
                vec![(base_dir.join("sherry_qat.py"), false), (python, true)],
            )),
            setup: std::sync::Arc::new(super::setup::SetupOwner::for_backend(
                launcher_root.to_path_buf(),
                QuantBackend::Sherry,
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

    fn train_script(&self) -> PathBuf {
        self.base_dir.join("sherry_qat.py")
    }

    /// Close installer/probe admission and drain retained async work before
    /// stopping the host runtime. Finish caller-owned synchronous reads first.
    pub async fn shutdown_setup(&self) -> Result<()> {
        super::readiness::shutdown_backend(&self.setup, &self.readiness).await
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
            name: "Sherry-1.25bit".to_string(),
            description:
                "1.25-bit ternary QAT via AngelSlim (long training, best extreme compression)"
                    .to_string(),
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
                backend: "sherry".to_string(),
                message:
                    "Sherry QAT environment not set up. Call setup_quantization_backend first."
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
        let workspace = OutputWorkspace::prepare(&output_dir).await?;
        let temp_dir = workspace.staging_path().to_path_buf();

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

        let mut command = Command::new(self.venv_python());
        command.args(&args);
        script_process::run(
            &mut command,
            "sherry-qat",
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
        let backend = SherryBackend::new(Path::new("/app"));
        assert_eq!(backend.base_dir, PathBuf::from("/app/launcher-data/sherry"));
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
