//! Opt-in AC10 measurement through public acquisition APIs, in a fresh process.
//! Linux proc counters cover this process, including the controlled source.
#![cfg(target_os = "linux")]

use pumas_library::{
    acquisition::{
        AcquisitionCapacity, AcquisitionDemand, AcquisitionHost, AcquisitionHttpRequest,
        AcquisitionHttpSource, AcquisitionPhase, AcquisitionRetryPolicy, AcquisitionService,
        AcquisitionStore, AcquisitionWorkspace, ArtifactFile, ArtifactManifest,
        ArtifactRevisionEvidence, ArtifactSourceIdentity, FileVerificationRequirement,
        HttpAttemptHost, RevisionStrength, Sha256Evidence,
    },
    network::RetryConfig,
    PumasError, Result,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    future::pending,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{mpsc, Notify},
    task::{JoinHandle, JoinSet},
};

const CHUNK: usize = 64 * 1024;
const MIB: u64 = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(300);

struct Settings {
    bytes: u64,
    workers: usize,
    control: bool,
}
impl Settings {
    fn new(bytes: u64, workers: usize, control: bool) -> Self {
        assert!(
            (MIB..=256 * MIB).contains(&bytes),
            "payload must be 1..256 MiB"
        );
        assert!((1..=4).contains(&workers), "workers must be 1..4");
        assert!(
            bytes * workers as u64 <= 512 * MIB,
            "total payload exceeds 512 MiB"
        );
        Self {
            bytes,
            workers,
            control,
        }
    }
}

fn counters() -> BTreeMap<String, u64> {
    let mut values = BTreeMap::new();
    for (file, fields) in [
        ("/proc/self/status", vec!["VmRSS", "VmHWM"]),
        (
            "/proc/self/io",
            vec!["rchar", "wchar", "read_bytes", "write_bytes"],
        ),
    ] {
        let text = std::fs::read_to_string(file).expect("Linux process counters required");
        for line in text.lines() {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            if fields.contains(&name) {
                let mut value = value.split_whitespace();
                let number: u64 = value.next().unwrap().parse().unwrap();
                values.insert(
                    name.into(),
                    if value.next() == Some("kB") {
                        number * 1024
                    } else {
                        number
                    },
                );
            }
        }
    }
    assert_eq!(values.len(), 6);
    values
}

#[derive(Default)]
struct Traffic {
    requests: AtomicU64,
    payload: AtomicU64,
    responses: AtomicU64,
}
struct Source {
    address: String,
    release: Arc<Notify>,
    traffic: Arc<Traffic>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Source {
    async fn start(bytes: u64) -> (Self, mpsc::UnboundedReceiver<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let traffic = Arc::new(Traffic::default());
        let release = Arc::new(Notify::new());
        let (started, receiver) = mpsc::unbounded_channel();
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let traffic_for_task = traffic.clone();
        let release_for_task = release.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    connection = listener.accept() => {
                        let (mut socket, _) = connection.unwrap();
                        let traffic = traffic_for_task.clone();
                        let release = release_for_task.clone();
                        let started = started.clone();
                        connections.spawn(async move {
                            let mut request = Vec::new();
                            while !request.ends_with(b"\r\n\r\n") {
                                assert!(request.len() < 16 * 1024);
                                request.push(socket.read_u8().await.unwrap());
                            }
                            let request = String::from_utf8(request).unwrap();
                            assert!(request.starts_with("GET "));
                            let index: usize = request.split_whitespace().nth(1).unwrap().trim_start_matches('/').parse().unwrap();
                            traffic.requests.fetch_add(1, Ordering::SeqCst);
                            let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {bytes}\r\nETag: \"resource-v1-{index}\"\r\nConnection: close\r\n\r\n");
                            socket.write_all(header.as_bytes()).await.unwrap();
                            traffic.responses.fetch_add(header.len() as u64, Ordering::SeqCst);
                            let block = vec![index as u8 + 1; CHUNK];
                            let released = release.notified();
                            tokio::pin!(released);
                            released.as_mut().enable();
                            socket.write_all(&block).await.unwrap();
                            traffic.payload.fetch_add(CHUNK as u64, Ordering::SeqCst);
                            traffic.responses.fetch_add(CHUNK as u64, Ordering::SeqCst);
                            started.send(index).unwrap();
                            released.await;
                            let mut remaining = bytes - CHUNK as u64;
                            while remaining > 0 {
                                let count = remaining.min(CHUNK as u64) as usize;
                                socket.write_all(&block[..count]).await.unwrap();
                                traffic.payload.fetch_add(count as u64, Ordering::SeqCst);
                                traffic.responses.fetch_add(count as u64, Ordering::SeqCst);
                                remaining -= count as u64;
                            }
                            socket.shutdown().await.unwrap();
                        });
                    }
                }
            }
            while let Some(result) = connections.join_next().await {
                result.unwrap();
            }
        });
        (
            Self {
                address,
                release,
                traffic,
                stop: Some(stop),
                task,
            },
            receiver,
        )
    }
    async fn finish(&mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(DEADLINE, &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Default)]
