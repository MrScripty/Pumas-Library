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
fn json(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).unwrap()
}
fn tokenizer() -> Vec<u8> {
    json(
        serde_json::json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":{"[UNK]":0,"hello":1},"unk_token":"[UNK]"}}),
    )
}
fn transformer() -> Files {
    vec![
        (
            "config.json".into(),
            json(
                serde_json::json!({"model_type":"llama","architectures":["LlamaForCausalLM"],"hidden_size":1}),
            ),
        ),
        (
            "tokenizer_config.json".into(),
            json(
                serde_json::json!({"tokenizer_class":"PreTrainedTokenizerFast","unk_token":"[UNK]"}),
            ),
        ),
        ("tokenizer.json".into(), tokenizer()),
        ("model-00001-of-00002.safetensors".into(), tensor("a")),
        ("model-00002-of-00002.safetensors".into(), tensor("b")),
        (
            "model.safetensors.index.json".into(),
            json(
                serde_json::json!({"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}),
            ),
        ),
    ]
}
fn diffusers() -> Files {
    let mut files = vec![
        (
            "model_index.json".into(),
            json(serde_json::json!({
                "_class_name":"StableDiffusionPipeline", "unet":["diffusers","UNet2DConditionModel"], "vae":["diffusers","AutoencoderKL"],
                "text_encoder":["transformers","CLIPTextModel"], "tokenizer":["transformers","CLIPTokenizerFast"], "scheduler":["diffusers","DDIMScheduler"],
                "safety_checker":[null,null], "feature_extractor":[null,null]
            })),
        ),
        (
            "scheduler/scheduler_config.json".into(),
            json(serde_json::json!({"_class_name":"DDIMScheduler","num_train_timesteps":1000})),
        ),
        (
            "tokenizer/tokenizer_config.json".into(),
            json(serde_json::json!({"tokenizer_class":"CLIPTokenizerFast"})),
        ),
        ("tokenizer/tokenizer.json".into(), tokenizer()),
    ];
    for component in ["unet", "vae", "text_encoder"] {
        files.push((
            format!("{component}/config.json"),
            json(serde_json::json!({"_class_name":"Model","hidden_size":1})),
        ));
        files.push((format!("{component}/model.safetensors"), tensor("weight")));
    }
    files
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
async fn safetensors_single_file_is_genuine_and_receipt_bound() {
    assert!(
        import(
            vec![("weights.safetensors".into(), tensor("weight"))],
            "weights.safetensors",
            Fault::None
        )
        .await
        .0
    );
}
#[tokio::test]
async fn complete_sharded_transformers_package_preserves_layout_and_reconciles() {
    assert!(
        import(
            transformer(),
            "model-00001-of-00002.safetensors",
            Fault::None
        )
        .await
        .0
    );
}
#[tokio::test]
async fn complete_diffusers_package_preserves_components_and_reconciles() {
    assert!(
        import(diffusers(), "unet/model.safetensors", Fault::None)
            .await
            .0
    );
}
#[tokio::test]
async fn malformed_or_incomplete_packages_acquire_bytes_but_never_publish() {
    let primary = "model-00001-of-00002.safetensors";
    for missing in [
        "config.json",
        "tokenizer_config.json",
        "tokenizer.json",
        "model.safetensors.index.json",
        "model-00002-of-00002.safetensors",
    ] {
        let mut files = transformer();
        files.retain(|(path, _)| path != missing);
        assert!(
            !import(files, primary, Fault::None).await.0,
            "accepted missing {missing}"
        );
    }
    for bad in [
        "config.json",
        "tokenizer.json",
        "model.safetensors.index.json",
        primary,
    ] {
        let mut files = transformer();
        files.iter_mut().find(|(path, _)| path == bad).unwrap().1 = b"not a model".to_vec();
        assert!(
            !import(files, primary, Fault::None).await.0,
            "accepted malformed {bad}"
        );
    }
    for map in [
        serde_json::json!({"a":"model-00002-of-00002.safetensors","b":"model-00001-of-00002.safetensors"}),
        serde_json::json!({"absent":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}),
    ] {
        let mut files = transformer();
        files
            .iter_mut()
            .find(|(path, _)| path == "model.safetensors.index.json")
            .unwrap()
            .1 = json(serde_json::json!({"weight_map":map}));
        assert!(!import(files, primary, Fault::None).await.0);
    }
}
#[tokio::test]
async fn diffusers_requires_whole_declared_and_standard_components() {
    for missing in [
        "scheduler/scheduler_config.json",
        "unet/config.json",
        "vae/model.safetensors",
        "tokenizer/tokenizer.json",
    ] {
        let mut files = diffusers();
        files.retain(|(path, _)| path != missing);
        assert!(!import(files, "unet/model.safetensors", Fault::None).await.0);
    }
    let mut files = diffusers();
    let index = &mut files
        .iter_mut()
        .find(|(path, _)| path == "model_index.json")
        .unwrap()
        .1;
    let mut value: serde_json::Value = serde_json::from_slice(index).unwrap();
    value.as_object_mut().unwrap().remove("vae");
    *index = json(value);
    assert!(!import(files, "unet/model.safetensors", Fault::None).await.0);
}
#[tokio::test]
async fn required_processor_and_custom_code_policy_are_enforced() {
    let primary = "model-00001-of-00002.safetensors";
    let mut files = transformer();
    files
        .iter_mut()
        .find(|(path, _)| path == "config.json")
        .unwrap()
        .1 = json(serde_json::json!({"model_type":"whisper"}));
    assert!(!import(files.clone(), primary, Fault::None).await.0);
    files.push(("preprocessor_config.json".into(), b"{}".to_vec()));
    assert!(!import(files.clone(), primary, Fault::None).await.0);
    files.last_mut().unwrap().1 = json(
        serde_json::json!({"feature_extractor_type":"WhisperFeatureExtractor","feature_size":80}),
    );
    assert!(import(files, primary, Fault::None).await.0);
    for code in [false, true] {
        let mut files = transformer();
        if code {
            files.push((
                "modeling_custom.py".into(),
                b"raise Exception('never execute')".to_vec(),
            ));
        } else {
            files
                .iter_mut()
                .find(|(path, _)| path == "config.json")
                .unwrap()
                .1 = b"{\"model_type\":\"llama\",\"auto_map\":{\"AutoModel\":\"custom.Model\"}}"
                .to_vec();
        }
        assert!(!import(files, primary, Fault::None).await.0);
    }
}
#[tokio::test]
async fn custody_receipt_mismatch_and_copy_mutation_refuse_publication() {
    for fault in [Fault::WrongSpec, Fault::ChangedBytes] {
        assert!(
            !import(
                vec![("weights.safetensors".into(), tensor("weight"))],
                "weights.safetensors",
                fault
            )
            .await
            .0
        );
    }
}
#[tokio::test]
async fn cancelled_sharded_transfer_has_no_receipt_or_publication() {
    assert!(
        !import(
            transformer(),
            "model-00001-of-00002.safetensors",
            Fault::Cancel
        )
        .await
        .0
    );
}
#[tokio::test]
async fn unknown_onnx_and_pickle_remain_byte_acquisition_only() {
    for (path, bytes) in [
        ("unknown.dat", b"arbitrary data".as_slice()),
        ("model.onnx", b"\x08\x09\x12\x04onnx\x3a\x00".as_slice()),
        ("model.pt", b"\x80\x02N.".as_slice()),
    ] {
        assert!(
            !import(vec![(path.into(), bytes.to_vec())], path, Fault::None)
                .await
                .0
        );
    }
}

#[tokio::test]
async fn changed_acquisition_path_is_refused_before_descriptor_handoff() {
    assert!(
        !import(
            vec![("weights.safetensors".into(), tensor("weight"))],
            "weights.safetensors",
            Fault::ReplacedPath
        )
        .await
        .0
    );
}

#[tokio::test]
async fn safetensors_data_and_header_closure_are_required() {
    let mut truncated = tensor("weight");
    truncated.pop();
    let mut trailing = tensor("weight");
    trailing.push(0);
    let bad_header = br#"{"x":{"dtype":"F32","shape":[2],"data_offsets":[0,4]}}"#;
    let shape = [
        (bad_header.len() as u64).to_le_bytes().as_slice(),
        bad_header.as_slice(),
        0_f32.to_le_bytes().as_slice(),
    ]
    .concat();
    let duplicate=br#"{"x":{"dtype":"F32","shape":[1],"data_offsets":[0,4]},"x":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
    let duplicate = [
        (duplicate.len() as u64).to_le_bytes().as_slice(),
        duplicate.as_slice(),
        0_f32.to_le_bytes().as_slice(),
    ]
    .concat();
    for bytes in [truncated, trailing, shape, duplicate] {
        assert!(
            !import(
                vec![("weights.safetensors".into(), bytes)],
                "weights.safetensors",
                Fault::None
            )
            .await
            .0
        );
    }
}

#[tokio::test]
async fn malformed_component_tokenizer_and_split_indexes_are_refused() {
    for signature in [
        serde_json::json!(["diffusers"]),
        serde_json::json!(["diffusers", 42]),
    ] {
        let mut files = diffusers();
        let index = &mut files
            .iter_mut()
            .find(|(path, _)| path == "model_index.json")
            .unwrap()
            .1;
        let mut value: serde_json::Value = serde_json::from_slice(index).unwrap();
        value["unet"] = signature;
        *index = json(value);
        assert!(!import(files, "unet/model.safetensors", Fault::None).await.0);
    }
    for merges in [
        serde_json::json!([42]),
        serde_json::json!([["missing", "token"]]),
        serde_json::json!([["a", 42, "b"]]),
        serde_json::json!([["a", null, "b"]]),
    ] {
        let mut files = transformer();
        files
            .iter_mut()
            .find(|(path, _)| path == "tokenizer.json")
            .unwrap()
            .1 = json(
            serde_json::json!({"version":"1.0","model":{"type":"BPE","vocab":{"a":0,"b":1,"ab":2},"merges":merges}}),
        );
        assert!(
            !import(files, "model-00001-of-00002.safetensors", Fault::None)
                .await
                .0
        );
    }
    let mut files = diffusers();
    files.retain(|(path, _)| path != "unet/model.safetensors");
    for name in ["a", "b"] {
        files.push((format!("unet/{name}.safetensors"), tensor(name)));
    }
    files.push((
        "model.safetensors.index.json".into(),
        json(serde_json::json!({"weight_map":{"a":"unet/a.safetensors","b":"unet/b.safetensors"}})),
    ));
    assert!(!import(files, "unet/a.safetensors", Fault::None).await.0);
    let mut files = transformer();
    files.retain(|(path, _)| !path.contains("safetensors"));
    for name in ["a", "b"] {
        files.push((format!("{name}.safetensors"), tensor(name)));
        files.push((
            format!("{name}.safetensors.index.json"),
            json(serde_json::json!({"weight_map":{name:format!("{name}.safetensors")}})),
        ));
    }
    assert!(!import(files, "a.safetensors", Fault::None).await.0);
}
