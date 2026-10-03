//! Publication protocol, readiness and recovery-boundary regressions.

use super::*;

#[tokio::test]
async fn copied_import_pending_confirmed_ready_are_distinct_durable_steps() {
    let mut fixture = Fixture::new().await;
    let observations = Arc::new(AtomicUsize::new(0));
    let observed = observations.clone();
    fixture.importer.import_hook = Some(Arc::new(move |boundary, destination| {
        if matches!(
            boundary,
            ImportBoundary::BeforePublish | ImportBoundary::Published | ImportBoundary::BeforeReady
        ) {
            let metadata = destination.read_model_metadata()?.unwrap();
            assert_eq!(
                metadata.import_state,
                Some(crate::models::ImportState::Pending)
            );
            assert!(!metadata.copied_import_ready());
            let receipt = receipt_at(destination.display_path());
            assert_eq!(receipt["version"], 1);
            assert_eq!(
                receipt["id"].as_str(),
                metadata
                    .import_publication
                    .as_ref()
                    .map(|value| value.id.as_str())
            );
            assert_eq!(receipt["state"], "pending");
            assert!(receipt["payload"]["files"]["model.onnx"]["sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64));
            observed.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }));
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(result.success);
    assert_eq!(observations.load(Ordering::SeqCst), 3);
    let metadata = fixture
        .library
        .load_metadata(&fixture.target())
        .unwrap()
        .unwrap();
    assert!(metadata.copied_import_ready());
    assert_eq!(receipt_at(&fixture.target())["state"], "confirmed");
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_release_replacement_is_published_pending_and_never_promoted() {
    for mutation in ["directory", "same_size_bytes", "unknown_file"] {
        let mut fixture = Fixture::new().await;
        let bundle = crate::model_library::importer::tests::create_external_diffusers_bundle(
            fixture.temp.path(),
        );
        fixture.spec.path = bundle.display().to_string();
        let original = fixture.temp.path().join("released-original");
        let old = original.clone();
        fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
            if boundary == ImportBoundary::DescendantsReleased {
                match mutation {
                    "directory" => {
                        std::fs::rename(stage.display_path().join("tokenizer"), &old)?;
                        std::fs::create_dir(stage.display_path().join("tokenizer"))?;
                        std::fs::write(
                            stage.display_path().join("tokenizer/sentinel"),
                            b"replacement",
                        )?;
                    }
                    "same_size_bytes" => {
                        let path = stage.display_path().join("model_index.json");
                        let mut bytes = std::fs::read(&path)?;
                        bytes[0] = b' ';
                        std::fs::write(path, bytes)?;
                    }
                    _ => std::fs::write(
                        stage.display_path().join("unknown.payload"),
                        b"preserve unknown",
                    )?,
                }
            }
            Ok(())
        }));
        let error = fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("was published"), "{error}");
        if mutation != "same_size_bytes" {
            assert!(error.contains("custody unknown"), "{error}");
        }
        let target = fixture
            .library
            .build_model_path("diffusion", "fixture", "Owned Import");
        assert_eq!(receipt_at(&target)["state"], "pending");
        assert!(!fixture
            .library
            .load_metadata(&target)
            .unwrap()
            .unwrap()
            .copied_import_ready());
        assert!(fixture.stages().is_empty());
        if mutation == "directory" {
            assert!(original.join("tokenizer.json").is_file());
            assert_eq!(
                std::fs::read(target.join("tokenizer/sentinel")).unwrap(),
                b"replacement"
            );
        } else if mutation == "unknown_file" {
            assert_eq!(
                std::fs::read(target.join("unknown.payload")).unwrap(),
                b"preserve unknown"
            );
        }
        fixture.library.rebuild_index().await.unwrap();
        let id = fixture.library.get_model_id(&target).unwrap();
        assert!(fixture
            .library
            .resolve_model_execution_descriptor(&id)
            .await
            .unwrap_err()
            .to_string()
            .contains("unconfirmed"));
        assert_eq!(receipt_at(&target)["state"], "pending");
        assert_eq!(fixture.importer.adopt_orphans(false).await.adopted, 0);
        assert!(fixture.tasks.shutdown_owned().await.is_err());
    }
}

#[tokio::test]
async fn copied_import_failed_rename_rebind_cleanup_and_mismatch_receipts() {
    for mismatch in [false, true] {
        let mut fixture = Fixture::new().await;
        let target = fixture.target();
        let destination = target.clone();
        fixture.importer.import_hook = Some(Arc::new(move |boundary, stage| {
            if boundary == ImportBoundary::DescendantsReleased {
                std::fs::create_dir_all(&destination)?;
                std::fs::write(destination.join("sentinel"), b"existing target")?;
                if mismatch {
                    std::fs::write(stage.display_path().join("unknown.payload"), b"retain me")?;
                }
            }
            Ok(())
        }));
        let result = fixture.importer.import(&fixture.spec).await;
        if mismatch {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("already exists"), "{error}");
            assert!(error.contains("exact identity rebind failed"), "{error}");
            let stage = fixture.stages().pop().unwrap();
            assert_eq!(
                std::fs::read(stage.join("unknown.payload")).unwrap(),
                b"retain me"
            );
            assert_eq!(receipt_at(&stage)["state"], "pending");
            assert!(fixture.tasks.shutdown_owned().await.is_err());
        } else {
            assert!(!result.unwrap().success);
            assert!(fixture.stages().is_empty());
            fixture.tasks.shutdown_owned().await.unwrap();
        }
        assert_eq!(
            std::fs::read(target.join("sentinel")).unwrap(),
            b"existing target"
        );
    }
}

