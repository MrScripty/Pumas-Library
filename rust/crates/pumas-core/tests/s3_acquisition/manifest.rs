//! Explicit whole-set selection -> shared custody -> real GGUF bundle publication.
use super::*;
use pumas_library::acquisition::{
    AcquisitionS3ManifestRequest, ArtifactManifest, ArtifactSourceIdentity, S3ManifestEntry,
    S3ReaderError,
};

const AUX_VERSION: &str = "auxiliary-v7";
const AUX: &[u8] = b"{}";
const AUX_PATH: &str = "config/tokenizer_config.json";

fn reader(endpoint: &str) -> S3Reader {
    S3Reader::new(S3ReaderConfig {
        endpoint: endpoint.into(),
        region: "fixture-region".into(),
        bucket: "fixture-bucket".into(),
        addressing: S3Addressing::Path,
        allow_http: true,
        operation_timeout: Duration::from_secs(5),
    })
    .unwrap()
}
fn members(aux_path: &str) -> Vec<S3ManifestEntry> {
    vec![
        S3ManifestEntry {
            source_key: "models/weights.gguf".into(),
            version: VERSION.into(),
            logical_path: LOGICAL.into(),
            expected_sha256: Sha256Evidence::new(
                "fixture.sha256",
                hex::encode(Sha256::digest(gguf())),
            )
            .unwrap(),
        },
        S3ManifestEntry {
            source_key: "models/auxiliary.json".into(),
            version: AUX_VERSION.into(),
            logical_path: aux_path.into(),
            expected_sha256: Sha256Evidence::new(
                "fixture.sha256",
                hex::encode(Sha256::digest(AUX)),
            )
            .unwrap(),
        },
    ]
}
fn head_version(size: usize, version: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {version}\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n").into_bytes()
}
fn responses(entries: &[S3ManifestEntry]) -> Vec<Vec<u8>> {
    let mut order: Vec<_> = entries.iter().collect();
    order.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
    let mut wire = Vec::new();
    for entry in &order {
        let size = if entry.logical_path == LOGICAL {
            gguf().len()
        } else {
            AUX.len()
        };
        wire.push(head_version(size, &entry.version));
    }
    for entry in order {
        let bytes = if entry.logical_path == LOGICAL {
            gguf()
        } else {
            AUX.to_vec()
        };
        wire.push(range(&entry.version, 0, bytes.len(), &bytes));
    }
    wire
}
fn request_for(
    selection: pumas_library::acquisition::S3ManifestSelection,
    workspace: AcquisitionWorkspace,
) -> AcquisitionS3ManifestRequest {
    AcquisitionS3ManifestRequest {
        demand: AcquisitionDemand {
            consumer: "model.s3".into(),
            operation: "explicit-gguf-bundle".into(),
        },
        selection,
        workspace,
        retry: retry(2),
    }
}

#[tokio::test]
async fn explicit_manifest_preserves_each_pin_and_canonical_order_without_reselection() {
    let entries = members(AUX_PATH);
    let mut wire = responses(&entries);
    wire.truncate(2);
    wire.extend(wire.clone());
    let fixture = Fixture::serve(wire).await;
    let reader = reader(&fixture.endpoint);
    let first = reader.select_manifest(entries.clone()).await.unwrap();
    let second = reader
        .select_manifest(entries.into_iter().rev().collect())
        .await
        .unwrap();
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 4);
    assert!(requests.iter().all(|request| request.starts_with("HEAD ")));
    assert_eq!(first.manifest(), second.manifest());
    let manifest = first.manifest();
    assert_eq!(
        manifest.total_bytes(),
        Some((gguf().len() + AUX.len()) as u64)
    );
    assert_eq!(manifest.files()[0].logical_path(), AUX_PATH);
    assert_eq!(
        manifest.files()[0].source_key(),
        r#"["models/auxiliary.json","auxiliary-v7"]"#
    );
    assert_eq!(
        manifest.files()[1].source_key(),
        r#"["models/weights.gguf","selected-v1"]"#
    );
    let pins: Vec<ArtifactSourceIdentity> =
        serde_json::from_str(manifest.source().revision().value()).unwrap();
    assert_eq!(pins.len(), 2);
    assert_eq!(pins[0].revision().value(), AUX_VERSION);
    assert_eq!(pins[1].revision().value(), VERSION);
    for (pin, file) in pins.iter().zip(manifest.files()) {
        let (key, version): (String, String) = serde_json::from_str(file.source_key()).unwrap();
        assert_eq!(
            hex::decode(pin.source_id().rsplit(':').next().unwrap()).unwrap(),
            key.as_bytes()
        );
        assert_eq!(pin.revision().value(), version);
    }
}

