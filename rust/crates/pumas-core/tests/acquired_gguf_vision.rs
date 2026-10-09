//! Owned tiny format fixtures exercise import qualification, never inference.
use pumas_library::{
    acquisition::*,
    model_library::{AcquiredGgufVisionSpec, AcquiredGgufVisionTask, ModelImporter},
    models::{ImportState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, PumasError, Result,
};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

type Files = Vec<(String, Vec<u8>)>;

const PRIMARY: &str = "model.gguf";
const PROJECTOR: &str = "mmproj.gguf";
fn gguf(architecture: Option<&str>, padding: usize) -> Vec<u8> {
    let mut bytes = b"GGUF".to_vec();
    bytes.extend(3_u32.to_le_bytes());
    bytes.extend(0_u64.to_le_bytes());
    bytes.extend(u64::from(architecture.is_some()).to_le_bytes());
    if let Some(value) = architecture {
        let key = "general.architecture";
        bytes.extend((key.len() as u64).to_le_bytes());
        bytes.extend(key.as_bytes());
        bytes.extend(8_u32.to_le_bytes());
        bytes.extend((value.len() as u64).to_le_bytes());
        bytes.extend(value.as_bytes());
    }
    bytes.resize(bytes.len() + padding, 0);
    bytes
}
fn pair() -> Files {
    vec![
        (PRIMARY.into(), gguf(Some("llama"), 0)),
        (PROJECTOR.into(), gguf(Some("clip"), 4096)),
    ]
}
fn vision_spec(primary: &str) -> AcquiredGgufVisionSpec {
    AcquiredGgufVisionSpec {
        primary_model: spec(primary),
        vision_projector: PROJECTOR.into(),
        task: AcquiredGgufVisionTask::ImageToText,
    }
}
fn spec(primary: &str) -> ModelImportSpec {
    ModelImportSpec {
        path: primary.into(),
        family: "owned-fixture".into(),
        official_name: "acquired-safe-model".into(),
        model_type: None,
        repo_id: None,
        subtype: None,
        tags: None,
        security_acknowledged: None,
    }
}
async fn api(root: &Path) -> PumasApi {
    PumasApi::builder(root)
        .with_registry(
            pumas_library::registry::LibraryRegistry::open_at(&root.join("registry.db")).unwrap(),
        )
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}
#[derive(Default)]
struct Host {
    cancel: Arc<AtomicBool>,
    cancel_on_progress: bool,
}
#[async_trait::async_trait]
impl HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        std::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    async fn record_progress(&mut self, _: u64) -> Result<()> {
        if self.cancel_on_progress {
            self.cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    async fn retry(&mut self, _: u32, _: Option<Duration>, _: Option<&str>) -> Result<()> {
        Ok(())
    }
}

// Read requests by exact file index. The fixture has no repository/model network.
async fn source(files: Files) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(socket.read_u8().await.unwrap());
            }
            let text = String::from_utf8(request).unwrap();
            let index: usize = text
                .split_whitespace()
                .nth(1)
                .unwrap()
                .trim_start_matches('/')
                .parse()
                .unwrap();
            let bytes = &files[index].1;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            let _ = socket.write_all(bytes).await;
        }
    });
    (endpoint, task)
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    WrongSpec,
    ChangedBytes,
    ReplacedPath,
    Cancel,
    WrongProjector,
    Generic,
    ChangedProjector,
    ChangedPublishedProjector,
    MissingPublication,
    MissingReceipt,
    MissingAcquisition,
    Maintenance,
    NotReady,
    WrongModelId,
}

async fn import(files: Files, primary: &str, fault: Fault) -> (bool, Option<String>) {
    let root = tempfile::TempDir::new().unwrap();
    let staging = tempfile::TempDir::new().unwrap();
    let api = api(root.path()).await;
    let result = acquire(&api, staging.path(), files, primary, fault).await;
    let success = result.is_ok();
    let id = result.ok().and_then(|result| result.model_id);
    if !success {
        assert_eq!(api.model_library().model_dirs().count(), 0);
    }
    api.shutdown_instance().await.unwrap();
    (success, id)
}

