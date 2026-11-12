//! Torch wheels over a loopback origin: interrupted transfers resume from
//! retained bytes, verification is by SHA-256, and retention follows use.

use super::*;
use pumas_library::acquisition::{AcquisitionService, AcquisitionStore};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt as _};

const WHEEL_NAME: &str = "torch-2.10.0+cpu-cp312-cp312-linux_x86_64.whl";

fn wheel_bytes() -> Vec<u8> {
    (0..64 * 1024).map(|index| (index % 251) as u8).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// How the origin answers one request, in order.
#[derive(Clone, Copy, Debug)]
enum Reply {
    /// Announce the whole body, send this many bytes, then drop the connection.
    Truncated(usize),
    /// Honor the request's `Range: bytes=N-` with a 206 for the remainder.
    Ranged,
    /// Send the whole body.
    Full,
}

#[derive(Debug, Clone)]
struct Served {
    range_start: Option<usize>,
    body_bytes: usize,
}

struct WheelOrigin {
    url: String,
    served: Arc<StdMutex<Vec<Served>>>,
    server: JoinHandle<()>,
}

impl Drop for WheelOrigin {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl WheelOrigin {
    fn served(&self) -> Vec<Served> {
        self.served.lock().unwrap().clone()
    }
}

async fn serve_wheel(body: Vec<u8>, script: Vec<Reply>) -> WheelOrigin {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    // The CDN-style path escapes the local-version `+`, as the PyTorch index does.
    let url = format!(
        "http://{}/whl/cpu/torch-2.10.0%2Bcpu-cp312-cp312-linux_x86_64.whl",
        listener.local_addr().unwrap()
    );
    let served = Arc::new(StdMutex::new(Vec::new()));
    let log = served.clone();
    let server = tokio::spawn(async move {
        let mut script = script.into_iter();
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut head = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                match stream.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => head.extend_from_slice(&chunk[..read]),
                }
            }
            let head = String::from_utf8_lossy(&head).to_ascii_lowercase();
            let range_start = head
                .lines()
                .find_map(|line| line.strip_prefix("range: bytes="))
                .and_then(|range| range.trim().strip_suffix('-'))
                .and_then(|start| start.parse::<usize>().ok());
            let reply = script.next().unwrap_or(Reply::Full);
            let (status, payload) = match (reply, range_start) {
                (Reply::Ranged, Some(start)) if start < body.len() => (
                    format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{}\r\nContent-Length: {}\r\nETag: \"wheel-v1\"\r\nConnection: close\r\n\r\n",
                        body.len() - 1,
                        body.len(),
                        body.len() - start
                    ),
                    &body[start..],
                ),
                (Reply::Truncated(sent), _) => (
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"wheel-v1\"\r\nConnection: close\r\n\r\n",
                        body.len()
                    ),
                    &body[..sent],
                ),
                _ => (
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"wheel-v1\"\r\nConnection: close\r\n\r\n",
                        body.len()
                    ),
                    &body[..],
                ),
            };
            log.lock().unwrap().push(Served {
                range_start,
                body_bytes: payload.len(),
            });
            let _ = stream.write_all(status.as_bytes()).await;
            let _ = stream.write_all(payload).await;
            let _ = stream.flush().await;
            // Dropping the stream ends a truncated body mid-transfer.
        }
    });
    WheelOrigin {
        url,
        served,
        server,
    }
}

/// One process lifetime of the shared acquisition owner over a durable root.
struct Owner {
    service: Arc<AcquisitionService>,
    acquisition: TorchWheelAcquisition,
}

impl Owner {
    fn open(root: &Path, attempts: u32) -> Self {
        let store_root = root.join("store");
        std::fs::create_dir_all(&store_root).unwrap();
        let wheel_root = root.join("wheels");
        std::fs::create_dir_all(&wheel_root).unwrap();
        let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            &store_root,
        ))));
        let consumer = Arc::new(service.open_consumer(TORCH_WHEEL_CONSUMER).unwrap());
        let acquisition = TorchWheelAcquisition {
            consumer,
            root: std::fs::canonicalize(&wheel_root).unwrap(),
            lease: Arc::new(()),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(RwLock::new(InstallationProgressTracker::new(
                root.join("cache"),
            ))),
            log_path: root.join("install.log"),
            client: reqwest::Client::new(),
            retry: AcquisitionRetryPolicy {
                attempts: Some(attempts),
                elapsed: Duration::ZERO,
                backoff: RetryConfig::new()
                    .with_max_attempts(attempts)
                    .with_base_delay(Duration::from_millis(10)),
            },
        };
        Self {
            service,
            acquisition,
        }
    }

    async fn records(&self) -> Vec<AcquisitionRecord> {
        self.acquisition
            .consumer
            .retained_acquisitions()
            .await
            .unwrap()
    }

    async fn close(self) {
        self.acquisition.consumer.shutdown().await.unwrap();
        self.service.shutdown().await.unwrap();
    }
}