#[tokio::test]
async fn manifest_resolution_requires_every_declared_version_without_partial_admission() {
    let root = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let store_path = root.path().join("launcher-data/downloads.json");
    let before = std::fs::read(&store_path).ok();
    let fixture = Fixture::serve(vec![
        head_version(AUX.len(), AUX_VERSION),
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    ])
    .await;
    let result = reader(&fixture.endpoint)
        .select_manifest(members(AUX_PATH))
        .await;
    close(&api).await;
    let requests = fixture.finish().await;
    assert!(matches!(result, Err(S3ReaderError::Unavailable)));
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.starts_with("HEAD ")));
    assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
    assert_eq!(std::fs::read(&store_path).ok(), before);
    assert!(api.model_library().index().list_all().unwrap().is_empty());
}

#[tokio::test]
async fn same_s3_key_with_distinct_versions_publishes_and_recovers_each_selected_payload() {
    bundle_publication_and_cold_proof_with_same_key(false, "none", true).await;
}

#[tokio::test]
async fn same_s3_key_and_version_with_conflicting_digests_is_refused_before_head() {
    let fixture = Fixture::serve(Vec::new()).await;
    let mut entries = members(AUX_PATH);
    entries[1].source_key = entries[0].source_key.clone();
    entries[1].version = entries[0].version.clone();
    let result = reader(&fixture.endpoint).select_manifest(entries).await;
    assert!(matches!(result, Err(S3ReaderError::Manifest(_))));
    assert!(fixture.finish().await.is_empty());
}

#[tokio::test]
async fn same_s3_key_and_version_with_conflicting_sizes_is_refused_after_head() {
    let fixture = Fixture::serve(vec![head_version(2, VERSION), head_version(24, VERSION)]).await;
    let mut entries = members(AUX_PATH);
    entries[1].source_key = entries[0].source_key.clone();
    entries[1].version = entries[0].version.clone();
    entries[1].expected_sha256 = entries[0].expected_sha256.clone();
    let result = reader(&fixture.endpoint).select_manifest(entries).await;
    assert!(matches!(result, Err(S3ReaderError::Manifest(_))));
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.starts_with("HEAD ")));
}

#[tokio::test]
async fn same_s3_key_and_version_with_consistent_evidence_may_have_two_logical_paths() {
    let size = gguf().len();
    let fixture = Fixture::serve(vec![
        head_version(size, VERSION),
        head_version(size, VERSION),
    ])
    .await;
    let mut entries = members(AUX_PATH);
    entries[1].source_key = entries[0].source_key.clone();
    entries[1].version = entries[0].version.clone();
    entries[1].expected_sha256 = entries[0].expected_sha256.clone();
    let selected = reader(&fixture.endpoint)
        .select_manifest(entries)
        .await
        .unwrap();
    let files = selected.manifest().files();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].source_key(), files[1].source_key());
    assert_ne!(files[0].logical_path(), files[1].logical_path());
    assert_eq!(selected.manifest().total_bytes(), Some(2 * size as u64));
    assert_eq!(fixture.finish().await.len(), 2);
}

#[tokio::test]
async fn whole_manifest_namespace_and_pin_budget_are_refused_before_source_io() {
    for fault in [
        "empty",
        "duplicate",
        "staging",
        "parent",
        "case",
        "mutable",
        "budget",
    ] {
        let fixture = Fixture::serve(Vec::new()).await;
        let mut entries = members(AUX_PATH);
        match fault {
            "empty" => entries.clear(),
            "duplicate" => entries[1].logical_path = LOGICAL.into(),
            "staging" => entries[1].logical_path = format!("{LOGICAL}.part"),
            "parent" => entries[1].logical_path = format!("{LOGICAL}/config.json"),
            "case" => entries[1].logical_path = LOGICAL.to_uppercase(),
            "mutable" => entries[1].version = "null".into(),
            "budget" => {
                for entry in &mut entries {
                    entry.version = "v".repeat(8500);
                }
            }
            _ => unreachable!(),
        }
        let result = reader(&fixture.endpoint).select_manifest(entries).await;
        let requests = fixture.finish().await;
        assert!(
            matches!(
                result,
                Err(S3ReaderError::Manifest(_) | S3ReaderError::Configuration(_))
            ),
            "{fault}"
        );
        assert!(requests.is_empty(), "{fault} performed source IO");
    }
}

