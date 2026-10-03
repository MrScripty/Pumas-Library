//! Focused regressions for publication-aware library maintenance and queries.
use super::*;
use crate::api::RuntimeTasks;
use crate::models::{ImportPublicationIdentity, ImportState};
use tempfile::TempDir;

async fn fixture() -> (TempDir, ModelLibrary, RuntimeTasks) {
    let temp = tempfile::tempdir().unwrap();
    let library = ModelLibrary::new(temp.path().join("library"))
        .await
        .unwrap();
    let tasks = RuntimeTasks::new();
    let downloads = temp.path().join("downloads");
    std::fs::create_dir(&downloads).unwrap();
    library
        .install_mutation_authority(
            tasks.clone(),
            crate::model_library::DownloadDestinationRoot::open(library.library_root()).unwrap(),
            Arc::new(crate::model_library::DownloadPersistence::new(&downloads)),
        )
        .unwrap();
    (temp, library, tasks)
}

fn pending_metadata() -> ModelMetadata {
    ModelMetadata {
        import_publication: Some(ImportPublicationIdentity {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            confirmed: false,
        }),
        import_state: Some(ImportState::Pending),
        validation_state: Some(AssetValidationState::Invalid),
        ..Default::default()
    }
}

fn insert_model(
    library: &ModelLibrary,
    id: &str,
    mut metadata: ModelMetadata,
    payload: bool,
) -> PathBuf {
    let path = library.library_root().join(id);
    std::fs::create_dir_all(&path).unwrap();
    let segments = id.split('/').collect::<Vec<_>>();
    metadata.model_id = Some(id.into());
    metadata.model_type = Some(segments[0].into());
    metadata.family = Some(segments[1].into());
    metadata.cleaned_name = Some(segments[2].into());
    metadata.official_name = Some(segments[2].into());
    std::fs::write(
        path.join(METADATA_FILENAME),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    if metadata.import_publication.is_some() {
        std::fs::write(
            path.join(super::super::importer::publication::RECEIPT_FILENAME),
            b"{\"state\":\"pending\"}",
        )
        .unwrap();
    }
    if payload {
        std::fs::write(path.join("model.onnx"), b"retained fixture payload").unwrap();
    }
    library
        .index
        .upsert(&metadata_to_record(id, &path, &metadata))
        .unwrap();
    path
}

#[tokio::test]
async fn copied_import_cleanup_skips_only_eligible_blocked_models() {
    let (_temp, library, tasks) = fixture().await;
    let metadata = ModelMetadata {
        repo_id: Some("example/duplicate".into()),
        ..Default::default()
    };
    let canonical = insert_model(&library, "llm/review/canonical", metadata.clone(), true);
    let duplicate = insert_model(&library, "unknown/review/stub", metadata, false);
    let mut pending = pending_metadata();
    pending.repo_id = Some("example/duplicate".into());
    let blocked = insert_model(&library, "llm/review/blocked", pending, true);
    let irrelevant = insert_model(&library, "llm/review/no-repo", pending_metadata(), true);
    let before = std::fs::read(blocked.join(METADATA_FILENAME)).unwrap();
    let report = library.cleanup_duplicate_repo_entries().unwrap();
    assert_eq!(report.removed_duplicate_dirs, 1);
    assert_eq!(report.blocked_models.len(), 1);
    assert_eq!(report.blocked_models[0].0, "llm/review/blocked");
    assert!(report.blocked_models[0].1.contains("import_publication"));
    assert!(report.blocked_models[0]
        .1
        .contains(blocked.to_str().unwrap()));
    assert!(canonical.join("model.onnx").is_file());
    assert!(!duplicate.exists());
    assert!(irrelevant.join("model.onnx").is_file());
    assert_eq!(
        std::fs::read(blocked.join(METADATA_FILENAME)).unwrap(),
        before
    );
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_cleanup_preserves_unexpected_metadata_errors() {
    let (_temp, library, tasks) = fixture().await;
    let path = insert_model(
        &library,
        "llm/review/broken",
        ModelMetadata::default(),
        false,
    );
    std::fs::write(path.join(METADATA_FILENAME), b"{broken").unwrap();
    assert!(matches!(
        library.cleanup_duplicate_repo_entries(),
        Err(PumasError::Json { .. })
    ));
    assert!(path.exists());
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_record_observation_preserves_raw_projection_fields() {
    let (_temp, library, tasks) = fixture().await;
    let id = "llm/review/projection";
    insert_model(&library, id, pending_metadata(), true);
    let mut record = library.index.get(id).unwrap().unwrap();
    record.metadata.as_object_mut().unwrap().remove("license");
    record.metadata["future_extension"] = serde_json::json!({"keep": [1, 2, 3]});
    record.metadata["size_bytes"] = serde_json::json!("unrecognized old projection");
    let original = record.metadata.clone();
    library.index.upsert(&record).unwrap();
    let mut observed = vec![record];
    library.observe_publication_records(&mut observed).unwrap();
    for (key, value) in original.as_object().unwrap() {
        if !["import_state", "validation_state", "validation_errors"].contains(&key.as_str()) {
            assert_eq!(observed[0].metadata.get(key), Some(value));
        }
    }
    assert!(observed[0].metadata.get("license").is_none());
    assert_eq!(observed[0].metadata["validation_state"], "invalid");
    assert_eq!(
        library.list_models().await.unwrap()[0].metadata["future_extension"],
        original["future_extension"]
    );
    assert_eq!(
        library
            .search_models("projection", 10, 0)
            .await
            .unwrap()
            .models[0]
            .metadata["future_extension"],
        original["future_extension"]
    );
    assert_eq!(
        library.get_model(id).await.unwrap().unwrap().metadata["future_extension"],
        original["future_extension"]
    );
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_malformed_index_identity_is_diagnostic_not_collection_failure() {
    let (_temp, library, tasks) = fixture().await;
    let id = "llm/review/malformed";
    insert_model(&library, id, pending_metadata(), false);
    let mut record = library.index.get(id).unwrap().unwrap();
    let malformed = serde_json::json!({"version": "invalid", "id": 7, "future": "retain"});
    record.metadata["import_publication"] = malformed.clone();
    record.metadata["import_state"] = serde_json::json!("ready");
    record.metadata["validation_state"] = serde_json::json!("valid");
    library.index.upsert(&record).unwrap();
    insert_model(
        &library,
        "llm/review/healthy",
        ModelMetadata::default(),
        false,
    );
    let models = library.list_models().await.unwrap();
    assert_eq!(models.len(), 2);
    let observed = models.iter().find(|record| record.id == id).unwrap();
    assert_eq!(observed.metadata["import_publication"], malformed);
    assert_eq!(observed.metadata["import_state"], "pending");
    assert_eq!(observed.metadata["validation_state"], "invalid");
    assert_eq!(
        observed.metadata["validation_errors"][0]["code"],
        "import_publication_index_invalid"
    );
    assert_eq!(
        library
            .search_models("malformed", 10, 0)
            .await
            .unwrap()
            .models[0]
            .metadata["import_publication"],
        malformed
    );
    assert_eq!(
        library.get_model(id).await.unwrap().unwrap().metadata["import_publication"],
        malformed
    );
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_record_observation_preserves_io_and_database_errors() {
    let (_temp, library, tasks) = fixture().await;
    let id = "llm/review/io-error";
    let path = insert_model(&library, id, pending_metadata(), false);
    std::fs::remove_file(path.join(METADATA_FILENAME)).unwrap();
    std::fs::create_dir(path.join(METADATA_FILENAME)).unwrap();
    let mut records = library.index.list_all().unwrap();
    assert!(matches!(
        library.observe_publication_records(&mut records),
        Err(PumasError::Io { .. })
    ));
    let connection = rusqlite::Connection::open(library.index.db_path()).unwrap();
    connection.execute_batch("DROP TABLE models").unwrap();
    assert!(matches!(
        library.observe_publication_records(&mut records),
        Err(PumasError::Database { .. })
    ));
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_rebuild_prunes_invalid_legacy_paths_and_retains_protocol_evidence() {
    let (temp, library, tasks) = fixture().await;
    let mut protocol = metadata_to_record(
        "llm/review/retained",
        &temp.path().join("outside"),
        &pending_metadata(),
    );
    protocol.metadata["future_extension"] = serde_json::json!("preserve");
    library.index.upsert(&protocol).unwrap();
    for (id, path) in [
        ("llm/review/stale-first", "../outside"),
        ("llm/review/stale-second", "missing/model"),
    ] {
        let record = metadata_to_record(id, Path::new(path), &ModelMetadata::default());
        library.index.upsert(&record).unwrap();
    }
    library.rebuild_index().await.unwrap();
    assert!(library
        .index
        .get("llm/review/stale-first")
        .unwrap()
        .is_none());
    assert!(library
        .index
        .get("llm/review/stale-second")
        .unwrap()
        .is_none());
    let retained = library.index.get(&protocol.id).unwrap().unwrap();
    assert_eq!(retained.path, protocol.path);
    assert_eq!(
        retained.metadata["import_publication"],
        protocol.metadata["import_publication"]
    );
    assert_eq!(retained.metadata["future_extension"], "preserve");
    assert_eq!(
        retained.metadata["validation_errors"][0]["code"],
        "import_publication_index_path_invalid"
    );
    let wal_path = PathBuf::from(format!("{}-wal", library.index.db_path().display()));
    assert_eq!(
        std::fs::metadata(wal_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0),
        0
    );
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_migration_resume_retains_source_and_conversion_blocks() {
    let (_temp, library, tasks) = fixture().await;
    let blocked_source = insert_model(
        &library,
        "llm/review/blocked-source",
        pending_metadata(),
        true,
    );
    let conversion_source = insert_model(
        &library,
        "llm/review/conversion-source",
        ModelMetadata::default(),
        true,
    );
    let mut converted = pending_metadata();
    converted.conversion_source = Some(crate::conversion::ConversionSource {
        source_model_id: "llm/review/conversion-source".into(),
        source_format: "onnx".into(),
        source_quant: None,
        target_format: "onnx".into(),
        target_quant: None,
        was_dequantized: false,
        conversion_date: "2026-10-03T00:00:00Z".into(),
    });
    let blocked_conversion = insert_model(&library, "llm/review/converted", converted, true);
    insert_model(
        &library,
        "llm/review/independent",
        ModelMetadata::default(),
        true,
    );
    insert_model(
        &library,
        "llm/review/previously-moved",
        ModelMetadata::default(),
        true,
    );
    let checkpoint_path = library.library_root.join(MIGRATION_CHECKPOINT_FILENAME);
    let checkpoint = MigrationCheckpointState {
        created_at: "2026-10-03T00:00:00Z".into(),
        updated_at: "2026-10-03T00:00:00Z".into(),
        pending_moves: ["blocked-source", "conversion-source", "independent"]
            .into_iter()
            .map(|name| {
                let id = format!("llm/review/{name}");
                let target = format!("llm/review/{name}-moved");
                MigrationPlannedMove {
                    current_path: library.library_root.join(&id).display().to_string(),
                    target_path: library.library_root.join(&target).display().to_string(),
                    model_id: id,
                    target_model_id: target,
                    action_kind: Some("move".into()),
                    ..Default::default()
                }
            })
            .collect(),
        completed_results: vec![MigrationExecutionItem {
            model_id: "llm/review/previous".into(),
            target_model_id: "llm/review/previously-moved".into(),
            action: "moved".into(),
            error: None,
        }],
    };
    save_migration_checkpoint(&checkpoint_path, &checkpoint).unwrap();
    let receipt_name = super::super::importer::publication::RECEIPT_FILENAME;
    let source_receipt = std::fs::read(blocked_source.join(receipt_name)).unwrap();
    let conversion_receipt = std::fs::read(blocked_conversion.join(receipt_name)).unwrap();
    for _ in 0..2 {
        let report = library.execute_migration_with_checkpoint().await.unwrap();
        assert!(report.resumed_from_checkpoint);
        assert_eq!(report.planned_move_count, 4);
        assert_eq!(report.completed_move_count, 2);
        assert_eq!(report.skipped_move_count, 2);
        assert!(report.error_count >= 2);
        assert!(report.completed_at.is_none());
        assert_eq!(
            report
                .results
                .iter()
                .filter(|item| item.action == "blocked_import_publication"
                    && item
                        .error
                        .as_ref()
                        .is_some_and(|error| error.contains("manual diagnosis")))
                .count(),
            2
        );
        let saved = load_migration_checkpoint(&checkpoint_path)
            .unwrap()
            .unwrap();
        assert!(saved.pending_moves.is_empty());
        assert_eq!(saved.completed_results.len(), 4);
        assert_eq!(
            saved
                .completed_results
                .iter()
                .filter(|item| item.action == "blocked_import_publication")
                .count(),
            2
        );
        assert_eq!(
            std::fs::read(blocked_source.join(receipt_name)).unwrap(),
            source_receipt
        );
        assert_eq!(
            std::fs::read(blocked_conversion.join(receipt_name)).unwrap(),
            conversion_receipt
        );
        assert!(conversion_source.join("model.onnx").is_file());
        assert!(!library
            .library_root
            .join("llm/review/blocked-source-moved")
            .exists());
        assert!(!library
            .library_root
            .join("llm/review/conversion-source-moved")
            .exists());
        assert!(!library.library_root.join("llm/review/independent").exists());
        assert!(library
            .library_root
            .join("llm/review/independent-moved/model.onnx")
            .is_file());
    }
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_migration_preserves_unexpected_error_and_checkpoint() {
    let (_temp, library, tasks) = fixture().await;
    let id = "llm/review/broken-source";
    let source = insert_model(&library, id, ModelMetadata::default(), true);
    std::fs::write(source.join(METADATA_FILENAME), b"{invalid json").unwrap();
    let target = library.library_root.join("llm/review/broken-source-moved");
    let checkpoint_path = library.library_root.join(MIGRATION_CHECKPOINT_FILENAME);
    let checkpoint = MigrationCheckpointState {
        created_at: "2026-10-03T00:00:00Z".into(),
        updated_at: "2026-10-03T00:00:00Z".into(),
        pending_moves: vec![MigrationPlannedMove {
            model_id: id.into(),
            target_model_id: "llm/review/broken-source-moved".into(),
            current_path: source.display().to_string(),
            target_path: target.display().to_string(),
            action_kind: Some("move".into()),
            ..Default::default()
        }],
        completed_results: vec![],
    };
    save_migration_checkpoint(&checkpoint_path, &checkpoint).unwrap();
    let before = std::fs::read(&checkpoint_path).unwrap();
    assert!(matches!(
        library.execute_migration_with_checkpoint().await,
        Err(PumasError::Json { .. })
    ));
    assert_eq!(std::fs::read(&checkpoint_path).unwrap(), before);
    assert!(source.join("model.onnx").is_file());
    assert!(!target.exists());
    assert!(tasks.shutdown_owned().await.is_err());
}

#[tokio::test]
async fn copied_import_receipt_only_malformed_gates_remain_unavailable_in_public_queries() {
    let (_temp, library, tasks) = fixture().await;
    let id = "llm/review/receipt-only";
    insert_model(&library, id, pending_metadata(), false);
    let mut record = library.index.get(id).unwrap().unwrap();
    record.metadata = serde_json::json!({
        "import_state": 17,
        "size_bytes": "old projection",
        "future_extension": ["keep"],
    });
    library.index.upsert(&record).unwrap();
    let listed = library.list_models().await.unwrap();
    let searched = library.search_models("receipt-only", 10, 0).await.unwrap();
    let fetched = library.get_model(id).await.unwrap().unwrap();
    for observed in [&listed[0], &searched.models[0], &fetched] {
        assert_eq!(observed.metadata["size_bytes"], "old projection");
        assert_eq!(
            observed.metadata["future_extension"],
            serde_json::json!(["keep"])
        );
        assert_eq!(observed.metadata["import_state"], "pending");
        assert_eq!(observed.metadata["validation_state"], "invalid");
        assert!(!crate::models::copied_import_ready_value(
            &observed.metadata
        ));
    }
    assert_eq!(
        library.index.get(id).unwrap().unwrap().metadata,
        record.metadata
    );
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_receipt_backed_nonobject_evidence_is_retained_in_diagnostics() {
    let (_temp, library, tasks) = fixture().await;
    for (number, raw) in [
        serde_json::json!("raw scalar"),
        serde_json::json!([1, {"keep": true}]),
        Value::Null,
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("llm/review/raw-{number}");
        insert_model(&library, &id, pending_metadata(), false);
        let mut record = library.index.get(&id).unwrap().unwrap();
        record.metadata = raw.clone();
        library.index.upsert(&record).unwrap();
        let listed = library.list_models().await.unwrap();
        let searched = library
            .search_models(&format!("raw-{number}"), 10, 0)
            .await
            .unwrap();
        let fetched = library.get_model(&id).await.unwrap().unwrap();
        for observed in [
            listed.iter().find(|row| row.id == id).unwrap(),
            &searched.models[0],
            &fetched,
        ] {
            assert_eq!(observed.metadata["unparsed_index_metadata"], raw);
            assert_eq!(observed.metadata["import_state"], "pending");
            assert!(!crate::models::copied_import_ready_value(
                &observed.metadata
            ));
        }
        assert_eq!(library.index.get(&id).unwrap().unwrap().metadata, raw);
    }
    tasks.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn copied_import_oversized_evidence_is_per_model_unavailable_in_public_queries() {
    let (_temp, library, tasks) = fixture().await;
    let oversized = super::super::download_recovery::IMPORT_DOCUMENT_MAX_BYTES + 1;
    for (number, filename) in [
        METADATA_FILENAME,
        super::super::importer::publication::RECEIPT_FILENAME,
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("llm/review/oversized-{number}");
        let mut metadata = pending_metadata();
        metadata.import_publication.as_mut().unwrap().confirmed = true;
        metadata.import_state = Some(ImportState::Ready);
        metadata.validation_state = Some(AssetValidationState::Valid);
        let path = insert_model(&library, &id, metadata, false);
        // Sparse synthetic evidence tests the actual bounded readers without
        // constructing a multi-megabyte parsed document or any model payload.
        std::fs::File::create(path.join(filename))
            .unwrap()
            .set_len(oversized)
            .unwrap();
        let listed = library.list_models().await.unwrap();
        let searched = library
            .search_models(&format!("oversized-{number}"), 10, 0)
            .await
            .unwrap();
        let fetched = library.get_model(&id).await.unwrap().unwrap();
        for observed in [
            listed.iter().find(|row| row.id == id).unwrap(),
            &searched.models[0],
            &fetched,
        ] {
            assert_eq!(observed.metadata["validation_state"], "invalid");
            assert!(!crate::models::copied_import_ready_value(
                &observed.metadata
            ));
        }
        assert_eq!(
            std::fs::metadata(path.join(filename)).unwrap().len(),
            oversized
        );
    }
    tasks.shutdown_owned().await.unwrap();
}
