//! Public HTTP JSON-RPC qualification with a real, controlled HF source.
use super::tests::start_test_server;
use crate::handlers::test_support::build_test_api_with_hf_fixture;
use pumas_library::model_library::test_support::{
    admit_paused_download, HfLoopbackFixture, PersistedDownload,
};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const ID: &str = "rpc-live-resume";
const MODEL_ID: &str = "llm/acme/http-resume-model";
const ARTIFACT: &str = "acme--model__files_971f5ccbb554";
const SHA256: &str = "69cb86ffffe1039003092edf0dc9415a36b5016eae5f93b7300f97e7fe67dcd9";
const WATCHDOG: Duration = Duration::from_secs(15);

struct Rpc {
    client: reqwest::Client,
    address: std::net::SocketAddr,
    next_id: u64,
}

impl Rpc {
    fn new(address: std::net::SocketAddr) -> Self {
        Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            address,
            next_id: 1,
        }
    }

    async fn call(&mut self, method: &str) -> Value {
        let request_id = self.next_id;
        self.next_id += 1;
        let response: Value = tokio::time::timeout(WATCHDOG, async {
            self.client.post(format!("http://{}/rpc", self.address))
                .json(&json!({"jsonrpc":"2.0", "id":request_id, "method":method, "params":{"downloadId":ID}}))
                .send().await.unwrap().error_for_status().unwrap().json().await.unwrap()
        }).await.expect("public RPC response watchdog");
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], request_id);
        assert!(
            response.get("error").is_none(),
            "unexpected RPC error: {response}"
        );
        response["result"].clone()
    }

    async fn status(&mut self, status: &str, bytes: Option<u64>) -> Value {
        tokio::time::timeout(WATCHDOG, async {
            loop {
                let value = self.call("get_model_download_status").await;
                assert_eq!(
                    value["success"], true,
                    "tracked download remains publicly observable"
                );
                assert_eq!(value["downloadId"], ID);
                assert_eq!(value["repoId"], "acme/model");
                assert_eq!(value["selectedArtifactId"], ARTIFACT);
                assert_eq!(value["totalBytes"], 24);
                if value["status"] == status
                    && bytes.is_none_or(|bytes| value["downloadedBytes"] == bytes)
                {
                    return value;
                }
                assert!(
                    value["status"] != "error" || status == "error",
                    "unexpected download failure: {value}"
                );
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("public lifecycle observation watchdog")
    }
}

fn document(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join("launcher-data/downloads.json")).unwrap())
        .unwrap()
}

fn download(document: &Value) -> &Value {
    document["downloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["download_id"] == ID)
        .unwrap()
}

async fn request(socket: &mut tokio::net::TcpStream) -> String {
    tokio::time::timeout(WATCHDOG, async {
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            assert!(headers.len() < 4096);
            headers.push(socket.read_u8().await.unwrap());
        }
        let request = String::from_utf8(headers).unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /acme/model/resolve/main/weights.gguf http/1.1"));
        assert!(
            !request.contains("authorization:"),
            "fixture must never load ambient credentials"
        );
        request
    })
    .await
    .expect("source request watchdog")
}

