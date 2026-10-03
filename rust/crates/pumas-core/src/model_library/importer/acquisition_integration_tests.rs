//! Cross-owner regressions use real copied publication and managed-HF import
//! producers. Synthetic payloads and independent temporary stores need no model
//! download, live library or supported inference runtime.

use super::staging::ImportBoundary;
use super::tests::{completed_download, custody_fixture, orphan_spec, real_admission};
use super::*;
use crate::acquisition::task_custody::TaskRole;
use crate::acquisition::*;
use crate::model_library::download_store::{
    DownloadAdmissionDomain, DownloadPersistence, HfCompletionReceipt,
};
use crate::model_library::mutation_authority::DownloadCancellation;
use crate::model_library::DownloadDestinationRoot;
use std::sync::Mutex;

fn copy_spec(temp: &Path, name: &str) -> ModelImportSpec {
    let source = temp.join(format!("source-{name}"));
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("detector.onnx"), b"data").unwrap();
    ModelImportSpec {
        path: source.display().to_string(),
        family: "publisher".into(),
        official_name: name.into(),
        repo_id: None,
        model_type: Some("vision".into()),
        subtype: None,
        tags: None,
        security_acknowledged: Some(true),
    }
}

async fn guarded_observation(
    library: Arc<ModelLibrary>,
    model: PathBuf,
    inspect: impl FnOnce(&ModelLibrary, &Path, &crate::model_library::mutation_authority::LibraryImportGuard)
        + Send
        + 'static,
) {
    let authority = library.mutation_authority().unwrap();
    authority
        .tasks()
        .run_owned(
            "observe composed copied publication",
            move |context| async move {
                let guard_context = context.clone();
                context
                    .run_blocking("held copied publication observation", move || {
                        let guard = authority.protect_import(&model, guard_context)?;
                        let scoped = library.with_import_guard(guard.clone());
                        inspect(&scoped, &model, &guard);
                        Ok::<_, PumasError>(())
                    })
                    .await?
            },
        )
        .await
        .unwrap();
}

fn observed_ready(library: &ModelLibrary, model: &Path) -> bool {
    library
        .load_metadata(model)
        .ok()
        .flatten()
        .is_some_and(|metadata| metadata.copied_import_ready())
}

