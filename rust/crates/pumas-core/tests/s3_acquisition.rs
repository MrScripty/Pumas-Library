//! Credential-free loopback protocol -> canonical acquisition -> real importer.
#![cfg(feature = "s3")]

use pumas_library::{
    acquisition::{
        AcquisitionDemand, AcquisitionHost, AcquisitionPhase, AcquisitionRetryPolicy,
        AcquisitionS3Request, AcquisitionService, AcquisitionStore, AcquisitionWorkspace,
        HttpAttemptHost, S3Addressing, S3ObjectSelection, S3Reader, S3ReaderConfig, Sha256Evidence,
    },
    model_library::{ModelImporter, PumasReadOnlyLibrary},
    models::{ImportState, ModelArtifactState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, PumasError, Result,
};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const VERSION: &str = "selected-v1";
const LOGICAL: &str = "weights.gguf";

#[path = "s3_acquisition/empty.rs"]
mod empty;
#[path = "s3_acquisition/manifest.rs"]
mod manifest;

fn gguf() -> Vec<u8> {
    [
        b"GGUF".as_slice(),
        &3_u32.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
    ]
    .concat()
}

fn head(size: usize) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {VERSION}\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n").into_bytes()
}

fn range(version: &str, start: usize, size: usize, body: &[u8]) -> Vec<u8> {
    let end = size - 1;
    let mut response = format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{size}\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {version}\r\nETag: \"selected\"\r\nConnection: close\r\n\r\n", size-start).into_bytes();
    response.extend(body);
    response
}

async fn request(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        assert!(bytes.len() < 16 * 1024);
        bytes.push(socket.read_u8().await.unwrap());
    }
    String::from_utf8(bytes).unwrap()
}

struct Fixture {
    endpoint: String,
    task: JoinHandle<Vec<String>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Fixture {
    async fn unfinished(size: usize) -> (Self, tokio::sync::oneshot::Receiver<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (started, waiting) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let first = request(&mut socket).await;
            socket.write_all(&head(size)).await.unwrap();
            socket.shutdown().await.unwrap();
            let (mut socket, _) = listener.accept().await.unwrap();
            let second = request(&mut socket).await;
            socket
                .write_all(&range(VERSION, 0, size, &[]))
                .await
                .unwrap();
            started.send(()).unwrap();
            let mut byte = [0];
            assert_eq!(
                socket.read(&mut byte).await.unwrap(),
                0,
                "the unfinished response must close before terminal drain"
            );
            vec![first, second]
        });
        (
            Self {
                endpoint,
                task,
                stop: None,
            },
            waiting,
        )
    }

    async fn serve(responses: Vec<Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                requests.push(request(&mut socket).await);
                socket.write_all(&response).await.unwrap();
                socket.shutdown().await.unwrap();
            }
            loop {
                tokio::select! {
                    _=&mut stopped => break,
                    socket=listener.accept() => {
                        let (mut socket,_)=socket.unwrap(); requests.push(request(&mut socket).await);
                        socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                    }
                }
            }
            requests
        });
        Self {
            endpoint,
            task,
            stop: Some(stop),
        }
    }
    async fn finish(mut self) -> Vec<String> {
        if let Some(stop) = self.stop.take() {
            stop.send(()).unwrap();
        }
        tokio::time::timeout(Duration::from_secs(10), &mut self.task)
            .await
            .unwrap()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Default)]
