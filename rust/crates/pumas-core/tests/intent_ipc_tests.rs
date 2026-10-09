#![warn(unsafe_code)]

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelOutcome, EnsureModelRequest,
    GetEnsureStatusOutcome, IntentDiagnosticCode, ListModelDeclarationsOutcome, ModelRequirement,
    ModelSelector, ObservedModelState, QueryModelsOutcome, ReleaseModelOutcome,
};
use pumas_library::models::{PackageArtifactKind, PumasModelRef};
use pumas_library::registry::{InstanceEntry, LibraryRegistry};
use pumas_library::{PumasApi, PumasError, PumasLocalClient};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CHILD_TEST: &str = "intent_ipc_registry_child";
const CHILD_ROOT: &str = "PUMAS_INTENT_IPC_ROOT";
const CHILD_READY: &str = "PUMAS_INTENT_IPC_READY";
const CHILD_SHUTDOWN: &str = "PUMAS_INTENT_IPC_SHUTDOWN";
const CHILD_REGISTRY: &str = "PUMAS_REGISTRY_DB_PATH";
const AVAILABLE_MODEL_ID: &str = "llm/intent/ipc-available";
const MAX_RESPONSE_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct ReadyFixture {
    entry: InstanceEntry,
    available_requirement: ModelRequirement,
    native_available: ObservedModelState,
}

fn local_requirement(model_id: &str) -> ModelRequirement {
    ModelRequirement {
        selector: ModelSelector::LocalModel {
            model_ref: PumasModelRef {
                model_id: model_id.to_string(),
                ..PumasModelRef::default()
            },
        },
        artifact: ArtifactRequirement::default(),
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    }
}

fn ensure_request() -> EnsureModelRequest {
    EnsureModelRequest {
        consumer_key: "intent-ipc-test".to_string(),
        requirement: local_requirement("llm/intent/ipc-missing"),
    }
}

fn write_durable(path: &Path, contents: &[u8]) {
    publish_ready(path, contents, || {}).unwrap();
}

fn publish_ready(
    path: &Path,
    contents: &[u8],
    before_publish: impl FnOnce(),
) -> std::io::Result<()> {
    // Existence is the parent's readiness signal. Publish the complete synced
    // bytes at once; opening the final name before writing exposes an empty file.
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    file.write_all(contents)?;
    file.as_file().sync_all()?;
    before_publish();
    file.persist_noclobber(path).map_err(|error| error.error)?;
    Ok(())
}

