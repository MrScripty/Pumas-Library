//! Native source-facing composition; synthetic sources, no account access.
#![cfg(feature = "s3")]
use pumas_library::{
    acquisition::{
        AcquisitionPhase, AcquisitionRetryPolicy, AcquisitionWorkspace, S3Addressing,
        S3Credentials, S3ManifestEntry, S3PrefixLimits, S3Reader, S3ReaderConfig, Sha256Evidence,
    },
    models::{ImportState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, PumasError, S3ModelImportControl, S3ModelImportError, S3ModelImportPhase,
    S3ModelImportRequest,
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
const VERSION: &str = "workflow-v1";
const ACCESS: &str = "workflow-synthetic-access";
const SECRET: &str = "workflow-synthetic-secret";
const TOKEN: &str = "workflow-synthetic-token";
fn gguf() -> Vec<u8> {
    [
        b"GGUF".as_slice(),
        &3_u32.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
    ]
    .concat()
}
fn wire(bytes: &[u8], head: bool) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {}\r\nContent-Length: {}\r\n{}Last-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {VERSION}\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n",
        if head { "200 OK" } else { "206 Partial Content" }, bytes.len(),
        if head { String::new() } else { format!("Content-Range: bytes 0-{}/{}\r\n",bytes.len()-1,bytes.len()) }).into_bytes();
    if !head {
        out.extend(bytes);
    }
    out
}
struct Fixture {
    endpoint: String,
    task: JoinHandle<Vec<String>>,
}
impl Fixture {
    async fn serve(
        responses: Vec<Vec<u8>>,
        unfinished: bool,
    ) -> (Self, tokio::sync::oneshot::Receiver<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (started, waiting) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut requests = vec![];
            let mut started = Some(started);
            let count = responses.len();
            for (i, response) in responses.into_iter().enumerate() {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = vec![];
                while !bytes.ends_with(b"\r\n\r\n") {
                    assert!(bytes.len() < 16 * 1024);
                    bytes.push(socket.read_u8().await.unwrap());
                }
                requests.push(String::from_utf8(bytes).unwrap());
                socket.write_all(&response).await.unwrap();
                if unfinished && i + 1 == count {
                    started.take().unwrap().send(()).unwrap();
                    let mut byte = [0];
                    assert_eq!(
                        socket.read(&mut byte).await.unwrap(),
                        0,
                        "drain must close stalled response"
                    );
                }
                socket.shutdown().await.unwrap();
            }
            requests
        });
        (Self { endpoint, task }, waiting)
    }
    async fn finish(mut self) -> Vec<String> {
        tokio::time::timeout(Duration::from_secs(10), &mut self.task)
            .await
            .unwrap()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn setup_api(root: &Path) -> PumasApi {
    PumasApi::builder(root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}
async fn close(api: &PumasApi) {
    api.shutdown_intent().await.unwrap();
    api.shutdown_downloads().await.unwrap();
    api.shutdown_acquisition().await.unwrap();
}
fn request(endpoint: &str, stage: &Path, aux: bool) -> S3ModelImportRequest {
    std::fs::create_dir(stage.join("stage")).unwrap();
    let mut entries = vec![entry("weights.gguf", &gguf())];
    if aux {
        entries.push(entry("config/tokenizer_config.json", b"{}"));
    }
    S3ModelImportRequest {
        operation_id: uuid::Uuid::new_v4(),
        source: S3ReaderConfig {
            endpoint: endpoint.into(),
            region: "fixture-region".into(),
            bucket: "fixture-bucket".into(),
            addressing: S3Addressing::Path,
            allow_http: true,
            operation_timeout: Duration::from_secs(5),
        },
        credentials: None,
        entries,
        import: ModelImportSpec {
            path: "weights.gguf".into(),
            family: "fixture".into(),
            official_name: "Workflow GGUF".into(),
            model_type: Some("llm".into()),
            repo_id: None,
            subtype: None,
            tags: None,
            security_acknowledged: None,
        },
        workspace: AcquisitionWorkspace::from_reserved_directory(
            stage,
            Path::new("stage"),
            Arc::new(()),
            || Ok(()),
        )
        .unwrap(),
        retry: AcquisitionRetryPolicy {
            attempts: Some(2),
            elapsed: Duration::from_secs(10),
            backoff: RetryConfig::default(),
        },
    }
}
fn entry(path: &str, bytes: &[u8]) -> S3ManifestEntry {
    S3ManifestEntry {
        source_key: format!("models/{path}"),
        version: VERSION.into(),
        logical_path: path.into(),
        expected_sha256: Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(bytes)))
            .unwrap(),
    }
}
async fn assert_published(api: &PumasApi, id: &str, aux: bool) {
    assert!(api.get_model(id).await.unwrap().is_some());
    let library = api.model_library();
    assert_eq!(
        library
            .get_effective_metadata(id)
            .unwrap()
            .unwrap()
            .import_state,
        Some(ImportState::Ready)
    );
    let target = library.library_root().join(id);
    assert_eq!(std::fs::read(target.join("weights.gguf")).unwrap(), gguf());
    if aux {
        assert_eq!(
            std::fs::read(target.join("config/tokenizer_config.json")).unwrap(),
            b"{}"
        );
    }
    let records = api.acquisition().store().acquisitions().unwrap();
    assert_eq!(records.len(), 1);
    let record = records.values().next().unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    let saved = serde_json::to_string(record).unwrap();
    let consumer = api
        .acquisition()
        .open_consumer("model.s3.workflow")
        .unwrap();
    let receipt = consumer.completion_receipt(record).unwrap().unwrap();
    assert_eq!(receipt.owner, "model.s3.workflow");
    assert_eq!(receipt.demand, record.demand);
    assert_eq!(receipt.manifest, record.manifest);
    assert_eq!(receipt.payload["path"], "weights.gguf");
    consumer.shutdown().await.unwrap();
    let publication =
        std::fs::read_to_string(target.join(".pumas_import_publication.json")).unwrap();
    for secret in [ACCESS, SECRET, TOKEN] {
        assert!(!saved.contains(secret));
        assert!(!publication.contains(secret));
    }
}
#[tokio::test]
async fn public_single_and_bundle_workflows_publish_ready_and_settle_exact_receipts() {
    for aux in [false, true] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let api = setup_api(root.path()).await;
        let bytes = gguf();
        let responses = if aux {
            vec![
                wire(b"{}", true),
                wire(&bytes, true),
                wire(b"{}", false),
                wire(&bytes, false),
            ]
        } else {
            vec![wire(&bytes, true), wire(&bytes, false)]
        };
        let (fixture, _) = Fixture::serve(responses, false).await;
        let request = request(&fixture.endpoint, stage.path(), aux);
        let operation = request.operation_id.to_string();
        let control = S3ModelImportControl::new();
        let progress = control.subscribe();
        let bundle_progress = control.subscribe_bundle();
        let result = api.import_s3_model(request, control.clone()).await.unwrap();
        assert!(result.success);
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Completed);
        let bundle = *bundle_progress.borrow();
        assert_eq!(bundle.files_total, if aux { 2 } else { 1 });
        assert_eq!(bundle.files_acquired, bundle.files_total);
        assert_eq!(
            bundle.bytes_acquired,
            bytes.len() as u64 + if aux { 2 } else { 0 }
        );
        assert_eq!(bundle.total_expected_bytes, Some(bundle.bytes_acquired));
        assert_eq!(bundle.file_index, None);
        assert_eq!(bundle.phase, S3ModelImportPhase::Completed);
        assert!(!control.cancel());
        let id = result.model_id.unwrap();
        assert_published(&api, &id, aux).await;
        let records = api.acquisition().store().acquisitions().unwrap();
        assert_eq!(records.values().next().unwrap().demand.operation, operation);
        let captured = fixture.finish().await;
        assert_eq!(captured.len(), if aux { 4 } else { 2 });
        assert!(captured.iter().all(|r| !r.contains("authorization:")));
        close(&api).await;
        drop(api);
        let cold = setup_api(root.path()).await;
        assert_published(&cold, &id, aux).await;
        close(&cold).await;
    }
}