struct ImportRelease(Option<mpsc::Sender<()>>);
impl ImportRelease {
    fn release(&mut self) {
        let _ = self.0.take().unwrap().send(());
    }
}
impl Drop for ImportRelease {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[test]
fn hf_rpc_fixture_rejects_ambient_origins_before_construction() {
    assert!(HfLoopbackFixture::parse("http://127.0.0.1:12345").is_ok());
    assert!(HfLoopbackFixture::parse("http://[::1]:12345").is_ok());
    for origin in [
        "https://127.0.0.1:12345",
        "http://localhost:12345",
        "http://example.invalid:12345",
        "http://0.0.0.0:12345",
        "http://127.0.0.1",
        "http://127.0.0.1:0",
        "http://user:secret@127.0.0.1:12345",
        "http://127.0.0.1:12345/path",
        "http://127.0.0.1:12345?grant=secret",
        "http://127.0.0.1:12345#fragment",
    ] {
        assert!(HfLoopbackFixture::parse(origin).is_err());
    }
}

#[tokio::test]
async fn retained_partial_rpc_http_resume_interrupts_retries_and_awaits_durable_import() {
    tokio::time::timeout(Duration::from_secs(60), exercise_resume())
        .await
        .expect("whole qualification watchdog");
}

async fn exercise_resume() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    std::fs::create_dir_all(root.join("launcher-data")).unwrap();
    let destination = root.join("shared-resources/models/llm/acme/http-resume-model");
    std::fs::create_dir_all(&destination).unwrap();
    let mut bytes = b"GGUF".to_vec();
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    let partial = destination.join("weights.gguf.part");
    let payload = destination.join("weights.gguf");
    let marker = destination.join(".pumas_download");
    let sentinel = destination.join("unrelated.bin");
    std::fs::write(&partial, &bytes[..8]).unwrap();
    std::fs::write(
        &marker,
        br#"{"repo_id":"acme/model","files":["weights.gguf"]}"#,
    )
    .unwrap();
    std::fs::write(&sentinel, b"unrelated retained bytes").unwrap();
    let snapshot: PersistedDownload = serde_json::from_value(json!({
        "download_id":ID, "repo_id":"acme/model", "revision":null, "filename":"weights.gguf",
        "filenames":["weights.gguf"], "dest_dir":destination, "total_bytes":24, "status":"paused",
        "download_request":{"repo_id":"acme/model", "family":"acme", "official_name":"HTTP Resume Model", "model_type":"llm", "filenames":["weights.gguf"]},
        "created_at":"2026-10-04T00:00:00Z", "known_sha256":SHA256
    })).unwrap();
    admit_paused_download(root, &snapshot).unwrap();
    let admitted = document(root)["queue_admissions"][ID].clone();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let source = HfLoopbackFixture::parse(&origin).unwrap();
    let (first_requested, first_seen) = tokio::sync::oneshot::channel();
    let (interrupt, interrupted) = tokio::sync::oneshot::channel::<()>();
    let (cold_requested, cold_seen) = tokio::sync::oneshot::channel();
    let (close_cold, cold_held) = tokio::sync::oneshot::channel::<()>();
    let (range_requested, range_seen) = tokio::sync::oneshot::channel();
    let (finish_range, range_held) = tokio::sync::oneshot::channel::<()>();
    let source_bytes = bytes.clone();
    let source_server = tokio::spawn(async move {
        let mut requests = Vec::new();
        let (mut first, _) = tokio::time::timeout(WATCHDOG, listener.accept())
            .await
            .unwrap()
            .unwrap();
        let headers = request(&mut first).await;
        // A cold partial has no live representation checkpoint: never append
        // merely because a digest-backed selection authorizes explicit resume.
        assert!(!headers.contains("range:"));
        requests.push(headers);
        first.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 24\r\nETag: \"rpc-v1\"\r\nConnection: close\r\n\r\n").await.unwrap();
        first.write_all(&source_bytes[..12]).await.unwrap();
        first_requested.send(()).unwrap();
        tokio::time::timeout(WATCHDOG, interrupted)
            .await
            .unwrap()
            .unwrap();
        first.shutdown().await.unwrap();
        drop(first);
        let (mut retry, _) = tokio::time::timeout(WATCHDOG, listener.accept())
            .await
            .unwrap()
            .unwrap();
        let headers = request(&mut retry).await;
        assert!(headers.contains("range: bytes=12-"));
        assert!(headers.contains("if-match: \"rpc-v1\""));
        requests.push(headers);
        retry
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        drop(retry);

        let (mut cold, _) = tokio::time::timeout(WATCHDOG, listener.accept())
            .await
            .unwrap()
            .unwrap();
        let headers = request(&mut cold).await;
        assert!(
            !headers.contains("range:"),
            "fresh owner must reacquire representation evidence"
        );
        requests.push(headers);
        cold.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 24\r\nETag: \"rpc-v1\"\r\nConnection: close\r\n\r\n").await.unwrap();
        cold.write_all(&source_bytes[..18]).await.unwrap();
        cold_requested.send(()).unwrap();
        tokio::time::timeout(WATCHDOG, cold_held)
            .await
            .unwrap()
            .unwrap();
        drop(cold);

