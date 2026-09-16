#![warn(unsafe_code)]
#![allow(clippy::await_holding_lock)]

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelOutcome, EnsureModelRequest,
    GetEnsureStatusOutcome, ModelDeclaration, ModelEnsureRef, ModelRequirement, ModelSelector,
    ObservedModelState, ReleaseModelOutcome,
};
use pumas_library::models::PumasModelRef;
use pumas_library::PumasApi;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tempfile::TempDir;

const CHILD_TEST: &str = "intent_desired_crash_child";
const CHILD_ROOT: &str = "PUMAS_INTENT_CRASH_ROOT";
const CHILD_PHASE: &str = "PUMAS_INTENT_CRASH_PHASE";
const CHILD_ACK: &str = "PUMAS_INTENT_CRASH_ACK";
const CHILD_RELEASE: &str = "PUMAS_INTENT_CRASH_RELEASE";
const CHILD_REFERENCE: &str = "PUMAS_INTENT_CRASH_REFERENCE";
const CHILD_REGISTRY: &str = "PUMAS_REGISTRY_DB_PATH";

static REGISTRY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct RegistryGuard(std::sync::MutexGuard<'static, ()>);

impl RegistryGuard {
    #[allow(unsafe_code)]
    fn new(registry: &Path) -> Self {
        let guard = REGISTRY_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // SAFETY: This integration-test process serializes registry environment access.
        unsafe { std::env::set_var(CHILD_REGISTRY, registry) };
        Self(guard)
    }
}

impl Drop for RegistryGuard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let _ = &self.0;
        // SAFETY: The guard still owns this process's registry environment access.
        unsafe { std::env::remove_var(CHILD_REGISTRY) };
    }
}

fn request() -> EnsureModelRequest {
    EnsureModelRequest {
        consumer_key: "crash-gate".to_string(),
        requirement: ModelRequirement {
            selector: ModelSelector::LocalModel {
                model_ref: PumasModelRef {
                    model_id: "llm/intent/crash-missing".to_string(),
                    ..Default::default()
                },
            },
            artifact: ArtifactRequirement::default(),
            acquisition_policy: AcquisitionPolicy::LocalOnly,
        },
    }
}

async fn open(root: &Path) -> PumasApi {
    PumasApi::builder(root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}

fn durable_marker(path: &Path, bytes: &[u8]) {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap()).unwrap();
    file.write_all(bytes).unwrap();
    file.as_file().sync_all().unwrap();
    file.persist_noclobber(path).unwrap();
}

fn wait_for_release(path: &Path) {
    for _ in 0..800 {
        if path.is_file() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("parent did not release crash-test child");
}

#[tokio::test]
#[ignore = "subprocess helper invoked by acknowledged_intent_survives_abrupt_exit_and_release_preserves_aba"]
async fn intent_desired_crash_child() {
    let Some(root) = std::env::var_os(CHILD_ROOT).map(PathBuf::from) else {
        return;
    };
    let phase = std::env::var(CHILD_PHASE).unwrap();
    let ack = PathBuf::from(std::env::var_os(CHILD_ACK).unwrap());
    let release = PathBuf::from(std::env::var_os(CHILD_RELEASE).unwrap());
    let api = open(&root).await;

    match phase.as_str() {
        "ensure" => {
            let outcome = api.intent().ensure_model(&request()).await.unwrap();
            let EnsureModelOutcome::Accepted { declaration, state } = outcome else {
                panic!("ensure was not durably accepted: {outcome:?}")
            };
            assert!(!matches!(state, ObservedModelState::Available { .. }));
            durable_marker(&ack, &serde_json::to_vec(&declaration).unwrap());
        }
        "release" => {
            let reference: ModelEnsureRef = serde_json::from_slice(
                &std::fs::read(std::env::var_os(CHILD_REFERENCE).unwrap()).unwrap(),
            )
            .unwrap();
            let outcome = api.intent().release_model(&reference).await.unwrap();
            assert!(matches!(outcome, ReleaseModelOutcome::Released { .. }));
            durable_marker(&ack, b"released");
        }
        other => panic!("unknown child phase {other}"),
    }

    wait_for_release(&release);
    std::process::exit(0);
}

fn spawn_child(
    root: &Path,
    registry: &Path,
    phase: &str,
    ack: &Path,
    release: &Path,
    reference: Option<&Path>,
) -> Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--ignored")
        .arg("--exact")
        .arg(CHILD_TEST)
        .arg("--nocapture")
        .env(CHILD_ROOT, root)
        .env(CHILD_PHASE, phase)
        .env(CHILD_ACK, ack)
        .env(CHILD_RELEASE, release)
        .env(CHILD_REGISTRY, registry)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(reference) = reference {
        command.env(CHILD_REFERENCE, reference);
    }
    command.spawn().unwrap()
}

