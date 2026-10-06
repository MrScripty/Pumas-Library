//! Known empty versioned objects still require owned byte and receipt verification.
use super::*;

#[tokio::test]
async fn empty_selection_refuses_absence_unknown_length_and_changed_version() {
    for response in [
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        String::from_utf8(head(0))
            .unwrap()
            .replace("Content-Length: 0\r\n", "")
            .into_bytes(),
        String::from_utf8(head(0))
            .unwrap()
            .replace("Content-Length: 0", "Content-Length: invalid")
            .into_bytes(),
        String::from_utf8(head(0))
            .unwrap()
            .replace(VERSION, "different-version")
            .into_bytes(),
    ] {
        let fixture = Fixture::serve(vec![response]).await;
        let result = S3Reader::new(S3ReaderConfig {
            endpoint: fixture.endpoint.clone(),
            region: "fixture-region".into(),
            bucket: "fixture-bucket".into(),
            addressing: S3Addressing::Path,
            allow_http: true,
            operation_timeout: Duration::from_secs(2),
        })
        .unwrap()
        .select(
            "empty.txt",
            VERSION,
            "empty.txt",
            Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(b""))).unwrap(),
        )
        .await;
        assert!(
            result.is_err(),
            "absence or unknown identity/length became empty data"
        );
        let requests = fixture.finish().await;
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("HEAD "));
    }
}

#[tokio::test]
async fn empty_owned_single_file_verifies_digest_and_truncates_untrusted_partial() {
    for scenario in ["valid", "digest", "cancel"] {
        let wrong_digest = scenario == "digest";
        let cancelled = scenario == "cancel";
        let state = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let fixture = Fixture::serve(vec![head(0)]).await;
        let selected =
            selection(&fixture.endpoint, if wrong_digest { b"wrong" } else { b"" }).await;
        assert_eq!(selected.manifest().files()[0].expected_size(), Some(0));
        // Public byte-range reads retain their nonempty-range contract.
        assert!(matches!(
            selected.read_range(0..0, &mut tokio::io::sink()).await,
            Err(pumas_library::acquisition::S3ReaderError::InvalidRange)
        ));
        let owner = workspace(stage.path());
        std::fs::write(
            stage.path().join("stage/weights.gguf.part"),
            b"untrusted retained bytes",
        )
        .unwrap();
        let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            state.path(),
        ))));
        let consumer = service.open_consumer("model.s3").unwrap();
        let result = consumer
            .acquire_s3(
                acquire_request(selected, owner, 1),
                Box::new(Host {
                    control: Arc::new(Control {
                        mode: AtomicU8::new(if cancelled { 2 } else { 0 }),
                        ..Control::default()
                    }),
                    stop_at: None,
                }),
                |acquired| async move {
                    let file = &acquired.record().files[0];
                    assert_eq!(file.bytes, 0);
                    assert_eq!(file.sha256, hex::encode(Sha256::digest(b"")));
                    Ok(((), serde_json::Value::Null))
                },
                |(), _| async { Ok(()) },
            )
            .await;
        assert_eq!(result.is_err(), wrong_digest || cancelled);
        if cancelled {
            assert!(matches!(result, Err(PumasError::DownloadCancelled)));
        }
        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
        let requests = fixture.finish().await;
        assert_eq!(requests.len(), 1, "empty selection issued GET");
        if wrong_digest || cancelled {
            assert!(!stage.path().join("stage/weights.gguf").exists());
            let record = service
                .store()
                .acquisitions()
                .unwrap()
                .into_values()
                .next()
                .unwrap();
            assert_eq!(record.phase, AcquisitionPhase::Transferring);
            assert!(record.files.is_empty());
        } else {
            assert_eq!(
                std::fs::read(stage.path().join("stage/weights.gguf")).unwrap(),
                b""
            );
        }
    }
}
