#![warn(unsafe_code)]

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelOutcome, EnsureModelRequest,
    GetEnsureStatusOutcome, IntentDiagnosticCode, ListModelDeclarationsOutcome, ModelRequirement,
    ModelSelector, ObservedModelState, QueryModelsOutcome, ReleaseModelOutcome,
};
use pumas_library::models::{PackageArtifactKind, PumasModelRef};
use pumas_library::registry::{InstanceEntry, LibraryRegistry};
use pumas_library::{PumasApi, PumasLocalClient};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CHILD_TEST: &str = "intent_ipc_registry_child";
const CHILD_ROOT: &str = "PUMAS_INTENT_IPC_ROOT";
const CHILD_READY: &str = "PUMAS_INTENT_IPC_READY";
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
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    file.write_all(contents).unwrap();
    file.sync_all().unwrap();
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

    std::future::pending::<()>().await;
}

struct RunningPrimary {
    child: Child,
    ready: ReadyFixture,
}

impl RunningPrimary {
    fn stop(mut self) {
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
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--ignored")
        .arg("--exact")
        .arg(CHILD_TEST)
        .arg("--nocapture")
        .env(CHILD_ROOT, root)
        .env(CHILD_READY, ready)
        .env(CHILD_REGISTRY, registry)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    for _ in 0..1_200 {
        if ready.is_file() {
            let ready = serde_json::from_slice(&std::fs::read(ready).unwrap()).unwrap();
            return RunningPrimary { child, ready };
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("registry-backed IPC fixture exited before ready ({status})");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("timed out waiting for registry-backed IPC fixture");
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

#[tokio::test(flavor = "multi_thread")]
async fn native_intent_ipc_is_authenticated_and_durable_across_disconnect_and_restart() {
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
    first_primary.stop();

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
    second_primary.stop();

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
    third_primary.stop();
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
    primary.stop();
}
