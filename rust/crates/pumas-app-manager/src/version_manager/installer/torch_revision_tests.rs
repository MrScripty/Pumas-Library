//! Synthetic cooperative ownership tests; no wheel/interpreter is executed.
use super::*;
use std::os::unix::fs::{symlink, MetadataExt};

fn lock_path(root: &Path, tag: &str) -> PathBuf {
    root.join(format!(
        ".torch-revision-{:x}.lock",
        Sha256::digest(tag.as_bytes())
    ))
}

#[test]
fn last_revision_clone_excludes_only_its_registration_and_keeps_lock_inode() {
    let root = tempfile::tempdir().unwrap();
    let global = TorchVersionsLock::try_acquire_read(root.path()).unwrap();
    let first = TorchRevisionLease::read(root.path(), "v2.9.1", &global).unwrap();
    let second = TorchRevisionLease::read(root.path(), "v2.9.1", &global).unwrap();
    let clone = first.clone();
    let inode = std::fs::metadata(lock_path(root.path(), "v2.9.1"))
        .unwrap()
        .ino();
    drop(global);
    let global = TorchVersionsLock::try_acquire(root.path()).unwrap();
    assert_eq!(
        TorchRevisionLease::mutation(root.path(), "v2.9.1", &global)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
    let unrelated = TorchRevisionLease::mutation(root.path(), "v2.10.0", &global).unwrap();
    drop(first);
    drop(second);
    assert!(TorchRevisionLease::mutation(root.path(), "v2.9.1", &global).is_err());
    drop(clone);
    let mutation = TorchRevisionLease::mutation(root.path(), "v2.9.1", &global).unwrap();
    assert!(TorchRevisionLease::read(root.path(), "v2.9.1", &global).is_err());
    drop(mutation);
    drop(unrelated);
    assert_eq!(
        std::fs::metadata(lock_path(root.path(), "v2.9.1"))
            .unwrap()
            .ino(),
        inode
    );
    assert!(lock_path(root.path(), "v2.10.0").is_file());
}

#[test]
fn revision_keys_are_bounded_and_distinct_without_path_aliases() {
    let root = tempfile::tempdir().unwrap();
    let global = TorchVersionsLock::try_acquire(root.path()).unwrap();
    for tag in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        "a b",
        "a\n",
        "é",
        &"a".repeat(201),
    ] {
        assert!(TorchRevisionLease::read(root.path(), tag, &global).is_err());
        assert!(TorchRevisionLease::mutation(root.path(), tag, &global).is_err());
    }
    let a = TorchRevisionLease::read(root.path(), "v1+cpu", &global).unwrap();
    TorchRevisionLease::mutation(root.path(), "v1_cpu", &global).unwrap();
    assert_ne!(
        lock_path(root.path(), "v1+cpu"),
        lock_path(root.path(), "v1_cpu")
    );
    drop(a);
}

#[test]
fn revision_admission_requires_the_actual_owner_root_and_exclusive_mutation_token() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let shared = TorchVersionsLock::try_acquire_read(root.path()).unwrap();
    assert!(TorchRevisionLease::mutation(root.path(), "v2.9.1", &shared).is_err());
    assert!(TorchRevisionLease::read(other.path(), "v2.9.1", &shared).is_err());
    drop(shared);
    let exclusive = TorchVersionsLock::try_acquire(root.path()).unwrap();
    assert!(TorchRevisionLease::mutation(other.path(), "v2.9.1", &exclusive).is_err());
    assert!(!lock_path(other.path(), "v2.9.1").exists());
}

