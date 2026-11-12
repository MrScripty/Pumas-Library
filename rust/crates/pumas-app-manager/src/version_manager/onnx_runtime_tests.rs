//! ONNX Runtime as a downloaded, version-managed native runtime.
//!
//! A loopback server stands in for the GitHub release API and the release
//! archive. The archive mirrors the upstream layout (one wrapper directory,
//! `lib/` with the unversioned library name linked to the versioned file),
//! but holds fixture bytes: no real ONNX Runtime is downloaded.

use super::*;
use pumas_library::network::{GitHubAsset, GitHubRelease, ReleasesCache};
use pumas_library::onnx_runtime_distribution as distribution;
use sha2::Digest;
use std::sync::atomic::AtomicUsize;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const VERSION: &str = "1.24.2";
const RELEASE_TAG: &str = "v1.24.2";
const INSTALLED_TAG: &str = "v1.24.2+cpu";
const LIBRARY_BYTES: &[u8] = b"onnx runtime library fixture";

fn host_archive_name() -> String {
    let version = distribution::release_version(VERSION).unwrap();
    distribution::platform_archive_names(&version, std::env::consts::OS, std::env::consts::ARCH)
        .into_iter()
        .next()
        .expect("test host has an upstream ONNX Runtime archive")
}

/// Upstream-shaped `.tgz`: `onnxruntime-<platform>-<version>/lib/<name>` links
/// to the versioned library file, as the Linux and macOS archives do.
fn upstream_shaped_archive() -> Vec<u8> {
    let wrapper = host_archive_name().trim_end_matches(".tgz").to_owned();
    let entry = Path::new(distribution::library_relative_path());
    let unversioned = entry.file_name().unwrap().to_str().unwrap().to_owned();
    let versioned = if cfg!(target_os = "macos") {
        format!("libonnxruntime.{VERSION}.dylib")
    } else {
        format!("{unversioned}.{VERSION}")
    };
    let compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(compressed);
    let mut append_file = |path: String, bytes: &[u8]| {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, path, bytes).unwrap();
    };
    append_file(format!("{wrapper}/lib/{versioned}"), LIBRARY_BYTES);
    append_file(
        format!("{wrapper}/include/onnxruntime_c_api.h"),
        b"/* api */",
    );
    append_file(format!("{wrapper}/LICENSE"), b"license fixture");
    append_file(format!("{wrapper}/VERSION_NUMBER"), VERSION.as_bytes());
    let mut append_link = |name: &str, target: &str| {
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_size(0);
        link.set_mode(0o777);
        archive
            .append_link(&mut link, format!("{wrapper}/lib/{name}"), target)
            .unwrap();
    };
    if cfg!(target_os = "macos") {
        append_link(&unversioned, &versioned);
    } else {
        // Linux archives chain the names: .so -> .so.1 -> .so.<version>.
        let soname = format!("{unversioned}.1");
        append_link(&soname, &versioned);
        append_link(&unversioned, &soname);
    }
    archive.into_inner().unwrap().finish().unwrap()
}