struct Control {
    mode: AtomicU8,
    progress: AtomicU64,
    changed: tokio::sync::Notify,
}
struct Host {
    control: Arc<Control>,
    stop_at: Option<(u64, u8)>,
}
impl Host {
    fn quiet() -> Self {
        Self {
            control: Arc::new(Control::default()),
            stop_at: None,
        }
    }
}
#[async_trait::async_trait]
impl HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        loop {
            let changed = self.control.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.control.mode.load(Ordering::SeqCst) != 0 {
                return;
            }
            changed.await;
        }
    }
    fn pause_requested_now(&self) -> bool {
        self.control.mode.load(Ordering::SeqCst) == 1
    }
    fn cancel_requested(&self) -> bool {
        self.control.mode.load(Ordering::SeqCst) == 2
    }
    async fn record_progress(&mut self, bytes: u64) -> Result<()> {
        self.control.progress.store(bytes, Ordering::SeqCst);
        if let Some((at, mode)) = self.stop_at {
            if bytes >= at {
                self.control.mode.store(mode, Ordering::SeqCst);
                self.control.changed.notify_waiters();
            }
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    async fn retry(
        &mut self,
        _attempt: u32,
        _delay: Option<Duration>,
        _error: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
}

fn retry(attempts: u32) -> AcquisitionRetryPolicy {
    AcquisitionRetryPolicy {
        attempts: Some(attempts),
        elapsed: Duration::from_secs(10),
        backoff: RetryConfig::new()
            .with_base_delay(Duration::ZERO)
            .with_jitter(false),
    }
}
fn workspace(root: &Path) -> AcquisitionWorkspace {
    std::fs::create_dir(root.join("stage")).unwrap();
    reopen_workspace(root)
}
fn reopen_workspace(root: &Path) -> AcquisitionWorkspace {
    AcquisitionWorkspace::from_reserved_directory(root, Path::new("stage"), Arc::new(()), || Ok(()))
        .unwrap()
}
async fn selection(endpoint: &str, bytes: &[u8]) -> S3ObjectSelection {
    S3Reader::new(S3ReaderConfig {
        endpoint: endpoint.into(),
        region: "fixture-region".into(),
        bucket: "fixture-bucket".into(),
        addressing: S3Addressing::Path,
        allow_http: true,
        operation_timeout: Duration::from_secs(5),
    })
    .unwrap()
    .select(
        "models/model.gguf",
        VERSION,
        LOGICAL,
        Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(bytes))).unwrap(),
    )
    .await
    .unwrap()
}
fn acquire_request(
    selection: S3ObjectSelection,
    workspace: AcquisitionWorkspace,
    attempts: u32,
) -> AcquisitionS3Request {
    AcquisitionS3Request {
        demand: AcquisitionDemand {
            consumer: "model.s3".into(),
            operation: "selected-import".into(),
        },
        selection,
        workspace,
        retry: retry(attempts),
    }
}
fn spec(path: &str) -> ModelImportSpec {
    ModelImportSpec {
        path: path.into(),
        family: "fixture".into(),
        official_name: "S3 GGUF".into(),
        model_type: Some("llm".into()),
        repo_id: None,
        subtype: None,
        tags: None,
        security_acknowledged: None,
    }
}
async fn api(root: &Path) -> PumasApi {
    PumasApi::builder(root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}
async fn close(api: &PumasApi) {
    api.shutdown_intent().await.unwrap();
    api.shutdown_downloads().await.unwrap();
    api.shutdown_acquisition().await.unwrap();
}

#[tokio::test]
async fn s3_protocol_through_shared_custody_imports_a_ready_indexed_model() {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let bytes = gguf();
    let fixture = Fixture::serve(vec![
        head(bytes.len()),
        range(VERSION, 0, bytes.len(), &bytes),
    ])
    .await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let manifest = selected.manifest().clone();
    let consumer = api.acquisition().open_consumer("model.s3").unwrap();
    let importer = ModelImporter::new(api.model_library().clone());
    let result = consumer
        .acquire_s3(
            acquire_request(selected, workspace(stage.path()), 2),
            Box::new(Host::quiet()),
            |acquired| async move {
                assert!(matches!(
                    acquired.record().phase,
                    AcquisitionPhase::Using { .. }
                ));
                let payload = serde_json::to_value(spec(LOGICAL))?;
                Ok((acquired, payload))
            },
            move |acquired, receipt| async move {
                let result = importer
                    .import_acquired_gguf(&acquired, &receipt, &spec(LOGICAL))
                    .await?;
                assert!(result.success, "{:?}", result.error);
                Ok(result)
            },
        )
        .await
        .unwrap();
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 2);
    assert!(requests[1]
        .starts_with("GET /fixture-bucket/models/model.gguf?versionId=selected-v1 HTTP/1.1"));
    assert!(requests[1]
        .to_ascii_lowercase()
        .contains("\r\nif-match: \"selected\"\r\n"));
    let id = result.model_id.unwrap();
    let library = api.model_library();
    let model = library.library_root().join(&id);
    assert_eq!(std::fs::read(model.join(LOGICAL)).unwrap(), bytes);
    let metadata = library.get_effective_metadata(&id).unwrap().unwrap();
    assert_eq!(metadata.import_state, Some(ImportState::Ready));
    assert!(metadata.import_publication.unwrap().confirmed);
    let reader = PumasReadOnlyLibrary::open(library.library_root()).unwrap();
    let snapshot = reader
        .model_library_selector_snapshot(Default::default())
        .unwrap();
    assert_eq!(
        snapshot
            .rows
            .iter()
            .find(|row| row.model_id == id)
            .unwrap()
            .artifact_state,
        ModelArtifactState::Ready
    );
    let record = api
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    assert_eq!(record.manifest, manifest);
    let receipt = consumer.completion_receipt(&record).unwrap().unwrap();
    assert_eq!(receipt.verified_files, record.files);
    assert!(!stage.path().join("stage/weights.gguf.part").exists());
    consumer.shutdown().await.unwrap();
    close(&api).await;
    let reopened = AcquisitionStore::new(&root.path().join("launcher-data"));
    assert_eq!(
        reopened.acquisitions().unwrap().get(&record.id),
        Some(&record)
    );
}

