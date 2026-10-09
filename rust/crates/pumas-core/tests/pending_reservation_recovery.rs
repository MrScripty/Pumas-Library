//! Actual owned Linux processes plus separately identified corruption/cancel
//! fixtures. This qualifies pending reservations, never operating-owner recovery.
#![cfg(target_os = "linux")]

use pumas_library::discovery::{
    prepare_local_access, recover_pending_reservation, CompatibilityRequirements,
    LocalStartAuthority, PendingReservationCheckpoint, PreparedLocalAccess,
};
use pumas_library::registry::LibraryRegistry;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::time::Duration;

const PAYLOAD: &str =
    "{\n  \"intent\": \"preserve exact acknowledged bytes\", \"revision\": 17\n}\n";
const PAYLOAD_SHA256: &str = "4974d3860c94a51a2691e9a8dd3eee17e482ed1d04b6f10ede8f74b1e06ca16b";

fn fixture() -> (tempfile::TempDir, PathBuf, LibraryRegistry) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("existing-sentinel"), b"existing local payload").unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    (temp, root, registry)
}

async fn reserve(root: &Path, registry: LibraryRegistry) -> LocalStartAuthority {
    match prepare_local_access(registry, root, &CompatibilityRequirements::default())
        .await
        .unwrap()
    {
        PreparedLocalAccess::Start(authority) => authority,
        _ => panic!("expected unstarted authority"),
    }
}