struct ReleaseServer {
    base_url: String,
    archive_requests: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for ReleaseServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Serve publisher metadata for `RELEASE_TAG` and the archive bytes.
async fn release_server(root: &Path, archive: Vec<u8>) -> ReleaseServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let archive_url = format!("{base_url}/archive");
    let asset_name = host_archive_name();
    let size = archive.len() as u64;
    let digest = format!("{:x}", sha2::Sha256::digest(&archive));
    let listed_asset = |name: &str, size: u64| GitHubAsset {
        name: name.into(),
        size,
        download_url: archive_url.clone(),
        content_type: Some("application/gzip".into()),
    };
    let release = |tag: &str, assets: Vec<GitHubAsset>| GitHubRelease {
        tag_name: tag.into(),
        name: tag.into(),
        published_at: "2026-10-01T00:00:00Z".into(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease: false,
        assets,
        html_url: format!("https://github.com/microsoft/onnxruntime/releases/tag/{tag}"),
        total_size: None,
        archive_size: None,
        dependencies_size: None,
        unavailable_reason: None,
        install_availability: None,
    };
    ReleasesCache::new(root.join("launcher-data/cache"), Duration::from_secs(3600))
        .set_disk(
            AppId::OnnxRuntime.github_repo(),
            &[
                release(RELEASE_TAG, vec![listed_asset(&asset_name, size)]),
                // Older than the C API the binding requests: never offered.
                release(
                    "v1.23.2",
                    vec![listed_asset(&asset_name.replace(VERSION, "1.23.2"), size)],
                ),
            ],
        )
        .unwrap();
    let metadata = serde_json::to_vec(&serde_json::json!({
        "tag_name": RELEASE_TAG,
        "assets": [{
            "id": 2402, "name": asset_name, "size": size,
            "browser_download_url": archive_url,
            "digest": format!("sha256:{digest}")
        }]
    }))
    .unwrap();
    let archive_requests = Arc::new(AtomicUsize::new(0));
    let counted = archive_requests.clone();
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut request = [0; 4096];
            let read = stream.read(&mut request).await.unwrap_or(0);
            let line = String::from_utf8_lossy(&request[..read])
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned();
            let body = if line.starts_with(&format!(
                "GET /repos/microsoft/onnxruntime/releases/tags/{RELEASE_TAG} "
            )) {
                metadata.clone()
            } else if line.starts_with("GET /archive ") {
                counted.fetch_add(1, Ordering::SeqCst);
                archive.clone()
            } else {
                let _ = stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await;
                continue;
            };
            let _ = stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await;
            let _ = stream.write_all(&body).await;
        }
    });
    ReleaseServer {
        base_url,
        archive_requests,
        task,
    }
}

async fn onnx_manager(
    root: &Path,
    api: &pumas_library::PumasApi,
    base_url: &str,
) -> VersionManager {
    let mut manager =
        VersionManager::new_with_acquisition(root, AppId::OnnxRuntime, api.acquisition().clone())
            .await
            .unwrap();
    manager.github_client = Arc::new(
        GitHubClient::with_loopback_api(
            manager.cache_dir(),
            Duration::from_secs(3600),
            base_url.to_owned(),
        )
        .unwrap(),
    );
    manager
}

async fn wait_for_completion(mut updates: mpsc::Receiver<ProgressUpdate>) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match updates.recv().await {
                Some(ProgressUpdate::Completed { success: true }) => break,
                Some(ProgressUpdate::Error { message }) => {
                    panic!("ONNX Runtime installation failed: {message}")
                }
                Some(_) => {}
                None => panic!("installation ended without completion"),
            }
        }
    })
    .await
    .unwrap();
}

async fn test_api(root: &Path) -> pumas_library::PumasApi {
    pumas_library::PumasApi::builder(root)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}

