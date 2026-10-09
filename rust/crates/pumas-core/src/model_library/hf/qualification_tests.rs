// Opt-in qualification of the actual managed importer, with caller-supplied,
// hash-verified bytes. Neither the normal test suite nor this fixture downloads
// a model. All source endpoints are replaced with a loopback deny server.
#[tokio::test]
#[ignore = "requires a separately copied, verified Qwen GGUF fixture"]
async fn verified_qwen_full_managed_importer_qualification() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const REPO: &str = "Qwen/Qwen2.5-0.5B-Instruct-GGUF";
    const COMMIT: &str = "9217f5db79a29953eb74d5343926648285ec7e67";
    const FILE: &str = "qwen2.5-0.5b-instruct-q4_k_m.gguf";
    const SIZE: u64 = 491400032;
    const HASH: &str = "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db";
    fn digest(path: &Path) -> String {
        let mut reader = std::fs::File::open(path).unwrap();
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            let count = reader.read(&mut buffer).unwrap();
            if count == 0 { break; }
            hasher.update(&buffer[..count]);
        }
        hex::encode(hasher.finalize())
    }
    let source = PathBuf::from(std::env::var("PUMAS_VERIFIED_GGUF_FIXTURE").expect("verified copied GGUF path"));
    let source_metadata = PathBuf::from(std::env::var("PUMAS_VERIFIED_METADATA_FIXTURE").expect("source metadata snapshot"));
    assert_eq!(std::fs::metadata(&source).unwrap().len(), SIZE);
    assert_eq!(digest(&source), HASH, "immutable payload source binding");
    let mut header = [0_u8; 8];
    std::fs::File::open(&source).unwrap().read_exact(&mut header).unwrap();
    assert_eq!(&header[..4], b"GGUF");
    assert_eq!(u32::from_le_bytes(header[4..].try_into().unwrap()), 3);
    let metadata: crate::models::ModelMetadata = serde_json::from_slice(&std::fs::read(source_metadata).unwrap()).unwrap();
    assert_eq!(metadata.schema_version, Some(2));
    assert_eq!(metadata.upstream_revision.as_deref(), Some(COMMIT));
    let root = PathBuf::from(std::env::var("PUMAS_QUALIFICATION_OUTPUT").expect("fresh evidence root"));
    std::fs::create_dir(&root).expect("qualification root must be new; preserve prior outcomes");
    let library = Arc::new(crate::model_library::ModelLibrary::new(root.join("library")).await.unwrap());
    let destination = library.build_model_path("llm", "qwen2_5", "fixture");
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::copy(&source, destination.join(FILE)).unwrap();
    assert_eq!(digest(&destination.join(FILE)), HASH);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(AtomicU64::new(0));
    let counter = requests.clone();
    let server = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            let mut request = [0_u8; 2048];
            let _ = socket.read(&mut request).await;
            let _ = socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
        }
    });
    let persistence = Arc::new(DownloadPersistence::new(&root));
    let mut client = HuggingFaceClient::new(root.join("cache")).unwrap();
    client.configure_download_destination_root(library.library_root()).unwrap();
    client.set_persistence(persistence.clone());
    client.set_download_importer(importer_with_authority_for_test(library.clone(), &client).await);
    client.set_test_download_base_url(endpoint);
    *client.auth_token.write().await = None;
    let revision = DownloadRevision::from_commit(COMMIT).unwrap();
    cache_pinned_repo_tree(&client, REPO, &revision, vec![LfsFileInfo { filename: FILE.into(), size: SIZE, sha256: HASH.into() }], Vec::new());
    let mut request = recovery_test_request(REPO, &[FILE.into()]);
    request.family = "qwen2_5".into();
    request.quant = Some("Q4_K_M".into());
    request.pipeline_tag = Some("text-generation".into());
    request.download_url = metadata.download_url.clone();
    request.release_date = metadata.release_date.clone();
    request.license_status = metadata.license_status.clone();
    request.model_card_json = Some(serde_json::to_string(&metadata.model_card).unwrap());
    let download_id = client.start_download_at_revision(&request, &destination, metadata.huggingface_evidence.clone(), revision).await.unwrap();
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            client.observe_finished_download_tasks().await;
            if matches!(client.get_download_status(&download_id).await, Some(DownloadStatus::Error | DownloadStatus::Completed)) && !client.download_tasks.contains(&download_id) { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("bounded managed importer completion");
    let status = client.get_download_status(&download_id).await;
    let error = client.downloads.read().await.get(&download_id).and_then(|state| state.error.clone());
    let records = client.acquisition.store().acquisitions().unwrap();
    let record = records.values().find(|record| record.demand.consumer == "hf.model").unwrap();
    let receipt = persistence.read_hf_completion_receipt(record.id).unwrap();
    let model_id = library.index().list_all().unwrap().first().unwrap().id.clone();
    let imported_metadata: crate::models::ModelMetadata = serde_json::from_slice(&std::fs::read(destination.join("metadata.json")).unwrap()).unwrap();
    let cached = library.index().get_model_package_facts_cache(&model_id, imported_metadata.selected_artifact_id.as_deref(), crate::index::ModelPackageFactsCacheScope::Detail).unwrap().unwrap();
    let facts: crate::models::ResolvedModelPackageFacts = serde_json::from_str(&cached.facts_json).unwrap();
    let mut current_reads = Vec::new();
    for _ in 0..32 {
        current_reads.push(library.cached_model_package_facts_are_current(&cached, None, None).await.unwrap());
    }
    let shutdown = client.shutdown_downloads().await;
    let result = serde_json::json!({"source": {"repo": REPO, "revision": COMMIT, "file": FILE, "size": SIZE, "sha256": HASH}, "request": request, "download_id": download_id, "status": status, "error": error, "acquisition": record, "receipt": receipt, "cache": cached, "actual_parser_and_contract_output": facts, "current_reads": current_reads, "source_requests": requests.load(Ordering::SeqCst), "shutdown": format!("{shutdown:?}")});
    std::fs::write(root.join("qualification.json"), serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("QUALIFICATION={} status={status:?} error={error:?}", root.display());
    server.abort();
    assert_eq!(requests.load(Ordering::SeqCst), 0, "fixture must not fetch source bytes or metadata");
    assert_eq!(status, Some(DownloadStatus::Completed));
    assert!(current_reads.iter().all(|value| *value));
    assert!(receipt.is_some(), "pinned importer must issue its own validated receipt");
    assert!(matches!(record.phase, crate::acquisition::AcquisitionPhase::Adopted { .. }));
    assert_eq!(facts.package_facts_contract_version, 3);
    assert_eq!(facts.model_ref.model_ref_contract_version, 1);
    let gguf = facts.gguf.as_ref().expect("actual GGUF parser output required");
    assert_eq!(gguf.architecture.as_deref(), Some("qwen2"));
    assert_eq!(gguf.file_type.as_deref(), Some("MOSTLY_Q4_K_M"));
    assert_eq!(gguf.context_length, Some(32768));
    assert_eq!(gguf.embedding_length, Some(896));
    assert_eq!(gguf.block_count, Some(24));
    assert_eq!(gguf.attention_head_count, Some(14));
    assert_eq!(gguf.tokenizer_model.as_deref(), Some("gpt2"));
    assert_eq!(gguf.chat_template_present, Some(true));
    for version in [2, 4] {
        let mut stale = cached.clone();
        stale.package_facts_contract_version = version;
        assert!(!library.cached_model_package_facts_are_current(&stale, None, None).await.unwrap());
        let mut unsupported = facts.clone();
        unsupported.package_facts_contract_version = version as u32;
        stale = cached.clone();
        stale.facts_json = serde_json::to_string(&unsupported).unwrap();
        assert!(!library.cached_model_package_facts_are_current(&stale, None, None).await.unwrap());
    }
    let mut stale = cached.clone();
    stale.source_fingerprint = "legacy-raw-metadata-fingerprint".into();
    assert!(!library.cached_model_package_facts_are_current(&stale, None, None).await.unwrap());
    assert!(shutdown.is_ok());
}