// Interrupt only acquisition acknowledgement after the real model producer
// confirms Ready. The listener remains active through recovery to observe replay.
async fn cold_publication_reconciliation(fault: &str) {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let first = api(root.path()).await;
    let bytes = gguf();
    let fixture = Fixture::serve(vec![
        head(bytes.len()),
        range(VERSION, 0, bytes.len(), &bytes),
    ])
    .await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let manifest = selected.manifest().clone();
    let consumer = first.acquisition().open_consumer("model.s3").unwrap();
    let request = acquire_request(selected, workspace(stage.path()), 2);
    let demand = request.demand.clone();
    let importer = ModelImporter::new(first.model_library().clone());
    let (published, observed) = tokio::sync::oneshot::channel();
    let result: Result<()> = consumer
        .acquire_s3(
            request,
            Box::new(Host::quiet()),
            |acquired| async { Ok((acquired, serde_json::to_value(spec(LOGICAL))?)) },
            move |acquired, receipt| async move {
                let model = importer
                    .import_acquired_gguf(&acquired, &receipt, &spec(LOGICAL))
                    .await?;
                published.send(model).unwrap();
                Err(PumasError::Validation {
                    field: "fixture.after_confirmed_publication".into(),
                    message: "Acknowledgement interrupted".into(),
                })
            },
        )
        .await;
    assert!(
        matches!(&result, Err(PumasError::Validation { field, .. }) if field == "fixture.after_confirmed_publication"),
        "{fault}: acquisition/import failed before the intended interruption: {result:?}"
    );
    let model = observed
        .await
        .expect("confirmed publication callback must report its model");
    consumer.shutdown().await.unwrap();
    close(&first).await;
    let model_id = model.model_id.unwrap();
    let target = first.model_library().library_root().join(&model_id);
    let record = first
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
    let issued = consumer.completion_receipt(&record).unwrap().unwrap();
    let receipt_path = target.join(".pumas_import_publication.json");
    let receipt_bytes = std::fs::read(&receipt_path).unwrap();
    let mut output_receipt: serde_json::Value = serde_json::from_slice(&receipt_bytes).unwrap();
    assert_eq!(output_receipt["version"], 2);
    assert_eq!(output_receipt["state"], "confirmed");
    assert_eq!(
        output_receipt["acquisition"],
        serde_json::to_value(&issued).unwrap()
    );
    let metadata_path = target.join("metadata.json");
    let metadata_bytes = std::fs::read(&metadata_path).unwrap();
    let payload_path = target.join(LOGICAL);
    assert_eq!(std::fs::read(&payload_path).unwrap(), bytes);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&metadata_bytes).unwrap()["import_state"],
        "ready"
    );
    let publication_identity = serde_json::from_slice::<serde_json::Value>(&metadata_bytes)
        .unwrap()["import_publication"]
        .clone();
    drop(consumer);
    drop(first);

    if fault.contains("identity") {
        if !fault.starts_with("index-") {
            let mut metadata: serde_json::Value = serde_json::from_slice(&metadata_bytes).unwrap();
            if fault.starts_with("null-") {
                metadata["import_publication"] = serde_json::Value::Null;
            } else {
                metadata
                    .as_object_mut()
                    .unwrap()
                    .remove("import_publication");
            }
            std::fs::write(
                &metadata_path,
                serde_json::to_vec_pretty(&metadata).unwrap(),
            )
            .unwrap();
        }
        if fault.ends_with("missing") {
            std::fs::remove_file(&payload_path).unwrap();
        } else if !fault.ends_with("intact") {
            let mut changed = bytes.clone();
            changed[5] ^= 1;
            std::fs::write(&payload_path, changed).unwrap();
        }
    }
    match fault {
        "none" => {}
        "bytes" => {
            let mut changed = bytes.clone();
            changed[5] ^= 1;
            std::fs::write(&payload_path, changed).unwrap();
        }
        "lease" => {
            output_receipt["acquisition"]["use_lease"] = uuid::Uuid::new_v4().to_string().into()
        }
        "pending" => output_receipt["state"] = "pending".into(),
        "metadata" => std::fs::remove_file(&metadata_path).unwrap(),
        "legacy" => {
            output_receipt["version"] = 1.into();
            output_receipt
                .as_object_mut()
                .unwrap()
                .remove("acquisition");
        }
        "future" => output_receipt["version"] = 99.into(),
        "unbound" => {
            output_receipt
                .as_object_mut()
                .unwrap()
                .remove("acquisition");
        }
        _ if fault.contains("identity") => {}
        _ => panic!("unknown fixture fault"),
    }
    if !matches!(fault, "none" | "bytes" | "metadata") && !fault.contains("identity") {
        std::fs::write(
            &receipt_path,
            serde_json::to_vec_pretty(&output_receipt).unwrap(),
        )
        .unwrap();
    }
    // Fresh owner/root capabilities, with no original use handle or task scope.
    let cold = api(root.path()).await;
    if fault.contains("identity") {
        // Set the exact disposable cold index projection under test. Startup may
        // already have reprojected missing canonical identity as a legacy row.
        let mut indexed = cold
            .model_library()
            .index()
            .get(&model_id)
            .unwrap()
            .unwrap();
        if fault.starts_with("canonical-") {
            indexed.metadata["import_publication"] = publication_identity;
        } else if fault.starts_with("null-") || fault.starts_with("index-null-") {
            indexed.metadata["import_publication"] = serde_json::Value::Null;
        } else {
            indexed
                .metadata
                .as_object_mut()
                .unwrap()
                .remove("import_publication");
        }
        cold.model_library().index().upsert(&indexed).unwrap();
        let canonical: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
        let explicit_identity = |value: &serde_json::Value| {
            value
                .get("import_publication")
                .is_some_and(|identity| !identity.is_null())
        };
        assert_eq!(explicit_identity(&canonical), fault.starts_with("index-"));
        assert_eq!(
            explicit_identity(&indexed.metadata),
            fault.starts_with("canonical-")
        );
    }
    let consumer = cold.acquisition().open_consumer("model.s3").unwrap();
    let store_path = root.path().join("launcher-data/downloads.json");
    let store_before = std::fs::read(&store_path).unwrap();
    let output_before = std::fs::read(&receipt_path).unwrap();
    let payload_before = std::fs::read(&payload_path).ok();
    let metadata_before = std::fs::read(&metadata_path).ok();
    let index_before = cold
        .model_library()
        .index()
        .get(&model_id)
        .unwrap()
        .map(|row| row.metadata);
    let source_before = std::fs::read(stage.path().join("stage").join(LOGICAL)).unwrap();
    let importer = ModelImporter::new(cold.model_library().clone());
    let checked_id = model_id.clone();
    let reconciled = consumer
        .reconcile(
            demand.clone(),
            manifest.clone(),
            reopen_workspace(stage.path()),
            move |receipt, acquired| async move {
                importer
                    .reconcile_acquired_gguf(&acquired, &receipt, &spec(LOGICAL), &checked_id)
                    .await
            },
        )
        .await;
    if fault != "none" {
        consumer.shutdown().await.unwrap();
        close(&cold).await;
        let requests = fixture.finish().await;
        assert!(reconciled.is_err(), "{fault} unexpectedly settled");
        if fault.contains("identity") {
            assert!(
                matches!(reconciled, Err(PumasError::Validation { ref field, .. })
                if field == "import.acquired_recovery_required")
            );
            assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
        }
        assert_eq!(
            std::fs::read(&store_path).unwrap(),
            store_before,
            "{fault} mutated acquisition state"
        );
        assert_eq!(
            std::fs::read(&receipt_path).unwrap(),
            output_before,
            "{fault} rewrote model receipt"
        );
        assert_eq!(
            std::fs::read(&payload_path).ok(),
            payload_before,
            "{fault} modified model payload"
        );
        assert_eq!(
            cold.acquisition()
                .store()
                .acquisitions()
                .unwrap()
                .get(&record.id),
            Some(&record)
        );
        assert_eq!(std::fs::read(&metadata_path).ok(), metadata_before);
        assert_eq!(
            cold.model_library()
                .index()
                .get(&model_id)
                .unwrap()
                .map(|row| row.metadata),
            index_before
        );
        assert_eq!(
            std::fs::read(stage.path().join("stage").join(LOGICAL)).unwrap(),
            source_before
        );
        assert_eq!(requests.len(), 2, "cold refusal replayed the source");
        return;
    } else {
        assert_eq!(
            reconciled.unwrap().unwrap().model_id.as_deref(),
            Some(model_id.as_str())
        );
    }
    let importer = ModelImporter::new(cold.model_library().clone());
    let checked_id = model_id.clone();
    let settled = consumer
        .reconcile(
            demand.clone(),
            manifest.clone(),
            reopen_workspace(stage.path()),
            move |receipt, acquired| async move {
                importer
                    .reconcile_acquired_gguf(&acquired, &receipt, &spec(LOGICAL), &checked_id)
                    .await
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(settled.model_id.as_deref(), Some(model_id.as_str()));
    let adopted = cold
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .remove(&record.id)
        .unwrap();
    assert!(matches!(adopted.phase, AcquisitionPhase::Adopted { .. }));
    assert_eq!(consumer.completion_receipt(&adopted).unwrap(), Some(issued));
    assert_eq!(std::fs::read(&receipt_path).unwrap(), receipt_bytes);
    assert_eq!(std::fs::read(&metadata_path).unwrap(), metadata_bytes);
    assert_eq!(std::fs::read(&payload_path).unwrap(), bytes);
    assert_eq!(
        std::fs::read(stage.path().join("stage").join(LOGICAL)).unwrap(),
        source_before
    );
    let reader = PumasReadOnlyLibrary::open(cold.model_library().library_root()).unwrap();
    assert_eq!(
        reader
            .model_library_selector_snapshot(Default::default())
            .unwrap()
            .rows
            .iter()
            .find(|row| row.model_id == model_id)
            .unwrap()
            .artifact_state,
        ModelArtifactState::Ready
    );
    drop(reader);
    consumer.shutdown().await.unwrap();
    close(&cold).await;
    assert_eq!(
        fixture.finish().await.len(),
        2,
        "cold reconciliation replayed the source"
    );
}

#[tokio::test]
async fn confirmed_model_publication_cold_reconciles_and_repeats_without_source_replay() {
    cold_publication_reconciliation("none").await;
}
#[tokio::test]
async fn changed_model_output_cold_reconciliation_preserves_custody() {
    cold_publication_reconciliation("bytes").await;
}
#[tokio::test]
async fn another_use_generation_cannot_settle_confirmed_model_output() {
    cold_publication_reconciliation("lease").await;
}
#[tokio::test]
async fn pending_model_publication_is_not_promoted_by_cold_reconciliation() {
    cold_publication_reconciliation("pending").await;
}
#[tokio::test]
async fn missing_canonical_metadata_cannot_be_replaced_by_index_or_backup() {
    cold_publication_reconciliation("metadata").await;
}
#[tokio::test]
async fn legacy_unbound_model_receipt_retains_acquisition_uncertainty() {
    cold_publication_reconciliation("legacy").await;
}
#[tokio::test]
async fn future_model_receipt_version_is_refused_without_mutation() {
    cold_publication_reconciliation("future").await;
}
#[tokio::test]
async fn current_model_receipt_without_binding_is_refused_without_mutation() {
    cold_publication_reconciliation("unbound").await;
}

#[tokio::test]
async fn missing_publication_identities_cannot_settle_changed_or_missing_output() {
    for fault in ["identity-bytes", "identity-missing", "null-identity-bytes"] {
        cold_publication_reconciliation(fault).await;
    }
}

#[tokio::test]
async fn missing_publication_identities_cannot_settle_intact_output() {
    for fault in ["identity-intact", "null-identity-intact"] {
        cold_publication_reconciliation(fault).await;
    }
}

#[tokio::test]
async fn missing_index_publication_identity_cannot_settle_changed_output() {
    for fault in ["index-identity-bytes", "index-null-identity-bytes"] {
        cold_publication_reconciliation(fault).await;
    }
}

#[tokio::test]
async fn missing_canonical_publication_identity_cannot_be_replaced_by_index() {
    cold_publication_reconciliation("canonical-identity-bytes").await;
}

#[tokio::test]
async fn changed_version_and_wrong_digest_never_reach_model_import() {
    for changed in [true, false] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let api = api(root.path()).await;
        let bytes = gguf();
        let mut wrong = bytes.clone();
        wrong[5] ^= 1;
        let response = if changed {
            range("other-version", 0, bytes.len(), &bytes)
        } else {
            range(VERSION, 0, bytes.len(), &wrong)
        };
        let fixture = Fixture::serve(vec![head(bytes.len()), response]).await;
        let selected = selection(&fixture.endpoint, &bytes).await;
        let consumer = api.acquisition().open_consumer("model.s3").unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let observe = called.clone();
        let result = consumer
            .acquire_s3(
                acquire_request(selected, workspace(stage.path()), 2),
                Box::new(Host::quiet()),
                move |_| async move {
                    observe.store(true, Ordering::SeqCst);
                    Ok(((), serde_json::Value::Null))
                },
                |(), _| async { Ok(()) },
            )
            .await;
        if changed {
            assert!(matches!(result, Err(PumasError::Validation { .. })));
        } else {
            assert!(matches!(result, Err(PumasError::HashMismatch { .. })));
        }
        assert!(!called.load(Ordering::SeqCst));
        assert!(api.model_library().index().list_all().unwrap().is_empty());
        let record = api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        assert_eq!(record.phase, AcquisitionPhase::Transferring);
        assert!(record.files.is_empty());
        assert!(consumer.completion_receipt(&record).is_err());
        assert!(!stage.path().join("stage/weights.gguf").exists());
        assert_eq!(fixture.finish().await.len(), 2);
        consumer.shutdown().await.unwrap();
        close(&api).await;
    }
}

