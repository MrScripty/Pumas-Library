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
        assert_eq!(metadata.copied_import_ready(), ready_write);
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
        let ready = crate::models::copied_import_ready_value(
            &fixture.library.index().get(&id).unwrap().unwrap().metadata,
        );
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
