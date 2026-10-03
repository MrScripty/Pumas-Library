use super::*;
use crate::api::RuntimeTasks;
use crate::model_library::{DownloadDestinationRoot, DownloadPersistence};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

fn receipt_at(directory: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(directory.join(RECEIPT_FILENAME)).unwrap()).unwrap()
}

struct Fixture {
    temp: TempDir,
    library: Arc<ModelLibrary>,
    tasks: RuntimeTasks,
    importer: ModelImporter,
    spec: ModelImportSpec,
}

impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let library = Arc::new(
            ModelLibrary::new(temp.path().join("library"))
                .await
                .unwrap(),
        );
        let tasks = RuntimeTasks::new();
        // Match composition setup: the strict custody store requires its
        // configured parent to exist even before downloads.json is created.
        std::fs::create_dir(temp.path().join("downloads")).unwrap();
        library
            .install_mutation_authority(
                tasks.clone(),
                DownloadDestinationRoot::open(library.library_root()).unwrap(),
                Arc::new(DownloadPersistence::new(&temp.path().join("downloads"))),
            )
            .unwrap();
        let source = temp.path().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("Model.onnx"), b"synthetic ONNX payload").unwrap();
        let spec = ModelImportSpec {
            path: source.display().to_string(),
            family: "fixture".into(),
            official_name: "Owned Import".into(),
            repo_id: None,
            model_type: Some("vision".into()),
            subtype: None,
            tags: None,
            security_acknowledged: Some(true),
        };
        let importer = ModelImporter::new(library.clone());
        Self {
            temp,
            library,
            tasks,
            importer,
            spec,
        }
    }

    fn stages(&self) -> Vec<PathBuf> {
        std::fs::read_dir(self.library.library_root())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(TEMP_IMPORT_PREFIX)
            })
            .collect()
    }

    fn target(&self) -> PathBuf {
        self.library
            .build_model_path("vision", "fixture", "Owned Import")
    }
}

fn fault(label: &str) -> PumasError {
    io::Error::other(label.to_string()).into()
}

#[tokio::test]
async fn copied_import_ownerless_refuses_before_staging() {
    let temp = tempfile::tempdir().unwrap();
    let library = Arc::new(ModelLibrary::new(temp.path()).await.unwrap());
    let importer = ModelImporter::new(library.clone());
    let fixture = Fixture::new().await;
    let (tx, _rx) = mpsc::channel(1);
    for result in [
        importer.import(&fixture.spec).await,
        importer.import_with_progress(&fixture.spec, tx).await,
    ] {
        assert!(
            matches!(result, Err(PumasError::Config { ref message }) if message.contains("PumasApi"))
        );
    }
    assert!(!std::fs::read_dir(temp.path()).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(TEMP_IMPORT_PREFIX)));
    assert_eq!(library.model_dirs().count(), 0);
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_success_keeps_final_identity_source_permissions_and_index() {
    let fixture = Fixture::new().await;
    let source = Path::new(&fixture.spec.path).join("Model.onnx");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o440)).unwrap();
    }
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(result.success, "{:?}", result.error);
    let id = result.model_id.unwrap();
    let target = fixture.library.library_root().join(&id);
    let metadata = fixture.library.load_metadata(&target).unwrap().unwrap();
    assert_eq!(metadata.model_id.as_deref(), Some(id.as_str()));
    assert_eq!(
        std::fs::read(&source).unwrap(),
        std::fs::read(target.join("model.onnx")).unwrap()
    );
    assert!(metadata.hashes.unwrap().sha256.is_some());
    assert_eq!(
        metadata.recommended_backend.as_deref(),
        Some("onnx-runtime")
    );
    let indexed = fixture.library.index().get(&id).unwrap().unwrap();
    assert_eq!(
        indexed.metadata["recommended_backend"].as_str(),
        Some("onnx-runtime")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(target.join("model.onnx"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o440
        );
        assert_eq!(
            std::fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            0o440
        );
    }
    assert!(fixture.stages().is_empty());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_collision_refusals_do_not_poison_shutdown() {
    for progress in [false, true] {
        let fixture = Fixture::new().await;
        let source = Path::new(&fixture.spec.path);
        std::fs::write(source.join("Model A.onnx"), b"one").unwrap();
        std::fs::write(source.join("Model_A.onnx"), b"two").unwrap();
        let result = if progress {
            let (tx, _rx) = mpsc::channel(1);
            fixture
                .importer
                .import_with_progress(&fixture.spec, tx)
                .await
        } else {
            fixture.importer.import(&fixture.spec).await
        }
        .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("normalized import filename"));
        assert!(fixture.stages().is_empty());
        assert!(!fixture.target().exists());
        assert_eq!(std::fs::read(source.join("Model A.onnx")).unwrap(), b"one");
        assert_eq!(std::fs::read(source.join("Model_A.onnx")).unwrap(), b"two");
        fixture.tasks.shutdown_owned().await.unwrap();
    }
}

