use super::*;
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
use crate::server::{start_server, LoopbackHost};
use serde_json::{json, Value};
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
use std::io::{Read, Write};
const ID: &str = "c3f7d104-1234-4321-abcd-aaaaaaaaaaaa";
const HASH: &str = "a4e5e156ddec27e286f75328784d7106b60a4eb1d246e950a001a3f944fbda99";
const ACCESS: &str = "desktop-auth-synthetic-key";
const SECRET: &str = "desktop-auth-synthetic-secret";
const TOKEN: &str = "desktop-auth-synthetic-token";
fn authenticated_params(endpoint: &str, token: bool) -> Value {
    json!({"source": params(endpoint), "credentials":{"access_key_id":ACCESS,"secret_access_key":SECRET,"session_token":if token {Some(TOKEN)} else {None}}})
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
pub(super) fn gguf() -> Vec<u8> {
    [
        b"GGUF".as_slice(),
        &3_u32.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
    ]
    .concat()
}
pub(super) fn params(endpoint: &str) -> Value {
    json!({"operation_id":ID,"endpoint":endpoint,"region":"fixture-region","bucket":"fixture-bucket","addressing":"path","key":"models/weights.gguf","version_id":"desktop-v1","filename":"weights.gguf","sha256":HASH,"family":"fixture","official_name":"Desktop GGUF"})
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
pub(super) async fn rpc(
    server: &crate::server::ServerHandle,
    method: &str,
    params: Value,
) -> Value {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
        .post(format!("http://{}/rpc", server.addr()))
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
struct Source {
    endpoint: String,
    thread: Option<std::thread::JoinHandle<Vec<String>>>,
    started: tokio::sync::oneshot::Receiver<()>,
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
impl Source {
    fn start(mode: u8) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "https://localhost:{}",
            listener.local_addr().unwrap().port()
        );
        let (started, waiting) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let identity = native_tls::Identity::from_pkcs12(
                include_bytes!("../../../pumas-core/tests/fixtures/http-tls/localhost.p12"),
                "fixture",
            )
            .unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            let mut requests = vec![];
            let mut started = Some(started);
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            for head in [true, false] {
                let socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "TLS source accept deadline"
                            );
                            std::thread::sleep(Duration::from_millis(5));
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
                let mut request = vec![];
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 16 * 1024);
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                requests.push(String::from_utf8(request).unwrap());
                if mode == 3 && !head {
                    let body = format!("{ACCESS} {SECRET} {TOKEN}");
                    write!(socket, "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                    break;
                }
                if mode == 1 && head {
                    started.take().unwrap().send(()).unwrap();
                    let mut byte = [0];
                    assert_eq!(
                        socket.read(&mut byte).unwrap(),
                        0,
                        "cancel/shutdown must close stalled HEAD"
                    );
                    break;
                }
                let bytes = gguf();
                let header=format!("HTTP/1.1 {}\r\nContent-Length: {}\r\n{}Last-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: desktop-v1\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n",if head{"200 OK"}else{"206 Partial Content"},bytes.len(),if head{String::new()}else{format!("Content-Range: bytes 0-{}/{}\r\n",bytes.len()-1,bytes.len())});
                socket.write_all(header.as_bytes()).unwrap();
                if !head {
                    socket
                        .write_all(if mode == 2 { &bytes[..1] } else { &bytes })
                        .unwrap();
                    socket.flush().unwrap();
                    if mode == 2 {
                        started.take().unwrap().send(()).unwrap();
                        let mut byte = [0];
                        assert_eq!(
                            socket.read(&mut byte).unwrap(),
                            0,
                            "cancel/shutdown must close stalled GET"
                        );
                    }
                }
            }
            requests
        });
        Self {
            endpoint,
            thread: Some(thread),
            started: waiting,
        }
    }
    fn finish(mut self) -> Vec<String> {
        self.thread.take().unwrap().join().unwrap()
    }
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
pub(super) async fn terminal(server: &crate::server::ServerHandle) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let result = rpc(server, "get_s3_model_import", json!({"operation_id":ID})).await;
            assert!(result["error"].is_null(), "{result}");
            if result["result"]["status"] == "finished" {
                break result["result"].clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
#[tokio::test]
async fn source_rpc_https_owned_import_cancel_and_shutdown() {
    const MARKER: &str = "PUMAS_S3_DESKTOP_TLS_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let config = tempfile::TempDir::new().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "s3_imports::tests::source_rpc_https_owned_import_cancel_and_shutdown",
                "--nocapture",
            ])
            .env(MARKER, "1")
            .env("XDG_CONFIG_HOME", config.path())
            .env(
                "SSL_CERT_FILE",
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../pumas-core/tests/fixtures/http-tls/localhost.pem"),
            )
            .env("AWS_ACCESS_KEY_ID", "synthetic-desktop-ambient-key")
            .env("AWS_SECRET_ACCESS_KEY", "synthetic-desktop-ambient-secret")
            .env("AWS_SESSION_TOKEN", "synthetic-desktop-ambient-token")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    for (mode, shutdown) in [(0, false), (1, false), (2, false), (1, true)] {
        let root = tempfile::TempDir::new().unwrap();
        let api = pumas_library::PumasApi::builder(root.path())
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let library = api.model_library().clone();
        let acquisition = api.acquisition().clone();
        let server = start_server(
            api,
            LoopbackHost::parse("127.0.0.1").unwrap(),
            0,
            crate::http_transport::HttpShutdownPolicy::default(),
        )
        .await
        .unwrap();
        let mut source = Source::start(mode);
        assert_eq!(
            rpc(&server, "get_s3_model_import", json!({})).await["result"]["status"],
            "idle"
        );
        for (field, invalid) in [("version_id", "null"), ("key", "../weights.gguf")] {
            let mut bad = params(&source.endpoint);
            bad[field] = json!(invalid);
            let refused = rpc(&server, "start_s3_model_import", bad).await;
            assert_eq!(refused["error"]["code"], -32602);
            assert_eq!(
                rpc(&server, "get_s3_model_import", json!({})).await["result"]["status"],
                "idle"
            );
            assert!(acquisition.store().acquisitions().unwrap().is_empty());
            assert!(!root
                .path()
                .join("launcher-data")
                .join(format!(".s3-import-{ID}"))
                .exists());
        }
        let mut invalid = params(&source.endpoint);
        invalid["credentials"] = json!({"secret":"synthetic-secret"});
        let refused = rpc(&server, "start_s3_model_import", invalid).await;
        assert_eq!(refused["error"]["code"], -32602);
        assert!(!refused.to_string().contains("synthetic-secret"));
        let admitted = rpc(&server, "start_s3_model_import", params(&source.endpoint)).await;
        assert_eq!(admitted["result"]["status"], "running", "{admitted}");
        assert_eq!(admitted["result"]["operation_id"], ID);
        // Repeated start never creates another transfer/publication.
        let refused = rpc(&server, "start_s3_model_import", params(&source.endpoint)).await;
        assert_eq!(refused["result"]["status"], "rejected");
        if mode != 0 {
            tokio::time::timeout(Duration::from_secs(5), &mut source.started)
                .await
                .unwrap()
                .unwrap();
            if mode == 2 {
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let snapshot =
                            rpc(&server, "get_s3_model_import", json!({"operation_id":ID})).await;
                        if snapshot["result"]["progress"]["downloaded_for_current_file"] == "1" {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
            }
            if shutdown {
                server.shutdown().await.unwrap();
            } else {
                let cancelled = rpc(
                    &server,
                    "cancel_s3_model_import",
                    json!({"operation_id":ID}),
                )
                .await;
                assert_eq!(cancelled["result"]["accepted"], true, "{cancelled}");
                assert_eq!(terminal(&server).await["result"]["status"], "cancelled");
                server.shutdown().await.unwrap();
            }
            assert!(library.list_models().await.unwrap().is_empty());
        } else {
            let result = terminal(&server).await;
            assert_eq!(result["result"]["status"], "completed", "{result}");
            let id = result["result"]["model_id"].as_str().unwrap().to_owned();
            assert_eq!(
                std::fs::read(library.library_root().join(&id).join("weights.gguf")).unwrap(),
                gguf()
            );
            let public = rpc(&server, "get_models", json!({})).await;
            assert!(public.to_string().contains(&id));
            assert_eq!(
                rpc(
                    &server,
                    "cancel_s3_model_import",
                    json!({"operation_id":ID})
                )
                .await["result"]["accepted"],
                false
            );
            assert_eq!(
                library
                    .get_effective_metadata(&id)
                    .unwrap()
                    .unwrap()
                    .import_state,
                Some(pumas_library::models::ImportState::Ready)
            );
            let records = acquisition.store().acquisitions().unwrap();
            assert_eq!(records.len(), 1);
            let record = records.values().next().unwrap();
            assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
            assert_eq!(record.demand.operation, ID);
            let consumer = acquisition.open_consumer("model.s3.workflow").unwrap();
            let receipt = consumer.completion_receipt(record).unwrap().unwrap();
            assert_eq!(receipt.owner, "model.s3.workflow");
            assert_eq!(receipt.demand, record.demand);
            assert_eq!(receipt.manifest, record.manifest);
            assert_eq!(receipt.payload["path"], "weights.gguf");
            consumer.shutdown().await.unwrap();
            assert!(
                !root
                    .path()
                    .join("launcher-data")
                    .join(format!(".s3-import-{ID}"))
                    .exists(),
                "settled inputs must be cleaned by their reservation owner"
            );
            server.shutdown().await.unwrap();
            let cold = pumas_library::PumasApi::builder(root.path())
                .auto_create_dirs(true)
                .with_hf_client(false)
                .with_process_manager(false)
                .build()
                .await
                .unwrap();
            assert!(cold.get_model(&id).await.unwrap().is_some());
            cold.shutdown_intent().await.unwrap();
            cold.shutdown_downloads().await.unwrap();
            cold.shutdown_acquisition().await.unwrap();
        }
        let requests = source.finish();
        assert_eq!(requests.len(), if mode == 1 { 1 } else { 2 });
        for request in requests {
            assert!(request.contains("versionId=desktop-v1"));
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
            assert!(!request.contains("x-amz-security-token:"));
        }
        for entry in walk_owned_files(root.path()) {
            let data = std::fs::read(entry).unwrap();
            for secret in [
                "synthetic-desktop-ambient-key",
                "synthetic-desktop-ambient-secret",
                "synthetic-desktop-ambient-token",
            ] {
                assert!(!data
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes()));
            }
        }
    }
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
fn walk_owned_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![];
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            paths.extend(walk_owned_files(&entry.path()));
        } else if kind.is_file() {
            paths.push(entry.path());
        }
    }
    paths
}
#[test]
fn source_workspace_reservation_refuses_reuse_and_replaced_children() {
    let root = tempfile::TempDir::new().unwrap();
    let reserved = reserve(root.path().to_owned(), ID).unwrap();
    assert!(reserve(root.path().to_owned(), ID).is_err());
    reserved.validate().unwrap();
    #[cfg(unix)]
    {
        let path = root.path().join(format!(".s3-import-{ID}"));
        std::fs::rename(&path, root.path().join("retained-original")).unwrap();
        std::fs::create_dir(path).unwrap();
        assert!(reserved.acquisition_workspace().is_err());
    }
}
#[test]
fn source_progress_preserves_u64_and_job_admission_is_bounded() {
    let (client, _receiver) = S3Imports::channel();
    let request: S3ImportParams = serde_json::from_value(params("https://source.invalid")).unwrap();
    assert!(matches!(
        client.admit(request.clone()).unwrap(),
        S3ImportOutcome::Running { .. }
    ));
    assert!(matches!(
        client.admit(request).unwrap(),
        S3ImportOutcome::Rejected { .. }
    ));
    assert!(!client.cancel("different-operation").unwrap().accepted);
    assert!(client.cancel(ID).unwrap().accepted);
    assert!(!client.cancel(ID).unwrap().accepted);
    let (_, progress) = watch::channel(pumas_library::S3ModelImportProgress {
        phase: S3ModelImportPhase::Acquiring,
        downloaded_for_current_file: u64::MAX,
    });
    client.0.lock().unwrap().current.as_mut().unwrap().progress = progress;
    match client.snapshot(Some(ID)).unwrap() {
        S3ImportOutcome::Running { progress, .. } => {
            assert_eq!(progress.downloaded_for_current_file, "18446744073709551615");
        }
        _ => panic!("admitted job must remain observable"),
    }
    client.close();
    assert!(matches!(
        client.snapshot(None).unwrap(),
        S3ImportOutcome::Unavailable
    ));
}

