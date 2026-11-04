//! S3's facade delegates format qualification to the shared model importer.
#![cfg(feature = "s3")]
use pumas_library::{
    acquisition::*,
    models::{ImportState, ModelImportSpec},
    network::RetryConfig,
    PumasApi, S3ModelImportControl, S3ModelImportPhase, S3ModelImportRequest,
};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn run(primary: &str, mut files: Vec<(String, Vec<u8>)>, cancelled: bool) {
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let served = files.clone();
    let server = tokio::spawn(async move {
        let mut count = 0;
        for head in [true, false] {
            for (path, bytes) in &served {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    request.push(socket.read_u8().await.unwrap());
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with(if head { "HEAD " } else { "GET " }));
                assert!(request.contains(&format!("/{path}?")));
                assert!(request.contains("versionId=v1"));
                let headers=format!("HTTP/1.1 {}\r\nContent-Length: {}\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: v1\r\nETag: \"selected\"\r\n{}Connection: close\r\n\r\n",if head {"200 OK"}else{"206 Partial Content"},bytes.len(),if head {String::new()}else{format!("Content-Range: bytes 0-{}/{}\r\n",bytes.len()-1,bytes.len())});
                socket.write_all(headers.as_bytes()).await.unwrap();
                if !head {
                    socket.write_all(bytes).await.unwrap();
                }
                socket.shutdown().await.unwrap();
                count += 1;
            }
        }
        count
    });
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(stage.path().join("stage")).unwrap();
    let api = PumasApi::builder(root.path())
        .with_registry(
            pumas_library::registry::LibraryRegistry::open_at(&root.path().join("registry.db"))
                .unwrap(),
        )
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    let control = S3ModelImportControl::new();
    let progress = control.subscribe();
    if cancelled {
        assert!(control.cancel());
    }
    let result = api
        .import_s3_model(
            S3ModelImportRequest {
                operation_id: uuid::Uuid::new_v4(),
                source: S3ReaderConfig {
                    endpoint,
                    region: "fixture".into(),
                    bucket: "fixture".into(),
                    addressing: S3Addressing::Path,
                    allow_http: true,
                    operation_timeout: Duration::from_secs(5),
                },
                credentials: None,
                entries: files
                    .iter()
                    .map(|(path, bytes)| S3ManifestEntry {
                        source_key: path.clone(),
                        version: "v1".into(),
                        logical_path: path.clone(),
                        expected_sha256: Sha256Evidence::new(
                            "fixture.sha256",
                            hex::encode(Sha256::digest(bytes)),
                        )
                        .unwrap(),
                    })
                    .collect(),
                import: ModelImportSpec {
                    path: primary.into(),
                    family: "fixture".into(),
                    official_name: "S3 safe model".into(),
                    model_type: Some("llm".into()),
                    repo_id: None,
                    subtype: None,
                    tags: None,
                    security_acknowledged: None,
                },
                workspace: AcquisitionWorkspace::from_reserved_directory(
                    stage.path(),
                    Path::new("stage"),
                    Arc::new(()),
                    || Ok(()),
                )
                .unwrap(),
                retry: AcquisitionRetryPolicy {
                    attempts: Some(1),
                    elapsed: Duration::from_secs(5),
                    backoff: RetryConfig::default(),
                },
            },
            control.clone(),
        )
        .await;
    if cancelled {
        assert!(result.is_err());
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Cancelled);
        assert!(api.acquisition().store().acquisitions().unwrap().is_empty());
        server.abort();
    } else {
        let result = result.unwrap();
        let id = result.model_id.unwrap();
        let library = api.model_library();
        assert_eq!(progress.borrow().phase, S3ModelImportPhase::Completed);
        assert!(!control.cancel());
        assert_eq!(
            library
                .get_effective_metadata(&id)
                .unwrap()
                .unwrap()
                .import_state,
            Some(ImportState::Ready)
        );
        for (path, bytes) in &files {
            assert_eq!(
                std::fs::read(library.library_root().join(&id).join(path)).unwrap(),
                *bytes
            );
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(10), server)
                .await
                .unwrap()
                .unwrap(),
            files.len() * 2
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
        assert_eq!(
            api.acquisition()
                .consumer_receipt(record.id)
                .unwrap()
                .unwrap()
                .verified_files,
            record.files
        );
    }
    api.shutdown_instance().await.unwrap();
}
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
#[tokio::test]
async fn s3_single_safetensors_reaches_shared_publication() {
    run(
        "weights.safetensors",
        vec![("weights.safetensors".into(), tensor("weight"))],
        false,
    )
    .await;
}
#[tokio::test]
async fn s3_non_gguf_cancellation_precedes_source_effects() {
    run(
        "weights.safetensors",
        vec![("weights.safetensors".into(), tensor("weight"))],
        true,
    )
    .await;
}
#[tokio::test]
async fn gguf_inert_index_auxiliaries_keep_their_original_semantics() {
    let bytes = [
        b"GGUF".as_slice(),
        3_u32.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
    ]
    .concat();
    run(
        "weights.gguf",
        vec![
            ("weights.gguf".into(), bytes),
            ("model_index.json".into(), b"inert auxiliary".to_vec()),
            (
                "model.safetensors.index.json".into(),
                b"inert auxiliary".to_vec(),
            ),
        ],
        false,
    )
    .await;
}

#[tokio::test]
async fn s3_complete_sharded_safetensors_package_reaches_shared_publication() {
    let json = |value: serde_json::Value| serde_json::to_vec(&value).unwrap();
    run("model-00001-of-00002.safetensors",vec![
        ("config.json".into(),json(serde_json::json!({"model_type":"llama","architectures":["LlamaForCausalLM"],"hidden_size":1}))),
        ("tokenizer_config.json".into(),json(serde_json::json!({"tokenizer_class":"PreTrainedTokenizerFast"}))),
        ("tokenizer.json".into(),json(serde_json::json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":{"[UNK]":0},"unk_token":"[UNK]"}}))),
        ("model-00001-of-00002.safetensors".into(),tensor("a")),
        ("model-00002-of-00002.safetensors".into(),tensor("b")),
        ("model.safetensors.index.json".into(),json(serde_json::json!({"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}))),
    ],false).await;
}
