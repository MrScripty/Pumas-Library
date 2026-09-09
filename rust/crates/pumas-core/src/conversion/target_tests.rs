//! Direct backend option admission only; no native tools or model execution.

use std::path::Path;

use super::progress::ConversionProgressTracker;
use super::{
    ConversionDirection, ConversionProgress, ConversionStatus, LlamaCppBackend, Nvfp4Backend,
    QuantBackend, QuantizationBackend, QuantizeParams, SherryBackend,
};
use crate::cancel::CancellationToken;
use crate::PumasError;

fn backend(root: &Path, id: QuantBackend) -> Box<dyn QuantizationBackend> {
    match id {
        QuantBackend::LlamaCpp => Box::new(LlamaCppBackend::new(root)),
        QuantBackend::Nvfp4 => Box::new(Nvfp4Backend::new(root)),
        QuantBackend::Sherry => Box::new(SherryBackend::new(root)),
        QuantBackend::PythonConversion => unreachable!("only quantization backends are under test"),
    }
}

fn params(root: &Path, target: &str) -> QuantizeParams {
    QuantizeParams {
        conversion_id: "direct-target-fixture".into(),
        model_path: root.join("models/source"),
        source_model_id: "source".into(),
        target_quant: target.into(),
        calibration_file: None,
        force_imatrix: false,
    }
}

fn initial_progress(params: &QuantizeParams, id: QuantBackend) -> ConversionProgress {
    ConversionProgress {
        conversion_id: params.conversion_id.clone(),
        source_model_id: params.source_model_id.clone(),
        direction: match id {
            QuantBackend::LlamaCpp => ConversionDirection::GgufToQuantizedGguf,
            QuantBackend::Nvfp4 => ConversionDirection::SafetensorsToNvfp4,
            QuantBackend::Sherry => ConversionDirection::SafetensorsToSherryQat,
            QuantBackend::PythonConversion => {
                unreachable!("only quantization backends are under test")
            }
        },
        status: ConversionStatus::Validating,
        progress: None,
        current_tensor: None,
        tensors_completed: None,
        tensors_total: None,
        bytes_written: None,
        estimated_output_size: None,
        target_quant: Some(params.target_quant.clone()),
        error: None,
        output_model_id: None,
        pipeline_step: None,
        pipeline_steps_total: None,
        pipeline_step_label: None,
    }
}