fn locked(origin: &WheelOrigin, body: &[u8]) -> LockedWheel {
    LockedWheel::new("torch", &origin.url, &sha256_hex(body)).unwrap()
}

fn partial_files(root: &Path) -> Vec<(String, u64)> {
    walkdir::WalkDir::new(root.join("wheels"))
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.metadata().unwrap().len(),
            )
        })
        .collect()
}

#[test]
fn locked_wheels_decode_file_names_and_refuse_with_reasons() {
    let wheel = LockedWheel::new(
        "torch",
        "https://download.pytorch.org/whl/cpu/torch-2.10.0%2Bcpu-cp312-cp312-linux_x86_64.whl",
        &"A".repeat(64),
    )
    .unwrap();
    assert_eq!(wheel.filename, WHEEL_NAME);
    assert_eq!(wheel.sha256, "a".repeat(64));

    let hashless = LockedWheel::new(
        "numpy",
        "https://files.pythonhosted.org/packages/numpy.whl",
        "",
    )
    .unwrap_err();
    assert!(hashless.contains("numpy has no SHA-256"), "{hashless}");
    for url in [
        "https://files.pythonhosted.org/packages/",
        "https://files.pythonhosted.org/packages/numpy.tar.gz",
        "https://files.pythonhosted.org/packages/..%2Fescape.whl",
        "https://files.pythonhosted.org/packages/bad%zz.whl",
    ] {
        let refused = LockedWheel::new("numpy", url, &"b".repeat(64)).unwrap_err();
        assert!(
            refused.contains("no plain wheel file name"),
            "{url}: {refused}"
        );
    }
}

/// The network drops mid-wheel; the same install retries and asks only for
/// the bytes it does not have yet, then verifies the whole file.
#[tokio::test]
async fn interrupted_wheel_transfer_resumes_instead_of_restarting() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let cut = body.len() / 3;
    let origin = serve_wheel(body.clone(), vec![Reply::Truncated(cut), Reply::Ranged]).await;
    let owner = Owner::open(root.path(), 3);
    let wheel = locked(&origin, &body);

    let held = owner
        .acquisition
        .hold(vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(held.file(&wheel)).unwrap(), body);
    let served = origin.served();
    assert_eq!(served.len(), 2, "{served:?}");
    assert_eq!(served[0].range_start, None);
    assert_eq!(served[1].range_start, Some(cut), "{served:?}");
    // Every byte crossed the network once: nothing was downloaded twice.
    assert_eq!(
        served.iter().map(|reply| reply.body_bytes).sum::<usize>(),
        body.len()
    );
    let log = std::fs::read_to_string(root.path().join("install.log")).unwrap();
    assert!(
        log.contains("retrying from the bytes already received"),
        "{log}"
    );

    let directory = held.directory().to_path_buf();
    held.finish(WheelUse::Consumed).await.unwrap();
    assert!(!directory.exists(), "a consumed wheel set is removed");
    let records = owner.records().await;
    assert_eq!(records.len(), 1);
    assert!(matches!(records[0].phase, AcquisitionPhase::Adopted { .. }));
    owner.close().await;
}

