//! Public construction/migration and shared-consumer regressions. All roots
//! and HTTP endpoints are local temporary fixtures.
use pumas_library::{
    acquisition::{
        AcquisitionDemand, AcquisitionHttpRequest, AcquisitionHttpSource, AcquisitionRetryPolicy,
        AcquisitionService, AcquisitionStore, AcquisitionWorkspace, ArtifactFile, ArtifactManifest,
        ArtifactRevisionEvidence, ArtifactSourceIdentity, FileVerificationRequirement,
        ManifestValidationError, RevisionStrength,
    },
    model_library::DownloadPersistence,
    network::RetryConfig,
    PumasApi, PumasError, Result,
};
use sha2::{Digest, Sha256};
use std::{future::pending, io::Read, path::Path, sync::Arc, time::Duration};

fn fixture_source() -> ArtifactSourceIdentity {
    ArtifactSourceIdentity::new(
        "fixture",
        "selected-objects",
        ArtifactRevisionEvidence::new("fixture.revision", "v1", RevisionStrength::Immutable)
            .unwrap(),
    )
    .unwrap()
}

fn fixture_file(path: &str, source_key: &str, bytes: u64) -> ArtifactFile {
    ArtifactFile::new(
        path,
        source_key,
        Some(bytes),
        None,
        FileVerificationRequirement::SizeAndImmutableRevision,
    )
    .unwrap()
}

#[test]
fn public_manifest_rejects_final_staging_collisions_in_either_order() {
    for paths in [
        ["weights", "weights.part"],
        ["weights", "weights.part/config"],
        ["Weights", "WEIGHTS.PART"],
        ["Weights", "WEIGHTS.PART/config"],
        ["nested/weights", "nested/weights.part/config"],
        ["weights.part", "weights.part.part"],
    ] {
        for paths in [paths, [paths[1], paths[0]]] {
            let files = paths.map(|path| fixture_file(path, path, 1)).to_vec();
            assert_eq!(
                ArtifactManifest::new(fixture_source(), files.clone()),
                Err(ManifestValidationError::CollidingLogicalPath),
                "{paths:?}"
            );
            let wire = serde_json::json!({
                "schema_version": 1,
                "source": fixture_source(),
                "files": files
            });
            assert!(
                serde_json::from_value::<ArtifactManifest>(wire).is_err(),
                "{paths:?}"
            );
        }
    }
}

#[test]
fn schema_seven_reader_refuses_staging_aliases_without_rewriting_state() {
    let root = tempfile::TempDir::new().unwrap();
    let path = root.path().join("downloads.json");
    let id = uuid::Uuid::new_v4();
    let record = pumas_library::acquisition::AcquisitionRecord {
        id,
        demand: AcquisitionDemand {
            consumer: "fixture.consumer".into(),
            operation: "stage-conflict".into(),
        },
        manifest: ArtifactManifest::new(
            fixture_source(),
            vec![fixture_file("weights", "first", 1)],
        )
        .unwrap(),
        workspace: pumas_library::acquisition::WorkspaceIdentity {
            root_identity: "fixture.root".into(),
            relative_target: "stage".into(),
        },
        phase: pumas_library::acquisition::AcquisitionPhase::Transferring,
        files: Vec::new(),
    };
    let valid = serde_json::json!({
        "schema_version": 7,
        "acquisitions": {id.to_string(): record},
        "consumer_receipts": {}
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&valid).unwrap()).unwrap();
    let store = AcquisitionStore::new(root.path());
    // Establish that the production reader accepts the surrounding fixture.
    assert_eq!(store.acquisitions().unwrap().get(&id), Some(&record));
    for alias in ["weights.part", "weights.part/config", "WEIGHTS.PART"] {
        let mut invalid = valid.clone();
        invalid["acquisitions"][id.to_string()]["manifest"]["files"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::to_value(fixture_file(alias, "second", 1)).unwrap());
        let original = serde_json::to_vec_pretty(&invalid).unwrap();
        std::fs::write(&path, &original).unwrap();
        assert!(store.require_acquisition_schema().is_err(), "{alias}");
        assert!(store.acquisitions().is_err(), "{alias}");
        assert_eq!(std::fs::read(&path).unwrap(), original, "{alias}");
    }
}

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
    api.shutdown_instance().await.unwrap();
    drop(api);
    // This fixture has no old readers/writers. Migration is a separate operator action.
    DownloadPersistence::migrate_legacy_offline(&data).unwrap();
    let current: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(current["schema_version"], 7);
    assert_eq!(current["acquisitions"], serde_json::json!({}));
    assert!(DownloadPersistence::new(&data).load_all().is_empty());
    assert!(DownloadPersistence::migrate_legacy_offline(&data).is_err());
}