        let (mut successor, _) = tokio::time::timeout(WATCHDOG, listener.accept())
            .await
            .unwrap()
            .unwrap();
        let headers = request(&mut successor).await;
        // RPC resume reconstructs its workspace grant. The retained prefix is
        // progress, not authority to reuse the predecessor's HTTP checkpoint.
        assert!(!headers.contains("range:"));
        assert!(!headers.contains("if-match:"));
        requests.push(headers);
        successor.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 24\r\nETag: \"rpc-v1\"\r\nConnection: close\r\n\r\n").await.unwrap();
        successor.write_all(&source_bytes[..18]).await.unwrap();
        successor.shutdown().await.unwrap();
        drop(successor);

        // This retry retains the same live workspace and proves a successful
        // range continuation, not only the earlier terminal-refusal request.
        let (mut continuation, _) = tokio::time::timeout(WATCHDOG, listener.accept())
            .await
            .unwrap()
            .unwrap();
        let headers = request(&mut continuation).await;
        assert!(headers.contains("range: bytes=18-"));
        assert!(headers.contains("if-match: \"rpc-v1\""));
        requests.push(headers);
        range_requested.send(()).unwrap();
        tokio::time::timeout(WATCHDOG, range_held)
            .await
            .unwrap()
            .unwrap();
        continuation.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 6\r\nContent-Range: bytes 18-23/24\r\nETag: \"rpc-v1\"\r\nConnection: close\r\n\r\n").await.unwrap();
        continuation.write_all(&source_bytes[18..]).await.unwrap();
        requests
    });

    let api = build_test_api_with_hf_fixture(root, source.clone()).await;
    let server = start_test_server(api, root).await.unwrap();
    let mut rpc = Rpc::new(server.addr());
    rpc.status("paused", Some(8)).await;
    assert_eq!(rpc.call("resume_model_download").await["success"], true);
    tokio::time::timeout(WATCHDOG, first_seen)
        .await
        .unwrap()
        .unwrap();
    rpc.status("downloading", Some(12)).await;
    assert_eq!(rpc.call("resume_model_download").await["success"], false);
    assert_eq!(std::fs::read(&partial).unwrap(), bytes[..12]);
    assert!(!payload.exists());
    interrupt.send(()).unwrap();
    let failed = rpc.status("error", Some(12)).await;
    assert_eq!(
        failed["error"],
        "The model download did not complete successfully."
    );
    let failed_again = rpc.call("get_model_download_status").await;
    assert_eq!(failed_again, failed);
    let failed_document = document(root);
    assert_eq!(download(&failed_document)["status"], "error");
    assert_eq!(failed_document["queue_admissions"][ID], admitted);
    assert!(failed_document["consumer_receipts"]
        .as_object()
        .unwrap()
        .is_empty());
    assert_eq!(std::fs::read(&partial).unwrap(), bytes[..12]);
    assert!(!payload.exists());
    tokio::time::timeout(WATCHDOG, server.shutdown())
        .await
        .unwrap()
        .unwrap();
    drop(server);
    drop(rpc);

    let api = build_test_api_with_hf_fixture(root, source).await;
    let library = api.model_library().clone();
    let acquisition = api.acquisition().store().clone();
    let model_id = library.get_model_id(&destination).unwrap();
    let (import_entered, import_seen) = tokio::sync::oneshot::channel();
    let import_entered = Mutex::new(Some(import_entered));
    let (release_import, import_held) = mpsc::channel();
    let import_held = Mutex::new(import_held);
    let mut release = ImportRelease(Some(release_import));
    let import_destination = destination.clone();
    library.set_metadata_write_notifier(Some(Arc::new(move |_| {
        if import_destination.join(".pumas_download").exists()
            || !import_destination.join("weights.gguf").is_file()
        {
            return;
        }
        if let Some(entered) = import_entered.lock().unwrap().take() {
            let _ = entered.send(());
            let _ = import_held
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(30));
        }
    })));
    let server = start_test_server(api, root).await.unwrap();
    let mut rpc = Rpc::new(server.addr());
    // Reconstruction preserves a durable terminal Error. Only interrupted
    // Queued/Downloading snapshots project to Paused; explicit resume is allowed.
    rpc.status("error", Some(12)).await;
    assert_eq!(document(root)["queue_admissions"][ID], admitted);
    assert_eq!(rpc.call("resume_model_download").await["success"], true);
    tokio::time::timeout(WATCHDOG, cold_seen)
        .await
        .unwrap()
        .unwrap();
    rpc.status("downloading", Some(18)).await;
    assert_eq!(rpc.call("resume_model_download").await["success"], false);
    assert_eq!(rpc.call("pause_model_download").await["success"], true);
    rpc.status("paused", Some(18)).await;
    assert_eq!(std::fs::read(&partial).unwrap(), bytes[..18]);
    assert!(!payload.exists());
    let paused_document = document(root);
    assert_eq!(download(&paused_document)["status"], "paused");
    assert_eq!(paused_document["queue_admissions"][ID], admitted);
    close_cold.send(()).unwrap();
    assert_eq!(rpc.call("resume_model_download").await["success"], true);
    tokio::time::timeout(WATCHDOG, range_seen)
        .await
        .unwrap()
        .unwrap();
    rpc.status("downloading", Some(18)).await;
    assert_eq!(rpc.call("resume_model_download").await["success"], false);
    finish_range.send(()).unwrap();
    tokio::time::timeout(WATCHDOG, import_seen)
        .await
        .unwrap()
        .unwrap();
    rpc.status("downloading", Some(24)).await;
    assert_eq!(rpc.call("resume_model_download").await["success"], false);
    assert_eq!(std::fs::read(&payload).unwrap(), bytes);
    assert!(!partial.exists());
    // The notifier observes metadata projection before adoption settlement.
    // Publication alone must not expose Completed or release queue ownership.
    assert!(library.load_metadata(&destination).unwrap().is_some());
    let using_document = document(root);
    assert_eq!(using_document["queue_admissions"][ID], admitted);
    assert!(using_document["consumer_receipts"]
        .as_object()
        .unwrap()
        .is_empty());
    let using = acquisition.acquisitions().unwrap();
    assert_eq!(using.len(), 1);
    let using = using.values().next().unwrap();
    assert!(matches!(
        using.phase,
        pumas_library::acquisition::AcquisitionPhase::Using { .. }
    ));
    let acquisition_id = using.id.to_string();
    release.release();
    let completed = rpc.status("completed", Some(24)).await;
    assert_eq!(rpc.call("get_model_download_status").await, completed);
    assert_eq!(rpc.call("resume_model_download").await["success"], false);
    library.set_metadata_write_notifier(None);
    tokio::time::timeout(WATCHDOG, server.shutdown())
        .await
        .unwrap()
        .unwrap();
    drop(server);
    let requests = tokio::time::timeout(WATCHDOG, source_server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        requests.len(),
        5,
        "active/completed repeat resumes must not create extra HTTP attempts"
    );

    let final_document = document(root);
    assert!(final_document["queue_admissions"].get(ID).is_none());
    assert_eq!(final_document["released_queue_admissions"][ID], admitted);
    let receipt = &final_document["consumer_receipts"][&acquisition_id];
    let receipt = if receipt["receipt_kind"] == "pumas.consumer-completion" {
        &receipt["payload"]
    } else {
        receipt
    };
    assert_eq!(receipt["download_id"], ID);
    assert_eq!(receipt["model_id"], MODEL_ID);
    assert_eq!(receipt["verified_files"][0]["sha256"], SHA256);
    assert_eq!(receipt["verified_files"][0]["bytes"], 24);
    let persisted = pumas_library::acquisition::AcquisitionStore::new(&root.join("launcher-data"));
    let final_acquisitions = persisted.acquisitions().unwrap();
    assert!(matches!(
        final_acquisitions[&using.id].phase,
        pumas_library::acquisition::AcquisitionPhase::Adopted { .. }
    ));
    let reopened =
        pumas_library::model_library::ModelLibrary::new(root.join("shared-resources/models"))
            .await
            .unwrap();
    assert_eq!(
        reopened
            .load_metadata(&destination)
            .unwrap()
            .unwrap()
            .repo_id
            .as_deref(),
        Some("acme/model")
    );
    assert!(reopened.index().get(&model_id).unwrap().is_some());
    assert_eq!(std::fs::read(&payload).unwrap(), bytes);
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"unrelated retained bytes"
    );
    assert!(!partial.exists());
    assert!(!marker.exists());
}