/// The outage outlasts every retry (the 2026-11-11 failure). The install
/// fails, but the received bytes stay, and the next install, in a new
/// process, resumes them rather than starting over.
#[tokio::test]
async fn exhausted_retries_keep_bytes_and_the_next_install_resumes() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let cut = body.len() / 2;
    let origin = serve_wheel(body.clone(), vec![Reply::Truncated(cut), Reply::Ranged]).await;
    let wheel = locked(&origin, &body);

    let first = Owner::open(root.path(), 1);
    let failed = match first.acquisition.hold(vec![wheel.clone()], false).await {
        Err(error) => error.to_string(),
        Ok(_) => panic!("a single truncated attempt must fail"),
    };
    assert!(
        failed.contains("downloaded bytes are kept so the next install resumes them"),
        "{failed}"
    );
    let records = first.records().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].phase, AcquisitionPhase::Transferring);
    assert_eq!(
        partial_files(root.path()),
        vec![(format!("{WHEEL_NAME}.part"), cut as u64)]
    );
    first.close().await;

    let second = Owner::open(root.path(), 1);
    let held = second
        .acquisition
        .hold(vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(held.file(&wheel)).unwrap(), body);
    let served = origin.served();
    assert_eq!(served.len(), 2, "{served:?}");
    assert_eq!(served[1].range_start, Some(cut), "{served:?}");
    assert_eq!(
        served.iter().map(|reply| reply.body_bytes).sum::<usize>(),
        body.len()
    );
    // The resumed install continued the same durable demand.
    let resumed = second.records().await;
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].demand, records[0].demand);
    held.finish(WheelUse::Consumed).await.unwrap();
    second.close().await;
}

#[tokio::test]
async fn bytes_that_do_not_match_the_locked_sha256_are_never_handed_off() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let origin = serve_wheel(body.clone(), vec![Reply::Full]).await;
    let owner = Owner::open(root.path(), 1);
    let mut tampered = body.clone();
    tampered[0] ^= 0xff;
    let wheel = locked(&origin, &tampered);
    assert!(owner.acquisition.hold(vec![wheel], false).await.is_err());
    assert!(owner.records().await.iter().all(|record| !matches!(
        record.phase,
        AcquisitionPhase::Using { .. } | AcquisitionPhase::Adopted { .. }
    )));
    owner.close().await;
}

/// A failed install keeps its verified wheels; the next attempt reuses them
/// without a request. A discarded set is removed.
#[tokio::test]
async fn retained_wheels_are_reused_and_discarded_wheels_are_removed() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let origin = serve_wheel(body.clone(), vec![Reply::Full]).await;
    let owner = Owner::open(root.path(), 1);
    let wheel = locked(&origin, &body);

    let held = owner
        .acquisition
        .hold(vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    held.finish(WheelUse::Retain).await.unwrap();
    assert_eq!(owner.records().await[0].phase, AcquisitionPhase::Withdrawn);

    let held = owner
        .acquisition
        .hold(vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(origin.served().len(), 1, "the retained wheel was reused");
    assert_eq!(std::fs::read(held.file(&wheel)).unwrap(), body);
    let directory = held.directory().to_path_buf();
    held.finish(WheelUse::Discard).await.unwrap();
    assert!(!directory.exists());
    let records = owner.records().await;
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .all(|record| record.phase == AcquisitionPhase::Withdrawn));
    owner.close().await;
}

/// A crash while pip read the wheels leaves an unreceipted use. The next
/// attempt releases it (its stage was disposable) and reuses the inputs.
#[tokio::test]
async fn use_left_by_an_interrupted_install_is_released_and_its_inputs_reused() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let origin = serve_wheel(body.clone(), vec![Reply::Full]).await;
    let wheel = locked(&origin, &body);
    let (key, manifest) = wheel_set(std::slice::from_ref(&wheel)).unwrap();
    {
        let crashed = Owner::open(root.path(), 1);
        let directory = crashed.acquisition.root.join(&key);
        std::fs::create_dir(&directory).unwrap();
        let grant = ReservedDirectory::capture(
            &crashed.acquisition.root,
            Path::new(&key),
            Arc::new(()),
            || Ok(()),
        )
        .unwrap();
        let interrupted = crashed
            .acquisition
            .consumer
            .acquire_http(
                AcquisitionHttpRequest {
                    demand: AcquisitionDemand {
                        consumer: TORCH_WHEEL_CONSUMER.into(),
                        operation: format!("{DEMAND_PREFIX}:{key}:1"),
                    },
                    manifest,
                    workspace: grant.acquisition_workspace().unwrap(),
                    sources: vec![AcquisitionHttpSource {
                        url: wheel.url.clone(),
                        authorization: None,
                    }],
                    retry: crashed.acquisition.retry.clone(),
                },
                reqwest::Client::new(),
                Box::new(TorchWheelHost {
                    sources: vec![wheel.url.clone()],
                    current: 0,
                    cancel_flag: Arc::new(AtomicBool::new(false)),
                    shutdown_flag: Arc::new(AtomicBool::new(false)),
                    progress: crashed.acquisition.progress.clone(),
                    log_path: root.path().join("install.log"),
                    cancelled_grant: None,
                    last_report: None,
                }),
                |_use_set| async {
                    Err::<((), serde_json::Value), _>(PumasError::Other("crash".into()))
                },
                |(), _receipt| async { Ok(()) },
            )
            .await;
        assert!(interrupted.is_err());
        assert!(matches!(
            crashed.records().await[0].phase,
            AcquisitionPhase::Using { .. }
        ));
        crashed.close().await;
    }

    let owner = Owner::open(root.path(), 1);
    let held = owner
        .acquisition
        .hold(vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(origin.served().len(), 1, "verified inputs were reused");
    held.finish(WheelUse::Consumed).await.unwrap();
    let mut phases: Vec<_> = owner
        .records()
        .await
        .into_iter()
        .map(|record| (record.demand.operation, record.phase))
        .collect();
    phases.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(phases[0].0, format!("{DEMAND_PREFIX}:{key}:1"));
    assert_eq!(phases[0].1, AcquisitionPhase::Withdrawn);
    assert_eq!(phases[1].0, format!("{DEMAND_PREFIX}:{key}:2"));
    assert!(matches!(phases[1].1, AcquisitionPhase::Adopted { .. }));
    owner.close().await;
}