fn seed_available_model(root: &Path) {
    let model_dir = root
        .join("shared-resources/models")
        .join(AVAILABLE_MODEL_ID);
    let artifact = model_dir.join("model.safetensors");
    let metadata = model_dir.join("metadata.json");
    if model_dir.exists() {
        assert!(
            artifact.is_file() && metadata.is_file(),
            "restarted fixture must preserve its existing artifact and metadata"
        );
        return;
    }
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(&artifact, b"intent IPC available artifact").unwrap();
    std::fs::write(
        metadata,
        serde_json::to_vec_pretty(&serde_json::json!({
            "model_id": AVAILABLE_MODEL_ID,
            "family": "intent",
            "model_type": "llm",
            "pipeline_tag": "text-generation",
            "official_name": "Intent IPC Available",
            "cleaned_name": "ipc-available",
            "repo_id": "example/intent-ipc-available",
            "upstream_revision": "commit-ipc",
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

#[tokio::test]
#[ignore = "subprocess fixture invoked by intent IPC integration tests"]
async fn intent_ipc_registry_child() {
    let Some(root) = std::env::var_os(CHILD_ROOT).map(PathBuf::from) else {
        return;
    };
    let ready = PathBuf::from(std::env::var_os(CHILD_READY).unwrap());
    let shutdown = PathBuf::from(std::env::var_os(CHILD_SHUTDOWN).unwrap());
    let registry_path = PathBuf::from(std::env::var_os(CHILD_REGISTRY).unwrap());
    seed_available_model(&root);
    let api = PumasApi::builder(&root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    // Retained declarations trigger startup reconciliation on restart. Drain it
    // before caching facts so classification writes cannot stale the fixture.
    // The seeded metadata must agree with AVAILABLE_MODEL_ID after reconciliation.
    api.rebuild_model_index().await.unwrap();
    api.resolve_model_package_facts(AVAILABLE_MODEL_ID)
        .await
        .unwrap();
    let available_requirement = local_requirement(AVAILABLE_MODEL_ID);
    let native_available = api
        .intent()
        .get_model(&available_requirement)
        .await
        .unwrap();
    assert!(
        matches!(&native_available, ObservedModelState::Available { .. }),
        "native fixture must be Available before publishing {ready:?}: {native_available:#?}"
    );
    let registry = LibraryRegistry::open_at(&registry_path).unwrap();
    let entry = registry
        .get_instance(api.launcher_root())
        .unwrap()
        .expect("builder must publish its ready instance");
    write_durable(
        &ready,
        &serde_json::to_vec(&ReadyFixture {
            entry,
            available_requirement,
            native_available,
        })
        .unwrap(),
    );

    while !shutdown.is_file() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    api.shutdown_instance().await.unwrap();
}

struct RunningPrimary {
    child: Child,
    ready: ReadyFixture,
    shutdown: PathBuf,
}

/// Retain the child before readiness parsing can fail or panic. The receipt is
/// test-local evidence from waiting on this exact child, never a PID probe.
struct StartingPrimary {
    child: Option<Child>,
    exit_observed: Arc<AtomicBool>,
}

impl Drop for StartingPrimary {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            match child.wait() {
                Ok(_) => self.exit_observed.store(true, Ordering::Release),
                Err(error) => eprintln!("test child exit remains unobserved: {error}"),
            }
        }
    }
}

impl RunningPrimary {
    fn shutdown(mut self) {
        write_durable(&self.shutdown, b"shutdown");
        for _ in 0..3_000 {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "fixture shutdown failed: {status}");
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Drop still kills and waits on this exact child if its drain stalled.
        panic!("timed out waiting for fixture's composed instance shutdown");
    }

    fn hard_exit(mut self) {
        self.child.kill().unwrap();
        let status = self.child.wait().unwrap();
        assert!(
            !status.success(),
            "fixture unexpectedly exited successfully"
        );
    }
}

impl Drop for RunningPrimary {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_primary(root: &Path, registry: &Path, ready: &Path) -> RunningPrimary {
    start_primary_observed(
        root,
        registry,
        ready,
        CHILD_TEST,
        Arc::new(AtomicBool::new(false)),
    )
}

fn start_primary_observed(
    root: &Path,
    registry: &Path,
    ready: &Path,
    child_test: &str,
    exit_observed: Arc<AtomicBool>,
) -> RunningPrimary {
    let shutdown = ready.with_extension("shutdown");
    let child = Command::new(std::env::current_exe().unwrap())
        .arg("--ignored")
        .arg("--exact")
        .arg(child_test)
        .arg("--nocapture")
        .env(CHILD_ROOT, root)
        .env(CHILD_READY, ready)
        .env(CHILD_SHUTDOWN, &shutdown)
        .env(CHILD_REGISTRY, registry)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut owner = StartingPrimary {
        child: Some(child),
        exit_observed,
    };

    for _ in 0..1_200 {
        if ready.is_file() {
            let ready = serde_json::from_slice(&std::fs::read(ready).unwrap()).unwrap();
            return RunningPrimary {
                child: owner.child.take().unwrap(),
                ready,
                shutdown,
            };
        }
        if let Some(status) = owner.child.as_mut().unwrap().try_wait().unwrap() {
            panic!("registry-backed IPC fixture exited before ready ({status})");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for registry-backed IPC fixture");
}

#[test]
fn readiness_publication_never_exposes_partial_json_or_replaces_an_existing_file() {
    let temp = TempDir::new().unwrap();
    let ready = temp.path().join("ready.json");
    let bytes = br#"{"ready":true}"#;
    publish_ready(&ready, bytes, || assert!(!ready.exists())).unwrap();
    assert_eq!(std::fs::read(&ready).unwrap(), bytes);
    assert!(publish_ready(&ready, b"replacement", || {}).is_err());
    assert_eq!(std::fs::read(&ready).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
#[ignore = "owned subprocess for failed-readiness cleanup"]
fn intent_ipc_waiting_child() {
    // No Pumas instance, network probe or library mutation is needed here.
    // Running the ignored suite manually must not create an unowned waiter.
    if std::env::var_os(CHILD_ROOT).is_none() {
        return;
    }
    loop {
        std::thread::park();
    }
}

#[test]
fn failed_readiness_parsing_still_observes_owned_child_exit() {
    let temp = TempDir::new().unwrap();
    let ready = temp.path().join("ready.json");
    std::fs::write(&ready, b"{").unwrap();
    let exited = Arc::new(AtomicBool::new(false));
    let result = std::panic::catch_unwind(|| {
        start_primary_observed(
            &temp.path().join("library"),
            &temp.path().join("registry.db"),
            &ready,
            "intent_ipc_waiting_child",
            exited.clone(),
        )
    });
    assert!(result.is_err());
    assert!(
        exited.load(Ordering::Acquire),
        "exact child exit was not observed"
    );
    assert_eq!(std::fs::read(&ready).unwrap(), b"{");
}

async fn raw_local_ipc_call(port: u16, request: &[u8]) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        stream
            .write_all(&(request.len() as u32).to_be_bytes())
            .await
            .unwrap();
        stream.write_all(request).await.unwrap();
        stream.flush().await.unwrap();

        let mut length = [0_u8; 4];
        stream.read_exact(&mut length).await.unwrap();
        let response_len = u32::from_be_bytes(length) as usize;
        assert!(
            response_len <= MAX_RESPONSE_FRAME_BYTES,
            "local IPC response frame exceeded test bound"
        );
        let mut response = vec![0_u8; response_len];
        stream.read_exact(&mut response).await.unwrap();
        serde_json::from_slice(&response).unwrap()
    })
    .await
    .expect("local IPC request timed out")
}

async fn raw_json_call(port: u16, request: serde_json::Value) -> serde_json::Value {
    raw_local_ipc_call(port, &serde_json::to_vec(&request).unwrap()).await
}

fn assert_invalid_params(response: &serde_json::Value) {
    assert_eq!(response["error"]["code"], -32602);
    assert_eq!(response["error"]["data"]["class"], "invalid_params");
    assert!(response.get("result").is_none());
}

fn durable_declarations(root: &Path) -> Vec<serde_json::Value> {
    let connection = rusqlite::Connection::open_with_flags(
        root.join("shared-resources/models/models.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mut statement = connection
        .prepare(
            "SELECT declaration_id, consumer_key, original_requirement_json, generation,
                    model_id, bound_target_json, created_at, updated_at
             FROM intent_declarations ORDER BY declaration_id",
        )
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok(serde_json::json!({
                "declaration_id": row.get::<_, String>(0)?,
                "consumer_key": row.get::<_, String>(1)?,
                "original_requirement_json": row.get::<_, String>(2)?,
                "generation": row.get::<_, String>(3)?,
                "model_id": row.get::<_, Option<String>>(4)?,
                "bound_target_json": row.get::<_, Option<String>>(5)?,
                "created_at": row.get::<_, String>(6)?,
                "updated_at": row.get::<_, String>(7)?,
            }))
        })
        .unwrap();
    rows.collect::<rusqlite::Result<_>>().unwrap()
}

fn durable_instance(registry: &Path, root: &Path) -> serde_json::Value {
    let connection =
        rusqlite::Connection::open_with_flags(registry, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    connection
        .query_row(
            "SELECT library_path, pid, port, started_at, version, status, claim_token,
                    transport_kind, endpoint, connection_token
             FROM instances WHERE library_path = ?1",
            [root.canonicalize().unwrap().to_string_lossy().to_string()],
            |row| {
                Ok(serde_json::json!({
                    "library_path": row.get::<_, String>(0)?,
                    "pid": row.get::<_, u32>(1)?,
                    "port": row.get::<_, u16>(2)?,
                    "started_at": row.get::<_, String>(3)?,
                    "version": row.get::<_, Option<String>>(4)?,
                    "status": row.get::<_, String>(5)?,
                    "claim_token": row.get::<_, Option<String>>(6)?,
                    "transport_kind": row.get::<_, String>(7)?,
                    "endpoint": row.get::<_, Option<String>>(8)?,
                    "connection_token": row.get::<_, Option<String>>(9)?,
                }))
            },
        )
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn acknowledged_ipc_declaration_survives_hard_exit_without_licensing_primary_reopen() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("library");
    let registry = temp.path().join("registry.db");
    let ready = temp.path().join("ready.json");
    let primary = start_primary(&root, &registry, &ready);
    let client = PumasLocalClient::connect(primary.ready.entry.clone())
        .await
        .unwrap();
    let request = ensure_request();
    let declaration = match client.intent().ensure_model(&request).await.unwrap() {
        EnsureModelOutcome::Accepted { declaration, state } => {
            assert!(matches!(state, ObservedModelState::Missing { .. }));
            declaration
        }
        other => panic!("local declaration was not acknowledged through IPC: {other:?}"),
    };
    let declarations = durable_declarations(&root);
    assert_eq!(declarations.len(), 1);
    assert_eq!(
        declarations[0]["declaration_id"],
        declaration.reference.declaration_id
    );
    assert_eq!(
        declarations[0]["consumer_key"],
        declaration.reference.consumer_key
    );
    assert_eq!(
        declarations[0]["generation"],
        declaration.reference.generation
    );
    assert_eq!(
        serde_json::from_str::<ModelRequirement>(
            declarations[0]["original_requirement_json"]
                .as_str()
                .unwrap()
        )
        .unwrap(),
        request.requirement
    );
    let instance = durable_instance(&registry, &root);
    let owner_pid = primary.child.id();
    assert_eq!(instance["pid"], owner_pid);
    assert_eq!(instance["status"], "ready");
    assert!(!primary.shutdown.exists());
    drop(client);
    primary.hard_exit();

    // Waiting on the exact killed child proves this fixture's exit, not global
    // store custody. Observe persistence without reopening a mutating service.
    assert_eq!(durable_declarations(&root), declarations);
    assert_eq!(durable_instance(&registry, &root), instance);
    let result = PumasApi::builder(&root)
        .with_registry(LibraryRegistry::open_at(&registry).unwrap())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await;
    match result {
        Err(PumasError::InvalidParams { message }) => {
            assert!(message.contains("already running"), "{message}");
            assert!(message.contains(&format!("(pid {owner_pid})")), "{message}");
        }
        Err(other) => panic!("reopen failed for an unexpected reason: {other}"),
        Ok(api) => {
            api.shutdown_instance().await.unwrap();
            panic!("hard exit must retain unresolved registry custody");
        }
    }
    assert_eq!(durable_instance(&registry, &root), instance);
    assert_eq!(durable_declarations(&root), declarations);
}

#[tokio::test(flavor = "multi_thread")]
async fn native_intent_ipc_is_authenticated_and_durable_across_disconnect_and_graceful_restart() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("library");
    let registry = temp.path().join("registry.db");
    let ready_one = temp.path().join("ready-one.json");
    let first_primary = start_primary(&root, &registry, &ready_one);
    let client = PumasLocalClient::connect(first_primary.ready.entry.clone())
        .await
        .unwrap();
    let intent = client.intent();
    let request = ensure_request();

    let ipc_available = intent
        .get_model(&first_primary.ready.available_requirement)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&ipc_available).unwrap(),
        serde_json::to_value(&first_primary.ready.native_available).unwrap(),
        "native and IPC observations must preserve every wire field"
    );
    assert!(matches!(
        ipc_available,
        ObservedModelState::Available { .. }
    ));

    assert!(matches!(
        intent.query_models(&request.requirement).await.unwrap(),
        QueryModelsOutcome::Matches { candidates } if candidates.is_empty()
    ));
    assert!(matches!(
        intent.get_model(&request.requirement).await.unwrap(),
        ObservedModelState::Missing { .. }
    ));
    assert!(matches!(
        intent.get_model_status(&request.requirement).await.unwrap(),
        ObservedModelState::Missing { .. }
    ));

    let declaration = match intent.ensure_model(&request).await.unwrap() {
        EnsureModelOutcome::Accepted { declaration, state } => {
            assert!(matches!(state, ObservedModelState::Missing { .. }));
            declaration
        }
        other => panic!("local declaration was not accepted through IPC: {other:?}"),
    };
    assert!(matches!(
        intent
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Found {
            declaration: observed,
            state: ObservedModelState::Missing { .. },
        } if observed == declaration
    ));
    assert!(matches!(
        intent.list_declarations().await.unwrap(),
        ListModelDeclarationsOutcome::Declarations { declarations }
            if declarations == vec![declaration.clone()]
    ));

    drop(client);
    let reconnected = PumasLocalClient::connect(first_primary.ready.entry.clone())
        .await
        .unwrap();
    assert!(matches!(
        reconnected
            .intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Found { declaration: observed, .. }
            if observed == declaration
    ));
    drop(reconnected);
    first_primary.shutdown();
    assert!(LibraryRegistry::open_read_only_at(&registry)
        .unwrap()
        .get_instance(&root)
        .unwrap()
        .is_none());

    let ready_two = temp.path().join("ready-two.json");
    let second_primary = start_primary(&root, &registry, &ready_two);
    let restarted = PumasLocalClient::connect(second_primary.ready.entry.clone())
        .await
        .unwrap();
    assert!(matches!(
        restarted.intent().list_declarations().await.unwrap(),
        ListModelDeclarationsOutcome::Declarations { declarations }
            if declarations == vec![declaration.clone()]
    ));
    assert!(matches!(
        restarted
            .intent()
            .release_model(&declaration.reference)
            .await
            .unwrap(),
        ReleaseModelOutcome::Released { reference } if reference == declaration.reference
    ));
    assert!(matches!(
        restarted
            .intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::NotFound
    ));
    drop(restarted);
    second_primary.shutdown();

    let ready_three = temp.path().join("ready-three.json");
    let third_primary = start_primary(&root, &registry, &ready_three);
    let after_release = PumasLocalClient::connect(third_primary.ready.entry.clone())
        .await
        .unwrap();
    assert!(matches!(
        after_release.intent().list_declarations().await.unwrap(),
        ListModelDeclarationsOutcome::Declarations { declarations } if declarations.is_empty()
    ));
    assert!(matches!(
        after_release
            .intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::NotFound
    ));
    drop(after_release);
    third_primary.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn intent_ipc_rejects_bad_auth_and_malformed_wire_requests() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("library");
    let registry = temp.path().join("registry.db");
    let ready = temp.path().join("ready.json");
    let primary = start_primary(&root, &registry, &ready);
    let port = primary.ready.entry.port;
    let token = primary.ready.entry.connection_token.as_deref().unwrap();

    let unauthorized = raw_json_call(
        port,
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "intent_list_declarations",
            "params": { "connection_token": "synthetic-invalid-intent-token" },
            "id": 1,
        }),
    )
    .await;
    assert_invalid_params(&unauthorized);
    assert!(!unauthorized
        .to_string()
        .contains("synthetic-invalid-intent-token"));

    let malformed_params = raw_json_call(
        port,
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "intent_query_models",
            "params": {
                "requirement": "not-a-requirement",
                "connection_token": token,
            },
            "id": 2,
        }),
    )
    .await;
    assert_invalid_params(&malformed_params);

    let unknown_param = raw_json_call(
        port,
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "intent_list_declarations",
            "params": {
                "connection_token": token,
                "unexpected": true,
            },
            "id": 3,
        }),
    )
    .await;
    assert_invalid_params(&unknown_param);

    let malformed_json = raw_local_ipc_call(port, br#"{"jsonrpc":"2.0","#).await;
    assert_eq!(malformed_json["error"]["code"], -32700);
    assert_eq!(malformed_json["error"]["data"]["class"], "parse_error");

    let client = PumasLocalClient::connect(primary.ready.entry.clone())
        .await
        .unwrap();
    let invalid_requirement = local_requirement("../outside-library");
    assert!(matches!(
        client
            .intent()
            .get_model(&invalid_requirement)
            .await
            .unwrap(),
        ObservedModelState::InvalidRequirement { .. }
    ));
    let mut unsupported_requirement = local_requirement(AVAILABLE_MODEL_ID);
    unsupported_requirement.artifact.format = Some(PackageArtifactKind::Unknown);
    assert!(matches!(
        client
            .intent()
            .get_model(&unsupported_requirement)
            .await
            .unwrap(),
        ObservedModelState::Unsupported { diagnostics }
            if diagnostics.iter().any(|diagnostic| {
                diagnostic.code == IntentDiagnosticCode::UnsupportedArtifactFormat
            })
    ));
    drop(client);
    primary.shutdown();
}