#[tokio::test]
async fn copied_import_partial_copy_collision_removes_only_workspace() {
    for progress in [false, true] {
        let mut fixture = Fixture::new().await;
        std::fs::write(Path::new(&fixture.spec.path).join("Z.onnx"), b"second").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
            if boundary == ImportBoundary::FileCopied && calls.fetch_add(1, Ordering::SeqCst) == 0 {
                use std::io::Write;
                stage
                    .create_import_file("z.onnx")?
                    .write_all(b"first held writer")?;
            }
            Ok(())
        }));
        let result = if progress {
            let (tx, _rx) = mpsc::channel(1);
            fixture
                .importer
                .import_with_progress(&fixture.spec, tx)
                .await
        } else {
            fixture.importer.import(&fixture.spec).await
        }
        .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("filesystem-equivalent"));
        assert!(fixture.stages().is_empty());
        assert!(!fixture.target().exists());
        assert_eq!(
            std::fs::read(Path::new(&fixture.spec.path).join("Z.onnx")).unwrap(),
            b"second"
        );
        fixture.tasks.shutdown_owned().await.unwrap();
    }
}

#[tokio::test]
async fn copied_import_effect_failures_settle_and_remain_owner_visible() {
    for boundary in [
        ImportBoundary::StageCreated,
        ImportBoundary::FileCopied,
        ImportBoundary::BeforeHash,
        ImportBoundary::BeforeMetadata,
        ImportBoundary::BeforePublish,
    ] {
        let mut fixture = Fixture::new().await;
        fixture.importer.import_hook = Some(Arc::new(move |current, _| {
            if current == boundary {
                Err(fault("injected preparation failure"))
            } else {
                Ok(())
            }
        }));
        let error = fixture.importer.import(&fixture.spec).await.unwrap_err();
        assert!(error.to_string().contains("injected preparation failure"));
        assert!(fixture.stages().is_empty());
        assert!(!fixture.target().exists());
        assert!(fixture
            .tasks
            .shutdown_owned()
            .await
            .unwrap_err()
            .to_string()
            .contains("injected preparation failure"));
    }
}

#[tokio::test]
async fn copied_import_parent_creation_failure_cleans_workspace() {
    let mut fixture = Fixture::new().await;
    let parent_path = fixture.library.library_root().join("vision");
    fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
        if boundary == ImportBoundary::BeforePublish {
            assert!(stage.read_model_metadata()?.is_some());
            std::fs::write(&parent_path, b"parent sentinel")?;
        }
        Ok(())
    }));
    assert!(fixture.importer.import(&fixture.spec).await.is_err());
    assert!(fixture.stages().is_empty());
    assert_eq!(
        std::fs::read(fixture.library.library_root().join("vision")).unwrap(),
        b"parent sentinel"
    );
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_rename_collision_keeps_existing_output_and_cleans_stage() {
    let mut fixture = Fixture::new().await;
    let target = fixture.target();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
        if boundary == ImportBoundary::BeforePublish {
            std::fs::create_dir_all(&target)?;
            std::fs::write(target.join("sentinel"), b"existing model")?;
        }
        Ok(())
    }));
    assert!(
        !fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    assert_eq!(
        std::fs::read(fixture.target().join("sentinel")).unwrap(),
        b"existing model"
    );
    assert!(fixture.stages().is_empty());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_replaced_stage_retains_original_and_replacement_sentinel() {
    for symlink in [false, true] {
        let mut fixture = Fixture::new().await;
        let retained = fixture.temp.path().join("renamed-original");
        let retained_for_hook = retained.clone();
        let replacement = fixture.temp.path().join("replacement");
        std::fs::create_dir(&replacement).unwrap();
        std::fs::write(replacement.join("sentinel"), b"replacement sentinel").unwrap();
        let replacement_for_hook = replacement.clone();
        fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
            if boundary == ImportBoundary::BeforeMetadata {
                std::fs::rename(stage.display_path(), &retained_for_hook)?;
                if symlink {
                    std::os::unix::fs::symlink(&replacement_for_hook, stage.display_path())?;
                } else {
                    std::fs::create_dir(stage.display_path())?;
                    std::fs::write(
                        stage.display_path().join("sentinel"),
                        b"replacement sentinel",
                    )?;
                }
                return Err(fault("original preparation failure"));
            }
            Ok(())
        }));
        let error = fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("original preparation failure"));
        assert!(error.contains("cleanup failed"));
        assert!(error.contains("no automatic cleanup retry"));
        assert!(retained.join("model.onnx").is_file());
        assert_eq!(
            std::fs::read(fixture.stages()[0].join("sentinel")).unwrap(),
            b"replacement sentinel"
        );
        assert!(!fixture.target().exists());
        assert!(fixture.tasks.shutdown_owned().await.is_err());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_root_replacement_refuses_publication_and_cleanup() {
    let mut fixture = Fixture::new().await;
    let old = fixture.temp.path().join("old-library");
    let root = fixture.library.library_root().to_path_buf();
    let old_for_hook = old.clone();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
        if boundary == ImportBoundary::BeforePublish {
            std::fs::rename(&root, &old_for_hook)?;
            std::fs::create_dir(&root)?;
            std::fs::write(root.join("sentinel"), b"new root")?;
        }
        Ok(())
    }));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("cleanup failed"));
    assert_eq!(
        std::fs::read(fixture.library.library_root().join("sentinel")).unwrap(),
        b"new root"
    );
    assert!(std::fs::read_dir(old).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(TEMP_IMPORT_PREFIX)));
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_post_publication_index_failure_preserves_model_identity() {
    let fixture = Fixture::new().await;
    let db = rusqlite::Connection::open(fixture.library.index().db_path()).unwrap();
    db.execute_batch("CREATE TRIGGER refuse_import_index BEFORE INSERT ON models BEGIN SELECT RAISE(ABORT, 'injected import index failure'); END;").unwrap();
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("was published"));
    assert!(error.contains("injected import index failure"));
    assert!(fixture.target().join("model.onnx").is_file());
    assert!(fixture.target().join("metadata.json").is_file());
    assert!(fixture.stages().is_empty());
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_full_progress_channel_cannot_block_settlement() {
    let fixture = Fixture::new().await;
    let (tx, _unread_rx) = mpsc::channel(1);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        fixture.importer.import_with_progress(&fixture.spec, tx),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.success);
    assert!(fixture.stages().is_empty());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_dropped_waiter_and_shutdown_wait_for_held_producer() {
    for boundary in [
        ImportBoundary::FileCopied,
        ImportBoundary::BeforeHash,
        ImportBoundary::DescendantsReleased,
        ImportBoundary::Published,
        ImportBoundary::BeforeConfirm,
        ImportBoundary::BeforeReady,
    ] {
        let mut fixture = Fixture::new().await;
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let entered = std::sync::Mutex::new(Some(entered_tx));
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release = std::sync::Mutex::new(release_rx);
        fixture.importer.import_hook = Some(Arc::new(move |current, _| {
            if current == boundary {
                if let Some(tx) = entered.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                release.lock().unwrap().recv().unwrap();
            }
            Ok(())
        }));
        let importer = fixture.importer.clone();
        let spec = fixture.spec.clone();
        let waiter = tokio::spawn(async move { importer.import(&spec).await });
        tokio::time::timeout(std::time::Duration::from_secs(10), entered_rx)
            .await
            .unwrap()
            .unwrap();
        waiter.abort();
        let _ = waiter.await;
        fixture.tasks.close();
        assert!(fixture.importer.import(&fixture.spec).await.is_err());
        let tasks = fixture.tasks.clone();
        let mut shutdown = Box::pin(tasks.shutdown_owned());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), &mut shutdown)
                .await
                .is_err()
        );
        if matches!(
            boundary,
            ImportBoundary::Published | ImportBoundary::BeforeConfirm | ImportBoundary::BeforeReady
        ) {
            assert!(fixture.target().is_dir());
            let mut attempted_edit = fixture
                .library
                .load_metadata(&fixture.target())
                .unwrap()
                .unwrap();
            attempted_edit.notes = Some("must not race finalization".into());
            assert!(fixture
                .library
                .save_metadata(&fixture.target(), &attempted_edit)
                .await
                .is_err());
        } else {
            assert_eq!(fixture.stages().len(), 1);
        }
        release_tx.send(()).unwrap();
        shutdown.await.unwrap();
        assert!(fixture.target().join("model.onnx").is_file());
        assert!(fixture.stages().is_empty());
    }
}

