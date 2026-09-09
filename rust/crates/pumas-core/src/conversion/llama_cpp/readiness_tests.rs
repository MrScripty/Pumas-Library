//! Public backend artifact contracts using tiny local shell fixtures only.

use super::*;

fn artifact(path: &Path, content: &str, executable: bool) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    // Keep writable script descriptors out of unrelated parallel test forks.
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod \"$3\" \"$1\"",
            "fixture-writer",
        ])
        .arg(path)
        .arg(content)
        .arg(if executable { "700" } else { "600" })
        .status()
        .unwrap();
    assert!(status.success(), "fixture writer failed: {status}");
}

fn params(root: &Path, extension: &str) -> QuantizeParams {
    let model_path = root.join("models/source");
    std::fs::create_dir_all(&model_path).unwrap();
    std::fs::write(
        model_path.join(format!("weights.{extension}")),
        "fixture input",
    )
    .unwrap();
    QuantizeParams {
        conversion_id: "fixture".into(),
        model_path,
        source_model_id: "source".into(),
        target_quant: "Q4_K_M".into(),
        calibration_file: None,
        force_imatrix: false,
    }
}

const QUANTIZER: &str = "#!/bin/sh\ntouch \"$0.started\"\nif test \"$1\" = '--imatrix'; then shift 2; fi\nprintf 'quantized fixture' > \"$2\"\n";
const CONVERTER: &str = "#!/bin/sh\nif test \"$1\" = '-I'; then exit 0; fi\ntouch \"$0.started\"\nwhile test $# -gt 0; do\n if test \"$1\" = '--outfile'; then printf 'converted fixture' > \"$2\"; exit 0; fi\n shift\ndone\nexit 2\n";

#[tokio::test]
async fn supplied_calibration_is_validated_before_every_direct_route_has_effects() {
    for extension in ["gguf", "safetensors"] {
        for (quant, force) in [("Q4_K_M", false), ("IQ3_XXS", false), ("Q4_K_M", true)] {
            let root = tempfile::tempdir().unwrap();
            let backend = LlamaCppBackend::new(root.path());
            let mut params = params(root.path(), extension);
            params.target_quant = quant.into();
            params.force_imatrix = force;
            artifact(&backend.quantize_binary(), QUANTIZER, true);
            artifact(
                &backend.convert_script(),
                "fixture converter passed to python",
                false,
            );
            artifact(
                &backend.venv_python(),
                &CONVERTER.replacen("#!/bin/sh", "#!/bin/sh\ntouch \"$0.started\"", 1),
                true,
            );
            artifact(&backend.imatrix_binary(), "#!/bin/sh\ntouch \"$0.started\"\nwhile test $# -gt 0; do\n if test \"$1\" = '-o'; then printf 'matrix fixture' > \"$2\"; exit 0; fi\n shift\ndone\nexit 2\n", true);
            let empty = root.path().join("empty.txt");
            std::fs::write(&empty, "").unwrap();
            let directory = root.path().join("calibration-directory");
            std::fs::create_dir(&directory).unwrap();
            for (path, expected) in [
                (
                    root.path().join("missing.txt"),
                    "Calibration file does not exist",
                ),
                (empty, "Calibration must be a nonempty regular file"),
                (directory, "Calibration must be a nonempty regular file"),
            ] {
                params.calibration_file = Some(path);
                let error = backend
                    .quantize(
                        &params,
                        &ConversionProgressTracker::new(),
                        &CancellationToken::new(),
                    )
                    .await
                    .unwrap_err();
                assert!(
                    matches!(&error, PumasError::InvalidParams { message } if message == expected),
                    "{extension}/{quant}/force={force}: {error}"
                );
                for tool in [
                    backend.venv_python(),
                    backend.quantize_binary(),
                    backend.imatrix_binary(),
                ] {
                    assert!(
                        !started(&tool).exists(),
                        "invalid calibration must reject before imports or native execution"
                    );
                }
                assert_eq!(
                    std::fs::read_dir(params.model_path.parent().unwrap())
                        .unwrap()
                        .count(),
                    1,
                    "invalid calibration must reject before staging"
                );
                assert_eq!(
                    std::fs::read_to_string(params.model_path.join(format!("weights.{extension}")))
                        .unwrap(),
                    "fixture input"
                );
            }
            backend.shutdown_setup().await.unwrap();
        }
    }
}

