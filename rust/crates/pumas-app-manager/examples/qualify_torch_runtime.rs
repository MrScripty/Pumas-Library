//! Qualify unpublished release assets using the production installation strategy.
//! This does not establish GitHub discovery or desktop acceptance.
use pumas_app_manager::version_manager::{InstallationProgressTracker, VersionInstaller};
use pumas_library::config::AppId;
use pumas_library::metadata::MetadataManager;
use pumas_library::network::{GitHubAsset, GitHubRelease};
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::{mpsc, RwLock};

#[tokio::main(flavor = "current_thread")]
async fn main() -> pumas_library::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err(pumas_library::PumasError::Other(
            "Usage: qualify_torch_runtime ISOLATED_ROOT ASSET_BASE_URL [RECIPE_TAG]".to_string(),
        ));
    }
    let root = PathBuf::from(&args[1]);
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
        tag_name: args
            .get(3)
            .cloned()
            .unwrap_or_else(|| "torch-runtime-0.1.0".to_string()),
        name: "Unpublished runtime qualification".to_string(),
        published_at: chrono::Utc::now().to_rfc3339(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease: false,
        assets: [
            "pumas-torch-runtime-linux-x86_64.tar.gz",
            "pumas-torch-runtime-linux-x86_64.tar.gz.sha256",
        ]
        .into_iter()
        .map(|name| GitHubAsset {
            name: name.to_string(),
            size: 0,
            download_url: format!("{}/{name}", args[2].trim_end_matches('/')),
            content_type: None,
        })
        .collect(),
        html_url: args[2].clone(),
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
    result
}
