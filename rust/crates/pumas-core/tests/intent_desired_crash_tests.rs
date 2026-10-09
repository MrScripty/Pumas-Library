//! Public acknowledgements remain durable after abrupt process exit. Reading
//! their stored evidence does not authorize a replacement primary instance.
#![warn(unsafe_code)]

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelOutcome, EnsureModelRequest,
    GetEnsureStatusOutcome, ModelDeclaration, ModelRequirement, ModelSelector, ObservedModelState,
    ReleaseModelOutcome,
};
use pumas_library::models::PumasModelRef;
use pumas_library::registry::{InstanceStatus, LibraryRegistry};
use pumas_library::{PumasApi, PumasError};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

const CHILD_TEST: &str = "intent_desired_crash_child";
const CHILD_ROOT: &str = "PUMAS_INTENT_CRASH_ROOT";
const CHILD_PHASE: &str = "PUMAS_INTENT_CRASH_PHASE";
const CHILD_ACK: &str = "PUMAS_INTENT_CRASH_ACK";
const CHILD_RELEASE: &str = "PUMAS_INTENT_CRASH_RELEASE";
const CHILD_REGISTRY: &str = "PUMAS_REGISTRY_DB_PATH";

#[derive(Serialize, Deserialize)]
struct Acknowledgement {
    first: ModelDeclaration,
    replacement: Option<ModelDeclaration>,
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

async fn open(root: &Path, registry: &Path) -> PumasApi {
    PumasApi::builder(root)
        .with_registry(LibraryRegistry::open_at(registry).unwrap())
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .with_connectivity_probe(false)
        .build()
        .await
        .unwrap()
}

async fn ensure(api: &PumasApi) -> ModelDeclaration {
    let outcome = api.intent().ensure_model(&request()).await.unwrap();
    let EnsureModelOutcome::Accepted { declaration, state } = outcome else {
        panic!("ensure was not durably accepted: {outcome:?}")
    };
    assert!(!matches!(state, ObservedModelState::Available { .. }));
    declaration
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
#[ignore = "subprocess helper for acknowledged intent persistence and retained-owner refusal"]
async fn intent_desired_crash_child() {
    let Some(root) = std::env::var_os(CHILD_ROOT).map(PathBuf::from) else {
        return;
    };
    let phase = std::env::var(CHILD_PHASE).unwrap();
    let ack = PathBuf::from(std::env::var_os(CHILD_ACK).unwrap());
    let release = PathBuf::from(std::env::var_os(CHILD_RELEASE).unwrap());
    let registry = PathBuf::from(std::env::var_os(CHILD_REGISTRY).unwrap());
    let api = open(&root, &registry).await;
    let first = ensure(&api).await;
    let replacement = match phase.as_str() {
        "ensure" => None,
        "release" | "replacement" => {
            assert!(matches!(
                api.intent().release_model(&first.reference).await.unwrap(),
                ReleaseModelOutcome::Released { .. }
            ));
            assert!(matches!(
                api.intent()
                    .get_ensure_status(&first.reference)
                    .await
                    .unwrap(),
                GetEnsureStatusOutcome::NotFound
            ));
            if phase == "replacement" {
                let replacement = ensure(&api).await;
                assert_eq!(
                    first.reference.declaration_id,
                    replacement.reference.declaration_id
                );
                assert_ne!(first.reference.generation, replacement.reference.generation);
                assert!(matches!(
                    api.intent().release_model(&first.reference).await.unwrap(),
                    ReleaseModelOutcome::Conflict { .. }
                ));
                assert!(matches!(
                    api.intent()
                        .get_ensure_status(&first.reference)
                        .await
                        .unwrap(),
                    GetEnsureStatusOutcome::Conflict
                ));
                Some(replacement)
            } else {
                None
            }
        }
        other => panic!("unknown child phase {other}"),
    };
    durable_marker(
        &ack,
        &serde_json::to_vec(&Acknowledgement { first, replacement }).unwrap(),
    );
    wait_for_release(&release);
    // Deliberately skip Drop and composed shutdown, retaining the real abrupt
    // process-loss boundary. No writer is admitted on this root afterward.
    std::process::exit(0);
}

struct OwnedChild {
    child: Child,
    exit_observed: bool,
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.exit_observed {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn spawn_child(
    root: &Path,
    registry: &Path,
    phase: &str,
    ack: &Path,
    release: &Path,
) -> OwnedChild {
    let child = Command::new(std::env::current_exe().unwrap())
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
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    OwnedChild {
        child,
        exit_observed: false,
    }
}

fn wait_for_ack(child: &mut OwnedChild, ack: &Path) {
    for _ in 0..1200 {
        if ack.is_file() {
            return;
        }
        if let Some(status) = child.child.try_wait().unwrap() {
            child.exit_observed = true;
            panic!("crash helper exited before acknowledgement ({status})");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for crash helper acknowledgement");
}

fn finish_abrupt_exit(child: &mut OwnedChild, release: &Path) {
    durable_marker(release, b"exit");
    let status = child.child.wait().unwrap();
    child.exit_observed = true;
    assert!(status.success(), "crash helper failed with {status}");
}

#[derive(Debug, PartialEq, Eq)]
struct StoredDeclaration {
    declaration_id: String,
    consumer_key: String,
    original_requirement_json: String,
    generation: String,
    model_id: Option<String>,
    bound_target_json: Option<String>,
    created_at: String,
    updated_at: String,
}

fn read_declarations(root: &Path) -> Vec<StoredDeclaration> {
    // A read-only SQL observer inspects the actual persisted rows, including
    // committed WAL content. It neither constructs a service nor migrates data.
    let connection = Connection::open_with_flags(
        root.join("shared-resources/models/models.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mut query = connection
        .prepare(
            "SELECT declaration_id, consumer_key, original_requirement_json, generation,
                model_id, bound_target_json, created_at, updated_at
         FROM intent_declarations ORDER BY declaration_id",
        )
        .unwrap();
    query
        .query_map([], |row| {
            Ok(StoredDeclaration {
                declaration_id: row.get(0)?,
                consumer_key: row.get(1)?,
                original_requirement_json: row.get(2)?,
                generation: row.get(3)?,
                model_id: row.get(4)?,
                bound_target_json: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[tokio::test]
async fn acknowledged_intent_commit_release_and_aba_survive_abrupt_exit_without_owner_reclaim() {
    for phase in ["ensure", "release", "replacement"] {
        // Each abrupt boundary starts with its own legitimate primary. A prior
        // crashed root is never made writable through a different registry or
        // lower-level service. Orderly public-API restart/ABA is tested separately.
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        let registry_path = temp.path().join("registry.db");
        let ack = temp.path().join("ack.json");
        let release = temp.path().join("exit");
        let mut child = spawn_child(&root, &registry_path, phase, &ack, &release);
        wait_for_ack(&mut child, &ack);
        let acknowledgement: Acknowledgement =
            serde_json::from_slice(&std::fs::read(&ack).unwrap()).unwrap();
        let registry = LibraryRegistry::open_read_only_at(&registry_path).unwrap();
        let owner = registry.get_instance(&root).unwrap().unwrap();
        assert_eq!(owner.pid, child.child.id());
        assert_eq!(owner.status, InstanceStatus::Ready);
        let retained_owner = serde_json::to_value(&owner).unwrap();
        let before = read_declarations(&root);
        if phase == "release" {
            assert!(
                before.is_empty(),
                "acknowledged release still has a declaration"
            );
        } else {
            assert_eq!(before.len(), 1);
            let expected = acknowledgement
                .replacement
                .as_ref()
                .unwrap_or(&acknowledgement.first);
            assert_eq!(before[0].declaration_id, expected.reference.declaration_id);
            assert_eq!(before[0].consumer_key, expected.reference.consumer_key);
            assert_eq!(before[0].generation, expected.reference.generation);
            assert_eq!(
                serde_json::from_str::<ModelRequirement>(&before[0].original_requirement_json)
                    .unwrap(),
                expected.requirement
            );
            let ModelSelector::LocalModel { model_ref } =
                &expected.resolved_requirement.as_ref().unwrap().selector
            else {
                panic!("fixture must resolve to its explicit local model");
            };
            assert_eq!(
                before[0].model_id.as_deref(),
                Some(model_ref.model_id.as_str())
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(
                    before[0].bound_target_json.as_ref().unwrap()
                )
                .unwrap(),
                serde_json::json!({"kind": "local", "model_ref": model_ref})
            );
        }
        finish_abrupt_exit(&mut child, &release);
        assert_eq!(
            read_declarations(&root),
            before,
            "lost acknowledged {phase} after process exit"
        );
        for _ in 0..2 {
            let retry = PumasApi::builder(&root)
                .with_registry(LibraryRegistry::open_at(&registry_path).unwrap())
                .with_hf_client(false)
                .with_process_manager(false)
                .with_connectivity_probe(false)
                .build()
                .await;
            assert!(
                matches!(retry, Err(PumasError::InvalidParams { message }) if message == format!(
                    "Pumas library instance is already running for {} (pid {}). Use PumasLocalClient for explicit local-client access.", root.display(), owner.pid
                ))
            );
            assert_eq!(
                serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap(),
                retained_owner
            );
            assert_eq!(read_declarations(&root), before);
        }
    }
}
