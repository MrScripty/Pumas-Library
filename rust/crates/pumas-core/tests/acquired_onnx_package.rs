//! Owned tiny format fixtures exercise import qualification, never inference.
use pumas_library::{
    acquisition::*,
    model_library::ModelImporter,
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

// Genuine ONNX ModelProto bytes, built from the official v1.16.2 schema.
// The successful corpus is additionally checked by the official Python checker.
fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    while value >= 128 {
        bytes.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
    bytes
}
fn number(tag: u64, value: u64) -> Vec<u8> {
    [varint(tag << 3), varint(value)].concat()
}
fn field(tag: u64, bytes: &[u8]) -> Vec<u8> {
    [
        varint((tag << 3) | 2),
        varint(bytes.len() as u64),
        bytes.to_vec(),
    ]
    .concat()
}
fn text(tag: u64, value: &str) -> Vec<u8> {
    field(tag, value.as_bytes())
}
fn value(name: &str, dims: &[u64]) -> Vec<u8> {
    let shape: Vec<u8> = dims
        .iter()
        .flat_map(|dim| field(1, &number(1, *dim)))
        .collect();
    let tensor = [number(1, 1), field(2, &shape)].concat();
    [text(1, name), field(2, &field(1, &tensor))].concat()
}
fn entry(key: &str, value: &str) -> Vec<u8> {
    field(13, &[text(1, key), text(2, value)].concat())
}
fn weight(
    name: &str,
    location: Option<&str>,
    offset: Option<&str>,
    length: Option<&str>,
) -> Vec<u8> {
    let mut tensor = [field(1, &varint(1)), number(2, 1), text(8, name)].concat();
    if let Some(location) = location {
        tensor.extend(number(14, 1));
        tensor.extend(entry("location", location));
        if let Some(offset) = offset {
            tensor.extend(entry("offset", offset));
        }
        if let Some(length) = length {
            tensor.extend(entry("length", length));
        }
    } else {
        tensor.extend(field(9, &1.0_f32.to_le_bytes()));
    }
    tensor
}
fn node(op: &str, inputs: &[&str], output: &str) -> Vec<u8> {
    [
        inputs.iter().flat_map(|input| text(1, input)).collect(),
        text(2, output),
        text(4, op),
    ]
    .concat()
}
fn graph(tensors: Vec<Vec<u8>>, nodes: Vec<Vec<u8>>) -> Vec<u8> {
    [
        nodes.iter().flat_map(|bytes| field(1, bytes)).collect(),
        text(2, "owned-dense-graph"),
        tensors.iter().flat_map(|bytes| field(5, bytes)).collect(),
        field(11, &value("input", &[1])),
        field(12, &value("output", &[1])),
    ]
    .concat()
}
fn model(graph: &[u8]) -> Vec<u8> {
    [
        number(1, 8),
        text(2, "owned-fixture"),
        field(7, graph),
        field(8, &number(2, 13)),
    ]
    .concat()
}
fn external(location: &str, offset: Option<&str>, length: Option<&str>) -> Vec<u8> {
    model(&graph(
        vec![weight("weight", Some(location), offset, length)],
        vec![node("Add", &["input", "weight"], "output")],
    ))
}
fn embedded() -> Vec<u8> {
    model(&graph(
        vec![weight("weight", None, None, None)],
        vec![
            node("Add", &["input", "weight"], "sum"),
            node("Relu", &["sum"], "output"),
        ],
    ))
}
fn package(location: &str, offset: Option<&str>, length: Option<&str>) -> Files {
    vec![
        ("model.onnx".into(), external(location, offset, length)),
        ("weights.data".into(), 1.0_f32.to_le_bytes().to_vec()),
    ]
}

fn spec(primary: &str) -> ModelImportSpec {
    ModelImportSpec {
        path: primary.into(),
        family: "owned-fixture".into(),
        official_name: "acquired-onnx-model".into(),
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
    mut files: Files,
    primary: &str,
    fault: Fault,
) -> Result<pumas_library::models::ModelImportResult> {
    files.sort_by(|a, b| a.0.cmp(&b.0));
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
    let spec = spec(primary);
    let prepare_spec = spec.clone();
    let publish_spec = spec.clone();
    let prepared = Arc::new(AtomicBool::new(false));
    let seen = prepared.clone();
    let primary_path = stage.join("stage").join(primary);
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
                    Fault::ChangedBytes => {
                        let mut bytes = std::fs::read(&primary_path)?;
                        // Preserve a valid graph while substituting actual tensor
                        // data. The verified descriptor handoff must refuse it.
                        let offset = bytes
                            .windows(4)
                            .position(|value| value == 1.0_f32.to_le_bytes())
                            .unwrap();
                        bytes[offset] ^= 1;
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
                Ok((acquired, serde_json::to_value(prepare_spec)?))
            },
            move |acquired, receipt| async move {
                let mut bound = publish_spec;
                if matches!(fault, Fault::WrongSpec) {
                    bound.official_name = "unbound".into();
                }
                publish
                    .import_acquired_model(&acquired, &receipt, &bound)
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
            assert_eq!(
                imported.security_tier,
                Some(pumas_library::models::SecurityTier::Safe)
            );
            let id = imported.model_id.as_ref().unwrap();
            let target = api.model_library().library_root().join(id);
            let metadata = api
                .model_library()
                .get_effective_metadata(id)
                .unwrap()
                .unwrap();
            assert_eq!(metadata.model_type.as_deref(), Some("unknown"));
            assert_eq!(metadata.task_type_primary.as_deref(), Some("unknown"));
            assert!(metadata.compatible_apps.as_ref().is_none_or(Vec::is_empty));
            // Preserve the existing format-derived hint; it is not admission.
            assert_eq!(
                metadata.recommended_backend.as_deref(),
                Some("onnx-runtime")
            );
            assert_eq!(metadata.metadata_needs_review, Some(true));
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
            let checked_id = id.clone();
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
                            .reconcile_acquired_model(&acquired, &receipt, &spec, &checked_id)
                            .await
                    },
                )
                .await;
            assert!(observed.is_ok(), "{observed:?}");
        } else {
            assert!(matches!(record.phase, AcquisitionPhase::Using { .. }));
        }
    }
    consumer.shutdown().await.unwrap();
    result
}