#[tokio::test]
async fn bounded_prefix_pins_feed_existing_bundle_import_and_cold_exact_receipts() {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let api = setup_api(root.path()).await;
    let bytes = gguf();
    let page = |key: &str, size: usize, truncated: bool, continuation: &str| {
        let body = format!("<ListBucketResult><Name>fixture-bucket</Name><Prefix>models/</Prefix><MaxKeys>1</MaxKeys><KeyCount>1</KeyCount><IsTruncated>{truncated}</IsTruncated>{continuation}<Contents><Key>{key}</Key><ETag>\"selected\"</ETag><Size>{size}</Size></Contents></ListBucketResult>");
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
    };
    let versioned = |payload: &[u8], head: bool, version: &str| {
        String::from_utf8(wire(payload, head))
            .unwrap()
            .replacen(
                &format!("x-amz-version-id: {VERSION}\r\n"),
                &format!("x-amz-version-id: {version}\r\n"),
                1,
            )
            .into_bytes()
    };
    let responses = vec![
        page(
            "models/config/tokenizer_config.json",
            2,
            true,
            "<NextContinuationToken>workflow+next/=</NextContinuationToken>",
        ),
        page(
            "models/weights.gguf",
            bytes.len(),
            false,
            "<ContinuationToken>workflow+next/=</ContinuationToken>",
        ),
        versioned(b"{}", true, "config-v1"),
        versioned(&bytes, true, "weights-v2"),
        versioned(b"{}", true, "config-v1"),
        versioned(&bytes, true, "weights-v2"),
        versioned(b"{}", false, "config-v1"),
        versioned(&bytes, false, "weights-v2"),
    ];
    let (fixture, _) = Fixture::serve(responses, false).await;
    let reader = S3Reader::new(S3ReaderConfig {
        endpoint: fixture.endpoint.clone(),
        region: "fixture-region".into(),
        bucket: "fixture-bucket".into(),
        addressing: S3Addressing::Path,
        allow_http: true,
        operation_timeout: Duration::from_secs(5),
    })
    .unwrap();
    let listing = reader
        .enumerate_prefix(
            "models/",
            S3PrefixLimits {
                page_size: 1,
                max_pages: 2,
                max_objects: 2,
                max_page_bytes: 4096,
                max_total_bytes: 8192,
            },
        )
        .await
        .unwrap();
    assert_eq!(listing.pages(), 2);
    assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(stage.path()).unwrap().count(), 0);
    let entries = listing
        .objects()
        .iter()
        .map(|object| {
            let (path, payload) = match object.key() {
                "models/config/tokenizer_config.json" => {
                    ("config/tokenizer_config.json", b"{}".as_slice())
                }
                "models/weights.gguf" => ("weights.gguf", bytes.as_slice()),
                _ => panic!("fixture returned an unapproved logical member"),
            };
            object.manifest_entry(path.into(), entry(path, payload).expected_sha256)
        })
        .collect();
    drop(reader);
    let mut request = request(&fixture.endpoint, stage.path(), true);
    request.entries = entries;
    let result = api
        .import_s3_model(request, S3ModelImportControl::new())
        .await
        .unwrap();
    let id = result.model_id.unwrap();
    assert_published(&api, &id, true).await;
    let records = api.acquisition().store().acquisitions().unwrap();
    let manifest = records.values().next().unwrap().manifest.clone();
    let saved = serde_json::to_string(records.values().next().unwrap()).unwrap();
    assert!(!saved.contains("workflow+next/="));
    assert!(!saved.contains("workflow%2Bnext%2F%3D"));
    assert_eq!(
        manifest
            .files()
            .iter()
            .map(|file| file.source_key().to_owned())
            .collect::<Vec<_>>(),
        vec![
            serde_json::to_string(&("models/config/tokenizer_config.json", "config-v1")).unwrap(),
            serde_json::to_string(&("models/weights.gguf", "weights-v2")).unwrap(),
        ]
    );
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 8);
    assert!(requests
        .iter()
        .all(|request| !request.contains("authorization:")));
    assert!(requests[1].contains("continuation-token=workflow%2Bnext%2F%3D"));
    assert!(requests[2..4]
        .iter()
        .all(|request| request.starts_with("HEAD ")
            && request.contains("if-match: \"selected\"")
            && !request.contains("versionId=")));
    assert!(requests[4].contains("versionId=config-v1"));
    assert!(requests[5].contains("versionId=weights-v2"));
    assert!(requests[6].contains("versionId=config-v1"));
    assert!(requests[7].contains("versionId=weights-v2"));
    close(&api).await;
    drop(api);
    let cold = setup_api(root.path()).await;
    assert_published(&cold, &id, true).await;
    assert_eq!(
        cold.acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .manifest,
        manifest
    );
    close(&cold).await;
}
#[tokio::test]
async fn invalid_inputs_and_plaintext_credentials_fail_before_network_or_admission() {
    let root = tempfile::TempDir::new().unwrap();
    let api = setup_api(root.path()).await;
    for case in 0..4 {
        let stage = tempfile::TempDir::new().unwrap();
        let mut request = request("http://127.0.0.1:1", stage.path(), false);
        match case {
            0 => request.import.path = "unselected.gguf".into(),
            1 => request.retry.attempts = None,
            2 => request.entries[0].version = String::new(),
            _ => {
                request.credentials = Some(
                    S3Credentials::new(ACCESS.into(), SECRET.into(), Some(TOKEN.into())).unwrap(),
                )
            }
        }
        let control = S3ModelImportControl::new();
        let progress = control.subscribe();
        let error = api.import_s3_model(request, control).await.unwrap_err();
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Failed);
        for value in [ACCESS, SECRET, TOKEN] {
            assert!(!format!("{error:?} {error}").contains(value));
        }
        assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
    }
    close(&api).await;
}
#[tokio::test]
async fn cancellation_during_head_and_get_drains_without_publication() {
    for during_get in [false, true] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let api = setup_api(root.path()).await;
        let bytes = gguf();
        let responses = if during_get {
            let mut partial = wire(&bytes, false);
            partial.truncate(partial.len() - bytes.len() + 1);
            vec![wire(&bytes, true), partial]
        } else {
            vec![]
        };
        let responses = if during_get {
            responses
        } else {
            vec![Vec::new()]
        };
        let (fixture, started) = Fixture::serve(responses, true).await;
        let request = request(&fixture.endpoint, stage.path(), false);
        let control = S3ModelImportControl::new();
        let mut progress = control.subscribe();
        let work = api.import_s3_model(request, control.clone());
        tokio::pin!(work);
        tokio::select! {result=&mut work=>panic!("premature result: {result:?}"), _=started=>{}}
        if during_get {
            while progress.borrow().downloaded_for_current_file == 0 {
                tokio::select! {
                    result = &mut work => panic!("premature result: {result:?}"),
                    changed = progress.changed() => changed.unwrap(),
                }
            }
            assert_eq!(progress.borrow().downloaded_for_current_file, 1);
        }
        assert_eq!(
            progress.borrow().phase,
            if during_get {
                S3ModelImportPhase::Acquiring
            } else {
                S3ModelImportPhase::Selecting
            }
        );
        assert!(control.cancel());
        let error = tokio::time::timeout(Duration::from_secs(10), &mut work)
            .await
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            error,
            S3ModelImportError::Operation(PumasError::DownloadCancelled)
        ));
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Cancelled);
        assert_eq!(fixture.finish().await.len(), if during_get { 2 } else { 1 });
        assert!(api.model_library().list_models().await.unwrap().is_empty());
        assert!(api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .values()
            .all(|r| !matches!(r.phase, AcquisitionPhase::Adopted { .. })));
        close(&api).await;
    }
}
#[tokio::test]
async fn dropped_selection_waiter_reports_interruption_and_shared_shutdown_drains() {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let api = setup_api(root.path()).await;
    let (fixture, started) = Fixture::serve(vec![vec![]], true).await;
    let request = request(&fixture.endpoint, stage.path(), false);
    let control = S3ModelImportControl::new();
    let progress = control.subscribe();
    {
        let work = api.import_s3_model(request, control.clone());
        tokio::pin!(work);
        tokio::select! {result=&mut work=>panic!("premature result: {result:?}"), _=started=>{}}
    }
    assert_eq!(progress.borrow().phase, S3ModelImportPhase::Interrupted);
    assert!(!control.cancel());
    close(&api).await;
    assert_eq!(fixture.finish().await.len(), 1);
    assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
}