#[tokio::test]
async fn pause_resumes_only_the_same_live_checked_prefix_and_selected_version() {
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let bytes = gguf();
    let fixture = Fixture::serve(vec![
        head(bytes.len()),
        range(VERSION, 0, bytes.len(), &bytes[..4]),
        range(VERSION, 4, bytes.len(), &bytes[4..]),
    ])
    .await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let workspace = workspace(stage.path());
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state.path(),
    ))));
    let consumer = service.open_consumer("model.s3").unwrap();
    let control = Arc::new(Control::default());
    let result = consumer
        .acquire_s3(
            acquire_request(selected.clone(), workspace.clone(), 2),
            Box::new(Host {
                control: control.clone(),
                stop_at: Some((4, 1)),
            }),
            |_| async {
                panic!("paused input reached consumer");
                #[allow(unreachable_code)]
                Ok(((), serde_json::Value::Null))
            },
            |(), _| async { Ok(()) },
        )
        .await;
    assert!(matches!(result, Err(PumasError::DownloadPaused)));
    assert_eq!(control.progress.load(Ordering::SeqCst), 4);
    assert_eq!(
        std::fs::read(stage.path().join("stage/weights.gguf.part")).unwrap(),
        bytes[..4]
    );
    consumer
        .acquire_s3(
            acquire_request(selected, workspace, 2),
            Box::new(Host::quiet()),
            |acquired| async move {
                assert_eq!(acquired.record().files[0].bytes, 24);
                Ok(((), serde_json::Value::Null))
            },
            |(), _| async { Ok(()) },
        )
        .await
        .unwrap();
    let requests = fixture.finish().await;
    assert_eq!(requests.len(), 3);
    assert!(requests[2]
        .to_ascii_lowercase()
        .contains("\r\nrange: bytes=4-23\r\n"));
    assert!(requests[2].contains("versionId=selected-v1"));
    assert_eq!(
        std::fs::read(stage.path().join("stage/weights.gguf")).unwrap(),
        bytes
    );
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancellation_after_prefix_has_no_verified_handoff_or_later_writes() {
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let bytes = gguf();
    let fixture = Fixture::serve(vec![
        head(bytes.len()),
        range(VERSION, 0, bytes.len(), &bytes[..4]),
    ])
    .await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state.path(),
    ))));
    let consumer = service.open_consumer("model.s3").unwrap();
    let control = Arc::new(Control::default());
    let result = consumer
        .acquire_s3(
            acquire_request(selected, workspace(stage.path()), 2),
            Box::new(Host {
                control,
                stop_at: Some((4, 2)),
            }),
            |_| async {
                panic!("cancelled input reached consumer");
                #[allow(unreachable_code)]
                Ok(((), serde_json::Value::Null))
            },
            |(), _| async { Ok(()) },
        )
        .await;
    assert!(matches!(result, Err(PumasError::DownloadCancelled)));
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    assert_eq!(fixture.finish().await.len(), 2);
    assert_eq!(
        std::fs::read(stage.path().join("stage/weights.gguf.part")).unwrap(),
        bytes[..4]
    );
    assert!(!stage.path().join("stage/weights.gguf").exists());
    let record = service
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert_eq!(record.phase, AcquisitionPhase::Transferring);
    assert!(record.files.is_empty());
}