#[tokio::test]
async fn incomplete_setup_vetoes_every_route_and_read_before_imports_or_staging() {
    for (extension, quant, force) in [
        ("gguf", "Q4_K_M", false),
        ("safetensors", "Q4_K_M", false),
        ("gguf", "IQ3_XXS", false),
        ("safetensors", "Q4_K_M", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let backend = LlamaCppBackend::new(root.path());
        let mut params = params(root.path(), extension);
        params.target_quant = quant.into();
        params.force_imatrix = force;
        let calibration = root.path().join("calibration.txt");
        std::fs::write(&calibration, "fixture calibration").unwrap();
        params.calibration_file = Some(calibration);
        artifact(&backend.quantize_binary(), QUANTIZER, true);
        artifact(&backend.imatrix_binary(), QUANTIZER, true);
        artifact(&backend.convert_script(), "fixture script", false);
        // This marker would expose even an import probe. No executable is run
        // in this test; all routes must stop at the persisted setup veto.
        artifact(
            &backend.venv_python(),
            "#!/bin/sh\ntouch \"$0.started\"\nexit 0\n",
            true,
        );
        std::fs::write(backend.base_dir.join("setup-incomplete"), "").unwrap();
        let reopened = LlamaCppBackend::new(root.path());
        assert!(!reopened.is_ready());
        assert!(!reopened.is_ready_async().await.unwrap());
        assert!(!reopened.has_imatrix());
        rejected(&reopened, &params, "setup is incomplete").await;
        assert!(!started(&reopened.imatrix_binary()).exists());
        assert!(reopened.base_dir.join("setup-incomplete").exists());
        reopened.shutdown_setup().await.unwrap();
    }
}

#[tokio::test]
async fn marker_inspection_io_errors_are_not_readiness_absence() {
    let root = tempfile::tempdir().unwrap();
    let backend = LlamaCppBackend::new(root.path());
    let params = params(root.path(), "gguf");
    std::fs::create_dir_all(backend.base_dir.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&backend.base_dir, &backend.base_dir).unwrap();
    assert!(!backend.is_ready());
    assert!(!backend.has_imatrix());
    assert!(matches!(
        backend.is_ready_async().await,
        Err(PumasError::ConversionFailed { message }) if message.contains("Checking native setup marker")
    ));
    assert!(matches!(
        backend
            .quantize(
                &params,
                &ConversionProgressTracker::new(),
                &CancellationToken::new()
            )
            .await,
        Err(PumasError::Io { message, .. }) if message.contains("checking llama.cpp setup marker")
    ));
    assert_eq!(
        std::fs::read_dir(params.model_path.parent().unwrap())
            .unwrap()
            .count(),
        1
    );
    assert!(backend.shutdown_setup().await.is_err());
}

fn started(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".started");
    PathBuf::from(name)
}

async fn rejected(backend: &LlamaCppBackend, params: &QuantizeParams, artifact_name: &str) {
    let outcome = backend
        .quantize(
            params,
            &ConversionProgressTracker::new(),
            &CancellationToken::new(),
        )
        .await;
    assert!(
        matches!(outcome, Err(PumasError::QuantizationEnvNotReady { backend, message }) if backend == "llama.cpp" && message.contains(artifact_name)),
        "expected missing {artifact_name}"
    );
    assert_eq!(
        std::fs::read_dir(params.model_path.parent().unwrap())
            .unwrap()
            .count(),
        1,
        "preflight must not allocate staging"
    );
    assert!(!started(&backend.quantize_binary()).exists());
    assert!(!started(&backend.venv_python()).exists());
}

#[tokio::test]
async fn gguf_requantization_does_not_require_converter_or_python_even_for_mixed_sources() {
    for mixed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let backend = LlamaCppBackend::new(root.path());
        let params = params(root.path(), "gguf");
        if mixed {
            std::fs::write(params.model_path.join("weights.safetensors"), "fixture").unwrap();
        }
        std::fs::create_dir(params.model_path.join("00-directory.gguf")).unwrap();
        std::fs::write(params.model_path.join("a-first.gguf"), "selected input").unwrap();
        artifact(
            &backend.quantize_binary(),
            "#!/bin/sh\ncp \"$1\" \"$2\"\n",
            true,
        );
        assert!(!backend.is_ready());
        let output = backend
            .quantize(
                &params,
                &ConversionProgressTracker::new(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(output.join("model-q4_k_m.gguf")).unwrap(),
            b"selected input"
        );
        assert!(!backend.convert_script().exists());
        assert!(!backend.venv_python().exists());
    }
}

