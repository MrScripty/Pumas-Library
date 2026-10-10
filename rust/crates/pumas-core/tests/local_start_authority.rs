//! Linux/inference-disabled local authority fixtures; no model/runtime download.
#![cfg(target_os = "linux")]
use pumas_library::discovery::{
    prepare_local_access, CompatibilityRequirements, LocalAccess, LocalStartAuthority,
    PreparedLocalAccess,
};
use pumas_library::registry::{InstanceStatus, LibraryRegistry};
use std::path::Path;
use std::time::Duration;

fn publish(path: &Path, bytes: &[u8]) {
    let pending = path.with_extension("pending");
    std::fs::write(&pending, bytes).unwrap();
    std::fs::rename(pending, path).unwrap();
}

async fn reserve(root: &Path, registry: LibraryRegistry) -> LocalStartAuthority {
    match prepare_local_access(registry, root, &CompatibilityRequirements::default())
        .await
        .unwrap()
    {
        PreparedLocalAccess::Start(authority) => authority,
        PreparedLocalAccess::Borrowed(_) => panic!("expected fresh reservation"),
    }
}

fn fixture() -> (tempfile::TempDir, std::path::PathBuf, LibraryRegistry) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("existing-library-sentinel"),
        b"preserve existing root",
    )
    .unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    (temp, root, registry)
}

