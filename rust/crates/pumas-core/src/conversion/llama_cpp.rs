//! llama.cpp quantization backend.
//!
//! Manages the llama.cpp build environment (git clone + cmake) and implements
//! the `QuantizationBackend` trait for GGUF quantization via `llama-quantize`,
//! `llama-imatrix`, and `convert_hf_to_gguf.py`.

use std::path::{Path, PathBuf};

use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

use super::pipeline;
use super::progress::ConversionProgressTracker;
use super::types::{
    BackendStatus, ConversionStatus, QuantBackend, QuantOption, QuantizationBackend, QuantizeParams,
};
use crate::cancel::CancellationToken;
use crate::{PumasError, Result};

/// Git repository URL for llama.cpp.
const LLAMA_CPP_REPO: &str = "https://github.com/ggml-org/llama.cpp.git";

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

/// llama.cpp quantization backend.
///
/// Manages the llama.cpp source checkout, cmake build, and a Python venv
/// for `convert_hf_to_gguf.py`. All artifacts live under
/// `{launcher_root}/launcher-data/llama-cpp/`.
pub struct LlamaCppBackend {
    base_dir: PathBuf,
}

impl LlamaCppBackend {
    pub fn new(launcher_root: &Path) -> Self {
        Self {
            base_dir: launcher_root.join("launcher-data").join("llama-cpp"),
        }
    }

    /// Path to the llama.cpp source checkout.
    fn source_dir(&self) -> PathBuf {
        self.base_dir.join("source")
    }

    /// Path to the cmake build directory.
    fn build_dir(&self) -> PathBuf {
        self.base_dir.join("build")
    }

    /// Path to the `llama-quantize` binary.
    pub fn quantize_binary(&self) -> PathBuf {
        self.build_dir().join("bin").join("llama-quantize")
    }

    /// Path to the `llama-imatrix` binary.
    pub fn imatrix_binary(&self) -> PathBuf {
        self.build_dir().join("bin").join("llama-imatrix")
    }

    /// Path to `convert_hf_to_gguf.py` from the llama.cpp repo.
    pub fn convert_script(&self) -> PathBuf {
        self.source_dir().join("convert_hf_to_gguf.py")
    }

    /// Path to the Python venv directory.
    fn venv_dir(&self) -> PathBuf {
        self.base_dir.join("venv")
    }

    /// Path to the Python binary inside the venv.
    pub fn venv_python(&self) -> PathBuf {
        self.venv_dir().join("bin").join("python")
    }

    /// Whether the `llama-imatrix` binary is available.
    pub fn has_imatrix(&self) -> bool {
        self.imatrix_binary().exists()
    }

    /// Returns the backend status summary.
    pub fn status(&self) -> BackendStatus {
        BackendStatus {
            backend: QuantBackend::LlamaCpp,
            name: "llama.cpp".to_string(),
            ready: self.is_ready(),
        }
    }

    // -- Build steps --------------------------------------------------------