fn publish(path: &Path, bytes: &[u8]) {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

struct OwnedProcess {
    child: Child,
    control: PathBuf,
}
impl OwnedProcess {
    fn spawn(temp: &Path, mode: &str, label: &str, gate: Option<&Path>) -> Self {
        let control = temp.join(label);
        std::fs::create_dir(&control).unwrap();
        let log = std::fs::File::create(control.join("process.log")).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "reservation_process", "--ignored", "--nocapture"])
            .env("PUMAS_PENDING_FIXTURE", temp)
            .env("PUMAS_PENDING_MODE", mode)
            .env("PUMAS_PENDING_CONTROL", &control)
            .stdout(log.try_clone().unwrap())
            .stderr(log);
        if let Some(gate) = gate {
            command.env("PUMAS_PENDING_GATE", gate);
        }
        Self {
            child: command.spawn().unwrap(),
            control,
        }
    }
    async fn marker(&mut self, name: &str) {
        tokio::time::timeout(Duration::from_secs(15), async {
            while !self.control.join(name).exists() {
                if let Some(status) = self.child.try_wait().unwrap() {
                    panic!(
                        "owned child {} exited {status}: {}",
                        self.child.id(),
                        std::fs::read_to_string(self.control.join("process.log")).unwrap()
                    );
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn exit(&mut self) -> ExitStatus {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    fn terminate(&mut self) {
        self.child.kill().unwrap();
        let status = self.child.wait().unwrap();
        assert!(!status.success());
        println!(
            "terminated and reaped owned pid={} status={status}",
            self.child.id()
        );
    }
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn action(control: &Path) -> String {
    loop {
        if let Ok(text) = std::fs::read_to_string(control.join("action")) {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
#[ignore = "owned subprocess helper invoked by native parent tests"]
async fn reservation_process() {
    let Some(temp) = std::env::var_os("PUMAS_PENDING_FIXTURE") else {
        return;
    };
    let temp = PathBuf::from(temp);
    let control = PathBuf::from(std::env::var_os("PUMAS_PENDING_CONTROL").unwrap());
    let mode = std::env::var("PUMAS_PENDING_MODE").unwrap();
    if let Some(gate) = std::env::var_os("PUMAS_PENDING_GATE") {
        publish(&control.join("armed"), b"armed");
        while !Path::new(&gate).exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    let root = temp.join("root");
    if mode == "unknown-writer" {
        publish(&control.join("writing"), b"writing");
        loop {
            std::fs::write(root.join("owned-unknown-writer-heartbeat"), b"alive").unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    let registry = LibraryRegistry::open_at(&temp.join("registry.db")).unwrap();
    let authority = if mode == "recover" {
        let expected: PendingReservationCheckpoint =
            serde_json::from_slice(&std::fs::read(temp.join("expected.json")).unwrap()).unwrap();
        match recover_pending_reservation(
            registry.clone(),
            &root,
            &expected,
            &CompatibilityRequirements::default(),
        ) {
            Ok(authority) => authority,
            Err(error) => {
                publish(&control.join("refused"), error.to_string().as_bytes());
                panic!("{error}");
            }
        }
    } else {
        reserve(&root, registry.clone()).await
    };
    // Controlled transaction-loss fixture in an actual owned process. This is
    // deliberately NOT a producer acknowledgement or complete qualification.
    if mode == "uncommitted" {
        registry
            .register(&root, "transaction-loss fixture")
            .unwrap();
        let conn = rusqlite::Connection::open(temp.join("registry.db")).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE; UPDATE libraries SET metadata_json='{\"uncommitted\":true}';
            INSERT INTO pending_reservation_checkpoints VALUES((SELECT library_path FROM instances),'{}');").unwrap();
        publish(
            &control.join("uncommitted"),
            b"transaction has not committed",
        );
        action(&control).await;
        conn.execute_batch("ROLLBACK").unwrap();
        drop(authority);
        return;
    }
    if mode == "reserve" {
        let checkpoint = authority
            .checkpoint_metadata_for_pending_recovery(PAYLOAD)
            .unwrap();
        publish(
            &control.join("checkpoint.json"),
            &serde_json::to_vec(&checkpoint).unwrap(),
        );
    }
    let generation = registry.get_instance(&root).unwrap().unwrap().started_at;
    publish(&control.join("held"), generation.as_bytes());
    println!(
        "held {mode} pid={} generation={generation}",
        std::process::id()
    );
    match action(&control).await.as_str() {
        "cancel" => authority.cancel().unwrap(),
        "start" => {
            let owned = authority.start().await.unwrap();
            publish(
                &control.join("ready"),
                &serde_json::to_vec(owned.description()).unwrap(),
            );
            assert_eq!(action_after_start(&control).await, "stop");
            owned.shutdown_owned().await.unwrap();
        }
        "exit" => drop(authority),
        other => panic!("unknown action {other}"),
    }
}

async fn action_after_start(control: &Path) -> String {
    loop {
        if let Ok(text) = std::fs::read_to_string(control.join("stop")) {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn qualified_predecessor(temp: &Path) -> PendingReservationCheckpoint {
    let mut process = OwnedProcess::spawn(temp, "reserve", "predecessor", None);
    process.marker("held").await;
    let checkpoint =
        serde_json::from_slice(&std::fs::read(process.control.join("checkpoint.json")).unwrap())
            .unwrap();
    publish(
        &temp.join("expected.json"),
        &serde_json::to_vec(&checkpoint).unwrap(),
    );
    process.terminate();
    checkpoint
}

fn checkpoint_count(temp: &Path) -> i64 {
    rusqlite::Connection::open(temp.join("registry.db"))
        .unwrap()
        .query_row(
            "SELECT count(*) FROM pending_reservation_checkpoints",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[tokio::test]
async fn acknowledged_sqlite_payload_survives_termination_and_exact_cold_recovery() {
    let (temp, root, registry) = fixture();
    let checkpoint = qualified_predecessor(temp.path()).await;
    assert_eq!(checkpoint.metadata_sha256, PAYLOAD_SHA256);
    let cold = LibraryRegistry::open_read_only_at(&temp.path().join("registry.db")).unwrap();
    assert_eq!(
        cold.get_by_path(&root).unwrap().unwrap().metadata_json,
        PAYLOAD
    );
    assert!(prepare_local_access(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    let mut recoverer = OwnedProcess::spawn(temp.path(), "recover", "cold-recoverer", None);
    recoverer.marker("held").await;
    let successor = registry.get_instance(&root).unwrap().unwrap().started_at;
    assert_ne!(checkpoint.generation, successor);
    assert_eq!(checkpoint_count(temp.path()), 0);
    assert_eq!(
        registry.get_by_path(&root).unwrap().unwrap().metadata_json,
        PAYLOAD
    );
    println!(
        "cold winner pid={} predecessor={} successor={} payload_sha256={}",
        recoverer.child.id(),
        checkpoint.generation,
        successor,
        checkpoint.metadata_sha256
    );
    publish(&recoverer.control.join("action"), b"start");
    recoverer.marker("ready").await;
    assert!(matches!(
        prepare_local_access(
            registry.clone(),
            &root,
            &CompatibilityRequirements::default()
        )
        .await
        .unwrap(),
        PreparedLocalAccess::Borrowed(_)
    ));
    publish(&recoverer.control.join("stop"), b"stop");
    assert!(recoverer.exit().await.success());
    assert!(registry.get_instance(&root).unwrap().is_none());
    let next = reserve(&root, registry.clone()).await;
    next.cancel().unwrap();
    assert_eq!(
        registry.get_by_path(&root).unwrap().unwrap().metadata_json,
        PAYLOAD
    );
    assert_eq!(
        std::fs::read(root.join("existing-sentinel")).unwrap(),
        b"existing local payload"
    );
}

#[tokio::test]
async fn actual_concurrent_cold_recoverers_admit_one_and_fence_stale_restart() {
    let (temp, root, registry) = fixture();
    let checkpoint = qualified_predecessor(temp.path()).await;
    let gate = temp.path().join("gate");
    let mut a = OwnedProcess::spawn(temp.path(), "recover", "recover-a", Some(&gate));
    let mut b = OwnedProcess::spawn(temp.path(), "recover", "recover-b", Some(&gate));
    a.marker("armed").await;
    b.marker("armed").await;
    publish(&gate, b"start");
    tokio::time::timeout(Duration::from_secs(15), async {
        while !(a.control.join("held").exists() || b.control.join("held").exists()) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (winner, loser) = if a.control.join("held").exists() {
        (&mut a, &mut b)
    } else {
        (&mut b, &mut a)
    };
    assert!(!loser.exit().await.success());
    assert!(loser.control.join("refused").exists());
    assert!(!loser.control.join("held").exists());
    let generation = registry.get_instance(&root).unwrap().unwrap().started_at;
    println!(
        "one recovery winner={} loser={} generation={generation}",
        winner.child.id(),
        loser.child.id()
    );
    winner.terminate();
    assert!(recover_pending_reservation(
        registry.clone(),
        &root,
        &checkpoint,
        &CompatibilityRequirements::default()
    )
    .is_err());
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        generation
    );
    assert_eq!(
        registry.get_by_path(&root).unwrap().unwrap().metadata_json,
        PAYLOAD
    );
}

#[tokio::test]
async fn actual_checkpoint_cancellation_withdraws_only_pending_authority() {
    let (temp, root, registry) = fixture();
    let mut holder = OwnedProcess::spawn(temp.path(), "reserve", "cancel-holder", None);
    holder.marker("held").await;
    assert_eq!(checkpoint_count(temp.path()), 1);
    publish(&holder.control.join("action"), b"cancel");
    assert!(holder.exit().await.success());
    assert_eq!(checkpoint_count(temp.path()), 0);
    assert!(registry.get_instance(&root).unwrap().is_none());
    assert_eq!(
        registry.get_by_path(&root).unwrap().unwrap().metadata_json,
        PAYLOAD
    );
    reserve(&root, registry).await.cancel().unwrap();
}

#[tokio::test]
async fn actual_operating_owner_loss_with_owned_unknown_writer_remains_refused() {
    let (temp, root, registry) = fixture();
    let mut holder = OwnedProcess::spawn(temp.path(), "reserve", "operating-owner", None);
    holder.marker("held").await;
    let checkpoint: PendingReservationCheckpoint =
        serde_json::from_slice(&std::fs::read(holder.control.join("checkpoint.json")).unwrap())
            .unwrap();
    publish(&holder.control.join("action"), b"start");
    holder.marker("ready").await;
    assert_eq!(checkpoint_count(temp.path()), 0);
    let mut writer =
        OwnedProcess::spawn(temp.path(), "unknown-writer", "owned-unknown-writer", None);
    writer.marker("writing").await;
    holder.terminate();
    assert!(writer.child.try_wait().unwrap().is_none());
    let before = serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap();
    assert!(recover_pending_reservation(
        registry.clone(),
        &root,
        &checkpoint,
        &CompatibilityRequirements::default()
    )
    .is_err());
    assert!(prepare_local_access(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    assert_eq!(
        serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap(),
        before
    );
    writer.terminate();
}

#[tokio::test]
async fn controlled_stale_corrupt_incomplete_and_changed_evidence_never_promote() {
    // Each receipt originates from an actual acknowledged child commit and kill.
    // The following SQL/namespace mutations are controlled evidence fixtures.
    for scenario in [
        "missing",
        "partial",
        "oversized",
        "policy",
        "boot",
        "inode",
        "registry",
        "payload",
        "library",
        "generation",
        "claim",
        "pid",
        "http",
        "root",
        "stale",
        "oversized-registry-id",
        "oversized-library-id",
        "oversized-metadata",
    ] {
        let (temp, root, registry) = fixture();
        let mut expected = qualified_predecessor(temp.path()).await;
        let conn = rusqlite::Connection::open(temp.path().join("registry.db")).unwrap();
        let original: String = conn
            .query_row(
                "SELECT qualification_json FROM pending_reservation_checkpoints",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut q: serde_json::Value = serde_json::from_str(&original).unwrap();
        match scenario {
            "missing" => {
                conn.execute("DELETE FROM pending_reservation_checkpoints", [])
                    .unwrap();
            }
            "partial" => {
                conn.execute(
                    "UPDATE pending_reservation_checkpoints SET qualification_json='{}'",
                    [],
                )
                .unwrap();
            }
            "oversized" => {
                conn.execute(
                    "UPDATE pending_reservation_checkpoints SET qualification_json=?1",
                    ["x".repeat(8193)],
                )
                .unwrap();
            }
            "payload" => {
                conn.execute("UPDATE libraries SET metadata_json='{}'", [])
                    .unwrap();
            }
            "library" => {
                conn.execute("UPDATE libraries SET id='changed-id'", [])
                    .unwrap();
            }
            "oversized-registry-id" => {
                conn.execute("UPDATE registry_config SET value=?1 WHERE key='pending_reservation_registry_id@1'", ["x".repeat(129)]).unwrap();
            }
            "oversized-library-id" => {
                conn.execute("UPDATE libraries SET id=?1", ["x".repeat(129)])
                    .unwrap();
            }
            "oversized-metadata" => {
                conn.execute("UPDATE libraries SET metadata_json=?1", ["x".repeat(65537)])
                    .unwrap();
            }
            "generation" => {
                conn.execute("UPDATE instances SET started_at='successor'", [])
                    .unwrap();
            }
            "claim" => {
                conn.execute("UPDATE instances SET claim_token='unknown'", [])
                    .unwrap();
            }
            "pid" => {
                conn.execute("UPDATE instances SET pid=42", []).unwrap();
            }
            "http" => {
                conn.execute(
                    "INSERT INTO http_services VALUES(?1,'unknown','unknown','unknown','{}')",
                    [root.to_str().unwrap()],
                )
                .unwrap();
            }
            "root" => {
                std::fs::rename(&root, temp.path().join("retired-root")).unwrap();
                std::fs::create_dir(&root).unwrap();
            }
            "stale" => {
                expected.checkpoint_id = uuid::Uuid::new_v4().to_string();
            }
            field => {
                match field {
                    "policy" => q["policy"] = "unknown-custody@1".into(),
                    "boot" => q["boot_id"] = uuid::Uuid::new_v4().to_string().into(),
                    "inode" => q["inode"] = 0.into(),
                    "registry" => q["registry_id"] = "different-registry".into(),
                    _ => unreachable!(),
                }
                conn.execute(
                    "UPDATE pending_reservation_checkpoints SET qualification_json=?1",
                    [serde_json::to_string(&q).unwrap()],
                )
                .unwrap();
            }
        }
        publish(
            &temp.path().join("expected.json"),
            &serde_json::to_vec(&expected).unwrap(),
        );
        let before = serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap();
        let payload = registry.get_by_path(&root).unwrap().unwrap().metadata_json;
        let mut denied = OwnedProcess::spawn(temp.path(), "recover", "denied", None);
        assert!(!denied.exit().await.success(), "scenario {scenario}");
        assert!(denied.control.join("refused").exists());
        assert_eq!(
            serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap(),
            before,
            "{scenario}"
        );
        assert_eq!(
            registry.get_by_path(&root).unwrap().unwrap().metadata_json,
            payload,
            "{scenario}"
        );
        println!(
            "controlled {scenario} refused by actual cold pid={}",
            denied.child.id()
        );
    }
}

#[tokio::test]
async fn actual_unqualified_and_uncommitted_process_loss_refuse_cold_recovery() {
    for mode in ["unqualified", "uncommitted"] {
        let (temp, root, registry) = fixture();
        let mut holder = OwnedProcess::spawn(temp.path(), mode, "unqualified-holder", None);
        holder
            .marker(if mode == "uncommitted" {
                "uncommitted"
            } else {
                "held"
            })
            .await;
        holder.terminate();
        assert_eq!(checkpoint_count(temp.path()), 0);
        let generation = registry.get_instance(&root).unwrap().unwrap().started_at;
        let fabricated = PendingReservationCheckpoint {
            contract_version: 1,
            generation: generation.clone(),
            library_id: "unknown".into(),
            metadata_sha256: "0".repeat(64),
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
        };
        assert!(recover_pending_reservation(
            registry.clone(),
            &root,
            &fabricated,
            &CompatibilityRequirements::default()
        )
        .is_err());
        assert_eq!(
            registry.get_instance(&root).unwrap().unwrap().started_at,
            generation
        );
        if mode == "uncommitted" {
            assert_eq!(
                registry.get_by_path(&root).unwrap().unwrap().metadata_json,
                "{}"
            );
        }
        assert!(!root.join("shared-resources").exists());
    }
}

#[tokio::test]
async fn controlled_issuance_rejects_oversized_existing_identities_atomically() {
    for field in ["generation", "library", "registry"] {
        let (temp, root, registry) = fixture();
        let authority = reserve(&root, registry.clone()).await;
        registry
            .register(&root, "bounded identity fixture")
            .unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("registry.db")).unwrap();
        match field {
            "generation" => {
                conn.execute("UPDATE instances SET started_at=?1", ["x".repeat(129)])
                    .unwrap();
            }
            "library" => {
                conn.execute("UPDATE libraries SET id=?1", ["x".repeat(129)])
                    .unwrap();
            }
            "registry" => {
                conn.execute(
                    "INSERT INTO registry_config VALUES('pending_reservation_registry_id@1',?1)",
                    ["x".repeat(129)],
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(authority
            .checkpoint_metadata_for_pending_recovery(PAYLOAD)
            .is_err());
        assert_eq!(checkpoint_count(temp.path()), 0);
        assert_eq!(
            registry.get_by_path(&root).unwrap().unwrap().metadata_json,
            "{}"
        );
        authority.cancel().unwrap();
    }
}

#[test]
fn controlled_cancelled_constructor_cannot_resurrect_consumed_checkpoint() {
    let (temp, root, registry) = fixture();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let authority = reserve(&root, registry.clone()).await;
        let checkpoint = authority
            .checkpoint_metadata_for_pending_recovery(PAYLOAD)
            .unwrap();
        let (started, observed) = tokio::sync::oneshot::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let occupier = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
        });
        observed.await.unwrap();
        let starting = tokio::spawn(authority.start());
        tokio::task::yield_now().await;
        assert_eq!(checkpoint_count(temp.path()), 0);
        starting.abort();
        assert!(matches!(starting.await, Err(error) if error.is_cancelled()));
        let probe = std::fs::File::open(&root).unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&probe).is_err());
        release.send(()).unwrap();
        occupier.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while fs2::FileExt::try_lock_exclusive(&probe).is_err() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        fs2::FileExt::unlock(&probe).unwrap();
        assert!(recover_pending_reservation(
            registry.clone(),
            &root,
            &checkpoint,
            &CompatibilityRequirements::default()
        )
        .is_err());
        assert_eq!(
            registry.get_by_path(&root).unwrap().unwrap().metadata_json,
            PAYLOAD
        );
    });
}

#[tokio::test]
async fn unpolled_start_keeps_checkpoint_and_incompatible_recovery_keeps_predecessor() {
    let (temp, root, registry) = fixture();
    let authority = reserve(&root, registry.clone()).await;
    let checkpoint = authority
        .checkpoint_metadata_for_pending_recovery(PAYLOAD)
        .unwrap();
    drop(authority.start());
    assert_eq!(checkpoint_count(temp.path()), 1);
    let requirements = CompatibilityRequirements {
        required_capabilities: vec!["unsupported@1".into()],
        ..Default::default()
    };
    assert!(
        recover_pending_reservation(registry.clone(), &root, &checkpoint, &requirements).is_err()
    );
    assert_eq!(checkpoint_count(temp.path()), 1);
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        checkpoint.generation
    );
    let recovered = recover_pending_reservation(
        registry.clone(),
        &root,
        &checkpoint,
        &CompatibilityRequirements::default(),
    )
    .unwrap();
    recovered.cancel().unwrap();
    assert!(registry.get_instance(&root).unwrap().is_none());
}

#[tokio::test]
async fn checkpoint_consumed_before_failed_constructor_and_corruption_blocks_start() {
    for corrupt in [false, true] {
        let (temp, root, registry) = fixture();
        let authority = reserve(&root, registry.clone()).await;
        let checkpoint = authority
            .checkpoint_metadata_for_pending_recovery(PAYLOAD)
            .unwrap();
        if corrupt {
            rusqlite::Connection::open(temp.path().join("registry.db"))
                .unwrap()
                .execute(
                    "UPDATE pending_reservation_checkpoints SET qualification_json='{}'",
                    [],
                )
                .unwrap();
        } else {
            std::fs::write(
                root.join("shared-resources"),
                b"blocked constructor fixture",
            )
            .unwrap();
        }
        assert!(authority.start().await.is_err());
        assert_eq!(checkpoint_count(temp.path()), i64::from(corrupt));
        assert!(recover_pending_reservation(
            registry,
            &root,
            &checkpoint,
            &CompatibilityRequirements::default()
        )
        .is_err());
    }
}

#[tokio::test]
async fn checkpoint_preflight_bounds_and_stale_legacy_transition_refuse() {
    let (temp, root, registry) = fixture();
    let authority = reserve(&root, registry.clone()).await;
    for bad in ["[]".to_owned(), "{".to_owned(), " ".repeat(65537)] {
        assert!(authority
            .checkpoint_metadata_for_pending_recovery(&bad)
            .is_err());
        assert_eq!(checkpoint_count(temp.path()), 0);
        assert!(registry.get_by_path(&root).unwrap().is_none());
    }
    let checkpoint = authority
        .checkpoint_metadata_for_pending_recovery(PAYLOAD)
        .unwrap();
    registry.register_instance(&root, 999_999, 1).unwrap();
    assert_eq!(checkpoint_count(temp.path()), 0);
    assert!(authority.cancel().is_err());
    assert!(recover_pending_reservation(
        registry,
        &root,
        &checkpoint,
        &CompatibilityRequirements::default()
    )
    .is_err());
}
