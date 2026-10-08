//! Existing PumasApi integration, not a recovery-qualified alternate owner.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use pumas_library::registry::{InstanceClaimResult, LibraryRegistry};
use pumas_library::{PumasApi, Result};
use std::fs::File;
use std::path::Path;
use std::time::Duration;

struct ChildGuard(std::process::Child);
impl std::ops::Deref for ChildGuard {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn owner(root: &Path, registry: LibraryRegistry) -> Result<PumasApi> {
    PumasApi::builder(root)
        .auto_create_dirs(true)
        .with_registry(registry)
        .with_hf_client(false)
        .with_process_manager(false)
        .with_connectivity_probe(false)
        .build()
        .await
}

fn held(root: &Path) -> bool {
    let descriptor = File::open(root).unwrap();
    match fs2::FileExt::try_lock_exclusive(&descriptor) {
        Ok(()) => {
            // This independent probe owns no effect. Unlock it explicitly so a
            // concurrently forked fixture cannot prolong the probe's lock.
            fs2::FileExt::unlock(&descriptor).unwrap();
            false
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::WouldBlock
                || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
        {
            true
        }
        Err(error) => panic!("unexpected lock failure: {error}"),
    }
}

async fn released(root: &Path) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while held(root) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("all physical lifetime shares should have ended");
}

#[tokio::test]
async fn distinct_registry_and_alias_cannot_construct_over_a_live_owner() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("first.db")).unwrap();
    let alternate = LibraryRegistry::open_at(&temp.path().join("second.db")).unwrap();
    let first = owner(&root, registry.clone()).await.unwrap();
    let generation = registry.get_instance(&root).unwrap().unwrap();
    let alias = temp.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let missing_directory = root.join("launcher-data/logs");
    std::fs::remove_dir(&missing_directory).unwrap();
    assert!(owner(&alias, alternate.clone()).await.is_err());
    assert!(
        !missing_directory.exists(),
        "a contending builder created directories before exclusion"
    );
    assert!(alternate.get_instance(&root).unwrap().is_none());
    assert_eq!(
        serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap(),
        serde_json::to_value(generation).unwrap()
    );
    first.shutdown_instance().await.unwrap();
    drop(first);
    released(&root).await;
    let next = owner(&alias, alternate).await.unwrap();
    next.shutdown_instance().await.unwrap();
    drop(next);
    released(&root).await;
}

#[tokio::test]
async fn escaped_library_index_and_link_registry_keep_physical_exclusion() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("first.db")).unwrap();
    let alternate = LibraryRegistry::open_at(&temp.path().join("second.db")).unwrap();
    let first = owner(&root, registry).await.unwrap();
    let library = first.model_library().clone();
    let index = library.index().clone();
    let links = library.link_registry().clone();
    first.shutdown_instance().await.unwrap();
    drop(first);
    assert!(held(&root));
    assert!(owner(&root, alternate.clone()).await.is_err());
    drop(library);
    assert!(held(&root));
    drop(links);
    assert!(held(&root));
    drop(index);
    released(&root).await;
    let next = owner(&root, alternate).await.unwrap();
    next.shutdown_instance().await.unwrap();
}

#[tokio::test]
async fn escaped_acquisition_store_keeps_physical_exclusion_after_shutdown() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let first = owner(&root, registry.clone()).await.unwrap();
    let service = first.acquisition().clone();
    let store = service.store().clone();
    first.shutdown_instance().await.unwrap();
    drop(first);
    assert!(held(&root));
    drop(service);
    assert!(held(&root));
    assert!(owner(&root, registry.clone()).await.is_err());
    drop(store);
    released(&root).await;
    let next = owner(&root, registry).await.unwrap();
    next.shutdown_instance().await.unwrap();
}