    async fn git_clone(&self) -> Result<()> {
        let source = self.source_dir();
        std::fs::create_dir_all(&source)
            .map_err(|e| PumasError::io("creating llama-cpp source dir", &source, e))?;

        let output = Command::new("git")
            .args(["clone", "--depth", "1", LLAMA_CPP_REPO, &source.to_string_lossy()])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to run git clone: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::Other(format!(
                "git clone failed: {stderr}"
            )));
        }
        Ok(())
    }

    async fn git_pull(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(self.source_dir())
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to run git pull: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("git pull failed (non-fatal): {}", stderr);
        }
        Ok(())
    }

    async fn cmake_configure(&self) -> Result<()> {
        let build = self.build_dir();
        std::fs::create_dir_all(&build)
            .map_err(|e| PumasError::io("creating llama-cpp build dir", &build, e))?;

        // Detect CUDA availability.
        let has_cuda = Command::new("nvcc").arg("--version").output().await.is_ok();

        let mut args = vec![
            format!("-B{}", build.display()),
            format!("-S{}", self.source_dir().display()),
            "-DCMAKE_BUILD_TYPE=Release".to_string(),
        ];

        if has_cuda {
            info!("CUDA detected — enabling GGML_CUDA for llama.cpp build");
            args.push("-DGGML_CUDA=ON".to_string());
        }

        let output = Command::new("cmake")
            .args(&args)
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("cmake configure failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::Other(format!(
                "cmake configure failed: {stderr}"
            )));
        }
        Ok(())
    }

    async fn cmake_build(&self) -> Result<()> {
        let nproc = std::thread::available_parallelism()
            .map(|n| n.get().to_string())
            .unwrap_or_else(|_| "4".to_string());

        let output = Command::new("cmake")
            .args([
                "--build",
                &self.build_dir().to_string_lossy(),
                "--config",
                "Release",
                "-j",
                &nproc,
                "--target",
                "llama-quantize",
                "--target",
                "llama-imatrix",
            ])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("cmake build failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::Other(format!(
                "cmake build failed: {stderr}"
            )));
        }
        Ok(())
    }

    async fn setup_python_venv(&self) -> Result<()> {
        let venv = self.venv_dir();
        let python = self.venv_python();

        // Create venv
        let output = Command::new("python3")
            .args(["-m", "venv", &venv.to_string_lossy()])
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("Failed to create llama-cpp venv: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::Other(format!(
                "Failed to create Python venv: {stderr}"
            )));
        }

        // Install convert_hf_to_gguf.py dependencies.
        // The llama.cpp repo has its own requirements but the core deps are:
        let deps = [
            "torch", "transformers", "gguf", "sentencepiece", "numpy", "protobuf", "safetensors",
        ];

        info!("Installing Python dependencies for convert_hf_to_gguf.py...");
        let output = Command::new(&python)
            .args(["-m", "pip", "install", "--upgrade", "pip"])
            .output()
            .await
            .ok();
        if let Some(o) = output {
            if !o.status.success() {
                warn!("pip upgrade failed (non-fatal)");
            }
        }

        let output = Command::new(&python)
            .arg("-m")
            .arg("pip")
            .arg("install")
            .args(&deps)
            .output()
            .await
            .map_err(|e| PumasError::Other(format!("pip install failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PumasError::Other(format!(
                "Failed to install convert_hf_to_gguf.py dependencies: {stderr}"
            )));
        }

        info!("llama.cpp Python environment ready");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// QuantizationBackend trait impl
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl QuantizationBackend for LlamaCppBackend {
    fn name(&self) -> &str {
        "llama.cpp"
    }

    fn backend_id(&self) -> QuantBackend {
        QuantBackend::LlamaCpp
    }

    fn is_ready(&self) -> bool {
        self.quantize_binary().exists() && self.convert_script().exists()
    }

    async fn ensure_environment(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)
            .map_err(|e| PumasError::io("creating llama-cpp dir", &self.base_dir, e))?;

        // Step 1: Clone or update source
        if self.source_dir().join(".git").exists() {
            info!("Updating llama.cpp source...");
            self.git_pull().await?;
        } else {
            info!("Cloning llama.cpp repository...");
            self.git_clone().await?;
        }

        // Step 2-3: cmake build (only if binaries missing)
        if !self.quantize_binary().exists() {
            info!("Building llama.cpp (cmake configure)...");
            self.cmake_configure().await?;
            info!("Building llama.cpp (compiling)...");
            self.cmake_build().await?;
        }

        // Step 4: Python venv (only if missing)
        if !self.venv_python().exists() {
            info!("Setting up Python environment for HF conversion...");
            self.setup_python_venv().await?;
        }

        info!("llama.cpp quantization environment ready");
        Ok(())
    }

    fn supported_quant_types(&self) -> Vec<QuantOption> {
        llama_cpp_quant_options()
    }

    async fn quantize(
        &self,
        params: &QuantizeParams,
        progress: &ConversionProgressTracker,
        cancel_token: &CancellationToken,
    ) -> Result<PathBuf> {
        // -- PHASE 1: GATHER (read-only, fail early) --
        if !self.is_ready() {
            return Err(PumasError::QuantizationEnvNotReady {
                backend: "llama.cpp".to_string(),
                message: "llama.cpp environment not built. Call ensure_environment() first."
                    .to_string(),
            });
        }

        let is_safetensors_source = has_safetensors_files(&params.model_path);
        let is_gguf_source = has_gguf_files(&params.model_path);

        if !is_safetensors_source && !is_gguf_source {
            return Err(PumasError::ConversionFailed {
                message: format!(
                    "No safetensors or GGUF files found in {}",
                    params.model_path.display()
                ),
            });
        }

        let needs_imatrix =
            params.force_imatrix || params.target_quant.starts_with("IQ");
        if needs_imatrix && params.calibration_file.is_none() {
            return Err(PumasError::ConversionFailed {
                message: format!(
                    "Quantization type '{}' requires a calibration file for importance matrix \
                     generation. Provide imatrix_calibration_file in the request.",
                    params.target_quant
                ),
            });
        }

        // -- PHASE 2: VALIDATE --
        let needs_f16_conversion = is_safetensors_source && !is_gguf_source;
        let total_steps = match (needs_f16_conversion, needs_imatrix) {
            (true, true) => 3u32,  // convert + imatrix + quantize
            (true, false) => 2u32, // convert + quantize
            (false, true) => 2u32, // imatrix + quantize
            (false, false) => 1u32, // quantize only
        };

        let quant_lower = params.target_quant.to_lowercase();
        let output_dir = determine_quantized_output_dir(&params.model_path, &params.target_quant)?;
        let temp_dir = output_dir.with_extension("quantizing");

        // Clean up any leftover temp dir from a previous failed run.
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir).ok();
        }
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| PumasError::io("creating quantization temp dir", &temp_dir, e))?;

        let f16_gguf = temp_dir.join("intermediate-f16.gguf");
        let imatrix_file = temp_dir.join("imatrix.dat");
        let output_gguf = temp_dir.join(format!("model-{quant_lower}.gguf"));

        let mut current_step = 0u32;

        // -- PHASE 3: CREATE (subprocess pipeline) --

        // Step: convert safetensors → F16 GGUF
        if needs_f16_conversion {
            current_step += 1;
            progress.update_pipeline(
                &params.conversion_id,
                current_step,
                total_steps,
                "Converting to F16 GGUF",
            );
            progress.set_status(&params.conversion_id, ConversionStatus::GeneratingF16Gguf);
            cancel_token.check().map_err(|_| PumasError::ConversionCancelled)?;

            let mut child = Command::new(self.venv_python())
                .arg(self.convert_script())
                .arg(&params.model_path)
                .arg("--outtype")
                .arg("f16")
                .arg("--outfile")
                .arg(&f16_gguf)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| PumasError::ConversionFailed {
                    message: format!("Failed to spawn convert_hf_to_gguf.py: {e}"),
                })?;

            pipeline::stream_subprocess_stderr_lines(
                &params.conversion_id,
                &mut child,
                progress,
                cancel_token,
            )
            .await?;
            pipeline::wait_and_check_exit(&mut child, "convert_hf_to_gguf.py").await?;
        }

        // Determine the GGUF file to feed into quantize.
        let source_gguf = if needs_f16_conversion {
            f16_gguf.clone()
        } else {
            find_gguf_file(&params.model_path)?
        };

        // Step: importance matrix generation
        if needs_imatrix {
            current_step += 1;
            progress.update_pipeline(
                &params.conversion_id,
                current_step,
                total_steps,
                "Computing importance matrix",
            );
            progress.set_status(&params.conversion_id, ConversionStatus::ComputingImatrix);
            cancel_token.check().map_err(|_| PumasError::ConversionCancelled)?;

            let cal = params.calibration_file.as_ref().expect("validated above");
            let mut child = Command::new(self.imatrix_binary())
                .arg("-m")
                .arg(&source_gguf)
                .arg("-f")
                .arg(cal)
                .arg("-o")
                .arg(&imatrix_file)
                .arg("--no-ppl")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| PumasError::ConversionFailed {
                    message: format!("Failed to spawn llama-imatrix: {e}"),
                })?;

            pipeline::stream_subprocess_stderr_lines(
                &params.conversion_id,
                &mut child,
                progress,
                cancel_token,
            )
            .await?;
            pipeline::wait_and_check_exit(&mut child, "llama-imatrix").await?;
        }

        // Step: quantize
        current_step += 1;
        progress.update_pipeline(
            &params.conversion_id,
            current_step,
            total_steps,
            &format!("Quantizing to {}", params.target_quant),
        );
        progress.set_status(&params.conversion_id, ConversionStatus::Quantizing);
        cancel_token.check().map_err(|_| PumasError::ConversionCancelled)?;

        let mut cmd = Command::new(self.quantize_binary());
        if needs_imatrix && imatrix_file.exists() {
            cmd.arg("--imatrix").arg(&imatrix_file);
        }
        cmd.arg(&source_gguf)
            .arg(&output_gguf)
            .arg(&params.target_quant);

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| PumasError::ConversionFailed {
                message: format!("Failed to spawn llama-quantize: {e}"),
            })?;

        stream_quantize_progress(&params.conversion_id, &mut child, progress, cancel_token)
            .await?;
        pipeline::wait_and_check_exit(&mut child, "llama-quantize").await?;

        // -- PHASE 4: CLEANUP --
        // Remove intermediate files (keep only the quantized output).
        if needs_f16_conversion && f16_gguf.exists() {
            std::fs::remove_file(&f16_gguf).ok();
        }
        if imatrix_file.exists() {
            std::fs::remove_file(&imatrix_file).ok();
        }

        // Atomic rename temp dir → final output dir.
        pipeline::finalize_output_dir(&temp_dir, &output_dir)?;

        Ok(output_dir)
    }
}

