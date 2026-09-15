//! Process-level intent transport and restart contracts.

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelOutcome, EnsureModelRequest,
    GetEnsureStatusOutcome, ListModelDeclarationsOutcome, ModelDeclaration, ModelEnsureRef,
    ModelRequirement, ModelSelector, ObservedModelState, QueryModelsOutcome, ReleaseModelOutcome,
};
use pumas_library::models::{PackageArtifactKind, PumasModelRef};
use pumas_library::registry::LibraryRegistry;
use pumas_library::PumasLocalClient;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

const RPC_START_TIMEOUT: Duration = Duration::from_secs(20);
const ONLINE_TIMEOUT: Duration = Duration::from_secs(180);
const TEST_REPOSITORY: &str = "ggml-org/test-model-stories260K";
const TEST_COMMIT: &str = "479896ec924af6d40fd419ab8f4d1eb2101de00d";
const TEST_FILENAME: &str = "stories260K-f32.gguf";
const TEST_SIZE: u64 = 1_185_376;
const AVAILABLE_MODEL_ID: &str = "llm/intent/rpc-available";

fn test_root() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    for relative in [
        "launcher-data/metadata",
        "launcher-data/cache",
        "shared-resources/models",
    ] {
        std::fs::create_dir_all(root.path().join(relative)).unwrap();
    }
    root
}

fn seed_available_model(root: &Path) {
    let model_dir = root
        .join("shared-resources/models")
        .join(AVAILABLE_MODEL_ID);
    let artifact = model_dir.join("model.safetensors");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(&artifact, b"intent RPC available artifact").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_vec_pretty(&json!({
            "model_id": AVAILABLE_MODEL_ID,
            "family": "intent-rpc-test",
            "model_type": "llm",
            "official_name": "Intent RPC Available",
            "cleaned_name": "intent-rpc-available",
            "repo_id": "example/intent-rpc-available",
            "upstream_revision": "commit-rpc",
            "selected_artifact_id": "model.safetensors",
            "entry_path": artifact.display().to_string(),
            "storage_kind": "library_owned",
            "validation_state": "valid",
            "files": [{"name": "model.safetensors"}],
        }))
        .unwrap(),
    )
    .unwrap();
}

fn local_requirement(model_id: &str) -> ModelRequirement {
    ModelRequirement {
        selector: ModelSelector::LocalModel {
            model_ref: PumasModelRef {
                model_id: model_id.to_string(),
                ..Default::default()
            },
        },
        artifact: ArtifactRequirement::default(),
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    }
}

fn upstream_request(consumer_key: &str) -> EnsureModelRequest {
    EnsureModelRequest {
        consumer_key: consumer_key.to_string(),
        requirement: ModelRequirement {
            selector: ModelSelector::UpstreamRepository {
                repository_id: TEST_REPOSITORY.to_string(),
                revision: Some(TEST_COMMIT.to_string()),
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

struct RpcProcess {
    child: tokio::process::Child,
    port: u16,
    drains: Vec<tokio::task::JoinHandle<()>>,
    diagnostics: Arc<StdMutex<String>>,
}

fn capture_rpc_diagnostic(capture: &StdMutex<String>, line: &str) {
    const MAX_BYTES: usize = 32 * 1024;
    let mut output = capture.lock().unwrap();
    output.push_str(line);
    output.push('\n');
    if output.len() > MAX_BYTES {
        let mut boundary = output.len() - MAX_BYTES;
        while !output.is_char_boundary(boundary) {
            boundary += 1;
        }
        output.drain(..boundary);
    }
}

impl RpcProcess {
    async fn stop(mut self) {
        let _: Value = rpc(self.port, "shutdown", json!({})).await;
        #[cfg(unix)]
        {
            let pid = self.child.id().expect("RPC child exited before SIGINT");
            let signal = tokio::process::Command::new("kill")
                .arg("-INT")
                .arg(pid.to_string())
                .status()
                .await
                .unwrap();
            assert!(signal.success(), "failed to request orderly RPC shutdown");
        }
        #[cfg(not(unix))]
        self.child.start_kill().unwrap();
        let status = tokio::time::timeout(Duration::from_secs(30), self.child.wait())
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "RPC process did not complete its owned shutdown\n{}",
                    self.diagnostics.lock().unwrap()
                )
            })
            .unwrap();

        #[cfg(not(unix))]
        let _ = status; // Tokio has no portable graceful console signal for a child process.
        for drain in self.drains.drain(..) {
            tokio::time::timeout(Duration::from_secs(2), drain)
                .await
                .expect("RPC output drain did not close after shutdown")
                .unwrap();
        }
        #[cfg(unix)]
        assert!(
            status.success(),
            "RPC process shutdown failed: {status}\n{}",
            self.diagnostics.lock().unwrap()
        );
    }
}

impl Drop for RpcProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        for drain in self.drains.drain(..) {
            drain.abort();
        }
    }
}

