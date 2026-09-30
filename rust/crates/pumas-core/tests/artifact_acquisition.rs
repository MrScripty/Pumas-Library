//! Public construction/migration regression; all roots are local temporary fixtures.
use pumas_library::{model_library::DownloadPersistence, PumasApi, PumasError};

#[tokio::test]
async fn ordinary_builder_leaves_legacy_state_read_only_until_explicit_offline_migration() {
    let root = tempfile::TempDir::new().unwrap();
    let data = root.path().join("launcher-data");
    std::fs::create_dir(&data).unwrap();
    let path = data.join("downloads.json");
    let legacy = serde_json::to_vec_pretty(&serde_json::json!({
        "schema_version": 5,
        "downloads": [],
        "recovery_revocations": {},
        "lifecycle_quarantines": {},
        "admission_attempts": {},
        "queue_admissions": {},
        "released_queue_admissions": {}
    }))
    .unwrap();
    std::fs::write(&path, &legacy).unwrap();
    let api = PumasApi::builder(root.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), legacy);
    assert!(
        matches!(api.acquisition().store().require_acquisition_schema(),
        Err(PumasError::Validation { field, .. }) if field == "acquisition.migration_required")
    );
    api.shutdown_intent().await.unwrap();
    api.shutdown_acquisition().await.unwrap();
    drop(api);
    // This fixture has no old readers/writers. Migration is a separate operator action.
    DownloadPersistence::migrate_legacy_offline(&data).unwrap();
    let current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(current["schema_version"], 6);
    assert_eq!(current["acquisitions"], serde_json::json!({}));
    assert!(DownloadPersistence::new(&data).load_all().is_empty());
    assert!(DownloadPersistence::migrate_legacy_offline(&data).is_err());
}
