use super::*;
use crate::models::PackageArtifactKind;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn minimal_gguf() -> Vec<u8> {
    let mut bytes = b"GGUF".to_vec();
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes
}

fn upstream_requirement(quantization: Option<&str>) -> ModelRequirement {
    ModelRequirement {
        selector: ModelSelector::UpstreamRepository {
            repository_id: "acme/model".to_string(),
            revision: None,
        },
        artifact: ArtifactRequirement {
            format: Some(PackageArtifactKind::Gguf),
            quantization: quantization.map(str::to_string),
            selected_artifact_id: None,
        },
        acquisition_policy: AcquisitionPolicy::AllowUpstream,
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        assert!(bytes.len() < 8192);
        bytes.push(socket.read_u8().await.unwrap());
    }
    String::from_utf8(bytes).unwrap()
}

async fn serve_json(socket: &mut tokio::net::TcpStream, body: &str) {
    socket
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
}

async fn acquisition_fixture(
    root: &TempDir,
    filename: &'static str,
) -> (
    crate::PumasApi,
    Arc<tokio::sync::Notify>,
    Arc<AtomicBool>,
    AcquisitionServer,
) {
    acquisition_fixture_server(root, filename).await
}

struct AcquisitionServer {
    requests: Arc<std::sync::Mutex<Vec<String>>>,
    stop: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl AcquisitionServer {
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    async fn stop(self) -> Vec<String> {
        let _ = self.stop.send(());
        self.task.await.unwrap();
        let requests = self.requests.lock().unwrap().clone();
        requests
    }
}

async fn acquisition_fixture_server(
    root: &TempDir,
    filename: &'static str,
) -> (
    crate::PumasApi,
    Arc<tokio::sync::Notify>,
    Arc<AtomicBool>,
    AcquisitionServer,
) {
    let payload = minimal_gguf();
    let sha = hex::encode(Sha256::digest(&payload));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let release = Arc::new(tokio::sync::Notify::new());
    let branch_moved = Arc::new(AtomicBool::new(false));
    let server_release = release.clone();
    let server_branch_moved = branch_moved.clone();
    let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let server_requests = requests.clone();
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut handlers = tokio::task::JoinSet::new();
        loop {
            let accepted = tokio::select! {
                accepted = listener.accept() => Some(accepted.unwrap()),
                _ = &mut stopped => None,
            };
            let Some((mut socket, _)) = accepted else {
                break;
            };
            let payload = payload.clone();
            let sha = sha.clone();
            let server_release = server_release.clone();
            let server_branch_moved = server_branch_moved.clone();
            let server_requests = server_requests.clone();
            handlers.spawn(async move {
                let request = read_request(&mut socket).await;
                let line = request.lines().next().unwrap().to_string();
                server_requests.lock().unwrap().push(line.clone());
                if line == "GET /api/models/acme/model/revision/main HTTP/1.1" {
                    let commit = if server_branch_moved.load(Ordering::SeqCst) {
                        "89abcdef0123456789abcdef0123456789abcdef"
                    } else {
                        COMMIT
                    };
                    serve_json(
                        &mut socket,
                        &format!(r#"{{"modelId":"acme/model","sha":"{commit}","tags":["gguf"]}}"#),
                    )
                    .await;
                } else if line
                    == format!("GET /api/models/acme/model/tree/{COMMIT}?recursive=true HTTP/1.1")
                {
                    serve_json(
                        &mut socket,
                        &format!(
                            r#"[{{"path":"{filename}","type":"file","lfs":{{"oid":"{sha}","size":{}}}}}]"#,
                            payload.len()
                        ),
                    )
                    .await;
                } else if line == format!("GET /api/models/acme/model/revision/{COMMIT} HTTP/1.1") {
                    serve_json(
                        &mut socket,
                        &format!(r#"{{"modelId":"acme/model","sha":"{COMMIT}","tags":["gguf"]}}"#),
                    )
                    .await;
                } else if line == format!("GET /acme/model/resolve/{COMMIT}/{filename} HTTP/1.1") {
                    server_release.notified().await;
                    socket
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                                payload.len()
                            )
                            .as_bytes(),
                        )
                        .await
                        .unwrap();
                    socket.write_all(&payload).await.unwrap();
                } else {
                    panic!("unexpected acquisition request: {line}");
                }
            });
        }
        while let Some(request) = handlers.join_next().await {
            request.unwrap();
        }
    });
    let api = crate::api::intent_acquisition_test_fixture(root.path(), Some(base_url)).await;
    (
        api,
        release,
        branch_moved,
        AcquisitionServer {
            requests,
            stop,
            task: server,
        },
    )
}