#[test]
fn source_authenticated_preflight_keeps_failures_out_of_job_and_snapshots() {
    let (client, mut receiver) = S3Imports::channel();
    for (field, value) in [
        ("endpoint", "http://127.0.0.1:1"),
        ("region", "invalid_region"),
        ("version_id", "null"),
        ("key", "../weights.gguf"),
    ] {
        let mut input = authenticated_params("https://source.invalid", true);
        input["source"][field] = json!(value);
        assert!(matches!(
            client
                .admit_authenticated(serde_json::from_value(input).unwrap())
                .unwrap(),
            S3ImportOutcome::Rejected { .. }
        ));
        assert!(matches!(
            client.snapshot(None).unwrap(),
            S3ImportOutcome::Idle
        ));
        assert!(receiver.try_recv().is_err());
    }
    assert!(matches!(
        client
            .admit_authenticated(
                serde_json::from_value(authenticated_params("https://source.invalid", true))
                    .unwrap()
            )
            .unwrap(),
        S3ImportOutcome::Running { .. }
    ));
    let Job::Import(job) = receiver.try_recv().unwrap() else {
        panic!("expected import job")
    };
    let redacted = format!("{:?}", job.credentials.unwrap());
    let snapshot = serde_json::to_string(&client.snapshot(None).unwrap()).unwrap();
    for secret in [ACCESS, SECRET, TOKEN] {
        assert!(!redacted.contains(secret));
        assert!(!snapshot.contains(secret));
    }
    client.close();
}

