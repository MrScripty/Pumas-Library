use super::*;
use crate::platform::capability_fs::open_pinned_directory;
use std::sync::atomic::{AtomicUsize, Ordering};

fn manifest(path: &str, bytes: &[u8]) -> RuntimeReadFile {
    RuntimeReadFile::new(
        path.into(),
        bytes.len() as u64,
        hex::encode(Sha256::digest(bytes)),
    )
    .unwrap()
}
fn selection(
    root: &Path,
    expected: Vec<RuntimeReadFile>,
    excluded: Vec<String>,
    lease: Arc<dyn Send + Sync>,
) -> RuntimeReadRoot {
    RuntimeReadRoot::new(
        RuntimeReadRole::Sidecar,
        open_pinned_directory(root).unwrap().into_std_file(),
        expected,
        excluded,
        lease,
    )
    .unwrap()
}
fn capture(root: &Path) -> Arc<RetainedRuntimeReadSource> {
    RetainedRuntimeReadSource::capture(vec![selection(
        root,
        vec![manifest("worker.py", b"original")],
        vec![],
        Arc::new(()),
    )])
    .unwrap()
}

#[test]
fn held_capability_survives_locator_replacement_without_selecting_successor() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("runtime");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("worker.py"), b"original").unwrap();
    let owner = capture(&root);
    std::fs::rename(&root, parent.path().join("retained")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("worker.py"), b"successor").unwrap();
    owner.validate().unwrap();
    let mut bytes = Vec::new();
    owner
        .clone_member(RuntimeReadRole::Sidecar, "worker.py")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, b"original");
}

#[test]
fn equal_bytes_replacement_and_same_size_mutation_are_rejected() {
    for replacement in [false, true] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("worker.py"), b"original").unwrap();
        let owner = capture(root.path());
        if replacement {
            std::fs::write(root.path().join("new.py"), b"original").unwrap();
            std::fs::rename(root.path().join("new.py"), root.path().join("worker.py")).unwrap();
        } else {
            std::fs::write(root.path().join("worker.py"), b"mutation").unwrap();
        }
        assert!(owner.validate().is_err());
        assert!(owner
            .clone_member(RuntimeReadRole::Sidecar, "worker.py")
            .is_err());
    }
}

#[test]
fn closed_namespace_refuses_missing_extra_and_untrusted_digest() {
    for case in 0..3 {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("worker.py"), b"original").unwrap();
        let expected = if case == 2 {
            manifest("worker.py", b"invented")
        } else {
            manifest("worker.py", b"original")
        };
        if case == 0 {
            std::fs::remove_file(root.path().join("worker.py")).unwrap();
        }
        if case == 1 {
            std::fs::write(root.path().join("extra.py"), b"extra").unwrap();
        }
        assert!(RetainedRuntimeReadSource::capture(vec![selection(
            root.path(),
            vec![expected],
            vec![],
            Arc::new(())
        )])
        .is_err());
    }
}

#[test]
fn links_and_unselected_regular_files_cannot_enter_selection() {
    for case in 0..3 {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("worker.py"), b"original").unwrap();
        let other = root.path().join("alias");
        if case == 0 {
            std::os::unix::fs::symlink("worker.py", &other).unwrap();
        }
        if case == 1 {
            std::fs::hard_link(root.path().join("worker.py"), &other).unwrap();
        }
        if case == 2 {
            std::fs::write(&other, b"unreported").unwrap();
        }
        let exclusions = if case == 2 {
            vec!["alias".into()]
        } else {
            vec![]
        };
        assert!(RetainedRuntimeReadSource::capture(vec![selection(
            root.path(),
            vec![manifest("worker.py", b"original")],
            exclusions,
            Arc::new(())
        )])
        .is_err());
    }
}

#[test]
fn explicit_unselected_directory_is_identity_bound() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("worker.py"), b"original").unwrap();
    std::fs::create_dir(root.path().join("venv")).unwrap();
    let owner = RetainedRuntimeReadSource::capture(vec![selection(
        root.path(),
        vec![manifest("worker.py", b"original")],
        vec!["venv".into()],
        Arc::new(()),
    )])
    .unwrap();
    std::fs::write(root.path().join("venv/unselected"), b"separate role").unwrap();
    owner.validate().unwrap();
    std::fs::rename(root.path().join("venv"), root.path().join("old")).unwrap();
    std::fs::create_dir(root.path().join("venv")).unwrap();
    assert!(owner.validate().is_err());
}

#[test]
fn mutation_lease_lives_until_final_byte_owner_drop() {
    struct Lease(Arc<AtomicUsize>);
    impl Drop for Lease {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("worker.py"), b"original").unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let owner = RetainedRuntimeReadSource::capture(vec![selection(
        root.path(),
        vec![manifest("worker.py", b"original")],
        vec![],
        Arc::new(Lease(drops.clone())),
    )])
    .unwrap();
    let child_guard = owner.clone();
    drop(owner);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(child_guard);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
