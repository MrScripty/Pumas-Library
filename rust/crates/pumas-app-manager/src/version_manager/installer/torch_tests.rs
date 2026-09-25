//! Installer lifecycle fixtures for the qualified upstream Torch recipe.
use super::*;
use std::os::unix::fs::PermissionsExt;
use tokio::process::Command;

fn fixture_installer() -> (VersionInstaller, tempfile::TempDir) {
    let root = tempfile::TempDir::new().unwrap();
    let metadata = Arc::new(MetadataManager::new(root.path()));
    metadata.ensure_directories().unwrap();
    let tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
        root.path().join("launcher-data/cache"),
    )));
    let installer = VersionInstaller::new(
        root.path().to_path_buf(),
        AppId::Torch,
        metadata,
        tracker,
        Arc::new(AtomicBool::new(false)),
    );
    (installer, root)
}

fn upstream_release() -> GitHubRelease {
    GitHubRelease {
        tag_name: "v2.9.1".to_string(),
        name: "PyTorch 2.9.1".to_string(),
        published_at: "2025-11-12T00:00:00Z".to_string(),
        body: None,
        tarball_url: Some(
            "https://github.com/pytorch/pytorch/archive/refs/tags/v2.9.1.tar.gz".to_string(),
        ),
        zipball_url: None,
        prerelease: false,
        assets: vec![],
        html_url: "https://github.com/pytorch/pytorch/releases/tag/v2.9.1".to_string(),
        total_size: Some(348_810_776),
        archive_size: Some(348_810_776),
        dependencies_size: None,
    }
}

fn mock_runtime(staging: &Path) -> Result<PathBuf> {
    let runtime = staging.join("runtime");
    std::fs::create_dir_all(runtime.join("venv/bin")).map_err(PumasError::from)?;
    let python = runtime.join("venv/bin/python");
    std::fs::write(&python, "#!/bin/sh\necho Python 3.12.0\n").map_err(PumasError::from)?;
    std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755))
        .map_err(PumasError::from)?;
    std::fs::write(
        runtime.join("runtime.json"),
        r#"{"recipe_id":"torch-upstream-2.9.1-r1"}"#,
    )
    .map_err(PumasError::from)?;
    Ok(runtime)
}

fn assert_no_staging(installer: &VersionInstaller) {
    let versions = installer.versions_dir();
    if versions.exists() {
        assert!(std::fs::read_dir(versions).unwrap().all(|item| {
            !item
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".torch-install-")
        }));
    }
}

#[test]
fn pending_stage_quarantine_retries_after_obstruction_clears() {
    let root = tempfile::tempdir().unwrap();
    let versions = root.path();
    let stage = versions.join(".torch-install-owned");
    let quarantine = versions.join(".torch-quarantine-.torch-install-owned");
    let marker = versions.join(".torch-pending-cleanup-.torch-install-owned");
    std::fs::create_dir(&stage).unwrap();
    std::fs::write(stage.join("payload"), "staged").unwrap();
    std::fs::write(&marker, "v2.9.1").unwrap();
    std::fs::write(&quarantine, "obstruction").unwrap();
    let metadata = MetadataManager::new(versions);
    torch::retry_pending_torch_cleanup(versions, &metadata).unwrap();
    assert!(stage.exists() && marker.exists());
    std::fs::remove_file(&quarantine).unwrap();
    torch::retry_pending_torch_cleanup(versions, &metadata).unwrap();
    assert!(!stage.exists() && !quarantine.exists() && !marker.exists());
}

#[test]
fn unregistered_publication_retry_removes_owned_directory() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("v2.9.1");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(
        destination.join(".pumas-publishing"),
        torch::TORCH_PUBLISHING_MARKER,
    )
    .unwrap();
    std::fs::write(destination.join("payload"), "unregistered").unwrap();
    let marker = root.path().join(".torch-pending-publish-v2.9.1");
    std::fs::write(&marker, torch::TORCH_PUBLISHING_MARKER).unwrap();
    let metadata = MetadataManager::new(root.path());
    torch::retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
    assert!(!destination.exists() && !marker.exists());
}

#[test]
fn registered_publication_recovery_preserves_runtime() {
    let root = tempfile::tempdir().unwrap();
    let metadata = MetadataManager::new(root.path());
    metadata.ensure_directories().unwrap();
    let destination = root.path().join("v2.9.1");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(
        destination.join(".pumas-publishing"),
        torch::TORCH_PUBLISHING_MARKER,
    )
    .unwrap();
    std::fs::write(destination.join("payload"), "registered").unwrap();
    let marker = root.path().join(".torch-pending-publish-v2.9.1");
    std::fs::write(&marker, torch::TORCH_PUBLISHING_MARKER).unwrap();
    metadata
        .update_installed_version(
            "v2.9.1",
            InstalledVersionMetadata {
                path: "v2.9.1".into(),
                release_tag: "v2.9.1".into(),
                ..Default::default()
            },
            Some(AppId::Torch),
        )
        .unwrap();
    torch::retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
    assert_eq!(
        std::fs::read_to_string(destination.join("payload")).unwrap(),
        "registered"
    );
    assert!(!marker.exists());
    assert!(!destination.join(".pumas-publishing").exists());
}

