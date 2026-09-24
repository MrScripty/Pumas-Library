//! Qualify the supported upstream PyTorch recipe in an empty isolated root.
//! This exercises the production installer without publishing a release.
use pumas_app_manager::version_manager::{InstallationProgressTracker, VersionInstaller};
use pumas_library::config::AppId;
use pumas_library::metadata::MetadataManager;
use pumas_library::network::GitHubRelease;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::{mpsc, RwLock};

#[tokio::main(flavor = "current_thread")]
async fn main() -> pumas_library::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err(pumas_library::PumasError::Other(
            "Usage: qualify_torch_runtime EMPTY_ISOLATED_ROOT (installs upstream v2.9.1 recipe)"
                .to_string(),
        ));
    }
    let root = PathBuf::from(&args[1]);
    if root.exists() && std::fs::read_dir(&root)?.next().is_some() {
        return Err(pumas_library::PumasError::Other(
            "Qualification root must be empty to protect existing runtimes".to_string(),
        ));
    }
    std::fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let metadata = Arc::new(MetadataManager::new(&root));
    metadata.ensure_directories()?;
    let tracker = Arc::new(RwLock::new(InstallationProgressTracker::new(
        root.join("launcher-data/cache"),
    )));
    let installer = VersionInstaller::new(
        root,
        AppId::Torch,
        metadata,
        tracker,
        Arc::new(AtomicBool::new(false)),
    );
    let release = GitHubRelease {
        tag_name: "v2.9.1".to_string(),
        name: "PyTorch v2.9.1 upstream recipe qualification".to_string(),
        published_at: chrono::Utc::now().to_rfc3339(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease: false,
        assets: vec![],
        html_url: "https://github.com/pytorch/pytorch/releases/tag/v2.9.1".to_string(),
        total_size: None,
        archive_size: None,
        dependencies_size: None,
    };
    let (tx, mut rx) = mpsc::channel(32);
    let install = installer.install_version(&release.tag_name, &release, tx);
    let progress = async {
        while let Some(update) = rx.recv().await {
            println!("{update:?}");
        }
    };
    let (result, ()) = tokio::join!(install, progress);
    let cleanup = installer.shutdown_torch_cleanup().await;
    result.and(cleanup)
}