#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
#[tokio::test]
async fn source_authenticated_rpc_https_credentials_are_scoped_and_redacted() {
    const MARKER: &str = "PUMAS_S3_AUTH_RPC_TLS_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let config = tempfile::TempDir::new().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "s3_imports::tests::source_authenticated_rpc_https_credentials_are_scoped_and_redacted", "--nocapture"])
            .env(MARKER, "1").env("XDG_CONFIG_HOME", config.path())
            .env("SSL_CERT_FILE", Path::new(env!("CARGO_MANIFEST_DIR")).join("../pumas-core/tests/fixtures/http-tls/localhost.pem"))
            .env("AWS_ACCESS_KEY_ID", "synthetic-unselected-ambient-key")
            .env("AWS_SECRET_ACCESS_KEY", "synthetic-unselected-ambient-secret")
            .output().unwrap();
        for value in [ACCESS, SECRET, TOKEN] {
            assert!(
                !output
                    .stdout
                    .windows(value.len())
                    .any(|bytes| bytes == value.as_bytes()),
                "stdout exposed a credential; output withheld"
            );
            assert!(
                !output
                    .stderr
                    .windows(value.len())
                    .any(|bytes| bytes == value.as_bytes()),
                "stderr exposed a credential; output withheld"
            );
        }
        assert!(
            output.status.success(),
            "credentialed child failed; output withheld"
        );
        return;
    }
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init()
        .unwrap();
    for (mode, token) in [(0, false), (0, true), (1, true), (2, true), (3, true)] {
        let root = tempfile::TempDir::new().unwrap();
        let api = pumas_library::PumasApi::builder(root.path())
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let library = api.model_library().clone();
        let acquisition = api.acquisition().clone();
        let server = start_server(
            api,
            LoopbackHost::parse("127.0.0.1").unwrap(),
            0,
            crate::http_transport::HttpShutdownPolicy::default(),
        )
        .await
        .unwrap();
        let mut source = Source::start(mode);
        let admitted = rpc(
            &server,
            "start_authenticated_s3_model_import",
            authenticated_params(&source.endpoint, token),
        )
        .await;
        assert_eq!(admitted["result"]["status"], "running");
        if mode == 1 || mode == 2 {
            tokio::time::timeout(Duration::from_secs(5), &mut source.started)
                .await
                .unwrap()
                .unwrap();
            let ack = rpc(
                &server,
                "cancel_s3_model_import",
                json!({"operation_id":ID}),
            )
            .await;
            assert_eq!(ack["result"]["accepted"], true);
        }
        let result = terminal(&server).await;
        for value in [ACCESS, SECRET, TOKEN] {
            assert!(!result.to_string().contains(value));
        }
        match mode {
            0 => {
                assert_eq!(result["result"]["status"], "completed");
                let model = result["result"]["model_id"].as_str().unwrap();
                assert_eq!(
                    library
                        .get_effective_metadata(model)
                        .unwrap()
                        .unwrap()
                        .import_state,
                    Some(pumas_library::models::ImportState::Ready)
                );
                assert_eq!(
                    std::fs::read(library.library_root().join(model).join("weights.gguf")).unwrap(),
                    gguf()
                );
                let records = acquisition.store().acquisitions().unwrap();
                assert_eq!(records.len(), 1);
                let record = records.values().next().unwrap();
                let consumer = acquisition.open_consumer("model.s3.workflow").unwrap();
                let receipt = consumer.completion_receipt(record).unwrap().unwrap();
                assert_eq!(receipt.demand, record.demand);
                assert_eq!(receipt.manifest, record.manifest);
                assert_eq!(receipt.owner, "model.s3.workflow");
                let saved = serde_json::to_string(&receipt).unwrap();
                for value in [ACCESS, SECRET, TOKEN] {
                    assert!(!saved.contains(value));
                }
                consumer.shutdown().await.unwrap();
            }
            1 | 2 => {
                assert_eq!(result["result"]["status"], "cancelled");
                assert!(library.list_models().await.unwrap().is_empty());
            }
            _ => {
                assert_eq!(result["result"]["status"], "failed");
                assert!(library.list_models().await.unwrap().is_empty());
            }
        }
        server.shutdown().await.unwrap();
        for request in source.finish() {
            let lower = request.to_ascii_lowercase();
            assert!(lower.contains("authorization: aws4-hmac-sha256"));
            assert!(request.contains(&format!("Credential={ACCESS}/")));
            assert!(!request.contains(SECRET));
            assert!(request.contains("versionId=desktop-v1"));
            assert_eq!(lower.contains("x-amz-security-token:"), token);
            if token {
                assert!(request.contains(TOKEN));
                assert!(lower
                    .split("signedheaders=")
                    .nth(1)
                    .unwrap()
                    .split(',')
                    .next()
                    .unwrap()
                    .contains("x-amz-security-token"));
            }
            assert!(!request.contains("synthetic-unselected-ambient-key"));
        }
        for path in walk_owned_files(root.path()) {
            let bytes = std::fs::read(path).unwrap();
            for value in [ACCESS, SECRET, TOKEN] {
                assert!(!bytes
                    .windows(value.len())
                    .any(|bytes| bytes == value.as_bytes()));
            }
        }
    }
}

#[tokio::test]
async fn source_pin_preflight_matches_reader_structural_refusal_without_io() {
    for (key, version) in [
        ("weights.gguf", "null"),
        ("../weights.gguf", "desktop-v1"),
        ("models/./weights.gguf", "desktop-v1"),
        ("models//weights.gguf", "desktop-v1"),
        ("/weights.gguf", "desktop-v1"),
        ("weights.gguf/", "desktop-v1"),
    ] {
        let mut input = params("https://source.invalid");
        input["key"] = json!(key);
        input["version_id"] = json!(version);
        let request: S3ImportParams = serde_json::from_value(input).unwrap();
        assert!(request.validate().is_err());
        let reader = pumas_library::acquisition::S3Reader::new(source_config(&request)).unwrap();
        let result = reader
            .select(
                key,
                version,
                "weights.gguf",
                Sha256Evidence::new("fixture.sha256", HASH).unwrap(),
            )
            .await;
        assert!(matches!(
            result,
            Err(pumas_library::acquisition::S3ReaderError::Configuration(_))
        ));
    }
    let (client, _receiver) = S3Imports::channel();
    for (field, invalid) in [("version_id", "null"), ("key", "../weights.gguf")] {
        let mut input = params("https://source.invalid");
        input[field] = json!(invalid);
        assert!(matches!(
            client
                .admit(serde_json::from_value(input).unwrap())
                .unwrap(),
            S3ImportOutcome::Rejected { .. }
        ));
        assert!(matches!(
            client.snapshot(None).unwrap(),
            S3ImportOutcome::Idle
        ));
    }
    assert!(matches!(
        client
            .admit(serde_json::from_value(params("https://source.invalid")).unwrap())
            .unwrap(),
        S3ImportOutcome::Running { .. }
    ));
    client.close();
}

