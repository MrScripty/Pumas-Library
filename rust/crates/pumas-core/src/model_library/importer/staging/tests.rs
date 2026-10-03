use super::*;
use crate::api::RuntimeTasks;
use crate::model_library::{DownloadDestinationRoot, DownloadPersistence};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

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
    assert!(fixture.library.index().get(&id).unwrap().is_some());
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
    let fixture = Fixture::new().await;
    std::fs::write(
        fixture.library.library_root().join("vision"),
        b"parent sentinel",
    )
    .unwrap();
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
        ImportBoundary::Published,
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
        entered_rx.await.unwrap();
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
        if boundary == ImportBoundary::Published {
            assert!(fixture.target().is_dir());
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
async fn copied_import_actual_hash_and_metadata_failures_cleanup_workspace() {
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
        assert!(fixture.importer.import(&fixture.spec).await.is_err());
        assert!(fixture.stages().is_empty());
        assert!(!fixture.target().exists());
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
            assert_eq!(result.unwrap().len(), 2);
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