#[test]
fn shared_and_exclusive_global_and_revision_locks_refuse_links_and_special_members() {
    for case in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("keep");
        std::fs::write(&target, b"unchanged").unwrap();
        let global = TorchVersionsLock::try_acquire(root.path()).unwrap();
        let revision = lock_path(root.path(), "v2.9.1");
        match case {
            0 => symlink(&target, &revision).unwrap(),
            1 => std::fs::hard_link(&target, &revision).unwrap(),
            _ => std::fs::create_dir(&revision).unwrap(),
        }
        assert!(TorchRevisionLease::read(root.path(), "v2.9.1", &global).is_err());
        assert!(TorchRevisionLease::mutation(root.path(), "v2.9.1", &global).is_err());
        drop(global);
        let named = root.path().join(".torch-versions.lock");
        std::fs::remove_file(&named).unwrap();
        match case {
            0 => symlink(&target, &named).unwrap(),
            1 => std::fs::hard_link(&target, &named).unwrap(),
            _ => std::fs::create_dir(&named).unwrap(),
        }
        assert!(TorchVersionsLock::try_acquire_read(root.path()).is_err());
        assert!(TorchVersionsLock::try_acquire(root.path()).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"unchanged");
    }
    let root = tempfile::tempdir().unwrap();
    let alias = root.path().join("alias");
    symlink(root.path(), &alias).unwrap();
    assert!(TorchVersionsLock::try_acquire_read(&alias).is_err());
    assert!(TorchVersionsLock::try_acquire(&alias).is_err());
}

#[test]
fn pending_recovery_skips_retained_a_and_reclaims_b_then_resumes_after_release() {
    for registered in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let metadata = MetadataManager::new(root.path());
        metadata.ensure_directories().unwrap();
        for tag in ["v2.9.1", "v2.10.0"] {
            let runtime = root.path().join(tag);
            std::fs::create_dir(&runtime).unwrap();
            std::fs::write(runtime.join(".pumas-publishing"), TORCH_PUBLISHING_MARKER).unwrap();
            std::fs::write(runtime.join("keep"), b"inert owned bytes").unwrap();
            std::fs::write(
                root.path().join(format!(".torch-pending-publish-{tag}")),
                TORCH_PUBLISHING_MARKER,
            )
            .unwrap();
            let stage = format!(".torch-install-{tag}");
            std::fs::create_dir(root.path().join(&stage)).unwrap();
            std::fs::write(
                root.path().join(format!(".torch-pending-cleanup-{stage}")),
                tag,
            )
            .unwrap();
        }
        if registered {
            metadata
                .update_installed_version(
                    "v2.9.1",
                    InstalledVersionMetadata::default(),
                    Some(AppId::Torch),
                )
                .unwrap();
        }
        let global = TorchVersionsLock::try_acquire_read(root.path()).unwrap();
        let retained = TorchRevisionLease::read(root.path(), "v2.9.1", &global).unwrap();
        drop(global);
        retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
        assert!(root.path().join("v2.9.1/keep").is_file());
        assert!(root.path().join("v2.9.1/.pumas-publishing").is_file());
        assert!(root.path().join(".torch-pending-publish-v2.9.1").is_file());
        assert!(root.path().join(".torch-install-v2.9.1").is_dir());
        assert!(!root.path().join("v2.10.0").exists());
        assert!(!root.path().join(".torch-install-v2.10.0").exists());
        drop(retained);
        retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
        assert_eq!(root.path().join("v2.9.1").exists(), registered);
        assert!(!root.path().join(".torch-pending-publish-v2.9.1").exists());
        assert!(!root.path().join(".torch-install-v2.9.1").exists());
        assert!(lock_path(root.path(), "v2.9.1").is_file());
    }
}

#[test]
fn orphan_prune_skips_retained_registration_and_reclaims_unrelated_orphan() {
    let root = tempfile::tempdir().unwrap();
    for (tag, timestamp) in [("v2.9.1", 100), ("v2.10.0", 101)] {
        let orphan = root.path().join(format!(".torch-orphan-{tag}-{timestamp}"));
        std::fs::create_dir(&orphan).unwrap();
        std::fs::write(orphan.join(".pumas-publishing"), TORCH_PUBLISHING_MARKER).unwrap();
    }
    let global = TorchVersionsLock::try_acquire_read(root.path()).unwrap();
    let retained = TorchRevisionLease::read(root.path(), "v2.9.1", &global).unwrap();
    drop(global);
    prune_torch_orphan_quarantines(root.path(), 0, None).unwrap();
    assert!(root.path().join(".torch-orphan-v2.9.1-100").is_dir());
    assert!(!root.path().join(".torch-orphan-v2.10.0-101").exists());
    drop(retained);
    prune_torch_orphan_quarantines(root.path(), 0, None).unwrap();
    assert!(!root.path().join(".torch-orphan-v2.9.1-100").exists());
    assert!(lock_path(root.path(), "v2.9.1").is_file());
}
