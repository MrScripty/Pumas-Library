use crate::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelOutcome, EnsureModelRequest,
    GetEnsureStatusOutcome, ModelEnsureRef, ModelRequirement, ModelSelector, ObservedModelState,
};
use crate::models::PackageArtifactKind;
use crate::PumasApi;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const CHILD_TEST: &str = "intent::desired_crash_tests::intent_desired_upstream_crash_child";
const CHILD_ROOT: &str = "PUMAS_DESIRED_CRASH_ROOT";
const CHILD_URL: &str = "PUMAS_DESIRED_CRASH_URL";
const CHILD_PHASE: &str = "PUMAS_DESIRED_CRASH_PHASE";
const CHILD_ACK: &str = "PUMAS_DESIRED_CRASH_ACK";
const CHILD_EXIT: &str = "PUMAS_DESIRED_CRASH_EXIT";

fn payload() -> Vec<u8> {
    let mut bytes = b"GGUF".to_vec();
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes
}

fn request() -> EnsureModelRequest {
    EnsureModelRequest {
        consumer_key: "crash-consumer".to_string(),
        requirement: ModelRequirement {
            selector: ModelSelector::UpstreamRepository {
                repository_id: "acme/model".to_string(),
                revision: Some("main".to_string()),
            },
            artifact: ArtifactRequirement {
                format: Some(PackageArtifactKind::Gguf),
                quantization: None,
                selected_artifact_id: None,
            },
            acquisition_policy: AcquisitionPolicy::AllowUpstream,
        },
    }
}

struct UpstreamServer {
    base_url: String,
    serve_payload: Arc<AtomicBool>,
    payload_release: Arc<tokio::sync::Notify>,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl UpstreamServer {
    async fn start(serve_payload: bool) -> Self {
        let bytes = payload();
        let sha = hex::encode(Sha256::digest(&bytes));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let serve_payload = Arc::new(AtomicBool::new(serve_payload));
        let payload_release = Arc::new(tokio::sync::Notify::new());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let server_serve = Arc::clone(&serve_payload);
        let server_release = Arc::clone(&payload_release);
        let server_requests = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => {
                        let (mut socket, _) = accepted.unwrap();
                        let bytes = bytes.clone();
                        let sha = sha.clone();
                        let serve_payload = Arc::clone(&server_serve);
                        let payload_release = Arc::clone(&server_release);
                        let requests = Arc::clone(&server_requests);
                        connections.spawn(async move {
                            let mut header = Vec::new();
                            while !header.ends_with(b"\r\n\r\n") {
                                if header.len() >= 8192 {
                                    return;
                                }
                                match socket.read_u8().await {
                                    Ok(byte) => header.push(byte),
                                    Err(_) => return,
                                }
                            }
                            let line = String::from_utf8(header)
                                .unwrap()
                                .lines()
                                .next()
                                .unwrap()
                                .to_string();
                            requests.lock().unwrap().push(line.clone());
                            if line == format!(
                                "GET /acme/model/resolve/{COMMIT}/model.gguf HTTP/1.1"
                            ) {
                                while !serve_payload.load(Ordering::SeqCst) {
                                    payload_release.notified().await;
                                }
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                                    bytes.len()
                                );
                                if socket.write_all(response.as_bytes()).await.is_ok() {
                                    let _ = socket.write_all(&bytes).await;
                                }
                                return;
                            }
                            let body = if line
                                == "GET /api/models/acme/model/revision/main HTTP/1.1"
                                || line
                                    == format!(
                                        "GET /api/models/acme/model/revision/{COMMIT} HTTP/1.1"
                                    )
                            {
                                format!(
                                    r#"{{"modelId":"acme/model","sha":"{COMMIT}","tags":["gguf"]}}"#
                                )
                            } else if line
                                == format!(
                                    "GET /api/models/acme/model/tree/{COMMIT}?recursive=true HTTP/1.1"
                                )
                            {
                                format!(
                                    r#"[{{"path":"model.gguf","type":"file","lfs":{{"oid":"{sha}","size":{}}}}}]"#,
                                    bytes.len()
                                )
                            } else {
                                panic!("unexpected upstream crash-test request: {line}");
                            };
                            let response = format!(
                                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                                body.len()
                            );
                            let _ = socket.write_all(response.as_bytes()).await;
                        });
                    }
                }
            }
            while let Some(result) = connections.join_next().await {
                result.unwrap();
            }
        });
        Self {
            base_url,
            serve_payload,
            payload_release,
            requests,
            stop: Some(stop),
            task,
        }
    }

    fn payload_requests(&self) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|line| line.contains("/resolve/"))
            .count()
    }

    fn release_payload(&self) {
        self.serve_payload.store(true, Ordering::SeqCst);
        self.payload_release.notify_waiters();
    }

    async fn stop(mut self) -> Vec<String> {
        self.release_payload();
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.await.unwrap();
        self.requests.lock().unwrap().clone()
    }
}