// OpenSSL's certificate-file setting is isolated to the child. Verification remains
// enabled; the repository's synthetic localhost CA is trusted in that child.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn authenticated_https_workflow_uses_explicit_credentials_and_persists_no_secrets() {
    const MARKER: &str = "PUMAS_S3_WORKFLOW_TLS_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let config = tempfile::TempDir::new().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "authenticated_https_workflow_uses_explicit_credentials_and_persists_no_secrets",
                "--nocapture",
            ])
            .env(MARKER, "1")
            .env("XDG_CONFIG_HOME", config.path())
            .env(
                "SSL_CERT_FILE",
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/http-tls/localhost.pem"),
            )
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let api = setup_api(root.path()).await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!(
        "https://localhost:{}",
        listener.local_addr().unwrap().port()
    );
    // Bound fixture waiting independently of the library's operation timeout.
    listener.set_nonblocking(true).unwrap();
    let thread = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let identity = native_tls::Identity::from_pkcs12(
            include_bytes!("fixtures/http-tls/localhost.p12"),
            "fixture",
        )
        .unwrap();
        let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
        let mut captured = vec![];
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        for head in [true, false] {
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "TLS fixture accept timeout"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut socket = acceptor.accept(socket).unwrap();
            let mut bytes = vec![];
            while !bytes.ends_with(b"\r\n\r\n") {
                assert!(bytes.len() < 16 * 1024);
                let mut b = [0];
                socket.read_exact(&mut b).unwrap();
                bytes.push(b[0]);
            }
            captured.push(String::from_utf8(bytes).unwrap());
            socket.write_all(&wire(&gguf(), head)).unwrap();
            socket.flush().unwrap();
        }
        captured
    });
    let mut request = request(&endpoint, stage.path(), false);
    request.source.allow_http = false;
    request.credentials =
        Some(S3Credentials::new(ACCESS.into(), SECRET.into(), Some(TOKEN.into())).unwrap());
    let control = S3ModelImportControl::new();
    let progress = control.subscribe();
    let result = api.import_s3_model(request, control).await;
    let captured = thread.join().unwrap();
    let result = result.unwrap();
    assert_eq!(progress.borrow().phase, S3ModelImportPhase::Completed);
    assert_published(&api, &result.model_id.unwrap(), false).await;
    assert_eq!(captured.len(), 2);
    for request in captured {
        assert!(request.contains(ACCESS));
        assert!(request.contains(&format!("x-amz-security-token: {TOKEN}\r\n")));
        assert!(request.contains("SignedHeaders="));
        assert!(!request.contains(SECRET));
    }
    // Walk all owned output/staging/cache files, not just the receipt serializer.
    for tree in [root.path(), stage.path()] {
        for entry in walkdir::WalkDir::new(tree) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() {
                let bytes = std::fs::read(entry.path()).unwrap();
                for value in [ACCESS, SECRET, TOKEN] {
                    assert!(
                        !bytes.windows(value.len()).any(|w| w == value.as_bytes()),
                        "secret persisted in {}",
                        entry.path().display()
                    );
                }
            }
        }
    }
    close(&api).await;
}