#[tokio::test]
async fn shared_retry_budget_counts_requests_without_sdk_retry_multiplication() {
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let bytes = gguf();
    let failure =
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_vec();
    let fixture = Fixture::serve(vec![head(bytes.len()), failure.clone(), failure]).await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state.path(),
    ))));
    let consumer = service.open_consumer("model.s3").unwrap();
    let result = consumer
        .acquire_s3(
            acquire_request(selected, workspace(stage.path()), 2),
            Box::new(Host::quiet()),
            |_| async { Ok(((), serde_json::Value::Null)) },
            |(), _| async { Ok(()) },
        )
        .await;
    assert!(matches!(result, Err(PumasError::DownloadFailed { .. })));
    assert_eq!(fixture.finish().await.len(), 3);
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn unbounded_retry_configuration_is_refused_before_durable_admission() {
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let bytes = gguf();
    let fixture = Fixture::serve(vec![head(bytes.len())]).await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let workspace = workspace(stage.path());
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state.path(),
    ))));
    let consumer = service.open_consumer("model.s3").unwrap();
    for policy in [
        (None, Duration::from_secs(1)),
        (Some(0), Duration::from_secs(1)),
        (Some(1), Duration::ZERO),
        (Some(1), Duration::MAX),
    ] {
        let mut req = acquire_request(selected.clone(), workspace.clone(), 1);
        req.retry.attempts = policy.0;
        req.retry.elapsed = policy.1;
        assert!(matches!(
            consumer
                .acquire_s3(
                    req,
                    Box::new(Host::quiet()),
                    |_| async { Ok(((), serde_json::Value::Null)) },
                    |(), _| async { Ok(()) }
                )
                .await,
            Err(PumasError::Validation { .. })
        ));
    }
    assert!(service.store().acquisitions().unwrap().is_empty());
    assert_eq!(fixture.finish().await.len(), 1);
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn elapsed_budget_closes_unfinished_body_without_starting_another_attempt() {
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let bytes = gguf();
    let (fixture, started) = Fixture::unfinished(bytes.len()).await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state.path(),
    ))));
    let consumer = service.open_consumer("model.s3").unwrap();
    let mut request = acquire_request(selected, workspace(stage.path()), 10);
    request.retry.elapsed = Duration::from_secs(1);
    let mut acquire = Box::pin(consumer.acquire_s3(
        request,
        Box::new(Host::quiet()),
        |_| async { Ok(((), serde_json::Value::Null)) },
        |(), _| async { Ok(()) },
    ));
    tokio::select! { result=&mut acquire=>panic!("unfinished source returned: {result:?}"), result=started=>result.unwrap() }
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::time::resume();
    assert!(matches!(
        acquire.await,
        Err(PumasError::DownloadFailed { .. })
    ));
    assert_eq!(fixture.finish().await.len(), 2);
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    assert!(!stage.path().join("stage/weights.gguf").exists());
}

