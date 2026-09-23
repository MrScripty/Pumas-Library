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
        installer.run_runtime_command(command, &root.path().join("child.log"), "test child", &tx),
    )
    .await
    .expect("cancelled command did not terminate promptly");
    trigger.await.unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelled"));
}
