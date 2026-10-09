//! Explicit authored sets, owned SDK loopback fixtures, structural import only.
#![cfg(feature = "s3")]
use pumas_library::{
    acquisition::*,
    models::{ImportState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, PumasError, S3ConditionalBundleModelImportRequest, S3ModelImportControl,
    S3ModelImportError,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{watch, Notify},
    task::{JoinHandle, JoinSet},
};

const PRIMARY: &str = "model-00001-of-00002.safetensors";
type Files = BTreeMap<String, Vec<u8>>;
fn tensor(name: &str) -> Vec<u8> {
    let mut header = serde_json::to_vec(
        &serde_json::json!({name:{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}),
    )
    .unwrap();
    while !header.len().is_multiple_of(8) {
        header.push(b' ');
    }
    [
        (header.len() as u64).to_le_bytes().as_slice(),
        header.as_slice(),
        1.0_f32.to_le_bytes().as_slice(),
    ]
    .concat()
}
fn package() -> Files {
    [
        ("config.json", serde_json::json!({"model_type":"llama","architectures":["LlamaForCausalLM"],"hidden_size":1})),
        ("tokenizer_config.json", serde_json::json!({"tokenizer_class":"PreTrainedTokenizerFast","unk_token":"[UNK]"})),
        ("tokenizer.json", serde_json::json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":{"[UNK]":0,"hello":1},"unk_token":"[UNK]"}})),
        ("model.safetensors.index.json", serde_json::json!({"weight_map":{"a":PRIMARY,"b":"model-00002-of-00002.safetensors"}})),
    ].into_iter().map(|(p,v)|(p.into(),serde_json::to_vec(&v).unwrap()))
        .chain([(PRIMARY.into(),tensor("a")),("model-00002-of-00002.safetensors".into(),tensor("b"))]).collect()
}
fn digest(bytes: &[u8]) -> Sha256Evidence {
    Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(bytes))).unwrap()
}
fn entries(files: &Files) -> Vec<S3ConditionalManifestEntry> {
    files
        .iter()
        .rev()
        .map(|(path, bytes)| S3ConditionalManifestEntry {
            source_key: format!("objects/{path}"),
            logical_path: path.clone(),
            expected_etag: "\"pin\"".into(),
            expected_size: bytes.len() as u64,
            expected_sha256: digest(bytes),
        })
        .collect()
}
#[derive(Clone, Debug)]
struct Call {
    method: String,
    key: String,
    range: Option<String>,
}
struct Source {
    endpoint: String,
    calls: Arc<Mutex<Vec<Call>>>,
    mode: Arc<AtomicU8>,
    held: Arc<Notify>,
    drained: Arc<Notify>,
    stop: watch::Sender<bool>,
    worker: JoinHandle<()>,
}
impl Source {
    async fn start(files: Files) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mode = Arc::new(AtomicU8::new(0));
        let held = Arc::new(Notify::new());
        let drained = Arc::new(Notify::new());
        let (stop, mut closing) = watch::channel(false);
        let (record, fault, started, finished) =
            (calls.clone(), mode.clone(), held.clone(), drained.clone());
        let files = Arc::new(files);
        let worker = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                tokio::select! { biased;
                    _=closing.changed()=>break,
                    accepted=listener.accept()=> {
                        let (socket,_)=accepted.unwrap();
                        let (files,record,fault,started,finished)=(files.clone(),record.clone(),fault.clone(),started.clone(),finished.clone());
                        tasks.spawn(async move {
                            tokio::time::timeout(Duration::from_secs(8),serve(socket,files,record,fault,started,finished)).await.unwrap();
                        });
                    }
                }
            }
            while let Some(result) = tasks.join_next().await {
                result.unwrap();
            }
        });
        Self {
            endpoint,
            calls,
            mode,
            held,
            drained,
            stop,
            worker,
        }
    }
    fn config(&self) -> S3ReaderConfig {
        S3ReaderConfig {
            endpoint: self.endpoint.clone(),
            region: "fixture".into(),
            bucket: "fixture".into(),
            addressing: S3Addressing::Path,
            allow_http: true,
            operation_timeout: Duration::from_secs(3),
        }
    }
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    async fn close(self) {
        self.stop.send_replace(true);
        tokio::time::timeout(Duration::from_secs(10), self.worker)
            .await
            .unwrap()
            .unwrap();
    }
}
async fn serve(
    mut socket: TcpStream,
    files: Arc<Files>,
    calls: Arc<Mutex<Vec<Call>>>,
    fault: Arc<AtomicU8>,
    held: Arc<Notify>,
    drained: Arc<Notify>,
) {
    let mut req = Vec::new();
    while !req.ends_with(b"\r\n\r\n") {
        assert!(req.len() < 16 * 1024);
        req.push(socket.read_u8().await.unwrap());
    }
    let req = String::from_utf8(req).unwrap();
    assert!(!req.contains("versionId="));
    let mut first = req.lines().next().unwrap().split_whitespace();
    let method = first.next().unwrap();
    let path = first.next().unwrap().split('?').next().unwrap();
    let key = path
        .splitn(3, '/')
        .nth(2)
        .unwrap()
        .strip_prefix("objects/")
        .unwrap();
    let head = method == "HEAD";
    let lower = req.to_ascii_lowercase();
    let range = lower
        .lines()
        .find_map(|line| line.strip_prefix("range: "))
        .map(str::to_owned);
    if !head {
        assert!(
            lower.contains("\r\nif-match: \"pin\"\r\n")
                || lower.contains("\r\nif-match: \"new\"\r\n")
        );
    }
    calls.lock().unwrap().push(Call {
        method: method.into(),
        key: key.into(),
        range: range.clone(),
    });
    let mode = if key == PRIMARY {
        fault.load(Ordering::SeqCst)
    } else {
        0
    };
    if mode == 9 && head {
        held.notify_one();
        let mut one = [0];
        assert_eq!(socket.read(&mut one).await.unwrap(), 0);
        drained.notify_one();
        return;
    }
    if mode == 1 && head {
        socket
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        return;
    }
    let mut bytes = files.get(key).unwrap().clone();
    if mode == 8 && !head {
        *bytes.last_mut().unwrap() ^= 1;
    }
    let start = range
        .as_ref()
        .map(|s| {
            s.strip_prefix("bytes=")
                .unwrap()
                .split('-')
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap()
        })
        .unwrap_or(0);
    if !head && !bytes.is_empty() {
        assert!(range.is_some());
    }
    if !head && bytes.is_empty() {
        assert!(range.is_none());
    }
    let size = bytes.len() + usize::from(head && (mode == 3 || mode == 12));
    let tag = if (head && mode == 2) || (!head && mode == 5) || mode == 11 {
        "\"new\""
    } else if head && mode == 4 {
        "W/\"pin\""
    } else {
        "\"pin\""
    };
    let version = if (!head && mode == 6) || (head && mode == 13) {
        "unexpected-v1"
    } else {
        "null"
    };
    let status = if head || bytes.is_empty() || mode == 7 {
        "200 OK"
    } else {
        "206 Partial Content"
    };
    let cr = if !head && !bytes.is_empty() {
        format!(
            "Content-Range: bytes {start}-{}/{}\r\n",
            bytes.len() - 1,
            bytes.len()
        )
    } else {
        String::new()
    };
    let header=format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nETag: {tag}\r\nx-amz-version-id: {version}\r\n{cr}Connection: close\r\n\r\n",size-start);
    socket.write_all(header.as_bytes()).await.unwrap();
    if !head {
        if mode == 10 {
            socket.write_all(&bytes[start..start + 4]).await.unwrap();
            held.notify_one();
            let mut one = [0];
            assert_eq!(socket.read(&mut one).await.unwrap(), 0);
            drained.notify_one();
        } else {
            let _ = socket.write_all(&bytes[start..]).await;
        }
    }
    let _ = socket.shutdown().await;
}
struct Rig {
    root: tempfile::TempDir,
    stage: tempfile::TempDir,
    api: PumasApi,
    workspace: AcquisitionWorkspace,
    id: uuid::Uuid,
}
impl Rig {
    async fn new() -> Self {
        let root = tempfile::TempDir::new().unwrap();
        let stage = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(stage.path().join("stage")).unwrap();
        let api = build(root.path()).await;
        let workspace = workspace(stage.path());
        Self {
            root,
            stage,
            api,
            workspace,
            id: uuid::Uuid::new_v4(),
        }
    }
    fn request(
        &self,
        source: &Source,
        entries: Vec<S3ConditionalManifestEntry>,
        primary: &str,
    ) -> S3ConditionalBundleModelImportRequest {
        S3ConditionalBundleModelImportRequest {
            operation_id: self.id,
            source: source.config(),
            credentials: None,
            entries,
            import: ModelImportSpec {
                path: primary.into(),
                family: "fixture".into(),
                official_name: "Conditional bundle".into(),
                model_type: Some("llm".into()),
                repo_id: None,
                subtype: None,
                tags: None,
                security_acknowledged: None,
            },
            workspace: self.workspace.clone(),
            retry: AcquisitionRetryPolicy {
                attempts: Some(1),
                elapsed: Duration::from_secs(5),
                backoff: RetryConfig::default(),
            },
        }
    }
    fn records(&self) -> BTreeMap<uuid::Uuid, AcquisitionRecord> {
        self.api.acquisition().store().acquisitions().unwrap()
    }
    fn no_model(&self) {
        assert!(self
            .api
            .model_library()
            .index()
            .list_all()
            .unwrap()
            .is_empty());
        fn contains_publication(root: &Path) -> bool {
            std::fs::read_dir(root).unwrap().any(|entry| {
                let entry = entry.unwrap();
                entry.file_name() == ".pumas_import_publication.json"
                    || (entry.file_type().unwrap().is_dir() && contains_publication(&entry.path()))
            })
        }
        assert!(!contains_publication(self.root.path()));
    }
}

