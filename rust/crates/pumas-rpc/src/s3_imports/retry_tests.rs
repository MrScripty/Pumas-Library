//! Disposable local HTTPS controls for the real RPC/worker/acquisition owners.
use super::tests::{gguf, params, rpc, terminal};
use super::*;
use crate::server::{start_server, LoopbackHost};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
const ID: &str = "c3f7d104-1234-4321-abcd-aaaaaaaaaaaa";
struct RetrySource {
    endpoint: String,
    mode: Arc<AtomicU8>,
    started: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl RetrySource {
    fn start(mode: u8) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "https://localhost:{}",
            listener.local_addr().unwrap().port()
        );
        let mode = Arc::new(AtomicU8::new(mode));
        let started = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let (m, count, captured, done) = (
            mode.clone(),
            started.clone(),
            requests.clone(),
            stop.clone(),
        );
        let thread = std::thread::spawn(move || {
            let identity = native_tls::Identity::from_pkcs12(
                include_bytes!("../../../pumas-core/tests/fixtures/http-tls/localhost.p12"),
                "fixture",
            )
            .unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            while !done.load(Ordering::SeqCst) {
                let socket = match listener.accept() {
                    Ok((s, _)) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut socket = acceptor.accept(socket).unwrap();
                let mut request = vec![];
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 16 * 1024);
                    let mut b = [0];
                    socket.read_exact(&mut b).unwrap();
                    request.push(b[0]);
                }
                let request = String::from_utf8(request).unwrap();
                let head = request.starts_with("HEAD ");
                let auxiliary = request.contains("versionId=data-v2");
                let version = if auxiliary {
                    "data-v2"
                } else if request.contains("versionId=weights-v1") {
                    "weights-v1"
                } else {
                    "desktop-v1"
                };
                captured.lock().unwrap().push(request);
                let mode = m.load(Ordering::SeqCst);
                if !head && (mode == 1 || (mode == 3 && auxiliary)) {
                    socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    continue;
                }
                let bytes = if auxiliary { b"{}".to_vec() } else { gguf() };
                write!(socket,"HTTP/1.1 {}\r\nContent-Length: {}\r\n{}Last-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {}\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n",if head {"200 OK"} else {"206 Partial Content"},bytes.len(),if head {String::new()} else {format!("Content-Range: bytes 0-{}/{}\r\n",bytes.len()-1,bytes.len())},version).unwrap();
                if !head {
                    socket
                        .write_all(if mode == 2 { &bytes[..1] } else { &bytes })
                        .unwrap();
                    socket.flush().unwrap();
                    if mode == 2 {
                        count.fetch_add(1, Ordering::SeqCst);
                        let mut b = [0];
                        assert_eq!(
                            socket.read(&mut b).unwrap(),
                            0,
                            "cancel must close prior GET before retry"
                        );
                    }
                }
            }
        });
        Self {
            endpoint,
            mode,
            started,
            requests,
            stop,
            thread: Some(thread),
        }
    }
    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
    async fn stalled(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(8), async {
            while self.started.load(Ordering::SeqCst) < count {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
}
impl Drop for RetrySource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
async fn open_server(root: &Path) -> crate::server::ServerHandle {
    let api = pumas_library::PumasApi::builder(root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    start_server(
        api,
        LoopbackHost::parse("127.0.0.1").unwrap(),
        0,
        crate::http_transport::HttpShutdownPolicy::default(),
    )
    .await
    .unwrap()
}
fn inventory(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(p: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for e in std::fs::read_dir(p).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.insert(p.clone(), std::fs::read(p).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, &mut out);
    out
}
async fn ready(server: &crate::server::ServerHandle) {
    assert_eq!(
        rpc(server, "get_s3_transfer_retry", json!({"operation_id":ID})).await["result"]["status"],
        "ready"
    );
}
async fn retry(server: &crate::server::ServerHandle) -> Value {
    rpc(
        server,
        "retry_s3_model_transfer",
        json!({"operation_id":ID,"credentials":null}),
    )
    .await
}

fn retained_admission_fixture(root: &Path) -> RetainedTransfer {
    use pumas_library::acquisition::{
        AcquisitionDemand, ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence,
        ArtifactSourceIdentity, FileVerificationRequirement, RevisionStrength,
    };
    let request: S3ImportParams = serde_json::from_value(params("https://source.invalid")).unwrap();
    let reservation = reserve(root.to_owned(), ID).unwrap();
    let digest = Sha256Evidence::new("caller.sha256", request.sha256.clone()).unwrap();
    let manifest = ArtifactManifest::new(
        ArtifactSourceIdentity::new(
            "s3",
            "admission-fixture",
            ArtifactRevisionEvidence::new(
                "s3.version",
                request.version_id.clone(),
                RevisionStrength::Immutable,
            )
            .unwrap(),
        )
        .unwrap(),
        vec![ArtifactFile::new(
            request.filename.clone(),
            request.key.clone(),
            Some(gguf().len() as u64),
            Some(digest.clone()),
            FileVerificationRequirement::Sha256,
        )
        .unwrap()],
    )
    .unwrap();
    let record = AcquisitionRecord {
        id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".parse().unwrap(),
        demand: AcquisitionDemand {
            consumer: "model.s3.workflow".into(),
            operation: ID.into(),
        },
        manifest,
        workspace: reservation.workspace_identity().clone(),
        phase: AcquisitionPhase::Transferring,
        files: vec![],
    };
    RetainedTransfer {
        entries: vec![S3ManifestEntry {
            source_key: request.key.clone(),
            version: request.version_id.clone(),
            logical_path: request.filename.clone(),
            expected_sha256: digest,
        }],
        request,
        reservation,
        record,
        authenticated: false,
    }
}

#[tokio::test]
async fn workspace_capacity_refusals_preserve_retry_and_fresh_admission() {
    use pumas_library::acquisition::{AcquisitionCapacity, AcquisitionStore};
    for resource in ["scopes", "workers", "blocking"] {
        for is_retry in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let capacity = AcquisitionCapacity {
                scopes: if resource == "scopes" { 1 } else { 3 },
                workers: if resource == "workers" { 1 } else { 2 },
                blocking: 1,
                ..AcquisitionCapacity::default()
            };
            let acquisition = Arc::new(
                AcquisitionService::with_capacity(
                    Arc::new(AcquisitionStore::new(root.path())),
                    capacity,
                )
                .unwrap(),
            );
            let blocker = Arc::new(acquisition.open_consumer("test.capacity").unwrap());
            let blocked_work = if resource == "scopes" {
                None
            } else {
                let held = blocker.clone();
                let (entered_tx, entered) = tokio::sync::oneshot::channel();
                let (release_tx, release) = std::sync::mpsc::channel();
                let waiter = tokio::spawn(async move {
                    held.run_blocking("hold admission capacity", move || {
                        entered_tx.send(()).unwrap();
                        release.recv_timeout(Duration::from_secs(10)).unwrap();
                        Ok(())
                    })
                    .await
                });
                entered.await.unwrap();
                Some((release_tx, waiter))
            };
            let (client, mut queue) = S3Imports::channel();
            let request = || serde_json::from_value(params("https://source.invalid")).unwrap();
            assert!(matches!(
                client.admit(request()).unwrap(),
                S3ImportOutcome::Running { .. }
            ));
            let Job::Import(mut job) = queue.recv().await.unwrap() else {
                panic!("expected import");
            };
            if is_retry {
                client.finish(
                    ID,
                    &job.attempt,
                    failure(PublicError::unavailable(), true, None),
                    Some(retained_admission_fixture(root.path())),
                );
                assert!(matches!(
                    client
                        .retry(S3TransferRetryParams {
                            operation_id: ID.into(),
                            credentials: None
                        })
                        .unwrap(),
                    S3ImportOutcome::Running { .. }
                ));
                let Job::Import(next) = queue.recv().await.unwrap() else {
                    panic!("expected retry");
                };
                job = next;
            }
            let expected = job
                .retry
                .as_ref()
                .map(|held| (held.record.clone(), held.reservation.binding().clone()));
            let before = inventory(root.path());
            let entered = Arc::new(AtomicBool::new(false));
            let observed = entered.clone();
            let grant = job.retry.as_ref().map(|held| held.reservation.clone());
            let path = root.path().to_owned();
            let refused = prepare_workspace(
                &acquisition,
                job.retry.map(|retained| *retained),
                move || {
                    observed.store(true, Ordering::SeqCst);
                    grant.map_or_else(|| reserve(path, ID), Ok)
                },
            )
            .await;
            let (result, retained) = match refused {
                Err(result) => *result,
                Ok(_) => panic!("{resource} saturation must refuse preparation"),
            };
            assert!(!entered.load(Ordering::SeqCst), "{resource}");
            assert_eq!(inventory(root.path()), before, "{resource}");
            assert!(
                matches!(result, S3ImportResultWire::Failed { retained_work, .. } if retained_work == is_retry)
            );
            assert_eq!(retained.is_some(), is_retry, "{resource}");
            if let (Some(held), Some((record, binding))) = (&retained, &expected) {
                assert_eq!(&held.record, record);
                assert_eq!(held.reservation.binding(), binding);
                held.reservation.validate().unwrap();
            }
            client.finish(ID, &job.attempt, result, retained);
            if is_retry {
                assert!(matches!(
                    client.retry_state(Some(ID)).unwrap(),
                    S3TransferRetryState::Ready { .. }
                ));
                assert!(matches!(
                    client
                        .retry(S3TransferRetryParams {
                            operation_id: ID.into(),
                            credentials: None
                        })
                        .unwrap(),
                    S3ImportOutcome::Running { .. }
                ));
            } else {
                let mut next: S3ImportParams = request();
                next.operation_id = "c3f7d104-1234-4321-abcd-bbbbbbbbbbbb".into();
                assert!(matches!(
                    client.admit(next).unwrap(),
                    S3ImportOutcome::Running { .. }
                ));
            }
            let Job::Import(next) = queue.recv().await.unwrap() else {
                panic!("expected readmitted import");
            };
            // A delayed result from the failed attempt cannot retire its retry,
            // even though both attempts deliberately keep the operation ID.
            client.finish(
                ID,
                &job.attempt,
                S3ImportResultWire::Cancelled {
                    retained_work: false,
                },
                None,
            );
            assert!(matches!(
                client.snapshot(None).unwrap(),
                S3ImportOutcome::Running { .. }
            ));
            if let Some((release, waiter)) = blocked_work {
                release.send(()).unwrap();
                waiter.await.unwrap().unwrap();
            }
            blocker.shutdown().await.unwrap();
            drop(blocker);
            // Scope retirement is asynchronous. Keep the same live grant while
            // waiting for the released admission capacity to become available.
            let (consumer, reservation) = tokio::time::timeout(Duration::from_secs(5), async {
                let mut retained = next.retry.map(|retained| *retained);
                loop {
                    let grant = retained.as_ref().map(|held| held.reservation.clone());
                    let path = root.path().to_owned();
                    let operation = next.request.operation_id.clone();
                    match prepare_workspace(&acquisition, retained, move || {
                        grant.map_or_else(|| reserve(path, &operation), Ok)
                    })
                    .await
                    {
                        Ok(value) => break value,
                        Err(refused) => {
                            let (_, held) = *refused;
                            assert_eq!(held.is_some(), is_retry);
                            retained = held;
                            tokio::task::yield_now().await;
                        }
                    }
                }
            })
            .await
            .unwrap();
            if let Some((_, binding)) = expected {
                assert_eq!(reservation.binding(), &binding);
            }
            consumer.shutdown().await.unwrap();
            reservation.clear_contents().unwrap();
            // Successful preparation must release its backup reservation clone.
            reservation.remove_empty().unwrap();
        }
    }
}

#[tokio::test]
async fn workspace_effect_error_cannot_impersonate_no_effect_capacity_refusal() {
    use pumas_library::acquisition::AcquisitionStore;
    let root = tempfile::tempdir().unwrap();
    let acquisition = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        root.path(),
    ))));
    let held = retained_admission_fixture(root.path());
    let entered = Arc::new(AtomicBool::new(false));
    let observed = entered.clone();
    let refused = prepare_workspace(&acquisition, Some(held), move || {
        observed.store(true, Ordering::SeqCst);
        Err(PumasError::AcquisitionCapacityExhausted {
            resource: "blocking",
        })
    })
    .await;
    let (result, retained) = match refused {
        Err(value) => *value,
        Ok(_) => panic!("effect error must refuse preparation"),
    };
    assert!(entered.load(Ordering::SeqCst));
    assert!(matches!(
        result,
        S3ImportResultWire::Failed {
            retained_work: true,
            ..
        }
    ));
    assert!(retained.is_none());
    assert!(root.path().join(format!(".s3-import-{ID}")).is_dir());
}