async fn open(root: &Path, base_url: &str) -> PumasApi {
    let api = crate::api::intent_acquisition_test_fixture(root, Some(base_url.to_string())).await;
    api.primary()
        .hf_client
        .as_ref()
        .unwrap()
        .restore_persisted_downloads()
        .await
        .unwrap();
    api
}

fn write_marker(path: &Path, reference: &ModelEnsureRef) {
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    serde_json::to_writer(&file, reference).unwrap();
    file.sync_all().unwrap();
}

async fn wait_for_file(path: &Path) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !path.is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "subprocess helper for desired-state crash boundaries"]
async fn intent_desired_upstream_crash_child() {
    let Some(root) = std::env::var_os(CHILD_ROOT).map(PathBuf::from) else {
        return;
    };
    let base_url = std::env::var(CHILD_URL).unwrap();
    let phase = std::env::var(CHILD_PHASE).unwrap();
    let ack = PathBuf::from(std::env::var_os(CHILD_ACK).unwrap());
    let exit = PathBuf::from(std::env::var_os(CHILD_EXIT).unwrap());
    let api = open(&root, &base_url).await;
    let outcome = api.intent().ensure_model(&request()).await.unwrap();
    let EnsureModelOutcome::Accepted { declaration, state } = outcome else {
        panic!("upstream declaration was not accepted: {outcome:?}")
    };
    match phase.as_str() {
        "admitted" => assert!(matches!(state, ObservedModelState::Acquiring { .. })),
        "available" => {
            tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    if matches!(
                        api.intent()
                            .get_ensure_status(&declaration.reference)
                            .await
                            .unwrap(),
                        GetEnsureStatusOutcome::Found {
                            state: ObservedModelState::Available { .. },
                            ..
                        }
                    ) {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
        other => panic!("unknown desired crash phase {other}"),
    }
    write_marker(&ack, &declaration.reference);
    wait_for_file(&exit).await;
    std::process::exit(0);
}

fn spawn_child(
    root: &Path,
    server: &UpstreamServer,
    phase: &str,
    ack: &Path,
    exit: &Path,
) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .arg("--ignored")
        .arg("--exact")
        .arg(CHILD_TEST)
        .arg("--nocapture")
        .env(CHILD_ROOT, root)
        .env(CHILD_URL, &server.base_url)
        .env(CHILD_PHASE, phase)
        .env(CHILD_ACK, ack)
        .env(CHILD_EXIT, exit)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
}

async fn wait_for_ack(child: &mut Child, ack: &Path) -> ModelEnsureRef {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if ack.is_file() {
                return serde_json::from_slice(&std::fs::read(ack).unwrap()).unwrap();
            }
            if let Some(status) = child.try_wait().unwrap() {
                panic!("desired crash helper exited before acknowledgement: {status}");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

async fn exit_child(mut child: Child, exit: &Path) {
    std::fs::write(exit, b"exit").unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(status.success(), "desired crash helper failed: {status}");
}

async fn wait_for_payload_request(server: &UpstreamServer, expected: usize) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while server.payload_requests() < expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn wait_available(api: &PumasApi, reference: &ModelEnsureRef) -> crate::intent::ModelHandle {
    let available = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let GetEnsureStatusOutcome::Found {
                state: ObservedModelState::Available { handle },
                ..
            } = api.intent().get_ensure_status(reference).await.unwrap()
            {
                break handle;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    match available {
        Ok(handle) => handle,
        Err(_) => {
            let status = api.intent().get_ensure_status(reference).await.unwrap();
            panic!("desired crash recovery did not become available: {status:?}")
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admitted_upstream_intent_survives_exit_and_recovers_one_pinned_writer() {
    let root = tempfile::TempDir::new().unwrap();
    let server = UpstreamServer::start(false).await;
    let ack = root.path().join("admitted-ack.json");
    let exit = root.path().join("admitted-exit");
    let mut child = spawn_child(root.path(), &server, "admitted", &ack, &exit);
    let reference = wait_for_ack(&mut child, &ack).await;
    wait_for_payload_request(&server, 1).await;
    exit_child(child, &exit).await;

    server.release_payload();
    let api = open(root.path(), &server.base_url).await;
    let status = api.intent().get_ensure_status(&reference).await.unwrap();
    let GetEnsureStatusOutcome::Found { declaration, state } = status else {
        panic!("durably admitted declaration was lost after exit: {status:?}")
    };
    assert_eq!(declaration.reference, reference);
    assert!(!matches!(state, ObservedModelState::Available { .. }));
    let duplicate = api.intent().ensure_model(&request()).await.unwrap();
    let EnsureModelOutcome::Accepted {
        declaration,
        state: _,
    } = duplicate
    else {
        panic!("recovered declaration was not accepted: {duplicate:?}")
    };
    assert_eq!(declaration.reference, reference);
    let handle = wait_available(&api, &reference).await;
    assert_eq!(handle.identity.model_ref.revision.as_deref(), Some(COMMIT));
    assert!(Path::new(&handle.local_load_path).is_file());
    wait_for_payload_request(&server, 2).await;
    assert_eq!(server.payload_requests(), 2);
    api.shutdown_intent().await.unwrap();
    let requests = server.stop().await;
    assert_eq!(
        requests
            .iter()
            .filter(|line| line.contains("/resolve/"))
            .count(),
        2
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn published_upstream_intent_reopens_available_without_http_or_duplicate_writer() {
    let root = tempfile::TempDir::new().unwrap();
    let server = UpstreamServer::start(true).await;
    let base_url = server.base_url.clone();
    let ack = root.path().join("available-ack.json");
    let exit = root.path().join("available-exit");
    let mut child = spawn_child(root.path(), &server, "available", &ack, &exit);
    let reference = wait_for_ack(&mut child, &ack).await;
    wait_for_payload_request(&server, 1).await;
    exit_child(child, &exit).await;
    let before = server.stop().await;
    assert_eq!(
        before
            .iter()
            .filter(|line| line.contains("/resolve/"))
            .count(),
        1
    );

    let api = open(root.path(), &base_url).await;
    let handle = wait_available(&api, &reference).await;
    assert_eq!(handle.identity.model_ref.revision.as_deref(), Some(COMMIT));
    let duplicate = api.intent().ensure_model(&request()).await.unwrap();
    let EnsureModelOutcome::Accepted { declaration, state } = duplicate else {
        panic!("published declaration was not accepted: {duplicate:?}")
    };
    assert_eq!(declaration.reference, reference);
    assert!(matches!(state, ObservedModelState::Available { .. }));
    assert!(Path::new(&handle.local_load_path).is_file());
    api.shutdown_intent().await.unwrap();
}
