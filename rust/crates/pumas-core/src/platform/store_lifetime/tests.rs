use super::{PhysicalStoreLease, StoreLeaseError};
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn assert_busy(root: &Path) {
    assert!(matches!(
        PhysicalStoreLease::try_acquire(root),
        Err(StoreLeaseError::Busy(_))
    ));
}

// Parallel process fixtures can fork while another test holds a descriptor.
// Close-on-exec removes that inherited share, but parent drop alone need not
// make a concurrent nonblocking acquisition succeed in the same instant.
fn wait_for_lease(root: &Path) -> PhysicalStoreLease {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match PhysicalStoreLease::try_acquire(root) {
            Ok(lease) => return lease,
            Err(StoreLeaseError::Busy(_)) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            result => panic!("physical lease did not become available: {result:?}"),
        }
    }
}

#[test]
fn independent_same_process_opens_exclude_but_clones_extend_the_original_lifetime() {
    let root = tempfile::tempdir().unwrap();
    let first = PhysicalStoreLease::try_acquire(root.path()).unwrap();
    let retained = first.clone();
    assert_busy(root.path());
    drop(first);
    assert_busy(root.path());
    retained.require_current().unwrap();
    drop(retained);
    wait_for_lease(root.path());
}

#[test]
fn aliases_and_different_registries_share_exclusion_without_changing_rows() {
    use crate::registry::LibraryRegistry;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("store");
    let alias = temp.path().join("alias");
    std::fs::create_dir(&root).unwrap();
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let first_registry = LibraryRegistry::open_at(&temp.path().join("first.db")).unwrap();
    let second_registry = LibraryRegistry::open_at(&temp.path().join("second.db")).unwrap();
    first_registry.register_instance(&root, 1, 1234).unwrap();
    second_registry.register_instance(&alias, 2, 2345).unwrap();
    let first_before = first_registry.get_instance(&root).unwrap().unwrap();
    let second_before = second_registry.get_instance(&alias).unwrap().unwrap();

    let lease = PhysicalStoreLease::try_acquire(&alias).unwrap();
    assert_eq!(lease.root(), root.canonicalize().unwrap());
    assert_busy(&root);
    assert_busy(&alias.join("."));
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    drop(lease);
    wait_for_lease(&root);

    let first_after = first_registry.get_instance(&root).unwrap().unwrap();
    let second_after = second_registry.get_instance(&alias).unwrap().unwrap();
    assert_eq!(first_after.connection_token, first_before.connection_token);
    assert_eq!(first_after.started_at, first_before.started_at);
    assert_eq!(
        second_after.connection_token,
        second_before.connection_token
    );
    assert_eq!(second_after.started_at, second_before.started_at);
}

#[test]
fn failed_construction_releases_only_after_its_last_effect_owner_drops() {
    let root = tempfile::tempdir().unwrap();
    let (effect, failure) = {
        let startup = PhysicalStoreLease::try_acquire(root.path()).unwrap();
        let effect = startup.retain_for_effect(|| 7);
        // A constructor error drops its own share, not the admitted effect's.
        (effect, Err::<(), _>("injected construction failure"))
    };
    assert!(failure.is_err());
    assert_busy(root.path());
    assert_eq!(effect(), 7);
    wait_for_lease(root.path());
}

#[test]
fn missing_and_nondirectory_roots_fail_without_creating_store_state() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing");
    assert!(matches!(
        PhysicalStoreLease::try_acquire(&missing),
        Err(StoreLeaseError::Io(_))
    ));
    assert!(!missing.exists());
    let file = temp.path().join("file");
    std::fs::write(&file, b"sentinel").unwrap();
    assert!(matches!(
        PhysicalStoreLease::try_acquire(&file),
        Err(StoreLeaseError::NotDirectory(_))
    ));
    assert_eq!(std::fs::read(file).unwrap(), b"sentinel");
    let fifo = temp.path().join("fifo");
    nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRUSR).unwrap();
    assert!(matches!(
        PhysicalStoreLease::try_acquire(&fifo),
        Err(StoreLeaseError::NotDirectory(_))
    ));
}

