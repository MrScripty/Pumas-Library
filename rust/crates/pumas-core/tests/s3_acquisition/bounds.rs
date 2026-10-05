//! Consumer document admission: generic manifests and callback data are not
//! bounded by the optional S3 pin encoding.
use super::*;
use pumas_library::acquisition::{
    AcquisitionConsumerReceipt, AcquisitionHttpRequest, AcquisitionHttpSource, ArtifactFile,
    ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, RevisionStrength,
};

/// Use a real version-2 publication, rather than reproducing its private schema.
/// Worst-width physical numbers and fixed strings must still fit the reserves
/// used by the neutral facade; a schema expansion must fail this contract test.
pub(super) fn check_publication_schema_reserve(output: &serde_json::Value) {
    fn size(value: &serde_json::Value) -> usize {
        serde_json::to_vec_pretty(value).unwrap().len()
    }
    let document_limit = 16 * 1024 * 1024;
    assert_eq!(output["version"], 2);
    assert!(size(output) <= document_limit);
    let binding = &output["acquisition"];
    let binding_bytes = size(binding);
    assert!(binding_bytes <= 4 * 1024 * 1024);
    let mut widest = output.clone();
    widest["id"] = "p".repeat(36).into();
    widest["model_id"] = "m".repeat(4096).into();
    widest["original_stage"] = "s".repeat(48).into();
    let identity = serde_json::json!({"volume": u64::MAX, "file": u64::MAX});
    for field in ["library_root", "stage"] {
        assert_eq!(widest["payload"][field].as_object().unwrap().len(), 2);
        widest["payload"][field] = identity.clone();
    }
    let payload = widest["payload"].as_object().unwrap();
    assert_eq!(payload.len(), 4);
    let files = payload["files"].as_object().unwrap().clone();
    let directories = payload["directories"].as_object().unwrap().clone();
    widest["payload"]["files"] = serde_json::json!({});
    widest["payload"]["directories"] = serde_json::json!({});
    widest["acquisition"] = serde_json::Value::Null;
    let header_bytes = size(&widest);
    assert!(header_bytes < 16 * 1024);
    let mut embedded = widest.clone();
    embedded["acquisition"] = binding.clone();
    assert!(size(&embedded) - header_bytes <= 2 * binding_bytes);
    for (map, entries) in [("files", files), ("directories", directories)] {
        for (name, mut entry) in entries {
            if map == "files" {
                assert_eq!(entry.as_object().unwrap().len(), 3);
                assert_eq!(entry["identity"].as_object().unwrap().len(), 2);
                entry["identity"] = identity.clone();
                entry["size"] = u64::MAX.into();
                entry["sha256"] = "f".repeat(64).into();
            } else {
                assert_eq!(entry.as_object().unwrap().len(), 2);
                entry = identity.clone();
            }
            let mut one = widest.clone();
            one["payload"][map][&name] = entry;
            let name_bytes = serde_json::to_vec(&name).unwrap().len();
            assert!(size(&one) - header_bytes <= name_bytes + 512);
        }
    }
    // Doubling both actual pretty-JSON budgets leaves a fixed-header reserve.
    assert!(2 * (4 * 1024 * 1024 + 2 * 1024 * 1024) + 16 * 1024 < document_limit);
}

fn generic_manifest(files: Vec<ArtifactFile>) -> ArtifactManifest {
    ArtifactManifest::new(
        ArtifactSourceIdentity::new(
            "fixture",
            "generic-selection",
            ArtifactRevisionEvidence::new("fixture.pin", "v1", RevisionStrength::Immutable)
                .unwrap(),
        )
        .unwrap(),
        files,
    )
    .unwrap()
}

fn selected_file(path: &str, key: &str) -> ArtifactFile {
    ArtifactFile::new(
        path,
        key,
        Some(0),
        Some(Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest([]))).unwrap()),
        FileVerificationRequirement::Sha256,
    )
    .unwrap()
}