#[tokio::test]
async fn stale_retry_completion_cannot_restore_previous_custody() {
    let root = tempfile::tempdir().unwrap();
    let (client, mut queue) = S3Imports::channel();
    client
        .admit(serde_json::from_value(params("https://source.invalid")).unwrap())
        .unwrap();
    let Job::Import(original) = queue.recv().await.unwrap() else {
        panic!("expected original import");
    };
    let stale_grant = retained_admission_fixture(root.path());
    client.finish(
        ID,
        &original.attempt,
        failure(PublicError::unavailable(), true, None),
        Some(stale_grant.clone()),
    );
    client
        .retry(S3TransferRetryParams {
            operation_id: ID.into(),
            credentials: None,
        })
        .unwrap();
    let Job::Import(newer) = queue.recv().await.unwrap() else {
        panic!("expected newer attempt");
    };
    assert!(!Arc::ptr_eq(&original.attempt, &newer.attempt));
    client.finish(
        ID,
        &original.attempt,
        failure(PublicError::unavailable(), true, None),
        Some(stale_grant.clone()),
    );
    assert!(matches!(
        client.snapshot(Some(ID)).unwrap(),
        S3ImportOutcome::Running { .. }
    ));
    assert!(matches!(
        client.retry_state(Some(ID)).unwrap(),
        S3TransferRetryState::Unavailable {
            reason: S3TransferRetryReason::Busy
        }
    ));
    assert!(client
        .0
        .lock()
        .unwrap()
        .current
        .as_ref()
        .unwrap()
        .retained
        .is_none());
    client.finish(
        ID,
        &newer.attempt,
        S3ImportResultWire::Completed {
            model_id: "fixture/newer".into(),
        },
        None,
    );
    client.finish(
        ID,
        &original.attempt,
        failure(PublicError::unavailable(), true, None),
        Some(stale_grant),
    );
    assert!(matches!(
        client.snapshot(Some(ID)).unwrap(),
        S3ImportOutcome::Finished {
            result: S3ImportResultWire::Completed { model_id }, ..
        } if model_id == "fixture/newer"
    ));
    assert!(matches!(
        client.retry_state(Some(ID)).unwrap(),
        S3TransferRetryState::Unavailable {
            reason: S3TransferRetryReason::NotRetryable
        }
    ));
}

