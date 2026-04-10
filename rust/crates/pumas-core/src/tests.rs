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
async fn test_new_returns_client_for_existing_primary() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let primary = PumasApi::new(temp_dir.path()).await.unwrap();
    assert!(primary.is_primary());

    let client = PumasApi::new(temp_dir.path()).await.unwrap();
    assert!(!client.is_primary());
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
async fn test_discover_returns_working_client_for_basic_ipc_methods() {
    let temp_dir = TempDir::new().unwrap();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let _primary = PumasApi::new(temp_dir.path()).await.unwrap();

    let client = PumasApi::discover().await.unwrap();
    assert!(!client.is_primary());

    let models = tokio::time::timeout(std::time::Duration::from_secs(10), client.list_models())
        .await
        .expect("list_models timed out")
        .unwrap();
    assert!(models.is_empty());

    let search = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.search_models("", 10, 0),
    )
    .await
    .expect("search_models timed out")
    .unwrap();
    assert!(search.models.is_empty());

    let status = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get_library_status(),
    )
    .await
    .expect("get_library_status timed out")
    .unwrap();
    assert!(status.success);

    let processes = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get_running_processes(),
    )
    .await
    .expect("get_running_processes timed out");
    assert!(processes.is_empty());

    let _ = client.is_online();
    let _ = client.list_conversions();

    let disk = tokio::time::timeout(std::time::Duration::from_secs(10), client.get_disk_space())
        .await
        .expect("get_disk_space timed out")
        .unwrap();
    assert!(disk.success);
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
        .join("shared-resources/models/llm/catplusplus/qwen-image-2512-heretic")
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
        .join("shared-resources/models/llm/catplusplus/qwen-image-2512-heretic")
        .exists());
}