fn rpc_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_pumas-rpc") {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().unwrap();
    let mut candidate = executable
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("pumas-rpc");
    if cfg!(windows) {
        candidate.set_extension("exe");
    }
    assert!(
        candidate.is_file(),
        "RPC binary missing at {}",
        candidate.display()
    );
    candidate
}

async fn start_rpc(root: &Path, proxy: Option<std::net::SocketAddr>) -> RpcProcess {
    let mut command = tokio::process::Command::new(rpc_binary());
    command
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .arg("--launcher-root")
        .arg(root)
        .env("PUMAS_REGISTRY_DB_PATH", root.join("registry.db"))
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.kill_on_drop(true);
    if let Some(proxy) = proxy {
        let value = format!("http://{proxy}");
        command
            .env("HTTPS_PROXY", &value)
            .env("https_proxy", value)
            .env_remove("HTTP_PROXY")
            .env_remove("http_proxy");
    }
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let diagnostics = Arc::new(StdMutex::new(String::new()));
    let stderr_capture = diagnostics.clone();
    let stderr_drain = tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            capture_rpc_diagnostic(&stderr_capture, &line);
        }
    });
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    let deadline = tokio::time::Instant::now() + RPC_START_TIMEOUT;
    let port = loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "RPC_PORT was not emitted"
        );
        match tokio::time::timeout(Duration::from_millis(250), lines.next_line()).await {
            Ok(Ok(Some(line))) => {
                capture_rpc_diagnostic(&diagnostics, &line);
                if let Some(port) = line.strip_prefix("RPC_PORT=") {
                    break port.parse().unwrap();
                }
            }
            Ok(Ok(None)) => panic!("pumas-rpc exited before publishing its port"),
            Ok(Err(error)) => panic!("failed reading pumas-rpc stdout: {error}"),
            Err(_) => {}
        }
    };
    let stdout_capture = diagnostics.clone();
    let drains = vec![
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                capture_rpc_diagnostic(&stdout_capture, &line);
            }
        }),
        stderr_drain,
    ];
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "RPC health timed out"
        );
        if client
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    RpcProcess {
        child,
        port,
        drains,
        diagnostics,
    }
}

async fn rpc<T: DeserializeOwned>(port: u16, method: &str, params: Value) -> T {
    // A fresh client makes each call an independent transport connection. The online test uses
    // this boundary to prove that dropping the ensuring caller cannot cancel owned acquisition.
    let response: Value = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap()
        .post(format!("http://127.0.0.1:{port}/rpc"))
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(response.get("error").is_none(), "RPC error: {response}");
    serde_json::from_value(response["result"].clone()).unwrap()
}

