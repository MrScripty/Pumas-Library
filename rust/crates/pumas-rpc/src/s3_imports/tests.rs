use super::*;
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
use crate::server::{start_server, LoopbackHost};
use serde_json::{json, Value};
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
use std::io::{Read, Write};
const ID: &str = "c3f7d104-1234-4321-abcd-aaaaaaaaaaaa";
const HASH: &str = "a4e5e156ddec27e286f75328784d7106b60a4eb1d246e950a001a3f944fbda99";
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
fn gguf() -> Vec<u8> {
    [
        b"GGUF".as_slice(),
        &3_u32.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
    ]
    .concat()
}
fn params(endpoint: &str) -> Value {
    json!({"operation_id":ID,"endpoint":endpoint,"region":"fixture-region","bucket":"fixture-bucket","addressing":"path","key":"models/weights.gguf","version_id":"desktop-v1","filename":"weights.gguf","sha256":HASH,"family":"fixture","official_name":"Desktop GGUF"})
}
#[cfg(all(target_os = "linux", not(feature = "inference-plugins")))]
async fn rpc(server: &crate::server::ServerHandle, method: &str, params: Value) -> Value {
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
async fn terminal(server: &crate::server::ServerHandle) -> Value {
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