#[tokio::test]
async fn cold_reopen_restarts_partial_at_zero_under_the_same_authored_record() {
    let files = package();
    let source = Source::start(files.clone()).await;
    source.mode.store(10, Ordering::SeqCst);
    let rig = Rig::new().await;
    let control = S3ModelImportControl::new();
    let mut progress = control.subscribe();
    let cancel = async {
        tokio::time::timeout(Duration::from_secs(5), source.held.notified())
            .await
            .unwrap();
        loop {
            if progress.borrow().downloaded_for_current_file >= 4 {
                break;
            }
            tokio::time::timeout(Duration::from_secs(5), progress.changed())
                .await
                .unwrap()
                .unwrap();
        }
        assert!(control.cancel());
    };
    let result = tokio::join!(
        rig.api.import_s3_conditional_bundle(
            rig.request(&source, entries(&files), PRIMARY),
            control.clone()
        ),
        cancel
    )
    .0;
    assert!(matches!(
        result,
        Err(S3ModelImportError::Operation(PumasError::DownloadCancelled))
    ));
    tokio::time::timeout(Duration::from_secs(5), source.drained.notified())
        .await
        .unwrap();
    let original = rig.records().into_values().next().unwrap();
    let Rig {
        root,
        stage,
        api,
        workspace: old_workspace,
        id,
    } = rig;
    api.shutdown_instance().await.unwrap();
    drop(api);
    drop(old_workspace);
    let api = build(root.path()).await;
    let workspace = workspace(stage.path());
    let rig = Rig {
        root,
        stage,
        api,
        workspace,
        id,
    };
    source.mode.store(0, Ordering::SeqCst);
    let set = entries(&files);
    let request = rig.request(&source, set.clone(), PRIMARY);
    let spec = serde_json::to_value(&request.import).unwrap();
    let start = source.calls().len();
    let result = rig
        .api
        .import_s3_conditional_bundle(request, S3ModelImportControl::new())
        .await
        .unwrap();
    assert_published(&rig, &files, &result.model_id.unwrap(), &set, spec);
    assert_eq!(rig.records().into_values().next().unwrap().id, original.id);
    assert!(source.calls()[start..].iter().any(|c| c.key == PRIMARY
        && c.range.as_deref() == Some(&format!("bytes=0-{}", files[PRIMARY].len() - 1))));
    rig.api.shutdown_instance().await.unwrap();
    source.close().await;
}
fn workspace(stage: &Path) -> AcquisitionWorkspace {
    AcquisitionWorkspace::from_reserved_directory(
        stage,
        Path::new("stage"),
        Arc::new(()),
        || Ok(()),
    )
    .unwrap()
}
async fn build(root: &Path) -> PumasApi {
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
fn assert_published(
    rig: &Rig,
    files: &Files,
    id: &str,
    entries: &[S3ConditionalManifestEntry],
    spec: serde_json::Value,
) {
    let records = rig.records();
    assert_eq!(records.len(), 1);
    let record = records.values().next().unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    assert_eq!(record.demand.operation, rig.id.to_string());
    assert_eq!(
        record.manifest.source().revision().strength(),
        RevisionStrength::Weak
    );
    assert_eq!(
        record.manifest.source().revision().authority(),
        "s3.explicit_conditional_objects"
    );
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
    let pins: Vec<_> = sorted
        .iter()
        .map(|e| (&e.source_key, &e.expected_etag, e.expected_size))
        .collect();
    assert_eq!(
        record.manifest.source().revision().value(),
        serde_json::to_string(&pins).unwrap()
    );
    assert_eq!(record.files.len(), files.len());
    for (declared, actual) in sorted.iter().zip(record.files.iter()) {
        assert_eq!(actual.path, declared.logical_path);
        assert_eq!(actual.sha256, declared.expected_sha256.value());
        assert_eq!(actual.bytes, declared.expected_size);
    }
    let receipt = rig
        .api
        .acquisition()
        .consumer_receipt(record.id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.verified_files, record.files);
    assert_eq!(receipt.manifest, record.manifest);
    assert_eq!(receipt.payload, spec);
    let library = rig.api.model_library();
    let target = library.library_root().join(id);
    for (path, bytes) in files {
        assert_eq!(std::fs::read(target.join(path)).unwrap(), *bytes);
    }
    let publication: serde_json::Value = serde_json::from_slice(
        &std::fs::read(target.join(".pumas_import_publication.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(publication["state"], "confirmed");
    assert_eq!(publication["model_id"], id);
    assert_eq!(
        publication["acquisition"],
        serde_json::to_value(receipt).unwrap()
    );
    assert_eq!(
        library
            .get_effective_metadata(id)
            .unwrap()
            .unwrap()
            .import_state,
        Some(ImportState::Ready)
    );
    assert_eq!(
        library.index().get(id).unwrap().unwrap().metadata["import_state"],
        "ready"
    );
}

#[tokio::test]
async fn complete_sharded_package_and_empty_gguf_auxiliary_publish_exact_authored_sets() {
    let gguf = [
        b"GGUF".as_slice(),
        3_u32.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
    ]
    .concat();
    for (files, primary) in [
        (package(), PRIMARY),
        (
            [("weights.gguf".into(), gguf), ("notes.txt".into(), vec![])]
                .into_iter()
                .collect(),
            "weights.gguf",
        ),
    ] {
        let source = Source::start(files.clone()).await;
        let rig = Rig::new().await;
        let entries = entries(&files);
        let request = rig.request(&source, entries.clone(), primary);
        let spec = serde_json::to_value(&request.import).unwrap();
        let result = rig
            .api
            .import_s3_conditional_bundle(request, S3ModelImportControl::new())
            .await
            .unwrap();
        assert_published(&rig, &files, &result.model_id.unwrap(), &entries, spec);
        let calls = source.calls();
        assert_eq!(calls.len(), files.len() * 2);
        assert!(calls[..files.len()].iter().all(|c| c.method == "HEAD"));
        if files.contains_key("notes.txt") {
            assert!(calls
                .iter()
                .any(|c| c.key == "notes.txt" && c.method == "GET" && c.range.is_none()));
        }
        rig.api.shutdown_instance().await.unwrap();
        source.close().await;
    }
}

#[tokio::test]
async fn entire_authored_manifest_preflight_has_no_head_or_acquisition_effects() {
    let files = package();
    let source = Source::start(files.clone()).await;
    let rig = Rig::new().await;
    for fault in 0..17 {
        let mut set = entries(&files);
        match fault {
            0 => set[0].source_key = "../escape".into(),
            1 => set[0].logical_path = "../escape".into(),
            2 => set[0].expected_etag = "W/\"pin\"".into(),
            3 => set[0].expected_size = i64::MAX as u64 + 1,
            4..=6 => {
                set[0].source_key = set[1].source_key.clone();
                set[0].expected_size = set[1].expected_size;
                set[0].expected_sha256 = set[1].expected_sha256.clone();
                match fault {
                    4 => set[0].expected_etag = "\"other\"".into(),
                    5 => set[0].expected_size += 1,
                    _ => set[0].expected_sha256 = digest(b"other"),
                }
            }
            7 => {
                for entry in &mut set {
                    entry.expected_size = i64::MAX as u64;
                }
            }
            8 => {
                for entry in &mut set {
                    entry.expected_etag = format!("\"{}\"", "x".repeat(4096));
                }
            }
            9 => set[0].logical_path = set[1].logical_path.clone(),
            10 => set[0].logical_path = format!("{}.part", set[1].logical_path),
            11 => set.clear(),
            12 => set.truncate(1),
            14 => {
                let original = set[0].clone();
                set = (0..33)
                    .map(|i| {
                        let mut e = original.clone();
                        e.logical_path = format!("entry-{i}.json");
                        e.source_key = format!("objects/entry-{i}");
                        e
                    })
                    .collect();
            }
            15 => set[0].expected_etag = "unquoted".into(),
            16 => set[0].expected_etag = "\"embedded\"quote\"".into(),
            13 => {
                set[0].logical_path = "MODEL.json/leaf".into();
                set[1].logical_path = "model.json".into();
            }
            _ => unreachable!(),
        }
        let reader = S3Reader::new(source.config()).unwrap();
        assert!(
            reader.validate_conditional_manifest_entries(&set).is_err(),
            "preflight fault {fault}"
        );
        assert!(
            reader
                .select_conditional_manifest(set.clone())
                .await
                .is_err(),
            "selection fault {fault}"
        );
        assert!(rig
            .api
            .import_s3_conditional_bundle(
                rig.request(&source, set, PRIMARY),
                S3ModelImportControl::new()
            )
            .await
            .is_err());
        assert!(source.calls().is_empty());
        assert!(rig.records().is_empty());
    }
    for (reserved, primary) in [
        ("metadata.json", PRIMARY),
        ("overrides.json/leaf", PRIMARY),
        ("safe.json", "missing.safetensors"),
    ] {
        let mut set = entries(&files);
        set[0].logical_path = reserved.into();
        assert!(rig
            .api
            .import_s3_conditional_bundle(
                rig.request(&source, set, primary),
                S3ModelImportControl::new()
            )
            .await
            .is_err());
        assert!(source.calls().is_empty());
        assert!(rig.records().is_empty());
    }
    rig.api.shutdown_instance().await.unwrap();
    source.close().await;
}

#[tokio::test]
async fn missing_or_inconsistent_members_never_shorten_selection_or_publish() {
    for mode in [1, 2, 3, 4, 5, 6, 7, 8, 13, 14] {
        let files = package();
        let source = Source::start(files.clone()).await;
        source
            .mode
            .store(if mode == 14 { 0 } else { mode }, Ordering::SeqCst);
        let rig = Rig::new().await;
        let mut set = entries(&files);
        if mode == 14 {
            set.iter_mut()
                .find(|e| e.logical_path == "tokenizer_config.json")
                .unwrap()
                .expected_sha256 = Sha256Evidence::new("fixture.sha256", "0".repeat(64)).unwrap();
        }
        assert!(rig
            .api
            .import_s3_conditional_bundle(
                rig.request(&source, set, PRIMARY),
                S3ModelImportControl::new()
            )
            .await
            .is_err());
        rig.no_model();
        if mode <= 4 || mode == 13 {
            assert!(rig.records().is_empty());
            assert!(source.calls().iter().all(|c| c.method == "HEAD"));
        } else {
            assert!(rig.records().values().all(|r| rig
                .api
                .acquisition()
                .consumer_receipt(r.id)
                .unwrap()
                .is_none()));
        }
        rig.api.shutdown_instance().await.unwrap();
        source.close().await;
    }
}

#[tokio::test]
async fn valid_transfers_with_incomplete_or_malformed_packages_retain_receipts_without_models() {
    for missing in [
        "config.json",
        "tokenizer_config.json",
        "tokenizer.json",
        "model.safetensors.index.json",
        "model-00002-of-00002.safetensors",
        "malformed-config",
        "malformed-shard",
        "wrong-index",
        "custom-code",
    ] {
        let mut files = package();
        match missing {
            "malformed-config" => {
                files.insert("config.json".into(), b"not JSON".to_vec());
            }
            "malformed-shard" => {
                files.insert(PRIMARY.into(), b"not safetensors".to_vec());
            }
            "custom-code" => {
                files.insert(
                    "custom.py".into(),
                    b"raise RuntimeError(\"must never execute\")\n".to_vec(),
                );
            }
            "wrong-index" => {
                files.insert("model.safetensors.index.json".into(),serde_json::to_vec(&serde_json::json!({"weight_map":{"absent":PRIMARY,"b":"model-00002-of-00002.safetensors"}})).unwrap());
            }
            _ => {
                files.remove(missing);
            }
        }
        let source = Source::start(files.clone()).await;
        let rig = Rig::new().await;
        assert!(rig
            .api
            .import_s3_conditional_bundle(
                rig.request(&source, entries(&files), PRIMARY),
                S3ModelImportControl::new()
            )
            .await
            .is_err());
        rig.no_model();
        let records = rig.records();
        assert_eq!(records.len(), 1);
        let record = records.values().next().unwrap();
        assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
        assert_eq!(record.files.len(), files.len());
        let receipt = rig
            .api
            .acquisition()
            .consumer_receipt(record.id)
            .unwrap()
            .unwrap();
        assert_eq!(receipt.verified_files, record.files);
        for file in &record.files {
            assert_eq!(file.sha256, digest(&files[&file.path]).value());
        }
        rig.api.shutdown_instance().await.unwrap();
        source.close().await;
    }
}

#[tokio::test]
async fn cancellation_and_exact_manifest_replay_preserve_retained_custody_and_refuse_changes() {
    let files = package();
    let source = Source::start(files.clone()).await;
    source.mode.store(9, Ordering::SeqCst);
    let rig = Rig::new().await;
    let control = S3ModelImportControl::new();
    let cancel = async {
        tokio::time::timeout(Duration::from_secs(5), source.held.notified())
            .await
            .unwrap();
        assert!(control.cancel());
    };
    let result = tokio::join!(
        rig.api.import_s3_conditional_bundle(
            rig.request(&source, entries(&files), PRIMARY),
            control.clone()
        ),
        cancel
    )
    .0;
    assert!(matches!(
        result,
        Err(S3ModelImportError::Operation(PumasError::DownloadCancelled))
    ));
    tokio::time::timeout(Duration::from_secs(5), source.drained.notified())
        .await
        .unwrap();
    assert!(rig.records().is_empty());
    source.mode.store(10, Ordering::SeqCst);
    let control = S3ModelImportControl::new();
    let mut progress = control.subscribe();
    let cancel = async {
        tokio::time::timeout(Duration::from_secs(5), source.held.notified())
            .await
            .unwrap();
        loop {
            if progress.borrow().downloaded_for_current_file >= 4 {
                break;
            }
            tokio::time::timeout(Duration::from_secs(5), progress.changed())
                .await
                .unwrap()
                .unwrap();
        }
        assert!(control.cancel());
    };
    let result = tokio::join!(
        rig.api.import_s3_conditional_bundle(
            rig.request(&source, entries(&files), PRIMARY),
            control.clone()
        ),
        cancel
    )
    .0;
    assert!(matches!(
        result,
        Err(S3ModelImportError::Operation(PumasError::DownloadCancelled))
    ));
    tokio::time::timeout(Duration::from_secs(5), source.drained.notified())
        .await
        .unwrap();
    let before = rig.records();
    assert_eq!(before.len(), 1);
    let record = before.values().next().unwrap();
    assert!(rig
        .api
        .acquisition()
        .consumer_receipt(record.id)
        .unwrap()
        .is_none());
    let partial = rig
        .stage
        .path()
        .join("stage")
        .join(format!("{PRIMARY}.part"));
    let prefix = std::fs::read(&partial).unwrap();
    assert_eq!(prefix.len(), 4);
    for change in 0..4 {
        let mut set = entries(&files);
        source.mode.store(
            if change == 0 {
                11
            } else if change == 1 {
                12
            } else {
                0
            },
            Ordering::SeqCst,
        );
        let target = set.iter_mut().find(|e| e.logical_path == PRIMARY).unwrap();
        match change {
            0 => target.expected_etag = "\"new\"".into(),
            1 => target.expected_size += 1,
            2 => target.expected_sha256 = digest(b"other"),
            _ => {}
        }
        let mut request = rig.request(&source, set, PRIMARY);
        if change == 3 {
            request.source.bucket = "other".into();
        }
        let start = source.calls().len();
        assert!(rig
            .api
            .import_s3_conditional_bundle(request, S3ModelImportControl::new())
            .await
            .is_err());
        assert!(source.calls()[start..].iter().all(|c| c.method == "HEAD"));
        assert_eq!(rig.records(), before);
        assert_eq!(std::fs::read(&partial).unwrap(), prefix);
        rig.no_model();
    }
    let other = Source::start(files.clone()).await;
    let mut request = rig.request(&other, entries(&files), PRIMARY);
    request.source.endpoint = other.endpoint.clone();
    assert!(rig
        .api
        .import_s3_conditional_bundle(request, S3ModelImportControl::new())
        .await
        .is_err());
    assert!(other.calls().iter().all(|c| c.method == "HEAD"));
    assert_eq!(rig.records(), before);
    assert_eq!(std::fs::read(&partial).unwrap(), prefix);
    other.close().await;
    source.mode.store(0, Ordering::SeqCst);
    let set = entries(&files);
    let request = rig.request(&source, set.clone(), PRIMARY);
    let spec = serde_json::to_value(&request.import).unwrap();
    let start = source.calls().len();
    let result = rig
        .api
        .import_s3_conditional_bundle(request, S3ModelImportControl::new())
        .await
        .unwrap();
    assert_published(&rig, &files, &result.model_id.unwrap(), &set, spec);
    assert_eq!(
        rig.records().keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>()
    );
    assert!(source.calls()[start..].iter().any(|c| c.key == PRIMARY
        && c.range.as_deref() == Some(&format!("bytes=0-{}", files[PRIMARY].len() - 1))));
    rig.api.shutdown_instance().await.unwrap();
    source.close().await;
}

struct PauseHost {
    mode: AtomicU8,
    file: usize,
    pause: bool,
}
#[async_trait::async_trait]
impl HttpAttemptHost for PauseHost {
    async fn pause_requested(&self) {
        if self.mode.load(Ordering::SeqCst) != 0 {
            return;
        }
        futures::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        self.mode.load(Ordering::SeqCst) == 1
    }
    fn cancel_requested(&self) -> bool {
        false
    }
    async fn record_progress(&mut self, bytes: u64) -> pumas_library::Result<()> {
        if self.pause && self.file == 1 && bytes >= 4 {
            self.mode.store(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for PauseHost {
    fn file_started(&mut self, index: usize) {
        self.file = index;
    }
    async fn retry(
        &mut self,
        _: u32,
        _: Option<Duration>,
        _: Option<&str>,
    ) -> pumas_library::Result<()> {
        Ok(())
    }
}
#[tokio::test]
async fn warm_pause_reuses_only_the_same_live_manifest_and_checked_prefix() {
    let files = package();
    let source = Source::start(files.clone()).await;
    source.mode.store(10, Ordering::SeqCst);
    let state = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(stage.path().join("stage")).unwrap();
    let workspace = workspace(stage.path());
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state.path(),
    ))));
    let consumer = service
        .open_consumer("fixture.authored-conditional")
        .unwrap();
    let reader = S3Reader::new(source.config()).unwrap();
    let selection = reader
        .select_conditional_manifest(entries(&files))
        .await
        .unwrap();
    let manifest = selection.manifest().clone();
    let demand = AcquisitionDemand {
        consumer: "fixture.authored-conditional".into(),
        operation: uuid::Uuid::new_v4().to_string(),
    };
    let request = |selection| AcquisitionS3ManifestRequest {
        demand: demand.clone(),
        selection,
        workspace: workspace.clone(),
        retry: AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(5),
            backoff: RetryConfig::default(),
        },
    };
    let result = consumer
        .acquire_s3_manifest(
            request(selection.clone()),
            Box::new(PauseHost {
                mode: AtomicU8::new(0),
                file: 0,
                pause: true,
            }),
            |_| async {
                panic!("partial set reached consumer");
                #[allow(unreachable_code)]
                Ok(((), serde_json::Value::Null))
            },
            |(), _| async { Ok(()) },
        )
        .await;
    assert!(matches!(result, Err(PumasError::DownloadPaused)));
    tokio::time::timeout(Duration::from_secs(5), source.drained.notified())
        .await
        .unwrap();
    let original = service
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert!(service.consumer_receipt(original.id).unwrap().is_none());
    source.mode.store(0, Ordering::SeqCst);
    let start = source.calls().len();
    consumer
        .acquire_s3_manifest(
            request(selection),
            Box::new(PauseHost {
                mode: AtomicU8::new(0),
                file: 0,
                pause: false,
            }),
            |acquired| async move {
                assert_eq!(acquired.record().files.len(), 6);
                Ok(((), serde_json::Value::Null))
            },
            |(), _| async { Ok(()) },
        )
        .await
        .unwrap();
    let record = service
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert_eq!(record.id, original.id);
    assert_eq!(record.manifest, manifest);
    assert!(source.calls()[start..].iter().any(|c| c.key == PRIMARY
        && c.range.as_deref() == Some(&format!("bytes=4-{}", files[PRIMARY].len() - 1))));
    assert_eq!(
        service
            .consumer_receipt(record.id)
            .unwrap()
            .unwrap()
            .verified_files,
        record.files
    );
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    source.close().await;
}