fn wait_for_ack(child: &mut Child, ack: &Path) {
    for _ in 0..1200 {
        if ack.is_file() {
            return;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("crash helper exited before acknowledgement ({status})");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    panic!("timed out waiting for crash helper acknowledgement");
}

fn finish_abrupt_exit(mut child: Child, release: &Path) -> Child {
    durable_marker(release, b"exit");
    let status = child.wait().unwrap();
    assert!(status.success(), "crash helper failed with {status}");
    // On Windows this retains the exited process object and pins its PID while
    // recovery checks liveness. An unrelated process cannot reuse that PID.
    child
}

#[tokio::test]
async fn acknowledged_intent_survives_abrupt_exit_and_release_preserves_aba() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("library");
    let registry = temp.path().join("registry.db");
    let ensure_ack = temp.path().join("ensure-ack.json");
    let ensure_release = temp.path().join("ensure-exit");
    let mut ensure_child = spawn_child(
        &root,
        &registry,
        "ensure",
        &ensure_ack,
        &ensure_release,
        None,
    );
    wait_for_ack(&mut ensure_child, &ensure_ack);
    let first: ModelDeclaration =
        serde_json::from_slice(&std::fs::read(&ensure_ack).unwrap()).unwrap();
    let ensure_child = finish_abrupt_exit(ensure_child, &ensure_release);

    let registry_guard = RegistryGuard::new(&registry);
    let api = open(&root).await;
    let status = api
        .intent()
        .get_ensure_status(&first.reference)
        .await
        .unwrap();
    let GetEnsureStatusOutcome::Found { declaration, state } = status else {
        panic!("acknowledged declaration was lost after abrupt exit: {status:?}")
    };
    assert_eq!(declaration, first);
    assert!(!matches!(state, ObservedModelState::Available { .. }));
    drop(api);
    drop(registry_guard);

    let reference_path = temp.path().join("reference.json");
    std::fs::write(
        &reference_path,
        serde_json::to_vec(&first.reference).unwrap(),
    )
    .unwrap();
    let release_ack = temp.path().join("release-ack");
    let release_exit = temp.path().join("release-exit");
    let mut release_child = spawn_child(
        &root,
        &registry,
        "release",
        &release_ack,
        &release_exit,
        Some(&reference_path),
    );
    wait_for_ack(&mut release_child, &release_ack);
    let release_child = finish_abrupt_exit(release_child, &release_exit);

    let _registry_guard = RegistryGuard::new(&registry);
    let reopened = open(&root).await;
    assert!(matches!(
        reopened
            .intent()
            .get_ensure_status(&first.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::NotFound
    ));
    let replacement = match reopened.intent().ensure_model(&request()).await.unwrap() {
        EnsureModelOutcome::Accepted { declaration, state } => {
            assert!(!matches!(state, ObservedModelState::Available { .. }));
            declaration
        }
        other => panic!("replacement ensure was not accepted: {other:?}"),
    };
    assert_eq!(
        replacement.reference.declaration_id,
        first.reference.declaration_id
    );
    assert_ne!(replacement.reference.generation, first.reference.generation);
    assert!(matches!(
        reopened
            .intent()
            .get_ensure_status(&first.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Conflict
    ));
    reopened.shutdown_intent().await.unwrap();
    drop((ensure_child, release_child));
}
