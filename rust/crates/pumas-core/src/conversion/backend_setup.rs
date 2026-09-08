//! Blocking installer recipes executed only inside a retained setup owner and
//! its launcher-data lease. Child custody and cancellation belong to the shared
//! setup runner; recipes neither spawn detached work nor construct backend owners.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use tracing::{info, warn};

use super::setup::{check_cancel, failed, run_command, Failure, Outcome, COMMAND_TIMEOUT};
use super::QuantBackend;
use crate::cancel::CancellationToken;

#[derive(Clone)]
pub(super) struct Programs {
    pub(super) python: PathBuf,
    pub(super) git: PathBuf,
    pub(super) cmake: PathBuf,
    pub(super) nvcc: PathBuf,
}

impl Default for Programs {
    fn default() -> Self {
        Self {
            python: "python3".into(),
            git: "git".into(),
            cmake: "cmake".into(),
            nvcc: "nvcc".into(),
        }
    }
}

pub(super) fn execute(
    root: &Path,
    backend: QuantBackend,
    cancel: &CancellationToken,
    programs: &Programs,
) -> Outcome {
    check_cancel(cancel)?;
    match backend {
        QuantBackend::LlamaCpp => {
            llama_cpp(&root.join("launcher-data/llama-cpp"), cancel, programs)
        }
        QuantBackend::Nvfp4 => python_backend(
            &root.join("launcher-data/nvfp4"),
            "NVFP4",
            "quantize_nvfp4.py",
            include_str!("nvfp4_script.py"),
            &[
                "nvidia-modelopt[all]",
                "transformers",
                "torch",
                "safetensors",
                "datasets",
                "accelerate",
            ],
            cancel,
            programs,
        ),
        QuantBackend::Sherry => python_backend(
            &root.join("launcher-data/sherry"),
            "Sherry",
            "sherry_qat.py",
            include_str!("sherry_script.py"),
            &[
                "angelslim",
                "transformers",
                "torch",
                "safetensors",
                "datasets",
                "accelerate",
                "bitsandbytes",
            ],
            cancel,
            programs,
        ),
        QuantBackend::PythonConversion => Err(Failure::Failed(
            "Base Python setup requires its dedicated recipe".into(),
        )),
    }
}

fn command_status(
    command: &mut Command,
    step: &str,
    cancel: &CancellationToken,
) -> std::result::Result<ExitStatus, Failure> {
    run_command(command, cancel, COMMAND_TIMEOUT).map_err(|error| match error {
        Failure::Cancelled => Failure::Cancelled,
        other => failed(step, format!("{other:?}")),
    })
}

fn required(command: &mut Command, step: &str, cancel: &CancellationToken) -> Outcome {
    let status = command_status(command, step, cancel)?;
    if !status.success() {
        return Err(failed(
            step,
            format!("command exited unsuccessfully ({status})"),
        ));
    }
    Ok(())
}

fn optional(command: &mut Command, step: &str, cancel: &CancellationToken) -> Outcome {
    let status = command_status(command, step, cancel)?;
    if !status.success() {
        warn!(%step, %status, "Optional setup step exited unsuccessfully");
    }
    Ok(())
}

fn exists(path: &Path, step: &str) -> std::result::Result<bool, Failure> {
    path.try_exists().map_err(|error| failed(step, error))
}

fn install_python(
    base: &Path,
    name: &str,
    dependencies: &[&str],
    cancel: &CancellationToken,
    programs: &Programs,
) -> Outcome {
    check_cancel(cancel)?;
    let venv = base.join("venv");
    let python = venv.join("bin/python");
    required(
        Command::new(&programs.python)
            .args(["-m", "venv"])
            .arg(&venv),
        &format!("Creating {name} venv"),
        cancel,
    )?;
    optional(
        Command::new(&python).args(["-m", "pip", "install", "--upgrade", "pip"]),
        &format!("Upgrading {name} pip"),
        cancel,
    )?;
    required(
        Command::new(&python)
            .args(["-m", "pip", "install"])
            .args(dependencies),
        &format!("Installing {name} dependencies"),
        cancel,
    )?;
    check_cancel(cancel)
}