#[tokio::test]
async fn dropping_acquisition_waiter_is_joined_before_source_and_workspace_release() {
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let bytes = gguf();
    let (fixture, started) = Fixture::unfinished(bytes.len()).await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state.path(),
    ))));
    let consumer = service.open_consumer("model.s3").unwrap();
    let mut acquire = Box::pin(consumer.acquire_s3(
        acquire_request(selected, workspace(stage.path()), 2),
        Box::new(Host::quiet()),
        |_| async { Ok(((), serde_json::Value::Null)) },
        |(), _| async { Ok(()) },
    ));
    tokio::select! {result=&mut acquire=>panic!("unfinished source returned: {result:?}"),result=started=>result.unwrap()}
    assert!(futures::poll!(&mut acquire).is_pending());
    drop(acquire);
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    assert_eq!(fixture.finish().await.len(), 2);
    assert!(!stage.path().join("stage/weights.gguf").exists());
    let record = service
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert_eq!(record.phase, AcquisitionPhase::Transferring);
    assert!(record.files.is_empty());
}

#[tokio::test]
async fn unissued_model_receipt_cannot_authorize_publication() {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let bytes = gguf();
    let fixture = Fixture::serve(vec![
        head(bytes.len()),
        range(VERSION, 0, bytes.len(), &bytes),
    ])
    .await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let consumer = api.acquisition().open_consumer("model.s3").unwrap();
    let importer = ModelImporter::new(api.model_library().clone());
    consumer.acquire_s3(acquire_request(selected,workspace(stage.path()),1),Box::new(Host::quiet()),
        move |acquired|async move {
            let record=acquired.record();let payload=serde_json::to_value(spec(LOGICAL))?;
            let lease=match record.phase {AcquisitionPhase::Using{lease}=>lease,_=>panic!("missing lease")};
            let unissued=pumas_library::acquisition::AcquisitionConsumerReceipt {
                receipt_kind:"pumas.consumer-completion".into(),receipt_version:1,owner:record.demand.consumer.clone(),
                acquisition_id:record.id.to_string(),use_lease:lease.to_string(),demand:record.demand.clone(),
                manifest:record.manifest.clone(),workspace:record.workspace.clone(),verified_files:record.files.clone(),payload:payload.clone(),
            };
            let result=importer.import_acquired_gguf(&acquired,&unissued,&spec(LOGICAL)).await;
            assert!(matches!(result,Err(PumasError::Validation{ref message,..})if message.contains("current issued consumer receipt")));
            Ok(((),payload))
        },|(),_|async{Ok(())}).await.unwrap();
    assert!(api.model_library().index().list_all().unwrap().is_empty());
    assert_eq!(fixture.finish().await.len(), 2);
    consumer.shutdown().await.unwrap();
    close(&api).await;
}