async fn acquire(
    api: &PumasApi,
    stage: &Path,
    files: Files,
    primary: &str,
    fault: Fault,
) -> Result<pumas_library::models::ModelImportResult> {
    // Preserve supplied order to exercise order-independent explicit roles.
    let (endpoint, server) = source(files.clone()).await;
    let manifest = ArtifactManifest::new(
        ArtifactSourceIdentity::new(
            "owned-fixture",
            "source",
            ArtifactRevisionEvidence::new("fixture.version", "v1", RevisionStrength::Immutable)
                .unwrap(),
        )
        .unwrap(),
        files
            .iter()
            .map(|(path, bytes)| {
                ArtifactFile::new(
                    path,
                    path,
                    Some(bytes.len() as u64),
                    Some(
                        Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(bytes)))
                            .unwrap(),
                    ),
                    FileVerificationRequirement::Sha256,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    std::fs::create_dir(stage.join("stage")).unwrap();
    let workspace = AcquisitionWorkspace::from_reserved_directory(
        stage,
        Path::new("stage"),
        Arc::new(()),
        || Ok(()),
    )
    .unwrap();
    let consumer = api
        .acquisition()
        .open_consumer("model.bridge.fixture")
        .unwrap();
    let importer = ModelImporter::new(api.model_library().clone());
    let publish = importer.clone();
    let spec = vision_spec(primary);
    let prepare_spec = spec.clone();
    let publish_spec = spec.clone();
    let prepared = Arc::new(AtomicBool::new(false));
    let seen = prepared.clone();
    let primary_path = stage.join("stage").join(primary);
    let projector_path = stage.join("stage").join(PROJECTOR);
    let demand = AcquisitionDemand {
        consumer: "model.bridge.fixture".into(),
        operation: uuid::Uuid::new_v4().to_string(),
    };
    let result = consumer
        .acquire_http(
            AcquisitionHttpRequest {
                demand: demand.clone(),
                manifest: manifest.clone(),
                workspace,
                sources: (0..files.len())
                    .map(|index| AcquisitionHttpSource {
                        url: format!("{endpoint}/{index}"),
                        authorization: None,
                    })
                    .collect(),
                retry: AcquisitionRetryPolicy {
                    attempts: Some(1),
                    elapsed: Duration::ZERO,
                    backoff: RetryConfig::default(),
                },
            },
            reqwest::Client::new(),
            Box::new(Host {
                cancel_on_progress: matches!(fault, Fault::Cancel),
                ..Host::default()
            }),
            move |acquired| async move {
                seen.store(true, Ordering::SeqCst);
                for (index, file) in acquired.record().files.iter().enumerate() {
                    let mut bytes = Vec::new();
                    acquired.open_file(index).await?.read_to_end(&mut bytes)?;
                    assert_eq!(file.sha256, hex::encode(Sha256::digest(&bytes)));
                }
                match fault {
                    Fault::ChangedProjector => {
                        let mut bytes = std::fs::read(&projector_path)?;
                        *bytes.last_mut().unwrap() ^= 1;
                        std::fs::write(&projector_path, bytes)?;
                    }
                    Fault::ChangedBytes => {
                        let mut bytes = std::fs::read(&primary_path)?;
                        *bytes.last_mut().unwrap() ^= 1;
                        std::fs::write(&primary_path, bytes)?;
                    }
                    Fault::ReplacedPath => {
                        let bytes = std::fs::read(&primary_path)?;
                        std::fs::rename(&primary_path, primary_path.with_extension("held"))?;
                        let mut replacement = bytes;
                        *replacement.last_mut().unwrap() ^= 1;
                        std::fs::write(&primary_path, replacement)?;
                    }
                    _ => {}
                }
                let payload = if matches!(fault, Fault::Generic) {
                    serde_json::to_value(&prepare_spec.primary_model)?
                } else {
                    serde_json::to_value(&prepare_spec)?
                };
                Ok((acquired, payload))
            },
            move |acquired, receipt| async move {
                let mut bound = publish_spec;
                if matches!(fault, Fault::WrongSpec) {
                    bound.primary_model.official_name = "unbound".into();
                }
                if matches!(fault, Fault::WrongProjector) {
                    bound.vision_projector = "other.gguf".into();
                }
                if matches!(fault, Fault::Generic) {
                    return publish
                        .import_acquired_model(&acquired, &receipt, &bound.primary_model)
                        .await;
                }
                publish
                    .import_acquired_gguf_vision(&acquired, &receipt, &bound)
                    .await
            },
        )
        .await;
    server.abort();
    let _ = server.await;
    let record = api
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .find(|record| record.demand == demand)
        .unwrap();
    if matches!(fault, Fault::Cancel) {
        assert!(!prepared.load(Ordering::SeqCst));
        assert!(api
            .acquisition()
            .consumer_receipt(record.id)
            .unwrap()
            .is_none());
        assert!(matches!(result, Err(PumasError::DownloadCancelled)));
    } else {
        assert!(prepared.load(Ordering::SeqCst));
        assert_eq!(record.files.len(), files.len()); // Successful exact-byte acquisition.
        if let Ok(imported) = &result {
            assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
            let id = imported.model_id.as_ref().unwrap();
            let target = api.model_library().library_root().join(id);
            assert_eq!(
                api.model_library()
                    .get_effective_metadata(id)
                    .unwrap()
                    .unwrap()
                    .import_state,
                Some(ImportState::Ready)
            );
            for (path, expected) in &files {
                assert_eq!(std::fs::read(target.join(path)).unwrap(), *expected);
            }
            // Canonical persisted producer metadata retains primary hashes;
            // the index's effective projection omits these column-owned fields.
            let metadata: pumas_library::models::ModelMetadata =
                serde_json::from_slice(&std::fs::read(target.join("metadata.json")).unwrap())
                    .unwrap();
            let primary_bytes = &files.iter().find(|(path, _)| path == primary).unwrap().1;
            assert_eq!(
                metadata.hashes.as_ref().unwrap().sha256.as_deref(),
                Some(hex::encode(Sha256::digest(primary_bytes)).as_str())
            );
            assert_eq!(metadata.metadata_needs_review, Some(true));
            assert_eq!(
                metadata.task_classification_source.as_deref(),
                Some("explicit-acquired-vision-selection")
            );
            assert_eq!(metadata.task_classification_confidence, Some(0.0));
            assert_eq!(metadata.task_type_primary.as_deref(), Some("image-to-text"));
            assert_eq!(
                api.model_library().get_primary_model_file(id),
                Some(target.join(primary))
            );
            let descriptor = api
                .model_library()
                .resolve_model_execution_descriptor(id)
                .await
                .unwrap();
            assert_eq!(Path::new(&descriptor.entry_path), target.join(primary));
            if matches!(fault, Fault::Maintenance) {
                let scan = api
                    .model_library()
                    .deep_scan_rebuild(true, Some(|_| {}))
                    .await
                    .unwrap();
                assert_eq!(scan.hash_verified, 1, "{scan:?}");
                assert!(scan.hash_mismatches.is_empty(), "{scan:?}");
                assert!(scan.errors.is_empty(), "{scan:?}");
                api.model_library().redetect_model_type(id).await.unwrap();
                assert_eq!(
                    api.model_library().get_primary_model_file(id),
                    Some(target.join(primary))
                );
                let descriptor = api
                    .model_library()
                    .resolve_model_execution_descriptor(id)
                    .await
                    .unwrap();
                assert_eq!(Path::new(&descriptor.entry_path), target.join(primary));
                let metadata: pumas_library::models::ModelMetadata =
                    serde_json::from_slice(&std::fs::read(target.join("metadata.json")).unwrap())
                        .unwrap();
                assert_eq!(
                    metadata.hashes.as_ref().unwrap().sha256.as_deref(),
                    Some(hex::encode(Sha256::digest(primary_bytes)).as_str())
                );
            }
            let checked_id = id.clone();
            if matches!(fault, Fault::ChangedPublishedProjector) {
                let path = target.join(PROJECTOR);
                let mut bytes = std::fs::read(&path).unwrap();
                *bytes.last_mut().unwrap() ^= 1;
                std::fs::write(path, bytes).unwrap();
            }
            if matches!(
                fault,
                Fault::MissingPublication
                    | Fault::MissingReceipt
                    | Fault::MissingAcquisition
                    | Fault::NotReady
                    | Fault::WrongModelId
            ) {
                let path = target.join("metadata.json");
                let mut metadata: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                match fault {
                    Fault::MissingAcquisition => {
                        let receipt_path = target.join(".pumas_import_publication.json");
                        let mut receipt: serde_json::Value =
                            serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
                        receipt.as_object_mut().unwrap().remove("acquisition");
                        std::fs::write(receipt_path, serde_json::to_vec(&receipt).unwrap())
                            .unwrap();
                    }
                    Fault::MissingReceipt => {
                        std::fs::remove_file(target.join(".pumas_import_publication.json"))
                            .unwrap();
                    }
                    Fault::MissingPublication => {
                        metadata
                            .as_object_mut()
                            .unwrap()
                            .remove("import_publication");
                    }
                    Fault::NotReady => metadata["import_state"] = serde_json::json!("pending"),
                    Fault::WrongModelId => {
                        metadata["model_id"] = serde_json::json!("foreign/model/identity")
                    }
                    _ => unreachable!(),
                }
                std::fs::write(path, serde_json::to_vec(&metadata).unwrap()).unwrap();
                assert!(
                    api.model_library().get_primary_model_file(id).is_none(),
                    "invalid canonical proof fell back to a primary"
                );
                assert!(api
                    .model_library()
                    .resolve_model_execution_descriptor(id)
                    .await
                    .is_err());
            }
            let observed = consumer
                .reconcile(
                    demand,
                    manifest,
                    AcquisitionWorkspace::from_reserved_directory(
                        stage,
                        Path::new("stage"),
                        Arc::new(()),
                        || Ok(()),
                    )
                    .unwrap(),
                    move |receipt, acquired| async move {
                        importer
                            .reconcile_acquired_gguf_vision(&acquired, &receipt, &spec, &checked_id)
                            .await
                    },
                )
                .await;
            if matches!(
                fault,
                Fault::ChangedPublishedProjector
                    | Fault::MissingPublication
                    | Fault::MissingReceipt
                    | Fault::MissingAcquisition
                    | Fault::NotReady
                    | Fault::WrongModelId
            ) {
                assert!(observed.is_err(), "mutated projector reconciled");
            } else {
                assert!(observed.is_ok(), "{observed:?}");
            }
        } else {
            assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
        }
    }
    consumer.shutdown().await.unwrap();
    result
}

#[tokio::test]
async fn larger_projector_keeps_explicit_primary_and_reconciles_without_replay() {
    let files = pair();
    assert!(files[1].1.len() > files[0].1.len());
    assert!(import(files.clone(), PRIMARY, Fault::None).await.0);
    let mut reverse = files;
    reverse.reverse();
    assert!(import(reverse, PRIMARY, Fault::None).await.0);
}
#[tokio::test]
async fn roles_and_complete_pair_are_required_before_publication() {
    let mut cases = Vec::new();
    for missing in [PRIMARY, PROJECTOR] {
        let mut files = pair();
        files.retain(|(path, _)| path != missing);
        cases.push(files);
    }
    let mut extra = pair();
    extra.push(("extra.gguf".into(), gguf(Some("llama"), 0)));
    cases.push(extra);
    let mut misplaced = pair();
    misplaced[1].0 = "nested/mmproj.gguf".into();
    cases.push(misplaced);
    for (index, architecture) in [
        (0, None),
        (1, None),
        (0, Some("clip")),
        (1, Some("llama")),
        (0, Some("")),
    ] {
        let mut files = pair();
        files[index].1 = gguf(architecture, 0);
        cases.push(files);
    }
    let mut truncated_metadata = pair();
    truncated_metadata[1].1 = gguf(Some("clip"), 0);
    truncated_metadata[1].1[16..24].copy_from_slice(&2_u64.to_le_bytes());
    cases.push(truncated_metadata);
    // Identification-only readers used to accept malformed declarations after
    // an already observed architecture. Admission must consume the full header.
    let valid = gguf(Some("clip"), 0);
    let mut duplicate = valid.clone();
    duplicate[16..24].copy_from_slice(&2_u64.to_le_bytes());
    duplicate.extend_from_slice(&valid[24..]);
    let mut excessive_count = valid.clone();
    excessive_count[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut excessive_string = valid.clone();
    excessive_string[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut unknown_type = valid.clone();
    let value_type_offset = 24 + 8 + "general.architecture".len();
    unknown_type[value_type_offset..value_type_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut nested_array = valid.clone();
    nested_array[16..24].copy_from_slice(&2_u64.to_le_bytes());
    nested_array.extend(1_u64.to_le_bytes());
    nested_array.extend(b"x");
    nested_array.extend(9_u32.to_le_bytes()); // array value
    nested_array.extend(9_u32.to_le_bytes()); // nested array element type
    nested_array.extend(1_u64.to_le_bytes());
    nested_array.extend(4_u32.to_le_bytes());
    nested_array.extend(1_u64.to_le_bytes());
    nested_array.extend(0_u32.to_le_bytes());
    for bytes in [
        duplicate,
        excessive_count,
        excessive_string,
        unknown_type,
        nested_array,
    ] {
        let mut files = pair();
        files[1].1 = bytes;
        cases.push(files);
    }
    let mut malformed = pair();
    malformed[1].1 = b"GGUF".to_vec();
    cases.push(malformed);
    let mut version = pair();
    version[1].1[4..8].copy_from_slice(&2_u32.to_le_bytes());
    cases.push(version);
    for files in cases {
        assert!(!import(files, PRIMARY, Fault::None).await.0);
    }
}
#[tokio::test]
async fn full_selection_receipt_and_copied_bytes_are_bound() {
    for fault in [
        Fault::WrongSpec,
        Fault::WrongProjector,
        Fault::ChangedBytes,
        Fault::ChangedProjector,
        Fault::ReplacedPath,
        Fault::Cancel,
        Fault::Generic,
    ] {
        assert!(
            !import(pair(), PRIMARY, fault).await.0,
            "accepted {fault:?}"
        );
    }
}
#[tokio::test]
async fn changed_published_projector_cannot_reconcile() {
    assert!(
        import(pair(), PRIMARY, Fault::ChangedPublishedProjector)
            .await
            .0
    );
}
#[test]
fn selection_is_closed_and_task_is_not_caller_extensible() {
    let base = serde_json::to_value(vision_spec(PRIMARY)).unwrap();
    let mut unknown = base.clone();
    unknown["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<AcquiredGgufVisionSpec>(unknown).is_err());
    let mut task = base;
    task["task"] = serde_json::json!("chat_generation");
    assert!(serde_json::from_value::<AcquiredGgufVisionSpec>(task).is_err());
}

#[tokio::test]
async fn canonical_publication_identity_is_required_for_primary_selection() {
    for fault in [
        Fault::MissingPublication,
        Fault::MissingReceipt,
        Fault::MissingAcquisition,
        Fault::NotReady,
        Fault::WrongModelId,
    ] {
        assert!(import(pair(), PRIMARY, fault).await.0);
    }
}

#[tokio::test]
async fn deep_hash_scan_and_redetection_preserve_explicit_primary() {
    assert!(import(pair(), PRIMARY, Fault::Maintenance).await.0);
}