#[tokio::test]
async fn failed_constructor_releases_physical_share_but_never_replaces_claim() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("shared-resources"), b"blocked fixture").unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    assert!(owner(&root, registry.clone()).await.is_err());
    let generation = registry.get_instance(&root).unwrap().unwrap();
    released(&root).await;
    std::fs::remove_file(root.join("shared-resources")).unwrap();
    assert!(owner(&root, registry.clone()).await.is_err());
    assert_eq!(
        serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap(),
        serde_json::to_value(generation).unwrap()
    );
    assert!(!root.join("shared-resources").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_acquisition_waiter_retains_actual_blocking_effect() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let first = owner(&root, registry).await.unwrap();
    let consumer = first
        .acquisition()
        .open_consumer("physical-lifetime-fixture")
        .unwrap();
    let (started, observed) = tokio::sync::oneshot::channel();
    let (finish, gate) = std::sync::mpsc::channel();
    let waiter = tokio::spawn(async move {
        consumer
            .run_blocking("gated lifetime fixture", move || {
                let _ = started.send(());
                gate.recv().unwrap();
                Ok(())
            })
            .await
    });
    observed.await.unwrap();
    waiter.abort();
    let _ = waiter.await;
    let shutdown = tokio::spawn(async move {
        let result = first.shutdown_instance().await;
        drop(first);
        result
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!shutdown.is_finished());
    assert!(held(&root));
    finish.send(()).unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(5), shutdown)
        .await
        .unwrap()
        .unwrap();
    released(&root).await;
}

#[test]
#[ignore = "isolated real-process helper, invoked by process-loss parent"]
fn process_child() {
    let Some(directory) = std::env::var_os("PUMAS_PHYSICAL_OWNER_FIXTURE") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let registry = LibraryRegistry::open_at(&directory.join("registry.db")).unwrap();
        let _owner = owner(&directory.join("root"), registry).await.unwrap();
        std::fs::write(directory.join("ready"), b"ready").unwrap();
        std::future::pending::<()>().await;
    });
}

#[tokio::test]
async fn real_process_loss_does_not_authorize_registry_generation_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = ChildGuard(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "process_child", "--nocapture"])
            .env("PUMAS_PHYSICAL_OWNER_FIXTURE", temp.path())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let ready = tokio::time::timeout(Duration::from_secs(10), async {
        while !temp.path().join("ready").exists() {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("helper exited: {status}");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    if ready.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    ready.unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let generation = registry.get_instance(&root).unwrap().unwrap();
    assert!(held(&root));
    child.kill().unwrap();
    child.wait().unwrap();
    released(&root).await;
    assert!(matches!(
        registry
            .try_claim_instance(&root, std::process::id())
            .unwrap(),
        InstanceClaimResult::Occupied(_)
    ));
    assert!(owner(&root, registry.clone()).await.is_err());
    assert_eq!(
        serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap(),
        serde_json::to_value(generation).unwrap()
    );
}

#[test]
fn cancelled_constructor_keeps_queued_initializer_lifetime() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let (started, occupied) = tokio::sync::oneshot::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let occupier = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
        });
        occupied.await.unwrap();
        let builder = PumasApi::builder(&root)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false);
        let constructing = tokio::spawn(builder.build());
        // The single async thread yields only once the builder reaches the
        // model initializer queued behind our occupied blocking-pool slot.
        tokio::task::yield_now().await;
        let claim = registry.get_instance(&root).unwrap().unwrap();
        constructing.abort();
        assert!(matches!(constructing.await, Err(error) if error.is_cancelled()));
        assert!(
            held(&root),
            "the queued constructor effect must retain its own share"
        );
        release.send(()).unwrap();
        occupier.await.unwrap();
        released(&root).await;
        assert_eq!(
            serde_json::to_value(registry.get_instance(&root).unwrap().unwrap()).unwrap(),
            serde_json::to_value(claim).unwrap()
        );
    });
}