#[tokio::test]
async fn cancellation_during_transfer_discards_the_partial_wheel() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let origin = serve_wheel(body.clone(), vec![Reply::Truncated(1024)]).await;
    let owner = Owner::open(root.path(), 1);
    let wheel = locked(&origin, &body);
    assert!(owner
        .acquisition
        .hold(vec![wheel.clone()], false)
        .await
        .is_err());
    assert_eq!(partial_files(root.path()).len(), 1);
    // The user cancels the retry that would resume those bytes.
    owner.acquisition.cancel_flag.store(true, Ordering::SeqCst);
    let result = owner.acquisition.hold(vec![wheel], false).await;
    let message = match result {
        Err(error) => error.to_string(),
        Ok(_) => panic!("cancelled acquisition must not hand off"),
    };
    assert!(message.contains("cancelled"), "{message}");
    assert!(partial_files(root.path()).is_empty());
    assert!(owner
        .records()
        .await
        .iter()
        .all(|record| record.phase == AcquisitionPhase::Withdrawn));
    owner.close().await;
}

#[tokio::test]
async fn settled_workspaces_of_other_selections_are_removed_and_unfinished_ones_kept() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let other_body: Vec<u8> = body.iter().rev().copied().collect();
    let unfinished = serve_wheel(other_body.clone(), vec![Reply::Truncated(512)]).await;
    let settled = serve_wheel(body.clone(), vec![Reply::Full]).await;
    let owner = Owner::open(root.path(), 1);

    let partial = LockedWheel::new("torch", &unfinished.url, &sha256_hex(&other_body)).unwrap();
    assert!(owner
        .acquisition
        .hold(vec![partial.clone()], false)
        .await
        .is_err());
    let kept = owner
        .acquisition
        .hold(vec![locked(&settled, &body)], false)
        .await;
    kept.unwrap()
        .unwrap()
        .finish(WheelUse::Retain)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_dir(&owner.acquisition.root).unwrap().count(),
        2
    );

    // Before the closure is known (the Torch wheel hold), nothing is removed.
    let third_body = vec![7_u8; 2048];
    let third = serve_wheel(third_body.clone(), vec![Reply::Full]).await;
    let third_wheel = LockedWheel::new("torch", &third.url, &sha256_hex(&third_body)).unwrap();
    owner
        .acquisition
        .hold(vec![third_wheel.clone()], false)
        .await
        .unwrap()
        .unwrap()
        .finish(WheelUse::Retain)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_dir(&owner.acquisition.root).unwrap().count(),
        3
    );

    // Holding the known closure removes the other settled (retained) set only.
    let held = owner
        .acquisition
        .hold(vec![third_wheel], true)
        .await
        .unwrap()
        .unwrap();
    let remaining: Vec<_> = partial_files(root.path())
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(
        remaining.contains(&format!("{WHEEL_NAME}.part")),
        "{remaining:?}"
    );
    assert_eq!(remaining.len(), 2, "{remaining:?}");
    held.finish(WheelUse::Consumed).await.unwrap();
    owner.close().await;
}
