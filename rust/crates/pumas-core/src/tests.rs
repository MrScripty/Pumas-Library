use super::*;
use crate::model_library::{ModelLibrary, ModelMetadata};
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[tokio::test]
async fn pre_worker_preparation_retains_root_after_caller_and_client_drop() {
    use crate::model_library::{DownloadDestinationRoot, HuggingFaceClient};
    use std::sync::Arc;
    use std::time::Duration;

    let temp = TempDir::new().unwrap();
    let library = Arc::new(
        ModelLibrary::new(temp.path().join("library"))
            .await
            .unwrap(),
    );
    let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
    client
        .configure_download_destination_root(library.library_root())
        .unwrap();
    let client = Arc::new(client);
    let artifact = "owner--repo__q4";
    let source = library.build_artifact_model_path("unknown", "owner", artifact);
    let target = library.build_artifact_model_path("llm", "owner", artifact);
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("weights.gguf.part"), b"retained partial").unwrap();
    library
        .save_metadata(
            &source,
            &ModelMetadata {
                model_type: Some("unknown".into()),
                family: Some("owner".into()),
                cleaned_name: Some(artifact.into()),
                selected_artifact_id: Some(artifact.into()),
                match_source: Some("download_partial".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let root = DownloadDestinationRoot::open(library.library_root()).unwrap();
    let (entered, ready) = tokio::sync::oneshot::channel();
    let entered = Mutex::new(Some(entered));
    let (release, held) = std::sync::mpsc::channel();
    let held = Mutex::new(held);
    library.set_metadata_write_notifier(Some(Arc::new(move |_| {
        if let Some(entered) = entered.lock().unwrap().take() {
            let _ = entered.send(());
            held.lock().unwrap().recv().unwrap();
        }
    })));
    let caller = tokio::spawn({
        let client = client.clone();
        let library = library.clone();
        async move {
            let preparing_client = client.clone();
            // Exercise the same real preparation and retained-effect Interface
            // used by core start, without its preceding remote metadata producer.
            client
                .run_download_invocation(move |context| async move {
                    let context = preparing_client.protect_download_mutation(&context).await?;
                    drop(preparing_client);
                    context
                        .run_fallible_blocking_named("prepare HF artifact destination", move || {
                            library.prepare_artifact_download_destination("llm", "owner", artifact)
                        })
                        .await
                        .map_err(|error| PumasError::Other(error.to_string()))?
                })
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(3), ready)
        .await
        .unwrap()
        .unwrap();
    let effect_started = !source.exists() && target.join("weights.gguf.part").exists();
    caller.abort();
    let _ = caller.await;
    drop(client);
    let contention = root.try_acquire_execution_grant();
    // Release before assertions so a failed oracle cannot strand the real writer.
    release.send(()).unwrap();
    assert!(effect_started);
    assert!(matches!(contention, Err(PumasError::DownloadRootBusy)));
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match root.try_acquire_execution_grant() {
                Ok(grant) => break grant,
                Err(PumasError::DownloadRootBusy) => tokio::task::yield_now().await,
                Err(error) => panic!("root acquisition failed: {error}"),
            }
        }
    })
    .await
    .expect("dropped client must retain then release observed preparation");
    assert_eq!(
        std::fs::read(target.join("weights.gguf.part")).unwrap(),
        b"retained partial"
    );
    assert_eq!(
        library
            .load_metadata(&target)
            .unwrap()
            .unwrap()
            .model_type
            .as_deref(),
        Some("llm")
    );
    assert!(library
        .index()
        .get(&library.build_artifact_model_id("llm", "owner", artifact))
        .unwrap()
        .is_some());
    library.set_metadata_write_notifier(None);
}

#[tokio::test]
async fn builder_requires_download_restore_grant_but_no_client_reads_do_not() {
    use crate::model_library::DownloadDestinationRoot;

    let temp = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp.path());
    let initial = PumasApi::builder(temp.path())
        .auto_create_dirs(true)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    let library_root = initial.model_library().library_root().to_path_buf();
    initial.shutdown_downloads().await.unwrap();
    drop(initial);

    let root = DownloadDestinationRoot::open(&library_root).unwrap();
    let grant = root.try_acquire_execution_grant().unwrap();
    let store_path = temp.path().join("launcher-data/downloads.json");
    let store_before = std::fs::read(&store_path).ok();
    let busy = PumasApi::builder(temp.path())
        .auto_create_dirs(true)
        .with_process_manager(false)
        .build()
        .await;
    assert!(matches!(busy, Err(PumasError::DownloadRootBusy)));
    assert_eq!(std::fs::read(&store_path).ok(), store_before);

    let without_client = PumasApi::builder(temp.path())
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    assert!(without_client.list_models().await.unwrap().is_empty());
    without_client.shutdown_downloads().await.unwrap();
    assert_eq!(std::fs::read(&store_path).ok(), store_before);
    drop(without_client);
    drop(grant);

    let restored = PumasApi::builder(temp.path())
        .auto_create_dirs(true)
        .with_process_manager(false)
        .build()
        .await
        .expect("startup must retry required restore after contention ends");
    assert!(restored.list_hf_downloads().await.unwrap().is_empty());
    restored.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn ticket_recovery_refuses_busy_before_index_or_download_mutation() {
    use crate::model_library::DownloadDestinationRoot;
    use crate::model_library::{
        issue_download_recovery_ticket, DownloadRecoveryModelId, DownloadRecoveryToken,
    };

    let temp = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp.path());
    let api = PumasApi::builder(temp.path())
        .auto_create_dirs(true)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();
    let library = api.model_library();
    let root = DownloadDestinationRoot::open(library.library_root()).unwrap();
    // Wait only for the empty startup's retained observations to release.
    // Acquire before creating partial files so background orphan discovery
    // cannot admit the fixture as real download work while it is being seeded.
    let grant = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match root.try_acquire_execution_grant() {
                Ok(grant) => break grant,
                Err(PumasError::DownloadRootBusy) => tokio::task::yield_now().await,
                Err(error) => panic!("empty startup root acquisition failed: {error}"),
            }
        }
    })
    .await
    .expect("empty startup must release download mutation custody");
    let model_id = "llm/acme/model";
    let destination = library.library_root().join(model_id);
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(destination.join("weights.gguf.part"), b"partial").unwrap();
    let metadata = ModelMetadata {
        model_id: Some(model_id.into()),
        family: Some("acme".into()),
        model_type: Some("llm".into()),
        cleaned_name: Some("model".into()),
        official_name: Some("Model".into()),
        repo_id: Some("acme/model".into()),
        selected_artifact_id: Some("acme/model::Q4_K_M".into()),
        selected_artifact_quant: Some("Q4_K_M".into()),
        selected_artifact_files: Some(vec!["weights.gguf".into()]),
        expected_files: Some(vec!["weights.gguf".into()]),
        ..Default::default()
    };
    library
        .save_metadata(&destination, &metadata)
        .await
        .unwrap();
    library.index_model_dir(&destination).await.unwrap();
    let record = api.get_model(model_id).await.unwrap().unwrap();
    let ticket = issue_download_recovery_ticket(library.library_root(), &record)
        .unwrap()
        .unwrap();
    let metadata_before = std::fs::read(destination.join("metadata.json")).unwrap();
    let store_path = temp.path().join("launcher-data/downloads.json");
    let store_before = std::fs::read(&store_path).ok();
    let index_before = serde_json::to_value(library.index().get(model_id).unwrap()).unwrap();
    let action = api
        .resume_partial_download_with_ticket(
            &DownloadRecoveryModelId::parse(model_id).unwrap(),
            &DownloadRecoveryToken::parse(ticket.token()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(action.action, "none");
    assert_eq!(action.reason_code.as_deref(), Some("download_root_busy"));
    assert!(action.download_id.is_none());
    assert_eq!(std::fs::read(&store_path).ok(), store_before);
    assert_eq!(
        serde_json::to_value(library.index().get(model_id).unwrap()).unwrap(),
        index_before
    );
    assert_eq!(
        std::fs::read(destination.join("metadata.json")).unwrap(),
        metadata_before
    );
    assert_eq!(
        std::fs::read(destination.join("weights.gguf.part")).unwrap(),
        b"partial"
    );
    assert!(!destination.join(".pumas_download").exists());
    assert!(api.list_hf_downloads().await.unwrap().is_empty());
    drop(grant);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn builder_retains_failed_download_import_and_retries_before_completion() {
    use crate::model_library::download_store::{
        DownloadAdmissionDomain, DownloadAdmissionRequest, DownloadPersistence,
        PersistedDestinationIdentity, PersistedDownload,
    };
    use crate::models::DownloadStatus;

    let temp = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp.path());
    let library_root = temp.path().join("shared-resources/models");
    let destination = library_root.join("vision/idea-research/grounding-dino-base");
    std::fs::create_dir_all(&destination).unwrap();
    let payload = b"not-a-real-model";
    std::fs::write(destination.join("detector.onnx.part"), payload).unwrap();
    std::fs::write(destination.join(".pumas_download"), b"{}").unwrap();
    // A directory at the metadata file path makes the real importer fail
    // without replacing its implementation or changing filesystem permissions.
    std::fs::create_dir(destination.join("metadata.json")).unwrap();
    std::fs::create_dir_all(temp.path().join("launcher-data")).unwrap();
    let store = DownloadPersistence::new(&temp.path().join("launcher-data"));
    let snapshot = PersistedDownload {
        revision: None,
        download_id: "builder-import-retry".into(),
        repo_id: "IDEA-Research/grounding-dino-base".into(),
        filename: "detector.onnx".into(),
        filenames: vec!["detector.onnx".into()],
        dest_dir: destination.clone(),
        total_bytes: Some(payload.len() as u64),
        status: DownloadStatus::Error,
        download_request: DownloadRequest {
            repo_id: "IDEA-Research/grounding-dino-base".into(),
            family: "idea-research".into(),
            official_name: "grounding-dino-base".into(),
            model_type: Some("vision".into()),
            quant: None,
            filename: Some("detector.onnx".into()),
            filenames: None,
            pipeline_tag: Some("zero-shot-object-detection".into()),
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: Some("apache-2.0".into()),
        },
        created_at: chrono::Utc::now().to_rfc3339(),
        known_sha256: None,
        huggingface_evidence: None,
    };
    let mut client = HuggingFaceClient::new(temp.path().join("fixture-cache")).unwrap();
    client
        .configure_download_destination_root(&library_root)
        .unwrap();
    let library_identity: serde_json::Value = serde_json::from_slice(
        &std::fs::read(library_root.join(".pumas-library-id.json")).unwrap(),
    )
    .unwrap();
    drop(client);
    let request = DownloadAdmissionRequest {
        snapshot,
        domain: DownloadAdmissionDomain::Ambient,
        destination: PersistedDestinationIdentity {
            library_root: format!("uuid:{}", library_identity["library_id"].as_str().unwrap()),
            relative_target: "vision/idea-research/grounding-dino-base".into(),
        },
        requested_payload_files: vec!["detector.onnx".into()],
        execution_files: vec!["detector.onnx".into()],
    };
    let attempt = uuid::Uuid::new_v4().to_string();
    store
        .admit_download(&attempt, &request)
        .unwrap()
        .into_result()
        .unwrap();
    let original_admission = serde_json::to_value(
        &store
            .load_lifecycle_inventory_strict()
            .unwrap()
            .queue_admissions["builder-import-retry"],
    )
    .unwrap();

    let api = PumasApi::builder(temp.path())
        .auto_create_dirs(true)
        .with_process_manager(false)
        .build()
        .await
        .expect("operational import failure must not prevent API startup");
    let downloads = api.list_hf_downloads().await.unwrap();
    assert_eq!(downloads.len(), 1, "failed import must remain tracked");
    assert_eq!(downloads[0].download_id, "builder-import-retry");
    assert_eq!(downloads[0].status, DownloadStatus::Error);
    let inventory = store.load_lifecycle_inventory_strict().unwrap();
    assert_eq!(inventory.downloads.len(), 1);
    assert_eq!(inventory.downloads[0].status, DownloadStatus::Error);
    assert_eq!(
        serde_json::to_value(&inventory.queue_admissions["builder-import-retry"]).unwrap(),
        original_admission,
        "failed import must retain exact admission custody"
    );
    assert_eq!(
        std::fs::read(destination.join("detector.onnx")).unwrap(),
        payload
    );
    assert!(api.model_library().index().list_all().unwrap().is_empty());
    // Shutdown reports the retained importer failure, but must drain before
    // the fixture repairs the obstruction and opens a fresh owning instance.
    assert!(matches!(
        api.shutdown_downloads().await,
        Err(PumasError::DownloadShutdownFailed { failures }) if failures > 0
    ));
    drop(api);
    std::fs::remove_dir(destination.join("metadata.json")).unwrap();

    for _ in 0..2 {
        let api = PumasApi::builder(temp.path())
            .auto_create_dirs(true)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let metadata = api
            .model_library()
            .load_metadata(&destination)
            .unwrap()
            .unwrap();
        assert_eq!(
            metadata.repo_id.as_deref(),
            Some("IDEA-Research/grounding-dino-base")
        );
        assert_eq!(metadata.match_source.as_deref(), Some("download"));
        let model_id = metadata.model_id.as_ref().unwrap();
        assert!(api.model_library().index().get(model_id).unwrap().is_some());
        assert_eq!(api.model_library().index().count().unwrap(), 1);
        assert!(api.list_hf_downloads().await.unwrap().is_empty());
        let inventory = store.load_lifecycle_inventory_strict().unwrap();
        assert!(inventory.downloads.is_empty());
        assert!(inventory.queue_admissions.is_empty());
        assert_eq!(
            std::fs::read(destination.join("detector.onnx")).unwrap(),
            payload
        );
        api.shutdown_downloads().await.unwrap();
        drop(api);
    }
}

struct RegistryTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl RegistryTestGuard {
    fn new(root: &std::path::Path) -> Self {
        let lock = REGISTRY_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("registry test lock poisoned");
        crate::platform::paths::set_test_registry_db_path(Some(
            root.join("registry-test")
                .join(config::RegistryConfig::DB_FILENAME),
        ));
        Self { _lock: lock }
    }
}

impl Drop for RegistryTestGuard {
    fn drop(&mut self) {
        crate::platform::paths::set_test_registry_db_path(None);
    }
}

async fn seed_stale_library_state(launcher_root: &std::path::Path) {
    let library_root = launcher_root.join("shared-resources").join("models");
    let library = ModelLibrary::new(&library_root).await.unwrap();

    let canonical_audio_dir = library.build_model_path("audio", "kittenml", "kitten-tts-mini-0_8");
    let duplicate_audio_dir =
        library.build_model_path("unknown", "kittenml", "kitten-tts-mini-0_8");
    for dir in [&canonical_audio_dir, &duplicate_audio_dir] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("config.json"), b"{}").unwrap();
        std::fs::write(dir.join("kitten_tts_mini_v0_8.onnx"), b"onnx").unwrap();
        std::fs::write(dir.join("voices.npz"), b"voices").unwrap();
        std::fs::write(
            dir.join(".pumas_download"),
            br#"{"repo_id":"KittenML/kitten-tts-mini-0.8"}"#,
        )
        .unwrap();
    }

    let canonical_audio_metadata = ModelMetadata {
        model_id: Some("audio/kittenml/kitten-tts-mini-0_8".to_string()),
        family: Some("KittenML".to_string()),
        model_type: Some("audio".to_string()),
        official_name: Some("kitten-tts-mini-0.8".to_string()),
        cleaned_name: Some("kitten-tts-mini-0_8".to_string()),
        repo_id: Some("KittenML/kitten-tts-mini-0.8".to_string()),
        metadata_needs_review: Some(true),
        review_reasons: Some(vec![
            "model-type-fallback-name-tokens".to_string(),
            "unknown-task-signature".to_string(),
        ]),
        ..Default::default()
    };
    let duplicate_audio_metadata = ModelMetadata {
        model_id: Some("unknown/kittenml/kitten-tts-mini-0_8".to_string()),
        family: Some("KittenML".to_string()),
        model_type: Some("audio".to_string()),
        official_name: Some("kitten-tts-mini-0.8".to_string()),
        cleaned_name: Some("kitten-tts-mini-0_8".to_string()),
        repo_id: Some("KittenML/kitten-tts-mini-0.8".to_string()),
        metadata_needs_review: Some(true),
        review_reasons: Some(vec![
            "model-type-fallback-name-tokens".to_string(),
            "unknown-task-signature".to_string(),
        ]),
        ..Default::default()
    };
    library
        .save_metadata(&canonical_audio_dir, &canonical_audio_metadata)
        .await
        .unwrap();
    library
        .save_metadata(&duplicate_audio_dir, &duplicate_audio_metadata)
        .await
        .unwrap();

    let stale_family_dir = library.build_model_path("llm", "vit", "qwen-image-2512-heretic");
    std::fs::create_dir_all(&stale_family_dir).unwrap();
    std::fs::write(
        stale_family_dir.join("config.json"),
        br#"{"architectures":["Qwen2ForCausalLM"]}"#,
    )
    .unwrap();
    std::fs::write(stale_family_dir.join("model.safetensors"), b"stub").unwrap();

    let stale_family_metadata = ModelMetadata {
        model_id: Some("llm/vit/qwen-image-2512-heretic".to_string()),
        family: Some("catplusplus".to_string()),
        model_type: Some("llm".to_string()),
        official_name: Some("Qwen-Image-2512-Heretic".to_string()),
        cleaned_name: Some("qwen-image-2512-heretic".to_string()),
        repo_id: Some("catplusplus/Qwen-Image-2512-Heretic".to_string()),
        metadata_needs_review: Some(true),
        review_reasons: Some(vec!["unknown-task-signature".to_string()]),
        ..Default::default()
    };
    library
        .save_metadata(&stale_family_dir, &stale_family_metadata)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_api_creation() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    assert_eq!(api.launcher_root(), temp_dir.path());
}