// ---------------------------------------------------------------------------
// llama-quantize stderr progress parser
// ---------------------------------------------------------------------------

/// Parse `llama-quantize` stderr output for per-tensor progress.
///
/// llama-quantize emits lines like:
/// ```text
/// [ 123/ 456]  model.layers.5.attn_k.weight - [ 4096,  4096,     1,     1], type = f16, ...
/// ```
async fn stream_quantize_progress(
    conversion_id: &str,
    child: &mut tokio::process::Child,
    progress: &ConversionProgressTracker,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let stderr = child.stderr.take().expect("stderr was piped");
    let mut reader = BufReader::new(stderr).lines();
    let re = Regex::new(r"\[\s*(\d+)/\s*(\d+)\]\s+(\S+)").expect("valid regex");

    loop {
        if cancel_token.is_cancelled() {
            child.kill().await.ok();
            return Err(PumasError::ConversionCancelled);
        }

        match reader.next_line().await {
            Ok(Some(line)) => {
                if let Some(caps) = re.captures(&line) {
                    let idx: u32 = caps[1].parse().unwrap_or(0);
                    let total: u32 = caps[2].parse().unwrap_or(0);
                    let tensor = caps[3].to_string();

                    progress.update_tensor_progress(conversion_id, idx, total, &tensor);
                } else {
                    debug!("llama-quantize: {}", line);
                }
            }
            Ok(None) => break,
            Err(e) => {
                warn!("Error reading llama-quantize stderr: {}", e);
                break;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Quant option catalog
// ---------------------------------------------------------------------------

/// Full catalog of llama.cpp quantization types.
pub fn llama_cpp_quant_options() -> Vec<QuantOption> {
    let b = Some(QuantBackend::LlamaCpp);
    vec![
        // K-quants
        QuantOption { name: "Q2_K".into(), description: "2-bit K-quant (smallest, lowest quality)".into(), bits_per_weight: 3.35, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q3_K_S".into(), description: "3-bit K-quant small".into(), bits_per_weight: 3.50, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q3_K_M".into(), description: "3-bit K-quant medium".into(), bits_per_weight: 3.91, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q3_K_L".into(), description: "3-bit K-quant large".into(), bits_per_weight: 4.27, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q4_K_S".into(), description: "4-bit K-quant small".into(), bits_per_weight: 4.58, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q4_K_M".into(), description: "4-bit K-quant medium — best balance of size and quality".into(), bits_per_weight: 4.85, recommended: true, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q5_K_S".into(), description: "5-bit K-quant small".into(), bits_per_weight: 5.54, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q5_K_M".into(), description: "5-bit K-quant medium".into(), bits_per_weight: 5.69, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q6_K".into(), description: "6-bit K-quant (high quality, larger)".into(), bits_per_weight: 6.56, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "Q8_0".into(), description: "8-bit (near-lossless)".into(), bits_per_weight: 8.50, recommended: false, backend: b, imatrix_recommended: false },
        // I-quants (importance matrix strongly recommended)
        QuantOption { name: "IQ1_S".into(), description: "1-bit importance quant (extreme compression, needs imatrix)".into(), bits_per_weight: 1.56, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ1_M".into(), description: "1-bit importance quant medium (needs imatrix)".into(), bits_per_weight: 1.75, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ2_XXS".into(), description: "2-bit importance quant extra-extra-small (needs imatrix)".into(), bits_per_weight: 2.06, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ2_XS".into(), description: "2-bit importance quant extra-small (needs imatrix)".into(), bits_per_weight: 2.31, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ2_S".into(), description: "2-bit importance quant small (needs imatrix)".into(), bits_per_weight: 2.50, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ2_M".into(), description: "2-bit importance quant medium (needs imatrix)".into(), bits_per_weight: 2.70, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ3_XXS".into(), description: "3-bit importance quant extra-extra-small (needs imatrix)".into(), bits_per_weight: 3.06, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ3_XS".into(), description: "3-bit importance quant extra-small (needs imatrix)".into(), bits_per_weight: 3.30, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ3_S".into(), description: "3-bit importance quant small (needs imatrix)".into(), bits_per_weight: 3.44, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ3_M".into(), description: "3-bit importance quant medium (needs imatrix)".into(), bits_per_weight: 3.66, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ4_NL".into(), description: "4-bit non-linear importance quant (needs imatrix)".into(), bits_per_weight: 4.50, recommended: false, backend: b, imatrix_recommended: true },
        QuantOption { name: "IQ4_XS".into(), description: "4-bit importance quant extra-small (needs imatrix)".into(), bits_per_weight: 4.25, recommended: false, backend: b, imatrix_recommended: true },
        // Lossless / base types
        QuantOption { name: "BF16".into(), description: "Brain float 16-bit (no quality loss)".into(), bits_per_weight: 16.0, recommended: false, backend: b, imatrix_recommended: false },
        QuantOption { name: "F16".into(), description: "Half-precision float 16-bit (no quality loss)".into(), bits_per_weight: 16.0, recommended: false, backend: b, imatrix_recommended: false },
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Determine the output directory for a quantized model.
///
/// Produces: `{parent}/{model_dir_name}-gguf-{quant_lower}/`
fn determine_quantized_output_dir(model_path: &Path, quant_type: &str) -> Result<PathBuf> {
    let dir_name = model_path
        .file_name()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: format!("Invalid model path: {}", model_path.display()),
        })?
        .to_string_lossy();

    let parent = model_path
        .parent()
        .ok_or_else(|| PumasError::ConversionFailed {
            message: format!("Model path has no parent: {}", model_path.display()),
        })?;

    Ok(parent.join(format!("{}-gguf-{}", dir_name, quant_type.to_lowercase())))
}

/// Check if a directory contains `.safetensors` files.
fn has_safetensors_files(path: &Path) -> bool {
    path.is_dir()
        && std::fs::read_dir(path)
            .ok()
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        == Some("safetensors")
                })
            })
            .unwrap_or(false)
}

