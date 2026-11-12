//! Ollama install integrity and native archive extraction bounds.

use super::*;
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt as _};

const TAG: &str = "v0.1.2";

/// The asset name `select_ollama_asset` picks on this host, as a `.tgz`.
fn host_ollama_asset() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("ollama-{}-{arch}.tgz", std::env::consts::OS)
}

fn ollama_binary_name() -> &'static str {
    if cfg!(windows) {
        "ollama.exe"
    } else {
        "ollama"
    }
}

/// Build a gzip tarball of regular `(path, contents, mode)` files followed by
/// `(path, target)` symbolic links.
fn tarball(files: &[(&str, &[u8], u32)], links: &[(&str, &str)]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (path, contents, mode) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(*mode);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder.append_data(&mut header, path, *contents).unwrap();
    }
    for (path, target) in links {
        let mut header = tar::Header::new_gnu();
        header.set_size(0);
        header.set_mode(0o777);
        header.set_entry_type(tar::EntryType::Symlink);
        builder.append_link(&mut header, path, target).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A loopback GitHub API and artifact origin for one Ollama release.
struct OllamaFixture {
    base_url: String,
    archive_requests: Arc<AtomicUsize>,
    server: JoinHandle<()>,
}

impl Drop for OllamaFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn serve_ollama_release(archive: Vec<u8>, digest: Option<String>) -> OllamaFixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let archive_requests = Arc::new(AtomicUsize::new(0));
    let requests = archive_requests.clone();
    let download_url = format!("{base_url}/archive");
    let server = tokio::spawn(async move {
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
            let body =
                if line.starts_with(&format!("GET /repos/ollama/ollama/releases/tags/{TAG} ")) {
                    serde_json::to_vec(&serde_json::json!({
                        "tag_name": TAG,
                        "assets": [{
                            "id": 4242,
                            "name": host_ollama_asset(),
                            "size": archive.len(),
                            "browser_download_url": download_url,
                            "digest": digest.as_ref().map(|digest| format!("sha256:{digest}")),
                        }]
                    }))
                    .unwrap()
                } else if line.starts_with("GET /archive ") {
                    requests.fetch_add(1, Ordering::SeqCst);
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
    OllamaFixture {
        base_url,
        archive_requests,
        server,
    }
}

fn ollama_installer(root: &Path, base_url: &str) -> (VersionInstaller, Arc<MetadataManager>) {
    let metadata = Arc::new(MetadataManager::new(root));
    metadata.ensure_directories().unwrap();
    let installer = VersionInstaller::new(
        root.to_path_buf(),
        AppId::Ollama,
        metadata.clone(),
        Arc::new(RwLock::new(InstallationProgressTracker::new(
            root.join("launcher-data/cache"),
        ))),
        Arc::new(AtomicBool::new(false)),
    )
    .with_github_client(Arc::new(
        GitHubClient::with_loopback_api(
            root.join("launcher-data/cache"),
            Duration::from_secs(3600),
            base_url.to_owned(),
        )
        .unwrap(),
    ));
    (installer, metadata)
}

fn ollama_release(fixture: &OllamaFixture, size: u64) -> GitHubRelease {
    GitHubRelease {
        tag_name: TAG.into(),
        name: "Ollama fixture".into(),
        published_at: "2026-09-29T00:00:00Z".into(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease: false,
        assets: vec![GitHubAsset {
            name: host_ollama_asset(),
            size,
            download_url: format!("{}/archive", fixture.base_url),
            content_type: Some("application/gzip".into()),
        }],
        html_url: format!("https://github.com/ollama/ollama/releases/tag/{TAG}"),
        total_size: Some(size),
        archive_size: Some(size),
        dependencies_size: None,
        unavailable_reason: None,
    }
}

async fn install(root: &Path, fixture: &OllamaFixture, size: u64) -> Result<()> {
    let (installer, _) = ollama_installer(root, &fixture.base_url);
    // A closed receiver never blocks progress delivery.
    let (tx, rx) = mpsc::channel(1);
    drop(rx);
    installer
        .install_version(TAG, &ollama_release(fixture, size), tx)
        .await
}

fn cached_archive(root: &Path) -> PathBuf {
    root.join("launcher-data/cache/downloads")
        .join(host_ollama_asset())
}

fn assert_nothing_installed(root: &Path) {
    let versions = root.join("ollama-versions");
    assert!(!versions.join(TAG).exists());
    if versions.exists() {
        assert!(
            std::fs::read_dir(&versions).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".ollama-install-")),
            "a failed attempt must not leave its stage behind"
        );
    }
    assert!(MetadataManager::new(root)
        .get_installed_version(TAG, Some(AppId::Ollama))
        .unwrap()
        .is_none());
}

fn genuine_archive() -> Vec<u8> {
    tarball(&[("bin/ollama", b"genuine ollama binary", 0o4755)], &[])
}

#[tokio::test]
async fn ollama_install_publishes_only_publisher_verified_bytes() {
    let root = tempfile::tempdir().unwrap();
    let archive = genuine_archive();
    let fixture = serve_ollama_release(archive.clone(), Some(sha256_hex(&archive))).await;

    install(root.path(), &fixture, archive.len() as u64)
        .await
        .unwrap();

    let binary = root
        .path()
        .join("ollama-versions")
        .join(TAG)
        .join(ollama_binary_name());
    assert_eq!(std::fs::read(&binary).unwrap(), b"genuine ollama binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&binary).unwrap().permissions().mode() & 0o7777,
            0o755,
            "the archive's setuid bit must not survive"
        );
    }
    assert!(MetadataManager::new(root.path())
        .get_installed_version(TAG, Some(AppId::Ollama))
        .unwrap()
        .is_some());
    assert_eq!(std::fs::read(cached_archive(root.path())).unwrap(), archive);
    assert_eq!(fixture.archive_requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ollama_install_refuses_a_wrong_or_missing_publisher_digest() {
    for digest in [Some(sha256_hex(b"different bytes")), None] {
        let root = tempfile::tempdir().unwrap();
        let archive = genuine_archive();
        let missing = digest.is_none();
        let fixture = serve_ollama_release(archive.clone(), digest).await;

        let error = install(root.path(), &fixture, archive.len() as u64)
            .await
            .unwrap_err()
            .to_string();

        if missing {
            assert!(error.contains("no publisher SHA-256 digest"), "{error}");
            assert_eq!(fixture.archive_requests.load(Ordering::SeqCst), 0);
        } else {
            assert!(
                error.contains("does not match the publisher digest"),
                "{error}"
            );
        }
        assert_nothing_installed(root.path());
        assert!(!cached_archive(root.path()).exists());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn ollama_install_refuses_a_symlinked_binary() {
    let root = tempfile::tempdir().unwrap();
    // The link stays inside the output, so only the binary check refuses it.
    let archive = tarball(
        &[("bin/libggml.so", b"shared library", 0o644)],
        &[("bin/ollama", "libggml.so")],
    );
    let fixture = serve_ollama_release(archive.clone(), Some(sha256_hex(&archive))).await;

    let error = install(root.path(), &fixture, archive.len() as u64)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("symbolic link"), "{error}");
    assert_nothing_installed(root.path());
}

#[tokio::test]
async fn ollama_cached_archive_is_reverified_by_hash() {
    let archive = genuine_archive();

    // A same-size tampered cache entry is discarded and downloaded again.
    let root = tempfile::tempdir().unwrap();
    let fixture = serve_ollama_release(archive.clone(), Some(sha256_hex(&archive))).await;
    let cache = cached_archive(root.path());
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    let mut tampered = archive.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0xff;
    std::fs::write(&cache, &tampered).unwrap();

    install(root.path(), &fixture, archive.len() as u64)
        .await
        .unwrap();
    assert_eq!(fixture.archive_requests.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read(&cache).unwrap(), archive);

    // A cache entry that still matches the digest is reused without download.
    let root = tempfile::tempdir().unwrap();
    let fixture = serve_ollama_release(archive.clone(), Some(sha256_hex(&archive))).await;
    let cache = cached_archive(root.path());
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, &archive).unwrap();

    install(root.path(), &fixture, archive.len() as u64)
        .await
        .unwrap();
    assert_eq!(fixture.archive_requests.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[test]
fn zip_extraction_strips_setuid_setgid_and_sticky_bits() {
    use std::os::unix::fs::PermissionsExt;
    use zip::{write::SimpleFileOptions, ZipWriter};

    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("modes.zip");
    let output = temp.path().join("out");
    let mut writer = ZipWriter::new(File::create(&archive_path).unwrap());
    writer
        .start_file(
            "bin/server",
            SimpleFileOptions::default().unix_permissions(0o7755),
        )
        .unwrap();
    writer.write_all(b"server").unwrap();
    writer.finish().unwrap();

    VersionInstaller::extract_zip(&archive_path, &output).unwrap();
    assert_eq!(
        std::fs::metadata(output.join("bin/server"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
}

#[test]
fn oversized_archives_are_refused_as_decompression_bombs() {
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    let zeros = vec![0u8; 8 * 1024 * 1024];
    let temp = tempfile::tempdir().unwrap();

    let tgz = temp.path().join("bomb.tgz");
    std::fs::write(&tgz, tarball(&[("bomb", &zeros, 0o644)], &[])).unwrap();
    let error = VersionInstaller::extract_tarball_from(
        File::open(&tgz).unwrap(),
        &temp.path().join("tgz-out"),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("decompression bomb"), "{error}");
    assert!(!temp.path().join("tgz-out/bomb").exists());

    let zip_path = temp.path().join("bomb.zip");
    let mut writer = ZipWriter::new(File::create(&zip_path).unwrap());
    writer
        .start_file(
            "bomb",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(&zeros).unwrap();
    writer.finish().unwrap();
    let error = VersionInstaller::extract_zip(&zip_path, &temp.path().join("zip-out"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("decompression bomb"), "{error}");
}

#[test]
fn extraction_budget_scales_with_the_archive_and_explains_refusals() {
    let mut small = ExtractionBudget::for_archive_bytes(32);
    small.admit(10).unwrap();
    small.admit(10).unwrap();
    let error = small.admit(0).unwrap_err().to_string();
    assert!(error.contains("more than 2 entries"), "{error}");

    let mut large = ExtractionBudget::for_archive_bytes(1 << 20);
    large
        .admit(ExtractionBudget::MAX_EXPANSION_RATIO << 20)
        .unwrap();
    let error = large.admit(1).unwrap_err().to_string();
    assert!(error.contains("64x"), "{error}");
}