#[test]
fn renamed_root_stays_locked_and_replacement_is_a_different_physical_store() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let moved = temp.path().join("moved");
    std::fs::create_dir(&root).unwrap();
    let original = PhysicalStoreLease::try_acquire(&root).unwrap();
    std::fs::rename(&root, &moved).unwrap();
    assert!(matches!(
        original.require_current(),
        Err(StoreLeaseError::RootChanged(_))
    ));
    assert_busy(&moved);
    std::fs::create_dir(&root).unwrap();
    assert!(matches!(
        original.require_current(),
        Err(StoreLeaseError::RootChanged(_))
    ));
    let replacement = PhysicalStoreLease::try_acquire(&root).unwrap();
    replacement.require_current().unwrap();
    assert_busy(&moved);
    drop(original);
    wait_for_lease(&moved);
    assert_busy(&root);
}

#[test]
fn blocking_effect_keeps_lock_after_requester_and_runtime_shutdown() {
    let root = tempfile::tempdir().unwrap();
    let lease = PhysicalStoreLease::try_acquire(root.path()).unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let effect = lease.retain_for_effect(move || {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        finished_tx.send(()).unwrap();
    });
    let requester = runtime.spawn(async move {
        tokio::task::spawn_blocking(effect).await.unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    drop(lease);
    requester.abort();
    drop(requester);
    // This cancels async observers without waiting for running blocking work.
    runtime.shutdown_background();
    assert_busy(root.path());
    release_tx.send(()).unwrap();
    finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let successor = wait_for_lease(root.path());
    successor.require_current().unwrap();
}

#[test]
fn panicking_effect_releases_its_lifetime_share_after_unwind() {
    let root = tempfile::tempdir().unwrap();
    let lease = PhysicalStoreLease::try_acquire(root.path()).unwrap();
    let effect = lease.retain_for_effect(|| panic!("fixture effect panic"));
    drop(lease);
    assert_busy(root.path());
    assert!(std::panic::catch_unwind(effect).is_err());
    wait_for_lease(root.path());
}

#[cfg(target_os = "linux")]
struct OwnedChild(Option<Child>);

#[cfg(target_os = "linux")]
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
#[ignore = "private child fixture invoked by process-loss test"]
fn physical_store_lease_child_fixture() {
    let Some(root) = std::env::var_os("PUMAS_TEST_PHYSICAL_LEASE_ROOT") else {
        return;
    };
    let ready = std::env::var_os("PUMAS_TEST_PHYSICAL_LEASE_READY").unwrap();
    let lease = PhysicalStoreLease::try_acquire(Path::new(&root)).unwrap();
    std::fs::write(ready, b"held").unwrap();
    loop {
        std::thread::park_timeout(Duration::from_secs(1));
        lease.require_current().unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn observed_sigkill_releases_os_lock_without_registry_reclamation() {
    use std::os::unix::process::ExitStatusExt;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let ready = temp.path().join("child-ready");
    std::fs::create_dir(&root).unwrap();
    let registry =
        crate::registry::LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    registry.register_instance(&root, 999_999_999, 1).unwrap();
    let before = registry.get_instance(&root).unwrap().unwrap();
    // Works both in the core unit suite and the focused source-path harness.
    let fixture = format!(
        "{}::physical_store_lease_child_fixture",
        module_path!().split_once("::").unwrap().1
    );
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &fixture, "--ignored", "--nocapture"])
        .env("PUMAS_TEST_PHYSICAL_LEASE_ROOT", &root)
        .env("PUMAS_TEST_PHYSICAL_LEASE_READY", &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut child = OwnedChild(Some(child));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(child.0.as_mut().unwrap().try_wait().unwrap().is_none());
        assert!(
            Instant::now() < deadline,
            "child did not acquire physical lease"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_busy(&root);
    child.0.as_mut().unwrap().kill().unwrap();
    let status = child.0.as_mut().unwrap().wait().unwrap();
    assert_eq!(status.signal(), Some(9));
    child.0.take();
    let successor = PhysicalStoreLease::try_acquire(&root).unwrap();
    successor.require_current().unwrap();
    let after = registry.get_instance(&root).unwrap().unwrap();
    assert_eq!(after.connection_token, before.connection_token);
    assert_eq!(after.started_at, before.started_at);
    assert!(matches!(
        registry
            .try_claim_instance(&root, std::process::id())
            .unwrap(),
        crate::registry::InstanceClaimResult::Occupied(_)
    ));
}