struct Progress {
    bytes: AtomicU64,
    step: AtomicU64,
}
struct Host(Arc<Progress>);
#[async_trait::async_trait]
impl HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        false
    }
    async fn record_progress(&mut self, bytes: u64) -> Result<()> {
        let previous = self.0.bytes.swap(bytes, Ordering::SeqCst);
        self.0
            .step
            .fetch_max(bytes.saturating_sub(previous), Ordering::SeqCst);
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    async fn retry(
        &mut self,
        attempt: u32,
        delay: Option<Duration>,
        error: Option<&str>,
    ) -> Result<()> {
        // The owner announces the initial attempt through this same hook.
        assert_eq!(attempt, 1, "resource probe must not retry");
        assert!(delay.is_none() && error.is_none());
        Ok(())
    }
}

fn payload_digest(bytes: u64, index: usize) -> String {
    let mut hash = Sha256::new();
    let block = vec![index as u8 + 1; CHUNK];
    let mut remaining = bytes;
    while remaining > 0 {
        let count = remaining.min(CHUNK as u64) as usize;
        hash.update(&block[..count]);
        remaining -= count as u64;
    }
    hex::encode(hash.finalize())
}

fn request(root: &Path, address: &str, bytes: u64, index: usize) -> AcquisitionHttpRequest {
    let stage = format!("stage-{index}");
    std::fs::create_dir(root.join(&stage)).unwrap();
    AcquisitionHttpRequest {
        demand: AcquisitionDemand {
            consumer: "fixture.resources".into(),
            operation: format!("payload-{index}"),
        },
        manifest: ArtifactManifest::new(
            ArtifactSourceIdentity::new(
                "fixture",
                format!("payload-{index}"),
                ArtifactRevisionEvidence::new(
                    "fixture.revision",
                    "v1",
                    RevisionStrength::Immutable,
                )
                .unwrap(),
            )
            .unwrap(),
            vec![ArtifactFile::new(
                "payload.bin",
                format!("payload-{index}"),
                Some(bytes),
                Some(Sha256Evidence::new("fixture.sha256", payload_digest(bytes, index)).unwrap()),
                FileVerificationRequirement::Sha256,
            )
            .unwrap()],
        )
        .unwrap(),
        workspace: AcquisitionWorkspace::from_reserved_directory(
            root,
            Path::new(&stage),
            Arc::new(()),
            || Ok(()),
        )
        .unwrap(),
        sources: vec![AcquisitionHttpSource {
            url: format!("http://{address}/{index}"),
            authorization: None,
        }],
        retry: AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::ZERO,
            backoff: RetryConfig::default().with_jitter(false),
        },
    }
}