#[tokio::test]
async fn digest_refusal_and_pre_cancelled_requests_never_publish() {
    let root = tempfile::TempDir::new().unwrap();
    let api = setup_api(root.path()).await;
    let stage = tempfile::TempDir::new().unwrap();
    let control = S3ModelImportControl::new();
    assert!(control.cancel());
    assert!(!control.cancel());
    let error = api
        .import_s3_model(
            request("http://127.0.0.1:1", stage.path(), false),
            control.clone(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        S3ModelImportError::Operation(PumasError::DownloadCancelled)
    ));
    assert_eq!(
        control.subscribe().borrow().phase,
        S3ModelImportPhase::Cancelled
    );
    assert!(api.acquisition().store().acquisitions().unwrap().is_empty());

    let stage = tempfile::TempDir::new().unwrap();
    let bytes = gguf();
    let (fixture, _) = Fixture::serve(vec![wire(&bytes, true), wire(&bytes, false)], false).await;
    let mut request = request(&fixture.endpoint, stage.path(), false);
    request.entries[0].expected_sha256 =
        Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(b"different"))).unwrap();
    let control = S3ModelImportControl::new();
    assert!(api.import_s3_model(request, control.clone()).await.is_err());
    assert_eq!(
        control.subscribe().borrow().phase,
        S3ModelImportPhase::Failed
    );
    assert_eq!(fixture.finish().await.len(), 2);
    assert!(api.model_library().list_models().await.unwrap().is_empty());
    assert!(api
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .values()
        .all(|record| !matches!(
            record.phase,
            AcquisitionPhase::Using { .. } | AcquisitionPhase::Adopted { .. }
        )));
    close(&api).await;
}