struct LocalHttpHost;

#[async_trait::async_trait]
impl pumas_library::acquisition::HttpAttemptHost for LocalHttpHost {
    async fn pause_requested(&self) {
        pending::<()>().await;
    }

    fn pause_requested_now(&self) -> bool {
        false
    }

    fn cancel_requested(&self) -> bool {
        false
    }

    async fn record_progress(&mut self, _downloaded_for_file: u64) -> Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl pumas_library::acquisition::AcquisitionHost for LocalHttpHost {
    async fn retry(
        &mut self,
        _attempt: u32,
        _delay: Option<Duration>,
        _error: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn public_multifile_acquisition_preserves_nonconflicting_part_names_and_opaque_keys() {
    let state = tempfile::TempDir::new().unwrap();
    let workspace_root = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(workspace_root.path().join("stage")).unwrap();
    let workspace = AcquisitionWorkspace::from_reserved_directory(
        workspace_root.path(),
        Path::new("stage"),
        Arc::new(()),
        || Ok(()),
    )
    .unwrap();
    let selected: [(&str, &str, &[u8]); 3] = [
        ("weights", "objects//../weights?version=1#fragment", b"DATA"),
        ("notes.part", "objects\\notes.part", b"notes"),
        ("nested/config.json", "objects//config", b"{}"),
    ];
    let manifest = ArtifactManifest::new(
        fixture_source(),
        selected
            .iter()
            .map(|(path, key, bytes)| fixture_file(path, key, bytes.len() as u64))
            .collect(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_value::<ArtifactManifest>(serde_json::to_value(&manifest).unwrap())
            .unwrap(),
        manifest
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for _ in 0..selected.len() {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let count = socket.read(&mut request).await.unwrap();
            let request = std::str::from_utf8(&request[..count]).unwrap();
            let index: usize = request
                .split_whitespace()
                .nth(1)
                .unwrap()
                .trim_start_matches('/')
                .parse()
                .unwrap();
            let bytes = selected[index].2;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(bytes).await.unwrap();
        }
    });
    let store = Arc::new(AcquisitionStore::new(state.path()));
    let service = Arc::new(AcquisitionService::new(store.clone()));
    let consumer = service.open_consumer("fixture.consumer").unwrap();
    consumer
        .acquire_http(
            AcquisitionHttpRequest {
                demand: AcquisitionDemand {
                    consumer: "fixture.consumer".into(),
                    operation: "multiple-files".into(),
                },
                manifest: manifest.clone(),
                workspace,
                sources: (0..selected.len())
                    .map(|index| AcquisitionHttpSource {
                        url: format!("http://{address}/{index}"),
                        authorization: None,
                    })
                    .collect(),
                retry: AcquisitionRetryPolicy {
                    attempts: Some(1),
                    elapsed: Duration::ZERO,
                    backoff: RetryConfig::default().with_jitter(false),
                },
            },
            reqwest::Client::new(),
            Box::new(LocalHttpHost),
            move |use_set| async move {
                for (index, (_, _, expected)) in selected.iter().enumerate() {
                    let mut file = use_set.open_file(index).await?;
                    let mut observed = Vec::new();
                    file.read_to_end(&mut observed)?;
                    assert_eq!(observed, *expected);
                }
                Ok(((), serde_json::json!({"installed": true})))
            },
            |(), receipt| async move {
                assert_eq!(receipt.verified_files.len(), 3);
                Ok(())
            },
        )
        .await
        .unwrap();
    server.await.unwrap();
    let record = store.acquisitions().unwrap().into_values().next().unwrap();
    let receipt = service.consumer_receipt(record.id).unwrap().unwrap();
    assert_eq!(receipt.manifest, manifest);
    assert_eq!(receipt.verified_files, record.files);
    for ((path, key, bytes), file) in selected.iter().zip(receipt.manifest.files()) {
        assert_eq!(file.logical_path(), *path);
        assert_eq!(file.source_key(), *key);
        assert_eq!(
            std::fs::read(workspace_root.path().join("stage").join(path)).unwrap(),
            *bytes
        );
        assert!(!workspace_root
            .path()
            .join("stage")
            .join(format!("{path}.part"))
            .exists());
    }
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    drop(consumer);
    drop(service);
    drop(store);
    let reopened = AcquisitionService::new(Arc::new(AcquisitionStore::new(state.path())));
    assert_eq!(reopened.consumer_receipt(record.id).unwrap(), Some(receipt));
}

#[tokio::test]
async fn public_consumer_acquires_http_and_settles_its_receipt_under_shared_custody() {
    let state = tempfile::TempDir::new().unwrap();
    let workspace_root = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(workspace_root.path().join("install-stage")).unwrap();
    let workspace = AcquisitionWorkspace::from_reserved_directory(
        workspace_root.path(),
        Path::new("install-stage"),
        Arc::new(()),
        || Ok(()),
    )
    .unwrap();

    let bytes = b"verified native archive";
    let digest = format!("{:x}", Sha256::digest(bytes));
    let manifest = ArtifactManifest::new(
        ArtifactSourceIdentity::new(
            "fixture",
            "native-release-asset",
            ArtifactRevisionEvidence::new(
                "fixture.release",
                "immutable-asset-v1",
                RevisionStrength::Immutable,
            )
            .unwrap(),
        )
        .unwrap(),
        vec![ArtifactFile::new(
            "runtime.tar.gz",
            "runtime archive",
            Some(bytes.len() as u64),
            Some(
                pumas_library::acquisition::Sha256Evidence::new("fixture.sha256", digest).unwrap(),
            ),
            FileVerificationRequirement::Sha256,
        )
        .unwrap()],
    )
    .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 1024];
        let _ = socket.read(&mut request).await.unwrap();
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        socket.write_all(bytes).await.unwrap();
    });