fn bundle_params(endpoint: &str) -> Value {
    json!({"operation_id":ID,"endpoint":endpoint,"region":"fixture-region","bucket":"fixture-bucket","addressing":"path",
        "primary_logical_path":"weights.gguf","family":"fixture","official_name":"Desktop Bundle",
        "files":[{"key":"models/shared","version_id":"weights-v1","logical_path":"weights.gguf","sha256":HASH},
          {"key":"models/shared","version_id":"data-v2","logical_path":"config/data.json","sha256":"44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"}]})
}
#[test]
fn source_bundle_structural_preflight_refuses_entire_set_before_admission() {
    let (client, mut receiver) = S3Imports::channel();
    for (field, value) in [
        ("logical_path", json!("../data.json")),
        ("logical_path", json!("WEIGHTS.GGUF")),
        ("logical_path", json!("weights.gguf.part/data.json")),
        ("logical_path", json!("config/run.py")),
        ("logical_path", json!("metadata.json")),
        ("logical_path", json!("overrides.json")),
        ("logical_path", json!("metadata.json/notes.txt")),
        ("logical_path", json!("OVERRIDES.JSON/notes.txt")),
        ("logical_path", json!("_metadata_.json")),
        ("logical_path", json!("metadata.json.bak/notes.txt")),
        (
            "logical_path",
            json!(".pumas_import_publication.json/notes.txt"),
        ),
        ("version_id", json!("null")),
        ("key", json!("../bad")),
        ("sha256", json!("bad")),
    ] {
        let mut input = bundle_params("https://source.invalid");
        input["files"][1][field] = value;
        assert!(matches!(
            client
                .admit_bundle(serde_json::from_value(input).unwrap(), None)
                .unwrap(),
            S3ImportOutcome::Rejected { .. }
        ));
        assert!(matches!(
            client.snapshot(None).unwrap(),
            S3ImportOutcome::Idle
        ));
        assert!(receiver.try_recv().is_err());
    }
    let mut conflicting = bundle_params("https://source.invalid");
    conflicting["files"][1]["version_id"] = json!("weights-v1");
    assert!(matches!(
        client
            .admit_bundle(serde_json::from_value(conflicting).unwrap(), None)
            .unwrap(),
        S3ImportOutcome::Rejected { .. }
    ));
    let mut overbudget = bundle_params("https://source.invalid");
    overbudget["files"]=json!((0..8).map(|i| json!({"key":"models/shared","version_id":"x".repeat(4096),"logical_path":if i==0 {"weights.gguf".to_string()} else {format!("data{i}.json")},"sha256":HASH})).collect::<Vec<_>>());
    assert!(matches!(
        client
            .admit_bundle(serde_json::from_value(overbudget).unwrap(), None)
            .unwrap(),
        S3ImportOutcome::Rejected { .. }
    ));
    assert!(receiver.try_recv().is_err());
    assert!(matches!(
        client
            .admit_bundle(
                serde_json::from_value(bundle_params("https://source.invalid")).unwrap(),
                None
            )
            .unwrap(),
        S3ImportOutcome::Running { .. }
    ));
    let Job::Import(job) = receiver.try_recv().unwrap() else {
        panic!("expected import job")
    };
    assert_eq!(job.entries.len(), 2);
    client.close();
}