#[test]
fn partial_publication_cleanup_preserves_ambiguous_directory() {
    let root = tempfile::tempdir().unwrap();
    let metadata = MetadataManager::new(root.path());
    let destination = root.path().join("v2.9.1");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("locked-payload"), "unfinished").unwrap();
    let marker = root.path().join(".torch-pending-publish-v2.9.1");
    std::fs::write(&marker, torch::TORCH_PUBLISHING_MARKER).unwrap();
    torch::retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
    torch::retry_pending_torch_cleanup(root.path(), &metadata).unwrap();
    assert!(destination.join("locked-payload").exists() && marker.exists());
}

#[tokio::test]
async fn direct_install_rejects_legacy_custom_tag_without_changing_existing_runtime() {
    let (installer, _root) = fixture_installer();
    let legacy = installer.versions_dir().join("torch-runtime-0.1.4");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("keep"), "legacy").unwrap();
    let mut release = upstream_release();
    release.tag_name = "torch-runtime-0.1.4".to_string();
    let (tx, _rx) = mpsc::channel(16);
    assert!(installer
        .install_version(&release.tag_name, &release, tx)
        .await
        .is_err());
    assert_eq!(
        std::fs::read_to_string(legacy.join("keep")).unwrap(),
        "legacy"
    );
    assert_no_staging(&installer);
}

#[tokio::test]
async fn stage_failure_cleans_attempt_and_preserves_legacy_runtime() {
    let (mut installer, _root) = fixture_installer();
    let legacy = installer.versions_dir().join("torch-runtime-0.1.4");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("keep"), "legacy").unwrap();
    installer.torch_stage_override = Some(Arc::new(|staging| {
        std::fs::create_dir_all(staging.join("partial")).map_err(PumasError::from)?;
        Err(PumasError::InstallationFailed {
            message: "injected staging failure".to_string(),
        })
    }));
    let (tx, _rx) = mpsc::channel(16);
    assert!(installer
        .install_version("v2.9.1", &upstream_release(), tx)
        .await
        .is_err());
    assert!(!installer.versions_dir().join("v2.9.1").exists());
    assert_eq!(
        std::fs::read_to_string(legacy.join("keep")).unwrap(),
        "legacy"
    );
    assert!(installer
        .metadata_manager
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap()
        .is_none());
    assert_no_staging(&installer);
}

#[tokio::test]
async fn cancellation_before_publication_cleans_attempt() {
    let (mut installer, _root) = fixture_installer();
    let cancel = installer.cancel_flag.clone();
    installer.torch_stage_override = Some(Arc::new(move |staging| {
        let runtime = mock_runtime(staging)?;
        cancel.store(true, Ordering::SeqCst);
        Ok(runtime)
    }));
    let (tx, _rx) = mpsc::channel(16);
    let error = installer
        .install_version("v2.9.1", &upstream_release(), tx)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert!(!installer.versions_dir().join("v2.9.1").exists());
    assert!(installer
        .metadata_manager
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap()
        .is_none());
    assert_no_staging(&installer);
}

#[tokio::test]
async fn publication_records_wheel_metadata_without_source_archive_size() {
    let (mut installer, _root) = fixture_installer();
    installer.torch_stage_override = Some(Arc::new(mock_runtime));
    let (tx, _rx) = mpsc::channel(16);
    installer
        .install_version("v2.9.1", &upstream_release(), tx)
        .await
        .unwrap();
    assert!(installer
        .versions_dir()
        .join("v2.9.1/venv/bin/python")
        .exists());
    let metadata = installer
        .metadata_manager
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap()
        .unwrap();
    assert_eq!(metadata.release_tag, "v2.9.1");
    assert_eq!(metadata.size, None);
    assert!(metadata
        .download_url
        .unwrap()
        .contains("download-r2.pytorch.org/whl/cu130/torch-2.9.1"));
    assert_no_staging(&installer);
}

#[tokio::test]
async fn existing_upstream_directory_is_never_replaced() {
    let (mut installer, _root) = fixture_installer();
    let destination = installer.versions_dir().join("v2.9.1");
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join("keep"), "first").unwrap();
    installer.torch_stage_override = Some(Arc::new(mock_runtime));
    let (tx, _rx) = mpsc::channel(16);
    assert!(installer
        .install_version("v2.9.1", &upstream_release(), tx)
        .await
        .is_err());
    assert_eq!(
        std::fs::read_to_string(destination.join("keep")).unwrap(),
        "first"
    );
    assert_no_staging(&installer);
}