async fn bundle_publication_and_cold_proof(interrupt: bool, fault: &str) {
    bundle_publication_and_cold_proof_with_same_key(interrupt, fault, false).await;
}

async fn bundle_publication_and_cold_proof_with_same_key(
    interrupt: bool,
    fault: &str,
    same_key: bool,
) {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let first = api(root.path()).await;
    let mut entries = members(AUX_PATH);
    if same_key {
        entries[1].source_key = entries[0].source_key.clone();
    }
    let fixture = Fixture::serve(responses(&entries)).await;
    let selected = reader(&fixture.endpoint)
        .select_manifest(entries)
        .await
        .unwrap();
    let manifest = selected.manifest().clone();
    let consumer = first.acquisition().open_consumer("model.s3").unwrap();
    let request = request_for(selected, workspace(stage.path()));
    let demand = request.demand.clone();
    let importer = ModelImporter::new(first.model_library().clone());
    let (published, observed) = tokio::sync::oneshot::channel();
    let result = consumer
        .acquire_s3_manifest(
            request,
            Box::new(Host::quiet()),
            |acquired| async { Ok((acquired, serde_json::to_value(spec(LOGICAL))?)) },
            move |acquired, receipt| async move {
                let model = importer
                    .import_acquired_gguf_bundle(&acquired, &receipt, &spec(LOGICAL))
                    .await?;
                published.send(model.model_id.clone().unwrap()).unwrap();
                if interrupt {
                    return Err(PumasError::Validation {
                        field: "fixture.bundle_acknowledgement".into(),
                        message: "Interrupted after confirmed bundle publication".into(),
                    });
                }
                Ok(model)
            },
        )
        .await;
    let published_id = observed.await;
    consumer.shutdown().await.unwrap();
    close(&first).await;
    let id = published_id.unwrap();
    if interrupt {
        assert!(
            matches!(result,Err(PumasError::Validation { ref field,.. }) if field=="fixture.bundle_acknowledgement")
        );
    } else {
        assert!(result.unwrap().success);
    }
    let record = first
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert_eq!(record.manifest, manifest);
    assert_eq!(record.files.len(), 2);
    assert_eq!(
        matches!(record.phase, AcquisitionPhase::Using { .. }),
        interrupt
    );
    let issued = consumer.completion_receipt(&record).unwrap().unwrap();
    let target = first.model_library().library_root().join(&id);
    assert_eq!(std::fs::read(target.join(LOGICAL)).unwrap(), gguf());
    assert_eq!(std::fs::read(target.join(AUX_PATH)).unwrap(), AUX);
    let receipt_bytes = std::fs::read(target.join(".pumas_import_publication.json")).unwrap();
    let output: serde_json::Value = serde_json::from_slice(&receipt_bytes).unwrap();
    super::bounds::check_publication_schema_reserve(&output);
    assert_eq!(
        output["acquisition"],
        serde_json::to_value(&issued).unwrap()
    );
    assert_eq!(output["payload"]["files"].as_object().unwrap().len(), 2);
    assert!(output["payload"]["files"].get(AUX_PATH).is_some());
    drop(consumer);
    drop(first);
    if fault == "auxiliary" || fault.contains("identity") {
        std::fs::write(target.join(AUX_PATH), b"[]").unwrap();
    }
    if fault.contains("identity") {
        let path = target.join("metadata.json");
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if fault.starts_with("null-") {
            metadata["import_publication"] = serde_json::Value::Null;
        } else {
            metadata
                .as_object_mut()
                .unwrap()
                .remove("import_publication");
        }
        std::fs::write(path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
    }
    let cold = api(root.path()).await;
    if fault.contains("identity") {
        let mut indexed = cold.model_library().index().get(&id).unwrap().unwrap();
        if fault.starts_with("null-") {
            indexed.metadata["import_publication"] = serde_json::Value::Null;
        } else {
            indexed
                .metadata
                .as_object_mut()
                .unwrap()
                .remove("import_publication");
        }
        cold.model_library().index().upsert(&indexed).unwrap();
    }
    let metadata_before = std::fs::read(target.join("metadata.json")).unwrap();
    let index_before = cold
        .model_library()
        .index()
        .get(&id)
        .unwrap()
        .unwrap()
        .metadata;
    if fault.contains("identity") {
        let canonical: serde_json::Value = serde_json::from_slice(&metadata_before).unwrap();
        for projection in [&canonical, &index_before] {
            assert!(projection
                .get("import_publication")
                .is_none_or(serde_json::Value::is_null));
        }
    }
    let consumer = cold.acquisition().open_consumer("model.s3").unwrap();
    let before = std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap();
    let auxiliary_before = std::fs::read(target.join(AUX_PATH)).unwrap();
    let retained_manifest = if fault == "subset" {
        ArtifactManifest::new(manifest.source().clone(), vec![manifest.files()[1].clone()]).unwrap()
    } else {
        manifest
    };
    let workspace = reopen_workspace(stage.path());
    let importer = ModelImporter::new(cold.model_library().clone());
    let selected_id = id.clone();
    let proof = consumer
        .reconcile(
            demand,
            retained_manifest,
            workspace,
            move |receipt, acquired| async move {
                importer
                    .reconcile_acquired_gguf_bundle(
                        &acquired,
                        &receipt,
                        &spec(LOGICAL),
                        &selected_id,
                    )
                    .await
            },
        )
        .await;
    consumer.shutdown().await.unwrap();
    close(&cold).await;
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 4, "bundle recovery replayed the source");
    if same_key {
        assert!(requests
            .iter()
            .all(|request| request.contains("/fixture-bucket/models/weights.gguf?")));
        for version in [VERSION, AUX_VERSION] {
            assert_eq!(
                requests
                    .iter()
                    .filter(|request| request.contains(&format!("versionId={version}")))
                    .count(),
                2
            );
        }
    }
    assert_eq!(
        std::fs::read(target.join(".pumas_import_publication.json")).unwrap(),
        receipt_bytes
    );
    assert_eq!(std::fs::read(target.join(LOGICAL)).unwrap(), gguf());
    assert_eq!(
        std::fs::read(target.join(AUX_PATH)).unwrap(),
        auxiliary_before
    );
    assert_eq!(
        std::fs::read(stage.path().join("stage").join(LOGICAL)).unwrap(),
        gguf()
    );
    assert_eq!(
        std::fs::read(stage.path().join("stage").join(AUX_PATH)).unwrap(),
        AUX
    );
    if fault != "none" {
        if fault.contains("identity") {
            assert!(
                matches!(proof, Err(PumasError::Validation { ref field, .. })
                if field == "import.acquired_recovery_required")
            );
            assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
        }
        assert_eq!(
            std::fs::read(target.join("metadata.json")).unwrap(),
            metadata_before
        );
        assert_eq!(
            cold.model_library()
                .index()
                .get(&id)
                .unwrap()
                .unwrap()
                .metadata,
            index_before
        );
        assert!(
            proof.is_err(),
            "{fault} settled an incomplete or changed output"
        );
        assert_eq!(
            std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
            before
        );
        assert_eq!(
            cold.acquisition()
                .store()
                .acquisitions()
                .unwrap()
                .get(&record.id),
            Some(&record)
        );
    } else {
        assert_eq!(
            proof.unwrap().unwrap().model_id.as_deref(),
            Some(id.as_str())
        );
        let adopted = cold
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .remove(&record.id)
            .unwrap();
        assert!(matches!(adopted.phase, AcquisitionPhase::Adopted { .. }));
        assert_eq!(consumer.completion_receipt(&adopted).unwrap(), Some(issued));
        let readonly = PumasReadOnlyLibrary::open(cold.model_library().library_root()).unwrap();
        assert_eq!(
            readonly
                .model_library_selector_snapshot(Default::default())
                .unwrap()
                .rows
                .iter()
                .find(|row| row.model_id == id)
                .unwrap()
                .artifact_state,
            ModelArtifactState::Ready
        );
    }
}