async fn local_client(root: &Path) -> PumasLocalClient {
    let registry = LibraryRegistry::open_at(&root.join("registry.db")).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let instances = PumasLocalClient::ready_instances_in_registry(&registry).unwrap();
        if let Some(instance) = instances.into_iter().next() {
            return PumasLocalClient::connect(instance).await.unwrap();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "local owner not registered"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn accepted(outcome: EnsureModelOutcome) -> (ModelDeclaration, ObservedModelState) {
    match outcome {
        EnsureModelOutcome::Accepted { declaration, state } => (declaration, state),
        other => panic!("expected accepted declaration, got {other:?}"),
    }
}

fn pinned_acquisition(
    declaration: &ModelDeclaration,
    state: &ObservedModelState,
) -> (PumasModelRef, String) {
    let resolved = declaration
        .resolved_requirement
        .as_ref()
        .expect("durable upstream declaration must record its immutable selection");
    let ModelSelector::LocalModel { model_ref } = &resolved.selector else {
        panic!("durable upstream declaration was not pinned locally");
    };
    assert_eq!(model_ref.revision.as_deref(), Some(TEST_COMMIT));
    assert_eq!(resolved.artifact.format, Some(PackageArtifactKind::Gguf));
    assert_eq!(
        resolved.artifact.selected_artifact_id,
        model_ref.selected_artifact_id
    );
    assert!(model_ref.selected_artifact_id.is_some());
    let ObservedModelState::Acquiring {
        resolved_requirement,
        download_hint: Some(download_hint),
        ..
    } = state
    else {
        panic!("expected an owned acquisition, got {state:?}");
    };
    assert_eq!(resolved_requirement, resolved);
    (model_ref.clone(), download_hint.download_id.clone())
}

fn assert_same_acquisition(
    outcome: &GetEnsureStatusOutcome,
    declaration: &ModelDeclaration,
    model_ref: &PumasModelRef,
    download_id: &str,
) {
    let GetEnsureStatusOutcome::Found {
        declaration: observed_declaration,
        state:
            ObservedModelState::Acquiring {
                resolved_requirement,
                download_hint: Some(download_hint),
                ..
            },
    } = outcome
    else {
        panic!("reconnected client did not observe the pending acquisition: {outcome:?}");
    };
    assert_eq!(observed_declaration.reference, declaration.reference);
    assert_eq!(download_hint.download_id, download_id);
    let ModelSelector::LocalModel {
        model_ref: observed_ref,
    } = &resolved_requirement.selector
    else {
        panic!("pending acquisition lost its local pin")
    };
    assert_eq!(observed_ref, model_ref);
}

#[tokio::test]
async fn http_intent_methods_are_typed_and_declarations_survive_owner_restart() {
    let root = test_root();
    seed_available_model(root.path());
    let requirement = local_requirement("llm/rpc/missing");
    let request = EnsureModelRequest {
        consumer_key: "rpc-http".to_string(),
        requirement: requirement.clone(),
    };
    let server = start_rpc(root.path(), None).await;
    let _: Value = rpc(
        server.port,
        "resolve_model_package_facts",
        json!({"model_id": AVAILABLE_MODEL_ID}),
    )
    .await;
    let available_requirement = local_requirement(AVAILABLE_MODEL_ID);
    let available: ObservedModelState = rpc(
        server.port,
        "intent_get_model",
        json!({"requirement": &available_requirement}),
    )
    .await;
    let ObservedModelState::Available { handle } = available else {
        panic!("seeded local model was not available: {available:?}")
    };
    assert_eq!(handle.identity.model_ref.model_id, AVAILABLE_MODEL_ID);

    let invalid = local_requirement("../outside");
    assert!(matches!(
        rpc::<QueryModelsOutcome>(
            server.port,
            "intent_query_models",
            json!({"requirement": &invalid}),
        )
        .await,
        QueryModelsOutcome::InvalidRequirement { .. }
    ));
    assert!(matches!(
        rpc::<ObservedModelState>(
            server.port,
            "intent_get_model_status",
            json!({"requirement": &invalid}),
        )
        .await,
        ObservedModelState::InvalidRequirement { .. }
    ));
    let mut unsupported = local_requirement("llm/rpc/unsupported");
    unsupported.artifact.format = Some(PackageArtifactKind::Unknown);
    assert!(matches!(
        rpc::<ObservedModelState>(
            server.port,
            "intent_get_model",
            json!({"requirement": &unsupported}),
        )
        .await,
        ObservedModelState::Unsupported { .. }
    ));
    let QueryModelsOutcome::Matches { candidates } = rpc(
        server.port,
        "intent_query_models",
        json!({"requirement": &requirement}),
    )
    .await
    else {
        panic!("valid local requirement was rejected")
    };
    assert!(candidates.is_empty());
    assert!(matches!(
        rpc::<ObservedModelState>(
            server.port,
            "intent_get_model",
            json!({"requirement": &request.requirement})
        )
        .await,
        ObservedModelState::Missing { .. }
    ));
    assert!(matches!(
        rpc::<ObservedModelState>(
            server.port,
            "intent_get_model_status",
            json!({"requirement": &request.requirement})
        )
        .await,
        ObservedModelState::Missing { .. }
    ));
    let (declaration, state) = accepted(
        rpc(
            server.port,
            "intent_ensure_model",
            json!({"request": &request}),
        )
        .await,
    );
    assert!(matches!(state, ObservedModelState::Missing { .. }));
    assert!(matches!(
        rpc::<GetEnsureStatusOutcome>(
            server.port,
            "intent_get_ensure_status",
            json!({"reference": &declaration.reference})
        )
        .await,
        GetEnsureStatusOutcome::Found { .. }
    ));
    let declarations = match rpc(server.port, "intent_list_declarations", json!({})).await {
        ListModelDeclarationsOutcome::Declarations { declarations } => declarations,
        _ => panic!("intent declaration listing returned an unknown outcome"),
    };
    assert_eq!(declarations, vec![declaration.clone()]);

    server.stop().await;
    let restarted = start_rpc(root.path(), None).await;
    let status: GetEnsureStatusOutcome = rpc(
        restarted.port,
        "intent_get_ensure_status",
        json!({"reference": &declaration.reference}),
    )
    .await;
    assert!(matches!(status, GetEnsureStatusOutcome::Found { .. }));
    assert!(matches!(
        rpc::<ReleaseModelOutcome>(
            restarted.port,
            "intent_release_model",
            json!({"reference": &declaration.reference})
        )
        .await,
        ReleaseModelOutcome::Released { .. }
    ));
    assert!(matches!(
        rpc::<GetEnsureStatusOutcome>(
            restarted.port,
            "intent_get_ensure_status",
            json!({"reference": &declaration.reference})
        )
        .await,
        GetEnsureStatusOutcome::NotFound
    ));
    restarted.stop().await;
}

#[tokio::test]
async fn local_client_intent_methods_share_the_rpc_owner_and_survive_restart() {
    let root = test_root();
    seed_available_model(root.path());
    let requirement = local_requirement("llm/local-client/missing");
    let request = EnsureModelRequest {
        consumer_key: "rpc-local-client".to_string(),
        requirement: requirement.clone(),
    };
    let server = start_rpc(root.path(), None).await;
    let _: Value = rpc(
        server.port,
        "resolve_model_package_facts",
        json!({"model_id": AVAILABLE_MODEL_ID}),
    )
    .await;
    let client = local_client(root.path()).await;
    let available = client
        .intent()
        .get_model(&local_requirement(AVAILABLE_MODEL_ID))
        .await
        .unwrap();
    let ObservedModelState::Available { handle } = available else {
        panic!("seeded local model was not available: {available:?}")
    };
    assert_eq!(handle.identity.model_ref.model_id, AVAILABLE_MODEL_ID);
    let invalid = local_requirement("../outside");
    assert!(matches!(
        client.intent().query_models(&invalid).await.unwrap(),
        QueryModelsOutcome::InvalidRequirement { .. }
    ));
    assert!(matches!(
        client.intent().get_model_status(&invalid).await.unwrap(),
        ObservedModelState::InvalidRequirement { .. }
    ));
    let mut unsupported = local_requirement("llm/local-client/unsupported");
    unsupported.artifact.format = Some(PackageArtifactKind::Unknown);
    assert!(matches!(
        client.intent().get_model(&unsupported).await.unwrap(),
        ObservedModelState::Unsupported { .. }
    ));
    let QueryModelsOutcome::Matches { candidates } =
        client.intent().query_models(&requirement).await.unwrap()
    else {
        panic!("valid local requirement was rejected")
    };
    assert!(candidates.is_empty());
    assert!(matches!(
        client.intent().get_model(&requirement).await.unwrap(),
        ObservedModelState::Missing { .. }
    ));
    assert!(matches!(
        client
            .intent()
            .get_model_status(&requirement)
            .await
            .unwrap(),
        ObservedModelState::Missing { .. }
    ));
    let (declaration, state) = accepted(client.intent().ensure_model(&request).await.unwrap());
    assert!(matches!(state, ObservedModelState::Missing { .. }));
    assert!(matches!(
        client
            .intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Found { .. }
    ));
    let declarations = match client.intent().list_declarations().await.unwrap() {
        ListModelDeclarationsOutcome::Declarations { declarations } => declarations,
        _ => panic!("intent declaration listing returned an unknown outcome"),
    };
    assert_eq!(declarations, vec![declaration.clone()]);
    drop(client);
    server.stop().await;

    let restarted = start_rpc(root.path(), None).await;
    let client = local_client(root.path()).await;
    assert!(matches!(
        client
            .intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Found { .. }
    ));
    assert!(matches!(
        client
            .intent()
            .release_model(&declaration.reference)
            .await
            .unwrap(),
        ReleaseModelOutcome::Released { .. }
    ));
    assert!(matches!(
        client
            .intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::NotFound
    ));
    drop(client);
    restarted.stop().await;
}

struct ThrottledConnectProxy {
    address: std::net::SocketAddr,
    released: Arc<AtomicBool>,
    release_notify: Arc<Notify>,
    task: tokio::task::JoinHandle<()>,
}

impl ThrottledConnectProxy {
    async fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let released = Arc::new(AtomicBool::new(false));
        let release_notify = Arc::new(Notify::new());
        let task_released = released.clone();
        let task_notify = release_notify.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break };
                        let released = task_released.clone();
                        let notify = task_notify.clone();
                        connections.spawn(async move {
                            let _ = proxy_connection(stream, released, notify).await;
                        });
                    }
                    Some(_) = connections.join_next(), if !connections.is_empty() => {}
                }
            }
        });
        Self {
            address,
            released,
            release_notify,
            task,
        }
    }

    fn release(&self) {
        self.released.store(true, Ordering::Release);
        self.release_notify.notify_waiters();
    }
}

