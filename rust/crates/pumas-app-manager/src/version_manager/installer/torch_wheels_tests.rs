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
    /// Announce the whole body, send this many bytes, then go silent while
    /// keeping the connection open.
    Stalled(usize),
    /// Refuse any range with 416 Range Not Satisfiable.
    NotSatisfiable,
}

/// One install selection, as `torch.rs` derives it from release, build,
/// Python, dependency profile, platform and wheel role.
const SELECTION: &str = "v2.10.0|cpu|python3.12|none|linux-x86_64|torch";

/// Test stall bound: long enough for loopback, short enough for a test.
const TEST_STALL: Duration = Duration::from_millis(300);

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
    // The CDN-style path escapes the local-version `+`, as the PyTorch index does.
    serve_named(
        "whl/cpu/torch-2.10.0%2Bcpu-cp312-cp312-linux_x86_64.whl",
        body,
        script,
    )
    .await
}

async fn serve_named(path: &str, body: Vec<u8>, script: Vec<Reply>) -> WheelOrigin {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/{path}", listener.local_addr().unwrap());
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
                (Reply::NotSatisfiable, _) => (
                    format!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        body.len()
                    ),
                    &body[..0],
                ),
                (Reply::Truncated(sent) | Reply::Stalled(sent), _) => (
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
            if matches!(reply, Reply::Stalled(_)) {
                // Hold the connection open and silent; keep serving others.
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    drop(stream);
                });
            }
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
            client: wheel_http_client(TEST_STALL).unwrap(),
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
        .hold(SELECTION, vec![wheel.clone()], false)
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
    let failed = match first
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
    {
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
        .hold(SELECTION, vec![wheel.clone()], false)
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
    assert!(owner
        .acquisition
        .hold(SELECTION, vec![wheel], false)
        .await
        .is_err());
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
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    held.finish(WheelUse::Retain).await.unwrap();
    assert_eq!(owner.records().await[0].phase, AcquisitionPhase::Withdrawn);

    let held = owner
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
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
                        operation: demand_operation(&selection_scope(SELECTION), &key, 1),
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
        .hold(SELECTION, vec![wheel.clone()], false)
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
    assert_eq!(
        phases[0].0,
        demand_operation(&selection_scope(SELECTION), &key, 1)
    );
    assert_eq!(phases[0].1, AcquisitionPhase::Withdrawn);
    assert_eq!(
        phases[1].0,
        demand_operation(&selection_scope(SELECTION), &key, 2)
    );
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
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
        .is_err());
    assert_eq!(partial_files(root.path()).len(), 1);
    // The user cancels the retry that would resume those bytes.
    owner.acquisition.cancel_flag.store(true, Ordering::SeqCst);
    let result = owner.acquisition.hold(SELECTION, vec![wheel], false).await;
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
        .hold("selection-unfinished", vec![partial.clone()], false)
        .await
        .is_err());
    let kept = owner
        .acquisition
        .hold("selection-settled", vec![locked(&settled, &body)], false)
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
        .hold("selection-third", vec![third_wheel.clone()], false)
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
        .hold("selection-third", vec![third_wheel], true)
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

/// The connection stays open but goes silent mid-wheel. The stall bound ends
/// the attempt and the retry resumes from the bytes already received.
#[tokio::test]
async fn stalled_wheel_transfer_retries_and_resumes() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let cut = body.len() / 4;
    let origin = serve_wheel(body.clone(), vec![Reply::Stalled(cut), Reply::Ranged]).await;
    let owner = Owner::open(root.path(), 3);
    let wheel = locked(&origin, &body);

    let held = tokio::time::timeout(
        Duration::from_secs(20),
        owner
            .acquisition
            .hold(SELECTION, vec![wheel.clone()], false),
    )
    .await
    .expect("a stalled transfer must not hang the install")
    .unwrap()
    .unwrap();
    assert_eq!(std::fs::read(held.file(&wheel)).unwrap(), body);
    let served = origin.served();
    assert_eq!(served.len(), 2, "{served:?}");
    assert_eq!(served[1].range_start, Some(cut), "{served:?}");
    held.finish(WheelUse::Consumed).await.unwrap();
    owner.close().await;
}