#[tokio::test]
async fn acquisition_integration_copied_readiness_survives_guarded_observation() {
    let (temp, library, _downloads, tasks, _root) = custody_fixture().await;
    let spec = copy_spec(temp.path(), "copied-ready");
    let result = ModelImporter::new(library.clone())
        .import(&spec)
        .await
        .unwrap();
    assert!(result.success);
    let id = result.model_id.unwrap();
    let model = library.library_root().join(&id);
    let receipt_path = model.join(publication::RECEIPT_FILENAME);
    let canonical = std::fs::read(model.join("metadata.json")).unwrap();
    let receipt = std::fs::read(&receipt_path).unwrap();
    let identity = library
        .load_metadata(&model)
        .unwrap()
        .unwrap()
        .import_publication;
    assert!(observed_ready(&library, &model));
    prime_summary(&library, &id, &model);
    assert!(cached_target_ready(&library, &id, &model));
    guarded_observation(library.clone(), model.clone(), |scoped, path, guard| {
        assert!(
            observed_ready(scoped, path),
            "genuine Confirmed/Ready positive control"
        );
        let destination = guard.held_destination(path).unwrap();
        let original = destination.read_model_metadata().unwrap().unwrap();
        let bytes = std::fs::read(path.join("metadata.json")).unwrap();
        let mut erased = original.clone();
        erased.import_publication = None;
        assert!(destination.write_model_metadata(&erased).is_err());
        let mut raw = serde_json::to_value(&original).unwrap();
        raw.as_object_mut().unwrap().remove("import_publication");
        assert!(destination.write_model_metadata_value(&raw).is_err());
        let mut oversized = serde_json::to_value(&original).unwrap();
        oversized["notes"] = "x"
            .repeat(crate::model_library::download_recovery::IMPORT_DOCUMENT_MAX_BYTES as usize)
            .into();
        assert!(destination.write_model_metadata_value(&oversized).is_err());
        assert_eq!(std::fs::read(path.join("metadata.json")).unwrap(), bytes);
    })
    .await;

    for fault in [
        "pending",
        "missing",
        "mismatched",
        "unreadable",
        "erased",
        "oversized",
    ] {
        let canonical = canonical.clone();
        let receipt = receipt.clone();
        guarded_observation(
            library.clone(),
            model.clone(),
            move |scoped, path, _guard| {
                let receipt_path = path.join(publication::RECEIPT_FILENAME);
                match fault {
                    "pending" | "mismatched" => {
                        let mut value: serde_json::Value =
                            serde_json::from_slice(&receipt).unwrap();
                        if fault == "pending" {
                            value["state"] = "pending".into();
                        } else {
                            value["id"] = uuid::Uuid::new_v4().to_string().into();
                        }
                        std::fs::write(&receipt_path, serde_json::to_vec(&value).unwrap()).unwrap();
                    }
                    "missing" => std::fs::remove_file(&receipt_path).unwrap(),
                    "unreadable" => std::fs::write(&receipt_path, b"{not-json").unwrap(),
                    "erased" => {
                        let mut value: serde_json::Value =
                            serde_json::from_slice(&canonical).unwrap();
                        value.as_object_mut().unwrap().remove("import_publication");
                        std::fs::write(
                            path.join("metadata.json"),
                            serde_json::to_vec(&value).unwrap(),
                        )
                        .unwrap();
                    }
                    "oversized" => {
                        let file = std::fs::OpenOptions::new()
                            .append(true)
                            .open(path.join("metadata.json"))
                            .unwrap();
                        file.set_len(
                            crate::model_library::download_recovery::IMPORT_DOCUMENT_MAX_BYTES + 1,
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
                assert!(!observed_ready(scoped, path), "guard bypass for {fault}");
                let id = scoped.get_model_id(path).unwrap();
                assert!(
                    !scoped
                        .get_effective_metadata(&id)
                        .ok()
                        .flatten()
                        .is_some_and(|metadata| metadata.copied_import_ready()),
                    "overlay bypass for {fault}"
                );
                assert!(
                    !cached_target_ready(scoped, &id, path),
                    "load-target cache bypass for {fault}"
                );
                let reader =
                    crate::model_library::PumasReadOnlyLibrary::open(scoped.library_root())
                        .unwrap();
                let snapshot = reader
                    .model_library_selector_snapshot(Default::default())
                    .unwrap();
                assert_eq!(
                    snapshot
                        .rows
                        .iter()
                        .find(|row| row.model_id == id)
                        .unwrap()
                        .artifact_state,
                    crate::models::ModelArtifactState::Invalid
                );
                let indexed = scoped.index().get(&id).unwrap().unwrap();
                assert!(
                    !publication::indexed_publication_ready(
                        scoped.library_root(),
                        &id,
                        &indexed.metadata
                    ),
                    "index/cache bypass for {fault}"
                );
                std::fs::write(path.join("metadata.json"), &canonical).unwrap();
                std::fs::write(&receipt_path, &receipt).unwrap();
            },
        )
        .await;
        assert!(
            observed_ready(&library, &model),
            "restored positive control after {fault}"
        );
    }
    #[cfg(unix)]
    {
        let outside = temp.path().join("outside-receipt");
        let original_receipt = receipt.clone();
        guarded_observation(
            library.clone(),
            model.clone(),
            move |scoped, path, _guard| {
                let receipt_path = path.join(publication::RECEIPT_FILENAME);
                std::fs::rename(&receipt_path, &outside).unwrap();
                std::os::unix::fs::symlink(&outside, &receipt_path).unwrap();
                assert!(
                    !observed_ready(scoped, path),
                    "held evidence must not follow receipt links"
                );
                assert_eq!(std::fs::read(&outside).unwrap(), original_receipt);
                std::fs::remove_file(&receipt_path).unwrap();
                std::fs::rename(&outside, &receipt_path).unwrap();
            },
        )
        .await;
    }
    let displaced = temp.path().join("held-original");
    let replacement = temp.path().join("rejected-replacement");
    let replacement_metadata = canonical.clone();
    let replacement_receipt = receipt.clone();
    guarded_observation(
        library.clone(),
        model.clone(),
        move |scoped, path, guard| {
            std::fs::rename(path, &displaced).unwrap();
            std::fs::create_dir(path).unwrap();
            std::fs::write(path.join("metadata.json"), &replacement_metadata).unwrap();
            std::fs::write(
                path.join(publication::RECEIPT_FILENAME),
                &replacement_receipt,
            )
            .unwrap();
            std::fs::write(path.join("detector.onnx"), b"replacement").unwrap();
            assert!(
                guard.held_destination(path).is_err(),
                "the original held destination must reject the replacement identity"
            );
            // Public reads preserve a diagnostic row rather than returning the
            // replacement's metadata as Ready. The held guard itself still fails.
            let observed = scoped.load_metadata(path).unwrap().unwrap();
            assert!(!observed.copied_import_ready());
            assert_eq!(
                observed.import_state,
                Some(crate::models::ImportState::Pending)
            );
            assert_eq!(
                observed.validation_state,
                Some(crate::models::AssetValidationState::Invalid)
            );
            assert!(observed
                .validation_errors
                .unwrap()
                .iter()
                .any(|error| error.code == "import_publication_metadata_unavailable"));
            assert!(!cached_target_ready(
                scoped,
                &scoped.get_model_id(path).unwrap(),
                path
            ));
            assert_eq!(
                std::fs::read(displaced.join("detector.onnx")).unwrap(),
                b"data"
            );
            assert_eq!(
                std::fs::read(displaced.join("metadata.json")).unwrap(),
                replacement_metadata
            );
            assert_eq!(
                std::fs::read(displaced.join(publication::RECEIPT_FILENAME)).unwrap(),
                replacement_receipt
            );
            assert_eq!(
                std::fs::read(path.join("detector.onnx")).unwrap(),
                b"replacement"
            );
            assert_eq!(
                std::fs::read(path.join("metadata.json")).unwrap(),
                replacement_metadata
            );
            assert_eq!(
                std::fs::read(path.join(publication::RECEIPT_FILENAME)).unwrap(),
                replacement_receipt
            );
            std::fs::rename(path, &replacement).unwrap();
            std::fs::rename(&displaced, path).unwrap();
        },
    )
    .await;
    assert!(observed_ready(&library, &model));
    assert_eq!(
        library
            .load_metadata(&model)
            .unwrap()
            .unwrap()
            .import_publication,
        identity
    );
    assert!(
        ModelImporter::new(library.clone())
            .import_in_place(&orphan_spec(&model))
            .await
            .unwrap()
            .success
    );
    assert_eq!(std::fs::read(&receipt_path).unwrap(), receipt);
    tasks.shutdown_owned().await.unwrap();

    for published_ready_without_index in [false, true] {
        let (temp, library, _downloads, tasks, _root) = custody_fixture().await;
        let spec = copy_spec(temp.path(), "pending-producer");
        let mut importer = ModelImporter::new(library.clone());
        importer.import_hook = Some(Arc::new(move |boundary, destination| {
            if boundary == ImportBoundary::BeforeReady {
                if published_ready_without_index {
                    destination.inject_import_document_uncertainty("metadata.json");
                } else {
                    return Err(std::io::Error::other("retain real Pending publication").into());
                }
            }
            Ok(())
        }));
        assert!(importer.import(&spec).await.is_err());
        let model = library.build_model_path("vision", "publisher", "pending-producer");
        let id = library.get_model_id(&model).unwrap();
        let before = std::fs::read(model.join("metadata.json")).unwrap();
        let before_receipt = std::fs::read(model.join(publication::RECEIPT_FILENAME)).unwrap();
        let canonical: ModelMetadata = serde_json::from_slice(&before).unwrap();
        assert_eq!(
            canonical.copied_import_ready(),
            published_ready_without_index
        );
        library.index().apply_metadata_overlay(&id, &uuid::Uuid::new_v4().to_string(),
            &serde_json::json!({"import_state":"ready", "validation_state":"valid", "import_publication":null}), "integration", None).unwrap();
        guarded_observation(library.clone(), model.clone(), |scoped, path, _guard| {
            assert!(!observed_ready(scoped, path));
            assert!(!scoped
                .get_effective_metadata(&scoped.get_model_id(path).unwrap())
                .unwrap()
                .unwrap()
                .copied_import_ready());
        })
        .await;
        assert!(importer
            .import_in_place(&orphan_spec(&model))
            .await
            .is_err());
        assert!(importer
            .finalize_downloaded_directory(&completed_download(&model, false))
            .await
            .is_err());
        let cold =
            crate::model_library::PumasReadOnlyLibrary::open(library.library_root()).unwrap();
        let snapshot = cold
            .model_library_selector_snapshot(Default::default())
            .unwrap();
        assert_eq!(
            snapshot
                .rows
                .iter()
                .find(|row| row.model_id == id)
                .unwrap()
                .artifact_state,
            crate::models::ModelArtifactState::Invalid
        );
        assert_eq!(std::fs::read(model.join("metadata.json")).unwrap(), before);
        assert_eq!(
            std::fs::read(model.join(publication::RECEIPT_FILENAME)).unwrap(),
            before_receipt
        );
        assert!(
            tasks.shutdown_owned().await.is_err(),
            "retain the real publication failure"
        );
    }
}

/// Run the real neutral-workspace -> verified lease -> guarded HF importer.
/// It intentionally leaves Using custody and its completion receipt unsettled,
/// so the caller can test exact consumer settlement and cold observation.
async fn produce_hf(
    library: Arc<ModelLibrary>,
    downloads: Arc<DownloadPersistence>,
    root: DownloadDestinationRoot,
    info: DownloadCompletionInfo,
    attempt: String,
) -> Result<(AcquisitionRecord, HfCompletionReceipt)> {
    let (record, receipt) = produce_hf_using(library, downloads, root, info, attempt, true).await?;
    Ok((
        record,
        receipt.expect("completed HF producer issues its receipt"),
    ))
}

async fn produce_hf_using(
    library: Arc<ModelLibrary>,
    downloads: Arc<DownloadPersistence>,
    root: DownloadDestinationRoot,
    info: DownloadCompletionInfo,
    attempt: String,
    complete_import: bool,
) -> Result<(AcquisitionRecord, Option<HfCompletionReceipt>)> {
    let acquisition = Arc::new(AcquisitionService::new(downloads.acquisition_store()));
    let scope = acquisition.supervisor().open_scope(|| async { Ok(()) })?;
    let service = acquisition.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    let prepared = scope.prepare(
        info.download_id.clone(),
        TaskRole::Worker,
        move |context| async move {
            let outcome = async {
                // Exclusion precedes every synthetic source/destination effect.
                let grant = Arc::new(root.try_acquire_execution_grant()?);
                let context = context.with_effect_lease(Some(grant.clone()));
                std::fs::create_dir_all(&info.dest_dir)?;
                std::fs::write(info.dest_dir.join("detector.onnx"), b"data")?;
                let workspace = root
                    .resolve(&info.dest_dir)?
                    .acquisition_workspace(grant.clone())?;
                let manifest = ArtifactManifest::new(
                    ArtifactSourceIdentity::new(
                        "huggingface",
                        &info.download_request.repo_id,
                        ArtifactRevisionEvidence::new(
                            "huggingface.commit",
                            "main",
                            RevisionStrength::Weak,
                        )
                        .expect("synthetic acquisition manifest must be valid"),
                    )
                    .expect("synthetic acquisition manifest must be valid"),
                    vec![ArtifactFile::new(
                        "detector.onnx",
                        "detector.onnx",
                        Some(4),
                        None,
                        FileVerificationRequirement::CompleteRepresentation,
                    )
                    .expect("synthetic acquisition manifest must be valid")],
                )
                .expect("synthetic acquisition manifest must be valid");
                let operation = service
                    .begin(
                        &context,
                        AcquisitionDemand {
                            consumer: "hf.model".into(),
                            operation: attempt,
                        },
                        manifest,
                        workspace.identity().clone(),
                        None,
                    )
                    .await?;
                let lease = service.files_ready(&context, operation, workspace).await?;
                let record = lease.record().clone();
                if !complete_import {
                    drop(lease);
                    return Ok((record, None));
                }
                let capability = ModelFinalImportCapability::new(
                    service.use_proof(&context, &lease)?,
                    &info.download_id,
                    DownloadAdmissionDomain::Ambient,
                    grant,
                    Arc::new(DownloadCancellation::new()),
                );
                let result = ModelImporter::new(library)
                    .finalize_downloaded_directory_with_capability(
                        &info,
                        &DownloadRevision::legacy_main(),
                        &context,
                        capability,
                    )
                    .await?;
                assert!(result.success);
                let receipt = downloads.read_hf_completion_receipt(record.id)?.unwrap();
                drop(lease);
                Ok((record, Some(receipt)))
            }
            .await;
            send.send(outcome).unwrap();
        },
    )?;
    scope.install_gated(prepared).unwrap().start();
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), receive)
        .await
        .unwrap()
        .unwrap();
    acquisition.shutdown().await?;
    result
}

fn no_copy_stages(library: &ModelLibrary) -> bool {
    std::fs::read_dir(library.library_root())
        .unwrap()
        .all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(TEMP_IMPORT_PREFIX)
        })
}