#[tokio::test]
async fn acquired_import_refuses_path_substitution_and_non_gguf_content() {
    for bad_path in [true, false] {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        let api = api(root.path()).await;
        let bytes = if bad_path { gguf() } else { vec![b'x'; 24] };
        let fixture = Fixture::serve(vec![
            head(bytes.len()),
            range(VERSION, 0, bytes.len(), &bytes),
        ])
        .await;
        let selected = selection(&fixture.endpoint, &bytes).await;
        let consumer = api.acquisition().open_consumer("model.s3").unwrap();
        let importer = ModelImporter::new(api.model_library().clone());
        let result = consumer
            .acquire_s3(
                acquire_request(selected, workspace(stage.path()), 1),
                Box::new(Host::quiet()),
                move |acquired| async move {
                    let spec = spec(if bad_path { "other.gguf" } else { LOGICAL });
                    let payload = serde_json::to_value(&spec)?;
                    Ok(((acquired, spec), payload))
                },
                move |(acquired, spec), receipt| async move {
                    let result = importer
                        .import_acquired_gguf(&acquired, &receipt, &spec)
                        .await?;
                    Ok(result)
                },
            )
            .await;
        assert!(
            matches!(result,Err(PumasError::Validation{ref field,..})if field=="import.acquired")
        );
        assert!(api.model_library().index().list_all().unwrap().is_empty());
        assert_eq!(fixture.finish().await.len(), 2);
        let record = api
            .acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
        assert!(consumer.completion_receipt(&record).unwrap().is_some());
        consumer.shutdown().await.unwrap();
        close(&api).await;
    }
}