#[tokio::test]
async fn interrupted_publication_is_quarantined_before_retry() {
    let (mut installer, _root) = fixture_installer();
    let destination = installer.versions_dir().join("v2.9.1");
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join(".pumas-publishing"), "metadata pending").unwrap();
    std::fs::write(destination.join("keep"), "orphaned attempt").unwrap();
    installer.torch_stage_override = Some(Arc::new(mock_runtime));
    let (tx, _rx) = mpsc::channel(16);
    installer
        .install_version("v2.9.1", &upstream_release(), tx)
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let quarantines = std::fs::read_dir(installer.versions_dir())
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".torch-orphan-v2.9.1-")
                })
                .count();
            if quarantines == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("background cleanup should reclaim the successful tag's quarantine");
    assert!(destination.join("venv/bin/python").exists());
    assert!(!destination.join(".pumas-publishing").exists());
    let recovered: Vec<_> = std::fs::read_dir(installer.versions_dir())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".torch-orphan-v2.9.1-")
        })
        .collect();
    assert!(
        recovered.is_empty(),
        "successful retry should reclaim its quarantine"
    );
}

#[test]
fn orphan_gc_keeps_two_newest_owned_dirs_and_preserves_unmarked_lookalikes() {
    let root = tempfile::tempdir().unwrap();
    let versions_dir = root.path();
    for (tag, timestamp) in [("v2.8.0", 10), ("v2.9.0", 20), ("v2.9.1", 30)] {
        let orphan = versions_dir.join(format!(".torch-orphan-{tag}-{timestamp}"));
        std::fs::create_dir_all(&orphan).unwrap();
        std::fs::write(
            orphan.join(".pumas-publishing"),
            torch::TORCH_PUBLISHING_MARKER,
        )
        .unwrap();
    }
    let lookalike = versions_dir.join(".torch-orphan-v2.9.2-40");
    std::fs::create_dir_all(&lookalike).unwrap();
    std::fs::write(lookalike.join("keep"), "not owned by the manager").unwrap();

    torch::prune_torch_orphan_quarantines(versions_dir, 2, None).unwrap();

    assert!(!versions_dir.join(".torch-orphan-v2.8.0-10").exists());
    assert!(versions_dir.join(".torch-orphan-v2.9.0-20").exists());
    assert!(versions_dir.join(".torch-orphan-v2.9.1-30").exists());
    assert!(lookalike.join("keep").exists());
}

#[tokio::test]
async fn direct_installer_shutdown_drains_scheduled_orphan_prune() {
    let (installer, _root) = fixture_installer();
    let versions_dir = installer.versions_dir();
    let orphan = versions_dir.join(".torch-orphan-v2.9.1-1");
    std::fs::create_dir_all(&orphan).unwrap();
    std::fs::write(
        orphan.join(".pumas-publishing"),
        torch::TORCH_PUBLISHING_MARKER,
    )
    .unwrap();

    torch::schedule_torch_orphan_prune(&installer.torch_cleanup, &versions_dir, 0, None, "test");
    installer.shutdown_torch_cleanup().await.unwrap();
    assert!(!orphan.exists());
}

#[tokio::test]
async fn cancelled_cleanup_drain_waiter_can_retry_without_losing_task() {
    let (installer, _root) = fixture_installer();
    let installer = Arc::new(installer);
    let (started, observed) = tokio::sync::oneshot::channel();
    let (release, wait) = std::sync::mpsc::channel();
    installer.torch_cleanup.schedule(move || {
        let _ = started.send(());
        wait.recv().unwrap();
    });
    observed.await.unwrap();

    let first_owner = installer.clone();
    let first = tokio::spawn(async move { first_owner.shutdown_torch_cleanup().await });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while installer
            .torch_cleanup
            .state
            .lock()
            .unwrap()
            .completion
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());

    let retry_owner = installer.clone();
    let mut retry = tokio::spawn(async move { retry_owner.shutdown_torch_cleanup().await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut retry)
            .await
            .is_err()
    );
    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), retry)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cancelled_child_drain_waiter_retains_slot_for_retry() {
    let cleanup = Arc::new(TorchCleanupTasks::default());
    let slot = cleanup.new_child_slot().unwrap();
    let mut command = std::process::Command::new("sh");
    command.args(["-c", "sleep 30"]);
    let child =
        pumas_library::platform::managed_child::ManagedChild::spawn(&mut command, slot.clone())
            .unwrap();
    let waiting_cleanup = cleanup.clone();
    let waiter = tokio::spawn(async move { waiting_cleanup.drain_child_slots().await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!waiter.is_finished());
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert!(slot.is_active());
    drop(child);
    cleanup.drain_child_slots().await.unwrap();
    assert!(!slot.is_active());
    assert!(!slot.has_parked_child());
}

#[tokio::test]
async fn cancelling_a_runtime_command_reaps_its_process_group() {
    let (installer, root) = fixture_installer();
    let cancel = installer.cancel_flag.clone();
    let trigger = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        cancel.store(true, Ordering::SeqCst);
    });
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    let (tx, _rx) = mpsc::channel(16);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        installer.run_runtime_command(
            command,
            &root.path().join("child.log"),
            "test child",
            &tx,
            None,
        ),
    )
    .await
    .expect("cancelled command did not terminate promptly");
    trigger.await.unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelled"));
}