#[tokio::test]
async fn copied_import_pending_and_confirmed_failures_do_not_become_ready_at_startup() {
    for boundary in [ImportBoundary::Published, ImportBoundary::BeforeReady] {
        let mut fixture = Fixture::new().await;
        fixture.importer.import_hook = Some(Arc::new(move |current, _| {
            if current == boundary {
                Err(fault("injected metadata finalization failure"))
            } else {
                Ok(())
            }
        }));
        let error = fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("was published"));
        let target = fixture.target();
        let before = std::fs::read(target.join("metadata.json")).unwrap();
        assert_eq!(receipt_at(&target)["state"], "pending");
        fixture.library.rebuild_index().await.unwrap();
        fixture.library.index_model_dir(&target).await.unwrap();
        assert_eq!(std::fs::read(target.join("metadata.json")).unwrap(), before);
        let id = fixture.library.get_model_id(&target).unwrap();
        assert!(fixture
            .library
            .resolve_model_execution_descriptor(&id)
            .await
            .is_err());
        assert!(
            !fixture.library.index().get(&id).unwrap().unwrap().metadata["import_publication"]
                ["confirmed"]
                .as_bool()
                .unwrap()
        );
        assert!(fixture.tasks.shutdown_owned().await.is_err());
    }
}

#[tokio::test]
async fn copied_import_indexed_identity_collision_preserves_existing_asset() {
    let fixture = Fixture::new().await;
    let id = fixture.library.get_model_id(&fixture.target()).unwrap();
    let old_path = fixture.temp.path().join("recoverable-original");
    std::fs::create_dir(&old_path).unwrap();
    std::fs::write(old_path.join("sentinel"), b"recoverable bytes").unwrap();
    let original = crate::index::ModelRecord {
        id: id.clone(),
        path: old_path.display().to_string(),
        cleaned_name: "original".into(),
        official_name: "Original".into(),
        model_type: "vision".into(),
        tags: vec![],
        hashes: Default::default(),
        metadata: serde_json::json!({"validation_state":"valid", "import_state":"ready"}),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    fixture.library.index().upsert(&original).unwrap();
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(!result.success);
    assert!(result.error.unwrap().contains("identity already exists"));
    let after = fixture.library.index().get(&id).unwrap().unwrap();
    assert_eq!(after.metadata, original.metadata);
    assert_eq!(after.path, original.path);
    assert_eq!(
        std::fs::read(old_path.join("sentinel")).unwrap(),
        b"recoverable bytes"
    );
    assert!(fixture.stages().is_empty());
    assert!(!fixture.target().exists());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_orphan_walk_prunes_nested_staging_subtrees() {
    let fixture = Fixture::new().await;
    let hidden = fixture
        .library
        .library_root()
        .join(".tmp_import_retained/nested/component");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::write(hidden.join("weights.onnx"), b"unpublished").unwrap();
    assert!(!fixture.importer.has_orphan_candidates());
    let report = fixture.importer.adopt_orphans(false).await;
    assert_eq!(report.orphans_found, 0);
    assert_eq!(report.adopted, 0);
    assert!(!hidden.join("metadata.json").exists());
    fixture.tasks.shutdown_owned().await.unwrap();
}
#[tokio::test]
async fn copied_import_receipt_uncertainty_and_ready_metadata_uncertainty_are_distinct() {
    for ready_write in [false, true] {
        let mut fixture = Fixture::new().await;
        fixture.importer.import_hook = Some(Arc::new(move |boundary, target| {
            if !ready_write && boundary == ImportBoundary::BeforeConfirm {
                target.inject_import_document_uncertainty(RECEIPT_FILENAME);
            }
            if ready_write && boundary == ImportBoundary::BeforeReady {
                target.inject_import_document_uncertainty("metadata.json");
            }
            Ok(())
        }));
        let error = fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("was published"), "{error}");
        assert!(
            error.contains(if ready_write {
                "Ready metadata finalization"
            } else {
                "Confirmed publication receipt"
            }),
            "{error}"
        );
        let target = fixture.target();
        assert!(target.join("model.onnx").is_file());
        assert_eq!(
            receipt_at(&target)["state"],
            "confirmed",
            "a visible receipt is not proof the producer observed durable confirmation"
        );
        let metadata = fixture.library.load_metadata(&target).unwrap().unwrap();
        assert!(
            !metadata.copied_import_ready(),
            "Pending index fences both visible document outcomes"
        );
        let id = fixture.library.get_model_id(&target).unwrap();
        assert!(!crate::models::copied_import_ready_value(
            &fixture.library.index().get(&id).unwrap().unwrap().metadata
        ));
        if !ready_write {
            fixture.library.rebuild_index().await.unwrap();
            assert!(!fixture
                .library
                .load_metadata(&target)
                .unwrap()
                .unwrap()
                .copied_import_ready());
            assert!(fixture
                .library
                .resolve_model_execution_descriptor(&id)
                .await
                .is_err());
        }
        assert!(fixture.tasks.shutdown_owned().await.is_err());
    }
}

#[tokio::test]
async fn copied_import_final_metadata_notifier_cannot_confirm_changed_payload() {
    let fixture = Fixture::new().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    fixture
        .library
        .set_metadata_write_notifier(Some(Arc::new(move |path| {
            if observed.fetch_add(1, Ordering::SeqCst) == 1 {
                let weights = path.parent().unwrap().join("model.onnx");
                let bytes = std::fs::read(&weights).unwrap();
                std::fs::write(weights, vec![b'x'; bytes.len()]).unwrap();
            }
        })));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("was published"));
    assert!(error.contains("payload identity, contents or completeness changed"));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(receipt_at(&fixture.target())["state"], "pending");
    assert!(!fixture
        .library
        .load_metadata(&fixture.target())
        .unwrap()
        .unwrap()
        .copied_import_ready());
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}
#[tokio::test]
async fn copied_import_pending_receipt_uncertainty_cleans_before_publication() {
    let mut fixture = Fixture::new().await;
    fixture.importer.import_hook = Some(Arc::new(|boundary, stage| {
        if boundary == ImportBoundary::BeforeMetadata {
            stage.inject_import_document_uncertainty(RECEIPT_FILENAME);
        }
        Ok(())
    }));
    let error = fixture
        .importer
        .import(&fixture.spec)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("Pending publication receipt"));
    assert!(error.contains("durability/visibility is uncertain"));
    assert!(fixture.stages().is_empty());
    assert!(!fixture.target().exists());
    assert_eq!(fixture.library.model_count().unwrap(), 0);
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}
#[tokio::test]
async fn copied_import_public_metadata_edits_and_reinspection_cannot_forge_confirmation() {
    let mut fixture = Fixture::new().await;
    fixture.importer.import_hook = Some(Arc::new(|boundary, _| {
        if boundary == ImportBoundary::Published {
            Err(fault("leave Pending for metadata edits"))
        } else {
            Ok(())
        }
    }));
    assert!(fixture.importer.import(&fixture.spec).await.is_err());
    let target = fixture.target();
    let id = fixture.library.get_model_id(&target).unwrap();
    let original_bytes = std::fs::read(target.join("metadata.json")).unwrap();
    let metadata = fixture.library.load_metadata(&target).unwrap().unwrap();
    for clear in [false, true] {
        let mut forged = metadata.clone();
        forged.import_state = Some(crate::models::ImportState::Ready);
        forged.validation_state = Some(crate::models::AssetValidationState::Valid);
        if clear {
            forged.import_publication = None;
        } else {
            forged.import_publication.as_mut().unwrap().confirmed = true;
        }
        assert!(fixture
            .library
            .save_metadata(&target, &forged)
            .await
            .unwrap_err()
            .to_string()
            .contains("finalization is incomplete"));
        assert!(fixture
            .library
            .upsert_index_from_metadata(&target, &forged)
            .is_err());
        let patch = serde_json::json!({
            "import_state":"ready", "validation_state":"valid", "import_publication": forged.import_publication,
        });
        assert!(fixture
            .library
            .submit_model_review(&id, patch.clone(), "test reviewer", None)
            .await
            .is_err());
        // Even lower-level stored overlays are observations, never producer proof.
        fixture
            .library
            .index()
            .apply_metadata_overlay(&id, &uuid::Uuid::new_v4().to_string(), &patch, "test", None)
            .unwrap();
        assert!(!fixture
            .library
            .get_effective_metadata(&id)
            .unwrap()
            .unwrap()
            .copied_import_ready());
        assert!(fixture
            .library
            .resolve_model_execution_descriptor(&id)
            .await
            .is_err());
    }
    assert!(fixture
        .library
        .resolve_model_package_facts(&id)
        .await
        .is_err());
    assert!(fixture
        .library
        .resolve_model_package_facts_summary(&id)
        .await
        .is_err());
    let spec = InPlaceImportSpec {
        model_dir: target.clone(),
        official_name: fixture.spec.official_name.clone(),
        family: fixture.spec.family.clone(),
        model_type: Some("vision".into()),
        repo_id: None,
        download_request: None,
        known_sha256: None,
        compute_hashes: false,
        expected_files: None,
        pipeline_tag: None,
        huggingface_evidence: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    };
    assert!(fixture
        .importer
        .import_in_place(&spec)
        .await
        .unwrap_err()
        .to_string()
        .contains("unconfirmed"));
    assert_eq!(fixture.importer.adopt_orphans(false).await.adopted, 0);
    assert_eq!(
        std::fs::read(target.join("metadata.json")).unwrap(),
        original_bytes
    );
    assert_eq!(receipt_at(&target)["state"], "pending");
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}
#[tokio::test]
async fn copied_import_unacknowledged_ready_restart_observations_respect_index_fence() {
    for outcome in ["old_pending", "new_ready", "changed_payload"] {
        let mut fixture = Fixture::new().await;
        let pending_bytes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let saved = pending_bytes.clone();
        fixture.importer.import_hook = Some(Arc::new(move |boundary, target| {
            if boundary == ImportBoundary::BeforeReady {
                *saved.lock().unwrap() =
                    std::fs::read(target.display_path().join("metadata.json"))?;
                target.inject_import_document_uncertainty("metadata.json");
            }
            Ok(())
        }));
        assert!(fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap_err()
            .to_string()
            .contains("Ready metadata finalization"));
        let target = fixture.target();
        let id = fixture.library.get_model_id(&target).unwrap();
        if outcome == "old_pending" {
            // Both old Pending and new Ready are legitimate crash observations
            // after a rename whose directory durability was not acknowledged.
            std::fs::write(
                target.join("metadata.json"),
                &*pending_bytes.lock().unwrap(),
            )
            .unwrap();
        }
        fixture.library.index_model_dir(&target).await.unwrap();
        fixture.library.rebuild_index().await.unwrap();
        let visible = fixture.library.load_metadata(&target).unwrap().unwrap();
        assert!(fixture
            .library
            .upsert_index_from_metadata(&target, &visible)
            .is_err());
        assert!(!crate::models::copied_import_ready_value(
            &fixture.library.index().get(&id).unwrap().unwrap().metadata
        ));
        assert!(fixture
            .library
            .resolve_model_execution_descriptor(&id)
            .await
            .is_err());
        // A cold index may discover an already Ready document only after the
        // receipt and entire physical payload have been independently checked.
        fixture.library.index().delete(&id).unwrap();
        if outcome == "changed_payload" {
            std::fs::write(target.join("unknown.payload"), b"unknown after restart").unwrap();
        }
        fixture.library.rebuild_index().await.unwrap();
        let ready = fixture
            .library
            .index()
            .get(&id)
            .unwrap()
            .is_some_and(|record| crate::models::copied_import_ready_value(&record.metadata));
        assert_eq!(ready, outcome == "new_ready");
        assert_eq!(
            fixture
                .library
                .resolve_model_execution_descriptor(&id)
                .await
                .is_ok(),
            outcome == "new_ready"
        );
        assert!(fixture.tasks.shutdown_owned().await.is_err());
    }
}