#[tokio::test]
async fn onnx_runtime_installs_selects_and_reopens_a_downloaded_version() {
    let root = TempDir::new().unwrap();
    let server = release_server(root.path(), upstream_shaped_archive()).await;
    let api = test_api(root.path()).await;
    let manager = onnx_manager(root.path(), &api, &server.base_url).await;

    let listed = manager.get_available_releases(false).await.unwrap();
    let tags: Vec<_> = listed
        .iter()
        .map(|release| release.tag_name.as_str())
        .collect();
    assert_eq!(tags, vec![INSTALLED_TAG]);

    let refused = manager.install_version("v1.23.2+cpu").await.unwrap_err();
    assert!(
        refused.to_string().contains("does not provide C API"),
        "{refused}"
    );

    wait_for_completion(manager.install_version(INSTALLED_TAG).await.unwrap()).await;
    assert_eq!(server.archive_requests.load(Ordering::SeqCst), 1);

    let version_dir = manager.version_path(INSTALLED_TAG).unwrap();
    assert!(version_dir.starts_with(root.path().join("onnx-runtime-versions")));
    let library = version_dir.join(distribution::library_relative_path());
    let metadata = std::fs::symlink_metadata(&library).unwrap();
    assert!(metadata.is_file(), "the loaded library is a regular file");
    assert_eq!(std::fs::read(&library).unwrap(), LIBRARY_BYTES);
    // Every upstream library name still resolves, as a link, to that file.
    let other_names: Vec<_> = std::fs::read_dir(library.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path != &library)
        .collect();
    let expected_links = if cfg!(target_os = "macos") { 1 } else { 2 };
    assert_eq!(other_names.len(), expected_links, "{other_names:?}");
    for name in &other_names {
        assert!(std::fs::symlink_metadata(name)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::canonicalize(name).unwrap(),
            std::fs::canonicalize(&library).unwrap()
        );
    }
    assert!(version_dir.join("include/onnxruntime_c_api.h").is_file());
    assert!(version_dir.join("LICENSE").is_file());

    assert_eq!(
        manager.get_installed_versions().await.unwrap(),
        vec![INSTALLED_TAG]
    );
    assert!(manager.set_active_version(INSTALLED_TAG).await.unwrap());
    assert!(manager
        .set_default_version(Some(INSTALLED_TAG))
        .await
        .unwrap());
    assert_eq!(
        std::fs::read_to_string(root.path().join(".active-version-onnx-runtime")).unwrap(),
        INSTALLED_TAG
    );
    assert!(
        !root.path().join(".active-version").exists(),
        "ONNX Runtime selection must not touch the llama.cpp/Ollama marker"
    );
    manager.shutdown_installations().await.unwrap();
    drop(manager);

    let reopened = onnx_manager(root.path(), &api, &server.base_url).await;
    assert_eq!(
        reopened.get_installed_versions().await.unwrap(),
        vec![INSTALLED_TAG]
    );
    assert_eq!(
        reopened.get_active_version().await.unwrap().as_deref(),
        Some(INSTALLED_TAG)
    );
    assert_eq!(
        reopened.get_default_version().await.unwrap().as_deref(),
        Some(INSTALLED_TAG)
    );
    let still_active = reopened.remove_version(INSTALLED_TAG).await.unwrap_err();
    assert!(still_active.to_string().contains("currently active"));
    reopened.shutdown_installations().await.unwrap();
}

#[tokio::test]
async fn onnx_runtime_archive_without_the_library_is_refused_with_a_reason() {
    let root = TempDir::new().unwrap();
    let compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(compressed);
    let mut header = tar::Header::new_gnu();
    header.set_size(6);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "onnxruntime/README", &b"readme"[..])
        .unwrap();
    let bytes = archive.into_inner().unwrap().finish().unwrap();
    let server = release_server(root.path(), bytes).await;
    let api = test_api(root.path()).await;
    let manager = onnx_manager(root.path(), &api, &server.base_url).await;

    let mut updates = manager.install_version(INSTALLED_TAG).await.unwrap();
    let message = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match updates.recv().await {
                Some(ProgressUpdate::Error { message }) => break message,
                Some(ProgressUpdate::Completed { success: true }) => {
                    panic!("an archive without the library must not install")
                }
                Some(_) => {}
                None => break String::new(),
            }
        }
    })
    .await
    .unwrap();
    manager.shutdown_installations().await.ok();
    let progress = manager.get_installation_progress().await;
    let reported = progress
        .and_then(|progress| progress.error)
        .unwrap_or(message);
    assert!(
        reported.contains(distribution::library_relative_path()),
        "{reported}"
    );
    assert!(manager.get_installed_versions().await.unwrap().is_empty());
}

#[tokio::test]
async fn onnx_runtime_does_not_share_the_llama_cpp_acquisition_consumer() {
    let root = TempDir::new().unwrap();
    let api = test_api(root.path()).await;
    let llama = VersionManager::new_with_acquisition(
        root.path(),
        AppId::LlamaCpp,
        api.acquisition().clone(),
    )
    .await
    .unwrap();
    let onnx = VersionManager::new_with_acquisition(
        root.path(),
        AppId::OnnxRuntime,
        api.acquisition().clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        llama.acquisition_consumer.as_ref().unwrap().owner(),
        "runtime.llama.cpp"
    );
    assert_eq!(
        onnx.acquisition_consumer.as_ref().unwrap().owner(),
        "runtime.onnx-runtime"
    );
    assert_ne!(llama.versions_dir(), onnx.versions_dir());
    llama.shutdown_installations().await.unwrap();
    onnx.shutdown_installations().await.unwrap();
}