#[tokio::test]
async fn test_api_paths() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    assert!(api.launcher_data_dir().ends_with("launcher-data"));
    assert!(api.metadata_dir().ends_with("metadata"));
    assert!(api.versions_dir(AppId::Ollama).ends_with("ollama-versions"));
}

#[tokio::test]
async fn test_get_status() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    let status = api.get_status().await.unwrap();
    assert!(status.success);
}

#[tokio::test]
async fn test_get_disk_space() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    let disk = api.get_disk_space().await.unwrap();
    assert!(disk.success);
    assert!(disk.total > 0);
}

#[tokio::test]
async fn test_new_rejects_existing_primary_without_implicit_client() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let primary = PumasApi::new(temp_dir.path()).await.unwrap();
    assert!(primary.is_primary());

    let err = match PumasApi::new(temp_dir.path()).await {
        Ok(_) => panic!("second PumasApi::new should reject an existing primary"),
        Err(err) => err,
    };
    assert!(
        matches!(err, PumasError::InvalidParams { message } if message.contains("PumasLocalClient"))
    );
}

#[tokio::test]
async fn test_start_ipc_server_is_idempotent() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    let first_port = api.start_ipc_server().await.unwrap();
    let second_port = api.start_ipc_server().await.unwrap();
    assert_eq!(first_port, second_port);
}

