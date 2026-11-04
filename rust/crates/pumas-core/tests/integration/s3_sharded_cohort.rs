//! Composed protocol qualification, driven by the independent Python HTTPS oracle.
//! This is a synthetic fixture, not acceptance of an S3-compatible service.
#![cfg(feature = "s3")]

use pumas_library::{
    acquisition::*,
    models::{ImportState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, PumasError, S3ModelImportControl, S3ModelImportError, S3ModelImportPhase,
    S3ModelImportRequest,
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

#[derive(Deserialize)]
struct Object {
    sha256: String,
    bytes: u64,
}

fn credentials() -> S3Credentials {
    S3Credentials::new(
        "cohort-fixture-access".into(),
        "cohort-fixture-secret".into(),
        Some("cohort-fixture-session".into()),
    )
    .unwrap()
}

fn config(endpoint: &str) -> S3ReaderConfig {
    S3ReaderConfig {
        endpoint: endpoint.into(),
        region: "fixture-region".into(),
        bucket: "fixture-bucket".into(),
        addressing: S3Addressing::Path,
        allow_http: false,
        operation_timeout: Duration::from_secs(15),
    }
}

async fn api(root: &Path) -> PumasApi {
    PumasApi::builder(root)
        .with_registry(
            pumas_library::registry::LibraryRegistry::open_at(&root.join("registry.db")).unwrap(),
        )
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}

fn assert_receipt(record: &AcquisitionRecord, receipt: &AcquisitionConsumerReceipt) {
    let lease = match record.phase {
        AcquisitionPhase::Using { lease } | AcquisitionPhase::Adopted { lease } => lease,
        _ => panic!("receipt lacks a bound use lease"),
    };
    assert_eq!(receipt.receipt_kind, "pumas.consumer-completion");
    assert_eq!(receipt.receipt_version, 1);
    assert_eq!(receipt.acquisition_id, record.id.to_string());
    assert_eq!(receipt.use_lease, lease.to_string());
    assert_eq!(receipt.owner, record.demand.consumer);
    assert_eq!(receipt.demand, record.demand);
    assert_eq!(receipt.workspace, record.workspace);
    assert_eq!(receipt.manifest, record.manifest);
    assert_eq!(receipt.verified_files, record.files);
}

#[tokio::test]
#[ignore = "run scripts/tests/qualify-s3-sharded-cohort.py for isolated HTTPS trust and signature oracle"]
async fn authenticated_prefix_resume_sharded_import_and_refusals() {
    let endpoint = std::env::var("PUMAS_S3_COHORT_ENDPOINT").unwrap();
    assert!(endpoint.starts_with("https://127.0.0.1:"));
    let fixture_root = std::env::var("PUMAS_S3_COHORT_OBJECTS").unwrap();
    let objects: BTreeMap<String, Object> = serde_json::from_slice(
        &std::fs::read(Path::new(&fixture_root).join("manifest.json")).unwrap(),
    )
    .unwrap();
    for mode in ["complete", "missing", "opaque", "cancel"] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(stage.path().join("stage")).unwrap();
        let api = api(root.path()).await;
        let prefix = format!("{mode}/");
        let reader = S3Reader::new_authenticated(config(&endpoint), credentials()).unwrap();
        let listing = reader
            .enumerate_prefix(
                &prefix,
                S3PrefixLimits {
                    page_size: 2,
                    max_pages: 4,
                    max_objects: 8,
                    max_page_bytes: 4096,
                    max_total_bytes: 16384,
                },
            )
            .await
            .unwrap();
        let expected: Vec<_> = objects
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .collect();
        assert_eq!(listing.objects().len(), expected.len());
        assert_eq!(listing.pages() as usize, expected.len().div_ceil(2));
        assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
        assert!(std::fs::read_dir(stage.path().join("stage"))
            .unwrap()
            .next()
            .is_none());
        let entries = listing
            .objects()
            .iter()
            .zip(expected)
            .map(|(object, key)| {
                assert_eq!(object.key(), key);
                assert_eq!(object.version(), "owned-v1");
                assert_eq!(object.size(), objects[key].bytes);
                object.manifest_entry(
                    key.strip_prefix(&prefix).unwrap().into(),
                    Sha256Evidence::new("owned-fixture.sha256", &objects[key].sha256).unwrap(),
                )
            })
            .collect();
        drop(reader);
        let control = S3ModelImportControl::new();
        let progress = control.subscribe();
        let mut bundle = control.subscribe_bundle();
        let request = S3ModelImportRequest {
            operation_id: uuid::Uuid::new_v4(),
            source: config(&endpoint),
            credentials: Some(credentials()),
            entries,
            import: ModelImportSpec {
                path: if mode == "opaque" {
                    "arbitrary.dat"
                } else {
                    "model-00001-of-00002.safetensors"
                }
                .into(),
                family: "fixture".into(),
                official_name: format!("Owned sharded {mode}"),
                model_type: Some("llm".into()),
                repo_id: None,
                subtype: None,
                tags: None,
                security_acknowledged: None,
            },
            workspace: AcquisitionWorkspace::from_reserved_directory(
                stage.path(),
                Path::new("stage"),
                Arc::new(()),
                || Ok(()),
            )
            .unwrap(),
            retry: AcquisitionRetryPolicy {
                attempts: Some(3),
                elapsed: Duration::from_secs(15),
                backoff: RetryConfig::default(),
            },
        };
        let operation_id = request.operation_id;
        let import_payload = serde_json::to_value(&request.import).unwrap();
        let mut work = Box::pin(api.import_s3_model(request, control.clone()));
        if mode == "cancel" {
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    tokio::select! {
                        result = &mut work => panic!("cancel fixture completed prematurely: {result:?}"),
                        changed = bundle.changed() => changed.unwrap(),
                    }
                    if bundle.borrow().file_index == Some(1)
                        && bundle.borrow().downloaded_for_current_file >= 4096
                    {
                        assert!(control.cancel());
                        break;
                    }
                }
            }).await.unwrap();
        }
        let result = tokio::time::timeout(Duration::from_secs(20), &mut work)
            .await
            .unwrap();
        drop(work);
        let record = api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        assert_eq!(record.manifest.source().provider(), "s3");
        assert_eq!(record.demand.consumer, "model.s3.workflow");
        assert_eq!(record.demand.operation, operation_id.to_string());
        for file in record.manifest.files() {
            let key = format!("{prefix}{}", file.logical_path());
            let pin: [String; 2] = serde_json::from_str(file.source_key()).unwrap();
            assert_eq!(pin, [key.clone(), "owned-v1".into()]);
            assert_eq!(
                file.expected_sha256().unwrap().value(),
                objects[&key].sha256
            );
        }
        if mode == "complete" {
            let result = result.unwrap();
            assert!(result.success);
            let id = result.model_id.unwrap();
            assert_eq!(progress.borrow().phase, S3ModelImportPhase::Completed);
            assert!(!control.cancel());
            assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
            let receipt = api
                .acquisition()
                .consumer_receipt(record.id)
                .unwrap()
                .unwrap();
            assert_receipt(&record, &receipt);
            assert_eq!(receipt.payload, import_payload);
            for file in &record.files {
                let key = format!("{prefix}{}", file.path);
                assert_eq!(file.sha256, objects[&key].sha256);
                assert_eq!(file.bytes, objects[&key].bytes);
                assert_eq!(
                    std::fs::read(
                        api.model_library()
                            .library_root()
                            .join(&id)
                            .join(&file.path)
                    )
                    .unwrap(),
                    std::fs::read(Path::new(&fixture_root).join(key)).unwrap()
                );
            }
            let metadata = api
                .model_library()
                .get_effective_metadata(&id)
                .unwrap()
                .unwrap();
            assert_eq!(metadata.model_id.as_deref(), Some(id.as_str()));
            assert_eq!(metadata.import_state, Some(ImportState::Ready));
            let identity = metadata.import_publication.as_ref().unwrap();
            assert_eq!(identity.version, 1);
            assert!(identity.confirmed);
            let publication_bytes = std::fs::read(
                api.model_library()
                    .library_root()
                    .join(&id)
                    .join(".pumas_import_publication.json"),
            )
            .unwrap();
            let publication: serde_json::Value =
                serde_json::from_slice(&publication_bytes).unwrap();
            assert_eq!(publication["version"], 2);
            assert_eq!(publication["state"], "confirmed");
            assert_eq!(publication["id"], identity.id);
            assert_eq!(publication["model_id"], id);
            assert_eq!(
                publication["acquisition"],
                serde_json::to_value(&receipt).unwrap()
            );
            for evidence in [
                serde_json::to_vec(&record).unwrap(),
                publication_bytes.clone(),
            ] {
                let evidence = String::from_utf8(evidence).unwrap();
                for secret in [
                    "cohort-fixture-access",
                    "cohort-fixture-secret",
                    "cohort-fixture-session",
                ] {
                    assert!(!evidence.contains(secret));
                }
            }
            api.shutdown_instance().await.unwrap();
            drop(api);
            let cold = self::api(root.path()).await;
            assert_eq!(
                cold.acquisition().consumer_receipt(record.id).unwrap(),
                Some(receipt)
            );
            let cold_metadata = cold
                .model_library()
                .get_effective_metadata(&id)
                .unwrap()
                .unwrap();
            assert_eq!(cold_metadata.model_id, metadata.model_id);
            assert_eq!(cold_metadata.official_name, metadata.official_name);
            assert_eq!(cold_metadata.import_state, Some(ImportState::Ready));
            assert_eq!(
                cold_metadata.import_publication,
                metadata.import_publication
            );
            assert_eq!(
                std::fs::read(
                    cold.model_library()
                        .library_root()
                        .join(&id)
                        .join(".pumas_import_publication.json")
                )
                .unwrap(),
                publication_bytes
            );
            cold.shutdown_instance().await.unwrap();
        } else {
            assert!(result.is_err());
            assert!(api.model_library().list_models().await.unwrap().is_empty());
            assert!(!matches!(record.phase, AcquisitionPhase::Adopted { .. }));
            if mode == "cancel" {
                assert_eq!(progress.borrow().phase, S3ModelImportPhase::Cancelled);
                assert!(record.files.len() < record.manifest.files().len());
                assert!(api
                    .acquisition()
                    .consumer_receipt(record.id)
                    .unwrap()
                    .is_none());
            } else {
                let expected_field = if mode == "missing" {
                    "download.package"
                } else {
                    "import.acquired_package"
                };
                assert!(
                    matches!(&result, Err(S3ModelImportError::Operation(PumasError::Validation { field, .. })) if field == expected_field),
                    "expected shared package qualification refusal: {result:?}"
                );
                assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
                let receipt = api
                    .acquisition()
                    .consumer_receipt(record.id)
                    .unwrap()
                    .unwrap();
                assert_receipt(&record, &receipt);
                assert_eq!(receipt.payload, import_payload);
                assert_eq!(progress.borrow().phase, S3ModelImportPhase::Failed);
                // Byte verification succeeded, but model qualification refused publication.
                assert_eq!(record.files.len(), record.manifest.files().len());
                for file in &record.files {
                    assert_eq!(
                        file.sha256,
                        objects[&format!("{prefix}{}", file.path)].sha256
                    );
                }
            }
            api.shutdown_instance().await.unwrap();
        }
    }
}
