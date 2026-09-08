//! Conversion manager for orchestrating model format conversions and quantization.
//!
//! Manages the Python virtual environment for format conversions, dispatches
//! quantization operations to registered backends, tracks progress, and
//! registers output models in the library.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::fs;
use tokio::process::Command;
use tracing::info;

use super::llama_cpp::LlamaCppBackend;
use super::nvfp4::Nvfp4Backend;
use super::outputs::OutputWorkspace;
use super::pipeline;
use super::progress::ConversionProgressTracker;
use super::script_process;
use super::scripts;
use super::sherry::SherryBackend;
use super::types::{
    BackendStatus, ConversionDirection, ConversionProgress, ConversionRequest, ConversionSource,
    ConversionStatus, QuantBackend, QuantOption, QuantizationBackend, QuantizeParams,
};
use crate::cancel::CancellationToken;
use crate::model_library::{ModelImporter, ModelLibrary};
use crate::models::ModelMetadata;
use crate::{PumasError, Result};

/// Maximum number of concurrent conversions (to avoid OOM on large models).
const MAX_CONCURRENT: usize = 1;

#[cfg(all(test, unix))]
mod output_tests;

#[cfg(test)]
mod admission_tests;

#[cfg(all(test, target_os = "linux"))]
mod setup_tests;

#[cfg(all(test, target_os = "linux"))]
mod probe_tests;

/// Readiness must fit within interactive status requests even for a stuck interpreter.
const ENVIRONMENT_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const ENVIRONMENT_PROBE_IMPORTS: &str = "import numpy, sentencepiece; from gguf import GGUFReader, GGUFWriter; from safetensors import safe_open; from safetensors.numpy import save_file";

pub(super) fn probe_conversion_environment(
    python: &Path,
    timeout: std::time::Duration,
) -> Result<bool> {
    use std::process::Stdio;
    let mut child = match std::process::Command::new(python)
        .args(["-I", "-B", "-c", ENVIRONMENT_PROBE_IMPORTS])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(PumasError::io(
                "probing conversion environment",
                python,
                error,
            ))
        }
    };
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(
                    std::time::Duration::from_millis(20)
                        .min(timeout.saturating_sub(started.elapsed())),
                );
            }
            Ok(None) => {
                let stopped = child.kill();
                let reaped = child.wait();
                stopped.map_err(|error| {
                    PumasError::io("stopping conversion readiness probe", python, error)
                })?;
                reaped.map_err(|error| {
                    PumasError::io("reaping conversion readiness probe", python, error)
                })?;
                return Ok(false);
            }
            Err(error) => {
                // Retain process ownership through cleanup before reporting the
                // original observation error; never leave a probe running.
                let _ = child.kill();
                let _ = child.wait();
                return Err(PumasError::io(
                    "observing conversion readiness probe",
                    python,
                    error,
                ));
            }
        }
    }
}

/// Orchestrates model format conversions and quantization.
pub struct ConversionManager {
    setup: super::setup::SetupOwner,
    backend_setups: Vec<Arc<super::setup::SetupOwner>>,
    backend_probes: Vec<Arc<super::readiness::ProbeOwner>>,
    launcher_root: PathBuf,
    model_library: Arc<ModelLibrary>,
    model_importer: Arc<ModelImporter>,
    progress: Arc<ConversionProgressTracker>,
    workers: super::workers::WorkerOwner,
    /// Counter for generating unique conversion IDs.
    id_counter: Mutex<u64>,
    /// Registered quantization backends (strategy pattern).
    backends: Vec<Arc<dyn QuantizationBackend>>,
}