#[tokio::test]
async fn copied_import_receipt_survives_missing_malformed_or_erased_metadata() {
    for damage in ["missing", "malformed", "erased_identity"] {
        let fixture = Fixture::new().await;
        assert!(
            fixture
                .importer
                .import(&fixture.spec)
                .await
                .unwrap()
                .success
        );
        let target = fixture.target();
        let id = fixture.library.get_model_id(&target).unwrap();
        prime_ready_publication_summary(&fixture, &id, &target);
        let reader_before =
            crate::model_library::PumasReadOnlyLibrary::open(fixture.library.library_root())
                .unwrap();
        assert!(reader_before
            .resolve_model_artifact_load_target(publication_artifact_request(
                &id,
                &target,
                crate::models::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed
            ))
            .unwrap()
            .is_ready());
        let metadata_path = target.join("metadata.json");
        // A valid historical backup cannot erase the new protocol or restore
        // Ready when canonical metadata is absent/damaged.
        std::fs::write(
            target.join(IMPORT_METADATA_BACKUP),
            std::fs::read(&metadata_path).unwrap(),
        )
        .unwrap();
        match damage {
            "missing" => std::fs::remove_file(&metadata_path).unwrap(),
            "malformed" => std::fs::write(&metadata_path, b"not JSON").unwrap(),
            _ => {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
                value.as_object_mut().unwrap().remove("import_publication");
                std::fs::write(&metadata_path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
        }
        assert!(!fixture
            .library
            .get_effective_metadata(&id)
            .unwrap()
            .unwrap()
            .copied_import_ready());
        let reader =
            crate::model_library::PumasReadOnlyLibrary::open(fixture.library.library_root())
                .unwrap();
        for mode in [
            crate::models::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
            crate::models::PumasArtifactLoadTargetResolutionMode::OwnerFresh,
        ] {
            let request = publication_artifact_request(&id, &target, mode);
            assert!(!fixture
                .library
                .resolve_model_artifact_load_target(request.clone())
                .await
                .unwrap()
                .is_ready());
            if mode == crate::models::PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed {
                assert!(!reader
                    .resolve_model_artifact_load_target(request)
                    .unwrap()
                    .is_ready());
            }
        }
        let snapshot = reader
            .model_library_selector_snapshot(Default::default())
            .unwrap();
        assert!(!snapshot.rows[0].is_executable_reference_ready());
        assert!(snapshot.rows[0].package_facts_summary.is_none());
        let snapshot = fixture
            .library
            .model_library_selector_snapshot(Default::default())
            .await
            .unwrap();
        assert!(!snapshot.rows[0].is_executable_reference_ready());
        let summaries = fixture
            .library
            .model_package_facts_summary_snapshot(10, 0)
            .await
            .unwrap();
        assert_eq!(
            summaries.items[0].status,
            crate::models::ModelPackageFactsSummaryStatus::Invalid
        );
        assert!(
            crate::models::copied_import_ready_value(
                &fixture.library.index().get(&id).unwrap().unwrap().metadata
            ),
            "bounded public reads did not need reindexing"
        );
        assert!(!crate::models::copied_import_ready_value(
            &fixture
                .library
                .get_model(&id)
                .await
                .unwrap()
                .unwrap()
                .metadata
        ));
        let bytes_before = std::fs::read(&metadata_path).ok();
        assert!(!fixture
            .library
            .load_metadata(&target)
            .unwrap()
            .unwrap()
            .copied_import_ready());
        assert!(!fixture.importer.has_orphan_candidates());
        assert_eq!(fixture.importer.adopt_orphans(false).await.adopted, 0);
        let spec = InPlaceImportSpec {
            model_dir: target.clone(),
            official_name: "Do not re-adopt".into(),
            family: "fixture".into(),
            model_type: Some("vision".into()),
            repo_id: None,
            download_request: None,
            known_sha256: None,
            compute_hashes: false,
            expected_files: None,
            pipeline_tag: None,
            huggingface_evidence: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };
        assert!(fixture.importer.import_in_place(&spec).await.is_err());
        assert!(fixture
            .library
            .save_metadata(&target, &ModelMetadata::default())
            .await
            .is_err());
        fixture.library.rebuild_index().await.unwrap();
        assert!(!crate::models::copied_import_ready_value(
            &fixture.library.index().get(&id).unwrap().unwrap().metadata
        ));
        assert!(fixture
            .library
            .resolve_model_execution_descriptor(&id)
            .await
            .is_err());
        assert_eq!(std::fs::read(&metadata_path).ok(), bytes_before);
        assert_eq!(receipt_at(&target)["state"], "confirmed");
        fixture.tasks.shutdown_owned().await.unwrap();
    }
}

#[tokio::test]
async fn copied_import_ready_observation_requires_readable_matching_bounded_receipt() {
    for damage in ["missing", "malformed", "mismatched", "oversized"] {
        let fixture = Fixture::new().await;
        assert!(
            fixture
                .importer
                .import(&fixture.spec)
                .await
                .unwrap()
                .success
        );
        let target = fixture.target();
        let receipt = target.join(RECEIPT_FILENAME);
        match damage {
            "missing" => std::fs::remove_file(&receipt).unwrap(),
            "malformed" => std::fs::write(&receipt, b"not JSON").unwrap(),
            "oversized" => std::fs::File::create(&receipt)
                .unwrap()
                .set_len(16 * 1024 * 1024 + 1)
                .unwrap(),
            _ => {
                let mut value = receipt_at(&target);
                value["id"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
                std::fs::write(&receipt, serde_json::to_vec(&value).unwrap()).unwrap();
            }
        }
        assert!(!fixture
            .library
            .load_metadata(&target)
            .unwrap()
            .unwrap()
            .copied_import_ready());
        fixture.library.index_model_dir(&target).await.unwrap();
        let id = fixture.library.get_model_id(&target).unwrap();
        assert!(fixture
            .library
            .resolve_model_execution_descriptor(&id)
            .await
            .is_err());
        fixture.tasks.shutdown_owned().await.unwrap();
    }
}
#[tokio::test]
async fn copied_import_ordinary_metadata_edit_preserves_receipt_and_cold_readiness() {
    let fixture = Fixture::new().await;
    assert!(
        fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    let target = fixture.target();
    let id = fixture.library.get_model_id(&target).unwrap();
    let before_receipt = std::fs::read(target.join(RECEIPT_FILENAME)).unwrap();
    let mut metadata = fixture.library.load_metadata(&target).unwrap().unwrap();
    let mut forged = metadata.clone();
    forged.import_publication = None;
    assert!(fixture
        .library
        .save_metadata(&target, &forged)
        .await
        .is_err());
    metadata.notes = Some("Operator note".into());
    fixture
        .library
        .save_metadata(&target, &metadata)
        .await
        .unwrap();
    fixture.library.index_model_dir(&target).await.unwrap();
    assert!(target.join("metadata.json.bak").is_file());
    assert_eq!(
        std::fs::read(target.join(RECEIPT_FILENAME)).unwrap(),
        before_receipt
    );
    fixture.library.index().delete(&id).unwrap();
    fixture.library.rebuild_index().await.unwrap();
    assert!(fixture
        .library
        .resolve_model_execution_descriptor(&id)
        .await
        .is_ok());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn copied_import_receipt_observation_does_not_follow_replacement_symlink() {
    let fixture = Fixture::new().await;
    assert!(
        fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    let target = fixture.target();
    let outside = fixture.temp.path().join("outside-receipt");
    std::fs::rename(target.join(RECEIPT_FILENAME), &outside).unwrap();
    let before = std::fs::read(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, target.join(RECEIPT_FILENAME)).unwrap();
    assert!(!fixture
        .library
        .load_metadata(&target)
        .unwrap()
        .unwrap()
        .copied_import_ready());
    assert_eq!(std::fs::read(&outside).unwrap(), before);
    fixture.tasks.shutdown_owned().await.unwrap();
}
#[tokio::test]
async fn copied_import_reserved_backup_source_and_native_alias_collisions_are_refused() {
    let fixture = Fixture::new().await;
    std::fs::write(
        Path::new(&fixture.spec.path).join(IMPORT_METADATA_BACKUP),
        b"source backup payload",
    )
    .unwrap();
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(!result.success);
    assert!(fixture.stages().is_empty());
    assert_eq!(
        std::fs::read(Path::new(&fixture.spec.path).join(IMPORT_METADATA_BACKUP)).unwrap(),
        b"source backup payload"
    );
    fixture.tasks.shutdown_owned().await.unwrap();

    let fixture = Fixture::new().await;
    let root = DownloadDestinationRoot::open(fixture.library.library_root()).unwrap();
    let _grant = root.try_acquire_execution_grant().unwrap();
    let stage = root.create_import_stage().unwrap();
    let reservation = stage.create_import_file(IMPORT_METADATA_BACKUP).unwrap();
    let alias = "METADATA.JSON.BAK";
    let equivalent = stage.open_import_file(alias).is_ok();
    eprintln!("native reserved backup alias equivalence: {equivalent}");
    let source = Path::new(&fixture.spec.path);
    std::fs::write(source.join("a"), b"first").unwrap();
    std::fs::write(source.join("b"), b"backup collision").unwrap();
    let plan = CopyPlan {
        source: crate::platform::capability_fs::open_directory(source).unwrap(),
        directories: vec![],
        files: vec![
            (PathBuf::from("a"), "a".into(), "first.onnx".into()),
            (PathBuf::from("b"), "b".into(), alias.into()),
        ],
    };
    let copied = plan.copy_to(&stage, &fixture.importer);
    assert_eq!(copied.is_err(), equivalent);
    assert_eq!(
        std::fs::read(stage.display_path().join("first.onnx")).unwrap(),
        b"first"
    );
    assert_eq!(
        std::fs::read(stage.display_path().join(IMPORT_METADATA_BACKUP)).unwrap(),
        b""
    );
    stage
        .finish_import_document_reservation(IMPORT_METADATA_BACKUP, reservation)
        .unwrap();
    stage.remove_import_stage_all().unwrap();
    assert_eq!(
        std::fs::read(source.join("b")).unwrap(),
        b"backup collision"
    );
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_hashes_destination_once_after_every_callback() {
    let mut fixture = Fixture::new().await;
    let observed = Arc::new(std::sync::Mutex::new(None));
    let capture = observed.clone();
    fixture.importer.import_hook = Some(Arc::new(move |_, destination| {
        assert_eq!(destination.import_hash_pass_count(), 0);
        *capture.lock().unwrap() = Some(destination.clone());
        Ok(())
    }));
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(result.success);
    assert_eq!(
        observed
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .import_hash_pass_count(),
        1
    );
    fixture.tasks.shutdown_owned().await.unwrap();
}

fn publication_artifact_request(
    id: &str,
    target: &Path,
    mode: crate::models::PumasArtifactLoadTargetResolutionMode,
) -> crate::models::ResolveModelArtifactLoadTargetRequest {
    crate::models::ResolveModelArtifactLoadTargetRequest {
        model_ref: crate::models::PumasModelRef {
            model_ref_contract_version: crate::models::PUMAS_MODEL_REF_CONTRACT_VERSION,
            model_id: id.into(),
            revision: None,
            selected_artifact_id: Some("model.onnx".into()),
            selected_artifact_path: Some(target.join("model.onnx").display().to_string()),
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

fn publication_in_place_spec(directory: PathBuf) -> InPlaceImportSpec {
    InPlaceImportSpec {
        model_dir: directory,
        family: "fixture".into(),
        official_name: "Owned Import".into(),
        model_type: Some("vision".into()),
        repo_id: None,
        download_request: None,
        known_sha256: None,
        compute_hashes: false,
        expected_files: None,
        pipeline_tag: None,
        huggingface_evidence: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    }
}

#[tokio::test]
async fn copied_import_stale_watcher_projection_preserves_producer_ready() {
    let mut fixture = Fixture::new().await;
    let (published_tx, published_rx) = tokio::sync::oneshot::channel();
    let published_tx = std::sync::Mutex::new(Some(published_tx));
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let resume_rx = std::sync::Mutex::new(resume_rx);
    fixture.importer.import_hook = Some(Arc::new(move |boundary, _| {
        if boundary == ImportBoundary::Published {
            let _ = published_tx.lock().unwrap().take().unwrap().send(());
            resume_rx.lock().unwrap().recv().unwrap();
        }
        Ok(())
    }));
    let importer = fixture.importer.clone();
    let spec = fixture.spec.clone();
    let producer = tokio::spawn(async move { importer.import(&spec).await });
    published_rx.await.unwrap();
    let (prepared_tx, prepared_rx) = tokio::sync::oneshot::channel();
    let (commit_tx, commit_rx) = tokio::sync::oneshot::channel();
    let library = fixture.library.clone();
    let target = fixture.target();
    let watcher = tokio::spawn(async move {
        library
            .index_model_dir_paused_projection(&target, prepared_tx, commit_rx)
            .await
    });
    prepared_rx.await.unwrap();
    resume_tx.send(()).unwrap();
    assert!(producer.await.unwrap().unwrap().success);
    commit_tx.send(()).unwrap();
    watcher.await.unwrap().unwrap();
    let id = fixture.library.get_model_id(&fixture.target()).unwrap();
    assert!(crate::models::copied_import_ready_value(
        &fixture.library.index().get(&id).unwrap().unwrap().metadata
    ));
    fixture.library.rebuild_index().await.unwrap();
    assert!(fixture
        .library
        .get_effective_metadata(&id)
        .unwrap()
        .unwrap()
        .copied_import_ready());
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_deep_rebuild_and_in_place_retry_preserve_pending_fence() {
    let mut fixture = Fixture::new().await;
    fixture.importer.import_hook = Some(Arc::new(|boundary, target| {
        if boundary == ImportBoundary::BeforeReady {
            target.inject_import_document_uncertainty("metadata.json");
        }
        Ok(())
    }));
    assert!(fixture.importer.import(&fixture.spec).await.is_err());
    let target = fixture.target();
    let id = fixture.library.get_model_id(&target).unwrap();
    let bytes = std::fs::read(target.join("metadata.json")).unwrap();
    assert!(serde_json::from_slice::<ModelMetadata>(&bytes)
        .unwrap()
        .copied_import_ready());
    assert!(fixture
        .importer
        .import_in_place(&publication_in_place_spec(target.clone()))
        .await
        .is_err());
    fixture
        .library
        .deep_scan_rebuild(
            false,
            None::<fn(crate::model_library::library::DeepScanProgress)>,
        )
        .await
        .unwrap();
    assert!(!crate::models::copied_import_ready_value(
        &fixture.library.index().get(&id).unwrap().unwrap().metadata
    ));
    assert!(fixture
        .library
        .save_overrides(&target, &Default::default())
        .await
        .is_err());
    assert!(!target.join("overrides.json").exists());
    assert_eq!(std::fs::read(target.join("metadata.json")).unwrap(), bytes);
    assert!(fixture.tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_ready_overrides_and_transient_documents_preserve_cold_contract() {
    let fixture = Fixture::new().await;
    assert!(
        fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    let target = fixture.target();
    let id = fixture.library.get_model_id(&target).unwrap();
    fixture
        .library
        .save_overrides(
            &target,
            &crate::models::ModelOverrides {
                version_ranges: Some(std::collections::HashMap::from([(
                    "app".into(),
                    ">=1".into(),
                )])),
            },
        )
        .await
        .unwrap();
    assert!(!target.join("overrides.json.bak").exists());
    // Matching Ready observations do not scan transient writer namespace.
    let transient = target.join("overrides.json.writer.tmp");
    std::fs::write(&transient, b"in progress").unwrap();
    fixture
        .library
        .deep_scan_rebuild(
            false,
            None::<fn(crate::model_library::library::DeepScanProgress)>,
        )
        .await
        .unwrap();
    assert!(fixture
        .library
        .get_effective_metadata(&id)
        .unwrap()
        .unwrap()
        .copied_import_ready());
    fixture.library.index().delete(&id).unwrap();
    assert!(fixture
        .library
        .save_overrides(&target, &Default::default())
        .await
        .is_err());
    fixture.library.rebuild_index().await.unwrap();
    assert!(
        fixture.library.index().get(&id).unwrap().is_none(),
        "unknown temporary-looking file is not silently excluded or a permanent Pending row"
    );
    std::fs::remove_file(transient).unwrap();
    fixture.library.rebuild_index().await.unwrap();
    assert!(fixture
        .library
        .get_effective_metadata(&id)
        .unwrap()
        .unwrap()
        .copied_import_ready());
    assert_eq!(
        fixture
            .library
            .load_overrides(&target)
            .unwrap()
            .unwrap()
            .version_ranges
            .unwrap()["app"],
        ">=1"
    );
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_oversized_metadata_refuses_before_publication() {
    let mut fixture = Fixture::new().await;
    fixture.spec.official_name =
        "x".repeat(crate::model_library::download_recovery::IMPORT_DOCUMENT_MAX_BYTES as usize + 1);
    let result = fixture.importer.import(&fixture.spec).await.unwrap();
    assert!(!result.success);
    assert!(fixture.stages().is_empty());
    assert_eq!(fixture.library.model_count().unwrap(), 0);
    fixture.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_reserved_original_and_normalized_documents_refuse_before_stage() {
    for name in [
        "metadata.json",
        "metadata.json.bak",
        "overrides.json",
        RECEIPT_FILENAME,
        "metadata_json.bak",
        "pumas_import_publication.json",
    ] {
        let fixture = Fixture::new().await;
        let source = Path::new(&fixture.spec.path).join(name);
        std::fs::write(&source, b"preserve source document").unwrap();
        let result = fixture.importer.import(&fixture.spec).await.unwrap();
        assert!(!result.success, "{name}");
        assert!(fixture.stages().is_empty());
        assert_eq!(std::fs::read(&source).unwrap(), b"preserve source document");
        fixture.tasks.shutdown_owned().await.unwrap();
    }
}

#[tokio::test]
async fn copied_import_cross_root_merge_refuses_before_source_effects() {
    let source = Fixture::new().await;
    let destination = Fixture::new().await;
    assert!(source.importer.import(&source.spec).await.unwrap().success);
    let receipt = std::fs::read(source.target().join(RECEIPT_FILENAME)).unwrap();
    let metadata = std::fs::read(source.target().join("metadata.json")).unwrap();
    let result = crate::model_library::LibraryMerger::new(destination.library.clone())
        .merge_from_library(source.library.clone())
        .await
        .unwrap();
    assert_eq!(result.moved, 0);
    assert!(result
        .errors
        .iter()
        .any(|error| error.contains("Cross-root merge")));
    assert_eq!(
        std::fs::read(source.target().join(RECEIPT_FILENAME)).unwrap(),
        receipt
    );
    assert_eq!(
        std::fs::read(source.target().join("metadata.json")).unwrap(),
        metadata
    );
    assert_eq!(source.library.model_count().unwrap(), 1);
    assert_eq!(destination.library.model_count().unwrap(), 0);
    source.tasks.shutdown_owned().await.unwrap();
    destination.tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_same_root_move_preserves_receipt_physical_identity() {
    let fixture = Fixture::new().await;
    assert!(
        fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    let old_path = fixture.target();
    let old_id = fixture.library.get_model_id(&old_path).unwrap();
    let mut record = fixture.library.index().get(&old_id).unwrap().unwrap();
    let authority = fixture.library.mutation_authority().unwrap();
    let _grant = authority.root().try_acquire_execution_grant().unwrap();
    let new_id = "vision/fixture/renamed";
    let source = authority.root().resolve(&old_path).unwrap();
    let target = authority.root().resolve(Path::new(new_id)).unwrap();
    let mut metadata = source.read_model_metadata().unwrap().unwrap();
    source.rename_model_directory_noreplace(&target).unwrap();
    metadata.model_id = Some(new_id.into());
    target.write_model_metadata(&metadata).unwrap();
    record.id = new_id.into();
    record.path = target.display_path().display().to_string();
    record.metadata = serde_json::to_value(&metadata).unwrap();
    fixture
        .library
        .index()
        .replace_model_id_preserving_references(&old_id, &record)
        .unwrap();
    assert_eq!(receipt_at(target.display_path())["model_id"], old_id);
    assert!(fixture
        .library
        .get_effective_metadata(new_id)
        .unwrap()
        .unwrap()
        .copied_import_ready());
    assert!(!old_path.exists());
    drop(_grant);
    fixture.tasks.shutdown_owned().await.unwrap();
}

fn prime_ready_publication_summary(fixture: &Fixture, id: &str, target: &Path) {
    use crate::models::*;
    let request = publication_artifact_request(
        id,
        target,
        PumasArtifactLoadTargetResolutionMode::ReadOnlyIndexed,
    );
    let summary = ResolvedModelPackageFactsSummary {
        package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION,
        model_ref: request.model_ref,
        artifact_kind: PackageArtifactKind::Onnx,
        entry_path: target.join("model.onnx").display().to_string(),
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
    for selected in ["", "model.onnx"] {
        fixture
            .library
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

#[tokio::test]
async fn copied_import_public_reclassification_conditionally_moves_ready_generation() {
    let fixture = Fixture::new().await;
    assert!(
        fixture
            .importer
            .import(&fixture.spec)
            .await
            .unwrap()
            .success
    );
    let old_path = fixture.target();
    let old_id = fixture.library.get_model_id(&old_path).unwrap();
    let mut metadata = fixture.library.load_metadata(&old_path).unwrap().unwrap();
    metadata.family = Some("relocated".into());
    fixture
        .library
        .save_metadata(&old_path, &metadata)
        .await
        .unwrap();
    let weak = Arc::downgrade(&fixture.library);
    let old = old_id.clone();
    let observed = Arc::new(AtomicUsize::new(0));
    let calls = observed.clone();
    fixture
        .library
        .set_metadata_write_notifier(Some(Arc::new(move |path| {
            let library = weak.upgrade().unwrap();
            let target = path.parent().unwrap();
            if library.get_model_id(target).as_deref() == Some(old.as_str()) {
                return;
            }
            // The moved Ready document cannot be cold-adopted while another model
            // ID still owns its publication generation.
            assert!(!library
                .load_metadata(target)
                .unwrap()
                .unwrap()
                .copied_import_ready());
            let expected = library.index().get(&old).unwrap().unwrap();
            let mut stale_damage = expected.clone();
            stale_damage.metadata["import_state"] = serde_json::json!("pending");
            stale_damage.metadata["validation_state"] = serde_json::json!("invalid");
            assert_eq!(
                library
                    .index()
                    .upsert_projection_if_unchanged(&stale_damage, Some(&expected))
                    .unwrap(),
                crate::index::ProjectionCommit::Conflict
            );
            calls.fetch_add(1, Ordering::SeqCst);
        })));
    let new_id = fixture
        .library
        .reclassify_model(&old_id)
        .await
        .unwrap()
        .expect("family changes the canonical directory");
    fixture.library.set_metadata_write_notifier(None);
    assert_eq!(observed.load(Ordering::SeqCst), 1);
    assert_ne!(new_id, old_id);
    assert!(!old_path.exists());
    assert!(fixture.library.index().get(&old_id).unwrap().is_none());
    let target = fixture.library.library_root().join(&new_id);
    assert_eq!(receipt_at(&target)["model_id"], old_id);
    assert!(fixture
        .library
        .get_effective_metadata(&new_id)
        .unwrap()
        .unwrap()
        .copied_import_ready());
    assert!(fixture
        .library
        .resolve_model_execution_descriptor(&new_id)
        .await
        .is_ok());
    assert!(fixture
        .library
        .index()
        .list_intent_model_deletion_claims()
        .unwrap()
        .is_empty());
    fixture.tasks.shutdown_owned().await.unwrap();
}
