//! An unavailable copied publication is a pre-admission refusal. Once guarded
//! importer effects start, failures remain owned and visible to shutdown.

use super::tests::{custody_fixture, orphan_spec};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn copied_import_in_place_readiness_refusal_does_not_poison_shutdown() {
    for damage in [
        "missing",
        "malformed",
        "erased_identity",
        "pending_receipt",
        "oversized",
    ] {
        let (temp, library, _downloads, tasks, root) = custody_fixture().await;
        let source = temp.path().join("source.onnx");
        std::fs::write(&source, b"synthetic payload").unwrap();
        let importer = ModelImporter::new(library.clone());
        let copied = importer
            .import(&ModelImportSpec {
                path: source.display().to_string(),
                family: "publisher".into(),
                official_name: "refusal".into(),
                repo_id: None,
                model_type: Some("vision".into()),
                subtype: None,
                tags: None,
                security_acknowledged: Some(true),
            })
            .await
            .unwrap();
        assert!(copied.success);
        let id = copied.model_id.unwrap();
        let model = library.library_root().join(&id);
        let spec = orphan_spec(&model);
        // Positive control: the same public path still admits a confirmed copy.
        assert!(importer.import_in_place(&spec).await.unwrap().success);
        let metadata_path = model.join("metadata.json");
        let receipt_path = model.join(publication::RECEIPT_FILENAME);
        let backup_path =
            model.join(crate::model_library::download_recovery::IMPORT_METADATA_BACKUP);
        let backup = std::fs::read(&metadata_path).unwrap();
        std::fs::write(&backup_path, &backup).unwrap();
        match damage {
            "missing" => std::fs::remove_file(&metadata_path).unwrap(),
            "malformed" => std::fs::write(&metadata_path, b"not JSON").unwrap(),
            "erased_identity" => {
                let mut metadata: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
                metadata
                    .as_object_mut()
                    .unwrap()
                    .remove("import_publication");
                std::fs::write(&metadata_path, serde_json::to_vec(&metadata).unwrap()).unwrap();
            }
            "oversized" => {
                std::fs::File::create(&metadata_path)
                    .unwrap()
                    .set_len(crate::model_library::download_recovery::IMPORT_DOCUMENT_MAX_BYTES + 1)
                    .unwrap();
            }
            _ => {
                let mut receipt: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
                receipt["state"] = "pending".into();
                std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
            }
        }
        let metadata_before = std::fs::read(&metadata_path).ok();
        let receipt_before = std::fs::read(&receipt_path).unwrap();
        let index_before = library.index().get(&id).unwrap().unwrap().metadata;
        let writes = Arc::new(AtomicUsize::new(0));
        let observed = writes.clone();
        library.set_metadata_write_notifier(Some(Arc::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
        })));
        // Observation needs no mutation grant, and no owner is admitted on
        // refusal. A valid backup must never substitute for canonical evidence.
        let competing_grant = root.try_acquire_execution_grant().unwrap();
        let error = importer.import_in_place(&spec).await.unwrap_err();
        drop(competing_grant);
        match damage {
            "malformed" => assert!(matches!(&error, PumasError::Json { .. }), "{error}"),
            "oversized" => assert!(
                matches!(&error, PumasError::Validation { field, .. } if field == publication::EVIDENCE_SIZE_FIELD),
                "{error}"
            ),
            _ => assert!(
                matches!(&error, PumasError::Validation { field, .. } if field == "import_publication"),
                "{damage}: {error}"
            ),
        }
        assert_eq!(std::fs::read(&backup_path).unwrap(), backup);
        assert_eq!(writes.load(Ordering::SeqCst), 0, "{damage}");
        assert_eq!(std::fs::read(&metadata_path).ok(), metadata_before);
        assert_eq!(std::fs::read(&receipt_path).unwrap(), receipt_before);
        assert_eq!(
            library.index().get(&id).unwrap().unwrap().metadata,
            index_before
        );
        assert_eq!(
            std::fs::read(model.join("source.onnx")).unwrap(),
            b"synthetic payload"
        );
        assert!(root.try_acquire_execution_grant().is_ok());
        library.set_metadata_write_notifier(None);
        tasks.shutdown_owned().await.unwrap();
        tasks.shutdown_owned().await.unwrap();
    }
}

#[tokio::test]
async fn copied_import_in_place_preflight_rejects_unowned_or_special_metadata() {
    let cases = [
        "outside_root",
        "oversized_legacy",
        #[cfg(unix)]
        "directory_symlink",
        #[cfg(unix)]
        "metadata_symlink",
    ];
    for case in cases {
        assert_preflight_metadata_refusal(case).await;
    }
}