#[test]
fn acquisition_blocking_leaf_retains_lock_after_api_and_runtime_teardown() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(2)
        .build()
        .unwrap();
    let (release, gate) = std::sync::mpsc::channel();
    let completed = root.join("completed-effect");
    let output = completed.clone();
    let (library, service, store) = runtime.block_on(async {
        let first = owner(&root, registry).await.unwrap();
        first.shutdown_intent().await.unwrap();
        let library = std::sync::Arc::downgrade(first.model_library());
        let service = std::sync::Arc::downgrade(first.acquisition());
        let store = std::sync::Arc::downgrade(first.acquisition().store());
        let consumer = first
            .acquisition()
            .open_consumer("runtime-teardown-fixture")
            .unwrap();
        let (started, observed) = tokio::sync::oneshot::channel();
        let waiter = tokio::spawn(async move {
            consumer
                .run_blocking("leaf surviving runtime teardown", move || {
                    let _ = started.send(());
                    gate.recv().unwrap();
                    std::fs::write(output, b"effect completed")?;
                    Ok(())
                })
                .await
        });
        observed.await.unwrap();
        waiter.abort();
        let _ = waiter.await;
        drop(first);
        (library, service, store)
    });
    // No API, acquisition service/consumer, result waiter or async observer
    // remains. Only the already-running blocking effect may own exclusion.
    runtime.shutdown_background();
    // A callback already executing may briefly retain PrimaryState. Observe
    // those strong references end rather than allowing them to mask a missing
    // blocking-leaf share. The separate mutation run validates discrimination.
    let owner_drop_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while library.upgrade().is_some() || service.upgrade().is_some() || store.upgrade().is_some() {
        assert!(
            std::time::Instant::now() < owner_drop_deadline,
            "async/library/store owners did not drop independently of the gated leaf"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        held(&root),
        "actual acquisition closure lost its physical lifetime"
    );
    assert!(!completed.exists());
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while held(&root) {
        assert!(
            std::time::Instant::now() < deadline,
            "blocking leaf did not release after completion"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read(completed).unwrap(), b"effect completed");
}

#[tokio::test]
async fn escaped_link_registry_independently_retains_exclusion() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let first = owner(&root, registry).await.unwrap();
    let links = first.model_library().link_registry().clone();
    first.shutdown_instance().await.unwrap();
    drop(first);
    assert!(held(&root));
    links.read().await.save().await.unwrap();
    assert!(held(&root));
    drop(links);
    released(&root).await;
}

#[tokio::test]
async fn escaped_runtime_cleanup_ticket_retains_in_process_lifetime() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let first = owner(&root, registry).await.unwrap();
    let mut ticket = first.owned_runtime_profile_cleanup_ticket(
        pumas_library::models::RuntimeProfileId::parse("lifetime-fixture").unwrap(),
        1,
    );
    ticket.disarm();
    first.shutdown_instance().await.unwrap();
    drop(first);
    assert!(held(&root));
    drop(ticket);
    released(&root).await;
}

#[cfg(feature = "test-support")]
#[tokio::test]
async fn shutdown_drains_actual_startup_blocking_readers_before_immediate_reopen() {
    use std::sync::{Arc, Mutex};

    for operation in ["discover shard recovery", "discover interrupted downloads"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("library");
        let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        let api = owner(&root, registry.clone()).await.unwrap();
        // Both readers are queued after the builder's last suspension. This
        // current-thread test installs the gate before either task can run.
        let (entered, waiting) = tokio::sync::oneshot::channel();
        let entered = Mutex::new(Some(entered));
        let (release, released) = std::sync::mpsc::channel();
        let released = Mutex::new(released);
        api.model_library()
            .set_blocking_effect_observer_for_test(Some(Arc::new(move |actual| {
                if actual == operation {
                    if let Some(entered) = entered.lock().unwrap().take() {
                        entered.send(()).unwrap();
                        released.lock().unwrap().recv().unwrap();
                    }
                }
            })));
        tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .unwrap()
            .unwrap();

        let mut shutdown = Box::pin(api.shutdown_instance());
        let early = tokio::time::timeout(Duration::from_millis(30), &mut shutdown).await;
        let waited_for_reader = early.is_err();
        assert!(held(&root), "the blocked reader must retain exclusion");
        release.send(()).unwrap();
        match early {
            Ok(result) => result.unwrap(),
            Err(_) => tokio::time::timeout(Duration::from_secs(5), &mut shutdown)
                .await
                .unwrap()
                .unwrap(),
        }
        drop(shutdown);
        drop(api);
        let reopened = owner(&root, registry).await;
        assert!(
            waited_for_reader,
            "shutdown returned before the held {operation} reader completed"
        );
        let reopened = reopened.expect("completed shutdown must permit immediate reopen");
        reopened.shutdown_instance().await.unwrap();
    }
}