impl Drop for ThrottledConnectProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn proxy_connection(
    mut downstream: TcpStream,
    released: Arc<AtomicBool>,
    notify: Arc<Notify>,
) -> std::io::Result<()> {
    let header = read_http_header(&mut downstream).await?;
    let header_text = std::str::from_utf8(&header)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let authority = header_text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("CONNECT "))
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "expected CONNECT request")
        })?;
    let upstream = TcpStream::connect(authority).await?;
    downstream
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;
    let (mut downstream_read, mut downstream_write) = downstream.into_split();
    let (mut upstream_read, mut upstream_write) = upstream.into_split();
    let upload = tokio::io::copy(&mut downstream_read, &mut upstream_write);
    let download = async {
        let mut buffer = [0_u8; 4096];
        loop {
            let count = upstream_read.read(&mut buffer).await?;
            if count == 0 {
                return Ok::<(), std::io::Error>(());
            }
            downstream_write.write_all(&buffer[..count]).await?;
            if !released.load(Ordering::Acquire) {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs_f64(count as f64 / 65_536.0)) => {}
                    _ = notify.notified() => {}
                }
            }
        }
    };
    tokio::pin!(upload);
    tokio::pin!(download);
    tokio::select! {
        result = &mut upload => result.map(|_| ()),
        result = &mut download => result,
    }
}