#[tokio::test]
async fn ordinary_copied_import_still_uses_the_directory_source_path() {
    let root = tempfile::TempDir::new().unwrap();
    let source = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let path = source.path().join(LOGICAL);
    let bytes = gguf();
    std::fs::write(&path, &bytes).unwrap();
    let result = ModelImporter::new(api.model_library().clone())
        .import(&spec(path.to_str().unwrap()))
        .await
        .unwrap();
    assert!(result.success);
    let id = result.model_id.unwrap();
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            api.model_library()
                .library_root()
                .join(&id)
                .join(".pumas_import_publication.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["version"], 1);
    assert!(receipt.get("acquisition").is_none());
    assert_eq!(
        std::fs::read(api.model_library().library_root().join(&id).join(LOGICAL)).unwrap(),
        bytes
    );
    assert_eq!(
        api.model_library()
            .get_effective_metadata(&id)
            .unwrap()
            .unwrap()
            .import_state,
        Some(ImportState::Ready)
    );
    let target = api.model_library().library_root().join(&id);
    close(&api).await;
    // A pre-protocol ordinary model has no publication identity or receipt.
    // Acquired settlement is strict; this separate legacy read contract stays.
    let metadata_path = target.join("metadata.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
    metadata
        .as_object_mut()
        .unwrap()
        .remove("import_publication");
    std::fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(target.join(".pumas_import_publication.json")).unwrap();
    let mut legacy = api.model_library().index().get(&id).unwrap().unwrap();
    legacy
        .metadata
        .as_object_mut()
        .unwrap()
        .remove("import_publication");
    api.model_library().index().upsert(&legacy).unwrap();
    drop(api);
    let cold = self::api(root.path()).await;
    let metadata = cold
        .model_library()
        .get_effective_metadata(&id)
        .unwrap()
        .unwrap();
    let reader = PumasReadOnlyLibrary::open(cold.model_library().library_root()).unwrap();
    let snapshot = reader
        .model_library_selector_snapshot(Default::default())
        .unwrap();
    drop(reader);
    close(&cold).await;
    assert!(metadata.import_publication.is_none());
    assert_eq!(
        snapshot
            .rows
            .iter()
            .find(|row| row.model_id == id)
            .unwrap()
            .artifact_state,
        ModelArtifactState::Ready
    );
    assert_eq!(std::fs::read(target.join(LOGICAL)).unwrap(), bytes);
}

#[tokio::test]
async fn model_collision_retains_consumer_use_and_never_adopts_a_refused_import() {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let source = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let bytes = gguf();
    let path = source.path().join(LOGICAL);
    std::fs::write(&path, &bytes).unwrap();
    let importer = ModelImporter::new(api.model_library().clone());
    let initial = importer
        .import(&spec(path.to_str().unwrap()))
        .await
        .unwrap();
    assert!(initial.success);
    let id = initial.model_id.unwrap();
    let model = api.model_library().library_root().join(&id);
    let metadata = std::fs::read(model.join("metadata.json")).unwrap();
    let fixture = Fixture::serve(vec![
        head(bytes.len()),
        range(VERSION, 0, bytes.len(), &bytes),
    ])
    .await;
    let selected = selection(&fixture.endpoint, &bytes).await;
    let consumer = api.acquisition().open_consumer("model.s3").unwrap();
    let result = consumer
        .acquire_s3(
            acquire_request(selected, workspace(stage.path()), 1),
            Box::new(Host::quiet()),
            |acquired| async move { Ok((acquired, serde_json::to_value(spec(LOGICAL))?)) },
            move |acquired, receipt| async move {
                importer
                    .import_acquired_gguf(&acquired, &receipt, &spec(LOGICAL))
                    .await
            },
        )
        .await;
    assert!(matches!(result, Err(PumasError::ImportFailed { .. })));
    assert_eq!(fixture.finish().await.len(), 2);
    assert_eq!(api.model_library().index().list_all().unwrap().len(), 1);
    assert_eq!(
        std::fs::read(model.join("metadata.json")).unwrap(),
        metadata
    );
    let record = api
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
    assert!(consumer.completion_receipt(&record).unwrap().is_some());
    consumer.shutdown().await.unwrap();
    close(&api).await;
}

#[path = "s3_acquisition/bounds.rs"]
mod bounds;
