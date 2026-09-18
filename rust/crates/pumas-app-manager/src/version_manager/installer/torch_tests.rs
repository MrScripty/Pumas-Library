//! HTTP bundle fixtures exercise the production installer, without model downloads.
use super::*;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

fn create_test_installer() -> (VersionInstaller, tempfile::TempDir) {
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

fn bundle(requirements: &str, validation: &str) -> Vec<u8> {
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for (name, data) in [
        (
            "runtime.json",
            r#"{"recipe_id":"torch-runtime-0.1.0","protocol":2,"python":"3.12","platform":"linux-x86_64"}"#,
        ),
        ("serve.py", ""),
        ("requirements.txt", requirements),
        ("validate_runtime.py", validation),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, name, data.as_bytes())
            .unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

async fn fixture_release(
    archive: Vec<u8>,
    valid_checksum: bool,
) -> (GitHubRelease, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let checksum = if valid_checksum {
        format!("{:x}", Sha256::digest(&archive))
    } else {
        "0".repeat(64)
    };
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let n = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(n, 0, "fixture request ended before its headers");
                    request.extend_from_slice(&buffer[..n]);
                    assert!(request.len() <= 8192, "fixture request headers too large");
                }
            })
            .await
            .expect("fixture request headers did not arrive");
            let body = if String::from_utf8_lossy(&request).contains(".sha256") {
                checksum.as_bytes()
            } else {
                &archive
            };
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(body).await.unwrap();
        }
    });
    let release = GitHubRelease {
        tag_name: "torch-runtime-0.1.0".to_string(),
        name: "fixture".to_string(),
        published_at: Utc::now().to_rfc3339(),
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
            download_url: format!("{base}/{name}"),
            content_type: None,
        })
        .collect(),
        html_url: base,
        total_size: None,
        archive_size: None,
        dependencies_size: None,
    };
    (release, server)
}

#[tokio::test]
async fn bundle_fixture_waits_for_a_complete_request() {
    let archive = bundle("--no-index\n", "");
    let expected_checksum = format!("{:x}", Sha256::digest(&archive));
    let (release, server) = fixture_release(archive, true).await;
    let address = release.html_url.strip_prefix("http://").unwrap();
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(b"GET /pumas-torch-runtime-linux-x86_64.tar.gz")
        .await
        .unwrap();
    let premature_response =
        tokio::time::timeout(std::time::Duration::from_millis(100), stream.readable())
            .await
            .is_ok();
    stream
        .write_all(b".sha256 HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        stream.read_to_string(&mut response),
    )
    .await;
    server.abort();
    assert!(
        !premature_response,
        "fixture answered an incomplete request"
    );
    read.unwrap().unwrap();
    assert!(response.ends_with(&expected_checksum));
}

#[test]
fn completed_download_is_immediately_readable_by_checksum_reader() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap()
        .block_on(async {
            let (installer, root) = create_test_installer();
            let destination = root.path().join("archive.tar.gz");
            // A single-byte response guarantees a single write: a later write
            // would implicitly wait for the first and mask the completion race.
            let archive = vec![0x5a];
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/archive", listener.local_addr().unwrap());
            let (body_tx, body_rx) = tokio::sync::oneshot::channel();
            let body = archive.clone();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert_ne!(n, 0);
                    request.extend_from_slice(&buffer[..n]);
                }
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                body_rx.await.unwrap();
                socket.write_all(&body).await.unwrap();
            });
            let (tx, _rx) = mpsc::channel(32);
            let completed = AtomicBool::new(false);
            let download = async {
                installer
                    .download_archive(&url, &destination, &tx)
                    .await
                    .unwrap();
                completed.store(true, Ordering::SeqCst);
            };
            let delayed_disk = async {
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    while !destination.exists() {
                        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    }
                })
                .await
                .unwrap();
                // Occupy Tokio's only filesystem worker after file creation,
                // then send the body. The final write must remain queued until
                // this worker is released; download completion must wait for it.
                let (started_tx, started_rx) = tokio::sync::oneshot::channel();
                let (release_tx, release_rx) = std::sync::mpsc::channel();
                let worker = tokio::task::spawn_blocking(move || {
                    started_tx.send(()).unwrap();
                    let _ = release_rx.recv();
                });
                started_rx.await.unwrap();
                body_tx.send(()).unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                let premature_completion = completed.load(Ordering::SeqCst);
                release_tx.send(()).unwrap();
                worker.await.unwrap();
                premature_completion
            };
            let ((), premature_completion) = tokio::join!(download, delayed_disk);
            server.await.unwrap();
            assert!(
                !premature_completion,
                "download completed before its file write"
            );
            assert_eq!(std::fs::read(destination).unwrap(), archive);
        });
}