async fn assert_preflight_metadata_refusal(case: &str) {
    let (temp, library, _downloads, tasks, root) = custody_fixture().await;
    let outside = temp.path().join("unowned");
    std::fs::create_dir(&outside).unwrap();
    // Reading this unowned content would yield Json, rather than the
    // containment/no-follow Io refusal required before parsing any bytes.
    let sentinel = b"unowned metadata must not be parsed";
    std::fs::write(outside.join("metadata.json"), sentinel).unwrap();
    std::fs::write(outside.join("detector.onnx"), b"unowned payload").unwrap();
    let model = library.build_model_path("vision", "publisher", "refusal");
    std::fs::create_dir_all(model.parent().unwrap()).unwrap();
    let target = match case {
        "outside_root" => outside.clone(),
        "oversized_legacy" => {
            std::fs::create_dir(&model).unwrap();
            std::fs::write(model.join("detector.onnx"), b"owned payload").unwrap();
            std::fs::File::create(model.join("metadata.json"))
                .unwrap()
                .set_len(crate::model_library::download_recovery::IMPORT_DOCUMENT_MAX_BYTES + 1)
                .unwrap();
            model.clone()
        }
        #[cfg(unix)]
        "directory_symlink" => {
            std::os::unix::fs::symlink(&outside, &model).unwrap();
            model.clone()
        }
        #[cfg(unix)]
        "metadata_symlink" => {
            std::fs::create_dir(&model).unwrap();
            std::fs::write(model.join("detector.onnx"), b"owned payload").unwrap();
            std::os::unix::fs::symlink(outside.join("metadata.json"), model.join("metadata.json"))
                .unwrap();
            model.clone()
        }
        #[cfg(unix)]
        "metadata_fifo" => {
            std::fs::create_dir(&model).unwrap();
            std::fs::write(model.join("detector.onnx"), b"owned payload").unwrap();
            rustix::fs::mkfifoat(
                rustix::fs::CWD,
                model.join("metadata.json"),
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .unwrap();
            model.clone()
        }
        _ => unreachable!(),
    };
    let writes = Arc::new(AtomicUsize::new(0));
    let observed = writes.clone();
    library.set_metadata_write_notifier(Some(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
    })));
    let competing_grant = root.try_acquire_execution_grant().unwrap();
    // The FIFO caller runs this entire future in an owned child process.
    // Never drop it on a timer while its blocking observation may be live.
    let error = ModelImporter::new(library.clone())
        .import_in_place(&orphan_spec(&target))
        .await
        .unwrap_err();
    drop(competing_grant);
    if case == "oversized_legacy" {
        assert!(
            matches!(&error, PumasError::Validation { field, .. } if field == publication::EVIDENCE_SIZE_FIELD),
            "{error}"
        );
        assert_eq!(
            std::fs::metadata(model.join("metadata.json"))
                .unwrap()
                .len(),
            crate::model_library::download_recovery::IMPORT_DOCUMENT_MAX_BYTES + 1,
        );
    } else {
        assert!(matches!(&error, PumasError::Io { .. }), "{case}: {error}");
    }
    #[cfg(unix)]
    match case {
        "directory_symlink" => assert_eq!(std::fs::read_link(&model).unwrap(), outside),
        "metadata_symlink" => assert_eq!(
            std::fs::read_link(model.join("metadata.json")).unwrap(),
            outside.join("metadata.json"),
        ),
        "metadata_fifo" => {
            use std::os::unix::fs::FileTypeExt;
            assert!(std::fs::symlink_metadata(model.join("metadata.json"))
                .unwrap()
                .file_type()
                .is_fifo());
        }
        _ => {}
    }
    if case != "outside_root" && case != "directory_symlink" {
        assert_eq!(
            std::fs::read(model.join("detector.onnx")).unwrap(),
            b"owned payload"
        );
    }
    assert_eq!(writes.load(Ordering::SeqCst), 0, "{case}");
    assert_eq!(
        std::fs::read(outside.join("metadata.json")).unwrap(),
        sentinel
    );
    assert_eq!(
        std::fs::read(outside.join("detector.onnx")).unwrap(),
        b"unowned payload"
    );
    assert!(!outside.join(publication::RECEIPT_FILENAME).exists());
    assert!(!model.join(publication::RECEIPT_FILENAME).exists());
    assert!(library
        .index()
        .get("vision/publisher/refusal")
        .unwrap()
        .is_none());
    assert!(root.try_acquire_execution_grant().is_ok());
    library.set_metadata_write_notifier(None);
    tasks.shutdown_owned().await.unwrap();
    tasks.shutdown_owned().await.unwrap();
}

#[cfg(unix)]
struct FifoProbeChild(Option<std::process::Child>);

#[cfg(unix)]
impl FifoProbeChild {
    fn child(&mut self) -> &mut std::process::Child {
        self.0.as_mut().expect("FIFO probe child already reaped")
    }
}

