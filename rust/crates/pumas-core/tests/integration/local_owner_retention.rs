//! Linux local ownership tests without inference, downloads or crash reclamation.
#![cfg(target_os = "linux")]
use pumas_library::discovery::{prepare_local_access, CompatibilityRequirements};
use pumas_library::{registry::LibraryRegistry, PumasApi};
use std::{path::Path, sync::Arc, time::Duration};

fn held(root: &Path) -> bool {
    let file = std::fs::File::open(root).unwrap();
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            fs2::FileExt::unlock(&file).unwrap();
            false
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::WouldBlock
                || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
        {
            true
        }
        Err(error) => panic!("native lock observation: {error}"),
    }
}
async fn owner(root: &Path, registry: LibraryRegistry) -> PumasApi {
    PumasApi::builder(root)
        .with_registry(registry)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .with_connectivity_probe(false)
        .build()
        .await
        .unwrap()
}
fn fixture() -> (tempfile::TempDir, std::path::PathBuf, LibraryRegistry) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("existing-library-sentinel"), b"preserve root").unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    (temp, root, registry)
}
async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn multiple_retention_guards_block_release_even_after_shutdown_waiter_cancellation() {
    let (temp, root, registry) = fixture();
    let api = Arc::new(owner(&root, registry.clone()).await);
    let description = api.instance_description().unwrap();
    let first = api.retain_local_owner(&description).unwrap();
    let second = api.retain_local_owner(&description).unwrap();
    let closing = tokio::spawn({
        let api = api.clone();
        async move { api.shutdown_instance().await }
    });
    until(|| api.instance_description().is_err()).await;
    closing.abort();
    assert!(closing.await.unwrap_err().is_cancelled());
    assert!(api.retain_local_owner(&description).is_err());
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
    drop(first);
    tokio::task::yield_now().await;
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        description.generation
    );
    assert!(held(&root));
    second.release().await.unwrap();
    api.shutdown_instance().await.unwrap();
    assert!(registry.get_instance(&root).unwrap().is_none());
    assert!(
        held(&root),
        "existing API still retains its physical lifetime"
    );
    drop(api);
    until(|| !held(&root)).await;
}

#[tokio::test]
async fn ordinary_owner_drop_retains_exact_row_and_physical_lease_until_guard_release() {
    let (_temp, root, registry) = fixture();
    let api = owner(&root, registry.clone()).await;
    let description = api.instance_description().unwrap();
    let guard = api.retain_local_owner(&description).unwrap();
    drop(api);
    tokio::task::yield_now().await;
    assert!(held(&root));
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        description.generation
    );
    assert!(prepare_local_access(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    guard.release().await.unwrap();
    until(|| registry.get_instance(&root).unwrap().is_none() && !held(&root)).await;
    let restart = owner(&root, registry.clone()).await;
    assert_ne!(
        restart.instance_description().unwrap().generation,
        description.generation
    );
    assert_eq!(
        std::fs::read(root.join("existing-library-sentinel")).unwrap(),
        b"preserve root"
    );
    restart.shutdown_instance().await.unwrap();
}

#[tokio::test]
async fn stale_description_or_replaced_physical_root_cannot_admit_retention() {
    let (temp, root, registry) = fixture();
    let api = owner(&root, registry.clone()).await;
    let description = api.instance_description().unwrap();
    let mut stale = description.clone();
    stale.generation.push_str("-stale");
    assert!(api.retain_local_owner(&stale).is_err());
    // Controlled root-replacement fixture, not an arbitrary-path safety claim.
    std::fs::rename(&root, temp.path().join("retired")).unwrap();
    std::fs::create_dir(&root).unwrap();
    assert!(api.retain_local_owner(&description).is_err());
    assert!(held(&temp.path().join("retired")));
    let _ = api.shutdown_instance().await;
}

#[tokio::test]
async fn stale_retention_release_cannot_remove_an_administratively_replaced_generation() {
    let (_temp, root, registry) = fixture();
    let api = owner(&root, registry.clone()).await;
    let description = api.instance_description().unwrap();
    let guard = api.retain_local_owner(&description).unwrap();
    // Explicit corruption fixture; this API is not a qualified takeover path.
    registry.register_instance(&root, 999999999, 1).unwrap();
    let replacement = registry.get_instance(&root).unwrap().unwrap();
    assert!(api.retain_local_owner(&description).is_err());
    guard.release().await.unwrap();
    api.shutdown_instance().await.unwrap();
    let retained = registry.get_instance(&root).unwrap().unwrap();
    assert_eq!(retained.started_at, replacement.started_at);
    assert_eq!(retained.connection_token, replacement.connection_token);
}