/// Shutdown sets both the shutdown and cancel flags. It pauses the transfer
/// and keeps its bytes; the next start resumes them.
#[tokio::test]
async fn shutdown_keeps_partial_wheels_for_the_next_start() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let cut = body.len() / 2;
    let origin = serve_wheel(body.clone(), vec![Reply::Truncated(cut), Reply::Ranged]).await;
    let wheel = locked(&origin, &body);

    let first = Owner::open(root.path(), 1);
    assert!(first
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
        .is_err());
    first
        .acquisition
        .shutdown_flag
        .store(true, Ordering::SeqCst);
    first.acquisition.cancel_flag.store(true, Ordering::SeqCst);
    let stopped = match first
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
    {
        Err(error) => error.to_string(),
        Ok(_) => panic!("shutdown must not hand off"),
    };
    assert!(stopped.contains("cancelled"), "{stopped}");
    assert_eq!(
        partial_files(root.path()),
        vec![(format!("{WHEEL_NAME}.part"), cut as u64)]
    );
    assert_eq!(
        first.records().await[0].phase,
        AcquisitionPhase::Transferring
    );
    first.close().await;

    let second = Owner::open(root.path(), 1);
    let held = second
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(held.file(&wheel)).unwrap(), body);
    assert_eq!(origin.served().last().unwrap().range_start, Some(cut));
    held.finish(WheelUse::Consumed).await.unwrap();
    second.close().await;
}

/// The resolver may list the same closure in another order on the next
/// attempt; that attempt still resumes the same durable demand.
#[tokio::test]
async fn the_same_wheel_set_in_another_order_resumes() {
    let root = tempfile::TempDir::new().unwrap();
    let alpha_body = vec![1_u8; 4096];
    let beta_body = wheel_bytes();
    let cut = beta_body.len() / 2;
    let alpha = serve_named(
        "packages/alpha-1.0-py3-none-any.whl",
        alpha_body.clone(),
        vec![Reply::Full],
    )
    .await;
    let beta = serve_named(
        "packages/beta-1.0-py3-none-any.whl",
        beta_body.clone(),
        vec![Reply::Truncated(cut), Reply::Ranged],
    )
    .await;
    let alpha_wheel = LockedWheel::new("alpha", &alpha.url, &sha256_hex(&alpha_body)).unwrap();
    let beta_wheel = LockedWheel::new("beta", &beta.url, &sha256_hex(&beta_body)).unwrap();
    let owner = Owner::open(root.path(), 1);

    assert!(owner
        .acquisition
        .hold(
            SELECTION,
            vec![alpha_wheel.clone(), beta_wheel.clone()],
            true
        )
        .await
        .is_err());
    let interrupted = owner.records().await;
    let held = owner
        .acquisition
        .hold(
            SELECTION,
            vec![beta_wheel.clone(), alpha_wheel.clone()],
            true,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(held.file(&beta_wheel)).unwrap(), beta_body);
    assert_eq!(std::fs::read(held.file(&alpha_wheel)).unwrap(), alpha_body);
    assert_eq!(
        alpha.served().len(),
        1,
        "the verified wheel was not fetched again"
    );
    assert_eq!(beta.served().last().unwrap().range_start, Some(cut));
    let resumed = owner.records().await;
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].demand, interrupted[0].demand);
    held.finish(WheelUse::Consumed).await.unwrap();
    owner.close().await;
}

/// The source answers the resume request with the whole body (it ignores
/// Range). The partial cannot be continued, so the wheel restarts from byte
/// zero and is still verified, instead of failing forever.
#[tokio::test]
async fn a_source_that_ignores_range_restarts_the_wheel_from_byte_zero() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let cut = body.len() / 3;
    let origin = serve_wheel(
        body.clone(),
        vec![Reply::Truncated(cut), Reply::Full, Reply::Full],
    )
    .await;
    let owner = Owner::open(root.path(), 3);
    let wheel = locked(&origin, &body);

    let held = owner
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(held.file(&wheel)).unwrap(), body);
    let ranges: Vec<_> = origin
        .served()
        .iter()
        .map(|reply| reply.range_start)
        .collect();
    // Fresh, refused continuation (the full body is not appended), restart.
    assert_eq!(ranges, vec![None, Some(cut), None]);
    held.finish(WheelUse::Consumed).await.unwrap();
    owner.close().await;
}

