//! Focused contract tests for the supported upstream PyTorch runtime.

use super::*;
use crate::version_manager::{InstallationProgressTracker, VersionInstaller, VersionManager};
use pumas_library::config::AppId;
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager, VersionsMetadata};
use pumas_library::network::{GitHubAsset, GitHubRelease, ReleasesCache};
use pumas_library::PumasError;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::RwLock;

const TORCH_SHA256: &str = "e70e1b18881e6b3c1ce402d0a989da39f956a3a057526e03c354df23d704ce9b";
const TORCHVISION_SHA256: &str = "6939dd403cc28ab0a46f53e6c86e2e852cf65771c1b0ddd09c44c541a1cdbad9";

fn release(tag: &str, prerelease: bool, assets: Vec<GitHubAsset>) -> GitHubRelease {
    GitHubRelease {
        tag_name: tag.to_string(),
        name: tag.to_string(),
        published_at: "2026-01-01T00:00:00Z".to_string(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease,
        assets,
        html_url: format!("https://github.com/pytorch/pytorch/releases/tag/{tag}"),
        total_size: None,
        archive_size: None,
        dependencies_size: None,
    }
}

fn official_wheel_url(url: &str) -> bool {
    url.starts_with("https://download.pytorch.org/")
        || url.starts_with("https://download-r2.pytorch.org/")
}

#[test]
fn v291_resolves_only_to_its_immutable_independent_recipe() {
    let recipe = torch_recipe_for_tag("v2.9.1").expect("v2.9.1 is supported");
    assert_eq!(recipe.release_tag, "v2.9.1");
    assert_eq!(recipe.recipe_id, "torch-upstream-2.9.1-r1");
    assert_ne!(recipe.recipe_id, recipe.release_tag);
    assert_eq!(recipe.torch_version, "2.9.1+cu130");

    assert!(official_wheel_url(recipe.torch_wheel_url));
    assert!(official_wheel_url(recipe.torchvision_wheel_url));
    assert!(recipe
        .torch_wheel_url
        .contains("/cu130/torch-2.9.1%2Bcu130-cp312-cp312-manylinux_2_28_x86_64.whl"));
    assert!(recipe
        .torchvision_wheel_url
        .contains("/cu130/torchvision-0.24.1%2Bcu130-cp312-cp312-manylinux_2_28_x86_64.whl"));
    assert_eq!(recipe.torch_wheel_sha256, TORCH_SHA256);
    assert_eq!(recipe.torchvision_wheel_sha256, TORCHVISION_SHA256);
    for hash in [recipe.torch_wheel_sha256, recipe.torchvision_wheel_sha256] {
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert!(!recipe
        .torch_wheel_url
        .to_ascii_lowercase()
        .contains("pumas"));
    assert!(!recipe
        .torchvision_wheel_url
        .to_ascii_lowercase()
        .contains("pumas"));

    for unsupported in ["v2.10.0", "v2.9.0", "2.9.1"] {
        assert!(
            torch_recipe_for_tag(unsupported).is_none(),
            "unexpected recipe for {unsupported}"
        );
    }
}

#[test]
fn official_release_metadata_does_not_need_pumas_bundle_assets() {
    let upstream = release("v2.9.1", false, Vec::new());
    assert!(is_torch_runtime_release(&upstream));

    assert!(is_torch_runtime_release(&release(
        "v2.10.0",
        false,
        Vec::new()
    )));
    assert!(is_torch_runtime_release(&release(
        "v2.9.0",
        false,
        Vec::new()
    )));
    assert!(!is_torch_runtime_release(&release(
        "2.9.1",
        false,
        Vec::new()
    )));
    assert!(!is_torch_runtime_release(&release(
        "v2.9.1",
        true,
        Vec::new()
    )));
}

#[test]
fn embedded_sidecar_and_lock_materialize_outside_the_checkout() {
    let destination = TempDir::new().unwrap();
    let checkout = std::env::current_dir().unwrap();
    assert!(destination.path().starts_with(std::env::temp_dir()));
    assert!(!destination.path().starts_with(&checkout));

    write_embedded_torch_runtime(destination.path()).unwrap();

    for required in [
        "serve.py",
        "validate_runtime.py",
        "requirements.txt",
        "runtime.json",
        "nunchaku_compat.py",
        "control_api.py",
        "device_manager.py",
        "diffusion.py",
        "flux2.py",
        "image_api.py",
        "model_manager.py",
        "openai_api.py",
        "validation.py",
        "loaders/__init__.py",
        "loaders/dllm_loader.py",
        "loaders/safetensors_loader.py",
        "loaders/sherry_loader.py",
    ] {
        let path = destination.path().join(required);
        assert!(path.is_file(), "missing embedded runtime file {required}");
        assert!(
            std::fs::metadata(&path).unwrap().len() > 0,
            "embedded runtime file is empty: {required}"
        );
    }

    let lock = std::fs::read_to_string(destination.path().join("requirements.txt")).unwrap();
    assert!(lock.contains("--hash=sha256:"));
    assert!(lock.contains(TORCH_SHA256));
    assert!(lock.contains(TORCHVISION_SHA256));
    assert!(!lock.to_ascii_lowercase().contains("github.com/pumas"));

    let recipe: serde_json::Value =
        serde_json::from_slice(&std::fs::read(destination.path().join("runtime.json")).unwrap())
            .unwrap();
    assert_eq!(recipe["recipe_id"], "torch-upstream-2.9.1-r1");
    assert_eq!(recipe["python"], "3.12");
    assert_eq!(recipe["platform"], "linux-x86_64");
    assert_eq!(recipe["capabilities"][0], "image_generation");
}

#[tokio::test]
async fn manager_filters_upstream_releases_and_preserves_legacy_runtime_state() {
    let root = TempDir::new().unwrap();
    let metadata_manager = MetadataManager::new(root.path());
    metadata_manager.ensure_directories().unwrap();

    let legacy_tags = ["0.1.1", "0.1.2", "0.1.3", "0.1.4", "0.1.6"];
    let mut installed = HashMap::new();
    for tag in legacy_tags {
        let runtime = root.path().join("torch-versions").join(tag);
        std::fs::create_dir_all(runtime.join("venv/bin")).unwrap();
        let recipe = if tag == "0.1.6" {
            r#"{"recipe_id":"torch-runtime-0.1.6"}"#
        } else {
            "{}"
        };
        std::fs::write(runtime.join("runtime.json"), recipe).unwrap();
        std::fs::write(runtime.join("serve.py"), "# legacy fixture\n").unwrap();
        std::fs::write(runtime.join("requirements.txt"), "# legacy fixture\n").unwrap();
        std::fs::write(runtime.join("venv/bin/python"), "# fixture\n").unwrap();
        installed.insert(
            tag.to_string(),
            InstalledVersionMetadata {
                path: tag.to_string(),
                installed_date: "2024-01-01T00:00:00Z".to_string(),
                release_tag: tag.to_string(),
                download_url: Some(format!("https://legacy.example/runtime/{tag}")),
                ..Default::default()
            },
        );
    }
    metadata_manager
        .save_versions(
            &VersionsMetadata {
                installed,
                last_selected_version: Some("0.1.6".to_string()),
                default_version: None,
            },
            Some(AppId::Torch),
        )
        .unwrap();
    std::fs::write(root.path().join(".active-version-torch"), "0.1.6\n").unwrap();

    let cache = ReleasesCache::new(
        root.path().join("launcher-data/cache"),
        Duration::from_secs(3600),
    );
    let mut supported_release = release("v2.9.1", false, Vec::new());
    supported_release.total_size = Some(348_810_776);
    supported_release.archive_size = Some(348_810_776);
    supported_release.dependencies_size = Some(123_456_789);
    let releases = [
        supported_release,
        release("v2.9.1-rc1", true, Vec::new()),
        release("v2.10.0", false, Vec::new()),
        release("v2.9.0", false, Vec::new()),
    ];
    cache
        .set_disk(AppId::Torch.github_repo(), &releases)
        .unwrap();

    let manager = VersionManager::new(root.path(), AppId::Torch)
        .await
        .unwrap();
    let discovered = manager.get_available_releases(false).await.unwrap();
    assert_eq!(
        discovered
            .iter()
            .map(|item| item.tag_name.as_str())
            .collect::<Vec<_>>(),
        ["v2.9.1", "v2.10.0", "v2.9.0"]
    );
    assert!(discovered[0].assets.is_empty());
    assert_eq!(discovered[0].total_size, None);
    assert_eq!(discovered[0].archive_size, None);
    assert_eq!(discovered[0].dependencies_size, None);
    let by_tag = manager
        .get_release_by_tag("v2.9.1", false)
        .await
        .unwrap()
        .expect("supported release should resolve by tag");
    assert_eq!(by_tag.total_size, None);
    assert_eq!(by_tag.archive_size, None);
    assert_eq!(by_tag.dependencies_size, None);

    let mut installed_after = manager.get_installed_versions().await.unwrap();
    installed_after.sort();
    assert_eq!(installed_after, legacy_tags.map(str::to_string).to_vec());
    assert_eq!(
        manager.get_active_version().await.unwrap().as_deref(),
        Some("0.1.6")
    );
    assert_eq!(manager.get_default_version().await.unwrap(), None);

    let saved = metadata_manager.load_versions(Some(AppId::Torch)).unwrap();
    assert_eq!(saved.installed.len(), 5);
    for tag in legacy_tags {
        assert_eq!(saved.installed[tag].release_tag, tag);
        assert_eq!(
            saved.installed[tag].download_url.as_deref(),
            Some(format!("https://legacy.example/runtime/{tag}").as_str())
        );
    }
    assert_eq!(saved.default_version, None);

    assert!(matches!(
        manager.install_version("v2.10.0-rc1").await,
        Err(pumas_library::PumasError::VersionNotFound { .. })
    ));
    assert!(!manager.is_installing().await);
    assert!(manager.get_installation_progress().await.is_none());
}

#[tokio::test]
async fn direct_installer_preserves_existing_upstream_directory() {
    let root = TempDir::new().unwrap();
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
    let destination = root.path().join("torch-versions/v2.10.0");
    std::fs::create_dir_all(&destination).unwrap();
    let unsupported = release("v2.10.0", false, Vec::new());
    let (progress_tx, _progress_rx) = tokio::sync::mpsc::channel(1);

    let error = installer
        .install_version("v2.10.0", &unsupported, progress_tx)
        .await
        .expect_err("existing directory must be preserved before staging");
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert!(error
            .to_string()
            .contains("Runtime directory already exists"));
    }
    assert!(destination.is_dir());
}

fn mock_runtime_for_publication(
    staging: &std::path::Path,
) -> pumas_library::Result<std::path::PathBuf> {
    let runtime = staging.join("runtime");
    std::fs::create_dir_all(runtime.join("venv/bin")).map_err(PumasError::from)?;
    std::fs::write(
        runtime.join("runtime.json"),
        r#"{"recipe_id":"torch-upstream-2.9.1-r1"}"#,
    )
    .map_err(PumasError::from)?;
    std::fs::write(runtime.join("serve.py"), "# isolated publication fixture\n")
        .map_err(PumasError::from)?;
    std::fs::write(
        runtime.join("requirements.txt"),
        "# isolated publication fixture\n",
    )
    .map_err(PumasError::from)?;
    std::fs::write(runtime.join("venv/bin/python"), "# fake interpreter\n")
        .map_err(PumasError::from)?;
    Ok(runtime)
}

#[tokio::test]
async fn unverified_release_installs_without_becoming_active_or_default() {
    let root = TempDir::new().unwrap();
    let cache = ReleasesCache::new(
        root.path().join("launcher-data/cache"),
        Duration::from_secs(3600),
    );
    let upstream = release("v2.10.0", false, Vec::new());
    cache
        .set_disk(AppId::Torch.github_repo(), &[upstream])
        .unwrap();
    let manager = VersionManager::new(root.path(), AppId::Torch)
        .await.unwrap()
        .with_torch_stage_override(|staging| {
            let runtime = mock_runtime_for_publication(staging)?;
            std::fs::write(runtime.join("resolution.json"),
                r#"{"artifacts":[{"name":"torch","url":"https://download.pytorch.org/whl/cpu/torch/example.whl"}]}"#)
                .map_err(PumasError::from)?;
            Ok(runtime)
        });
    let mut progress = manager.install_version("v2.10.0").await.unwrap();
    while let Some(update) = progress.recv().await {
        if matches!(
            update,
            crate::version_manager::ProgressUpdate::Completed { success: true }
        ) {
            break;
        }
        if let crate::version_manager::ProgressUpdate::Error { message } = update {
            panic!("unverified release fixture failed: {message}");
        }
    }
    assert_eq!(manager.get_active_version().await.unwrap(), None);
    assert_eq!(manager.get_default_version().await.unwrap(), None);
    assert!(root.path().join("torch-versions/v2.10.0").is_dir());
    let reconstructed = VersionManager::new(root.path(), AppId::Torch)
        .await
        .unwrap();
    assert_eq!(reconstructed.get_active_version().await.unwrap(), None);
    assert_eq!(reconstructed.get_default_version().await.unwrap(), None);
}

#[tokio::test]
async fn cancel_after_publication_begins_is_rejected() {
    let root = TempDir::new().unwrap();
    let cache = ReleasesCache::new(
        root.path().join("launcher-data/cache"),
        Duration::from_secs(3600),
    );
    let supported = release("v2.9.1", false, Vec::new());
    cache
        .set_disk(AppId::Torch.github_repo(), std::slice::from_ref(&supported))
        .unwrap();

    let pause = Arc::new(TorchPublicationPause::new());
    let manager = VersionManager::new(root.path(), AppId::Torch)
        .await
        .unwrap()
        .with_torch_publication_pause(pause.clone())
        .with_torch_stage_override(mock_runtime_for_publication);
    let reached = pause.reached.notified();
    let mut progress = manager.install_version("v2.9.1").await.unwrap();

    tokio::time::timeout(Duration::from_secs(10), reached)
        .await
        .expect("installer did not reach the publication boundary");
    assert_eq!(
        manager.get_installing_tag().await.as_deref(),
        Some("v2.9.1")
    );
    let cancellation_accepted = manager.cancel_installation().await.unwrap();
    let cancellation_flag = manager
        .cancel_flag
        .load(std::sync::atomic::Ordering::SeqCst);

    pause.resume.add_permits(1);
    assert!(!cancellation_accepted);
    assert!(!cancellation_flag);
    let completed = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match progress.recv().await {
                Some(crate::version_manager::ProgressUpdate::Completed { success }) => {
                    return success;
                }
                Some(crate::version_manager::ProgressUpdate::Error { message }) => {
                    panic!("publication fixture failed: {message}");
                }
                Some(_) => {}
                None => panic!("progress channel closed before installation completed"),
            }
        }
    })
    .await
    .expect("installer did not finish after publication resumed");
    assert!(completed);
    assert!(!manager.is_installing().await);

    let installed_dir = root.path().join("torch-versions/v2.9.1");
    assert!(installed_dir.is_dir());
    let metadata = manager
        .metadata_manager
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap()
        .expect("publication must durably record installed-version metadata");
    assert_eq!(metadata.release_tag, "v2.9.1");
    assert!(metadata
        .download_url
        .unwrap()
        .contains("download-r2.pytorch.org/whl/cu130/torch-2.9.1"));
}