#[tokio::test]
async fn acquisition_integration_copy_and_hf_exclude_each_other() {
    // Hold the copied producer before publication. The HF producer cannot take
    // its first destination effect or manufacture a second root owner.
    let (temp, library, downloads, tasks, root) = custody_fixture().await;
    let spec = copy_spec(temp.path(), "copy-owner");
    let model = library.build_model_path("vision", "publisher", "copy-owner");
    let (entered_send, entered) = tokio::sync::oneshot::channel();
    let entered_send = Mutex::new(Some(entered_send));
    let (release_send, release) = std::sync::mpsc::channel();
    let release = Mutex::new(release);
    let mut importer = ModelImporter::new(library.clone());
    importer.import_hook = Some(Arc::new(move |boundary, _| {
        if boundary == ImportBoundary::StageCreated {
            entered_send
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(())
                .unwrap();
            release.lock().unwrap().recv().unwrap();
        }
        Ok(())
    }));
    let copy = tokio::spawn(async move { importer.import(&spec).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered)
        .await
        .unwrap()
        .unwrap();
    let mut info = completed_download(&model, false);
    info.download_id = "blocked-hf".into();
    // The copy already owns the root. A competing producer is refused before
    // source/workspace/metadata effects even if its request was selected.
    assert!(matches!(
        produce_hf(
            library.clone(),
            downloads.clone(),
            root.clone(),
            info,
            "not-admitted".into()
        )
        .await,
        Err(PumasError::DownloadRootBusy)
    ));
    assert!(!model.exists());
    assert!(downloads
        .acquisition_store()
        .acquisitions()
        .unwrap()
        .is_empty());
    release_send.send(()).unwrap();
    assert!(copy.await.unwrap().unwrap().success);
    assert!(no_copy_stages(&library));

    // Hold managed HF at its actual metadata publication boundary. Ordinary
    // copied admission must refuse before allocating a private stage.
    let hf_spec = copy_spec(temp.path(), "hf-owner");
    let hf_model = library.build_model_path("vision", "publisher", "hf-owner");
    let mut info = completed_download(&hf_model, false);
    info.download_id = "hf-owner".into();
    let attempt = real_admission(&downloads, &root, &info, &info.download_id);
    let (entered_send, entered) = tokio::sync::oneshot::channel();
    let entered_send = Mutex::new(Some(entered_send));
    let (release_send, release) = std::sync::mpsc::channel();
    let release = Mutex::new(release);
    library.set_metadata_write_notifier(Some(Arc::new(move |_| {
        if let Some(send) = entered_send.lock().unwrap().take() {
            send.send(()).unwrap();
            release.lock().unwrap().recv().unwrap();
        }
    })));
    let hf = tokio::spawn(produce_hf(
        library.clone(),
        downloads.clone(),
        root.clone(),
        info,
        attempt,
    ));
    tokio::time::timeout(std::time::Duration::from_secs(5), entered)
        .await
        .unwrap()
        .unwrap();
    let held_store = std::fs::read(temp.path().join("downloads.json")).unwrap();
    assert!(matches!(
        ModelImporter::new(library.clone()).import(&hf_spec).await,
        Err(PumasError::DownloadRootBusy)
    ));
    assert!(no_copy_stages(&library));
    assert!(!hf_model.join("metadata.json").exists());
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        held_store
    );
    assert_eq!(
        std::fs::read(hf_model.join("detector.onnx")).unwrap(),
        b"data"
    );
    release_send.send(()).unwrap();
    let (record, receipt) = hf.await.unwrap().unwrap();
    library.set_metadata_write_notifier(None);
    let grant = Arc::new(root.try_acquire_execution_grant().unwrap());
    assert!(library
        .settle_hf_completion_receipt(&hf_model, &record, &receipt, grant)
        .await
        .unwrap());
    assert!(
        ModelImporter::new(library.clone())
            .import_in_place(&orphan_spec(&hf_model))
            .await
            .unwrap()
            .success
    );

    let fresh = copy_spec(temp.path(), "fresh-after-settlement");
    assert!(
        ModelImporter::new(library.clone())
            .import(&fresh)
            .await
            .unwrap()
            .success
    );

    // Durable queued and cold/hidden admissions retain the same exclusion even
    // when no worker currently holds the native grant.
    for state in ["queued", "hidden", "pending"] {
        let hidden = state == "hidden";
        let name = match state {
            "hidden" => "hidden-owner",
            "pending" => "pending-owner",
            _ => "queued-owner",
        };
        let spec = copy_spec(temp.path(), name);
        let target = library.build_model_path("vision", "publisher", name);
        let original = DownloadPersistence::new(temp.path());
        let mut info = completed_download(&target, false);
        info.download_id = name.into();
        let store = if hidden {
            &original
        } else {
            downloads.as_ref()
        };
        let attempt = real_admission(store, &root, &info, name);
        if state == "pending" {
            let snapshot = downloads
                .load_lifecycle_inventory_strict()
                .unwrap()
                .downloads
                .into_iter()
                .find(|snapshot| snapshot.download_id == name)
                .unwrap();
            downloads
                .begin_lifecycle_quarantine(
                    &snapshot,
                    crate::model_library::download_store::LifecycleQuarantineDomain::Ambient,
                    false,
                    Some(&attempt),
                )
                .unwrap();
        }
        if hidden {
            assert!(downloads
                .load_lifecycle_inventory_strict()
                .unwrap()
                .hidden_admissions
                .contains_key(name));
        }
        let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
        assert!(matches!(
            ModelImporter::new(library.clone()).import(&spec).await,
            Err(PumasError::DownloadRootBusy)
        ));
        assert!(!target.exists());
        assert!(no_copy_stages(&library));
        assert_eq!(
            std::fs::read(temp.path().join("downloads.json")).unwrap(),
            before
        );
    }
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn acquisition_integration_completion_receipts_are_not_interchangeable() {
    let (temp, library, downloads, tasks, root) = custody_fixture().await;
    let copied = ModelImporter::new(library.clone())
        .import(&copy_spec(temp.path(), "copied"))
        .await
        .unwrap();
    assert!(copied.success);
    let copied_id = copied.model_id.unwrap();
    let copied_model = library.library_root().join(&copied_id);
    prime_summary(&library, &copied_id, &copied_model);
    assert!(cached_target_ready(&library, &copied_id, &copied_model));
    let copied_receipt = std::fs::read(copied_model.join(publication::RECEIPT_FILENAME)).unwrap();
    let hf_model = library.build_model_path("vision", "publisher", "managed-hf");
    let mut info = completed_download(&hf_model, false);
    info.download_id = "managed-hf".into();
    let attempt = real_admission(&downloads, &root, &info, &info.download_id);
    let (record, receipt) = produce_hf(
        library.clone(),
        downloads.clone(),
        root.clone(),
        info,
        attempt,
    )
    .await
    .unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
    assert!(!hf_model.join(publication::RECEIPT_FILENAME).exists());
    assert!(serde_json::from_slice::<HfCompletionReceipt>(&copied_receipt).is_err());
    let store_before = std::fs::read(temp.path().join("downloads.json")).unwrap();

    // A valid HF completion document cannot stand in for the copied producer's
    // physical publication receipt, whether the observer is warm or reopened.
    std::fs::write(
        copied_model.join(publication::RECEIPT_FILENAME),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    assert!(!observed_ready(&library, &copied_model));
    let cold_library =
        crate::model_library::PumasReadOnlyLibrary::open(library.library_root()).unwrap();
    assert!(!cold_library
        .resolve_model_artifact_load_target(artifact_request(
            &copied_id,
            &copied_model,
            crate::models::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed
        ))
        .unwrap()
        .is_ready());
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        store_before
    );
    std::fs::write(
        copied_model.join(publication::RECEIPT_FILENAME),
        &copied_receipt,
    )
    .unwrap();
    assert!(observed_ready(&library, &copied_model));
    assert!(cold_library
        .resolve_model_artifact_load_target(artifact_request(
            &copied_id,
            &copied_model,
            crate::models::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed
        ))
        .unwrap()
        .is_ready());

    // Copied evidence does not clear a retained HF use or its queue. Exact HF
    // output proof and record identity remain mandatory for warm/cold settlement.
    let mut wrong = receipt.clone();
    wrong.model_id = copied_id;
    for store in [
        downloads.clone(),
        Arc::new(DownloadPersistence::new(temp.path())),
    ] {
        assert_eq!(
            store.acquisition_store().acquisitions().unwrap()[&record.id],
            record
        );
        assert_eq!(
            store.read_hf_completion_receipt(record.id).unwrap(),
            Some(receipt.clone())
        );
        let grant = Arc::new(root.try_acquire_execution_grant().unwrap());
        assert!(library
            .validate_hf_completion_receipt(&copied_model, &wrong, &record, grant)
            .await
            .is_err());
        assert_eq!(
            std::fs::read(temp.path().join("downloads.json")).unwrap(),
            store_before
        );
    }
    std::fs::write(
        hf_model.join(publication::RECEIPT_FILENAME),
        &copied_receipt,
    )
    .unwrap();
    let grant = Arc::new(root.try_acquire_execution_grant().unwrap());
    assert!(
        library
            .settle_hf_completion_receipt(&hf_model, &record, &receipt, grant)
            .await
            .is_err(),
        "a copied receipt cannot settle the real HF producer"
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        store_before
    );
    std::fs::remove_file(hf_model.join(publication::RECEIPT_FILENAME)).unwrap();

    for fault in ["missing", "copied", "wrong_lease"] {
        let mut document: serde_json::Value = serde_json::from_slice(&store_before).unwrap();
        let key = record.id.to_string();
        match fault {
            "missing" => {
                document["consumer_receipts"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&key);
            }
            "copied" => {
                document["consumer_receipts"][&key] =
                    serde_json::from_slice(&copied_receipt).unwrap();
            }
            _ => {
                // The exact canonical partition is deliberately corrupted only
                // in this temporary fixture, never converted or auto-repaired.
                document["consumer_receipts"][&key]["use_lease"] =
                    uuid::Uuid::new_v4().to_string().into();
            }
        }
        let damaged = serde_json::to_vec(&document).unwrap();
        std::fs::write(temp.path().join("downloads.json"), &damaged).unwrap();
        for store in [
            downloads.clone(),
            Arc::new(DownloadPersistence::new(temp.path())),
        ] {
            let observed = store.read_hf_completion_receipt(record.id);
            if fault == "missing" {
                assert!(observed.unwrap().is_none());
            } else {
                assert!(observed.is_err());
            }
            let grant = Arc::new(root.try_acquire_execution_grant().unwrap());
            assert!(library
                .settle_hf_completion_receipt(&hf_model, &record, &receipt, grant)
                .await
                .is_err());
            assert_eq!(
                std::fs::read(temp.path().join("downloads.json")).unwrap(),
                damaged
            );
            assert!(observed_ready(&library, &copied_model));
        }
        std::fs::write(temp.path().join("downloads.json"), &store_before).unwrap();
    }
    let grant = Arc::new(root.try_acquire_execution_grant().unwrap());
    assert!(library
        .settle_hf_completion_receipt(&hf_model, &record, &receipt, grant)
        .await
        .unwrap());
    assert!(matches!(
        downloads.acquisition_store().acquisitions().unwrap()[&record.id].phase,
        AcquisitionPhase::Adopted { .. }
    ));
    assert!(!downloads
        .load_lifecycle_inventory_strict()
        .unwrap()
        .queue_admissions
        .contains_key("managed-hf"));
    assert!(observed_ready(&library, &copied_model));
    let receiptless_model = library.build_model_path("vision", "publisher", "receiptless");
    let mut receiptless_info = completed_download(&receiptless_model, false);
    receiptless_info.download_id = "receiptless".into();
    let attempt = real_admission(&downloads, &root, &receiptless_info, "receiptless");
    let (receiptless_record, no_receipt) = produce_hf_using(
        library.clone(),
        downloads.clone(),
        root.clone(),
        receiptless_info,
        attempt,
        false,
    )
    .await
    .unwrap();
    assert!(no_receipt.is_none());
    let retained = std::fs::read(temp.path().join("downloads.json")).unwrap();
    let retained_document: serde_json::Value = serde_json::from_slice(&retained).unwrap();
    let expected_queue: crate::model_library::download_store::PersistedQueueAdmission =
        serde_json::from_value(retained_document["queue_admissions"]["receiptless"].clone())
            .unwrap();
    let expected_snapshot = retained_document["downloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|snapshot| snapshot["download_id"] == "receiptless")
        .unwrap();
    for (cold, store) in [
        (false, downloads.clone()),
        (true, Arc::new(DownloadPersistence::new(temp.path()))),
    ] {
        assert!(store
            .read_hf_completion_receipt(receiptless_record.id)
            .unwrap()
            .is_none());
        assert_eq!(
            store.acquisition_store().acquisitions().unwrap()[&receiptless_record.id],
            receiptless_record
        );
        let inventory = store.load_lifecycle_inventory_strict().unwrap();
        if cold {
            // A reopened facade has no in-memory durability acknowledgement.
            // Its exact durable queue record therefore remains hidden custody;
            // absence from the warm projection is never a release receipt.
            assert!(!inventory.queue_admissions.contains_key("receiptless"));
            let hidden = inventory.hidden_admissions.get("receiptless").unwrap();
            assert_eq!(hidden.position, expected_queue.position);
            assert_eq!(hidden.request.domain, expected_queue.domain);
            assert_eq!(hidden.request.destination, expected_queue.destination);
            assert_eq!(
                hidden.request.requested_payload_files,
                expected_queue.requested_payload_files
            );
            assert_eq!(
                hidden.request.execution_files,
                expected_queue.execution_files
            );
            assert_eq!(
                serde_json::to_value(&hidden.request.snapshot).unwrap(),
                *expected_snapshot
            );
        } else {
            assert_eq!(
                inventory.queue_admissions.get("receiptless"),
                Some(&expected_queue)
            );
            assert!(!inventory.hidden_admissions.contains_key("receiptless"));
        }
        assert!(!receiptless_model.join("metadata.json").exists());
        assert_eq!(
            std::fs::read(receiptless_model.join("detector.onnx")).unwrap(),
            b"data"
        );
        assert_eq!(
            std::fs::read(temp.path().join("downloads.json")).unwrap(),
            retained
        );
    }
    let competitor = copy_spec(temp.path(), "receiptless");
    assert!(matches!(
        ModelImporter::new(library.clone())
            .import(&competitor)
            .await,
        Err(PumasError::DownloadRootBusy)
    ));
    assert!(no_copy_stages(&library));
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        retained
    );
    tasks.shutdown_owned().await.unwrap();
    assert_completion_grant_survives_caller_loss().await;
}

async fn assert_completion_grant_survives_caller_loss() {
    for outcome in ["valid", "missing", "wrong_lease"] {
        let (temp, library, downloads, tasks, root) = custody_fixture().await;
        let model = library.build_model_path("vision", "publisher", "held-completion");
        let mut info = completed_download(&model, false);
        info.download_id = "held-completion".into();
        let attempt = real_admission(&downloads, &root, &info, "held-completion");
        let (record, receipt) = produce_hf(
            library.clone(),
            downloads.clone(),
            root.clone(),
            info,
            attempt,
        )
        .await
        .unwrap();
        let path = temp.path().join("downloads.json");
        let mut document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if outcome == "missing" {
            document["consumer_receipts"]
                .as_object_mut()
                .unwrap()
                .remove(&record.id.to_string());
        } else if outcome == "wrong_lease" {
            document["consumer_receipts"][record.id.to_string()]["use_lease"] =
                uuid::Uuid::new_v4().to_string().into();
        }
        let evidence = serde_json::to_vec(&document).unwrap();
        let expected_evidence = evidence.clone();
        let (entered_send, entered) = tokio::sync::oneshot::channel();
        let entered_send = Mutex::new(Some(entered_send));
        let (release_send, release) = std::sync::mpsc::channel();
        let release = Mutex::new(release);
        library.set_hf_completion_validation_hook(Some(Arc::new(move || {
            // Inject only after the held metadata read, at the exact lifetime
            // gap. Neither this callback nor its channels retain a root grant.
            std::fs::write(&path, &evidence).unwrap();
            entered_send
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(())
                .unwrap();
            release.lock().unwrap().recv().unwrap();
        })));
        let acquisition = Arc::new(AcquisitionService::new(downloads.acquisition_store()));
        let scope = acquisition
            .supervisor()
            .open_scope(|| async { Ok(()) })
            .unwrap();
        let worker_library = library.clone();
        let worker_root = root.clone();
        let worker_record = record.clone();
        let caller = tokio::spawn(async move {
            scope
                .run_invocation(move |context| async move {
                    let grant = Arc::new(worker_root.try_acquire_execution_grant()?);
                    context
                        .run_fallible_async_named(
                            "settle held real HF receipt",
                            move || async move {
                                worker_library
                                    .settle_hf_completion_receipt(
                                        &model,
                                        &worker_record,
                                        &receipt,
                                        grant,
                                    )
                                    .await
                            },
                        )
                        .await
                        .map_err(|error| {
                            PumasError::Other(format!("Completion effect failed: {error}"))
                        })??;
                    Ok(())
                })
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), entered)
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                root.try_acquire_execution_grant(),
                Err(PumasError::DownloadRootBusy)
            ),
            "grant escaped after held read: {outcome}"
        );
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        let shutdown_service = acquisition.clone();
        let shutdown = tokio::spawn(async move { shutdown_service.shutdown().await });
        tokio::task::yield_now().await;
        assert!(!shutdown.is_finished());
        assert!(
            matches!(
                root.try_acquire_execution_grant(),
                Err(PumasError::DownloadRootBusy)
            ),
            "caller loss released completion custody: {outcome}"
        );
        release_send.send(()).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), shutdown)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.is_ok(), outcome == "valid", "{outcome}: {result:?}");
        library.set_hf_completion_validation_hook(None);
        assert!(root.try_acquire_execution_grant().is_ok());
        if outcome == "valid" {
            assert!(matches!(
                downloads.acquisition_store().acquisitions().unwrap()[&record.id].phase,
                AcquisitionPhase::Adopted { .. }
            ));
            assert!(!downloads
                .load_lifecycle_inventory_strict()
                .unwrap()
                .queue_admissions
                .contains_key("held-completion"));
        } else {
            assert_eq!(
                std::fs::read(temp.path().join("downloads.json")).unwrap(),
                expected_evidence
            );
            let retained: serde_json::Value = serde_json::from_slice(&expected_evidence).unwrap();
            assert_eq!(
                serde_json::from_value::<AcquisitionRecord>(
                    retained["acquisitions"][record.id.to_string()].clone()
                )
                .unwrap(),
                record
            );
            // Failure remains repeatable; no swallowed ambiguity or apparent
            // successful shutdown is manufactured after cancellation.
            assert!(acquisition.shutdown().await.is_err());
        }
        tasks.shutdown_owned().await.unwrap();
    }
}