#[tokio::test]
async fn direct_backends_reject_invalid_targets_before_environment_progress_or_file_effects() {
    for (id, name, valid, wrong) in [
        (
            QuantBackend::LlamaCpp,
            "llama.cpp",
            "Q4_K_M",
            ["NVFP4", "Sherry-1.25bit"],
        ),
        (
            QuantBackend::Nvfp4,
            "nvfp4",
            "NVFP4",
            ["Q4_K_M", "Sherry-1.25bit"],
        ),
        (
            QuantBackend::Sherry,
            "sherry",
            "Sherry-1.25bit",
            ["Q4_K_M", "NVFP4"],
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let backend = backend(root.path(), id);
        let model = root.path().join("models/source");
        std::fs::create_dir_all(&model).unwrap();
        let extension = if id == QuantBackend::LlamaCpp {
            "gguf"
        } else {
            "safetensors"
        };
        let source = model.join(format!("weights.{extension}"));
        std::fs::write(&source, "fixture source").unwrap();
        for target in [
            wrong[0].to_string(),
            wrong[1].to_string(),
            String::new(),
            valid.to_lowercase(),
            format!(" {valid}"),
            format!("{valid} "),
            format!("../{valid}"),
            format!("{valid}/output"),
            "--help".into(),
        ] {
            let params = params(root.path(), &target);
            let progress = ConversionProgressTracker::new();
            let initial = initial_progress(&params, id);
            let expected_progress = serde_json::to_value(&initial).unwrap();
            progress.insert(initial);
            let error = backend
                .quantize(&params, &progress, &CancellationToken::new())
                .await
                .unwrap_err();
            assert!(
                matches!(&error, PumasError::InvalidParams { message } if message == &format!("Unsupported target quantization for {name}")),
                "{name}/{target:?}: {error}"
            );
            assert!(!root.path().join("launcher-data").exists());
            assert_eq!(
                std::fs::read_dir(model.parent().unwrap()).unwrap().count(),
                1,
                "no staging or published output"
            );
            assert_eq!(
                std::fs::read_dir(root.path()).unwrap().count(),
                1,
                "no other root effects"
            );
            assert_eq!(std::fs::read_dir(&model).unwrap().count(), 1);
            assert_eq!(std::fs::read(&source).unwrap(), b"fixture source");
            assert_eq!(
                serde_json::to_value(progress.get(&params.conversion_id).unwrap()).unwrap(),
                expected_progress
            );
        }
    }
}

#[tokio::test]
async fn direct_non_llama_backends_reject_forced_imatrix_before_effects() {
    for (id, target) in [
        (QuantBackend::Nvfp4, "NVFP4"),
        (QuantBackend::Sherry, "Sherry-1.25bit"),
    ] {
        for supplied in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let backend = backend(root.path(), id);
            let mut params = params(root.path(), target);
            params.force_imatrix = true;
            std::fs::create_dir_all(&params.model_path).unwrap();
            let source = params.model_path.join("weights.safetensors");
            std::fs::write(&source, "fixture source").unwrap();
            let calibration = root.path().join("calibration.txt");
            if supplied {
                std::fs::write(&calibration, "fixture calibration").unwrap();
                params.calibration_file = Some(calibration.clone());
            }
            let progress = ConversionProgressTracker::new();
            let initial = initial_progress(&params, id);
            let expected_progress = serde_json::to_value(&initial).unwrap();
            progress.insert(initial);
            let error = backend
                .quantize(&params, &progress, &CancellationToken::new())
                .await
                .unwrap_err();
            assert!(
                matches!(&error, PumasError::InvalidParams { message } if message == "force_imatrix is only supported by llama.cpp"),
                "{id:?}/calibration={supplied}: {error}"
            );
            assert!(!root.path().join("launcher-data").exists());
            assert_eq!(
                std::fs::read_dir(params.model_path.parent().unwrap())
                    .unwrap()
                    .count(),
                1,
                "no staging or published output"
            );
            assert_eq!(
                std::fs::read_dir(root.path()).unwrap().count(),
                if supplied { 2 } else { 1 }
            );
            assert_eq!(std::fs::read_dir(&params.model_path).unwrap().count(), 1);
            assert_eq!(std::fs::read(source).unwrap(), b"fixture source");
            if supplied {
                assert_eq!(std::fs::read(calibration).unwrap(), b"fixture calibration");
            }
            assert_eq!(
                serde_json::to_value(progress.get(&params.conversion_id).unwrap()).unwrap(),
                expected_progress
            );
        }
    }
}

#[tokio::test]
async fn llama_forced_imatrix_passes_options_and_reaches_missing_source_validation() {
    let root = tempfile::tempdir().unwrap();
    let backend = LlamaCppBackend::new(root.path());
    let mut params = params(root.path(), "Q4_K_M");
    params.force_imatrix = true;
    let calibration = root.path().join("calibration.txt");
    std::fs::write(&calibration, "fixture calibration").unwrap();
    params.calibration_file = Some(calibration.clone());
    let error = backend
        .quantize(
            &params,
            &ConversionProgressTracker::new(),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    let expected = format!(
        "No safetensors or GGUF files found in {}",
        params.model_path.display()
    );
    assert!(
        matches!(&error, PumasError::ConversionFailed { message } if message == &expected),
        "{error}"
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(std::fs::read(calibration).unwrap(), b"fixture calibration");
    backend.shutdown_setup().await.unwrap();
}

#[tokio::test]
async fn catalog_targets_pass_target_admission_and_reach_missing_source_validation() {
    for id in [
        QuantBackend::LlamaCpp,
        QuantBackend::Nvfp4,
        QuantBackend::Sherry,
    ] {
        let root = tempfile::tempdir().unwrap();
        let backend = backend(root.path(), id);
        let options = backend.supported_quant_types();
        assert!(!options.is_empty());
        for option in options {
            let params = params(root.path(), &option.name);
            let error = backend
                .quantize(
                    &params,
                    &ConversionProgressTracker::new(),
                    &CancellationToken::new(),
                )
                .await
                .unwrap_err();
            let expected = if id == QuantBackend::LlamaCpp {
                format!(
                    "No safetensors or GGUF files found in {}",
                    params.model_path.display()
                )
            } else {
                format!(
                    "Source model path is not a directory: {}",
                    params.model_path.display()
                )
            };
            assert!(
                matches!(&error, PumasError::ConversionFailed { message } if message == &expected),
                "{:?}/{}: {error}",
                id,
                option.name
            );
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        }
    }
}