#[tokio::test]
async fn copied_import_metadata_is_hidden_from_discovery_until_publication() {
    let mut fixture = Fixture::new().await;
    let library = fixture.library.clone();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
        if boundary == ImportBoundary::BeforePublish {
            assert!(stage.read_model_metadata()?.is_some());
            assert!(library.model_dirs().next().is_none());
        }
        Ok(())
    }));
    assert!(
        fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    assert_eq!(fixture.library.model_dirs().count(), 1);
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_diffusers_prepublication_failure_cleans_and_postpublication_retains() {
    for failure in [
        Some(ImportBoundary::BeforeMetadata),
        Some(ImportBoundary::Published),
        None,
    ] {
        let mut fixture = Fixture::new().await;
        let bundle = crate::model_library::importer::tests::create_external_diffusers_bundle(
            fixture.temp.path(),
        );
        let index_path = bundle.join("model_index.json");
        let mut index: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();
        index["_name_or_path"] = serde_json::json!("stabilityai/sd-turbo");
        index["_diffusers_version"] = serde_json::json!("0.32.0");
        std::fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
        fixture.spec.path = bundle.display().to_string();
        let target = fixture
            .library
            .build_model_path("diffusion", "fixture", "Owned Import");
        fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
            if Some(boundary) == failure {
                Err(fault("injected bundle failure"))
            } else {
                Ok(())
            }
        }));
        let (tx, _rx) = mpsc::channel(1);
        let result = fixture
            .importer
            .import_with_progress(&fixture.spec, tx)
            .await;
        match failure {
            Some(ImportBoundary::BeforeMetadata) => {
                assert!(result.is_err());
                assert!(!target.exists());
                assert!(fixture.tasks.shutdown_owned().await.is_err());
            }
            Some(ImportBoundary::Published) => {
                assert!(result.unwrap_err().to_string().contains("was published"));
                assert!(target.join("metadata.json").is_file());
                assert!(target
                    .join("unet/diffusion_pytorch_model.safetensors")
                    .is_file());
                assert!(fixture.tasks.shutdown_owned().await.is_err());
            }
            None => {
                let result = result.unwrap();
                assert!(result.success);
                let metadata = fixture.library.load_metadata(&target).unwrap().unwrap();
                assert_eq!(metadata.model_id, result.model_id);
                assert_eq!(
                    metadata.entry_path.as_deref(),
                    Some(target.to_str().unwrap())
                );
                assert!(metadata
                    .expected_files
                    .as_ref()
                    .unwrap()
                    .contains(&"unet/diffusion_pytorch_model.safetensors".into()));
                assert!(metadata
                    .dependency_bindings
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|binding| binding.profile_id.as_deref()
                        == Some("sd-turbo-diffusers-runtime")));
                assert!(fixture
                    .library
                    .index()
                    .get(result.model_id.as_deref().unwrap())
                    .unwrap()
                    .is_some());
                fixture.tasks.shutdown_owned().await.unwrap();
            }
            _ => unreachable!(),
        }
        assert!(fixture.stages().is_empty());
        assert!(bundle.join("model_index.json").is_file());
    }
}