#[tokio::test]
async fn embedded_standard_graph_is_safe_receipt_bound_registration() {
    let (success, id) = import(
        vec![("model.onnx".into(), embedded())],
        "model.onnx",
        Fault::None,
    )
    .await;
    assert!(success);
    assert!(id.unwrap().starts_with("unknown/")); // No inferred embedding/runtime compatibility.
}

#[tokio::test]
async fn identity_and_rank_two_matmul_complete_the_bounded_operator_class() {
    let identity = model(&graph(vec![], vec![node("Identity", &["input"], "output")]));
    assert!(
        import(
            vec![("model.onnx".into(), identity)],
            "model.onnx",
            Fault::None
        )
        .await
        .0
    );
    let tensor = [
        field(1, &[2, 2]),
        number(2, 1),
        text(8, "weight"),
        field(9, &[0; 16]),
    ]
    .concat();
    let graph = [
        field(1, &node("MatMul", &["input", "weight"], "output")),
        text(2, "dense-matrix"),
        field(5, &tensor),
        field(11, &value("input", &[1, 2])),
        field(12, &value("output", &[1, 2])),
    ]
    .concat();
    assert!(
        import(
            vec![("model.onnx".into(), model(&graph))],
            "model.onnx",
            Fault::None
        )
        .await
        .0
    );
}

#[tokio::test]
async fn relative_external_data_preserves_nested_layout_and_exact_package() {
    for (offset, length) in [(None, None), (Some("0"), Some("4"))] {
        let files = vec![
            (
                "onnx/model.onnx".into(),
                external("data/weights.data", offset, length),
            ),
            (
                "onnx/data/weights.data".into(),
                1.0_f32.to_le_bytes().to_vec(),
            ),
        ];
        assert!(import(files, "onnx/model.onnx", Fault::None).await.0);
    }
}