/// A sampled payload footprint, not an exact peak or a filesystem-wide budget.
fn footprint(root: &Path, workers: usize) -> (u64, u64, usize) {
    let mut inodes = BTreeMap::new();
    for index in 0..workers {
        for entry in std::fs::read_dir(root.join(format!("stage-{index}"))).unwrap() {
            let entry = entry.unwrap();
            assert!(matches!(
                entry.file_name().to_str(),
                Some("payload.bin.part" | "payload.bin")
            ));
            match std::fs::symlink_metadata(entry.path()) {
                Ok(metadata) => {
                    assert!(metadata.is_file());
                    let sizes = inodes
                        .entry((metadata.dev(), metadata.ino()))
                        .or_insert((0_u64, 0_u64));
                    // A rename racing this sample can expose the same inode twice.
                    sizes.0 = sizes.0.max(metadata.len());
                    sizes.1 = sizes.1.max(metadata.blocks() * 512);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("payload measurement failed: {error}"),
            }
        }
    }
    (
        inodes.values().map(|v| v.0).sum(),
        inodes.values().map(|v| v.1).sum(),
        inodes.len(),
    )
}

struct Sampler {
    stop: Arc<AtomicBool>,
    task: JoinHandle<(u64, u64, u64)>,
}
impl Sampler {
    fn start(root: PathBuf, workers: usize) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let task = tokio::task::spawn_blocking(move || {
            let mut logical = 0;
            let mut allocated = 0;
            let mut samples = 0;
            loop {
                let observed = footprint(&root, workers);
                logical = logical.max(observed.0);
                allocated = allocated.max(observed.1);
                samples += 1;
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            (logical, allocated, samples)
        });
        Self { stop, task }
    }
    async fn finish(&mut self) -> (u64, u64, u64) {
        self.stop.store(true, Ordering::SeqCst);
        tokio::time::timeout(DEADLINE, &mut self.task)
            .await
            .unwrap()
            .unwrap()
    }
}
impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "explicit fresh-process resource measurement; see scripts/release/measure-acquisition-resources.py"]
async fn resource_probe() {
    let bytes = std::env::var("PUMAS_RESOURCE_BYTES")
        .unwrap()
        .parse()
        .unwrap();
    let workers = std::env::var("PUMAS_RESOURCE_WORKERS")
        .unwrap()
        .parse()
        .unwrap();
    let control = match std::env::var("PUMAS_RESOURCE_RETAIN_PAYLOAD")
        .unwrap()
        .as_str()
    {
        "0" => false,
        "1" => true,
        _ => panic!("invalid control mode"),
    };
    let config = Settings::new(bytes, workers, control);
    let root = tempfile::tempdir_in(
        std::env::var_os("PUMAS_RESOURCE_ROOT").expect("explicit measurement filesystem required"),
    )
    .unwrap();
    std::fs::create_dir(root.path().join("state")).unwrap();
    let store = Arc::new(AcquisitionStore::new(&root.path().join("state")));
    let service = Arc::new(
        AcquisitionService::with_capacity(
            store.clone(),
            AcquisitionCapacity {
                workers,
                blocking: workers,
                rescue_workers: 2,
                rescue_blocking: 2,
                scopes: 4,
            },
        )
        .unwrap(),
    );
    let consumer = Arc::new(service.open_consumer("fixture.resources").unwrap());
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .unwrap();
    let (mut source, mut started) = Source::start(bytes).await;
    let requests: Vec<_> = (0..workers)
        .map(|index| request(root.path(), &source.address, bytes, index))
        .collect();
    let extra = request(root.path(), &source.address, bytes, workers);
    let before = counters();
    let begun = Instant::now();
    // Deliberate measurement control only; not part of any production path.
    let retained = config
        .control
        .then(|| vec![0x58_u8; (bytes * workers as u64) as usize]);
    let after_allocation = counters();
    let mut sampler = Sampler::start(root.path().to_path_buf(), workers);
    let mut tasks = JoinSet::new();
    let mut progress = Vec::new();
    for (index, request) in requests.into_iter().enumerate() {
        let consumer = consumer.clone();
        let client = client.clone();
        let observed = Arc::new(Progress::default());
        progress.push(observed.clone());
        let final_path = root.path().join(format!("stage-{index}/payload.bin"));
        tasks.spawn(async move {
            consumer
                .acquire_http(
                    request,
                    client,
                    Box::new(Host(observed)),
                    move |use_set| async move {
                        let mut file = use_set.open_file(0).await?;
                        use_set
                            .run_blocking(
                                "resource probe reads verified ordinary file",
                                move || {
                                    assert_eq!(
                                        file.metadata().unwrap().ino(),
                                        std::fs::metadata(final_path).unwrap().ino()
                                    );
                                    let mut hash = Sha256::new();
                                    let mut buffer = vec![0_u8; CHUNK];
                                    loop {
                                        let count = file.read(&mut buffer).unwrap();
                                        if count == 0 {
                                            break;
                                        }
                                        hash.update(&buffer[..count]);
                                    }
                                    assert_eq!(
                                        hex::encode(hash.finalize()),
                                        payload_digest(bytes, index)
                                    );
                                    Ok(())
                                },
                            )
                            .await?;
                        Ok(((), serde_json::json!({"verified_ordinary_file": true})))
                    },
                    |(), receipt| async move {
                        assert_eq!(receipt.verified_files.len(), 1);
                        Ok(())
                    },
                )
                .await
                .unwrap();
        });
    }
    tokio::time::timeout(DEADLINE, async {
        tokio::select! {
            () = async {
                for _ in 0..workers { started.recv().await.unwrap(); }
                while progress.iter().any(|p| p.bytes.load(Ordering::SeqCst) < CHUNK as u64) {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            } => {},
            outcome = tasks.join_next() => panic!("acquisition ended before the held-response observation: {outcome:?}"),
        }
    })
    .await
    .unwrap();
    let partial_inodes: Vec<_> = (0..workers)
        .map(|i| {
            std::fs::metadata(root.path().join(format!("stage-{i}/payload.bin.part")))
                .unwrap()
                .ino()
        })
        .collect();
    let inventory = store.acquisitions().unwrap();
    let refusal = consumer
        .acquire_http(
            extra,
            client,
            Box::new(Host(Arc::new(Progress::default()))),
            |_| async { unreachable!("overload must not grant consumer use") },
            |(): (), _| async { Ok(()) },
        )
        .await;
    assert!(matches!(
        refusal,
        Err(PumasError::AcquisitionCapacityExhausted {
            resource: "workers"
        })
    ));
    assert_eq!(store.acquisitions().unwrap(), inventory);
    let held_source_requests = source.traffic.requests.load(Ordering::SeqCst);
    assert_eq!(held_source_requests, workers as u64);
    assert!(!root
        .path()
        .join(format!("stage-{workers}/payload.bin.part"))
        .exists());
    source.release.notify_waiters();
    tokio::time::timeout(DEADLINE, async {
        while let Some(result) = tasks.join_next().await {
            result.unwrap();
        }
    })
    .await
    .unwrap();
    for (index, inode) in partial_inodes.into_iter().enumerate() {
        assert_eq!(
            std::fs::metadata(root.path().join(format!("stage-{index}/payload.bin")))
                .unwrap()
                .ino(),
            inode
        );
        assert_eq!(progress[index].bytes.load(Ordering::SeqCst), bytes);
    }
    let records = store.acquisitions().unwrap();
    assert_eq!(records.len(), workers);
    for record in records.values() {
        assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
        let receipt = service.consumer_receipt(record.id).unwrap().unwrap();
        assert_eq!(receipt.manifest, record.manifest);
        assert_eq!(receipt.verified_files, record.files);
        assert_eq!(
            receipt.payload,
            serde_json::json!({"verified_ordinary_file": true})
        );
    }
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    source.finish().await;
    let final_disk = footprint(root.path(), workers);
    assert_eq!(final_disk.0, bytes * workers as u64);
    assert_eq!(final_disk.2, workers);
    let sampled = sampler.finish().await;
    assert_eq!(sampled.0, final_disk.0);
    assert_eq!(
        source.traffic.payload.load(Ordering::SeqCst),
        bytes * workers as u64
    );
    assert_eq!(
        source.traffic.requests.load(Ordering::SeqCst),
        workers as u64
    );
    std::hint::black_box(&retained);
    let after = counters();
    println!(
        "PUMAS_RESOURCE_RESULT:{}",
        serde_json::json!({
            "schema_version": 1, "configured": {"payload_bytes_per_worker": config.bytes, "workers": config.workers, "blocking": workers, "source_chunk_bytes": CHUNK, "sampling_interval_ms": 25, "deadline_seconds": DEADLINE.as_secs(), "retain_payload_control": config.control},
            "measured": {"process_before": before, "process_after_control_allocation": after_allocation, "process_after": after,
                "sampled_peak_payload_logical_bytes": sampled.0, "sampled_peak_payload_allocated_bytes": sampled.1, "disk_samples": sampled.2,
                "final_payload_logical_bytes": final_disk.0, "final_payload_allocated_bytes": final_disk.1, "final_payload_inodes": final_disk.2,
            "source_requests": source.traffic.requests.load(Ordering::SeqCst), "simultaneously_held_source_requests": held_source_requests, "source_payload_bytes_written": source.traffic.payload.load(Ordering::SeqCst), "source_response_bytes_written": source.traffic.responses.load(Ordering::SeqCst),
                "max_progress_step_bytes": progress.iter().map(|p| p.step.load(Ordering::SeqCst)).max().unwrap(), "elapsed_seconds": begun.elapsed().as_secs_f64()},
            "checks": {"overload_refused_before_source_and_admission": true, "sha256_and_receipt_verified": true, "same_payload_inode": true, "drained_shutdown": true},
            "not_measured": ["network_wire_bytes", "exact_peak_disk", "internal_buffer_or_hash_concurrency", "system_or_cgroup_ram", "real_model_import", "native_provider", "production_release_performance"]
        })
    );
}