#[cfg(unix)]
impl Drop for FifoProbeChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            // Assertion failures keep ownership of this exact process until
            // termination/reaping; never detach a blocking reader or a reaper.
            let kill = child.kill();
            if let Err(error) = child.wait() {
                eprintln!("FIFO probe child cleanup failed: kill={kill:?}, wait={error}");
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn copied_import_in_place_preflight_rejects_fifo_in_owned_child() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const CHILD_ROOT: &str = "PUMAS_IMPORT_FIFO_PROBE_ROOT";
    const ACKNOWLEDGMENT: &[u8] = b"FIFO refused; shutdown clean; runtime drained";
    const CHILD_TEST: &str = "model_library::importer::admission_tests::copied_import_in_place_preflight_rejects_fifo_in_owned_child";

    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(assert_preflight_metadata_refusal("metadata_fifo"));
        drop(runtime);
        std::fs::write(Path::new(&root).join("fifo-completed"), ACKNOWLEDGMENT).unwrap();
        return;
    }

    // This parent also owns all child-created temp fixtures. If the child must
    // be killed, its leftover FIFO and scratch files are removed after reaping.
    let temp = tempfile::tempdir().unwrap();
    let mut child = FifoProbeChild(Some(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_TEST, "--nocapture"])
            .env(CHILD_ROOT, temp.path())
            .env("TMPDIR", temp.path())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    ));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.child().try_wait().unwrap() {
            drop(child.0.take()); // try_wait observed and reaped this exact child.
            break status;
        }
        if started.elapsed() >= Duration::from_secs(30) {
            let kill = child.child().kill();
            let reap = child.child().wait();
            if reap.is_ok() {
                drop(child.0.take());
            }
            panic!("FIFO probe timed out; child kill={kill:?}, reap={reap:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "FIFO probe child failed: {status}");
    assert_eq!(
        std::fs::read(temp.path().join("fifo-completed")).unwrap(),
        ACKNOWLEDGMENT,
        "the exact child test must execute and drain its runtime",
    );
}

#[tokio::test]
async fn custody_guard_in_place_admitted_effect_failures_remain_owned() {
    for failure in ["publication_gate", "io", "panic"] {
        let (_temp, library, _downloads, tasks, root) = custody_fixture().await;
        let model = library.build_model_path("vision", "publisher", "effect-failure");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("detector.onnx"), b"data").unwrap();
        let mutation_root = root.clone();
        library.set_metadata_write_notifier(Some(Arc::new(move |path| {
            assert!(matches!(
                mutation_root.try_acquire_execution_grant(),
                Err(PumasError::DownloadRootBusy)
            ));
            let directory = path.parent().unwrap();
            std::fs::write(directory.join("effect-entered"), failure).unwrap();
            match failure {
                "publication_gate" => {
                    // A new claim appears only after admission, at the real
                    // write boundary. The same error field must remain a failed
                    // owned effect rather than a global Validation exemption.
                    let metadata = ModelMetadata {
                        import_publication: Some(crate::models::ImportPublicationIdentity {
                            version: 1,
                            id: uuid::Uuid::new_v4().to_string(),
                            confirmed: false,
                        }),
                        import_state: Some(crate::models::ImportState::Pending),
                        validation_state: Some(crate::models::AssetValidationState::Invalid),
                        ..Default::default()
                    };
                    std::fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
                }
                "io" => std::fs::create_dir(&path).unwrap(),
                _ => panic!("admitted importer effect failed"),
            }
        })));
        let error = ModelImporter::new(library.clone())
            .import_in_place(&orphan_spec(&model))
            .await
            .unwrap_err();
        match failure {
            "publication_gate" => assert!(
                matches!(&error, PumasError::Validation { field, .. } if field == "import_publication"),
                "{error}"
            ),
            "io" => assert!(matches!(&error, PumasError::Io { .. }), "{error}"),
            _ => assert!(
                error
                    .to_string()
                    .contains("Failed to join metadata projection")
                    || error.to_string().contains("panicked"),
                "{error}"
            ),
        }
        assert_eq!(
            std::fs::read(model.join("effect-entered")).unwrap(),
            failure.as_bytes()
        );
        assert_eq!(std::fs::read(model.join("detector.onnx")).unwrap(), b"data");
        assert!(root.try_acquire_execution_grant().is_ok());
        assert!(library
            .index()
            .get(&library.get_model_id(&model).unwrap())
            .unwrap()
            .is_none());
        library.set_metadata_write_notifier(None);
        let first = tasks.shutdown_owned().await.unwrap_err().to_string();
        assert!(first.contains("import model in place"), "{first}");
        assert_eq!(tasks.shutdown_owned().await.unwrap_err().to_string(), first);
    }
}