async fn read_http_header(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() == 8 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "HTTP proxy header too large",
            ));
        }
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).await?;
        header.push(byte[0]);
    }
    Ok(header)
}

async fn wait_http_available(port: u16, reference: &ModelEnsureRef) -> ModelDeclaration {
    let deadline = tokio::time::Instant::now() + ONLINE_TIMEOUT;
    loop {
        let outcome: GetEnsureStatusOutcome = rpc(
            port,
            "intent_get_ensure_status",
            json!({"reference": reference}),
        )
        .await;
        if let GetEnsureStatusOutcome::Found {
            declaration,
            state: ObservedModelState::Available { ref handle },
        } = outcome
        {
            assert_eq!(
                Path::new(&handle.local_load_path)
                    .file_name()
                    .and_then(|name| name.to_str()),
                Some(TEST_FILENAME)
            );
            assert_eq!(
                std::fs::metadata(&handle.local_load_path).unwrap().len(),
                TEST_SIZE
            );
            return declaration;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "download did not become available: {outcome:?}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
#[ignore = "requires live Hugging Face/CDN access"]
async fn http_disconnect_does_not_cancel_real_pinned_intent_acquisition() {
    let root = test_root();
    let proxy = ThrottledConnectProxy::start().await;
    let server = start_rpc(root.path(), Some(proxy.address)).await;
    let (declaration, state) = accepted(
        rpc(
            server.port,
            "intent_ensure_model",
            json!({"request": upstream_request("online-http")}),
        )
        .await,
    );
    let (model_ref, download_id) = pinned_acquisition(&declaration, &state);
    let reconnected: GetEnsureStatusOutcome = rpc(
        server.port,
        "intent_get_ensure_status",
        json!({"reference": &declaration.reference}),
    )
    .await;
    assert_same_acquisition(&reconnected, &declaration, &model_ref, &download_id);
    proxy.release();
    let settled = wait_http_available(server.port, &declaration.reference).await;
    assert_eq!(settled.reference, declaration.reference);
    let path = match rpc::<GetEnsureStatusOutcome>(
        server.port,
        "intent_get_ensure_status",
        json!({"reference": &declaration.reference}),
    )
    .await
    {
        GetEnsureStatusOutcome::Found {
            state: ObservedModelState::Available { handle },
            ..
        } => {
            assert_eq!(handle.identity.model_ref, model_ref);
            handle.local_load_path
        }
        other => panic!("settled declaration changed: {other:?}"),
    };
    assert!(matches!(
        rpc::<ReleaseModelOutcome>(
            server.port,
            "intent_release_model",
            json!({"reference": &declaration.reference})
        )
        .await,
        ReleaseModelOutcome::Released { .. }
    ));
    assert!(Path::new(&path).is_file(), "release deleted acquired bytes");
    server.stop().await;
}

#[tokio::test]
#[ignore = "requires live Hugging Face/CDN access"]
async fn local_client_disconnect_does_not_cancel_real_pinned_intent_acquisition() {
    let root = test_root();
    let proxy = ThrottledConnectProxy::start().await;
    let server = start_rpc(root.path(), Some(proxy.address)).await;
    let client = local_client(root.path()).await;
    let (declaration, state) = accepted(
        client
            .intent()
            .ensure_model(&upstream_request("online-local"))
            .await
            .unwrap(),
    );
    let (model_ref, download_id) = pinned_acquisition(&declaration, &state);
    drop(client);

    let client = local_client(root.path()).await;
    let reconnected = client
        .intent()
        .get_ensure_status(&declaration.reference)
        .await
        .unwrap();
    assert_same_acquisition(&reconnected, &declaration, &model_ref, &download_id);
    proxy.release();
    let deadline = tokio::time::Instant::now() + ONLINE_TIMEOUT;
    let path = loop {
        let outcome = client
            .intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap();
        if let GetEnsureStatusOutcome::Found {
            declaration: settled,
            state: ObservedModelState::Available { handle },
        } = outcome
        {
            assert_eq!(settled.reference, declaration.reference);
            assert_eq!(handle.identity.model_ref, model_ref);
            assert_eq!(
                Path::new(&handle.local_load_path)
                    .file_name()
                    .and_then(|name| name.to_str()),
                Some(TEST_FILENAME)
            );
            assert_eq!(
                std::fs::metadata(&handle.local_load_path).unwrap().len(),
                TEST_SIZE
            );
            break handle.local_load_path;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "download did not become available: {outcome:?}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    assert!(matches!(
        client
            .intent()
            .release_model(&declaration.reference)
            .await
            .unwrap(),
        ReleaseModelOutcome::Released { .. }
    ));
    assert!(Path::new(&path).is_file(), "release deleted acquired bytes");
    drop(client);
    server.stop().await;
}