#[tokio::test]
async fn nested_single_embedded_graph_does_not_claim_unpreserved_layout() {
    assert!(
        !import(
            vec![("onnx/model.onnx".into(), embedded())],
            "onnx/model.onnx",
            Fault::None
        )
        .await
        .0
    );
}

#[tokio::test]
async fn two_tensors_share_bounded_ranges_in_one_blob() {
    let model = model(&graph(
        vec![
            weight("a", Some("weights.data"), Some("4"), Some("4")),
            weight("b", Some("weights.data"), Some("8"), Some("4")),
        ],
        vec![
            node("Add", &["input", "a"], "sum"),
            node("Add", &["sum", "b"], "output"),
        ],
    ));
    let mut data = vec![0; 4];
    data.extend(1.0_f32.to_le_bytes());
    data.extend(2.0_f32.to_le_bytes());
    assert!(
        import(
            vec![("model.onnx".into(), model), ("weights.data".into(), data)],
            "model.onnx",
            Fault::None
        )
        .await
        .0
    );
}

#[tokio::test]
async fn missing_truncated_overflowing_and_mismatched_tensor_ranges_refuse_publication() {
    let mut absent = package("weights.data", None, None);
    absent.pop();
    let mut truncated = package("weights.data", None, None);
    truncated[1].1.pop();
    let cases = vec![
        absent,
        truncated,
        package("weights.data", Some("5"), Some("4")),
        package("weights.data", Some("18446744073709551616"), Some("4")),
        package("weights.data", Some("0"), Some("3")),
        package("weights.data", Some("-1"), Some("4")),
        package("weights.data", None, Some("0")),
    ];
    for (index, files) in cases.into_iter().enumerate() {
        assert!(
            !import(files, "model.onnx", Fault::None).await.0,
            "range case {index}"
        );
    }
}

#[tokio::test]
async fn external_paths_never_discover_ambient_files_or_escape_selected_namespace() {
    for location in [
        "../weights.data",
        "/weights.data",
        "C:/weights.data",
        "data\\weights.data",
        "https://example.invalid/data",
        "./weights.data",
        "data//weights.data",
        "CON",
        "weights.data.",
        "model.onnx",
        "WEIGHTS.DATA",
    ] {
        assert!(
            !import(package(location, None, None), "model.onnx", Fault::None)
                .await
                .0,
            "unsafe/unselected location {location}"
        );
    }
    // The blob is selected at the package root, not beside this nested graph.
    let files = vec![
        (
            "onnx/model.onnx".into(),
            external("weights.data", None, None),
        ),
        ("weights.data".into(), 1.0_f32.to_le_bytes().to_vec()),
    ];
    assert!(!import(files, "onnx/model.onnx", Fault::None).await.0);
}

