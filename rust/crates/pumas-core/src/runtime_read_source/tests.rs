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
        let exclusions = vec![];
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

#[test]
fn explicit_inert_regular_file_never_becomes_a_read_capability() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("worker.py"), b"original").unwrap();
    let hook = root.path().join("ignored.pth");
    std::fs::write(&hook, b"import arbitrary_code").unwrap();
    let owner = RetainedRuntimeReadSource::capture(vec![selection(
        root.path(),
        vec![manifest("worker.py", b"original")],
        vec!["ignored.pth".into()],
        Arc::new(()),
    )])
    .unwrap();
    assert!(owner
        .clone_member(RuntimeReadRole::Sidecar, "ignored.pth")
        .is_err());
    assert!(!owner
        .manifest()
        .any(|(_, member)| member.path() == "ignored.pth"));
    // Contents of an unselected inode cannot authorize any content read.
    std::fs::write(&hook, b"different unselected bytes").unwrap();
    owner.validate().unwrap();
    let replacement = root.path().join("replacement");
    std::fs::write(&replacement, b"replacement inode").unwrap();
    std::fs::rename(replacement, hook).unwrap();
    assert!(owner.validate().is_err());
}

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[test]
fn retained_directory_capabilities_refuse_unknown_names_and_replacements() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("package")).unwrap();
    std::fs::write(root.path().join("package/worker.py"), b"original").unwrap();
    let owner = RetainedRuntimeReadSource::capture(vec![selection(
        root.path(),
        vec![manifest("package/worker.py", b"original")],
        vec![],
        Arc::new(()),
    )])
    .unwrap();
    assert_eq!(owner.directory_manifest().count(), 2);
    assert!(owner
        .clone_directory(RuntimeReadRole::Sidecar, "package")
        .unwrap()
        .metadata()
        .unwrap()
        .is_dir());
    assert!(owner
        .clone_directory(RuntimeReadRole::Sidecar, "missing")
        .is_err());
    std::fs::rename(
        root.path().join("package"),
        root.path().join("original-package"),
    )
    .unwrap();
    std::fs::create_dir(root.path().join("package")).unwrap();
    assert!(owner
        .clone_directory(RuntimeReadRole::Sidecar, "package")
        .is_err());
}

#[test]
fn omitted_absolute_link_is_literal_identity_not_target_read_authority() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("worker.py"), b"original").unwrap();
    std::fs::write(outside.path().join("unselected"), b"outside bytes").unwrap();
    let alias = root.path().join("omitted");
    std::os::unix::fs::symlink(outside.path(), &alias).unwrap();
    let retained = RetainedRuntimeReadSource::capture(vec![selection(
        root.path(),
        vec![manifest("worker.py", b"original")],
        vec!["omitted".into()],
        Arc::new(()),
    )])
    .unwrap();
    retained.validate().unwrap();
    assert!(retained
        .clone_member(RuntimeReadRole::Sidecar, "omitted/unselected")
        .is_err());
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(root.path(), &alias).unwrap();
    assert!(retained.validate().is_err());
}
