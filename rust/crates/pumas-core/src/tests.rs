use super::*;
use crate::model_library::{ModelLibrary, ModelMetadata};
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
    assert!(api
        .versions_dir(AppId::ComfyUI)
        .ends_with("comfyui-versions"));
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