async fn rejected_bundle_preserves_previous(
    requirements: &str,
    validation: &str,
    valid_checksum: bool,
    expected: &str,
) {
    let (installer, root) = create_test_installer();
    let previous = root.path().join("torch-versions/torch-runtime-old");
    std::fs::create_dir_all(previous.join("venv/bin")).unwrap();
    for name in ["runtime.json", "serve.py", "requirements.txt"] {
        std::fs::write(previous.join(name), "previous-version").unwrap();
    }
    std::os::unix::fs::symlink("/usr/bin/python3.12", previous.join("venv/bin/python")).unwrap();
    installer
        .metadata_manager
        .update_installed_version(
            "torch-runtime-old",
            InstalledVersionMetadata {
                path: "torch-runtime-old".into(),
                release_tag: "torch-runtime-old".into(),
                ..Default::default()
            },
            Some(AppId::Torch),
        )
        .unwrap();
    let (release, server) = fixture_release(bundle(requirements, validation), valid_checksum).await;
    let result = install_fixture(&installer, &release).await;
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains(expected),
        "expected {expected:?}, got {error}"
    );
    finish_fixture(server).await;
    assert!(!root
        .path()
        .join("torch-versions/torch-runtime-0.1.0")
        .exists());
    assert_eq!(
        std::fs::read_to_string(previous.join("serve.py")).unwrap(),
        "previous-version"
    );
    assert!(
        tokio::process::Command::new(previous.join("venv/bin/python"))
            .arg("--version")
            .output()
            .await
            .unwrap()
            .status
            .success()
    );
    let mut restarted = crate::version_manager::VersionState::new(
        root.path(),
        AppId::Torch,
        installer.metadata_manager.clone(),
    )
    .await
    .unwrap();
    restarted.validate_installations().await.unwrap();
    assert_eq!(restarted.get_installed_tags(), vec!["torch-runtime-old"]);
    assert_eq!(
        std::fs::read_dir(root.path().join("torch-versions"))
            .unwrap()
            .count(),
        1
    );
}

async fn install_fixture(installer: &VersionInstaller, release: &GitHubRelease) -> Result<()> {
    let (tx, mut rx) = mpsc::channel(32);
    let mut updates = Vec::new();
    let progress = async {
        while let Some(update) = rx.recv().await {
            updates.push(format!("{update:?}"));
        }
    };
    let install = async {
        tokio::join!(
            installer.install_version(&release.tag_name, release, tx),
            progress
        )
        .0
    };
    match tokio::time::timeout(std::time::Duration::from_secs(60), install).await {
        Ok(result) => result,
        Err(_) => {
            let logs: Vec<_> = std::fs::read_dir(installer.logs_dir())
                .unwrap()
                .map(|entry| {
                    let path = entry.unwrap().path();
                    (
                        path.clone(),
                        std::fs::read_to_string(path).unwrap_or_default(),
                    )
                })
                .collect();
            panic!("Local Torch fixture exceeded 60s; progress: {updates:?}; logs: {logs:?}");
        }
    }
}

async fn finish_fixture(server: tokio::task::JoinHandle<()>) {
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .expect("installer did not request both the fixture checksum and archive")
        .unwrap();
}

#[tokio::test]
async fn checksum_failure_preserves_previous_runtime_on_restart() {
    rejected_bundle_preserves_previous("", "", false, "checksum mismatch").await;
}

#[tokio::test]
async fn dependency_failure_preserves_previous_runtime_on_restart() {
    rejected_bundle_preserves_previous("--no-index\npumas-nonexistent-fixture==0 --hash=sha256:0000000000000000000000000000000000000000000000000000000000000000\n", "", true, "Installing locked runtime dependencies failed").await;
}

#[tokio::test]
async fn validation_failure_preserves_previous_runtime_on_restart() {
    rejected_bundle_preserves_previous(
        "--no-index\n",
        "raise RuntimeError('fixture validation failure')",
        true,
        "Validating GPU and sidecar protocol failed",
    )
    .await;
}

#[tokio::test]
async fn cancellation_reaps_installer_and_its_child() {
    let (installer, root) = create_test_installer();
    let pid_file = root.path().join("child.pid");
    let mut command = tokio::process::Command::new("python3.12");
    command.args(["-c", "import subprocess,sys,time; from pathlib import Path; child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); Path(sys.argv[1]).write_text(str(child.pid)); time.sleep(60)"])
        .arg(&pid_file);
    let (tx, _rx) = mpsc::channel(32);
    let log_path = root.path().join("install.log");
    let run = installer.run_runtime_command(command, &log_path, "fixture", &tx);
    let cancel = async {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !pid_file.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        installer.cancel_flag.store(true, Ordering::SeqCst);
    };
    let (result, ()) = tokio::join!(run, cancel);
    assert!(result.is_err());
    let pid = std::fs::read_to_string(pid_file).unwrap();
    // A terminated orphan can briefly remain a zombie until init reaps it.
    if let Ok(status) = std::fs::read_to_string(format!("/proc/{}/stat", pid.trim())) {
        assert!(
            status.split_whitespace().nth(2) == Some("Z"),
            "child still computing: {status}"
        );
    }
}

#[tokio::test]
async fn validated_bundle_publishes_relocatable_python_and_shared_metadata() {
    let (installer, root) = create_test_installer();
    let (release, server) = fixture_release(bundle("--no-index\n", ""), true).await;
    install_fixture(&installer, &release).await.unwrap();
    finish_fixture(server).await;
    let runtime = root.path().join("torch-versions/torch-runtime-0.1.0");
    let python = tokio::process::Command::new(runtime.join("venv/bin/python"))
        .args(["-c", "import sys; print(sys.prefix)"])
        .output()
        .await
        .unwrap();
    assert!(python.status.success());
    assert_eq!(
        String::from_utf8(python.stdout).unwrap().trim(),
        runtime.join("venv").to_str().unwrap()
    );
    let mut restarted = crate::version_manager::VersionState::new(
        root.path(),
        AppId::Torch,
        installer.metadata_manager.clone(),
    )
    .await
    .unwrap();
    let validation = restarted.validate_installations().await.unwrap();
    assert_eq!(validation.valid_count, 1);
    assert_eq!(restarted.get_installed_tags(), vec![release.tag_name]);
    assert_eq!(
        std::fs::read_dir(root.path().join("torch-versions"))
            .unwrap()
            .count(),
        1
    );
}
