//! Cross-owner startup fixtures. These never migrate an existing store.
use super::*;
use crate::model_library::{DownloadPersistence, ModelImporter};
use std::path::Path;

#[tokio::test]
async fn acquisition_integration_startup_refuses_legacy_store_without_rewriting_it() {
    for version in [4, 5, 6] {
        let root = TempDir::new().unwrap();
        let _registry = RegistryTestGuard::new(root.path());
        let data_dir = root.path().join("launcher-data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let document = if version == 6 {
            serde_json::json!({"schema_version": 6, "acquisitions": {}, "consumer_receipts": {}})
        } else {
            serde_json::json!({
                "schema_version": version, "downloads": [], "recovery_revocations": {},
                "lifecycle_quarantines": {}, "admission_attempts": {}, "queue_admissions": {},
                "released_queue_admissions": {}
            })
        };
        let before = serde_json::to_vec_pretty(&document).unwrap();
        let store_path = data_dir.join("downloads.json");
        std::fs::write(&store_path, &before).unwrap();
        let result = PumasApi::builder(root.path())
            .auto_create_dirs(true)
            .with_process_manager(false)
            .build()
            .await;
        assert!(
            matches!(result, Err(PumasError::Validation { ref field, .. })
            if field == "acquisition.migration_required"),
            "schema {version} must refuse acquisition startup"
        );
        assert_eq!(std::fs::read(&store_path).unwrap(), before);
        let registry = registry::LibraryRegistry::open().unwrap();
        let retained_claim = registry.get_instance(root.path()).unwrap().unwrap();
        assert_eq!(retained_claim.status, registry::InstanceStatus::Claiming);
        let retained_claim = serde_json::to_value(retained_claim).unwrap();
        let retry = PumasApi::builder(root.path())
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert!(matches!(
            retry,
            Err(PumasError::InvalidParams { message })
                if message == format!(
                    "Pumas library instance is already running for physical store {}. Drop existing owner handles before constructing another owner.",
                    root.path().canonicalize().unwrap().display()
                )
        ));
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        assert!(matches!(retry, Err(PumasError::InvalidParams { message })
            if message.contains("PumasLocalClient")));
        assert_eq!(
            serde_json::to_value(registry.get_instance(root.path()).unwrap().unwrap()).unwrap(),
            retained_claim
        );
        let store = DownloadPersistence::new(&data_dir);
        if version != 6 {
            assert!(store
                .load_lifecycle_inventory_strict()
                .unwrap()
                .downloads
                .is_empty());
            // A failed builder retains its claim. Check the independent
            // HF-disabled compatibility path on its own authored store.
            let independent = TempDir::new().unwrap();
            let independent_data = independent.path().join("launcher-data");
            std::fs::create_dir_all(&independent_data).unwrap();
            let independent_store = independent_data.join("downloads.json");
            std::fs::write(&independent_store, &before).unwrap();
            let api = PumasApi::builder(independent.path())
                .auto_create_dirs(true)
                .with_hf_client(false)
                .with_process_manager(false)
                .build()
                .await
                .unwrap();
            assert!(api.list_models().await.unwrap().is_empty());
            api.shutdown_instance().await.unwrap();
            drop(api);
            assert_eq!(std::fs::read(independent_store).unwrap(), before);
        }
        assert!(
            matches!(store.acquisition_store().require_acquisition_schema(),
            Err(PumasError::Validation { ref field, .. }) if field == "acquisition.migration_required")
        );
        assert_eq!(std::fs::read(&store_path).unwrap(), before);
    }
}

pub(super) async fn assert_startup_retained_evidence(
    library: &Arc<ModelLibrary>,
    copied_pending: &Path,
    metadata: &[u8],
    shards: &Path,
) {
    assert_eq!(
        std::fs::read(copied_pending.join("metadata.json")).unwrap(),
        metadata
    );
    assert_eq!(
        std::fs::read(copied_pending.join("weights.gguf")).unwrap(),
        b"retained copied payload"
    );
    assert!(!copied_pending
        .join(".pumas_import_publication.json")
        .exists());
    assert_eq!(
        std::fs::read(shards.join("weights-00001-of-00002.gguf")).unwrap(),
        b"retained shard"
    );
    assert!(!shards.join(".pumas_download").exists());
    assert!(!shards.join("weights-00002-of-00002.gguf").exists());
    let report = ModelImporter::new(library.clone())
        .discover_shard_recovery_async()
        .await;
    assert!(report.enumeration_complete);
    // Discovery reports the canonical root; macOS temporary roots may be
    // spelled through /var while their held location is under /private/var.
    let canonical_shards = shards.canonicalize().unwrap();
    assert!(report
        .model_roots
        .iter()
        .any(|model| model.model_dir == canonical_shards
            && model
                .shard_sets
                .iter()
                .any(|set| set.status
                    == crate::model_library::ShardSetDiscoveryStatus::MissingOrdinals)));
    for record in library.index().list_all().unwrap() {
        assert_ne!(
            record.id, "vision/idea-research/grounding-dino-base",
            "startup must not publish the failed HF import"
        );
        if record.id == "llm/local/copied-pending" {
            assert!(!crate::models::copied_import_ready_value(&record.metadata));
        }
    }
}