#[tokio::test]
async fn explicit_retry_https_fail_cancel_reuse_and_exact_refusals() {
    const MARKER: &str = "PUMAS_S3_RETRY_FIXTURE_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let config = tempfile::tempdir().unwrap();
        let result=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","s3_imports::retry_tests::explicit_retry_https_fail_cancel_reuse_and_exact_refusals","--nocapture"])
            .env(MARKER,"1").env("XDG_CONFIG_HOME",config.path()).env("SSL_CERT_FILE",Path::new(env!("CARGO_MANIFEST_DIR")).join("../pumas-core/tests/fixtures/http-tls/localhost.pem")).status().unwrap();
        assert!(result.success());
        return;
    }
    // Repeated explicit failures and final retry retain one operation/acquisition.
    for cancelled in [false, true] {
        let source = RetrySource::start(if cancelled { 2 } else { 1 });
        let root = tempfile::tempdir().unwrap();
        let server = open_server(root.path()).await;
        assert_eq!(
            rpc(&server, "start_s3_model_import", params(&source.endpoint)).await["result"]
                ["status"],
            "running"
        );
        if cancelled {
            source.stalled(1).await;
            assert_eq!(
                rpc(
                    &server,
                    "cancel_s3_model_import",
                    json!({"operation_id":ID})
                )
                .await["result"]["accepted"],
                true
            );
        }
        let result = terminal(&server).await;
        assert_eq!(
            result["result"]["status"],
            if cancelled { "cancelled" } else { "failed" }
        );
        ready(&server).await;
        let path = root.path().join("launcher-data/downloads.json");
        let original: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let acq = original["acquisitions"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        // Closed wire forbids replacing pins/source/intent. Refusals preserve all bytes.
        for field in [
            "endpoint",
            "version_id",
            "sha256",
            "key",
            "files",
            "family",
            "stage",
        ] {
            let before = inventory(root.path());
            let denied = rpc(
                &server,
                "retry_s3_model_transfer",
                json!({"operation_id":ID,"credentials":null,field:"changed"}),
            )
            .await;
            assert_eq!(denied["error"]["code"], -32602);
            assert_eq!(inventory(root.path()), before);
        }
        // Retry while prior cancellation is still being drained never becomes eligible.
        source.mode.store(2, Ordering::SeqCst);
        let (one, two) = tokio::join!(retry(&server), retry(&server));
        let states = [
            one["result"]["status"].as_str().unwrap(),
            two["result"]["status"].as_str().unwrap(),
        ];
        assert_eq!(states.iter().filter(|s| **s == "running").count(), 1);
        assert_eq!(states.iter().filter(|s| **s == "rejected").count(), 1);
        let mut competitor = params(&source.endpoint);
        competitor["operation_id"] = json!("c3f7d104-1234-4321-abcd-bbbbbbbbbbbb");
        assert_eq!(
            rpc(&server, "start_s3_model_import", competitor).await["result"]["status"],
            "rejected"
        );
        source.stalled(if cancelled { 2 } else { 1 }).await;
        assert_eq!(
            rpc(&server, "start_s3_model_import", params(&source.endpoint)).await["result"]
                ["status"],
            "rejected"
        );
        assert_eq!(
            rpc(&server, "get_s3_transfer_retry", json!({"operation_id":ID})).await["result"]
                ["reason"],
            "busy"
        );
        assert_eq!(
            rpc(
                &server,
                "cancel_s3_model_import",
                json!({"operation_id":ID})
            )
            .await["result"]["accepted"],
            true
        );
        terminal(&server).await;
        ready(&server).await;
        source.mode.store(1, Ordering::SeqCst);
        assert_eq!(retry(&server).await["result"]["status"], "running");
        assert_eq!(terminal(&server).await["result"]["status"], "failed");
        ready(&server).await;
        source.mode.store(0, Ordering::SeqCst);
        assert_eq!(retry(&server).await["result"]["status"], "running");
        assert_eq!(terminal(&server).await["result"]["status"], "completed");
        let final_doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(final_doc["acquisitions"].as_object().unwrap().len(), 1);
        assert_eq!(final_doc["acquisitions"][&acq]["demand"]["operation"], ID);
        assert!(final_doc["acquisitions"][&acq]["phase"]["state"] == "adopted");
        assert_eq!(retry(&server).await["result"]["status"], "rejected");
        let count = source.count();
        server.shutdown().await.unwrap();
        let cold = open_server(root.path()).await;
        assert_eq!(
            rpc(&cold, "get_s3_transfer_retry", json!({"operation_id":ID})).await["result"]
                ["reason"],
            "no_live_custody"
        );
        assert_eq!(retry(&cold).await["result"]["status"], "rejected");
        assert_eq!(source.count(), count);
        cold.shutdown().await.unwrap();
    }
    // A bundle's first verified member is reused, never fetched a second time.
    {
        let source = RetrySource::start(3);
        let root = tempfile::tempdir().unwrap();
        let server = open_server(root.path()).await;
        let input = json!({"operation_id":ID,"endpoint":source.endpoint,"region":"fixture-region","bucket":"fixture-bucket","addressing":"path","family":"fixture","official_name":"Retry bundle","primary_logical_path":"weights.gguf","files":[{"key":"models/weights.gguf","version_id":"weights-v1","logical_path":"weights.gguf","sha256":"a4e5e156ddec27e286f75328784d7106b60a4eb1d246e950a001a3f944fbda99"},{"key":"models/data","version_id":"data-v2","logical_path":"z-data.json","sha256":"44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"}]});
        rpc(&server, "start_s3_model_bundle_import", input).await;
        assert_eq!(terminal(&server).await["result"]["status"], "failed");
        ready(&server).await;
        source.mode.store(0, Ordering::SeqCst);
        retry(&server).await;
        assert_eq!(terminal(&server).await["result"]["status"], "completed");
        assert_eq!(
            source
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.starts_with("GET ") && r.contains("versionId=weights-v1"))
                .count(),
            1
        );
        server.shutdown().await.unwrap();
    }
    // Authenticated retries require new explicit access material; no secret is retained.
    {
        let source = RetrySource::start(1);
        let root = tempfile::tempdir().unwrap();
        let server = open_server(root.path()).await;
        let old = json!({"access_key_id":"retry-synthetic-old-key","secret_access_key":"retry-synthetic-old-secret","session_token":null});
        rpc(
            &server,
            "start_authenticated_s3_model_import",
            json!({"source":params(&source.endpoint),"credentials":old}),
        )
        .await;
        terminal(&server).await;
        ready(&server).await;
        assert_eq!(
            rpc(&server, "get_s3_transfer_retry", json!({"operation_id":ID})).await["result"]
                ["authentication_required"],
            true
        );
        let count = source.count();
        assert_eq!(retry(&server).await["result"]["status"], "rejected");
        assert_eq!(source.count(), count);
        source.mode.store(0, Ordering::SeqCst);
        let fresh = json!({"access_key_id":"retry-synthetic-fresh-key","secret_access_key":"retry-synthetic-fresh-secret","session_token":null});
        rpc(
            &server,
            "retry_s3_model_transfer",
            json!({"operation_id":ID,"credentials":fresh}),
        )
        .await;
        assert_eq!(terminal(&server).await["result"]["status"], "completed");
        let captured = source.requests.lock().unwrap().clone();
        assert!(captured[0].contains("retry-synthetic-old-key"));
        assert!(captured
            .last()
            .unwrap()
            .contains("retry-synthetic-fresh-key"));
        drop(captured);
        for bytes in inventory(root.path()).values() {
            for secret in [
                "retry-synthetic-old-key",
                "retry-synthetic-old-secret",
                "retry-synthetic-fresh-key",
                "retry-synthetic-fresh-secret",
            ] {
                assert!(!bytes.windows(secret.len()).any(|b| b == secret.as_bytes()));
            }
        }
        server.shutdown().await.unwrap();
    }
    // Genuine consumer receipt on refused duplicate publication cannot authorize replay.
    {
        let source = RetrySource::start(0);
        let root = tempfile::tempdir().unwrap();
        let server = open_server(root.path()).await;
        rpc(&server, "start_s3_model_import", params(&source.endpoint)).await;
        assert_eq!(terminal(&server).await["result"]["status"], "completed");
        let second = "c3f7d104-1234-4321-abcd-bbbbbbbbbbbb";
        let mut input = params(&source.endpoint);
        input["operation_id"] = json!(second);
        rpc(&server, "start_s3_model_import", input).await;
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let result = rpc(
                    &server,
                    "get_s3_model_import",
                    json!({"operation_id":second}),
                )
                .await;
                if result["result"]["status"] == "finished" {
                    assert_eq!(result["result"]["result"]["status"], "failed");
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let doc: Value = serde_json::from_slice(
            &std::fs::read(root.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        let (id, record) = doc["acquisitions"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, r)| r["demand"]["operation"] == second)
            .unwrap();
        assert_eq!(record["phase"]["state"], "using");
        assert!(doc["consumer_receipts"].get(id).is_some());
        assert_eq!(
            rpc(
                &server,
                "get_s3_transfer_retry",
                json!({"operation_id":second})
            )
            .await["result"]["reason"],
            "not_retryable"
        );
        let receipt_path = inventory(root.path())
            .keys()
            .find(|p| {
                p.file_name()
                    .is_some_and(|s| s == ".pumas_import_publication.json")
            })
            .unwrap()
            .clone();
        let mut receipt: Value =
            serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
        receipt["state"] = json!("pending");
        std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let before = inventory(root.path());
        let count = source.count();
        assert_eq!(
            rpc(
                &server,
                "retry_s3_model_transfer",
                json!({"operation_id":second,"credentials":null})
            )
            .await["result"]["status"],
            "rejected"
        );
        assert_eq!(source.count(), count);
        assert_eq!(inventory(root.path()), before);
        let _ = server.shutdown().await;
    }
    // Changed/missing held stage, saved identity/pins/phase/receipt refuse before I/O.
    for fault in [
        "stage",
        "missing-stage",
        "manifest",
        "digest",
        "version",
        "workspace",
        "adopted",
        "withdrawn",
        "using",
        "receipt",
        "missing-record",
    ] {
        let source = RetrySource::start(1);
        let root = tempfile::tempdir().unwrap();
        let server = open_server(root.path()).await;
        rpc(&server, "start_s3_model_import", params(&source.endpoint)).await;
        terminal(&server).await;
        ready(&server).await;
        let stage = root.path().join(format!("launcher-data/.s3-import-{ID}"));
        let doc = root.path().join("launcher-data/downloads.json");
        let mut saved: Value = serde_json::from_slice(&std::fs::read(&doc).unwrap()).unwrap();
        let acq = saved["acquisitions"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        match fault {
            "stage" | "missing-stage" => {
                std::fs::rename(&stage, root.path().join("saved-stage")).unwrap();
                if fault == "stage" {
                    std::fs::create_dir(&stage).unwrap();
                    std::fs::write(stage.join("foreign"), b"preserve").unwrap();
                }
            }
            "manifest" => {
                saved["acquisitions"][&acq]["manifest"]["source"]["namespace"] =
                    json!("altered-source")
            }
            "digest" => {
                assert!(
                    saved["acquisitions"][&acq]["manifest"]["files"][0]["expected_sha256"]
                        .is_object()
                );
                saved["acquisitions"][&acq]["manifest"]["files"][0]["expected_sha256"]["value"] =
                    json!("b".repeat(64));
            }
            "version" => {
                saved["acquisitions"][&acq]["manifest"]["source"]["revision"]["value"] =
                    json!("changed-pin");
            }
            "adopted" => {
                saved["acquisitions"][&acq]["phase"] =
                    json!({"state":"adopted","lease":"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"});
            }
            "withdrawn" => {
                saved["acquisitions"][&acq]["phase"] = json!({"state":"withdrawn"});
            }
            "workspace" => {
                saved["acquisitions"][&acq]["workspace"]["relative_target"] = json!("foreign-stage")
            }
            "using" => {
                saved["acquisitions"][&acq]["phase"] =
                    json!({"state":"using","lease":"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"})
            }
            "receipt" => saved["consumer_receipts"][&acq] = json!({"receipt_version":99}),
            "missing-record" => {
                saved["acquisitions"].as_object_mut().unwrap().remove(&acq);
            }
            _ => unreachable!(),
        }
        if !matches!(fault, "stage" | "missing-stage") {
            std::fs::write(&doc, serde_json::to_vec(&saved).unwrap()).unwrap();
        }
        let before = inventory(root.path());
        let count = source.count();
        retry(&server).await;
        assert_eq!(
            terminal(&server).await["result"]["status"],
            "failed",
            "{fault}"
        );
        assert_eq!(source.count(), count, "{fault}");
        assert_eq!(inventory(root.path()), before, "{fault}");
        server.shutdown().await.unwrap();
    }
}

/// Explicit browser coordinator only; never runs in ordinary aggregates.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires explicit local Chromium retry coordinator"]
async fn browser_retry_real_rpc_host() {
    let out = PathBuf::from(std::env::var("PUMAS_S3_RETRY_BROWSER_OUTPUT").unwrap());
    let root = out.join("runtime/library");
    let mut server = open_server(&root).await;
    let port = server.addr().port();
    std::fs::write(
        out.join("rpc-ready.json"),
        json!({"origin":format!("http://{}",server.addr())}).to_string(),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(180), async {
        while !out.join("cold-request").exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    server.shutdown().await.unwrap();
    let record_path = root.join("launcher-data/downloads.json");
    let before = std::fs::read(&record_path).unwrap();
    let api = pumas_library::PumasApi::builder(&root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    server = start_server(
        api,
        LoopbackHost::parse("127.0.0.1").unwrap(),
        port,
        crate::http_transport::HttpShutdownPolicy::default(),
    )
    .await
    .unwrap();
    std::fs::write(
        out.join("rpc-cold-ready.json"),
        json!({"origin":format!("http://{}",server.addr())}).to_string(),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(120), async {
        while !out.join("browser-finished.json").exists() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let result: Value =
        serde_json::from_slice(&std::fs::read(out.join("browser-finished.json")).unwrap()).unwrap();
    assert_eq!(result["pass"], true);
    assert_eq!(std::fs::read(&record_path).unwrap(), before);
    std::fs::write(out.join("native-browser-outcome.json"),json!({"pass":true,"cold_acquisition_bytes_unchanged":true,"records":serde_json::from_slice::<Value>(&before).unwrap()["acquisitions"]}).to_string()).unwrap();
    server.shutdown().await.unwrap();
}