#[tokio::test]
async fn missing_bundle_publication_identities_cannot_settle_changed_auxiliary() {
    for fault in ["identity", "null-identity"] {
        bundle_publication_and_cold_proof(true, fault).await;
    }
}

#[tokio::test]
async fn explicit_gguf_bundle_publishes_all_paths_and_cold_adopted_proof() {
    bundle_publication_and_cold_proof(false, "none").await;
}
#[tokio::test]
async fn interrupted_confirmed_bundle_cold_settles_without_transfer_or_import_replay() {
    bundle_publication_and_cold_proof(true, "none").await;
}
#[tokio::test]
async fn changed_auxiliary_retains_the_exact_interrupted_bundle_use() {
    bundle_publication_and_cold_proof(true, "auxiliary").await;
}
#[tokio::test]
async fn omitted_member_cannot_reconcile_a_complete_bundle_generation() {
    bundle_publication_and_cold_proof(true, "subset").await;
}

#[tokio::test]
async fn changed_or_corrupt_last_member_blocks_whole_set_model_handoff() {
    for changed in [true, false] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let api = api(root.path()).await;
        let entries = members("z/tokenizer_config.json");
        let mut wire = responses(&entries);
        wire[3] = if changed {
            range("other-version", 0, AUX.len(), AUX)
        } else {
            range(AUX_VERSION, 0, AUX.len(), b"[]")
        };
        let fixture = Fixture::serve(wire).await;
        let selected = reader(&fixture.endpoint)
            .select_manifest(entries)
            .await
            .unwrap();
        let consumer = api.acquisition().open_consumer("model.s3").unwrap();
        let prepared = Arc::new(AtomicBool::new(false));
        let observed = prepared.clone();
        let result = consumer
            .acquire_s3_manifest(
                request_for(selected, workspace(stage.path())),
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
        if changed {
            assert!(matches!(result, Err(PumasError::Validation { .. })));
        } else {
            assert!(matches!(result, Err(PumasError::HashMismatch { .. })));
        }
        assert!(!prepared.load(Ordering::SeqCst));
        assert_eq!(requests.len(), 4);
        let record = api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        assert_eq!(record.phase, AcquisitionPhase::Transferring);
        assert_eq!(record.manifest.files().len(), 2);
        assert!(consumer.completion_receipt(&record).is_err());
        assert!(api.model_library().index().list_all().unwrap().is_empty());
        assert_eq!(
            std::fs::read(stage.path().join("stage").join(LOGICAL)).unwrap(),
            gguf()
        );
    }
}