#[tokio::test]
async fn concurrent_direct_install_rejection_does_not_disrupt_active_torch_attempt() {
    let root = TempDir::new().unwrap();
    let metadata = Arc::new(MetadataManager::new(root.path()));
    metadata.ensure_directories().unwrap();
    let tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
        root.path().join("launcher-data/cache"),
    )));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let pause = Arc::new(TorchPublicationPause::new());
    let installer = Arc::new(
        VersionInstaller::new(
            root.path().to_path_buf(),
            AppId::Torch,
            metadata.clone(),
            tracker,
            cancel_flag,
        )
        .with_torch_stage_override(Arc::new(mock_runtime_for_publication))
        .with_torch_stage_pause(pause.clone()),
    );

    let reached = pause.reached.notified();
    let (first_tx, _first_rx) = tokio::sync::mpsc::channel(16);
    let first_installer = installer.clone();
    let first_release = release("v2.9.1", false, Vec::new());
    let first = tokio::spawn(async move {
        first_installer
            .install_version("v2.9.1", &first_release, first_tx)
            .await
    });

    tokio::time::timeout(Duration::from_secs(10), reached)
        .await
        .expect("first install did not reach the staged-runtime pause");
    assert!(!installer.versions_dir().join("v2.9.1").exists());

    let (second_tx, _second_rx) = tokio::sync::mpsc::channel(16);
    let second_release = release("v2.10.0", false, Vec::new());
    let second_error = installer
        .install_version("v2.10.0", &second_release, second_tx)
        .await
        .expect_err("a second direct call must be rejected while the first is active");
    assert!(second_error
        .to_string()
        .contains("Torch installation already active"));

    pause.resume.add_permits(1);
    tokio::time::timeout(Duration::from_secs(10), first)
        .await
        .expect("first install did not finish after the stage pause resumed")
        .expect("first install task panicked")
        .expect("concurrent call must not disrupt the supported install");

    let installed = installer.versions_dir().join("v2.9.1");
    assert!(installed.is_dir());
    let installed_metadata = metadata
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap()
        .expect("successful publication must persist runtime metadata");
    assert_eq!(installed_metadata.release_tag, "v2.9.1");
    assert!(installed_metadata
        .download_url
        .unwrap()
        .contains("download-r2.pytorch.org/whl/cu130/torch-2.9.1"));
}