#[tokio::test]
async fn test_explicit_local_client_connects_to_running_primary_for_selector_snapshot() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let _primary = PumasApi::new(temp_dir.path()).await.unwrap();

    let instances = PumasLocalClient::discover_ready_instances().unwrap();
    assert_eq!(instances.len(), 1);
    let client = PumasLocalClient::connect(instances[0].clone())
        .await
        .unwrap();

    let snapshot = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.model_library_selector_snapshot(
            crate::models::ModelLibrarySelectorSnapshotRequest::default(),
        ),
    )
    .await
    .expect("model_library_selector_snapshot timed out")
    .unwrap();
    assert!(snapshot.rows.is_empty());
}

#[tokio::test]
async fn test_get_library_status_reconciles_stale_library_state_on_first_read() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    seed_stale_library_state(temp_dir.path()).await;

    let api = PumasApi::builder(temp_dir.path())
        .auto_create_dirs(true)
        .build()
        .await
        .unwrap();

    let status = api.get_library_status().await.unwrap();
    assert!(status.success);

    assert!(!temp_dir
        .path()
        .join("shared-resources/models/unknown/kittenml/kitten-tts-mini-0_8")
        .exists());
    assert!(!temp_dir
        .path()
        .join("shared-resources/models/llm/vit/qwen-image-2512-heretic")
        .exists());
    assert!(temp_dir
        .path()
        .join("shared-resources/models/diffusion/catplusplus/qwen-image-2512-heretic")
        .exists());
}