fn held(root: &Path) -> bool {
    let descriptor = std::fs::File::open(root).unwrap();
    match fs2::FileExt::try_lock_exclusive(&descriptor) {
        Ok(()) => {
            fs2::FileExt::unlock(&descriptor).unwrap();
            false
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::WouldBlock
                || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
        {
            true
        }
        Err(error) => panic!("unexpected native lock error: {error}"),
    }
}

async fn await_physical_release(root: &Path) {
    // Parallel consumer fixtures can inherit this descriptor until exec closes
    // it. Parent drop alone does not promise immediate native lock release.
    tokio::time::timeout(Duration::from_secs(5), async {
        while held(root) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("physical lease remained held after parent and inherited handles closed");
}

#[tokio::test]
async fn reserved_authority_excludes_contenders_and_cancel_then_restart_preserves_root() {
    let (temp, root, registry) = fixture();
    let first = reserve(&root, registry.clone()).await;
    assert_eq!(first.library_root(), root);
    assert!(held(&root));
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().status,
        InstanceStatus::Claiming
    );
    assert!(prepare_local_access(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    let alternate = LibraryRegistry::open_at(&temp.path().join("alternate.db")).unwrap();
    assert!(prepare_local_access(
        alternate.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    assert!(alternate.list_instances().unwrap().is_empty());
    assert!(!root.join("shared-resources").exists());
    first.cancel().unwrap();
    await_physical_release(&root).await;
    assert!(registry.list_instances().unwrap().is_empty());
    let owned = reserve(&root, registry.clone())
        .await
        .start()
        .await
        .unwrap();
    assert!(matches!(owned, LocalAccess::Owned { .. }));
    let borrowed = prepare_local_access(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await
    .unwrap();
    assert!(matches!(borrowed, PreparedLocalAccess::Borrowed(_)));
    drop(borrowed);
    owned.shutdown_owned().await.unwrap();
    drop(owned);
    assert_eq!(
        std::fs::read(root.join("existing-library-sentinel")).unwrap(),
        b"preserve existing root"
    );
}

#[tokio::test]
async fn incompatible_preflight_cannot_reserve_and_abandoned_authority_cannot_be_reclaimed() {
    let (_temp, root, registry) = fixture();
    let requirements = CompatibilityRequirements {
        required_capabilities: vec!["unsupported@1".into()],
        ..Default::default()
    };
    assert!(prepare_local_access(registry.clone(), &root, &requirements)
        .await
        .is_err());
    assert!(registry.list_instances().unwrap().is_empty());
    let abandoned = reserve(&root, registry.clone()).await;
    let generation = registry.get_instance(&root).unwrap().unwrap().started_at;
    drop(abandoned);
    await_physical_release(&root).await;
    assert!(prepare_local_access(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        generation
    );
}

#[tokio::test]
async fn replaced_claim_cannot_be_started_or_cancelled_by_stale_authority() {
    // Explicit rendezvous corruption fixture; no historical successor is fabricated.
    for cancel in [false, true] {
        let (_temp, root, registry) = fixture();
        let authority = reserve(&root, registry.clone()).await;
        registry.register_instance(&root, 123456, 1).unwrap();
        let successor = registry.get_instance(&root).unwrap().unwrap();
        if cancel {
            assert!(authority.cancel().is_err());
        } else {
            assert!(authority.start().await.is_err());
        }
        let retained = registry.get_instance(&root).unwrap().unwrap();
        assert_eq!(retained.started_at, successor.started_at);
        assert_eq!(retained.connection_token, successor.connection_token);
        assert!(!root.join("shared-resources").exists());
    }
}

#[tokio::test]
async fn replaced_root_and_failed_constructor_do_not_license_restart() {
    for replace_root in [false, true] {
        let (temp, root, registry) = fixture();
        let authority = reserve(&root, registry.clone()).await;
        let generation = registry.get_instance(&root).unwrap().unwrap().started_at;
        if replace_root {
            std::fs::rename(&root, temp.path().join("retired-root")).unwrap();
            std::fs::create_dir(&root).unwrap();
        } else {
            std::fs::write(root.join("shared-resources"), b"blocked fixture").unwrap();
        }
        assert!(authority.start().await.is_err());
        assert_eq!(
            registry.get_instance(&root).unwrap().unwrap().started_at,
            generation
        );
        assert!(prepare_local_access(
            registry.clone(),
            &root,
            &CompatibilityRequirements::default()
        )
        .await
        .is_err());
    }
}

#[test]
fn cancelled_start_keeps_queued_constructor_lease_and_unresolved_claim() {
    let (_temp, root, registry) = fixture();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let authority = reserve(&root, registry.clone()).await;
        let (started, occupied) = tokio::sync::oneshot::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let occupier = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
        });
        occupied.await.unwrap();
        let starting = tokio::spawn(authority.start());
        tokio::task::yield_now().await;
        let generation = registry.get_instance(&root).unwrap().unwrap().started_at;
        starting.abort();
        assert!(matches!(starting.await, Err(error) if error.is_cancelled()));
        assert!(
            held(&root),
            "the actual queued constructor closure retains exclusion"
        );
        release.send(()).unwrap();
        occupier.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while held(&root) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            registry.get_instance(&root).unwrap().unwrap().started_at,
            generation
        );
        assert!(prepare_local_access(
            registry.clone(),
            &root,
            &CompatibilityRequirements::default()
        )
        .await
        .is_err());
    });
}

async fn marker(path: &Path) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

struct ConsumerProcess {
    child: std::process::Child,
    control: std::path::PathBuf,
}
impl ConsumerProcess {
    fn spawn(root: &Path, db: &Path, control: &Path, gate: Option<&Path>) -> Self {
        std::fs::create_dir_all(control).unwrap();
        let output = std::fs::File::create(control.join("process.log")).unwrap();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "local_authority_child",
                "--ignored",
                "--nocapture",
            ])
            .env("PUMAS_AUTHORITY_ROOT", root)
            .env("PUMAS_AUTHORITY_DB", db)
            .env("PUMAS_AUTHORITY_CONTROL", control)
            .stdout(std::process::Stdio::from(output.try_clone().unwrap()))
            .stderr(std::process::Stdio::from(output));
        if let Some(gate) = gate {
            command.env("PUMAS_AUTHORITY_GATE", gate);
        }
        Self {
            child: command.spawn().unwrap(),
            control: control.to_owned(),
        }
    }
    async fn exit(&mut self) -> std::process::ExitStatus {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn start(&mut self) -> pumas_library::discovery::InstanceDescription {
        marker(&self.control.join("reserved")).await;
        publish(&self.control.join("action"), b"start");
        marker(&self.control.join("owned.json")).await;
        serde_json::from_slice(&std::fs::read(self.control.join("owned.json")).unwrap()).unwrap()
    }
    async fn stop(&mut self) {
        std::fs::write(self.control.join("stop"), b"").unwrap();
        assert!(
            self.exit().await.success(),
            "{}",
            std::fs::read_to_string(self.control.join("process.log")).unwrap()
        );
    }
}
impl Drop for ConsumerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "exact helper invoked in real local consumer processes"]
fn local_authority_child() {
    let root = std::path::PathBuf::from(std::env::var_os("PUMAS_AUTHORITY_ROOT").unwrap());
    let db = std::path::PathBuf::from(std::env::var_os("PUMAS_AUTHORITY_DB").unwrap());
    let control = std::path::PathBuf::from(std::env::var_os("PUMAS_AUTHORITY_CONTROL").unwrap());
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            if let Some(gate) = std::env::var_os("PUMAS_AUTHORITY_GATE") {
                std::fs::write(control.join("armed"), b"").unwrap();
                marker(Path::new(&gate)).await;
            }
            match prepare_local_access(
                LibraryRegistry::open_at(&db).unwrap(),
                &root,
                &CompatibilityRequirements::default(),
            )
            .await
            .unwrap()
            {
                PreparedLocalAccess::Borrowed(access) => {
                    std::fs::write(
                        control.join("borrowed.json"),
                        serde_json::to_vec(access.description()).unwrap(),
                    )
                    .unwrap();
                    // This exits without requesting owner shutdown.
                }
                PreparedLocalAccess::Start(authority) => {
                    std::fs::write(control.join("reserved"), b"").unwrap();
                    marker(&control.join("action")).await;
                    if std::fs::read(control.join("action")).unwrap() == b"cancel" {
                        authority.cancel().unwrap();
                        return;
                    }
                    let owned = authority.start().await.unwrap();
                    publish(
                        &control.join("owned.json"),
                        &serde_json::to_vec(owned.description()).unwrap(),
                    );
                    marker(&control.join("stop")).await;
                    owned.shutdown_owned().await.unwrap();
                    drop(owned);
                }
            }
        });
}