impl ConversionManager {
    /// Create a new ConversionManager with all quantization backends registered.
    pub fn new(
        launcher_root: PathBuf,
        model_library: Arc<ModelLibrary>,
        model_importer: Arc<ModelImporter>,
    ) -> Self {
        let llama = Arc::new(LlamaCppBackend::new(&launcher_root));
        let nvfp4 = Arc::new(Nvfp4Backend::new(&launcher_root));
        let sherry = Arc::new(SherryBackend::new(&launcher_root));
        let backend_setups = vec![
            llama.setup.clone(),
            nvfp4.setup.clone(),
            sherry.setup.clone(),
        ];
        let backend_probes = vec![
            llama.readiness.clone(),
            nvfp4.readiness.clone(),
            sherry.readiness.clone(),
        ];
        let backends: Vec<Arc<dyn QuantizationBackend>> = vec![llama, nvfp4, sherry];

        let progress = Arc::new(ConversionProgressTracker::new());
        Self {
            setup: super::setup::SetupOwner::new(launcher_root.clone()),
            backend_setups,
            backend_probes,
            launcher_root,
            model_library,
            model_importer,
            workers: super::workers::WorkerOwner::new(Arc::clone(&progress), MAX_CONCURRENT),
            progress,
            id_counter: Mutex::new(0),
            backends,
        }
    }

    // -----------------------------------------------------------------------
    // Python conversion environment (existing)
    // -----------------------------------------------------------------------

    /// Probe required imports with the conversion interpreter (bounded to five seconds).
    /// This boolean surface reports unavailable inspection as not ready.
    pub fn is_environment_ready(&self) -> bool {
        probe_conversion_environment(
            &scripts::venv_python(&self.launcher_root),
            ENVIRONMENT_PROBE_TIMEOUT,
        )
        .unwrap_or(false)
    }

