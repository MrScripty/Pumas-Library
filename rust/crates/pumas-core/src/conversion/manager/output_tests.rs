//! Consumer regression: simulated executables write tiny fixture payloads; no
//! model tools, downloads, GPU or real model data are used.

use super::*;
use std::os::unix::fs::PermissionsExt;

fn executable(path: &Path, script: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

const DIRECTORY_WRITER: &str = r#"#!/bin/sh
if test "$1" = '-I'; then exit 0; fi
printf 'fixture diagnostic\n' >&2
printf '{"stage":"converting","tensor_index":1,"tensor_count":2,"tensor_name":"fixture.weight"}\n'
while [ $# -gt 0 ]; do
 if [ "$1" = '--output-dir' ]; then printf 'fixture payload' > "$2/model.safetensors"; exit 0; fi
 shift
done
exit 2
"#;

#[cfg(target_os = "linux")]
#[tokio::test]
async fn script_observations_wait_for_cleanup_import_and_worker_receipt() {
    use std::time::Duration;
    for outcome in [
        "success",
        "exit_failure",
        "script_failure",
        "cancel",
        "script_and_exit_failure",
        "script_then_cancel",
    ] {
        let reports_error = matches!(
            outcome,
            "script_failure" | "script_and_exit_failure" | "script_then_cancel"
        );
        let cancels = matches!(outcome, "cancel" | "script_then_cancel");
        let root = tempfile::tempdir().unwrap();
        let library = Arc::new(ModelLibrary::new(root.path().join("models")).await.unwrap());
        let source = library.library_root().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("source.gguf"), b"source fixture").unwrap();
        library
            .save_metadata(&source, &ModelMetadata::default())
            .await
            .unwrap();
        library.index_model_dir(&source).await.unwrap();
        let python = scripts::venv_python(root.path());
        let release_child = python.with_file_name("python.release");
        let script_error = if reports_error {
            "printf '{\"stage\":\"error\",\"message\":\"controlled script failure\"}\\n'"
        } else {
            ":"
        };
        let exit_code = if matches!(outcome, "exit_failure" | "script_and_exit_failure") {
            7
        } else {
            0
        };
        executable(
            &python,
            &format!(
                r#"#!/bin/sh
{script_error}
printf '{{"stage":"complete","output_size":17}}\n'
attempt=0
while ! test -f "$0.release"; do
 attempt=$((attempt + 1))
 if test "$attempt" -ge 500; then exit 9; fi
 sleep 0.02
done
while [ $# -gt 0 ]; do
 if [ "$1" = '--output-dir' ]; then printf 'fixture payload' > "$2/model.safetensors"; exit {exit_code}; fi
 shift
done
exit 2
"#
            ),
        );
        let (entered, held_import) = tokio::sync::oneshot::channel();
        let entered = std::sync::Mutex::new(Some(entered));
        let (release_import, held) = std::sync::mpsc::channel();
        let held = std::sync::Mutex::new(held);
        library.set_metadata_write_notifier(Some(Arc::new(move |_| {
            if let Some(entered) = entered.lock().unwrap().take() {
                let _ = entered.send(());
                let _ = held.lock().unwrap().recv_timeout(Duration::from_secs(5));
            }
        })));
        let manager = ConversionManager::new(
            root.path().to_path_buf(),
            library.clone(),
            Arc::new(ModelImporter::new(library.clone())),
        );
        let id = manager
            .start_conversion(ConversionRequest {
                model_id: "source".into(),
                direction: ConversionDirection::GgufToSafetensors,
                target_quant: Some("F16".into()),
                output_name: None,
                imatrix_calibration_file: None,
                force_imatrix: None,
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if manager.get_progress(&id).unwrap().estimated_output_size == Some(17) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("script completion record observed");
        let held_progress = manager.get_progress(&id).unwrap();
        assert_eq!(held_progress.status, ConversionStatus::Writing, "{outcome}");
        assert_eq!(held_progress.progress, Some(0.95));
        assert_eq!(held_progress.error, None);
        assert_eq!(held_progress.output_model_id, None);
        assert_eq!(
            manager.list_conversions()[0].status,
            ConversionStatus::Writing
        );
        assert!(!library.library_root().join("source-safetensors").exists());
        if cancels {
            assert!(manager.cancel_conversion(&id).await.unwrap());
        } else {
            std::fs::write(&release_child, "").unwrap();
        }
        if outcome == "success" {
            tokio::time::timeout(Duration::from_secs(5), held_import)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                manager.get_progress(&id).unwrap().status,
                ConversionStatus::Importing
            );
            assert_eq!(manager.list_conversions()[0].output_model_id, None);
            assert!(library.index().get("source-safetensors").unwrap().is_none());
            release_import.send(()).unwrap();
        }
        let terminal = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let observed = manager.get_progress(&id).unwrap();
                if matches!(
                    observed.status,
                    ConversionStatus::Completed
                        | ConversionStatus::Error
                        | ConversionStatus::Cancelled
                ) {
                    break observed;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("observed worker terminal receipt");
        match outcome {
            "success" => {
                assert_eq!(terminal.status, ConversionStatus::Completed);
                assert_eq!(terminal.progress, Some(1.0));
                assert_eq!(
                    terminal.output_model_id.as_deref(),
                    Some("source-safetensors")
                );
                assert!(library
                    .get_model("source-safetensors")
                    .await
                    .unwrap()
                    .is_some());
                manager.shutdown().await.unwrap();
            }
            "cancel" | "script_then_cancel" => {
                assert_eq!(terminal.status, ConversionStatus::Cancelled);
                assert_eq!(terminal.error, None);
                manager.shutdown().await.unwrap();
            }
            _ => {
                assert_eq!(terminal.status, ConversionStatus::Error);
                let error = terminal.error.unwrap();
                if reports_error {
                    assert!(error.contains("controlled script failure"));
                }
                if exit_code != 0 {
                    assert!(error.contains("subprocess exited unsuccessfully"));
                }
                assert!(manager.shutdown().await.is_err());
            }
        }
        if outcome != "success" {
            assert_eq!(terminal.output_model_id, None);
            assert!(!library.library_root().join("source-safetensors").exists());
        }
        library.set_metadata_write_notifier(None);
    }
}

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
                    executable(
                        &backend.quantize_binary(),
                        "#!/bin/sh\nprintf 'fixture diagnostic\\n'\nprintf '[ 1/ 2] fixture.weight\\n' >&2\nprintf 'fixture payload' > \"$2\"\n",
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
            progress.get("fixture").unwrap().status,
            ConversionStatus::Importing,
            "pipeline supplies output identity but only a worker receipt completes the operation"
        );
        if matches!(
            backend_id,
            QuantBackend::PythonConversion | QuantBackend::LlamaCpp
        ) {
            let observed = progress.get("fixture").unwrap();
            assert_eq!(observed.tensors_completed, Some(1), "{backend_id:?}");
            assert_eq!(observed.tensors_total, Some(2), "{backend_id:?}");
            assert_eq!(observed.current_tensor.as_deref(), Some("fixture.weight"));
        } else {
            // Quantization advances to finalization, clearing the prior step's
            // counters. Held-phase tests observe script progress before this.
            let observed = progress.get("fixture").unwrap();
            assert_eq!(observed.tensors_completed, None, "{backend_id:?}");
            assert_eq!(observed.tensors_total, None, "{backend_id:?}");
            assert_eq!(observed.current_tensor, None, "{backend_id:?}");
        }
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

#[cfg(target_os = "linux")]
#[tokio::test]
async fn quantization_scripts_expose_phases_and_defer_failures_before_publication() {
    use std::time::Duration;
    for (directory, script_name, direction, quant, phase, expected, fraction) in [
        (
            "nvfp4",
            "quantize_nvfp4.py",
            ConversionDirection::SafetensorsToNvfp4,
            "NVFP4",
            r#"{"stage":"quantizing"}"#,
            ConversionStatus::Quantizing,
            None,
        ),
        (
            "sherry",
            "sherry_qat.py",
            ConversionDirection::SafetensorsToSherryQat,
            "Sherry-1.25bit",
            r#"{"stage":"training","epoch":2,"epochs_total":4}"#,
            ConversionStatus::Training,
            Some(0.25),
        ),
    ] {
        for fails in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let library = Arc::new(ModelLibrary::new(root.path().join("models")).await.unwrap());
            let source = library.library_root().join("source");
            std::fs::create_dir(&source).unwrap();
            std::fs::write(source.join("source.safetensors"), b"source fixture").unwrap();
            library
                .save_metadata(&source, &ModelMetadata::default())
                .await
                .unwrap();
            library.index_model_dir(&source).await.unwrap();
            let base = root.path().join("launcher-data").join(directory);
            let python = base.join("venv/bin/python");
            let diagnostic = if fails {
                "printf '%s\\n' '{\"stage\":\"error\",\"message\":\"controlled quantization failure\"}'"
            } else {
                ":"
            };
            executable(
                &python,
                &format!(
                    r#"#!/bin/sh
if test "$1" = '-I'; then exit 0; fi
{diagnostic}
printf '%s\n' '{phase}'
attempt=0
while ! test -f "$0.release"; do
 attempt=$((attempt + 1))
 if test "$attempt" -ge 500; then exit 9; fi
 sleep 0.02
done
printf '%s\n' '{{"stage":"complete"}}'
while [ $# -gt 0 ]; do
 if [ "$1" = '--output-dir' ]; then printf 'fixture payload' > "$2/model.safetensors"; exit 0; fi
 shift
done
exit 2
"#
                ),
            );
            std::fs::write(base.join(script_name), b"unused fixture script").unwrap();
            let manager = ConversionManager::new(
                root.path().to_path_buf(),
                library.clone(),
                Arc::new(ModelImporter::new(library.clone())),
            );
            let id = manager
                .start_conversion(ConversionRequest {
                    model_id: "source".into(),
                    direction,
                    target_quant: Some(quant.into()),
                    output_name: None,
                    imatrix_calibration_file: None,
                    force_imatrix: None,
                })
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let observed = manager.get_progress(&id).unwrap();
                    if observed.status == expected && observed.progress == fraction {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("actual backend script phase projected");
            let observed = manager.list_conversions().pop().unwrap();
            assert_eq!(observed.error, None);
            assert_eq!(observed.output_model_id, None);
            assert_eq!(library.list_models().await.unwrap().len(), 1);
            std::fs::write(python.with_file_name("python.release"), "").unwrap();
            let terminal = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let observed = manager.get_progress(&id).unwrap();
                    if matches!(
                        observed.status,
                        ConversionStatus::Completed | ConversionStatus::Error
                    ) {
                        break observed;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("quantization receipt observed");
            if fails {
                assert_eq!(terminal.status, ConversionStatus::Error);
                assert!(terminal
                    .error
                    .unwrap()
                    .contains("controlled quantization failure"));
                assert_eq!(terminal.output_model_id, None);
                assert_eq!(library.list_models().await.unwrap().len(), 1);
                assert!(manager.shutdown().await.is_err());
            } else {
                assert_eq!(terminal.status, ConversionStatus::Completed);
                assert_eq!(terminal.progress, Some(1.0));
                assert!(library
                    .get_model(&terminal.output_model_id.unwrap())
                    .await
                    .unwrap()
                    .is_some());
                manager.shutdown().await.unwrap();
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn interrupted_shutdown_still_reaps_quiet_conversion_without_publishing() {
    let root = tempfile::tempdir().unwrap();
    let library = Arc::new(ModelLibrary::new(root.path().join("models")).await.unwrap());
    let source = library.library_root().join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("source.gguf"), b"source fixture").unwrap();
    let python = scripts::venv_python(root.path());
    executable(&python, "#!/bin/sh\necho $$ > \"$0.pid\"\nexec sleep 30\n");
    let marker = python.with_file_name("python.pid");
    let progress = Arc::new(ConversionProgressTracker::new());
    let owner = crate::conversion::workers::WorkerOwner::new(progress.clone(), 1);
    let token = CancellationToken::new();
    let initial = serde_json::from_value(serde_json::json!({
        "conversionId": "quiet", "sourceModelId": "source",
        "direction": "gguf_to_safetensors", "status": "converting"
    }))
    .unwrap();
    let task_root = root.path().to_path_buf();
    let task_library = library.clone();
    let task_progress = progress.clone();
    owner
        .spawn(initial, token.clone(), async move {
            let importer = ModelImporter::new(task_library.clone());
            run_conversion(
                "quiet",
                ConversionDirection::GgufToSafetensors,
                &task_root,
                &source,
                "source",
                Some("F16"),
                ModelMetadata::default(),
                &task_progress,
                &token,
                &task_library,
                &importer,
            )
            .await
        })
        .unwrap();

    let pid: u32 = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Ok(contents) = tokio::fs::read_to_string(&marker).await {
                if let Ok(pid) = contents.trim().parse() {
                    break pid;
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("quiet native conversion started");
    let mut interrupted = Box::pin(owner.shutdown());
    assert!(futures::poll!(interrupted.as_mut()).is_pending());
    drop(interrupted);
    tokio::time::timeout(std::time::Duration::from_secs(5), owner.shutdown())
        .await
        .expect("retained shutdown cancels quiet native child")
        .expect("cancellation observed");

    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "foreground child reaped"
    );
    let observed = progress.get("quiet").unwrap();
    assert_eq!(observed.status, ConversionStatus::Cancelled);
    assert_eq!(observed.output_model_id, None);
    assert!(!library.library_root().join("source-safetensors").exists());
    assert!(library
        .get_model("source-safetensors")
        .await
        .unwrap()
        .is_none());
    let stages = std::fs::read_dir(library.library_root())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".pumas-conversion-")
        })
        .count();
    assert_eq!(
        stages, 1,
        "staging retained pending process-tree cleanup policy"
    );
}