#[tokio::test]
async fn public_intent_acquires_then_resolves_available_by_stable_requirement() {
    let root = TempDir::new().unwrap();
    let (api, release, _branch_moved, server) = acquisition_fixture(&root, "model.gguf").await;
    let first = api
        .intent()
        .get_model(&upstream_requirement(None))
        .await
        .unwrap();
    let ObservedModelState::Acquiring {
        resolved_requirement,
        download_hint,
        ..
    } = first
    else {
        panic!("expected managed acquisition")
    };
    assert!(download_hint.is_some());
    let ModelSelector::LocalModel { model_ref } = &resolved_requirement.selector else {
        panic!("pending state must return a stable local model identity")
    };
    assert_eq!(model_ref.revision.as_deref(), Some(COMMIT));
    assert!(model_ref.selected_artifact_id.is_some());
    assert!(matches!(
        api.intent()
            .get_model_status(&resolved_requirement)
            .await
            .unwrap(),
        ObservedModelState::Acquiring { .. }
    ));

    let mut wrong_artifact = resolved_requirement.clone();
    let ModelSelector::LocalModel { model_ref } = &mut wrong_artifact.selector else {
        unreachable!()
    };
    model_ref.selected_artifact_id = None;
    wrong_artifact.artifact.selected_artifact_id = Some("different-artifact".to_string());
    assert!(
        !matches!(
            api.intent()
                .get_model_status(&wrong_artifact)
                .await
                .unwrap(),
            ObservedModelState::Acquiring { .. }
        ),
        "an explicit different artifact must not observe this acquisition"
    );
    assert!(
        !matches!(
            api.intent().get_model(&wrong_artifact).await.unwrap(),
            ObservedModelState::Acquiring { .. }
        ),
        "get must not replace the requested artifact with the active artifact"
    );

    release.notify_one();
    let available = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let observed = api
                .intent()
                .get_model_status(&resolved_requirement)
                .await
                .unwrap();
            if matches!(observed, ObservedModelState::Available { .. }) {
                break observed;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(available, ObservedModelState::Available { .. }));
    assert_eq!(server.stop().await.len(), 4);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn two_consumer_ensures_share_one_download_and_release_is_scoped() {
    let root = TempDir::new().unwrap();
    let (api, release, branch_moved, server) =
        acquisition_fixture_server(&root, "model.gguf").await;
    let first = api
        .intent()
        .ensure_model(&EnsureModelRequest {
            consumer_key: "consumer-a".to_string(),
            requirement: upstream_requirement(None),
        })
        .await
        .unwrap();
    let EnsureModelOutcome::Accepted {
        declaration: first_declaration,
        state: first_state,
    } = first
    else {
        panic!("first declaration must be accepted")
    };
    assert!(matches!(first_state, ObservedModelState::Acquiring { .. }));

    let second = api
        .intent()
        .ensure_model(&EnsureModelRequest {
            consumer_key: "consumer-b".to_string(),
            requirement: upstream_requirement(None),
        })
        .await
        .unwrap();
    let EnsureModelOutcome::Accepted {
        declaration: second_declaration,
        state: second_state,
    } = second
    else {
        panic!("second declaration must be accepted")
    };
    assert!(matches!(second_state, ObservedModelState::Acquiring { .. }));
    assert_eq!(
        first_declaration.resolved_requirement,
        second_declaration.resolved_requirement
    );
    assert_eq!(
        api.primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await
            .len(),
        1
    );

    assert!(matches!(
        api.intent()
            .release_model(&first_declaration.reference)
            .await
            .unwrap(),
        ReleaseModelOutcome::Released { .. }
    ));
    assert!(matches!(
        api.intent()
            .get_ensure_status(&second_declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Found {
            state: ObservedModelState::Acquiring { .. },
            ..
        }
    ));

    release.notify_one();
    let local_path = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if let GetEnsureStatusOutcome::Found {
                state: ObservedModelState::Available { handle },
                ..
            } = api
                .intent()
                .get_ensure_status(&second_declaration.reference)
                .await
                .unwrap()
            {
                break handle.local_load_path;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let snapshot = api
                .primary()
                .hf_client
                .as_ref()
                .unwrap()
                .intent_download_snapshot()
                .await;
            if snapshot
                .downloads
                .iter()
                .any(|item| item.status == crate::models::DownloadStatus::Completed)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    std::fs::remove_file(&local_path).unwrap();
    branch_moved.store(true, Ordering::SeqCst);
    let repaired = api
        .intent()
        .ensure_model(&EnsureModelRequest {
            consumer_key: "consumer-b".to_string(),
            requirement: upstream_requirement(None),
        })
        .await
        .unwrap();
    let EnsureModelOutcome::Accepted { declaration, state } = repaired else {
        panic!("duplicate retained declaration must be accepted")
    };
    assert_eq!(declaration.reference, second_declaration.reference);
    assert!(matches!(state, ObservedModelState::Acquiring { .. }));
    release.notify_one();
    let repaired_available = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if matches!(
                api.intent()
                    .get_ensure_status(&second_declaration.reference)
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
    .await;
    if repaired_available.is_err() {
        let status = api
            .intent()
            .get_ensure_status(&second_declaration.reference)
            .await
            .unwrap();
        let downloads = api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .list_downloads()
            .await;
        let requests = server.requests();
        panic!(
            "repair did not become available: status={status:?}, downloads={downloads:?}, requests={requests:?}"
        );
    }
    let requests = server.stop().await;
    assert_eq!(
        requests
            .iter()
            .filter(|line| line.contains("/resolve/"))
            .count(),
        2
    );
    assert_eq!(
        requests
            .iter()
            .filter(|line| line.contains("/revision/main"))
            .count(),
        2,
        "bound repair must not resolve the moved branch again"
    );
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn filename_quant_hint_cannot_become_verified_quantization_evidence() {
    let root = TempDir::new().unwrap();
    let (api, release, _branch_moved, server) =
        acquisition_fixture(&root, "model-Q4_K_M.gguf").await;
    let observed = api
        .intent()
        .get_model(&upstream_requirement(Some("Q4_K_M")))
        .await
        .unwrap();
    let ObservedModelState::Acquiring {
        resolved_requirement,
        ..
    } = observed
    else {
        panic!("expected managed acquisition")
    };
    release.notify_one();
    let incomplete = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let observed = api
                .intent()
                .get_model_status(&resolved_requirement)
                .await
                .unwrap();
            if matches!(observed, ObservedModelState::Incomplete { .. }) {
                break observed;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        incomplete,
        ObservedModelState::Incomplete { diagnostics, .. }
            if diagnostics.iter().any(|item| item.code == IntentDiagnosticCode::QuantizationEvidenceMissing)
    ));
    server.stop().await;
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn explicit_main_requirement_resolves_its_completed_immutable_acquisition() {
    let root = TempDir::new().unwrap();
    let (api, release, _branch_moved, server) =
        acquisition_fixture_server(&root, "model.gguf").await;
    let mut requirement = upstream_requirement(None);
    let ModelSelector::UpstreamRepository { revision, .. } = &mut requirement.selector else {
        unreachable!()
    };
    *revision = Some("main".to_string());
    let first = api.intent().get_model(&requirement).await.unwrap();
    let ObservedModelState::Acquiring {
        resolved_requirement,
        ..
    } = first
    else {
        panic!("expected acquisition")
    };
    release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let snapshot = api
                .primary()
                .hf_client
                .as_ref()
                .unwrap()
                .intent_download_snapshot()
                .await;
            if snapshot
                .downloads
                .iter()
                .any(|item| item.status == crate::models::DownloadStatus::Completed)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let completed = api.intent().get_model(&requirement).await.unwrap();
    let ObservedModelState::Available { handle } = completed else {
        panic!("completed explicit branch requirement was not available: {completed:?}")
    };
    assert_eq!(handle.identity.model_ref.revision.as_deref(), Some(COMMIT));
    assert!(matches!(
        api.intent()
            .get_model_status(&resolved_requirement)
            .await
            .unwrap(),
        ObservedModelState::Available { .. }
    ));
    assert_eq!(server.stop().await.len(), 5);
    api.shutdown_downloads().await.unwrap();
}