#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
struct BundleSource {
    endpoint: String,
    thread: Option<std::thread::JoinHandle<Vec<String>>>,
    started: tokio::sync::oneshot::Receiver<()>,
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
impl BundleSource {
    fn start(mode: u8) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "https://localhost:{}",
            listener.local_addr().unwrap().port()
        );
        let (started, waiting) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let identity = native_tls::Identity::from_pkcs12(
                include_bytes!("../../../pumas-core/tests/fixtures/http-tls/localhost.p12"),
                "fixture",
            )
            .unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            let mut captured = vec![];
            let mut started = Some(started);
            for (step, (head, primary)) in
                [(true, false), (true, true), (false, false), (false, true)]
                    .into_iter()
                    .filter(|(head, primary)| {
                        *head
                            || !((!primary && matches!(mode, 5 | 6 | 8 | 9 | 10))
                                || (*primary && matches!(mode, 7 | 10)))
                    })
                    .enumerate()
            {
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                let socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "bundle source accept deadline"
                            );
                            std::thread::sleep(Duration::from_millis(5));
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
                let mut request = vec![];
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 16 * 1024);
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with(if head { "HEAD " } else { "GET " }));
                assert!(request.contains(if primary {
                    "versionId=weights-v1"
                } else {
                    "versionId=data-v2"
                }));
                assert!(request.contains("/fixture-bucket/models/shared?"));
                captured.push(request);
                if mode == 1 && step == 1 {
                    started.take().unwrap().send(()).unwrap();
                    let mut byte = [0];
                    assert_eq!(socket.read(&mut byte).unwrap(), 0);
                    break;
                }
                if mode == 3 && step == 1 {
                    let body = format!("{ACCESS} {SECRET} {TOKEN}");
                    write!(socket,"HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                    break;
                }
                let bytes = if (primary && matches!(mode, 7 | 10))
                    || (!primary && matches!(mode, 5 | 6 | 8 | 9 | 10))
                {
                    vec![]
                } else if primary {
                    gguf()
                } else {
                    b"{}".to_vec()
                };
                let version = if primary { "weights-v1" } else { "data-v2" };
                write!(socket,"HTTP/1.1 {}\r\nContent-Length: {}\r\n{}Last-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {}\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n",if head {"200 OK"} else {"206 Partial Content"},bytes.len(),if head {String::new()} else {format!("Content-Range: bytes 0-{}/{}\r\n",bytes.len()-1,bytes.len())},version).unwrap();
                if mode == 6 && step == 1 {
                    break;
                }
                if !head {
                    socket
                        .write_all(if matches!(mode, 2 | 8 | 9) && primary {
                            &bytes[..1]
                        } else {
                            &bytes
                        })
                        .unwrap();
                    socket.flush().unwrap();
                    if matches!(mode, 2 | 9) && primary {
                        started.take().unwrap().send(()).unwrap();
                        let mut byte = [0];
                        assert_eq!(socket.read(&mut byte).unwrap(), 0);
                    }
                    if matches!(mode, 4 | 8) {
                        break;
                    }
                }
            }
            captured
        });
        Self {
            endpoint,
            thread: Some(thread),
            started: waiting,
        }
    }
    fn finish(mut self) -> Vec<String> {
        self.thread.take().unwrap().join().unwrap()
    }
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
#[tokio::test]
async fn source_bundle_rpc_https_complete_pins_totals_cancellation_and_redaction() {
    const MARKER: &str = "PUMAS_S3_BUNDLE_RPC_TLS_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let config = tempfile::TempDir::new().unwrap();
        let output=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","s3_imports::tests::source_bundle_rpc_https_complete_pins_totals_cancellation_and_redaction","--nocapture"])
            .env(MARKER,"1").env("XDG_CONFIG_HOME",config.path()).env("SSL_CERT_FILE",Path::new(env!("CARGO_MANIFEST_DIR")).join("../pumas-core/tests/fixtures/http-tls/localhost.pem"))
            .env("AWS_ACCESS_KEY_ID","synthetic-unselected-ambient-key").env("AWS_SECRET_ACCESS_KEY","synthetic-unselected-ambient-secret").output().unwrap();
        for value in [ACCESS, SECRET, TOKEN] {
            for stream in [&output.stdout, &output.stderr] {
                assert!(
                    !stream
                        .windows(value.len())
                        .any(|bytes| bytes == value.as_bytes()),
                    "bundle child exposed credential; output withheld"
                );
            }
        }
        assert!(
            output.status.success(),
            "bundle child failed; output withheld"
        );
        return;
    }
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init()
        .unwrap();
    for (mode, auth, token) in [
        (0, false, false),
        (0, true, false),
        (0, true, true),
        (1, true, true),
        (2, true, true),
        (3, true, true),
        (4, false, false),
        (5, false, false),
        (5, true, true),
        (6, false, false),
        (7, false, false),
        (8, false, false),
        (9, true, true),
        (10, false, false),
    ] {
        let root = tempfile::TempDir::new().unwrap();
        let api = pumas_library::PumasApi::builder(root.path())
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let library = api.model_library().clone();
        let acquisition = api.acquisition().clone();
        let server = start_server(
            api,
            LoopbackHost::parse("127.0.0.1").unwrap(),
            0,
            crate::http_transport::HttpShutdownPolicy::default(),
        )
        .await
        .unwrap();
        let mut source = BundleSource::start(mode);
        let mut input = bundle_params(&source.endpoint);
        // Real RPC refuses invalid complete sets before any stage or source I/O;
        // the corrected same-process request remains admissible afterwards.
        for path in [
            "../bad.json",
            "WEIGHTS.GGUF",
            "weights.gguf.part/data.json",
            "run.py",
            "metadata.json",
            "overrides.json",
            "metadata.json/notes.txt",
            "OVERRIDES.JSON/notes.txt",
            "_metadata_.json",
            "metadata.json.bak/notes.txt",
            ".pumas_import_publication.json/notes.txt",
        ] {
            let mut bad = input.clone();
            bad["files"][1]["logical_path"] = json!(path);
            let denied = rpc(&server, "start_s3_model_bundle_import", bad).await;
            assert_eq!(denied["error"]["code"], -32602);
            assert_eq!(
                rpc(&server, "get_s3_model_import", json!({})).await["result"]["status"],
                "idle"
            );
            assert!(!root
                .path()
                .join(format!("launcher-data/.s3-import-{ID}"))
                .exists());
            assert!(acquisition.store().acquisitions().unwrap().is_empty());
            assert!(library.list_models().await.unwrap().is_empty());
        }
        if mode == 4 {
            input["files"][1]["sha256"] = json!("b".repeat(64));
        }
        if matches!(mode, 5 | 8 | 9 | 10) {
            input["files"][1]["sha256"] =
                json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        }
        if matches!(mode, 7 | 10) {
            input["files"][0]["sha256"] =
                json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        }
        let (method, input) = if auth {
            (
                "start_authenticated_s3_model_bundle_import",
                json!({"source":input,"credentials":{"access_key_id":ACCESS,"secret_access_key":SECRET,"session_token":if token {Some(TOKEN)} else {None}}}),
            )
        } else {
            ("start_s3_model_bundle_import", input)
        };
        assert_eq!(
            rpc(&server, method, input).await["result"]["status"],
            "running"
        );
        if matches!(mode, 1 | 2 | 9) {
            tokio::time::timeout(Duration::from_secs(5), &mut source.started)
                .await
                .unwrap()
                .unwrap();
            if matches!(mode, 2 | 9) {
                let observation = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let value = rpc(
                            &server,
                            "get_s3_model_bundle_import",
                            json!({"operation_id":ID}),
                        )
                        .await;
                        if value["result"]["bundle_progress"]["total_bytes_observed"]
                            == if mode == 9 { "1" } else { "3" }
                        {
                            break value["result"].clone();
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let p = &observation["bundle_progress"];
                assert_eq!(p["bytes_acquired"], if mode == 9 { "0" } else { "2" });
                assert_eq!(p["files_acquired"], 1);
                assert_eq!(p["files_total"], 2);
                assert_eq!(
                    p["total_expected_bytes"],
                    if mode == 9 { "24" } else { "26" }
                );
                assert_eq!(p["file_index"], 1);
                assert_eq!(
                    observation["outcome"]["progress"]["downloaded_for_current_file"],
                    "1"
                );
            }
            assert_eq!(
                rpc(
                    &server,
                    "cancel_s3_model_import",
                    json!({"operation_id":ID})
                )
                .await["result"]["accepted"],
                true
            );
        }
        let result = terminal(&server).await;
        for value in [ACCESS, SECRET, TOKEN] {
            assert!(!result.to_string().contains(value));
        }
        let bundle = rpc(
            &server,
            "get_s3_model_bundle_import",
            json!({"operation_id":ID}),
        )
        .await;
        assert_eq!(bundle["result"]["outcome"], result);
        assert!(bundle["result"]["bundle_progress"].is_null());
        if matches!(mode, 0 | 5) {
            assert_eq!(result["result"]["status"], "completed");
            let model = result["result"]["model_id"].as_str().unwrap();
            assert_eq!(
                library
                    .get_effective_metadata(model)
                    .unwrap()
                    .unwrap()
                    .import_state,
                Some(pumas_library::models::ImportState::Ready)
            );
            let destination = library.library_root().join(model);
            assert_eq!(
                std::fs::read(destination.join("weights.gguf")).unwrap(),
                gguf()
            );
            assert_eq!(
                std::fs::read(destination.join("config/data.json")).unwrap(),
                if mode == 5 {
                    b"".as_slice()
                } else {
                    b"{}".as_slice()
                }
            );
            let records = acquisition.store().acquisitions().unwrap();
            assert_eq!(records.len(), 1);
            let record = records.values().next().unwrap();
            assert_eq!(record.manifest.files().len(), 2);
            assert!(record
                .manifest
                .files()
                .iter()
                .all(|file| file.source_key().starts_with("[\"models/shared\",")));
            let consumer = acquisition.open_consumer("model.s3.workflow").unwrap();
            let receipt = consumer.completion_receipt(record).unwrap().unwrap();
            assert_eq!(receipt.demand, record.demand);
            assert_eq!(receipt.manifest, record.manifest);
            assert_eq!(receipt.verified_files, record.files);
            assert_eq!(record.files[0].bytes, if mode == 5 { 0 } else { 2 });
            assert_eq!(
                record.files[0].sha256,
                record.manifest.files()[0]
                    .expected_sha256()
                    .unwrap()
                    .value()
            );
            assert_eq!(receipt.owner, "model.s3.workflow");
            assert_eq!(receipt.payload["path"], "weights.gguf");
            consumer.shutdown().await.unwrap();
        } else {
            assert_eq!(
                result["result"]["status"],
                if matches!(mode, 1 | 2 | 9) {
                    "cancelled"
                } else {
                    "failed"
                }
            );
            assert_eq!(result["result"]["retained_work"], true);
            assert!(library.list_models().await.unwrap().is_empty());
            if mode == 3 {
                assert!(acquisition.store().acquisitions().unwrap().is_empty());
            }
            if matches!(mode, 6 | 8 | 9) {
                let records = acquisition.store().acquisitions().unwrap();
                assert_eq!(records.len(), 1);
                let record = records.values().next().unwrap();
                assert!(matches!(record.phase, AcquisitionPhase::Transferring));
                assert!(record
                    .manifest
                    .files()
                    .iter()
                    .any(|file| file.expected_size() == Some(0)));
                assert!(record.files.is_empty());
            }
            let replay = rpc(
                &server,
                "start_s3_model_bundle_import",
                bundle_params(&source.endpoint),
            )
            .await;
            assert_eq!(replay["result"]["status"], "rejected");
        }
        let shutdown = server.shutdown().await;
        if matches!(mode, 7 | 10) {
            // Invalid GGUF parsing leaves an observed nested owner failure;
            // shutdown drains custody and must continue reporting that failure.
            assert!(shutdown
                .unwrap_err()
                .to_string()
                .contains("Download shutdown observed 1 failure(s)"));
        } else {
            shutdown.unwrap_or_else(|error| panic!("bundle mode {mode}: {error}"));
        }
        let captured = source.finish();
        assert_eq!(
            captured.len(),
            match mode {
                1 | 3 | 6 | 10 => 2,
                4 | 5 | 7 | 8 | 9 => 3,
                _ => 4,
            }
        );
        for request in captured {
            let lower = request.to_ascii_lowercase();
            assert_eq!(lower.contains("authorization: aws4-hmac-sha256"), auth);
            if request.starts_with("GET ") {
                assert!(lower.contains("\r\nif-match: \"selected\"\r\n"));
                if matches!(mode, 5 | 8 | 9) {
                    assert!(
                        request.contains("versionId=weights-v1"),
                        "empty member issued GET"
                    );
                }
            }
            assert!(!request.contains(SECRET));
            assert!(!request.contains("synthetic-unselected-ambient-key"));
            if auth {
                assert!(request.contains(&format!("Credential={ACCESS}/")));
                assert_eq!(request.contains(TOKEN), token);
                assert_eq!(lower.contains("x-amz-security-token:"), token);
            }
        }
        for path in walk_owned_files(root.path()) {
            let bytes = std::fs::read(path).unwrap();
            for value in [ACCESS, SECRET, TOKEN] {
                assert!(!bytes
                    .windows(value.len())
                    .any(|bytes| bytes == value.as_bytes()));
            }
        }
    }
}