#[tokio::test]
async fn generic_manifest_document_and_namespace_bounds_refuse_before_source_or_store_admission() {
    for namespace in [false, true] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let api = api(root.path()).await;
        let manifest = if namespace {
            generic_manifest(vec![selected_file(
                &("a/".repeat(1900) + "weights.gguf"),
                "one-object",
            )])
        } else {
            let key = "k".repeat(16 * 1024);
            generic_manifest(
                (0..1024)
                    .map(|index| selected_file(&format!("f{index:04}.json"), &key))
                    .collect(),
            )
        };
        // Retained manifest decoding is a separate, unchanged contract.
        assert_eq!(
            serde_json::from_value::<ArtifactManifest>(serde_json::to_value(&manifest).unwrap())
                .unwrap(),
            manifest
        );
        if !namespace {
            let wire_bytes = serde_json::to_vec_pretty(&manifest).unwrap().len();
            assert!(wire_bytes > 16 * 1024 * 1024);
            eprintln!("Valid generic manifest serialized bytes: {wire_bytes}");
        }
        let fixture = Fixture::serve(Vec::new()).await;
        let consumer = api.acquisition().open_consumer("model.s3").unwrap();
        let store_path = root.path().join("launcher-data/downloads.json");
        let before = std::fs::read(&store_path).ok();
        let called = Arc::new(AtomicBool::new(false));
        let observed = called.clone();
        let result = consumer
            .acquire_http(
                AcquisitionHttpRequest {
                    demand: AcquisitionDemand {
                        consumer: "model.s3".into(),
                        operation: "oversized-generic-selection".into(),
                    },
                    sources: manifest
                        .files()
                        .iter()
                        .map(|_| AcquisitionHttpSource {
                            url: format!("{}/unused", fixture.endpoint),
                            authorization: None,
                        })
                        .collect(),
                    manifest,
                    workspace: workspace(stage.path()),
                    retry: retry(1),
                },
                reqwest::Client::new(),
                Box::new(Host::quiet()),
                move |_| async move {
                    observed.store(true, Ordering::SeqCst);
                    Ok(((), serde_json::Value::Null))
                },
                |(), _| async { Ok(()) },
            )
            .await;
        consumer.shutdown().await.unwrap();
        close(&api).await;
        let requests = fixture.finish().await;
        let field = if namespace {
            "acquisition.consumer_namespace_size"
        } else {
            "acquisition.consumer_document_size"
        };
        assert!(
            matches!(result, Err(PumasError::Validation { field: ref actual, .. }) if actual == field),
            "Expected {field}, got {result:?}; requests={}",
            requests.len()
        );
        assert!(requests.is_empty());
        assert!(!called.load(Ordering::SeqCst));
        assert_eq!(std::fs::read(&store_path).ok(), before);
        assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
        assert!(api.model_library().index().list_all().unwrap().is_empty());
        assert!(std::fs::read_dir(stage.path().join("stage"))
            .unwrap()
            .next()
            .is_none());
    }
}

#[tokio::test]
async fn oversized_callback_binding_refuses_before_receipt_issuance_or_publication() {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let bytes = gguf();
    let fixture = Fixture::serve(vec![
        head(bytes.len()),
        range(VERSION, 0, bytes.len(), &bytes),
    ])
    .await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let consumer = api.acquisition().open_consumer("model.s3").unwrap();
    let published = Arc::new(AtomicBool::new(false));
    let observed = published.clone();
    let mut import_spec = spec(LOGICAL);
    import_spec.tags = Some(vec!["t".repeat(17 * 1024 * 1024)]);
    let result = consumer
        .acquire_s3(
            acquire_request(selected, workspace(stage.path()), 1),
            Box::new(Host::quiet()),
            move |acquired| async move { Ok((acquired, serde_json::to_value(import_spec)?)) },
            move |_, receipt: AcquisitionConsumerReceipt| async move {
                let wire_bytes = serde_json::to_vec_pretty(&receipt)?.len();
                assert!(wire_bytes > 16 * 1024 * 1024);
                eprintln!("Issued completion binding serialized bytes: {wire_bytes}");
                observed.store(true, Ordering::SeqCst);
                // The fixture observes issuance without invoking model side effects.
                Err::<(), _>(PumasError::Validation {
                    field: "fixture.oversized_binding_was_issued".into(),
                    message: "Publication must not receive this binding".into(),
                })
            },
        )
        .await;
    let record = api
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    let issued = consumer.completion_receipt(&record).unwrap();
    consumer.shutdown().await.unwrap();
    close(&api).await;
    let requests = fixture.finish().await;
    assert!(
        matches!(result, Err(PumasError::Validation { ref field, .. }) if field == "acquisition.consumer_document_size"),
        "Oversized binding reached issuance: {result:?}"
    );
    assert!(!published.load(Ordering::SeqCst));
    assert!(issued.is_none());
    assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
    assert_eq!(record.files.len(), 1);
    assert_eq!(
        std::fs::read(stage.path().join("stage").join(LOGICAL)).unwrap(),
        bytes
    );
    assert!(api.model_library().index().list_all().unwrap().is_empty());
    assert_eq!(requests.len(), 2);
}
