//! Copied publication control writes do not invalidate payload freshness.
//! The hint-only container remains subject to separate all-scope reclassification.
//! Controlled loopback safetensors container; no pretrained model or inference.
#![cfg(feature = "s3")]
use pumas_library::{
    acquisition::{
        AcquisitionPhase, AcquisitionRetryPolicy, AcquisitionWorkspace, S3Addressing,
        S3ManifestEntry, S3ReaderConfig, Sha256Evidence,
    },
    models::{ImportState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, S3ModelImportControl, S3ModelImportRequest,
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::test]
async fn finalized_acquired_publication_has_current_payload_projection() {
    let mut header = serde_json::to_vec(&serde_json::json!({
        "weight":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}
    }))
    .unwrap();
    while !header.len().is_multiple_of(8) {
        header.push(b' ');
    }
    let bytes = [
        (header.len() as u64).to_le_bytes().as_slice(),
        header.as_slice(),
        1.0_f32.to_le_bytes().as_slice(),
    ]
    .concat();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let served = bytes.clone();
    let server = tokio::spawn(async move {
        for head in [true, false] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut req = Vec::new();
            while !req.ends_with(b"\r\n\r\n") {
                assert!(req.len() < 16 * 1024);
                req.push(socket.read_u8().await.unwrap());
            }
            let req = String::from_utf8(req).unwrap();
            assert!(req.starts_with(if head { "HEAD " } else { "GET " }));
            assert!(req.contains("versionId=v1"));
            let headers = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nETag: \"selected\"\r\nx-amz-version-id: v1\r\n{}Connection: close\r\n\r\n",
                if head {"200 OK"} else {"206 Partial Content"}, served.len(),
                if head {String::new()} else {format!("Content-Range: bytes 0-{}/{}\r\n",served.len()-1,served.len())});
            socket.write_all(headers.as_bytes()).await.unwrap();
            if !head {
                socket.write_all(&served).await.unwrap();
            }
            socket.shutdown().await.unwrap();
        }
    });
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(stage.path().join("stage")).unwrap();
    let api = PumasApi::builder(root.path())
        .with_registry(
            pumas_library::registry::LibraryRegistry::open_at(&root.path().join("registry.db"))
                .unwrap(),
        )
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    let id = api
        .import_s3_model(
            S3ModelImportRequest {
                operation_id: uuid::Uuid::new_v4(),
                source: S3ReaderConfig {
                    endpoint,
                    region: "fixture".into(),
                    bucket: "fixture".into(),
                    addressing: S3Addressing::Path,
                    allow_http: true,
                    operation_timeout: Duration::from_secs(5),
                },
                credentials: None,
                entries: vec![S3ManifestEntry {
                    source_key: "object".into(),
                    version: "v1".into(),
                    logical_path: "weights.safetensors".into(),
                    expected_sha256: Sha256Evidence::new(
                        "fixture.sha256",
                        hex::encode(Sha256::digest(&bytes)),
                    )
                    .unwrap(),
                }],
                import: ModelImportSpec {
                    path: "weights.safetensors".into(),
                    family: "fixture".into(),
                    official_name: "Conditional fixture".into(),
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
                    attempts: Some(1),
                    elapsed: Duration::from_secs(5),
                    backoff: RetryConfig::default(),
                },
            },
            S3ModelImportControl::new(),
        )
        .await
        .unwrap()
        .model_id
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    let library = api.model_library();
    let target = library.library_root().join(&id);
    assert_eq!(
        std::fs::read(target.join("weights.safetensors")).unwrap(),
        bytes
    );
    let record = api
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    let receipt = api
        .acquisition()
        .consumer_receipt(record.id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.verified_files, record.files);
    let publication: serde_json::Value = serde_json::from_slice(
        &std::fs::read(target.join(".pumas_import_publication.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(publication["state"], "confirmed");
    assert_eq!(publication["model_id"], id);
    assert_eq!(
        publication["acquisition"],
        serde_json::to_value(receipt).unwrap()
    );
    assert_eq!(
        library
            .get_effective_metadata(&id)
            .unwrap()
            .unwrap()
            .import_state,
        Some(ImportState::Ready)
    );
    let row = library.index().get(&id).unwrap().unwrap();
    assert_eq!(row.metadata["import_state"], "ready");
    // Isolate freshness from the Full API's independently scheduled all-model
    // reclassification, which may relocate this hint-only fixture's identity.
    assert!(library.model_scope_is_current(&target).await.unwrap());
    assert_eq!(
        serde_json::to_value(library.index().get(&id).unwrap().unwrap()).unwrap(),
        serde_json::to_value(row).unwrap()
    );
    api.shutdown_instance().await.unwrap();
}
