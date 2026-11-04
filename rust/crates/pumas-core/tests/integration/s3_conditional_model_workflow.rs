//! Native conditional-object facade, shared acquisition and actual format bytes.
//! Owned loopback fixtures only; structural publication is not inference evidence.
#![cfg(feature = "s3")]
use pumas_library::{
    acquisition::{
        AcquisitionPhase, AcquisitionRetryPolicy, AcquisitionWorkspace, RevisionStrength,
        S3Addressing, S3ReaderConfig, Sha256Evidence,
    },
    models::{ImportState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, PumasError, S3ConditionalModelImportRequest, S3ModelImportControl,
    S3ModelImportError, S3ModelImportPhase,
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn tensor() -> Vec<u8> {
    let mut header = serde_json::to_vec(&serde_json::json!({
        "weight": {"dtype":"F32", "shape":[1], "data_offsets":[0,4]}
    }))
    .unwrap();
    while !header.len().is_multiple_of(8) {
        header.push(b' ');
    }
    [
        (header.len() as u64).to_le_bytes().as_slice(),
        header.as_slice(),
        1.0_f32.to_le_bytes().as_slice(),
    ]
    .concat()
}
fn gguf() -> Vec<u8> {
    [
        b"GGUF".as_slice(),
        3_u32.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
    ]
    .concat()
}

// 0: complete both responses; 1: hold HEAD; 2: hold GET after its headers.
async fn run(filename: &str, bytes: Vec<u8>, wrong_hash: bool, cancel: Option<u8>, success: bool) {
    run_etag(filename, bytes, wrong_hash, cancel, success, "\"selected\"").await;
}

async fn run_etag(
    filename: &str,
    bytes: Vec<u8>,
    wrong_hash: bool,
    cancel: Option<u8>,
    success: bool,
    etag: &str,
) {
    let weak_http_etag = etag.starts_with("W/");
    let etag = etag.to_owned();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let served = bytes.clone();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let stall = cancel.unwrap_or(0);
    let response_etag = etag.clone();
    let server = tokio::spawn(async move {
        let mut started = Some(started);
        let mut requests = Vec::new();
        for head in [true, false] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 16 * 1024);
                request.push(socket.read_u8().await.unwrap());
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with(if head {
                "HEAD /fixture/object"
            } else {
                "GET /fixture/object"
            }));
            assert!(!request.contains("versionId="));
            if !head {
                let lower = request.to_ascii_lowercase();
                assert!(lower.contains("\r\nif-match: \"selected\"\r\n"));
                assert!(lower.contains(&format!("\r\nrange: bytes=0-{}\r\n", served.len() - 1)));
            }
            requests.push(request);
            if stall == 1 && head {
                started.take().unwrap().send(()).unwrap();
                let mut byte = [0];
                assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
                break;
            }
            let headers = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nETag: {response_etag}\r\nx-amz-version-id: null\r\n{}Connection: close\r\n\r\n",
                if head {"200 OK"} else {"206 Partial Content"}, served.len(),
                if head {String::new()} else {format!("Content-Range: bytes 0-{}/{}\r\n", served.len()-1, served.len())}
            );
            socket.write_all(headers.as_bytes()).await.unwrap();
            if !head {
                if stall == 2 {
                    started.take().unwrap().send(()).unwrap();
                    let mut byte = [0];
                    assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
                    break;
                }
                socket.write_all(&served).await.unwrap();
            }
            socket.shutdown().await.unwrap();
            if head && weak_http_etag {
                break;
            }
        }
        requests
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
    let control = S3ModelImportControl::new();
    let progress = control.subscribe();
    if cancel == Some(0) {
        assert!(control.cancel());
    }
    let request = S3ConditionalModelImportRequest {
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
        source_key: "object".into(),
        expected_sha256: Sha256Evidence::new(
            "fixture.sha256",
            if wrong_hash {
                "0".repeat(64)
            } else {
                hex::encode(Sha256::digest(&bytes))
            },
        )
        .unwrap(),
        import: ModelImportSpec {
            path: filename.into(),
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
    };
    let operation_id = request.operation_id;
    let spec_payload = serde_json::to_value(&request.import).unwrap();
    let task = api.import_s3_conditional_model(request, control.clone());
    let result = if matches!(cancel, Some(1 | 2)) {
        let canceller = async {
            tokio::time::timeout(Duration::from_secs(5), waiting)
                .await
                .unwrap()
                .unwrap();
            assert!(control.cancel());
        };
        tokio::join!(task, canceller).0
    } else {
        task.await
    };
    if success {
        let result = result.unwrap();
        assert!(result.success);
        let id = result.model_id.unwrap();
        assert_eq!(
            std::fs::read(api.model_library().library_root().join(&id).join(filename)).unwrap(),
            bytes
        );
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Completed);
        assert!(!control.cancel());
        let record = api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        assert_eq!(
            record.manifest.source().revision().strength(),
            RevisionStrength::Weak
        );
        // Weak classifies mutable provenance. The wire validator is still a
        // strong quoted HTTP ETag, and integrity still requires full SHA-256.
        assert_eq!(
            record.manifest.source().revision().authority(),
            "s3.conditional_etag_size"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(record.manifest.source().revision().value())
                .unwrap(),
            serde_json::json!([etag, bytes.len()])
        );
        assert_eq!(record.files[0].sha256, hex::encode(Sha256::digest(&bytes)));
        assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
        assert_eq!(record.demand.operation, operation_id.to_string());
        assert_eq!(record.demand.consumer, "model.s3.workflow");
        let receipt = api
            .acquisition()
            .consumer_receipt(record.id)
            .unwrap()
            .unwrap();
        assert_eq!(receipt.payload, spec_payload);
        assert_eq!(receipt.manifest, record.manifest);
        assert_eq!(receipt.verified_files, record.files);
        let target = api.model_library().library_root().join(&id);
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
            api.model_library()
                .get_effective_metadata(&id)
                .unwrap()
                .unwrap()
                .import_state,
            Some(ImportState::Ready)
        );
        let row = api.model_library().index().get(&id).unwrap().unwrap();
        assert_eq!(row.id, id);
        assert_eq!(row.metadata["import_state"], "ready");
    } else {
        assert!(result.is_err());
        assert!(api.model_library().list_models().await.unwrap().is_empty());
        if cancel.is_some() {
            assert!(matches!(
                result,
                Err(S3ModelImportError::Operation(PumasError::DownloadCancelled))
            ));
            assert_eq!(progress.borrow().phase, S3ModelImportPhase::Cancelled);
        } else {
            assert_eq!(progress.borrow().phase, S3ModelImportPhase::Failed);
        }
        if weak_http_etag {
            assert!(matches!(result, Err(S3ModelImportError::Source(_))));
            assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
        }
        if !wrong_hash && cancel.is_none() && !weak_http_etag {
            // Arbitrary-byte acquisition succeeded. Model qualification refused
            // publication and retained the exact issued use for reconciliation.
            let records = api.acquisition().store().acquisitions().unwrap();
            assert_eq!(records.len(), 1);
            let record = records.values().next().unwrap();
            assert_eq!(record.demand.operation, operation_id.to_string());
            assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
            assert_eq!(record.files.len(), 1);
            assert_eq!(record.files[0].bytes, bytes.len() as u64);
            assert_eq!(record.files[0].sha256, hex::encode(Sha256::digest(&bytes)));
            let receipt = api
                .acquisition()
                .consumer_receipt(record.id)
                .unwrap()
                .unwrap();
            assert_eq!(receipt.verified_files, record.files);
            assert_eq!(receipt.manifest, record.manifest);
            assert_eq!(receipt.payload, spec_payload);
        }
        if wrong_hash || cancel.is_some() {
            for record in api
                .acquisition()
                .store()
                .acquisitions()
                .unwrap()
                .into_values()
            {
                assert!(api
                    .acquisition()
                    .consumer_receipt(record.id)
                    .unwrap()
                    .is_none());
            }
        }
    }
    if cancel == Some(0) {
        assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
        server.abort();
        assert!(server.await.unwrap_err().is_cancelled());
    } else {
        let requests = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            requests.len(),
            if cancel == Some(1) || weak_http_etag {
                1
            } else {
                2
            }
        );
    }
    api.shutdown_instance().await.unwrap();
}

#[tokio::test]
async fn conditional_actual_safetensors_and_gguf_share_publication() {
    run("weights.safetensors", tensor(), false, None, true).await;
    run("weights.gguf", gguf(), false, None, true).await;
}
#[tokio::test]
async fn conditional_digest_and_model_qualification_are_separate() {
    run("weights.safetensors", tensor(), true, None, false).await;
    run(
        "weights.safetensors",
        b"not safetensors".to_vec(),
        false,
        None,
        false,
    )
    .await;
    run(
        "unknown.blob",
        b"arbitrary data".to_vec(),
        false,
        None,
        false,
    )
    .await;
    // A valid shard cannot substitute for its selected package config/index/other shards.
    run(
        "model-00001-of-00002.safetensors",
        tensor(),
        false,
        None,
        false,
    )
    .await;
}
#[tokio::test]
async fn conditional_cancellation_drains_selection_and_transfer() {
    for phase in [0, 1, 2] {
        run("weights.safetensors", tensor(), false, Some(phase), false).await;
    }
}

#[tokio::test]
async fn conditional_http_weak_etag_is_rejected_before_acquisition() {
    run_etag(
        "weights.safetensors",
        tensor(),
        false,
        None,
        false,
        "W/\"selected\"",
    )
    .await;
}