#[tokio::test]
async fn copied_import_payload_and_metadata_mutations_retain_unknown_workspace() {
    for metadata_failure in [false, true] {
        let mut fixture = Fixture::new().await;
        fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
            if !metadata_failure && boundary == ImportBoundary::BeforeHash {
                std::fs::remove_file(stage.display_path().join("model.onnx"))?;
            }
            if metadata_failure && boundary == ImportBoundary::BeforeMetadata {
                std::fs::remove_file(stage.display_path().join("metadata.json"))?;
                std::fs::create_dir(stage.display_path().join("metadata.json"))?;
            }
            Ok(())
        }));
        let error = fixture.importer.import(&fixture.spec).await.unwrap_err();
        assert!(matches!(&error, PumasError::ImportFailed { .. }), "{error}");
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("custody unknown"), "{diagnostic}");
        assert!(diagnostic.contains("cleanup failed"), "{diagnostic}");
        let stages = fixture.stages();
        assert_eq!(stages.len(), 1);
        let retained = &stages[0];
        assert!(
            diagnostic.contains(retained.to_str().unwrap()),
            "{diagnostic}"
        );
        if metadata_failure {
            // A new directory at a metadata-file path is unknown custody.
            assert!(retained.join("metadata.json").is_dir());
            assert_eq!(
                std::fs::read(retained.join("model.onnx")).unwrap(),
                b"synthetic ONNX payload"
            );
        } else {
            // Hashes already came from the copy stream. Removing the copied
            // file now invalidates namespace evidence before receipt creation;
            // the producer cannot claim cleanup authority over that mutation.
            assert!(!retained.join("model.onnx").exists());
            assert_eq!(std::fs::read(retained.join("metadata.json")).unwrap(), b"");
            assert_eq!(std::fs::read(retained.join(RECEIPT_FILENAME)).unwrap(), b"");
        }
        assert!(!fixture.target().exists());
        assert_eq!(fixture.library.model_count().unwrap(), 0);
        assert_eq!(fixture.library.model_dirs().count(), 0);
        assert_eq!(
            std::fs::read(Path::new(&fixture.spec.path).join("Model.onnx")).unwrap(),
            b"synthetic ONNX payload"
        );
        assert!(fixture.tasks.shutdown_owned().await.is_err());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_target_ancestor_replacement_keeps_sentinel() {
    let mut fixture = Fixture::new().await;
    let family = fixture.target().parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&family).unwrap();
    let old = fixture.temp.path().join("original-family");
    fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
        if boundary == ImportBoundary::BeforePublish {
            std::fs::rename(&family, &old)?;
            std::fs::create_dir(&family)?;
            std::fs::write(family.join("sentinel"), b"replacement family")?;
        }
        Ok(())
    }));
    assert!(fixture.importer.import(&fixture.spec).await.is_err());
    assert_eq!(
        std::fs::read(fixture.target().parent().unwrap().join("sentinel")).unwrap(),
        b"replacement family"
    );
    assert!(fixture.stages().is_empty());
    assert!(!fixture.target().exists());
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_source_symlink_is_refused_without_writes() {
    let fixture = Fixture::new().await;
    let source = Path::new(&fixture.spec.path);
    std::os::unix::fs::symlink("Model.onnx", source.join("Alias.onnx")).unwrap();
    assert!(
        !fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    assert!(fixture.stages().is_empty());
    assert_eq!(
        std::fs::read(source.join("Model.onnx")).unwrap(),
        b"synthetic ONNX payload"
    );
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_dropped_before_admission_has_no_effects() {
    let fixture = Fixture::new().await;
    drop(fixture.importer.import(&fixture.spec));
    assert!(fixture.stages().is_empty());
    assert!(!fixture.target().exists());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_preparation_panic_settles_workspace_and_is_owner_visible() {
    let mut fixture = Fixture::new().await;
    fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
        assert!(
            boundary != ImportBoundary::FileCopied,
            "injected copy panic"
        );
        Ok(())
    }));
    assert!(fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string()
        .contains("injected copy panic"));
    assert!(fixture.stages().is_empty());
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_actual_copy_failure_after_partial_copy_cleans_stage() {
    let mut fixture = Fixture::new().await;
    let second = Path::new(&fixture.spec.path).join("Z.onnx");
    std::fs::write(&second, b"second source").unwrap();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
        if boundary == ImportBoundary::FileCopied {
            std::fs::remove_file(&second)?;
        }
        Ok(())
    }));
    assert!(fixture.importer.import(&fixture.spec).await.is_err());
    assert!(fixture.stages().is_empty());
    assert!(!fixture.target().exists());
    assert_eq!(
        std::fs::read(Path::new(&fixture.spec.path).join("Model.onnx")).unwrap(),
        b"synthetic ONNX payload"
    );
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_native_case_and_unicode_alias_oracle() {
    for (first, second) in [("Alias.onnx", "alias.onnx"), ("é.onnx", "e\u{301}.onnx")] {
        let fixture = Fixture::new().await;
        let root = DownloadDestinationRoot::open(fixture.library.library_root()).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let probe = root.create_import_stage().unwrap();
        drop(probe.create_import_file(first).unwrap());
        let equivalent = probe.open_import_file(second).is_ok();
        probe.remove_model_directory_all().unwrap();
        eprintln!("native import filename equivalence {first:?} / {second:?}: {equivalent}");
        let source = Path::new(&fixture.spec.path);
        std::fs::write(source.join("a"), b"first").unwrap();
        std::fs::write(source.join("b"), b"second").unwrap();
        // Separate source names make the destination's native equivalence
        // independently observable even on a case-insensitive source volume.
        let plan = CopyPlan {
            directories: Vec::new(),
            source: crate::platform::capability_fs::open_directory(source).unwrap(),
            files: vec![
                (PathBuf::from("a"), "a".into(), first.into()),
                (PathBuf::from("b"), "b".into(), second.into()),
            ],
        };
        let stage = root.create_import_stage().unwrap();
        let result = plan.copy_to(&stage, &fixture.importer);
        if equivalent {
            assert!(matches!(result, Err(PumasError::Validation { .. })));
            assert_eq!(
                std::fs::read(stage.display_path().join(first)).unwrap(),
                b"first"
            );
        } else {
            let (files, evidence) = result.unwrap();
            assert_eq!(files.len(), 2);
            assert_eq!(evidence.len(), 2);
            for (file, name) in files.iter().zip([first, second]) {
                assert_eq!(file.name, name);
                let copied = stage.open_import_file(name).unwrap();
                assert_eq!(
                    evidence.get(name).unwrap(),
                    &ImportFileIdentity::copied(
                        &copied,
                        file.size.unwrap(),
                        file.sha256.clone().unwrap(),
                    )
                    .unwrap()
                );
            }
        }
        stage.remove_model_directory_all().unwrap();
        assert_eq!(std::fs::read(source.join("a")).unwrap(), b"first");
        assert_eq!(std::fs::read(source.join("b")).unwrap(), b"second");
        assert!(fixture.stages().is_empty());
        fixture.tasks.shutdown_owned().await.unwrap();
    }
}