#[tokio::test]
async fn test_generate_migration_dry_run_reconciles_before_reporting() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    seed_stale_library_state(temp_dir.path()).await;

    let api = PumasApi::builder(temp_dir.path())
        .auto_create_dirs(true)
        .build()
        .await
        .unwrap();

    let report = api.generate_model_migration_dry_run_report().await.unwrap();
    assert_eq!(report.collision_count, 0);
    assert_eq!(report.move_candidates, 0);
    assert!(!temp_dir
        .path()
        .join("shared-resources/models/unknown/kittenml/kitten-tts-mini-0_8")
        .exists());
    assert!(temp_dir
        .path()
        .join("shared-resources/models/diffusion/catplusplus/qwen-image-2512-heretic")
        .exists());
}

#[tokio::test]
async fn test_execute_migration_notifies_model_library_refresh_even_when_no_moves() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());

    let api = PumasApi::builder(temp_dir.path())
        .auto_create_dirs(true)
        .build()
        .await
        .unwrap();

    let baseline = api
        .list_model_library_updates_since(None, 100)
        .await
        .unwrap()
        .cursor;

    let report = api.execute_model_migration().await.unwrap();
    assert_eq!(report.planned_move_count, 0);
    assert_eq!(report.error_count, 0, "{:?}", report.results);

    let feed = api
        .list_model_library_updates_since(Some(&baseline), 100)
        .await
        .unwrap();
    assert_eq!(feed.events.len(), 1, "{:?}", feed.events);
    let event = &feed.events[0];
    assert_eq!(event.model_id, "__library__/model-library-refresh");
    assert_eq!(
        event.change_kind,
        crate::models::ModelLibraryChangeKind::MetadataModified
    );
    assert_eq!(
        event.fact_family,
        crate::models::ModelFactFamily::SearchIndex
    );
    assert_eq!(
        event.refresh_scope,
        crate::models::ModelLibraryRefreshScope::SummaryAndDetail
    );
    assert_eq!(
        event.producer_revision.as_deref(),
        Some("migration_execution")
    );
}
