//! Real owner-composition fixtures with synthetic producer payloads and loopback
//! services. The composed API retains its existing startup connectivity probes
//! and HF credential resolution; this is not a globally offline-startup test.
use super::*;
use pumas_library::model_library::ModelImporter;
use pumas_library::models::ModelImportSpec;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn acquisition_integration_shutdown_retains_copy_and_hf_after_caller_loss() {
    exercise_shutdown(false).await;
}

#[cfg(all(feature = "inference-plugins", feature = "test-support"))]
#[tokio::test]
async fn acquisition_integration_shutdown_drains_real_native_consumer_before_service() {
    exercise_shutdown(true).await;
}

async fn exercise_shutdown(include_native: bool) {
    for copied in [false, true] {
        let temp = TempDir::new().unwrap();
        let api = crate::handlers::test_support::build_test_api_with_hf(temp.path()).await;
        let library = api.model_library().clone();
        let acquisition = api.acquisition().clone();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let effect = temp.path().join("settled-hf-effect");
        let caller = if copied {
            let source = temp.path().join("copy-source");
            std::fs::create_dir(&source).unwrap();
            std::fs::write(source.join("model.onnx"), b"synthetic owned ONNX").unwrap();
            let entered = StdMutex::new(Some(entered));
            let blocked = StdMutex::new(blocked);
            library.set_metadata_write_notifier(Some(Arc::new(move |_| {
                if let Some(entered) = entered.lock().unwrap().take() {
                    let _ = entered.send(());
                    // Closing the fixture sender also releases the finite worker
                    // if an assertion unwinds before the normal release point.
                    let _ = blocked.lock().unwrap().recv();
                }
            })));
            let importer = ModelImporter::new(library.clone());
            tokio::spawn(async move {
                let result = importer
                    .import(&ModelImportSpec {
                        path: source.display().to_string(),
                        family: "fixture".into(),
                        official_name: "Owned Shutdown".into(),
                        repo_id: None,
                        model_type: Some("vision".into()),
                        subtype: None,
                        tags: None,
                        security_acknowledged: Some(true),
                    })
                    .await?;
                if result.success {
                    Ok(())
                } else {
                    Err(pumas_library::PumasError::Other(format!(
                        "copy failed: {:?}",
                        result.error
                    )))
                }
            })
        } else {
            let effect = effect.clone();
            tokio::spawn(
                pumas_library::model_library::test_support::run_download_blocking_fixture(
                    &api,
                    move || {
                        let _ = entered.send(());
                        blocked
                            .recv()
                            .map_err(|error| pumas_library::PumasError::Other(error.to_string()))?;
                        std::fs::write(effect, b"settled")?;
                        Ok(())
                    },
                ),
            )
        };
        tokio::time::timeout(Duration::from_secs(10), ready)
            .await
            .unwrap()
            .unwrap();
        #[cfg(all(feature = "inference-plugins", feature = "test-support"))]
        let mut native = if include_native {
            Some(native_transfer(&api, temp.path()).await)
        } else {
            None
        };
        #[cfg(not(all(feature = "inference-plugins", feature = "test-support")))]
        assert!(!include_native);
        #[cfg(feature = "inference-plugins")]
        let managers = {
            #[allow(unused_mut)]
            let mut managers = HashMap::new();
            #[cfg(feature = "test-support")]
            if let Some((manager, _source, _release)) = &native {
                managers.insert("llama-cpp".into(), manager.clone());
            }
            managers
        };
        let server = Arc::new(
            start_server(
                api,
                #[cfg(feature = "inference-plugins")]
                managers,
                #[cfg(feature = "inference-plugins")]
                SizeCalculator::new_with_cache(temp.path().join("launcher-data/cache")).await,
                #[cfg(feature = "inference-plugins")]
                PluginLoader::new_async(temp.path().join("launcher-data/plugins"))
                    .await
                    .unwrap(),
                LoopbackHost::parse("127.0.0.1").unwrap(),
                0,
                crate::http_transport::HttpShutdownPolicy::default(),
            )
            .await
            .unwrap(),
        );
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut events = http
            .get(format!(
                "http://{}/events/model-library-updates",
                server.addr()
            ))
            .send()
            .await
            .unwrap();
        assert!(events.status().is_success());
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        // A real accepted HTTP shutdown response must finish before teardown,
        // even while the independently owned domain effect remains blocked.
        let acknowledgement: serde_json::Value = http
            .post(format!("http://{}/rpc", server.addr()))
            .json(&serde_json::json!({"jsonrpc":"2.0", "id":1, "method":"shutdown", "params":{}}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(acknowledgement["result"]["status"], "shutting_down");
        let waiter = tokio::spawn({
            let server = server.clone();
            async move { server.wait().await }
        });
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        let receipt = server.wait();
        tokio::pin!(receipt);
        let pending = futures::poll!(&mut receipt).is_pending();
        // The shared acquisition owner must still admit a fresh scope until its
        // consumers finish. Merely observing HTTP closure is not global closure.
        let probe = acquisition.open_consumer("fixture.shutdown-order");
        release.send(()).unwrap();
        assert!(
            pending,
            "shutdown returned before the retained producer settled"
        );
        let probe = probe.expect("shared service closed before its consumers drained");
        probe.shutdown().await.unwrap();
        #[cfg(all(feature = "inference-plugins", feature = "test-support"))]
        if let Some((manager, _source, native_release)) = &mut native {
            tokio::time::timeout(Duration::from_secs(10), async {
                while !server
                    .downloads_drained
                    .as_ref()
                    .unwrap()
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("copy/HF owner must settle while native consumer remains held");
            // Global service shutdown alone does not close manager admission.
            // Do not request an install here: shutdown retains its install lock
            // while the real native consumer is still held by our barrier.
            tokio::time::timeout(Duration::from_secs(3), async {
                while !manager.acquisition_fixture_admission_closed() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("server must close native admission before shared shutdown");
            assert!(
                futures::poll!(&mut receipt).is_pending(),
                "server must retain the native consumer after the library owner settles"
            );
            let probe = acquisition
                .open_consumer("fixture.native-still-draining")
                .expect("global closure must wait for native consumer settlement");
            probe.shutdown().await.unwrap();
            native_release.take().unwrap().send(()).unwrap();
        }
        let outcome = tokio::time::timeout(Duration::from_secs(15), &mut receipt)
            .await
            .unwrap();
        if include_native {
            let error = outcome.unwrap_err().to_string();
            assert!(
                error.to_ascii_lowercase().contains("installation"),
                "{error}"
            );
            assert_eq!(server.shutdown().await.unwrap_err().to_string(), error);
        } else {
            outcome.unwrap();
            server.shutdown().await.unwrap();
        }
        assert!(acquisition.open_consumer("fixture.after-shutdown").is_err());
        tokio::time::timeout(Duration::from_secs(3), async {
            while events.chunk().await.unwrap().is_some() {}
        })
        .await
        .expect("accepted SSE must finish");
        if copied {
            let rows = library.index().list_all().unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].metadata["import_state"], "ready");
            assert_eq!(rows[0].metadata["import_publication"]["confirmed"], true);
            let receipt: serde_json::Value = serde_json::from_slice(
                &std::fs::read(
                    library
                        .library_root()
                        .join(&rows[0].id)
                        .join(".pumas_import_publication.json"),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(receipt["state"], "confirmed");
            assert_eq!(receipt["id"], rows[0].metadata["import_publication"]["id"]);
        } else {
            assert_eq!(std::fs::read(effect).unwrap(), b"settled");
        }
        library.set_metadata_write_notifier(None);
        #[cfg(all(feature = "inference-plugins", feature = "test-support"))]
        if let Some((manager, mut source, native_release)) = native {
            assert!(native_release.is_none());
            assert!(!manager.version_path("b1234+cpu").exists());
            // The public refusal is observed after the server's drain, before
            // the fixture itself ever invokes the manager's shutdown method.
            let refusal =
                tokio::time::timeout(Duration::from_secs(3), manager.install_version("b1234+cpu"))
                    .await
                    .unwrap();
            assert!(matches!(refusal,
                Err(pumas_library::PumasError::InstallationFailed { ref message })
                    if message == "Version manager is shutting down"));
            assert!(manager.shutdown_installations().await.is_err());
            let task = source.0.take().unwrap();
            task.abort();
            if let Err(error) = task.await {
                assert!(error.is_cancelled(), "{error}");
            }
        }
    }
}

#[cfg(all(feature = "inference-plugins", feature = "test-support"))]
struct SourceTask(Option<tokio::task::JoinHandle<()>>);
#[cfg(all(feature = "inference-plugins", feature = "test-support"))]
impl Drop for SourceTask {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}

#[cfg(all(feature = "inference-plugins", feature = "test-support"))]
async fn native_transfer(
    api: &PumasApi,
    root: &std::path::Path,
) -> (
    VersionManager,
    SourceTask,
    Option<std::sync::mpsc::Sender<()>>,
) {
    use pumas_app_manager::version_manager::ProgressUpdate;
    use pumas_library::network::{GitHubAsset, GitHubRelease, ReleasesCache};
    use pumas_library::AppId;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let url = format!("{base}/archive");
    let os = match std::env::consts::OS {
        "linux" => "ubuntu",
        "windows" => "win",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    let name = format!("llama-b1234-bin-{os}-{arch}.tar.gz");
    ReleasesCache::new(root.join("launcher-data/cache"), Duration::from_secs(3600))
        .set_disk(
            AppId::LlamaCpp.github_repo(),
            &[GitHubRelease {
                tag_name: "b1234".into(),
                name: "Synthetic fixture".into(),
                published_at: "2026-10-03T00:00:00Z".into(),
                body: None,
                tarball_url: None,
                zipball_url: None,
                prerelease: false,
                html_url: format!("{base}/release"),
                total_size: Some(3),
                archive_size: Some(3),
                dependencies_size: None,
                assets: vec![GitHubAsset {
                    name: name.clone(),
                    size: 3,
                    download_url: url.clone(),
                    content_type: None,
                }],
            }],
        )
        .unwrap();
    let body = serde_json::to_vec(&serde_json::json!({"tag_name":"b1234", "assets":[{
        "id":1234, "name":name, "size":3, "browser_download_url":url,
        "digest":"sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    }]}))
    .unwrap();
    let source = SourceTask(Some(tokio::spawn(async move {
        for metadata in [true, false] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut byte = [0];
                stream.read_exact(&mut byte).await.unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
            assert!(
                !request.contains("authorization:"),
                "fixture must not receive ambient credentials"
            );
            if metadata {
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                stream.write_all(&body).await.unwrap();
            } else {
                // A real partial-file write occurs; there is no executable archive.
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\na",
                    )
                    .await
                    .unwrap();
                let mut closed = [0];
                let _ = stream.read(&mut closed).await;
            }
        }
    })));
    let manager = VersionManager::new_with_loopback_acquisition_fixture(
        root,
        api.acquisition().clone(),
        base,
    )
    .await
    .unwrap();
    let mut progress = manager.install_version("b1234+cpu").await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = progress.recv().await {
            if matches!(update, ProgressUpdate::Download { downloaded_bytes, .. } if downloaded_bytes > 0) { return; }
            if let ProgressUpdate::Error { message } = update { panic!("native fixture admission failed: {message}"); }
        }
        panic!("native fixture ended without an owned partial write");
    }).await.unwrap();
    drop(progress); // losing the consumer does not own or stop installation
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, blocked) = std::sync::mpsc::channel();
    let waiter = tokio::spawn({
        let manager = manager.clone();
        async move {
            manager
                .run_acquisition_fixture_effect(move || {
                    let _ = entered.send(());
                    blocked
                        .recv()
                        .map_err(|error| pumas_library::PumasError::Other(error.to_string()))?;
                    Ok(())
                })
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(10), ready)
        .await
        .unwrap()
        .unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    (manager, source, Some(release))
}

#[cfg(all(feature = "inference-plugins", feature = "test-support"))]
#[tokio::test]
async fn acquisition_integration_native_fixture_refuses_ambient_sources_before_effects() {
    for source in [
        "https://example.com/api",
        "http://localhost/api",
        "http://127.0.0.1?credential=x",
        "http://user:synthetic@127.0.0.1/api",
        "http://127.0.0.1/#fragment",
    ] {
        let root = TempDir::new().unwrap();
        let acquisition = Arc::new(pumas_library::acquisition::AcquisitionService::new(
            Arc::new(pumas_library::acquisition::AcquisitionStore::new(
                root.path(),
            )),
        ));
        assert!(VersionManager::new_with_loopback_acquisition_fixture(
            root.path(),
            acquisition.clone(),
            source.into()
        )
        .await
        .is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        acquisition.shutdown().await.unwrap();
    }
}