#[cfg(windows)]
#[tokio::test]
async fn copied_import_windows_readonly_payload_keeps_source_and_published_attribute() {
    let fixture = Fixture::new().await;
    let source = Path::new(&fixture.spec.path).join("Model.onnx");
    let mut permissions = std::fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source, permissions).unwrap();
    assert!(
        fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    assert!(std::fs::metadata(&source).unwrap().permissions().readonly());
    assert!(std::fs::metadata(fixture.target().join("model.onnx"))
        .unwrap()
        .permissions()
        .readonly());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn copied_import_windows_locked_readonly_cleanup_is_retained_and_reported() {
    use std::os::windows::fs::OpenOptionsExt;
    use std::sync::Mutex;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    let mut fixture = Fixture::new().await;
    let source = Path::new(&fixture.spec.path).join("Model.onnx");
    let original_bytes = std::fs::read(&source).unwrap();
    let mut permissions = std::fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source, permissions).unwrap();
    let held = Arc::new(Mutex::new(None::<std::fs::File>));
    let hook_held = held.clone();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
        if boundary == ImportBoundary::BeforeHash {
            // A real native handle denies deletion independently of whether
            // this Windows filesystem permits deleting a read-only file.
            *hook_held.lock().unwrap() = Some(
                std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                    .open(stage.display_path().join("model.onnx"))?,
            );
            return Err(fault("injected failure after read-only copy"));
        }
        Ok(())
    }));
    let error = fixture.importer.import(&fixture.spec).await.unwrap_err();
    let stages = fixture.stages();
    assert_eq!(stages.len(), 1);
    let output = stages[0].join("model.onnx");
    let source_preserved = std::fs::read(&source).unwrap() == original_bytes
        && std::fs::metadata(&source).unwrap().permissions().readonly();
    let output_retained = std::fs::read(&output).unwrap() == original_bytes
        && std::fs::metadata(&output).unwrap().permissions().readonly();
    let shutdown = fixture
        .tasks
        .shutdown_owned()
        .await
        .unwrap_err()
        .to_string();
    let unpublished = !fixture.target().exists();

    // Settle only this synthetic fixture after observing the production result.
    // Production deliberately did not clear source/output attributes or retry.
    drop(held.lock().unwrap().take());
    for path in [&source, &output] {
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions).unwrap();
    }
    std::fs::remove_dir_all(&stages[0]).unwrap();

    assert!(source_preserved && output_retained && unpublished);
    assert!(error
        .to_string()
        .contains("injected failure after read-only copy"));
    assert!(error.to_string().contains("cleanup failed"));
    assert!(shutdown.contains("injected failure after read-only copy"));
    assert!(shutdown.contains("cleanup failed"));
}