    /// Check if the Python conversion environment is ready on a blocking task.
    pub async fn is_environment_ready_async(&self) -> Result<bool> {
        let launcher_root = self.launcher_root.clone();
        tokio::task::spawn_blocking(move || {
            probe_conversion_environment(
                &scripts::venv_python(&launcher_root),
                ENVIRONMENT_PROBE_TIMEOUT,
            )
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join conversion environment readiness task: {}",
                err
            ))
        })?
    }

    /// Ensure the Python conversion environment is set up.
    ///
    /// Creates the virtual environment and installs required packages if needed.
    pub async fn ensure_environment(&self) -> Result<()> {
        self.setup.ensure().await
    }

    /// Start base Python setup or return the latest retained operation.
    /// `None` never retries a retained operation. A matching terminal operation
    /// ID explicitly admits one successor; stale IDs return the current record.
    /// IDs must be canonical lower-case hyphenated UUIDs. A retry token without
    /// a retained record is invalid. Records are owner/process-lifetime only.
    pub async fn start_conversion_setup(
        &self,
        expected_previous_operation_id: Option<&str>,
    ) -> Result<super::ConversionSetupSnapshot> {
        self.setup
            .start_or_get(expected_previous_operation_id)
            .await
    }

    /// Inspect the latest base Python setup without starting work or touching disk.
    pub fn get_conversion_setup(&self) -> Option<super::ConversionSetupSnapshot> {
        self.setup.snapshot()
    }

    fn backend_setup(&self, backend: QuantBackend) -> Result<&super::setup::SetupOwner> {
        if backend == QuantBackend::PythonConversion {
            return Ok(&self.setup);
        }
        self.backend_setups
            .iter()
            .find(|setup| setup.backend_id() == backend)
            .map(Arc::as_ref)
            .ok_or_else(|| PumasError::InvalidParams {
                message: format!("No managed setup owner for backend: {backend:?}"),
            })
    }

    /// Start or inspect setup for a built-in backend, including PythonConversion.
    /// Shares the owner used by the corresponding ensure method and shutdown.
    /// `None` starts only when no record exists; it never retries retained work.
    /// Only a matching terminal operation ID admits one successor. Stale IDs
    /// return the selected backend's current record, not another backend's record.
    /// IDs must be canonical lower-case hyphenated UUIDs; malformed IDs or a
    /// retry without a selected owner-local record return `InvalidParams`.
    /// Records are not persisted across owner/process restart.
    ///
    /// Exclude conversions and external environment use while setup runs; native
    /// repair may clean/rebuild generated outputs. This exclusion is caller-owned.
    /// Dropping this call does not cancel admitted work. Drain `shutdown_setup`
    /// before stopping the Tokio runtime; closed admission returns
    /// `InstallationCancelled`. Setup success is not execution readiness proof.
    pub async fn start_backend_setup(
        &self,
        backend: QuantBackend,
        expected_previous_operation_id: Option<&str>,
    ) -> Result<super::ConversionSetupSnapshot> {
        self.backend_setup(backend)?
            .start_or_get(expected_previous_operation_id)
            .await
    }

    /// Inspect the selected built-in setup owner without disk I/O or starting work.
    /// `None` means no operation in this owner/process lifetime, not "not ready".
    /// Includes PythonConversion and remains readable after shutdown. An absent
    /// managed owner returns `InvalidParams`; snapshots are advisory observations
    /// and may change immediately after this read.
    pub fn get_backend_setup(
        &self,
        backend: QuantBackend,
    ) -> Result<Option<super::ConversionSetupSnapshot>> {
        Ok(self.backend_setup(backend)?.snapshot())
    }

    /// Close all built-in setup/probe admission, cancel active work, and observe cleanup.
    /// Call before shutting down the hosting Tokio runtime. Repeated calls
    /// observe the same terminal result; dropping a waiter does not stop cleanup.
    /// Finish caller-owned synchronous readiness calls before invoking shutdown.
    pub async fn shutdown_setup(&self) -> Result<()> {
        // Close every owner before the first suspension; a held installer must
        // not leave other backends open to new setup while shutdown drains.
        self.setup.close();
        for setup in &self.backend_setups {
            setup.close();
        }
        for probe in &self.backend_probes {
            probe.close();
        }
        let mut failures = Vec::new();
        if let Err(error) = self.setup.shutdown().await {
            failures.push(error.to_string());
        }
        for setup in &self.backend_setups {
            if let Err(error) = setup.shutdown().await {
                failures.push(error.to_string());
            }
        }
        for probe in &self.backend_probes {
            if let Err(error) = probe.shutdown().await {
                failures.push(error.to_string());
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(PumasError::ConversionFailed {
                message: failures.join("; "),
            })
        }
    }

    // -----------------------------------------------------------------------
    // Quantization backend management
    // -----------------------------------------------------------------------

    /// Get backend readiness with caller-owned blocking probes. Finish these
    /// calls before shutdown; prefer the async method on a runtime thread.
    pub fn backend_status(&self) -> Vec<BackendStatus> {
        self.backends
            .iter()
            .map(|b| BackendStatus {
                backend: b.backend_id(),
                name: b.name().to_string(),
                ready: b.is_ready(),
            })
            .collect()
    }

    /// Get backend readiness through retained, bounded per-backend probes.
    /// Dropping this read does not detach a probe; shutdown_setup drains it.
    pub async fn backend_status_async(&self) -> Result<Vec<BackendStatus>> {
        let mut statuses = Vec::with_capacity(self.backends.len());
        for backend in &self.backends {
            statuses.push(BackendStatus {
                backend: backend.backend_id(),
                name: backend.name().to_string(),
                ready: backend.is_ready_async().await?,
            });
        }
        Ok(statuses)
    }

    /// Set up the environment for a specific quantization backend.
    /// The backend retains installer work if this waiter is dropped. Drain
    /// `shutdown_setup` before stopping the host runtime.
    ///
    /// # Preconditions
    /// - The backend must be registered.
    /// - No conversion or external tool may concurrently use its environment.
    ///   Setup may clean/rebuild generated native artifacts; installer exclusion
    ///   does not enforce this caller-owned execution exclusion.
    ///
    /// # Postconditions
    /// - The backend's setup recipe and dependency checks completed. Readiness
    ///   remains advisory; execution rechecks operation-specific requirements.
    pub async fn ensure_backend_environment(&self, backend: QuantBackend) -> Result<()> {
        let b = self
            .backends
            .iter()
            .find(|b| b.backend_id() == backend)
            .ok_or_else(|| PumasError::InvalidParams {
                message: format!("Unknown quantization backend: {:?}", backend),
            })?;
        b.ensure_environment().await
    }

    /// Get the list of supported quantization types across all backends.
    ///
    /// Includes the base F16 conversion option (always available) plus
    /// quantization types from any ready backends.
    pub fn supported_quant_types(&self) -> Vec<QuantOption> {
        let mut types = vec![QuantOption {
            name: "F16".to_string(),
            description: "Half-precision float, no quality loss".to_string(),
            bits_per_weight: 16.0,
            recommended: true,
            backend: Some(QuantBackend::PythonConversion),
            imatrix_recommended: false,
        }];

        for backend in &self.backends {
            if backend.is_ready() {
                types.extend(backend.supported_quant_types());
            }
        }

        types
    }

    /// Get supported quantization types through retained backend probes.
    pub async fn supported_quant_types_async(&self) -> Result<Vec<QuantOption>> {
        let mut types = vec![QuantOption {
            name: "F16".to_string(),
            description: "Half-precision float, no quality loss".to_string(),
            bits_per_weight: 16.0,
            recommended: true,
            backend: Some(QuantBackend::PythonConversion),
            imatrix_recommended: false,
        }];

        for backend in &self.backends {
            if backend.is_ready_async().await? {
                types.extend(backend.supported_quant_types());
            }
        }

        Ok(types)
    }

    // -----------------------------------------------------------------------
    // Starting operations
    // -----------------------------------------------------------------------

    /// Start a model format conversion or quantization.
    ///
    /// Returns a conversion ID that can be used to track progress.
    /// Managed quantization rejects unsupported target/backend pairs, missing
    /// required calibration, and invalid supplied calibration files before
    /// admitting a worker. File inspection is not retained custody: callers
    /// must keep calibration files stable and readable through execution.
    pub async fn start_conversion(&self, request: ConversionRequest) -> Result<String> {
        self.workers.observe_finished();

        // Validate source model exists
        let model = self
            .model_library
            .get_model(&request.model_id)
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: request.model_id.clone(),
            })?;

        let metadata: ModelMetadata =
            serde_json::from_value(model.metadata.clone()).unwrap_or_default();

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
            pipeline_step: None,
            pipeline_steps_total: None,
            pipeline_step_label: None,
        };
        let initial_progress = progress;

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Spawn the conversion/quantization task
        let conv_id = conversion_id.clone();
        let launcher_root = self.launcher_root.clone();
        let model_path = PathBuf::from(&model.path);
        let library = self.model_library.clone();
        let importer = self.model_importer.clone();
        let progress = self.progress.clone();
        let direction = request.direction;
        let target_quant = request.target_quant.clone();
        let source_model_id = request.model_id.clone();

        match direction {
            // Existing Python-based format conversions
            ConversionDirection::GgufToSafetensors | ConversionDirection::SafetensorsToGguf => self
                .workers
                .spawn(initial_progress, cancel_token.clone(), async move {
                    run_conversion(
                        &conv_id,
                        direction,
                        &launcher_root,
                        &model_path,
                        &source_model_id,
                        target_quant.as_deref(),
                        metadata,
                        progress.as_ref(),
                        &cancel_token,
                        &library,
                        &importer,
                    )
                    .await
                })?,
            // Quantization via backend — route to the appropriate backend by direction
            ConversionDirection::SafetensorsToQuantizedGguf
            | ConversionDirection::GgufToQuantizedGguf => {
                let (backend, params) = self
                    .prepare_backend_quantization(
                        QuantBackend::LlamaCpp,
                        "llama.cpp",
                        &conv_id,
                        &model_path,
                        &source_model_id,
                        target_quant,
                        &request,
                    )
                    .await?;

                self.workers
                    .spawn(initial_progress, cancel_token.clone(), async move {
                        run_quantization(
                            &conv_id,
                            backend.as_ref(),
                            params,
                            &source_model_id,
                            metadata,
                            progress.as_ref(),
                            &cancel_token,
                            &library,
                        )
                        .await
                    })?
            }
            ConversionDirection::SafetensorsToNvfp4 => {
                let (backend, params) = self
                    .prepare_backend_quantization(
                        QuantBackend::Nvfp4,
                        "nvfp4",
                        &conv_id,
                        &model_path,
                        &source_model_id,
                        target_quant,
                        &request,
                    )
                    .await?;

                self.workers
                    .spawn(initial_progress, cancel_token.clone(), async move {
                        run_quantization(
                            &conv_id,
                            backend.as_ref(),
                            params,
                            &source_model_id,
                            metadata,
                            progress.as_ref(),
                            &cancel_token,
                            &library,
                        )
                        .await
                    })?
            }
            ConversionDirection::SafetensorsToSherryQat => {
                let (backend, params) = self
                    .prepare_backend_quantization(
                        QuantBackend::Sherry,
                        "sherry",
                        &conv_id,
                        &model_path,
                        &source_model_id,
                        target_quant,
                        &request,
                    )
                    .await?;

                self.workers
                    .spawn(initial_progress, cancel_token.clone(), async move {
                        run_quantization(
                            &conv_id,
                            backend.as_ref(),
                            params,
                            &source_model_id,
                            metadata,
                            progress.as_ref(),
                            &cancel_token,
                            &library,
                        )
                        .await
                    })?
            }
        };

        Ok(conversion_id)
    }

    /// Prepare a backend handle and QuantizeParams for a quantization task.
    ///
    /// Finds the backend by ID, builds params from the request, and returns
    /// an owned backend handle suitable for use in a spawned task.
    #[allow(clippy::too_many_arguments)]
    async fn prepare_backend_quantization(
        &self,
        backend_id: QuantBackend,
        backend_name: &str,
        conv_id: &str,
        model_path: &Path,
        source_model_id: &str,
        target_quant: Option<String>,
        request: &ConversionRequest,
    ) -> Result<(Arc<dyn QuantizationBackend>, QuantizeParams)> {
        let quant_type = target_quant.unwrap_or_else(|| match backend_id {
            QuantBackend::LlamaCpp => "Q4_K_M".to_string(),
            QuantBackend::Nvfp4 => "NVFP4".to_string(),
            QuantBackend::Sherry => "Sherry-1.25bit".to_string(),
            QuantBackend::PythonConversion => "F16".to_string(),
        });
        let calibration_file = request.imatrix_calibration_file.as_ref().map(PathBuf::from);
        let force_imatrix = request.force_imatrix.unwrap_or(false);

        let backend = self
            .backends
            .iter()
            .find(|b| b.backend_id() == backend_id)
            .cloned()
            .ok_or_else(|| PumasError::QuantizationEnvNotReady {
                backend: backend_name.to_string(),
                message: format!("No {} backend registered", backend_name),
            })?;

        if !backend
            .supported_quant_types()
            .iter()
            .any(|option| option.backend == Some(backend_id) && option.name == quant_type)
        {
            return Err(PumasError::InvalidParams {
                message: format!("Unsupported target quantization for {backend_name}"),
            });
        }
        if force_imatrix && backend_id != QuantBackend::LlamaCpp {
            return Err(PumasError::InvalidParams {
                message: "force_imatrix is only supported by llama.cpp".into(),
            });
        }
        if backend_id == QuantBackend::LlamaCpp
            && (force_imatrix || quant_type.starts_with("IQ"))
            && calibration_file.is_none()
        {
            return Err(PumasError::InvalidParams {
                message: "IQ targets and forced importance matrices require a calibration file"
                    .into(),
            });
        }
        if let Some(path) = &calibration_file {
            let metadata = match fs::metadata(path).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(PumasError::InvalidParams {
                        message: "Calibration file does not exist".into(),
                    });
                }
                Err(error) => {
                    return Err(PumasError::io("inspecting calibration file", path, error))
                }
            };
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(PumasError::InvalidParams {
                    message: "Calibration must be a nonempty regular file".into(),
                });
            }
        }

        let params = QuantizeParams {
            conversion_id: conv_id.to_string(),
            model_path: model_path.to_path_buf(),
            source_model_id: source_model_id.to_string(),
            target_quant: quant_type,
            calibration_file,
            force_imatrix,
        };

        Ok((backend, params))
    }

    /// Get progress for a specific conversion.
    pub fn get_progress(&self, conversion_id: &str) -> Option<ConversionProgress> {
        self.workers.observe_finished();
        self.progress.get(conversion_id)
    }

    /// Request cooperative cancellation of an active conversion. `true` means
    /// requested, not finished; inspect progress for the worker's terminal result.
    pub async fn cancel_conversion(&self, conversion_id: &str) -> Result<bool> {
        Ok(self.workers.cancel(conversion_id))
    }

    /// List all tracked conversions (active and recently completed).
    pub fn list_conversions(&self) -> Vec<ConversionProgress> {
        self.workers.observe_finished();
        self.progress.list_all()
    }

    /// Close worker admission, request cancellation, and observe every retained
    /// Rust worker. Repeated or interrupted callers retain the same failures.
    /// This receipt does not establish native process-tree cleanup.
    pub async fn shutdown(&self) -> Result<()> {
        self.workers.shutdown().await
    }
}