#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
#[tokio::test]
async fn persisted_s3_inspection_cold_read_only_and_live_coexistence() {
    const MARKER: &str = "PUMAS_S3_PERSISTED_INSPECTION_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let config = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "s3_imports::tests::persisted_s3_inspection_cold_read_only_and_live_coexistence",
                "--nocapture",
            ])
            .env(MARKER, "1")
            .env("XDG_CONFIG_HOME", config.path())
            .env(
                "SSL_CERT_FILE",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../pumas-core/tests/fixtures/http-tls/localhost.pem"),
            )
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    async fn terminal_for(server: &crate::server::ServerHandle, id: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let result = rpc(server, "get_s3_model_import", json!({"operation_id":id})).await;
                assert!(result["error"].is_null(), "{result}");
                if result["result"]["status"] == "finished" {
                    break result["result"].clone();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    fn files(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        walk_owned_files(root)
            .into_iter()
            .map(|path| {
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }
    async fn reopen(root: &Path) -> pumas_library::PumasApi {
        pumas_library::PumasApi::builder(root)
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap()
    }
    let root = tempfile::tempdir().unwrap();
    let api = reopen(root.path()).await;
    let server = start_server(
        api,
        LoopbackHost::parse("127.0.0.1").unwrap(),
        0,
        crate::http_transport::HttpShutdownPolicy::default(),
    )
    .await
    .unwrap();
    let source = Source::start(0);
    assert_eq!(
        rpc(&server, "start_s3_model_import", params(&source.endpoint)).await["result"]["status"],
        "running"
    );
    let completed = terminal(&server).await;
    let model_id = completed["result"]["model_id"].as_str().unwrap().to_owned();
    assert_eq!(source.finish().len(), 2);
    server.shutdown().await.unwrap();
    let cold = reopen(root.path()).await;
    let library = cold.model_library().clone();
    let store_path = walk_owned_files(root.path())
        .into_iter()
        .find(|path| path.file_name().unwrap() == "downloads.json")
        .unwrap();
    let original = std::fs::read(&store_path).unwrap();
    let saved: Value = serde_json::from_slice(&original).unwrap();
    let acquisition_id = saved["acquisitions"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .to_owned();
    let receipt_path = library
        .library_root()
        .join(&model_id)
        .join(".pumas-import-receipt.json");
    // The producer filename is obtained from its actual saved output, not assumed.
    let receipt_path = if receipt_path.exists() {
        receipt_path
    } else {
        walk_owned_files(&library.library_root().join(&model_id))
            .into_iter()
            .find(|path| {
                std::fs::read(path)
                    .ok()
                    .and_then(|data| serde_json::from_slice::<Value>(&data).ok())
                    .is_some_and(|value| value.get("acquisition").is_some())
            })
            .unwrap()
    };
    let publication = std::fs::read(&receipt_path).unwrap();
    let server = start_server(
        cold,
        LoopbackHost::parse("127.0.0.1").unwrap(),
        0,
        crate::http_transport::HttpShutdownPolicy::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        rpc(&server, "get_s3_model_import", json!({})).await["result"]["status"],
        "idle"
    );
    // Unrelated and explicit-null model metadata must not spend the bounded
    // publication-candidate budget or hide this one exact recorded binding.
    let indexed_publication = library.index().get(&model_id).unwrap().unwrap();
    for index in 0..256 {
        let mut unrelated = indexed_publication.clone();
        unrelated.id = format!("aaa/legacy/unrelated-{index:03}");
        unrelated.path = unrelated.id.clone();
        unrelated.metadata = if index % 2 == 0 {
            json!({})
        } else {
            json!({"import_publication": null})
        };
        library.index().upsert(&unrelated).unwrap();
    }
    let before = files(root.path());
    let observed = rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"].clone();
    assert_eq!(observed["status"], "complete", "{observed}");
    let row = &observed["imports"][0];
    assert_eq!(observed["imports"].as_array().unwrap().len(), 1);
    assert_eq!(row["operation_id"], ID);
    assert_eq!(row["acquisition_id"], acquisition_id);
    assert_eq!(row["phase"], "adopted");
    assert_eq!(row["receipt_present"], true);
    assert_eq!(row["model_binding"]["model_id"], model_id);
    assert_eq!(row["model_binding"]["publication_state"], "confirmed");
    for _ in 0..3 {
        assert_eq!(
            rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
            observed
        );
    }
    assert_eq!(
        files(root.path()),
        before,
        "inspection must not write persistent files"
    );
    // An indexed claim is only a candidate. Even a malformed non-null claim
    // cannot replace canonical metadata and the exact physical receipt checks.
    for claim in [json!({}), json!(false), json!("malformed claim")] {
        let mut indexed = indexed_publication.clone();
        indexed.metadata["import_publication"] = claim;
        library.index().upsert(&indexed).unwrap();
        let before = files(root.path());
        assert_eq!(
            rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
            observed
        );
        assert_eq!(files(root.path()), before);
    }
    library.index().upsert(&indexed_publication).unwrap();
    let metadata_path = library.library_root().join(&model_id).join("metadata.json");
    let metadata = std::fs::read(&metadata_path).unwrap();
    for malformed in [b"{broken".to_vec(), b"{}".to_vec()] {
        std::fs::write(&metadata_path, &malformed).unwrap();
        let before = files(root.path());
        let result =
            rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"].clone();
        assert_eq!(result["status"], "complete");
        assert!(result["imports"][0]["model_binding"].is_null());
        assert_eq!(files(root.path()), before);
    }
    std::fs::write(&metadata_path, &metadata).unwrap();
    let rejected = rpc(
        &server,
        "inspect_persisted_s3_imports",
        json!({"credentials":{"secret":"synthetic-do-not-reflect"}}),
    )
    .await;
    assert_eq!(rejected["error"]["code"], -32602);
    assert!(!rejected.to_string().contains("synthetic-do-not-reflect"));
    // Missing lock must not be recreated, and missing output does not erase custody.
    let lock = store_path.parent().unwrap().join(".downloads.lock");
    std::fs::remove_file(&lock).unwrap();
    assert_eq!(
        rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
        observed
    );
    assert!(!lock.exists());
    std::fs::remove_file(&receipt_path).unwrap();
    let missing = rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"].clone();
    assert_eq!(missing["imports"][0]["phase"], "adopted");
    assert!(missing["imports"][0]["model_binding"].is_null());
    std::fs::write(&receipt_path, &publication).unwrap();
    // Mutated/foreign recorded publication cannot establish this model binding.
    for field in ["model_id", "acquisition_id", "root"] {
        let mut value: Value = serde_json::from_slice(&publication).unwrap();
        match field {
            "model_id" => value["model_id"] = json!("foreign/model"),
            "acquisition_id" => {
                value["acquisition"]["acquisition_id"] =
                    json!("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")
            }
            _ => value["payload"]["library_root"]["volume"] = json!(0),
        }
        std::fs::write(&receipt_path, serde_json::to_vec(&value).unwrap()).unwrap();
        let before = files(root.path());
        let result =
            rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"].clone();
        assert_eq!(result["status"], "complete");
        assert!(
            result["imports"][0]["model_binding"].is_null(),
            "{field}: {result}"
        );
        assert_eq!(files(root.path()), before);
    }
    std::fs::write(&receipt_path, vec![b' '; 64 * 1024 + 1]).unwrap();
    assert_eq!(
        rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
        json!({"status":"incomplete"})
    );
    let mut pending: Value = serde_json::from_slice(&publication).unwrap();
    pending["state"] = json!("pending");
    std::fs::write(&receipt_path, serde_json::to_vec(&pending).unwrap()).unwrap();
    let before = files(root.path());
    let result = rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"].clone();
    assert_eq!(
        result["imports"][0]["model_binding"]["publication_state"],
        "pending"
    );
    assert_eq!(files(root.path()), before);
    std::fs::write(&receipt_path, &publication).unwrap();
    // Using + confirmed output remains Using: inspection cannot settle or recover a final result.
    let mut using = saved.clone();
    using["acquisitions"][&acquisition_id]["phase"]["state"] = json!("using");
    std::fs::write(&store_path, serde_json::to_vec(&using).unwrap()).unwrap();
    let uncertain = rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"].clone();
    assert_eq!(uncertain["imports"][0]["phase"], "using");
    assert_eq!(
        uncertain["imports"][0]["model_binding"]["model_id"],
        model_id
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&store_path).unwrap()).unwrap(),
        using
    );
    for foreign in ["consumer", "root", "locator"] {
        let mut value = saved.clone();
        let record = &mut value["acquisitions"][&acquisition_id];
        match foreign {
            "consumer" => record["demand"]["consumer"] = json!("unrelated.consumer"),
            "root" => record["workspace"]["root_identity"] = json!("foreign-root"),
            _ => record["workspace"]["relative_target"] = json!("foreign-stage"),
        }
        let record = value["acquisitions"][&acquisition_id].clone();
        let receipt = &mut value["consumer_receipts"][&acquisition_id];
        receipt["owner"] = record["demand"]["consumer"].clone();
        receipt["demand"] = record["demand"].clone();
        receipt["workspace"] = record["workspace"].clone();
        std::fs::write(&store_path, serde_json::to_vec(&value).unwrap()).unwrap();
        let before = files(root.path());
        let result =
            rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"].clone();
        assert_eq!(
            result,
            json!({"status":"complete","imports":[]}),
            "{foreign}: {result}"
        );
        assert_eq!(files(root.path()), before);
    }
    for malformed in [
        b"{broken".to_vec(),
        {
            let mut value = saved.clone();
            value["consumer_receipts"][&acquisition_id]["owner"] =
                json!("synthetic-secret-mismatch");
            serde_json::to_vec(&value).unwrap()
        },
        b"{\"schema_version\":7,\"schema_version\":7}".to_vec(),
    ] {
        std::fs::write(&store_path, &malformed).unwrap();
        let before = files(root.path());
        assert_eq!(
            rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
            json!({"status":"unavailable"})
        );
        assert_eq!(files(root.path()), before);
    }
    for count in [33, 129] {
        let template = saved["acquisitions"][&acquisition_id].clone();
        let mut value = saved.clone();
        value["acquisitions"] = json!({});
        value["consumer_receipts"] = json!({});
        for index in 0..count {
            let id = format!("00000000-0000-0000-0000-{:012x}", index + 1);
            let mut record = template.clone();
            record["id"] = json!(id);
            record["demand"]["operation"] = json!(id);
            record["workspace"]["relative_target"] = json!(format!(".s3-import-{id}"));
            record["phase"] = json!({"state":"transferring"});
            record["files"] = json!([]);
            value["acquisitions"][&id] = record;
        }
        std::fs::write(&store_path, serde_json::to_vec(&value).unwrap()).unwrap();
        let before = files(root.path());
        assert_eq!(
            rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
            json!({"status":"incomplete"}),
            "record cap {count}"
        );
        assert_eq!(files(root.path()), before);
    }
    std::fs::write(&store_path, vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert_eq!(
        rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
        json!({"status":"incomplete"})
    );
    std::fs::write(&store_path, &original).unwrap();
    // An unmarked lookalike is never used to synthesize a record or a phase.
    let orphan = root
        .path()
        .join("launcher-data/.s3-import-aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
    std::fs::create_dir(&orphan).unwrap();
    std::fs::write(orphan.join("foreign.txt"), b"preserve").unwrap();
    assert_eq!(
        rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
        observed
    );
    assert_eq!(
        std::fs::read(orphan.join("foreign.txt")).unwrap(),
        b"preserve"
    );
    // Aggregate read and index-candidate limits refuse complete lists, not partial bindings.
    let template = library.index().get(&model_id).unwrap().unwrap();
    let mut other_publication: Value = serde_json::from_slice(&publication).unwrap();
    other_publication["acquisition"]["acquisition_id"] =
        json!("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
    let mut large_receipt = serde_json::to_vec(&other_publication).unwrap();
    large_receipt.resize(64 * 1024 - 1, b' ');
    let mut candidates = Vec::new();
    for index in 0..17 {
        let mut row = template.clone();
        row.id = format!("aaa/fixture/candidate-{index:03}");
        row.path = library
            .library_root()
            .join(&row.id)
            .to_string_lossy()
            .into_owned();
        let directory = library.library_root().join(&row.id);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(receipt_path.file_name().unwrap()),
            &large_receipt,
        )
        .unwrap();
        library.index().upsert(&row).unwrap();
        candidates.push(row.id);
    }
    assert_eq!(
        rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
        json!({"status":"incomplete"})
    );
    assert_eq!(std::fs::read(&store_path).unwrap(), original);
    for id in candidates {
        library.index().delete(&id).unwrap();
        std::fs::remove_dir_all(library.library_root().join(id)).unwrap();
    }
    let mut candidates = Vec::new();
    for index in 0..128 {
        let mut row = template.clone();
        row.id = format!("aaa/fixture/absent-{index:03}");
        row.path = row.id.clone();
        library.index().upsert(&row).unwrap();
        candidates.push(row.id);
    }
    assert_eq!(
        rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
        json!({"status":"incomplete"})
    );
    for id in candidates {
        library.index().delete(&id).unwrap();
    }
    let mut source = Source::start(2);
    let mut request = params(&source.endpoint);
    let next = "c3f7d104-1234-4321-abcd-bbbbbbbbbbbb";
    request["operation_id"] = json!(next);
    assert_eq!(
        rpc(&server, "start_s3_model_import", request).await["result"]["status"],
        "running"
    );
    tokio::time::timeout(Duration::from_secs(5), &mut source.started)
        .await
        .unwrap()
        .unwrap();
    let live_before = rpc(&server, "get_s3_model_import", json!({})).await;
    let live_observed = rpc(&server, "inspect_persisted_s3_imports", json!({})).await;
    assert_eq!(live_observed["result"]["status"], "complete");
    assert!(live_observed["result"]["imports"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["operation_id"] == next && row["phase"] == "transferring"));
    assert_eq!(
        rpc(&server, "get_s3_model_import", json!({})).await["result"]["operation_id"],
        live_before["result"]["operation_id"]
    );
    assert_eq!(
        rpc(
            &server,
            "cancel_s3_model_import",
            json!({"operation_id":next})
        )
        .await["result"]["accepted"],
        true
    );
    assert_eq!(
        terminal_for(&server, next).await["result"]["status"],
        "cancelled"
    );
    assert_eq!(source.finish().len(), 2);
    // Retained cancellation blocks new live admission; reopening does not resume it.
    server.shutdown().await.unwrap();
    // This observer keeps the old physical store alive even after server shutdown.
    drop(library);
    drop(server);
    let cold = reopen(root.path()).await;
    let library = cold.model_library().clone();
    let server = start_server(
        cold,
        LoopbackHost::parse("127.0.0.1").unwrap(),
        0,
        crate::http_transport::HttpShutdownPolicy::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        rpc(&server, "get_s3_model_import", json!({})).await["result"]["status"],
        "idle"
    );
    // Two genuinely published local fixtures with a conflicting saved binding remain unresolved.
    let source = Source::start(0);
    let mut request = params(&source.endpoint);
    request["operation_id"] = json!("c3f7d104-1234-4321-abcd-cccccccccccc");
    request["family"] = json!("second-fixture");
    request["official_name"] = json!("Second local fixture");
    assert_eq!(
        rpc(&server, "start_s3_model_import", request).await["result"]["status"],
        "running"
    );
    let completed = terminal_for(&server, "c3f7d104-1234-4321-abcd-cccccccccccc").await;
    assert_eq!(completed["result"]["status"], "completed", "{completed}");
    let second_model = completed["result"]["model_id"].as_str().unwrap();
    assert_eq!(source.finish().len(), 2);
    let second_receipt = library
        .library_root()
        .join(second_model)
        .join(receipt_path.file_name().unwrap());
    let mut conflicting: Value =
        serde_json::from_slice(&std::fs::read(&second_receipt).unwrap()).unwrap();
    conflicting["acquisition"] = saved["consumer_receipts"][&acquisition_id].clone();
    std::fs::write(&second_receipt, serde_json::to_vec(&conflicting).unwrap()).unwrap();
    let before = files(root.path());
    assert_eq!(
        rpc(&server, "inspect_persisted_s3_imports", json!({})).await["result"],
        json!({"status":"unavailable"})
    );
    assert_eq!(files(root.path()), before);
    server.shutdown().await.unwrap();
}
