//! Consumer regression: simulated executables write tiny fixture payloads; no
//! model tools, downloads, GPU or real model data are used.

use super::*;
use std::os::unix::fs::PermissionsExt;

fn executable(path: &Path, script: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

const DIRECTORY_WRITER: &str = "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n if [ \"$1\" = '--output-dir' ]; then printf 'fixture payload' > \"$2/model.safetensors\"; exit 0; fi\n shift\ndone\nexit 2\n";

#[tokio::test]
async fn every_conversion_path_indexes_the_actual_versioned_output_without_touching_old_data() {
    for (backend_id, suffix, quant) in [
        (QuantBackend::PythonConversion, "safetensors", "F16"),
        (QuantBackend::LlamaCpp, "gguf-q4_k_m", "Q4_K_M"),
        (QuantBackend::Nvfp4, "nvfp4", "NVFP4"),
        (QuantBackend::Sherry, "sherry-1.25bit", "Sherry-1.25bit"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let library = Arc::new(ModelLibrary::new(root.path().join("models")).await.unwrap());
        let source = library.library_root().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("source.gguf"), b"source fixture").unwrap();
        std::fs::write(source.join("source.safetensors"), b"source fixture").unwrap();
        let destination = library.library_root().join(format!("source-{suffix}"));
        std::fs::create_dir(&destination).unwrap();
        let old_metadata = destination.join("metadata.json");
        std::fs::write(&old_metadata, b"existing metadata must not change").unwrap();
        std::fs::write(destination.join("existing.bin"), b"existing payload").unwrap();
        let previous_stage = destination.with_extension(if backend_id == QuantBackend::LlamaCpp {
            "quantizing"
        } else {
            "converting"
        });
        std::fs::create_dir(&previous_stage).unwrap();
        std::fs::write(previous_stage.join("keep.bin"), b"other attempt").unwrap();
        let progress = ConversionProgressTracker::new();
        progress.insert(
            serde_json::from_value(serde_json::json!({
                "conversionId": "fixture", "sourceModelId": "source",
                "direction": "gguf_to_safetensors", "status": "converting"
            }))
            .unwrap(),
        );
        let metadata = ModelMetadata {
            family: Some("fixture".into()),
            model_type: Some("llm".into()),
            official_name: Some("Fixture".into()),
            ..Default::default()
        };
        let cancel = CancellationToken::new();
        if backend_id == QuantBackend::PythonConversion {
            executable(&scripts::venv_python(root.path()), DIRECTORY_WRITER);
            let importer = ModelImporter::new(Arc::clone(&library));
            run_conversion(
                "fixture",
                ConversionDirection::GgufToSafetensors,
                root.path(),
                &source,
                "source",
                Some(quant),
                metadata,
                &progress,
                &cancel,
                &library,
                &importer,
            )
            .await
            .unwrap();
        } else {
            let backend: Box<dyn QuantizationBackend> = match backend_id {
                QuantBackend::LlamaCpp => {
                    let backend = LlamaCppBackend::new(root.path());
                    executable(&backend.venv_python(), "#!/bin/sh\nexit 2\n");
                    executable(&backend.convert_script(), "#!/bin/sh\nexit 2\n");
                    executable(
                        &backend.quantize_binary(),
                        "#!/bin/sh\nprintf 'fixture payload' > \"$2\"\n",
                    );
                    Box::new(backend)
                }
                QuantBackend::Nvfp4 | QuantBackend::Sherry => {
                    let (directory, script) = if backend_id == QuantBackend::Nvfp4 {
                        ("nvfp4", "quantize_nvfp4.py")
                    } else {
                        ("sherry", "sherry_qat.py")
                    };
                    let base = root.path().join("launcher-data").join(directory);
                    executable(&base.join("venv/bin/python"), DIRECTORY_WRITER);
                    std::fs::write(base.join(script), b"unused fixture script").unwrap();
                    if backend_id == QuantBackend::Nvfp4 {
                        Box::new(Nvfp4Backend::new(root.path()))
                    } else {
                        Box::new(SherryBackend::new(root.path()))
                    }
                }
                QuantBackend::PythonConversion => unreachable!(),
            };
            let params = QuantizeParams {
                conversion_id: "fixture".into(),
                model_path: source,
                source_model_id: "source".into(),
                target_quant: quant.into(),
                calibration_file: None,
                force_imatrix: false,
            };
            run_quantization(
                "fixture",
                backend.as_ref(),
                params,
                "source",
                metadata,
                &progress,
                &cancel,
                &library,
            )
            .await
            .unwrap();
        }
        let actual = library.library_root().join(format!("source-{suffix}-v2"));
        assert!(actual.join("metadata.json").is_file(), "{backend_id:?}");
        assert_eq!(
            std::fs::read(old_metadata).unwrap(),
            b"existing metadata must not change"
        );
        assert_eq!(
            std::fs::read(destination.join("existing.bin")).unwrap(),
            b"existing payload"
        );
        assert_eq!(
            std::fs::read(previous_stage.join("keep.bin")).unwrap(),
            b"other attempt"
        );
        let actual_id = format!("source-{suffix}-v2");
        assert_eq!(
            progress.get("fixture").unwrap().output_model_id.as_deref(),
            Some(actual_id.as_str())
        );
        let indexed = library
            .get_model(&actual_id)
            .await
            .unwrap()
            .expect("actual output indexed");
        assert_eq!(Path::new(&indexed.path), actual);
        assert!(library
            .get_model(&format!("source-{suffix}"))
            .await
            .unwrap()
            .is_none());
    }
}
