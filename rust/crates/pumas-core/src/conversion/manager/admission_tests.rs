//! Managed request contracts; isolated metadata/file fixtures, no model tools.

use super::*;

async fn manager(root: &Path) -> ConversionManager {
    let library = Arc::new(ModelLibrary::new(root.join("models")).await.unwrap());
    let source = library.library_root().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("source.gguf"), b"source fixture").unwrap();
    std::fs::write(source.join("source.safetensors"), b"source fixture").unwrap();
    library
        .save_metadata(&source, &ModelMetadata::default())
        .await
        .unwrap();
    library.index_model_dir(&source).await.unwrap();
    ConversionManager::new(
        root.to_path_buf(),
        library.clone(),
        Arc::new(ModelImporter::new(library)),
    )
}

fn request(direction: ConversionDirection, target: Option<&str>) -> ConversionRequest {
    ConversionRequest {
        model_id: "source".into(),
        direction,
        target_quant: target.map(str::to_owned),
        output_name: None,
        imatrix_calibration_file: None,
        force_imatrix: None,
    }
}

async fn rejected(manager: &ConversionManager, request: ConversionRequest, expected: &str) {
    let root = manager.model_library.library_root();
    let entries = || {
        std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let before = entries();
    match manager.start_conversion(request).await {
        Err(PumasError::InvalidParams { message }) => assert_eq!(message, expected),
        result => panic!("expected InvalidParams({expected}), received {result:?}"),
    }
    assert!(manager.list_conversions().is_empty());
    assert!(!manager.launcher_root.join("launcher-data").exists());
    assert_eq!(manager.model_library.list_models().await.unwrap().len(), 1);
    assert_eq!(entries(), before, "no staging or output");
    assert_eq!(
        std::fs::read(root.join("source/source.gguf")).unwrap(),
        b"source fixture"
    );
}

#[tokio::test]
async fn managed_targets_are_backend_qualified_and_exact_before_admission() {
    let root = tempfile::tempdir().unwrap();
    let manager = manager(root.path()).await;
    for (direction, name, foreign) in [
        (
            ConversionDirection::GgufToQuantizedGguf,
            "llama.cpp",
            "NVFP4",
        ),
        (
            ConversionDirection::SafetensorsToQuantizedGguf,
            "llama.cpp",
            "Sherry-1.25bit",
        ),
        (ConversionDirection::SafetensorsToNvfp4, "nvfp4", "Q4_K_M"),
        (
            ConversionDirection::SafetensorsToSherryQat,
            "sherry",
            "Q4_K_M",
        ),
    ] {
        for target in [foreign, "", "q4_k_m", " Q4_K_M", "../Q4_K_M", "--help"] {
            rejected(
                &manager,
                request(direction, Some(target)),
                &format!("Unsupported target quantization for {name}"),
            )
            .await;
        }
    }
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn managed_calibration_requirements_do_not_allow_false_to_bypass_iq() {
    let root = tempfile::tempdir().unwrap();
    let manager = manager(root.path()).await;
    for direction in [
        ConversionDirection::GgufToQuantizedGguf,
        ConversionDirection::SafetensorsToQuantizedGguf,
    ] {
        for (target, force) in [
            ("IQ3_XXS", None),
            ("IQ3_XXS", Some(false)),
            ("Q4_K_M", Some(true)),
        ] {
            let mut input = request(direction, Some(target));
            input.force_imatrix = force;
            rejected(
                &manager,
                input,
                "IQ targets and forced importance matrices require a calibration file",
            )
            .await;
        }
    }
    for direction in [
        ConversionDirection::SafetensorsToNvfp4,
        ConversionDirection::SafetensorsToSherryQat,
    ] {
        let mut input = request(direction, None);
        input.force_imatrix = Some(true);
        rejected(
            &manager,
            input,
            "force_imatrix is only supported by llama.cpp",
        )
        .await;
    }
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn managed_supplied_calibration_is_checked_even_when_optional() {
    let root = tempfile::tempdir().unwrap();
    let manager = manager(root.path()).await;
    let empty = root.path().join("empty.txt");
    std::fs::write(&empty, "").unwrap();
    for direction in [
        ConversionDirection::GgufToQuantizedGguf,
        ConversionDirection::SafetensorsToQuantizedGguf,
        ConversionDirection::SafetensorsToNvfp4,
        ConversionDirection::SafetensorsToSherryQat,
    ] {
        for (path, expected) in [
            (
                root.path().join("missing.txt"),
                "Calibration file does not exist",
            ),
            (PathBuf::new(), "Calibration file does not exist"),
            (
                root.path().to_path_buf(),
                "Calibration must be a nonempty regular file",
            ),
            (empty.clone(), "Calibration must be a nonempty regular file"),
        ] {
            let mut input = request(direction, None);
            input.imatrix_calibration_file = Some(path.to_str().unwrap().into());
            rejected(&manager, input, expected).await;
        }
    }
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn preparation_preserves_catalog_targets_defaults_and_calibration_paths() {
    let root = tempfile::tempdir().unwrap();
    let manager = manager(root.path()).await;
    let calibration = root.path().join("calibration text.txt");
    std::fs::write(&calibration, "fixture calibration").unwrap();
    for (id, direction, default) in [
        (
            QuantBackend::LlamaCpp,
            ConversionDirection::GgufToQuantizedGguf,
            "Q4_K_M",
        ),
        (
            QuantBackend::Nvfp4,
            ConversionDirection::SafetensorsToNvfp4,
            "NVFP4",
        ),
        (
            QuantBackend::Sherry,
            ConversionDirection::SafetensorsToSherryQat,
            "Sherry-1.25bit",
        ),
    ] {
        let backend = manager
            .backends
            .iter()
            .find(|b| b.backend_id() == id)
            .unwrap();
        let options = backend.supported_quant_types();
        for target in
            std::iter::once(None).chain(options.iter().map(|option| Some(option.name.as_str())))
        {
            let mut input = request(direction, target);
            input.force_imatrix = Some(false);
            // Exercise missing optional calibration as well as exact supplied paths.
            for supplied in [false, true] {
                if !supplied && target.is_some_and(|name| name.starts_with("IQ")) {
                    continue;
                }
                input.imatrix_calibration_file =
                    supplied.then(|| calibration.to_str().unwrap().into());
                let (selected, params) = manager
                    .prepare_backend_quantization(
                        id,
                        backend.name(),
                        "fixture",
                        &root.path().join("models/source"),
                        "source",
                        input.target_quant.clone(),
                        &input,
                    )
                    .await
                    .unwrap();
                assert_eq!(selected.backend_id(), id);
                assert_eq!(params.target_quant, target.unwrap_or(default));
                assert_eq!(
                    params.calibration_file,
                    supplied.then(|| calibration.clone())
                );
                assert!(!params.force_imatrix);
            }
        }
    }
    assert!(manager.list_conversions().is_empty());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn every_managed_direction_reports_environment_busy_before_execution_effects() {
    let root = tempfile::tempdir().unwrap();
    let manager = manager(root.path()).await;
    let environment = root.path().join("launcher-data");
    std::fs::create_dir(&environment).unwrap();
    let lease = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(environment.join("conversion-setup.lock"))
        .unwrap();
    fs2::FileExt::try_lock_exclusive(&lease).unwrap();
    let entries = || {
        std::fs::read_dir(manager.model_library.library_root())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let before = entries();
    for (direction, target) in [
        (ConversionDirection::GgufToSafetensors, "F16"),
        (ConversionDirection::SafetensorsToGguf, "F16"),
        (ConversionDirection::GgufToQuantizedGguf, "Q4_K_M"),
        (ConversionDirection::SafetensorsToQuantizedGguf, "Q4_K_M"),
        (ConversionDirection::SafetensorsToNvfp4, "NVFP4"),
        (
            ConversionDirection::SafetensorsToSherryQat,
            "Sherry-1.25bit",
        ),
    ] {
        let id = manager
            .start_conversion(request(direction, Some(target)))
            .await
            .unwrap();
        let terminal = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let progress = manager.get_progress(&id).unwrap();
                if progress.status == ConversionStatus::Error {
                    return progress;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("busy worker result observed");
        assert!(
            terminal.error.unwrap().contains("environment is busy"),
            "{direction:?}"
        );
        assert_eq!(terminal.output_model_id, None);
        assert_eq!(
            std::fs::read_dir(&environment).unwrap().count(),
            1,
            "only held lock exists; no scripts or backend environment"
        );
        assert_eq!(entries(), before, "no output or staging");
        assert_eq!(manager.model_library.list_models().await.unwrap().len(), 1);
        assert_eq!(
            std::fs::read(root.path().join("models/source/source.gguf")).unwrap(),
            b"source fixture"
        );
        assert_eq!(
            std::fs::read(root.path().join("models/source/source.safetensors")).unwrap(),
            b"source fixture"
        );
    }
    assert!(manager
        .shutdown()
        .await
        .unwrap_err()
        .to_string()
        .contains("environment is busy"));
    fs2::FileExt::unlock(&lease).unwrap();
}