/// A later install finds a retained partial, but the source now answers 416.
/// The partial is discarded and the wheel downloads again in full.
#[tokio::test]
async fn a_416_for_a_retained_partial_restarts_the_wheel_from_byte_zero() {
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let cut = body.len() / 2;
    let origin = serve_wheel(
        body.clone(),
        vec![Reply::Truncated(cut), Reply::NotSatisfiable, Reply::Full],
    )
    .await;
    let wheel = locked(&origin, &body);

    let first = Owner::open(root.path(), 1);
    assert!(first
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
        .is_err());
    assert_eq!(partial_files(root.path()).len(), 1);
    first.close().await;

    let second = Owner::open(root.path(), 1);
    let held = second
        .acquisition
        .hold(SELECTION, vec![wheel.clone()], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(held.file(&wheel)).unwrap(), body);
    let ranges: Vec<_> = origin
        .served()
        .iter()
        .map(|reply| reply.range_start)
        .collect();
    assert_eq!(ranges, vec![None, Some(cut), None]);
    held.finish(WheelUse::Consumed).await.unwrap();
    second.close().await;
}

/// A new resolution of the same install selection names a different wheel
/// set. No current demand can resume the old set's bytes, so it is withdrawn
/// and removed. Another selection's unfinished set stays resumable.
#[tokio::test]
async fn a_new_resolution_supersedes_its_unfinished_set_but_not_other_selections() {
    const OTHER_SELECTION: &str = "v2.10.0|cpu|python3.13|none|linux-x86_64|torch";
    let root = tempfile::TempDir::new().unwrap();
    let body = wheel_bytes();
    let old = serve_named(
        "packages/old-1.0-py3-none-any.whl",
        body.clone(),
        vec![Reply::Truncated(1024)],
    )
    .await;
    let other = serve_named(
        "packages/other-1.0-py3-none-any.whl",
        body.clone(),
        vec![Reply::Truncated(2048), Reply::Ranged],
    )
    .await;
    let new = serve_named(
        "packages/new-1.0-py3-none-any.whl",
        body.clone(),
        vec![Reply::Full],
    )
    .await;
    let old_wheel = LockedWheel::new("old", &old.url, &sha256_hex(&body)).unwrap();
    let other_wheel = LockedWheel::new("other", &other.url, &sha256_hex(&body)).unwrap();
    let new_wheel = LockedWheel::new("new", &new.url, &sha256_hex(&body)).unwrap();
    let owner = Owner::open(root.path(), 1);

    assert!(owner
        .acquisition
        .hold(SELECTION, vec![old_wheel], false)
        .await
        .is_err());
    assert!(owner
        .acquisition
        .hold(OTHER_SELECTION, vec![other_wheel.clone()], false)
        .await
        .is_err());
    assert_eq!(partial_files(root.path()).len(), 2);

    let held = owner
        .acquisition
        .hold(SELECTION, vec![new_wheel], false)
        .await
        .unwrap()
        .unwrap();
    let parts: Vec<_> = partial_files(root.path())
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(
        !parts.contains(&"old-1.0-py3-none-any.whl.part".to_owned()),
        "{parts:?}"
    );
    assert!(
        parts.contains(&"other-1.0-py3-none-any.whl.part".to_owned()),
        "{parts:?}"
    );
    let phase_of = |records: &[AcquisitionRecord], name: &str| {
        records
            .iter()
            .find(|record| record.manifest.files()[0].logical_path() == name)
            .map(|record| record.phase.clone())
    };
    let records = owner.records().await;
    assert_eq!(
        phase_of(&records, "old-1.0-py3-none-any.whl"),
        Some(AcquisitionPhase::Withdrawn)
    );
    assert_eq!(
        phase_of(&records, "other-1.0-py3-none-any.whl"),
        Some(AcquisitionPhase::Transferring)
    );
    held.finish(WheelUse::Consumed).await.unwrap();

    // The other selection's demand still resumes its bytes.
    let resumed = owner
        .acquisition
        .hold(OTHER_SELECTION, vec![other_wheel], false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(other.served().last().unwrap().range_start, Some(2048));
    resumed.finish(WheelUse::Consumed).await.unwrap();
    owner.close().await;
}