#[tokio::test]
async fn copied_import_diffusers_preserves_empty_referenced_directories() {
    for progress in [false, true] {
        let mut fixture = Fixture::new().await;
        let bundle = crate::model_library::importer::tests::create_external_diffusers_bundle(
            fixture.temp.path(),
        );
        std::fs::remove_file(bundle.join("tokenizer/tokenizer.json")).unwrap();
        std::fs::create_dir(bundle.join("scheduler")).unwrap();
        let index_path = bundle.join("model_index.json");
        let mut index: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();
        index["scheduler"] = serde_json::json!(["diffusers", "Scheduler"]);
        std::fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
        assert_eq!(
            validate_diffusers_directory_for_import(&bundle).validation_state,
            crate::models::AssetValidationState::Valid
        );
        fixture.spec.path = bundle.display().to_string();
        let result = if progress {
            let (tx, _rx) = mpsc::channel(1);
            fixture
                .importer
                .import_with_progress(&fixture.spec, tx)
                .await
        } else {
            fixture.importer.import(&fixture.spec).await
        }
        .unwrap();
        assert!(result.success);
        let target = fixture
            .library
            .library_root()
            .join(result.model_id.unwrap());
        for component in ["tokenizer", "scheduler"] {
            assert!(target.join(component).is_dir());
            assert_eq!(
                std::fs::read_dir(target.join(component)).unwrap().count(),
                0
            );
        }
        // Reopening through the normal reader independently observes the layout.
        assert_eq!(
            validate_diffusers_directory_for_import(&target).validation_state,
            crate::models::AssetValidationState::Valid
        );
        let metadata = fixture.library.load_metadata(&target).unwrap().unwrap();
        assert_eq!(
            metadata.validation_state,
            Some(crate::models::AssetValidationState::Valid)
        );
        assert_eq!(
            metadata.import_state,
            Some(crate::models::ImportState::Ready)
        );
        assert!(fixture.stages().is_empty());
        fixture.tasks.shutdown_owned().await.unwrap();
    }
}