#[tokio::test]
async fn reserved_bundle_destinations_fail_before_resolution_and_corrected_request_publishes() {
    let root = tempfile::TempDir::new().unwrap();
    let api = setup_api(root.path()).await;
    let bytes = gguf();
    let (fixture, _) = Fixture::serve(
        vec![
            wire(b"{}", true),
            wire(&bytes, true),
            wire(b"{}", false),
            wire(&bytes, false),
        ],
        false,
    )
    .await;
    let store_path = root.path().join("launcher-data/downloads.json");
    let before = std::fs::read(&store_path).ok();
    for path in [
        "metadata.json",
        "overrides.json",
        "metadata.json/data.json",
        "OVERRIDES.JSON/data.json",
        "_metadata_.json",
        "metadata.json.bak/data.json",
        ".pumas_import_publication.json/data.json",
    ] {
        let stage = tempfile::TempDir::new().unwrap();
        let mut bad = request(&fixture.endpoint, stage.path(), true);
        bad.entries[1].logical_path = path.into();
        let result = api.import_s3_model(bad, S3ModelImportControl::new()).await;
        assert!(result.is_err(), "reserved payload accepted: {path}");
        assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
        assert_eq!(std::fs::read(&store_path).ok(), before);
        assert_eq!(
            std::fs::read_dir(stage.path().join("stage"))
                .unwrap()
                .count(),
            0
        );
        assert!(api.model_library().list_models().await.unwrap().is_empty());
    }
    let stage = tempfile::TempDir::new().unwrap();
    let corrected = request(&fixture.endpoint, stage.path(), true);
    let result = api
        .import_s3_model(corrected, S3ModelImportControl::new())
        .await
        .unwrap();
    assert!(result.success);
    assert_published(&api, result.model_id.as_deref().unwrap(), true).await;
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 4, "rejected names performed source IO");
    assert!(requests
        .iter()
        .all(|request| !request.contains("metadata.json") && !request.contains("overrides.json")));
    close(&api).await;
}