#[tokio::test]
async fn bundle_refuses_other_weights_executable_auxiliaries_and_reserved_metadata() {
    for auxiliary in ["extra.safetensors", "script.py", "metadata.json"] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let api = api(root.path()).await;
        let entries = members(auxiliary);
        let fixture = Fixture::serve(responses(&entries)).await;
        let selected = reader(&fixture.endpoint)
            .select_manifest(entries)
            .await
            .unwrap();
        let consumer = api.acquisition().open_consumer("model.s3").unwrap();
        let importer = ModelImporter::new(api.model_library().clone());
        let result = consumer
            .acquire_s3_manifest(
                request_for(selected, workspace(stage.path())),
                Box::new(Host::quiet()),
                |acquired| async { Ok((acquired, serde_json::to_value(spec(LOGICAL))?)) },
                move |acquired, receipt| async move {
                    importer
                        .import_acquired_gguf_bundle(&acquired, &receipt, &spec(LOGICAL))
                        .await
                },
            )
            .await;
        consumer.shutdown().await.unwrap();
        close(&api).await;
        assert!(
            matches!(
                result,
                Err(PumasError::Validation { .. } | PumasError::ImportFailed { .. })
            ),
            "{auxiliary}"
        );
        assert_eq!(fixture.finish().await.len(), 4);
        let record = api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
        assert!(consumer.completion_receipt(&record).unwrap().is_some());
        assert!(api.model_library().index().list_all().unwrap().is_empty());
        assert_eq!(
            std::fs::read(stage.path().join("stage").join(auxiliary)).unwrap(),
            AUX
        );
    }
}