#[tokio::test]
async fn external_metadata_duplicates_checksums_basepaths_and_dual_storage_refuse() {
    for extra in [
        entry("location", "weights.data"),
        entry("basepath", "/tmp"),
        entry("checksum", "unverified"),
        field(9, &1.0_f32.to_le_bytes()),
        number(14, 1),
        field(14, &[1]),
    ] {
        let tensor = [weight("weight", Some("weights.data"), None, None), extra].concat();
        let model = model(&graph(
            vec![tensor],
            vec![node("Add", &["input", "weight"], "output")],
        ));
        assert!(
            !import(
                vec![
                    ("model.onnx".into(), model),
                    ("weights.data".into(), vec![0; 4])
                ],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
}

#[tokio::test]
async fn custom_functions_domains_attributes_code_and_unreferenced_companions_refuse() {
    for extra in [
        field(25, &[]),
        field(20, &[]),
        field(14, &[]),
        field(8, &[text(1, "custom"), number(2, 1)].concat()),
    ] {
        assert!(
            !import(
                vec![("model.onnx".into(), [embedded(), extra].concat())],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
    for extra in [
        text(7, "custom"),
        field(5, &text(1, "custom_attribute")),
        text(8, "custom_overload"),
    ] {
        let model = model(&graph(
            vec![],
            vec![[node("Identity", &["input"], "output"), extra].concat()],
        ));
        assert!(
            !import(
                vec![("model.onnx".into(), model)],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
    for (path, bytes) in [
        ("custom.py", b"print('no')".to_vec()),
        (
            "config.json",
            br#"{"auto_map":{"Model":"custom.Model"}}"#.to_vec(),
        ),
        ("extra.data", vec![0; 4]),
    ] {
        assert!(
            !import(
                vec![("model.onnx".into(), embedded()), (path.into(), bytes)],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
}

#[tokio::test]
async fn malformed_unknown_and_nonclosed_graphs_refuse_after_arbitrary_byte_acquisition() {
    let mut truncated = embedded();
    truncated.pop();
    let mut duplicate = embedded();
    duplicate.extend(number(1, 8));
    let cases = vec![
        b"not onnx".to_vec(),
        truncated,
        duplicate,
        model(&graph(vec![], vec![node("Unknown", &["input"], "output")])),
        model(&graph(
            vec![],
            vec![node("Identity", &["absent"], "output")],
        )),
        model(&graph(vec![], vec![node("Add", &["input"], "output")])),
        model(&graph(vec![], vec![node("Identity", &["input"], "input")])),
        model(&graph(vec![], vec![node("Identity", &["input"], "other")])),
        model(&graph(vec![], vec![])),
    ];
    for (index, bytes) in cases.into_iter().enumerate() {
        assert!(
            !import(
                vec![("model.onnx".into(), bytes)],
                "model.onnx",
                Fault::None
            )
            .await
            .0,
            "graph case {index}"
        );
    }
}

#[tokio::test]
async fn tensor_dtype_shape_size_and_graph_type_disagreements_refuse() {
    let valid_tensor = weight("weight", None, None, None);
    for tensor in [
        [number(1, 0), number(2, 1), text(8, "weight"), field(9, &[])].concat(),
        [
            number(1, u64::MAX),
            number(2, 1),
            text(8, "weight"),
            field(9, &[]),
        ]
        .concat(),
        [
            field(1, &[1; 17]),
            number(2, 1),
            text(8, "weight"),
            field(9, &[0; 4]),
        ]
        .concat(),
        [
            number(1, 1),
            number(2, 2),
            text(8, "weight"),
            field(9, &[0; 4]),
        ]
        .concat(),
        [
            number(1, 1),
            number(2, 1),
            text(8, "weight"),
            field(9, &[0; 3]),
        ]
        .concat(),
        [
            number(1, 2),
            number(2, 1),
            text(8, "weight"),
            field(9, &[0; 8]),
        ]
        .concat(),
        [valid_tensor.clone(), field(1, &[255; 10])].concat(),
    ] {
        let bytes = model(&graph(
            vec![tensor],
            vec![node("Add", &["input", "weight"], "output")],
        ));
        assert!(
            !import(
                vec![("model.onnx".into(), bytes)],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
    let wrong_output = [
        field(1, &node("Identity", &["input"], "output")),
        text(2, "bad-shape"),
        field(11, &value("input", &[1])),
        field(12, &value("output", &[2])),
    ]
    .concat();
    assert!(
        !import(
            vec![("model.onnx".into(), model(&wrong_output))],
            "model.onnx",
            Fault::None
        )
        .await
        .0
    );
}

#[tokio::test]
async fn model_ir_opset_and_size_limits_are_explicit() {
    let graph = graph(vec![], vec![node("Identity", &["input"], "output")]);
    for bytes in [
        [number(1, 11), field(7, &graph), field(8, &number(2, 13))].concat(),
        [number(1, 8), field(7, &graph), field(8, &number(2, 14))].concat(),
        [
            number(1, 8),
            field(7, &graph),
            field(8, &[text(1, "custom"), number(2, 13)].concat()),
        ]
        .concat(),
        vec![0; 16 * 1024 * 1024 + 1],
    ] {
        assert!(
            !import(
                vec![("model.onnx".into(), bytes)],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
}

#[tokio::test]
async fn dynamic_types_and_duplicate_graph_type_and_opset_fields_refuse() {
    let body = graph(vec![], vec![node("Identity", &["input"], "output")]);
    for bytes in [
        [model(&body), field(7, &body)].concat(),
        [model(&body), field(8, &number(2, 13))].concat(),
    ] {
        assert!(
            !import(
                vec![("model.onnx".into(), bytes)],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
    let static_type = field(
        1,
        &[number(1, 1), field(2, &field(1, &number(1, 1)))].concat(),
    );
    let dynamic_type = field(
        1,
        &[number(1, 1), field(2, &field(1, &text(2, "dynamic")))].concat(),
    );
    for kind in [
        [static_type.clone(), static_type.clone()].concat(),
        dynamic_type,
    ] {
        let input = [text(1, "input"), field(2, &kind)].concat();
        let body = [
            field(1, &node("Identity", &["input"], "output")),
            text(2, "invalid-type"),
            field(11, &input),
            field(12, &value("output", &[1])),
        ]
        .concat();
        assert!(
            !import(
                vec![("model.onnx".into(), model(&body))],
                "model.onnx",
                Fault::None
            )
            .await
            .0
        );
    }
    let rank_one_matmul = model(&graph(
        vec![],
        vec![node("MatMul", &["input", "input"], "output")],
    ));
    assert!(
        !import(
            vec![("model.onnx".into(), rank_one_matmul)],
            "model.onnx",
            Fault::None
        )
        .await
        .0
    );
}

#[tokio::test]
async fn qualification_does_not_waive_receipt_or_copied_byte_custody() {
    for fault in [Fault::WrongSpec, Fault::ChangedBytes, Fault::ReplacedPath] {
        assert!(
            !import(vec![("model.onnx".into(), embedded())], "model.onnx", fault)
                .await
                .0,
            "{fault:?}"
        );
    }
}

#[tokio::test]
async fn cancellation_before_finalization_creates_no_model_or_consumer_receipt() {
    assert!(
        !import(
            package("weights.data", None, None),
            "model.onnx",
            Fault::Cancel
        )
        .await
        .0
    );
}

#[test]
fn emit_successful_corpus_for_official_nonexecuting_checker() {
    let Ok(root) = std::env::var("PUMAS_ONNX_FIXTURE_EXPORT") else {
        return;
    };
    let root = Path::new(&root);
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("embedded.onnx"), embedded()).unwrap();
    std::fs::create_dir_all(root.join("onnx/data")).unwrap();
    std::fs::write(
        root.join("onnx/model.onnx"),
        external("data/weights.data", None, None),
    )
    .unwrap();
    std::fs::write(root.join("onnx/data/weights.data"), 1.0_f32.to_le_bytes()).unwrap();
    std::fs::write(
        root.join("identity.onnx"),
        model(&graph(vec![], vec![node("Identity", &["input"], "output")])),
    )
    .unwrap();
    let tensor = [
        field(1, &[2, 2]),
        number(2, 1),
        text(8, "weight"),
        field(9, &[0; 16]),
    ]
    .concat();
    let matrix = [
        field(1, &node("MatMul", &["input", "weight"], "output")),
        text(2, "dense-matrix"),
        field(5, &tensor),
        field(11, &value("input", &[1, 2])),
        field(12, &value("output", &[1, 2])),
    ]
    .concat();
    std::fs::write(root.join("matmul.onnx"), model(&matrix)).unwrap();
    std::fs::write(
        root.join("explicit.onnx"),
        external("explicit.data", Some("0"), Some("4")),
    )
    .unwrap();
    std::fs::write(root.join("explicit.data"), 1.0_f32.to_le_bytes()).unwrap();
    let shared = model(&graph(
        vec![
            weight("a", Some("shared.data"), Some("4"), Some("4")),
            weight("b", Some("shared.data"), Some("8"), Some("4")),
        ],
        vec![
            node("Add", &["input", "a"], "sum"),
            node("Add", &["sum", "b"], "output"),
        ],
    ));
    std::fs::write(root.join("shared.onnx"), shared).unwrap();
    std::fs::write(
        root.join("shared.data"),
        [
            vec![0; 4],
            1.0_f32.to_le_bytes().to_vec(),
            2.0_f32.to_le_bytes().to_vec(),
        ]
        .concat(),
    )
    .unwrap();
}