#[tokio::test]
async fn actual_competing_consumers_reserve_one_owner_borrow_and_restart() {
    let (temp, root, registry) = fixture();
    let db = temp.path().join("registry.db");
    let gate = temp.path().join("race-gate");
    let mut first = ConsumerProcess::spawn(&root, &db, &temp.path().join("first"), Some(&gate));
    let mut second = ConsumerProcess::spawn(&root, &db, &temp.path().join("second"), Some(&gate));
    marker(&first.control.join("armed")).await;
    marker(&second.control.join("armed")).await;
    std::fs::write(&gate, b"").unwrap();
    let first_won = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if first.control.join("reserved").exists() {
                return true;
            }
            if second.control.join("reserved").exists() {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (winner, loser) = if first_won {
        (&mut first, &mut second)
    } else {
        (&mut second, &mut first)
    };
    assert!(!loser.exit().await.success());
    assert!(held(&root));
    assert!(!root.join("shared-resources").exists());
    let description = winner.start().await;
    let mut borrower = ConsumerProcess::spawn(&root, &db, &temp.path().join("borrower"), None);
    assert!(borrower.exit().await.success());
    let borrowed: pumas_library::discovery::InstanceDescription =
        serde_json::from_slice(&std::fs::read(borrower.control.join("borrowed.json")).unwrap())
            .unwrap();
    assert_eq!(description, borrowed);
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        description.generation
    );
    println!(
        "actual consumer winner pid={}, borrower pid={}, generation={}",
        winner.child.id(),
        borrower.child.id(),
        description.generation
    );
    winner.stop().await;
    let mut restart = ConsumerProcess::spawn(&root, &db, &temp.path().join("restart"), None);
    let successor = restart.start().await;
    assert_ne!(description.generation, successor.generation);
    restart.stop().await;
}

#[tokio::test]
async fn actual_unstarted_cancel_allows_another_consumer_to_start() {
    let (temp, root, registry) = fixture();
    let db = temp.path().join("registry.db");
    let mut first = ConsumerProcess::spawn(&root, &db, &temp.path().join("cancel"), None);
    marker(&first.control.join("reserved")).await;
    publish(&first.control.join("action"), b"cancel");
    assert!(first.exit().await.success());
    assert!(registry.list_instances().unwrap().is_empty());
    assert!(!root.join("shared-resources").exists());
    let mut next = ConsumerProcess::spawn(&root, &db, &temp.path().join("next"), None);
    next.start().await;
    next.stop().await;
}

#[tokio::test]
async fn actual_reserved_process_loss_keeps_claim_and_refuses_new_consumer() {
    let (temp, root, registry) = fixture();
    let db = temp.path().join("registry.db");
    let mut first = ConsumerProcess::spawn(&root, &db, &temp.path().join("lost"), None);
    marker(&first.control.join("reserved")).await;
    let before = registry.get_instance(&root).unwrap().unwrap();
    first.child.kill().unwrap();
    assert!(!first.child.wait().unwrap().success());
    await_physical_release(&root).await;
    let mut denied = ConsumerProcess::spawn(&root, &db, &temp.path().join("denied"), None);
    assert!(!denied.exit().await.success());
    let retained = LibraryRegistry::open_read_only_at(&db)
        .unwrap()
        .get_instance(&root)
        .unwrap()
        .unwrap();
    assert_eq!(retained.started_at, before.started_at);
    assert_eq!(retained.status, InstanceStatus::Claiming);
    assert!(!root.join("shared-resources").exists());
}