#[tokio::test]
async fn safetensors_preflight_requires_converter_python_and_requested_imatrix_before_spawn() {
    let root = tempfile::tempdir().unwrap();
    let backend = LlamaCppBackend::new(root.path());
    let mut params = params(root.path(), "safetensors");
    artifact(&backend.quantize_binary(), QUANTIZER, true);
    rejected(&backend, &params, "convert_hf_to_gguf.py").await;
    artifact(
        &backend.convert_script(),
        "fixture converter passed to python",
        false,
    );
    rejected(&backend, &params, "python").await;
    artifact(&backend.venv_python(), CONVERTER, true);
    assert!(
        backend
            .is_ready_async()
            .await
            .expect("fixture readiness inspection"),
        "converter need not have execute permission"
    );
    let calibration = root.path().join("calibration.txt");
    std::fs::write(&calibration, "fixture calibration").unwrap();
    params.calibration_file = Some(calibration);
    for (quant, force) in [("IQ3_XXS", false), ("Q4_K_M", true)] {
        params.target_quant = quant.into();
        params.force_imatrix = force;
        rejected(&backend, &params, "llama-imatrix").await;
    }
    artifact(&backend.imatrix_binary(), "#!/bin/sh\nwhile test $# -gt 0; do\n if test \"$1\" = '-o'; then printf 'matrix fixture' > \"$2\"; exit 0; fi\n shift\ndone\nexit 2\n", true);
    assert!(backend.has_imatrix());
    let output = backend
        .quantize(
            &params,
            &ConversionProgressTracker::new(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(output.join("model-q4_k_m.gguf")).unwrap(),
        b"quantized fixture"
    );
    assert!(started(&backend.venv_python()).exists());
}

#[test]
fn aggregate_readiness_rejects_nonfiles_empty_and_nonexecutable_native_artifacts() {
    for target in ["quantizer", "converter", "python", "imatrix"] {
        for invalid in ["missing", "directory", "empty", "not_executable"] {
            if target == "converter" && invalid == "not_executable" {
                continue;
            }
            let root = tempfile::tempdir().unwrap();
            let backend = LlamaCppBackend::new(root.path());
            let paths = [
                backend.quantize_binary(),
                backend.convert_script(),
                backend.venv_python(),
                backend.imatrix_binary(),
            ];
            for path in &paths {
                artifact(path, "#!/bin/sh\nexit 0\n", true);
            }
            let path = match target {
                "quantizer" => &paths[0],
                "converter" => &paths[1],
                "python" => &paths[2],
                _ => &paths[3],
            };
            std::fs::remove_file(path).unwrap();
            match invalid {
                "directory" => std::fs::create_dir(path).unwrap(),
                "empty" => artifact(path, "", true),
                "not_executable" => artifact(path, "fixture", false),
                _ => {}
            }
            if target == "imatrix" {
                assert!(!backend.has_imatrix(), "{invalid}");
                assert!(
                    backend.is_ready(),
                    "optional imatrix is not aggregate readiness"
                );
            } else {
                assert!(!backend.is_ready(), "{target}: {invalid}");
            }
        }
    }
}

#[tokio::test]
async fn artifact_inspection_failure_is_not_reported_as_missing_setup() {
    let root = tempfile::tempdir().unwrap();
    let backend = LlamaCppBackend::new(root.path());
    let params = params(root.path(), "gguf");
    let quantizer = backend.quantize_binary();
    std::fs::create_dir_all(quantizer.parent().unwrap()).unwrap();
    // A self-loop produces a metadata error independent of effective user IDs.
    std::os::unix::fs::symlink(&quantizer, &quantizer).unwrap();
    assert!(!backend.is_ready(), "boolean summary is conservative");
    let error = backend
        .quantize(
            &params,
            &ConversionProgressTracker::new(),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, PumasError::Io { path: Some(path), .. } if path == quantizer));
    assert_eq!(
        std::fs::read_dir(params.model_path.parent().unwrap())
            .unwrap()
            .count(),
        1,
        "inspection failure must precede staging"
    );
}