#[tokio::test]
async fn copied_import_diffusers_validates_staged_index_instead_of_source_result() {
    let mut fixture = Fixture::new().await;
    let bundle = crate::model_library::importer::tests::create_external_diffusers_bundle(
        fixture.temp.path(),
    );
    let source_index = std::fs::read(bundle.join("model_index.json")).unwrap();
    fixture.spec.path = bundle.display().to_string();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
        if boundary == ImportBoundary::BeforeHash {
            let mut index: serde_json::Value =
                serde_json::from_reader(stage.open_import_file("model_index.json")?)?;
            index["missing_scheduler"] = serde_json::json!(["diffusers", "Scheduler"]);
            std::fs::write(
                stage.display_path().join("model_index.json"),
                serde_json::to_vec(&index)?,
            )?;
        }
        Ok(())
    }));
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(!result.success);
    assert!(result.error.unwrap().contains("missing_scheduler"));
    assert!(!fixture
        .library
        .build_model_path("diffusion", "fixture", "Owned Import")
        .exists());
    assert!(fixture.stages().is_empty());
    assert_eq!(
        std::fs::read(bundle.join("model_index.json")).unwrap(),
        source_index
    );
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_nested_replacement_retains_original_and_replacement_trees() {
    let mut fixture = Fixture::new().await;
    let bundle = crate::model_library::importer::tests::create_external_diffusers_bundle(
        fixture.temp.path(),
    );
    fixture.spec.path = bundle.display().to_string();
    let retained = fixture.temp.path().join("original-unet");
    let retained_for_hook = retained.clone();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
        if boundary == ImportBoundary::BeforeHash {
            std::fs::rename(stage.display_path().join("unet"), &retained_for_hook)?;
            std::fs::create_dir(stage.display_path().join("unet"))?;
            std::fs::write(
                stage.display_path().join("unet/sentinel"),
                b"replacement tree",
            )?;
        }
        Ok(())
    }));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("cleanup failed"));
    assert!(retained
        .join("diffusion_pytorch_model.safetensors")
        .is_file());
    assert_eq!(
        std::fs::read(fixture.stages()[0].join("unet/sentinel")).unwrap(),
        b"replacement tree"
    );
    // Validation precedes all deletion, so unrelated original siblings survive too.
    assert!(fixture.stages()[0]
        .join("vae/diffusion_pytorch_model.safetensors")
        .is_file());
    assert!(bundle
        .join("unet/diffusion_pytorch_model.safetensors")
        .is_file());
    assert!(!fixture
        .library
        .build_model_path("diffusion", "fixture", "Owned Import")
        .exists());
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_unknown_nested_directory_is_retained_before_any_deletion() {
    let mut fixture = Fixture::new().await;
    fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
        if boundary == ImportBoundary::BeforeHash {
            std::fs::create_dir(stage.display_path().join("unknown"))?;
            std::fs::write(
                stage.display_path().join("unknown/sentinel"),
                b"unknown custody",
            )?;
            return Err(fault("original operation failure"));
        }
        Ok(())
    }));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("original operation failure"));
    assert!(error.contains("cleanup failed"));
    assert_eq!(
        std::fs::read(fixture.stages()[0].join("unknown/sentinel")).unwrap(),
        b"unknown custody"
    );
    assert!(fixture.stages()[0].join("model.onnx").is_file());
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_metadata_notifier_observes_write_paths_without_authorizing_effects() {
    let fixture = Fixture::new().await;
    let notifications = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = notifications.clone();
    fixture
        .library
        .set_metadata_write_notifier(Some(Arc::new(move |path| {
            seen.lock().unwrap().push(path);
        })));
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(result.success);
    let paths = notifications.lock().unwrap().clone();
    assert_eq!(
        paths.len(),
        2,
        "Pending and Ready metadata writes both notify"
    );
    assert_eq!(paths[0].file_name().unwrap(), "metadata.json");
    assert!(paths[0]
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with(TEMP_IMPORT_PREFIX));
    assert!(
        !paths[0].exists(),
        "notification is the former stage path, not retained filesystem authority"
    );
    assert_eq!(paths[1], fixture.target().join("metadata.json"));
    assert!(fixture.target().join("metadata.json").is_file());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_notifier_cannot_redirect_held_metadata_write_to_replacement() {
    let fixture = Fixture::new().await;
    let original = fixture.temp.path().join("notifier-original");
    let original_for_callback = original.clone();
    fixture
        .library
        .set_metadata_write_notifier(Some(Arc::new(move |metadata_path| {
            let stage = metadata_path.parent().unwrap();
            std::fs::rename(stage, &original_for_callback).unwrap();
            std::fs::create_dir(stage).unwrap();
            std::fs::write(stage.join("sentinel"), b"replacement after notification").unwrap();
        })));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("cleanup failed"));
    assert!(original.join("model.onnx").is_file());
    assert_eq!(
        std::fs::read(fixture.stages()[0].join("sentinel")).unwrap(),
        b"replacement after notification"
    );
    assert!(!fixture.stages()[0].join("metadata.json").exists());
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_notifier_empty_component_changes_refuse_publication_and_retain_custody() {
    for replace in [false, true] {
        let mut fixture = Fixture::new().await;
        let bundle = crate::model_library::importer::tests::create_external_diffusers_bundle(
            fixture.temp.path(),
        );
        std::fs::remove_file(bundle.join("tokenizer/tokenizer.json")).unwrap();
        fixture.spec.path = bundle.display().to_string();
        let original = fixture.temp.path().join("original-empty-tokenizer");
        let original_for_callback = original.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_callback = calls.clone();
        fixture
            .library
            .set_metadata_write_notifier(Some(Arc::new(move |metadata_path| {
                calls_for_callback.fetch_add(1, Ordering::SeqCst);
                let component = metadata_path.parent().unwrap().join("tokenizer");
                if replace {
                    std::fs::rename(&component, &original_for_callback).unwrap();
                    std::fs::create_dir(&component).unwrap();
                    std::fs::write(component.join("sentinel"), b"replacement empty component")
                        .unwrap();
                } else {
                    std::fs::remove_dir(component).unwrap();
                }
            })));
        let error = fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(error.contains("cleanup failed"));
        assert!(error.contains("retained or custody unknown"));
        assert!(!fixture
            .library
            .build_model_path("diffusion", "fixture", "Owned Import")
            .exists());
        assert_eq!(fixture.stages().len(), 1);
        assert!(fixture.stages()[0]
            .join("unet/diffusion_pytorch_model.safetensors")
            .is_file());
        assert!(fixture.library.model_dirs().next().is_none());
        if replace {
            assert!(original.is_dir());
            assert_eq!(std::fs::read_dir(&original).unwrap().count(), 0);
            assert_eq!(
                std::fs::read(fixture.stages()[0].join("tokenizer/sentinel")).unwrap(),
                b"replacement empty component"
            );
        } else {
            assert!(!fixture.stages()[0].join("tokenizer").exists());
        }
        assert!(bundle.join("tokenizer").is_dir());
        assert_eq!(
            std::fs::read_dir(bundle.join("tokenizer")).unwrap().count(),
            0
        );
        assert!(fixture.tasks.shutdown_owned().await.is_err());
    }
}

#[tokio::test]
async fn copied_import_notifier_valid_same_size_index_rewrite_is_refused_and_retained() {
    let mut fixture = Fixture::new().await;
    let bundle = crate::model_library::importer::tests::create_external_diffusers_bundle(
        fixture.temp.path(),
    );
    let index_path = bundle.join("model_index.json");
    let mut index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();
    index["_name_or_path"] = serde_json::json!("stabilityai/sd-turbo");
    index["_diffusers_version"] = serde_json::json!("0.32.0");
    let source_index = serde_json::to_vec(&index).unwrap();
    std::fs::write(&index_path, &source_index).unwrap();
    fixture.spec.path = bundle.display().to_string();
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_callback = calls.clone();
    fixture
        .library
        .set_metadata_write_notifier(Some(Arc::new(move |metadata_path| {
            calls_for_callback.fetch_add(1, Ordering::SeqCst);
            let index_path = metadata_path.parent().unwrap().join("model_index.json");
            let before = std::fs::read(&index_path).unwrap();
            let mut index: serde_json::Value = serde_json::from_slice(&before).unwrap();
            index["_diffusers_version"] = serde_json::json!("0.33.0");
            let after = serde_json::to_vec(&index).unwrap();
            assert_eq!(
                before.len(),
                after.len(),
                "file-size proof alone must not qualify this rewrite"
            );
            std::fs::write(index_path, after).unwrap();
            assert_eq!(
                validate_diffusers_directory_for_import(metadata_path.parent().unwrap())
                    .validation_state,
                crate::models::AssetValidationState::Valid,
                "a still-valid index can disagree with prepared runtime metadata"
            );
        })));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("model_index.json changed after metadata preparation"));
    assert!(error.contains("cleanup failed"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.stages().len(), 1);
    assert!(!fixture
        .library
        .build_model_path("diffusion", "fixture", "Owned Import")
        .exists());
    assert_eq!(std::fs::read(index_path).unwrap(), source_index);
    assert_eq!(fixture.library.model_dirs().count(), 0);
    assert_eq!(fixture.library.model_count().unwrap(), 0);
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}
mod publication;

#[tokio::test]
async fn copied_import_batch_delivers_terminal_receipt_after_owner_settles() {
    let mut fixture = Fixture::new().await;
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let entered = std::sync::Mutex::new(Some(entered_tx));
    fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
        if boundary == ImportBoundary::StageCreated {
            if let Some(tx) = entered.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
        Ok(())
    }));
    let (tx, mut rx) = mpsc::channel(1);
    tx.send(BatchImportProgress::new(99)).await.unwrap();
    let importer = fixture.importer.clone();
    let spec = fixture.spec.clone();
    let batch = tokio::spawn(async move { importer.batch_import(vec![spec], Some(tx)).await });
    tokio::time::timeout(std::time::Duration::from_secs(10), entered_rx)
        .await
        .unwrap()
        .unwrap();
    // A full progress channel must not prevent actual producer settlement.
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        fixture.tasks.shutdown_owned(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(fixture.target().join("model.onnx").is_file());
    assert_eq!(rx.recv().await.unwrap().total, 99);
    let complete = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
        .await
        .unwrap()
        .expect("terminal batch receipt must not be dropped");
    assert_eq!(complete.stage, ImportStage::Complete);
    assert_eq!(complete.results.len(), 1);
    assert!(complete.results[0].success);
    assert_eq!(batch.await.unwrap().len(), 1);
}

