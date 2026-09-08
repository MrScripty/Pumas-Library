//! Blocking installer recipes executed only inside a retained setup owner and
//! its launcher-data lease. Child custody and cancellation belong to the shared
//! setup runner; recipes neither spawn detached work nor construct backend owners.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::Duration;

use tracing::{info, warn};

use super::setup::{check_cancel, failed, run_command, Failure, Outcome, COMMAND_TIMEOUT};
use super::QuantBackend;
use crate::cancel::CancellationToken;

// Setup probes may load native libraries; this is not an interactive readiness
// deadline. Cancellation and cleanup remain owned by the setup runner.
const SETUP_IMPORT_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const NVFP4_IMPORTS: &str = "import torch; from transformers import AutoModelForCausalLM, AutoTokenizer; import modelopt.torch.quantization; from modelopt.torch.export import export_tensorrt_llm_checkpoint";
pub(super) const SHERRY_IMPORTS: &str = "import torch; from transformers import AutoModelForCausalLM, AutoTokenizer; from angelslim import TernaryQuantizer";
// This proves the locally declared dependencies, not compatibility with every
// revision of the externally maintained llama.cpp conversion script.
pub(super) const LLAMA_IMPORTS: &str =
    "import torch, transformers, gguf, sentencepiece, numpy, google.protobuf, safetensors";

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
            (
                &[
                    "nvidia-modelopt[all]",
                    "transformers",
                    "torch",
                    "safetensors",
                    "datasets",
                    "accelerate",
                ],
                NVFP4_IMPORTS,
            ),
            cancel,
            programs,
        ),
        QuantBackend::Sherry => python_backend(
            &root.join("launcher-data/sherry"),
            "Sherry",
            "sherry_qat.py",
            include_str!("sherry_script.py"),
            (
                &[
                    "angelslim",
                    "transformers",
                    "torch",
                    "safetensors",
                    "datasets",
                    "accelerate",
                    "bitsandbytes",
                ],
                SHERRY_IMPORTS,
            ),
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

pub(super) fn imports_ready(
    python: &Path,
    name: &str,
    imports: &str,
    cancel: &CancellationToken,
    timeout: Duration,
) -> std::result::Result<bool, Failure> {
    match run_command(
        Command::new(python).args(["-I", "-B", "-c", imports]),
        cancel,
        timeout,
    ) {
        Ok(status) => match status.code() {
            Some(0) => Ok(true),
            Some(_) => Ok(false),
            None => Err(failed(
                &format!("Checking {name} imports"),
                format!("probe terminated without a normal exit ({status})"),
            )),
        },
        Err(Failure::CommandNotFound(_)) => Ok(false),
        Err(Failure::Cancelled) => Err(Failure::Cancelled),
        Err(error) => Err(failed(
            &format!("Checking {name} imports"),
            format!("{error:?}"),
        )),
    }
}

fn ensure_python(
    base: &Path,
    name: &str,
    dependencies: &[&str],
    imports: &str,
    cancel: &CancellationToken,
    programs: &Programs,
) -> Outcome {
    check_cancel(cancel)?;
    let venv = base.join("venv");
    let python = venv.join("bin/python");
    if imports_ready(&python, name, imports, cancel, SETUP_IMPORT_PROBE_TIMEOUT)? {
        return check_cancel(cancel);
    }
    if !exists(&python, &format!("Checking {name} interpreter"))? {
        required(
            Command::new(&programs.python)
                .args(["-m", "venv"])
                .arg(&venv),
            &format!("Creating {name} venv"),
            cancel,
        )?;
    }
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
    if !imports_ready(&python, name, imports, cancel, SETUP_IMPORT_PROBE_TIMEOUT)? {
        return Err(Failure::Failed(format!(
            "{name} dependencies were installed but required imports are not ready"
        )));
    }
    check_cancel(cancel)
}

fn python_backend(
    base: &Path,
    name: &str,
    script_name: &str,
    script: &str,
    requirements: (&[&str], &str),
    cancel: &CancellationToken,
    programs: &Programs,
) -> Outcome {
    check_cancel(cancel)?;
    fs::create_dir_all(base)
        .map_err(|error| failed(&format!("Creating {name} directory"), error))?;
    check_cancel(cancel)?;
    fs::write(base.join(script_name), script)
        .map_err(|error| failed(&format!("Deploying {name} script"), error))?;
    ensure_python(base, name, requirements.0, requirements.1, cancel, programs)?;
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
    ensure_python(
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
        LLAMA_IMPORTS,
        cancel,
        programs,
    )?;
    check_cancel(cancel)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn executable(path: &Path, script: &str) {
        // Other tests fork concurrently. Write in an awaited, single-threaded
        // child so they cannot inherit a writable script descriptor and cause
        // a transient ETXTBSY when this test immediately executes the fixture.
        let status = Command::new("/bin/sh")
            .args([
                "-c",
                "printf '%s' \"$2\" > \"$1\" && chmod 700 \"$1\"",
                "fixture-writer",
            ])
            .arg(path)
            .arg(script)
            .status()
            .unwrap();
        assert!(status.success(), "fixture writer failed: {status}");
    }

    #[test]
    fn import_probe_distinguishes_missing_imports_from_execution_failure() {
        let root = tempfile::tempdir().unwrap();
        let python = root.path().join("python");
        let cancel = CancellationToken::new();
        assert!(!imports_ready(
            &python,
            "fixture",
            "import fixture",
            &cancel,
            Duration::from_secs(1)
        )
        .unwrap());
        executable(&python, "#!/bin/sh\nexit 1\n");
        assert!(!imports_ready(
            &python,
            "fixture",
            "import fixture",
            &cancel,
            Duration::from_secs(1)
        )
        .unwrap());
        executable(&python, "#!/bin/sh\ntest \"$1\" = '-I' && test \"$2\" = '-B' && test \"$3\" = '-c' && test \"$4\" = 'import fixture'\n");
        assert!(imports_ready(
            &python,
            "fixture",
            "import fixture",
            &cancel,
            Duration::from_secs(1)
        )
        .unwrap());
        executable(&python, "#!/bin/sh\nkill -TERM $$\n");
        assert!(
            matches!(imports_ready(&python, "fixture", "import fixture", &cancel, Duration::from_secs(1)), Err(Failure::Failed(message)) if message.contains("Checking fixture imports") && message.contains("terminated without a normal exit") && message.contains("signal"))
        );
        fs::set_permissions(&python, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            matches!(imports_ready(&python, "fixture", "import fixture", &cancel, Duration::from_secs(1)), Err(Failure::Failed(message)) if message.contains("Checking fixture imports"))
        );
    }

    #[test]
    fn held_import_probe_timeout_and_cancellation_reap_before_returning() {
        for cancelled in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let python = root.path().join("python");
            let pid_path = root.path().join("python.pid");
            executable(&python, "#!/bin/sh\necho $$ > \"$0.pid\"\nexec sleep 10\n");
            let cancel = CancellationToken::new();
            let result = std::thread::scope(|scope| {
                let observer = cancelled.then(|| {
                    scope.spawn(|| {
                        let deadline = std::time::Instant::now() + Duration::from_secs(5);
                        while !pid_path.exists() && std::time::Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        cancel.cancel();
                        pid_path.exists()
                    })
                });
                let outcome = imports_ready(
                    &python,
                    "fixture",
                    "import fixture",
                    &cancel,
                    if cancelled {
                        Duration::from_secs(5)
                    } else {
                        Duration::from_millis(200)
                    },
                );
                if let Some(observer) = observer {
                    assert!(
                        observer.join().unwrap(),
                        "probe started before cancellation"
                    );
                }
                outcome
            });
            if cancelled {
                assert!(matches!(result, Err(Failure::Cancelled)));
            } else {
                assert!(
                    matches!(&result, Err(Failure::Failed(message)) if message.contains("Checking fixture imports") && message.contains("Conversion setup command did not complete successfully")),
                    "expected timed-out probe, got {result:?}"
                );
            }
            let pid: u32 = fs::read_to_string(pid_path)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "probe reaped before receipt"
            );
        }
    }
}