/// Check if a directory contains `.gguf` files.
fn has_gguf_files(path: &Path) -> bool {
    path.is_dir()
        && std::fs::read_dir(path)
            .ok()
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("gguf"))
            })
            .unwrap_or(false)
}

/// Find the first `.gguf` file in a model directory.
fn find_gguf_file(model_path: &Path) -> Result<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(model_path)
        .map_err(|e| PumasError::io("reading model directory", model_path, e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|ext| ext.to_str()) == Some("gguf"))
        .collect();

    files.sort();
    files.into_iter().next().ok_or_else(|| PumasError::ConversionFailed {
        message: format!("No GGUF file found in {}", model_path.display()),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_construction() {
        let backend = LlamaCppBackend::new(Path::new("/opt/pumas"));
        assert_eq!(
            backend.quantize_binary(),
            PathBuf::from("/opt/pumas/launcher-data/llama-cpp/build/bin/llama-quantize")
        );
        assert_eq!(
            backend.imatrix_binary(),
            PathBuf::from("/opt/pumas/launcher-data/llama-cpp/build/bin/llama-imatrix")
        );
        assert_eq!(
            backend.convert_script(),
            PathBuf::from("/opt/pumas/launcher-data/llama-cpp/source/convert_hf_to_gguf.py")
        );
        assert_eq!(
            backend.venv_python(),
            PathBuf::from("/opt/pumas/launcher-data/llama-cpp/venv/bin/python")
        );
    }

    #[test]
    fn test_not_ready_without_build() {
        let backend = LlamaCppBackend::new(Path::new("/nonexistent"));
        assert!(!backend.is_ready());
        assert!(!backend.has_imatrix());
    }

    #[test]
    fn test_quant_options_catalog() {
        let opts = llama_cpp_quant_options();
        assert!(!opts.is_empty());

        // Q4_K_M should be recommended
        let q4km = opts.iter().find(|o| o.name == "Q4_K_M").unwrap();
        assert!(q4km.recommended);
        assert!(!q4km.imatrix_recommended);

        // IQ3_XXS should recommend imatrix
        let iq3 = opts.iter().find(|o| o.name == "IQ3_XXS").unwrap();
        assert!(!iq3.recommended);
        assert!(iq3.imatrix_recommended);

        // All should have LlamaCpp backend
        for opt in &opts {
            assert_eq!(opt.backend, Some(QuantBackend::LlamaCpp));
        }
    }

    #[test]
    fn test_quantize_progress_regex() {
        let re = Regex::new(r"\[\s*(\d+)/\s*(\d+)\]\s+(\S+)").unwrap();

        let line = "[ 123/ 456]  model.layers.5.attn_k.weight - [ 4096,  4096,     1,     1], type = f16, converting to q4_K .. size =    32.00 MiB ->     9.00 MiB";
        let caps = re.captures(line).unwrap();
        assert_eq!(&caps[1], "123");
        assert_eq!(&caps[2], "456");
        assert_eq!(&caps[3], "model.layers.5.attn_k.weight");

        let line2 = "[  1/ 10]  token_embd.weight - [4096, 32000,     1,     1], type = f16";
        let caps2 = re.captures(line2).unwrap();
        assert_eq!(&caps2[1], "1");
        assert_eq!(&caps2[2], "10");
        assert_eq!(&caps2[3], "token_embd.weight");
    }

    #[test]
    fn test_determine_output_dir() {
        let path = PathBuf::from("/models/llm/llama/llama-3-8b");
        let result = determine_quantized_output_dir(&path, "Q4_K_M").unwrap();
        assert_eq!(
            result,
            PathBuf::from("/models/llm/llama/llama-3-8b-gguf-q4_k_m")
        );
    }

    #[test]
    fn test_backend_status() {
        let backend = LlamaCppBackend::new(Path::new("/nonexistent"));
        let status = backend.status();
        assert_eq!(status.backend, QuantBackend::LlamaCpp);
        assert_eq!(status.name, "llama.cpp");
        assert!(!status.ready);
    }
}