#[tokio::test]
async fn copied_import_batch_returns_results_when_progress_observer_is_closed() {
    let fixture = Fixture::new().await;
    let (tx, rx) = mpsc::channel(1);
    drop(rx);
    let results = fixture
        .importer
        .batch_import(vec![fixture.spec.clone()], Some(tx))
        .await;
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[cfg(unix)]
#[test]
fn copied_import_final_directory_respects_ordinary_umask() {
    for (mask, mode) in [
        ("000", "777"),
        ("002", "775"),
        ("022", "755"),
        ("077", "700"),
    ] {
        let output = std::process::Command::new("sh")
            .args(["-c", &format!("umask {mask}; exec \"$@\""), "sh"])
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "model_library::importer::staging::tests::copied_import_final_mode_child",
                "--ignored",
                "--nocapture",
            ])
            .env("PUMAS_IMPORT_FINAL_MODE_EXPECTED", mode)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "umask {mask}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "invoked in isolated subprocesses with specific umasks"]
async fn copied_import_final_mode_child() {
    use std::os::unix::fs::PermissionsExt;
    let Ok(expected) = std::env::var("PUMAS_IMPORT_FINAL_MODE_EXPECTED") else {
        return;
    };
    let expected = u32::from_str_radix(&expected, 8).unwrap();
    let mut fixture = Fixture::new().await;
    fixture.importer.import_hook = Some(Arc::new(move |boundary, destination| {
        if matches!(
            boundary,
            ImportBoundary::StageCreated | ImportBoundary::BeforeReady
        ) {
            assert_eq!(
                std::fs::metadata(destination.display_path())?
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        Ok(())
    }));
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(result.success);
    assert_eq!(
        std::fs::metadata(fixture.target())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        expected
    );
    assert!(fixture.stages().is_empty());
    assert!(fixture
        .library
        .load_metadata(&fixture.target())
        .unwrap()
        .unwrap()
        .copied_import_ready());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_permission_finalization_failure_retains_pending_payload() {
    use std::os::unix::fs::PermissionsExt;
    let mut fixture = Fixture::new().await;
    fixture.importer.import_hook = Some(Arc::new(move |boundary, destination| {
        if boundary == ImportBoundary::BeforeReady {
            destination.inject_import_permission_failure();
        }
        Ok(())
    }));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("injected import directory permission failure"));
    assert!(fixture.target().join("model.onnx").is_file());
    assert_eq!(
        std::fs::metadata(fixture.target())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(receipt_at(&fixture.target())["state"], "pending");
    let metadata = fixture
        .library
        .load_metadata(&fixture.target())
        .unwrap()
        .unwrap();
    assert!(!metadata.copied_import_ready());
    let id = fixture.library.get_model_id(&fixture.target()).unwrap();
    assert!(!crate::models::copied_import_ready_value(
        &fixture.library.index().get(&id).unwrap().unwrap().metadata
    ));
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}
