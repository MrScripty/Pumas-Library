//! Synthetic local-input custody; no wheel installation/import.
use super::*;
use crate::acquisition::{
    AcquisitionLocalSource, ArtifactFile, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, RevisionStrength, Sha256Evidence,
};
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
struct Host;
#[async_trait::async_trait]
impl HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        std::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        false
    }
    async fn record_progress(&mut self, _: u64) -> Result<()> {
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    async fn retry(&mut self, _: u32, _: Option<Duration>, _: Option<&str>) -> Result<()> {
        Ok(())
    }
}
fn manifest(bytes: &[u8]) -> ArtifactManifest {
    ArtifactManifest::new(
        ArtifactSourceIdentity::new(
            "local-fixture",
            "synthetic-component",
            ArtifactRevisionEvidence::new(
                "fixture.source",
                "immutable-v1",
                RevisionStrength::Immutable,
            )
            .unwrap(),
        )
        .unwrap(),
        vec![ArtifactFile::new(
            "component.bin",
            "selected-component",
            Some(bytes.len() as u64),
            Some(
                Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(bytes))).unwrap(),
            ),
            FileVerificationRequirement::Sha256,
        )
        .unwrap()],
    )
    .unwrap()
}
fn request(
    stage: &std::path::Path,
    source: AcquisitionLocalSource,
    manifest: ArtifactManifest,
) -> AcquisitionLocalRequest {
    let root = crate::platform::capability_fs::open_directory(stage).unwrap();
    let workspace = AcquisitionWorkspace::from_capability(
        root,
        WorkspaceIdentity {
            root_identity: "synthetic-local-root".into(),
            relative_target: "stage".into(),
        },
        Arc::new(()),
        || Ok(()),
    )
    .unwrap();
    AcquisitionLocalRequest {
        demand: AcquisitionDemand {
            consumer: "local-fixture".into(),
            operation: "ingest".into(),
        },
        manifest,
        workspace,
        sources: vec![source],
        retry: AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(5),
            backoff: crate::network::RetryConfig::new(),
        },
    }
}
async fn ingest(
    consumer: &AcquisitionConsumer,
    request: AcquisitionLocalRequest,
) -> Result<(Vec<u8>, AcquisitionConsumerReceipt)> {
    consumer
        .acquire_local(
            request,
            Box::new(Host),
            |use_handle| async move {
                let mut file = use_handle.open_file(0).await?;
                let bytes = use_handle
                    .run_blocking("read synthetic verified input", move || {
                        let mut bytes = Vec::new();
                        file.read_to_end(&mut bytes)?;
                        Ok(bytes)
                    })
                    .await?;
                Ok((bytes, serde_json::json!({"synthetic":"verified"})))
            },
            |bytes, receipt| async move { Ok((bytes, receipt)) },
        )
        .await
}
fn setup() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    Arc<AcquisitionService>,
    AcquisitionConsumer,
) {
    let temp = tempfile::tempdir().unwrap();
    let stage = temp.path().join("stage");
    std::fs::create_dir(&stage).unwrap();
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        temp.path(),
    ))));
    let consumer = service.open_consumer("local-fixture").unwrap();
    (temp, stage, service, consumer)
}
#[tokio::test]
async fn local_ingestion_uses_retained_descriptor_not_path_or_shared_cursor() {
    let (temp, stage, service, consumer) = setup();
    let input = temp.path().join("input");
    std::fs::write(&input, b"DATA").unwrap();
    let mut file = std::fs::File::open(&input).unwrap();
    file.seek(SeekFrom::End(0)).unwrap();
    let source = AcquisitionLocalSource::new(file, Arc::new(())).unwrap();
    std::fs::rename(&input, temp.path().join("old-input")).unwrap();
    std::fs::write(&input, b"EVIL").unwrap();
    let (bytes, receipt) = ingest(&consumer, request(&stage, source, manifest(b"DATA")))
        .await
        .unwrap();
    assert_eq!(bytes, b"DATA");
    assert_eq!(
        receipt.verified_files[0].sha256,
        hex::encode(Sha256::digest(b"DATA"))
    );
    let records = service.store.acquisitions().unwrap();
    let record = records.values().next().unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    assert_eq!(
        service.consumer_receipt(record.id).unwrap().unwrap(),
        receipt
    );
    assert_eq!(std::fs::read(input).unwrap(), b"EVIL");
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn local_ingestion_hash_and_size_refuse_before_store_or_consumer_effects() {
    for wrong in [b"EVIL".as_slice(), b"short".as_slice()] {
        let (temp, stage, service, consumer) = setup();
        let previous = temp.path().join("previous-active");
        std::fs::write(&previous, b"old-runtime").unwrap();
        let input = temp.path().join("input");
        std::fs::write(&input, wrong).unwrap();
        let source =
            AcquisitionLocalSource::new(std::fs::File::open(input).unwrap(), Arc::new(())).unwrap();
        assert!(matches!(
            ingest(&consumer, request(&stage, source, manifest(b"DATA"))).await,
            Err(PumasError::Validation { .. })
        ));
        assert!(service.store.acquisitions().unwrap().is_empty());
        assert!(std::fs::read_dir(&stage).unwrap().next().is_none());
        assert_eq!(std::fs::read(previous).unwrap(), b"old-runtime");
        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn local_ingestion_requires_digest_and_positive_budgets_and_handles_empty() {
    for mode in 0..4 {
        let (temp, stage, service, consumer) = setup();
        let input = temp.path().join("input");
        std::fs::write(&input, b"").unwrap();
        let source =
            AcquisitionLocalSource::new(std::fs::File::open(input).unwrap(), Arc::new(())).unwrap();
        let mut request = request(&stage, source, manifest(b""));
        match mode {
            0 => {
                request.manifest = ArtifactManifest::new(
                    request.manifest.source().clone(),
                    vec![ArtifactFile::new(
                        "component.bin",
                        "selected-component",
                        Some(0),
                        None,
                        FileVerificationRequirement::SizeAndImmutableRevision,
                    )
                    .unwrap()],
                )
                .unwrap()
            }
            1 => request.retry.attempts = None,
            2 => request.retry.elapsed = Duration::ZERO,
            _ => {}
        }
        let result = ingest(&consumer, request).await;
        if mode == 3 {
            assert!(result.unwrap().0.is_empty());
        } else {
            assert!(matches!(result, Err(PumasError::Validation { .. })));
            assert!(service.store.acquisitions().unwrap().is_empty());
        }
        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
}
struct DropFlag(Arc<AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[tokio::test]
async fn local_ingestion_caller_loss_retains_input_until_registered_read_drains() {
    let (temp, stage, service, consumer) = setup();
    let consumer = Arc::new(consumer);
    let input = temp.path().join("input");
    std::fs::write(&input, b"DATA").unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let source = AcquisitionLocalSource::new(
        std::fs::File::open(input).unwrap(),
        Arc::new(DropFlag(dropped.clone())),
    )
    .unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    let wait = Mutex::new(wait);
    let started = Arc::new(AtomicBool::new(false));
    let observed = started.clone();
    consumer
        .scope
        .set_blocking_observer(Some(Arc::new(move |label| {
            if label == "read local acquisition input" {
                observed.store(true, Ordering::SeqCst);
                wait.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            }
        })));
    let running = consumer.clone();
    let request = request(&stage, source, manifest(b"DATA"));
    let waiter = tokio::spawn(async move { ingest(&running, request).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    waiter.abort();
    let _ = waiter.await;
    assert!(!dropped.load(Ordering::SeqCst));
    let closing = consumer.clone();
    let mut shutdown = tokio::spawn(async move { closing.shutdown().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(40), &mut shutdown)
            .await
            .is_err()
    );
    assert!(!dropped.load(Ordering::SeqCst));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), shutdown)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    assert!(!stage.join("component.bin").exists());
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn local_ingestion_rechecks_copied_digest_after_input_changes() {
    let (temp, stage, service, consumer) = setup();
    let input = temp.path().join("input");
    std::fs::write(&input, b"DATA").unwrap();
    let source =
        AcquisitionLocalSource::new(std::fs::File::open(&input).unwrap(), Arc::new(())).unwrap();
    consumer
        .scope
        .set_blocking_observer(Some(Arc::new(move |label| {
            if label == "read local acquisition input" {
                std::fs::write(&input, b"EVIL").unwrap();
            }
        })));
    assert!(matches!(
        ingest(&consumer, request(&stage, source, manifest(b"DATA"))).await,
        Err(PumasError::HashMismatch { .. })
    ));
    assert!(!stage.join("component.bin").exists());
    let records = service.store.acquisitions().unwrap();
    assert!(matches!(
        records.values().next().unwrap().phase,
        AcquisitionPhase::Transferring
    ));
    assert!(records.values().next().unwrap().files.is_empty());
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn local_ingestion_preverification_budget_stops_new_reads_but_retains_pending_read() {
    let (temp, stage, service, consumer) = setup();
    let consumer = Arc::new(consumer);
    let input = temp.path().join("input");
    let payload = vec![7; 131072];
    std::fs::write(&input, &payload).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let source = AcquisitionLocalSource::new(
        std::fs::File::open(input).unwrap(),
        Arc::new(DropFlag(dropped.clone())),
    )
    .unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    let wait = Mutex::new(wait);
    let started = Arc::new(AtomicBool::new(false));
    let observed = started.clone();
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed_reads = reads.clone();
    consumer
        .scope
        .set_blocking_observer(Some(Arc::new(move |label| {
            if label == "verify local acquisition input" {
                observed_reads.fetch_add(1, Ordering::SeqCst);
                observed.store(true, Ordering::SeqCst);
                wait.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
            }
        })));
    let mut request = request(&stage, source, manifest(&payload));
    request.retry.elapsed = Duration::from_millis(100);
    let running = consumer.clone();
    let waiter = tokio::spawn(async move { ingest(&running, request).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(130)).await;
    assert!(!dropped.load(Ordering::SeqCst));
    let result = tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(PumasError::Validation { .. })));
    // A refusal reply does not claim the registered read has drained.
    assert!(!dropped.load(Ordering::SeqCst));
    let closing = consumer.clone();
    let mut shutdown = tokio::spawn(async move { closing.shutdown().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut shutdown)
            .await
            .is_err()
    );
    assert!(!dropped.load(Ordering::SeqCst));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), shutdown)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert!(dropped.load(Ordering::SeqCst));
    assert!(service.store.acquisitions().unwrap().is_empty());
    assert!(std::fs::read_dir(stage).unwrap().next().is_none());
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn local_ingestion_owner_count_unknown_size_and_unbounded_clock_refuse() {
    for mode in 0..5 {
        let (temp, stage, service, consumer) = setup();
        let input = temp.path().join("input");
        std::fs::write(&input, b"DATA").unwrap();
        let source =
            AcquisitionLocalSource::new(std::fs::File::open(input).unwrap(), Arc::new(())).unwrap();
        let mut request = request(&stage, source, manifest(b"DATA"));
        match mode {
            0 => request.demand.consumer = "other".into(),
            1 => request.sources.clear(),
            2 => request.retry.attempts = Some(0),
            3 => request.retry.elapsed = Duration::MAX,
            _ => {
                request.manifest = ArtifactManifest::new(
                    request.manifest.source().clone(),
                    vec![ArtifactFile::new(
                        "component.bin",
                        "selected-component",
                        None,
                        Some(
                            Sha256Evidence::new(
                                "fixture.sha256",
                                hex::encode(Sha256::digest(b"DATA")),
                            )
                            .unwrap(),
                        ),
                        FileVerificationRequirement::Sha256,
                    )
                    .unwrap()],
                )
                .unwrap()
            }
        }
        assert!(matches!(
            ingest(&consumer, request).await,
            Err(PumasError::Validation { .. })
        ));
        assert!(service.store.acquisitions().unwrap().is_empty());
        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
    let temp = tempfile::tempdir().unwrap();
    assert!(matches!(
        AcquisitionLocalSource::new(std::fs::File::open(temp.path()).unwrap(), Arc::new(())),
        Err(PumasError::Validation { .. })
    ));
}