    let store = Arc::new(AcquisitionStore::new(state.path()));
    let service = Arc::new(AcquisitionService::new(store.clone()));
    let consumer = service.open_consumer("runtime.llama.cpp").unwrap();
    let client = reqwest::Client::new();
    let result = consumer
        .acquire_http(
            AcquisitionHttpRequest {
                demand: AcquisitionDemand {
                    consumer: "runtime.llama.cpp".into(),
                    operation: "llama.cpp:v1:linux-x86_64".into(),
                },
                manifest,
                workspace,
                sources: vec![AcquisitionHttpSource {
                    url: format!("http://{address}/runtime.tar.gz"),
                    authorization: None,
                }],
                retry: AcquisitionRetryPolicy {
                    attempts: Some(1),
                    elapsed: Duration::ZERO,
                    backoff: RetryConfig::default().with_jitter(false),
                },
            },
            client,
            Box::new(LocalHttpHost),
            move |use_set| async move {
                let mut file = use_set.open_file(0).await?;
                let mut observed = Vec::new();
                file.read_to_end(&mut observed)?;
                assert_eq!(observed, bytes);
                Ok((observed, serde_json::json!({"installed": true})))
            },
            move |observed, receipt| async move {
                assert_eq!(receipt.payload, serde_json::json!({"installed": true}));
                Ok(observed)
            },
        )
        .await
        .unwrap();
    assert_eq!(result, bytes);
    server.await.unwrap();

    let record = store.acquisitions().unwrap().into_values().next().unwrap();
    let receipt = service.consumer_receipt(record.id).unwrap().unwrap();
    assert!(matches!(
        &record.phase,
        pumas_library::acquisition::AcquisitionPhase::Adopted { lease }
            if lease.to_string() == receipt.use_lease
    ));
    assert_eq!(receipt.acquisition_id, record.id.to_string());
    assert_eq!(receipt.owner, record.demand.consumer);
    assert_eq!(receipt.demand, record.demand);
    assert_eq!(receipt.manifest, record.manifest);
    assert_eq!(receipt.workspace, record.workspace);
    assert_eq!(receipt.verified_files, record.files);
    assert_eq!(receipt.owner, "runtime.llama.cpp");
    assert_eq!(receipt.payload, serde_json::json!({"installed": true}));
    let persisted: serde_json::Value =
        serde_json::from_slice(&std::fs::read(state.path().join("downloads.json")).unwrap())
            .unwrap();
    assert!(persisted.get("downloads").is_none());
    consumer.shutdown().await.unwrap();
    for _ in 0..2 {
        service.shutdown().await.unwrap();
        assert_eq!(store.acquisitions().unwrap().get(&record.id), Some(&record));
        assert_eq!(
            service.consumer_receipt(record.id).unwrap(),
            Some(receipt.clone())
        );
        assert!(matches!(
            service.open_consumer("runtime.llama.cpp"),
            Err(PumasError::DownloadLifecycleClosed)
        ));
    }
    drop(consumer);
    drop(service);
    drop(store);

    let reopened_store = Arc::new(AcquisitionStore::new(state.path()));
    let reopened_service = Arc::new(AcquisitionService::new(reopened_store.clone()));
    assert_eq!(
        reopened_store.acquisitions().unwrap().get(&record.id),
        Some(&record)
    );
    assert_eq!(
        reopened_service.consumer_receipt(record.id).unwrap(),
        Some(receipt)
    );
}