fn python_backend(
    base: &Path,
    name: &str,
    script_name: &str,
    script: &str,
    dependencies: &[&str],
    cancel: &CancellationToken,
    programs: &Programs,
) -> Outcome {
    check_cancel(cancel)?;
    fs::create_dir_all(base)
        .map_err(|error| failed(&format!("Creating {name} directory"), error))?;
    check_cancel(cancel)?;
    fs::write(base.join(script_name), script)
        .map_err(|error| failed(&format!("Deploying {name} script"), error))?;
    // Preserve the existing interpreter-present shortcut; dependency readiness
    // and repairing incomplete environments are separate from setup custody.
    if exists(
        &base.join("venv/bin/python"),
        &format!("Checking {name} interpreter"),
    )? {
        return check_cancel(cancel);
    }
    install_python(base, name, dependencies, cancel, programs)?;
    info!(%name, "Quantization backend setup finished");
    Ok(())
}

fn llama_cpp(base: &Path, cancel: &CancellationToken, programs: &Programs) -> Outcome {
    check_cancel(cancel)?;
    fs::create_dir_all(base).map_err(|error| failed("Creating llama.cpp directory", error))?;
    let source = base.join("source");
    if exists(&source.join(".git"), "Checking llama.cpp checkout")? {
        optional(
            Command::new(&programs.git)
                .args(["pull", "--ff-only"])
                .current_dir(&source),
            "Updating llama.cpp checkout",
            cancel,
        )?;
    } else {
        check_cancel(cancel)?;
        fs::create_dir_all(&source)
            .map_err(|error| failed("Creating llama.cpp source directory", error))?;
        required(
            Command::new(&programs.git)
                .args([
                    "clone",
                    "--depth",
                    "1",
                    "https://github.com/ggml-org/llama.cpp.git",
                ])
                .arg(&source),
            "Cloning llama.cpp checkout",
            cancel,
        )?;
    }
    let build = base.join("build");
    if !exists(
        &build.join("bin/llama-quantize"),
        "Checking llama.cpp quantize binary",
    )? {
        check_cancel(cancel)?;
        fs::create_dir_all(&build)
            .map_err(|error| failed("Creating llama.cpp build directory", error))?;
        let has_cuda = match run_command(
            Command::new(&programs.nvcc).arg("--version"),
            cancel,
            COMMAND_TIMEOUT,
        ) {
            // Preserve existing detection semantics: any observed nvcc exit
            // indicates an installed compiler, independently of its exit code.
            Ok(_) => true,
            Err(Failure::CommandNotFound(_)) => false,
            Err(Failure::Cancelled) => return Err(Failure::Cancelled),
            Err(error) => return Err(failed("Detecting CUDA compiler", format!("{error:?}"))),
        };
        let mut configure = Command::new(&programs.cmake);
        configure
            .arg(format!("-B{}", build.display()))
            .arg(format!("-S{}", source.display()))
            .arg("-DCMAKE_BUILD_TYPE=Release");
        if has_cuda {
            configure.arg("-DGGML_CUDA=ON");
        }
        required(&mut configure, "Configuring llama.cpp build", cancel)?;
        let parallelism = std::thread::available_parallelism()
            .map(|count| count.get().to_string())
            .unwrap_or_else(|_| "4".into());
        required(
            Command::new(&programs.cmake)
                .arg("--build")
                .arg(&build)
                .args([
                    "--config",
                    "Release",
                    "-j",
                    &parallelism,
                    "--target",
                    "llama-quantize",
                    "--target",
                    "llama-imatrix",
                ]),
            "Building llama.cpp",
            cancel,
        )?;
    }
    if !exists(
        &base.join("venv/bin/python"),
        "Checking llama.cpp interpreter",
    )? {
        install_python(
            base,
            "llama.cpp",
            &[
                "torch",
                "transformers",
                "gguf",
                "sentencepiece",
                "numpy",
                "protobuf",
                "safetensors",
            ],
            cancel,
            programs,
        )?;
    }
    check_cancel(cancel)
}