// ---------------------------------------------------------------------------
// Quantization pipeline (delegates to backend)
// ---------------------------------------------------------------------------

/// Run a quantization operation via a backend, then import the result.
#[allow(clippy::too_many_arguments)]
async fn run_quantization(
    conversion_id: &str,
    backend: &dyn QuantizationBackend,
    params: QuantizeParams,
    source_model_id: &str,
    source_metadata: ModelMetadata,
    progress: &ConversionProgressTracker,
    cancel_token: &CancellationToken,
    library: &ModelLibrary,
) -> Result<()> {
    info!(
        "Starting quantization {} via {}: {} → {}",
        conversion_id,
        backend.name(),
        source_model_id,
        params.target_quant,
    );

    // Delegate to backend
    let output_dir = backend.quantize(&params, progress, cancel_token).await?;

    // Determine source/target format for metadata based on backend
    let (source_format, target_format) = match backend.backend_id() {
        QuantBackend::LlamaCpp => {
            // llama.cpp backend — source could be safetensors or gguf
            let src = if output_dir.to_string_lossy().contains("-gguf-") {
                "safetensors"
            } else {
                "gguf"
            };
            (src, "gguf")
        }
        QuantBackend::Nvfp4 => ("safetensors", "safetensors"),
        QuantBackend::Sherry => ("safetensors", "safetensors"),
        QuantBackend::PythonConversion => ("safetensors", "gguf"),
    };

    // Write metadata and index
    pipeline::write_quantized_metadata(
        conversion_id,
        source_model_id,
        source_format,
        target_format,
        &params.target_quant,
        &source_metadata,
        &output_dir,
        progress,
        library,
    )
    .await?;

    info!("Quantization {} completed successfully", conversion_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Existing format conversion (Python-based, unchanged logic)
// ---------------------------------------------------------------------------

/// Execute the existing Python-based format conversion in a spawned task.
#[allow(clippy::too_many_arguments)]
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
    let python_path = scripts::venv_python(launcher_root);
    if !fs::try_exists(&python_path)
        .await
        .map_err(|e| PumasError::io("checking conversion python", &python_path, e))?
    {
        return Err(PumasError::ConversionFailed {
            message: "Conversion environment not set up. Call setup_conversion_environment first."
                .to_string(),
        });
    }

    scripts::ensure_scripts_deployed(launcher_root).await?;

    progress.set_status(conversion_id, ConversionStatus::Validating);

    let model_files =
        pipeline::list_files_with_extension(model_path, source_extension(direction)).await?;
    if model_files.is_empty() {
        return Err(PumasError::ConversionFailed {
            message: format!(
                "No {} files found in {}",
                source_extension(direction),
                model_path.display()
            ),
        });
    }

    let output_dir = determine_output_dir(model_path, direction)?;
    let workspace = OutputWorkspace::prepare(&output_dir).await?;
    let temp_dir = workspace.staging_path().to_path_buf();

    let scripts_dir = scripts::scripts_dir(launcher_root);
    let (script_name, args) = match direction {
        ConversionDirection::GgufToSafetensors => {
            let mut args = vec![
                "--output-dir".to_string(),
                temp_dir.to_string_lossy().to_string(),
            ];
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

            let config_path = model_path.join("config.json");
            if fs::try_exists(&config_path)
                .await
                .map_err(|e| PumasError::io("checking conversion config", &config_path, e))?
            {
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
        // Quantization directions are handled by run_quantization, not run_conversion.
        _ => unreachable!("run_conversion called with quantization direction"),
    };

    let script_path = scripts_dir.join(script_name);

    info!(
        "Starting conversion {}: {} with {} input file(s)",
        conversion_id,
        script_name,
        model_files.len()
    );

    let mut command = Command::new(&python_path);
    command.arg(&script_path).args(&args);
    script_process::run(
        &mut command,
        "conversion script",
        conversion_id,
        progress,
        cancel_token,
    )
    .await?;

    // Rename temp dir to final
    let output_dir = workspace.publish().await?;

    // Build conversion source metadata
    let is_dequantized = direction == ConversionDirection::GgufToSafetensors
        && target_quant.is_none_or(|q| q != "F16" && q != "F32");
    let conversion_source = ConversionSource {
        source_model_id: source_model_id.to_string(),
        source_format: source_extension(direction).to_string(),
        source_quant: None,
        target_format: target_extension(direction).to_string(),
        target_quant: target_quant.map(|s| s.to_string()),
        was_dequantized: is_dequantized,
        conversion_date: chrono::Utc::now().to_rfc3339(),
    };

    progress.set_status(conversion_id, ConversionStatus::Importing);

    let converted_metadata = ModelMetadata {
        model_id: source_metadata.model_id.clone(),
        family: source_metadata.family.clone(),
        model_type: source_metadata.model_type.clone(),
        official_name: source_metadata
            .official_name
            .as_ref()
            .map(|name| format!("{} ({})", name, target_extension(direction).to_uppercase())),
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

    library
        .save_metadata(&output_dir, &converted_metadata)
        .await?;
    library.index_model_dir(&output_dir).await?;

    let output_model_id = library
        .get_relative_path(&output_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| output_dir.to_string_lossy().to_string());

    progress.set_output_model_id(conversion_id, output_model_id);

    info!("Conversion {} completed successfully", conversion_id);
    Ok(())
}

/// Get the source file extension for a conversion direction.
fn source_extension(direction: ConversionDirection) -> &'static str {
    match direction {
        ConversionDirection::GgufToSafetensors | ConversionDirection::GgufToQuantizedGguf => "gguf",
        ConversionDirection::SafetensorsToGguf
        | ConversionDirection::SafetensorsToQuantizedGguf
        | ConversionDirection::SafetensorsToNvfp4
        | ConversionDirection::SafetensorsToSherryQat => "safetensors",
    }
}

/// Get the target file extension for a conversion direction.
fn target_extension(direction: ConversionDirection) -> &'static str {
    match direction {
        ConversionDirection::GgufToSafetensors
        | ConversionDirection::SafetensorsToNvfp4
        | ConversionDirection::SafetensorsToSherryQat => "safetensors",
        ConversionDirection::SafetensorsToGguf
        | ConversionDirection::SafetensorsToQuantizedGguf
        | ConversionDirection::GgufToQuantizedGguf => "gguf",
    }
}

#[cfg(all(test, unix))]
mod environment_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    async fn manager(root: &Path) -> ConversionManager {
        let library = Arc::new(ModelLibrary::new(root.join("models")).await.unwrap());
        let importer = Arc::new(ModelImporter::new(library.clone()));
        ConversionManager::new(root.to_path_buf(), library, importer)
    }

    fn executable(path: &Path, script: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    const FIXTURE_PYTHON: &str = "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$0.calls\"\nif [ \"$1\" = '-I' ]; then [ -f \"$0.ready\" ]; exit $?; fi\nif [ \"$1\" = '-m' ] && [ \"$2\" = 'pip' ]; then\n if [ -f \"$0.fail\" ]; then exit 1; fi\n if [ \"$4\" = '-r' ]; then touch \"$0.ready\"; fi\n exit 0\nfi\nexit 2\n";

    #[tokio::test]
    async fn readiness_requires_imports_and_repairs_existing_python_after_failed_install() {
        let root = tempfile::TempDir::new().unwrap();
        let manager = manager(root.path()).await;
        let python = scripts::venv_python(root.path());
        assert!(!manager.is_environment_ready_async().await.unwrap());
        executable(&python, FIXTURE_PYTHON);
        assert!(!manager.is_environment_ready());
        assert!(!manager.is_environment_ready_async().await.unwrap());
        std::fs::write(
            python.with_file_name("python.fail"),
            b"fail fixture install",
        )
        .unwrap();
        assert!(matches!(
            manager.ensure_environment().await,
            Err(PumasError::ConversionFailed { .. })
        ));
        assert!(!manager.is_environment_ready_async().await.unwrap());
        std::fs::remove_file(python.with_file_name("python.fail")).unwrap();
        manager.ensure_environment().await.unwrap();
        assert!(manager.is_environment_ready());
        assert!(manager.is_environment_ready_async().await.unwrap());
        assert_eq!(
            std::fs::read_to_string(&python).unwrap(),
            FIXTURE_PYTHON,
            "partial venv must not be recreated"
        );
        let calls = std::fs::read_to_string(python.with_file_name("python.calls")).unwrap();
        assert!(calls.contains("-I -B -c"));
        assert!(calls.contains("sentencepiece"));
        assert!(calls.contains("GGUFReader, GGUFWriter"));
        assert!(calls.contains("safetensors.numpy import save_file"));
        let installations = calls
            .lines()
            .filter(|line| line.starts_with("-m pip"))
            .count();
        manager.ensure_environment().await.unwrap();
        let after = std::fs::read_to_string(python.with_file_name("python.calls")).unwrap();
        assert_eq!(
            after
                .lines()
                .filter(|line| line.starts_with("-m pip"))
                .count(),
            installations
        );
    }

    #[test]
    fn readiness_probe_times_out_and_reaps_the_interpreter() {
        let root = tempfile::TempDir::new().unwrap();
        let python = root.path().join("python");
        executable(
            &python,
            "#!/bin/sh\necho $$ > \"$0.pid\"\nwhile :; do :; done\n",
        );
        assert!(
            !probe_conversion_environment(&python, std::time::Duration::from_millis(100)).unwrap()
        );
        #[cfg(target_os = "linux")]
        {
            let pid = std::fs::read_to_string(root.path().join("python.pid")).unwrap();
            assert!(!Path::new("/proc").join(pid.trim()).exists());
        }
    }

    #[tokio::test]
    async fn readiness_preserves_interpreter_spawn_io_failure() {
        let root = tempfile::TempDir::new().unwrap();
        let manager = manager(root.path()).await;
        let python = scripts::venv_python(root.path());
        std::fs::create_dir_all(&python).unwrap();
        assert!(matches!(
            manager.is_environment_ready_async().await,
            Err(PumasError::Io { .. })
        ));
        assert!(!manager.is_environment_ready());
    }
}

/// Determine the output directory for a format conversion.
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
        _ => unreachable!("determine_output_dir not used for quantization directions"),
    };

    let output_name = format!("{}-{}", dir_name, suffix);
    let parent = model_path
        .parent()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: format!("Model path has no parent: {}", model_path.display()),
        })?;

    Ok(parent.join(output_name))
}