fn artifact_request(
    id: &str,
    target: &Path,
    mode: crate::models::PumasArtifactLoadTargetResolutionMode,
) -> crate::models::ResolveModelArtifactLoadTargetRequest {
    crate::models::ResolveModelArtifactLoadTargetRequest {
        model_ref: crate::models::PumasModelRef {
            model_ref_contract_version: crate::models::PUMAS_MODEL_REF_CONTRACT_VERSION,
            model_id: id.into(),
            revision: None,
            selected_artifact_id: Some("detector.onnx".into()),
            selected_artifact_path: Some(target.join("detector.onnx").display().to_string()),
            migration_diagnostics: vec![],
        },
        expected_artifact_kind: None,
        caller_observed_entry_path: None,
        caller_observed_package_facts_contract_version: None,
        resolution_mode: mode,
        consumer: crate::models::PumasArtifactConsumer {
            consumer_name: "copied publication regression".into(),
            task_kind: None,
            runtime_family: None,
        },
    }
}

fn prime_summary(library: &ModelLibrary, id: &str, target: &Path) {
    use crate::models::*;
    let request = artifact_request(
        id,
        target,
        PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
    );
    let summary = ResolvedModelPackageFactsSummary {
        package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION,
        model_ref: request.model_ref,
        artifact_kind: PackageArtifactKind::Onnx,
        entry_path: target.join("detector.onnx").display().to_string(),
        storage_kind: StorageKind::LibraryOwned,
        validation_state: AssetValidationState::Valid,
        task: TaskEvidence {
            pipeline_tag: None,
            task_type_primary: None,
            input_modalities: vec![],
            output_modalities: vec![],
        },
        backend_hints: BackendHintFacts {
            accepted: vec![BackendHintLabel::OnnxRuntime],
            raw: vec![],
            unsupported: vec![],
        },
        requires_custom_code: false,
        config_status: PackageFactStatus::Present,
        tokenizer_status: PackageFactStatus::Uninspected,
        processor_status: PackageFactStatus::Uninspected,
        generation_config_status: PackageFactStatus::Uninspected,
        generation_defaults_status: PackageFactStatus::Uninspected,
        image_generation_family_evidence: vec![],
        diffusers_pipeline_class: None,
        gguf_architecture: None,
        diagnostic_codes: vec![],
    };
    for selected in ["", "detector.onnx"] {
        library
            .index()
            .upsert_model_package_facts_cache(&crate::index::ModelPackageFactsCacheRecord {
                model_id: id.into(),
                selected_artifact_id: selected.into(),
                cache_scope: crate::index::ModelPackageFactsCacheScope::Summary,
                package_facts_contract_version: i64::from(PACKAGE_FACTS_CONTRACT_VERSION),
                producer_revision: None,
                source_fingerprint: "fixture".into(),
                facts_json: serde_json::to_string(&summary).unwrap(),
                cached_at: "2026-10-03T00:00:00Z".into(),
                updated_at: "2026-10-03T00:00:00Z".into(),
            })
            .unwrap();
    }
}

fn cached_target_ready(library: &ModelLibrary, id: &str, path: &Path) -> bool {
    let reader = crate::model_library::PumasReadOnlyLibrary::open(library.library_root()).unwrap();
    reader
        .resolve_model_artifact_load_target(artifact_request(
            id,
            path,
            crate::models::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
        ))
        .unwrap()
        .is_ready()
}
