//! Integration tests for the PumasApi public interface.
//!
//! These tests verify that the main API struct works correctly with
//! all its components initialized properly.
//!
//! Note: Version management tests have been moved to pumas-app-manager.
//! PumasApi now focuses on model library and system utilities.
//! Shortcut tests have been moved to pumas-rpc.

#![warn(unsafe_code)]

use pumas_library::models::{
    ModelArtifactState, ModelEntryPathState, ModelFactFamily, ModelLibraryChangeKind,
    ModelLibraryRefreshScope, ModelLibrarySelectorSnapshotRequest, ModelPackageFactsSummaryStatus,
    RuntimeEndpointUrl, RuntimeLifecycleState, RuntimeManagementMode, RuntimePort,
    RuntimeProfileConfig, RuntimeProfileId, RuntimeProviderId, RuntimeProviderMode,
};
use pumas_library::{AppId, PumasApi};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;
use walkdir::WalkDir;

static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct RegistryTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl RegistryTestGuard {
    #[allow(unsafe_code)]
    fn new(root: &std::path::Path) -> Self {
        let lock = REGISTRY_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("registry test lock poisoned");

        let registry_path = root.join("registry-test").join("registry.db");

        // SAFETY: Integration tests in this binary serialize registry override
        // access with a process-wide mutex, so no concurrent environment
        // mutation occurs while the override is set.
        unsafe {
            std::env::set_var("PUMAS_REGISTRY_DB_PATH", &registry_path);
        }

        Self { _lock: lock }
    }
}

impl Drop for RegistryTestGuard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: Guarded by the same process-wide mutex as set_var above.
        unsafe {
            std::env::remove_var("PUMAS_REGISTRY_DB_PATH");
        }
    }
}

/// Create a test environment with proper directory structure.
fn create_test_env() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("launcher-data")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("launcher-data/logs")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("comfyui-versions")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("shared-resources")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("shared-resources/models")).unwrap();

    temp_dir
}

#[tokio::test]
async fn test_api_creation_succeeds() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await;
    assert!(api.is_ok());
}

#[tokio::test]
async fn test_api_creation_with_auto_create_dirs() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let _registry = RegistryTestGuard::new(temp_dir.path());
    // Don't create directories manually - test auto_create_dirs
    let api = PumasApi::builder(temp_dir.path())
        .auto_create_dirs(true)
        .build()
        .await;
    assert!(api.is_ok());
}

#[tokio::test]
async fn test_api_creation_fails_for_nonexistent_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let result = PumasApi::builder("/nonexistent/path/that/does/not/exist")
        .build()
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_api_paths() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Test path methods
    assert_eq!(api.launcher_root(), temp_dir.path());
    assert!(api.launcher_data_dir().ends_with("launcher-data"));
    assert!(api.metadata_dir().ends_with("metadata"));
    assert!(api.cache_dir().ends_with("cache"));
    assert!(api
        .versions_dir(AppId::ComfyUI)
        .ends_with("comfyui-versions"));
    assert!(api.versions_dir(AppId::Ollama).ends_with("ollama-versions"));
}

#[tokio::test]
async fn test_get_status() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let status = api.get_status().await;
    assert!(status.is_ok());

    let status = status.unwrap();
    assert!(status.success);
    assert!(!status.version.is_empty());
    assert_eq!(status.message, "Ready");
}

#[tokio::test]
async fn test_runtime_profile_snapshot_preserves_singleton_ollama_status() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let snapshot = api.get_runtime_profiles_snapshot().await.unwrap();
    assert!(snapshot.success);
    assert_eq!(snapshot.snapshot.profiles.len(), 1);
    assert_eq!(
        snapshot
            .snapshot
            .default_profile_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("ollama-default")
    );

    let singleton_ollama_running = api.is_ollama_running().await;
    let status = api.get_status().await.unwrap();

    assert!(status.success);
    assert_eq!(status.ollama_running, singleton_ollama_running);
}

#[tokio::test]
async fn test_launch_runtime_profile_reports_profile_scoped_failure() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let mut profile = RuntimeProfileConfig::default_ollama();
    profile.profile_id = RuntimeProfileId::parse("ollama-test-profile").unwrap();
    profile.name = "Ollama Test Profile".to_string();
    profile.endpoint_url = None;
    profile.port = RuntimePort::parse(12555).ok();
    api.upsert_runtime_profile(profile).await.unwrap();

    let version_dir = temp_dir.path().join("ollama-versions").join("missing-bin");
    std::fs::create_dir_all(&version_dir).unwrap();
    let response = api
        .launch_runtime_profile(
            RuntimeProfileId::parse("ollama-test-profile").unwrap(),
            "missing-bin",
            &version_dir,
        )
        .await
        .unwrap();

    assert!(!response.success);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("Binary not found"));

    let snapshot = api.get_runtime_profiles_snapshot().await.unwrap();
    let status = snapshot
        .snapshot
        .statuses
        .iter()
        .find(|status| status.profile_id.as_str() == "ollama-test-profile")
        .unwrap();
    assert_eq!(status.state, RuntimeLifecycleState::Failed);
    assert!(status
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("Binary not found"));
}

#[tokio::test]
async fn test_launch_llama_cpp_router_profile_reports_profile_scoped_failure() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let profile = RuntimeProfileConfig {
        profile_id: RuntimeProfileId::parse("llama-router-test").unwrap(),
        provider: RuntimeProviderId::LlamaCpp,
        provider_mode: RuntimeProviderMode::LlamaCppRouter,
        management_mode: RuntimeManagementMode::Managed,
        name: "llama.cpp Router Test".to_string(),
        enabled: true,
        endpoint_url: RuntimeEndpointUrl::parse("http://127.0.0.1:18080").ok(),
        port: RuntimePort::parse(18080).ok(),
        device: Default::default(),
        scheduler: Default::default(),
    };
    api.upsert_runtime_profile(profile).await.unwrap();

    let version_dir = temp_dir.path().join("launcher-data").join("llama-cpp");
    std::fs::create_dir_all(&version_dir).unwrap();
    let response = api
        .launch_runtime_profile(
            RuntimeProfileId::parse("llama-router-test").unwrap(),
            "local-build",
            &version_dir,
        )
        .await
        .unwrap();

    assert!(!response.success);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("llama-server"));
    let preset_path = temp_dir
        .path()
        .join("launcher-data/runtime-profiles/llama-cpp/llama-router-test/models-preset.ini");
    let preset = std::fs::read_to_string(&preset_path).unwrap();
    assert!(preset.contains("[*]\nload-on-startup = false"));

    let snapshot = api.get_runtime_profiles_snapshot().await.unwrap();
    let status = snapshot
        .snapshot
        .statuses
        .iter()
        .find(|status| status.profile_id.as_str() == "llama-router-test")
        .unwrap();
    assert_eq!(status.state, RuntimeLifecycleState::Failed);
    assert!(status
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("llama-server"));
}

#[tokio::test]
async fn test_launch_llama_cpp_dedicated_profile_requires_model_binding() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let profile = RuntimeProfileConfig {
        profile_id: RuntimeProfileId::parse("llama-dedicated-test").unwrap(),
        provider: RuntimeProviderId::LlamaCpp,
        provider_mode: RuntimeProviderMode::LlamaCppDedicated,
        management_mode: RuntimeManagementMode::Managed,
        name: "llama.cpp Dedicated Test".to_string(),
        enabled: true,
        endpoint_url: RuntimeEndpointUrl::parse("http://127.0.0.1:18081").ok(),
        port: RuntimePort::parse(18081).ok(),
        device: Default::default(),
        scheduler: Default::default(),
    };
    api.upsert_runtime_profile(profile).await.unwrap();

    let model_dir = temp_dir
        .path()
        .join("shared-resources/models/llm/test/dedicated-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    let model_path = model_dir.join("dedicated-model.gguf");
    std::fs::write(&model_path, b"gguf").unwrap();

    let version_dir = temp_dir.path().join("launcher-data").join("llama-cpp");
    std::fs::create_dir_all(&version_dir).unwrap();
    let response = api
        .launch_runtime_profile_for_model(
            RuntimeProfileId::parse("llama-dedicated-test").unwrap(),
            "local-build",
            &version_dir,
            Some("llm/test/dedicated-model"),
        )
        .await
        .unwrap();

    assert!(!response.success);
    assert!(response
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("llama-server"));

    let snapshot = api.get_runtime_profiles_snapshot().await.unwrap();
    let status = snapshot
        .snapshot
        .statuses
        .iter()
        .find(|status| status.profile_id.as_str() == "llama-dedicated-test")
        .unwrap();
    assert_eq!(status.state, RuntimeLifecycleState::Failed);
}

#[tokio::test]
async fn test_stop_runtime_profile_without_pid_is_profile_scoped() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let mut profile = RuntimeProfileConfig::default_ollama();
    profile.profile_id = RuntimeProfileId::parse("ollama-stop-profile").unwrap();
    profile.name = "Ollama Stop Profile".to_string();
    profile.endpoint_url = None;
    profile.port = RuntimePort::parse(12556).ok();
    api.upsert_runtime_profile(profile).await.unwrap();

    let stopped = api
        .stop_runtime_profile(RuntimeProfileId::parse("ollama-stop-profile").unwrap())
        .await
        .unwrap();

    assert!(!stopped);
    let snapshot = api.get_runtime_profiles_snapshot().await.unwrap();
    let status = snapshot
        .snapshot
        .statuses
        .iter()
        .find(|status| status.profile_id.as_str() == "ollama-stop-profile")
        .unwrap();
    assert_eq!(status.state, RuntimeLifecycleState::Stopped);
    assert!(status.last_error.is_none());
}

#[tokio::test]
async fn test_get_disk_space() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let disk_space = api.get_disk_space().await;
    assert!(disk_space.is_ok());

    let disk_space = disk_space.unwrap();
    assert!(disk_space.success);
    assert!(disk_space.total > 0);
    assert!(disk_space.free <= disk_space.total);
    assert!(disk_space.used <= disk_space.total);
    assert!(disk_space.percent >= 0.0 && disk_space.percent <= 100.0);
}

#[tokio::test]
async fn test_get_system_resources() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let resources = api.get_system_resources().await;
    assert!(resources.is_ok());

    let resources = resources.unwrap();
    assert!(resources.success);
    assert!(resources.resources.cpu.usage >= 0.0 && resources.resources.cpu.usage <= 100.0);
    assert!(resources.resources.ram.usage >= 0.0 && resources.resources.ram.usage <= 100.0);
    assert!(resources.resources.disk.usage >= 0.0 && resources.resources.disk.usage <= 100.0);
    assert!(resources.resources.ram.total > 0);
}

#[tokio::test]
async fn test_background_fetch_flag() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Initially should be false
    assert!(!api.has_background_fetch_completed().await);

    // Reset should work
    api.reset_background_fetch_flag().await;
    assert!(!api.has_background_fetch_completed().await);
}

#[tokio::test]
async fn test_process_methods() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // ComfyUI should not be running in test environment
    let running = api.is_comfyui_running().await;
    assert!(!running);

    // Getting running processes should return empty
    let processes = api.get_running_processes().await;
    assert!(processes.is_empty());
}

#[tokio::test]
async fn test_open_path_with_invalid_path() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Opening a non-existent path should fail gracefully
    let result = api.open_path("/nonexistent/path/that/does/not/exist").await;
    // This may succeed or fail depending on the system,
    // but it should not panic
    let _ = result;
}

#[tokio::test]
async fn test_open_url_with_invalid_url() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Opening an invalid URL should fail gracefully
    let result = api.open_url("not-a-valid-url").await;
    // This may succeed or fail depending on the system,
    // but it should not panic
    let _ = result;
}

#[tokio::test]
async fn test_open_directory_for_nonexistent_path() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Opening non-existent directory should fail
    let result = api.open_directory(Path::new("/nonexistent/path")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_launch_version_for_nonexistent_version() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Create a non-existent version directory path
    let version_dir = temp_dir.path().join("comfyui-versions/v999.999.999");

    // Launching non-installed version should return error response
    let result = api.launch_version("v999.999.999", &version_dir).await;
    assert!(result.is_ok());
    let launch = result.unwrap();
    assert!(!launch.success);
    assert!(launch.error.is_some());
}

#[tokio::test]
async fn test_model_library_list_with_empty_library() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // With no models, list should return empty
    let models = api.list_models().await;
    assert!(models.is_ok());
    assert!(models.unwrap().is_empty());
}

#[tokio::test]
async fn test_model_search_with_empty_library() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Searching empty library should return empty results
    let result = api.search_models("test", 10, 0).await;
    assert!(result.is_ok());
    let search = result.unwrap();
    assert!(search.models.is_empty());
    assert_eq!(search.total_count, 0);
}

#[tokio::test]
async fn test_api_creation_clean_startup_remains_idle() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();

    let models_root = temp_dir.path().join("shared-resources").join("models");
    let db_path = models_root.join("models.db");
    let wal_path = models_root.join("models.db-wal");
    let shm_path = models_root.join("models.db-shm");

    let db_modified_before = std::fs::metadata(&db_path).unwrap().modified().unwrap();
    let wal_modified_before = std::fs::metadata(&wal_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    let shm_modified_before = std::fs::metadata(&shm_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let db_modified_after = std::fs::metadata(&db_path).unwrap().modified().unwrap();
    let wal_modified_after = std::fs::metadata(&wal_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    let shm_modified_after = std::fs::metadata(&shm_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());

    assert_eq!(db_modified_before, db_modified_after);
    assert_eq!(wal_modified_before, wal_modified_after);
    assert_eq!(shm_modified_before, shm_modified_after);
    assert_eq!(
        WalkDir::new(&models_root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.file_name() == "metadata.json")
            .count(),
        0
    );

    drop(api);
}

#[tokio::test]
async fn test_get_model_nonexistent() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Getting non-existent model should return None
    let model = api.get_model("nonexistent-model-id").await;
    assert!(model.is_ok());
    assert!(model.unwrap().is_none());
}

#[tokio::test]
async fn test_get_inference_settings_applies_qwen_diffusion_overrides() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let model_id = "diffusion/qwen/qwen-image-2512";
    let model_dir = temp_dir
        .path()
        .join("shared-resources/models")
        .join(model_id);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.safetensors"), b"test").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": "Qwen",
            "model_type": "diffusion",
            "official_name": "Qwen-Image-2512",
            "cleaned_name": "qwen-image-2512",
            "repo_id": "Qwen/Qwen-Image-2512",
            "inference_settings": null
        }))
        .unwrap(),
    )
    .unwrap();

    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();
    let settings = api.get_inference_settings(model_id).await.unwrap();
    let keys: Vec<&str> = settings
        .iter()
        .map(|setting| setting.key.as_str())
        .collect();

    assert!(keys.contains(&"num_inference_steps"));
    assert!(keys.contains(&"true_cfg_scale"));
    assert!(keys.contains(&"width"));
    assert!(keys.contains(&"height"));
    assert!(keys.contains(&"seed"));
    assert!(!keys.contains(&"guidance_scale"));
}

#[tokio::test]
async fn test_resolve_model_package_facts_is_lazy_api_surface() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let model_id = "llm/test/lazy-package-facts";
    let model_dir = temp_dir
        .path()
        .join("shared-resources/models")
        .join(model_id);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(
        model_dir.join("config.json"),
        r#"{"model_type":"llama","architectures":["LlamaForCausalLM"]}"#,
    )
    .unwrap();
    std::fs::write(
        model_dir.join("generation_config.json"),
        r#"{"max_new_tokens":64}"#,
    )
    .unwrap();
    std::fs::write(model_dir.join("model.safetensors"), b"test").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": "test",
            "model_type": "llm",
            "official_name": "Lazy Package Facts",
            "cleaned_name": "lazy-package-facts",
            "files": [{"name": "model.safetensors"}],
            "runtime_engine_hints": ["transformers"]
        }))
        .unwrap(),
    )
    .unwrap();

    let api = PumasApi::builder(temp_dir.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();

    let facts = api.resolve_model_package_facts(model_id).await.unwrap();

    assert_eq!(facts.model_ref.model_id, model_id);
    assert!(facts.artifact.entry_path.ends_with("model.safetensors"));
    assert_eq!(
        facts
            .transformers
            .and_then(|evidence| evidence.config_model_type),
        Some("llama".to_string())
    );
    assert_eq!(
        facts.generation_defaults.source_path.as_deref(),
        Some("generation_config.json")
    );
}

#[tokio::test]
async fn test_model_library_update_feed_api_surface() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let model_id = "llm/test/update-feed";
    let model_dir = temp_dir
        .path()
        .join("shared-resources/models")
        .join(model_id);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(
        model_dir.join("config.json"),
        r#"{"model_type":"llama","architectures":["LlamaForCausalLM"]}"#,
    )
    .unwrap();
    std::fs::write(model_dir.join("model.safetensors"), b"test").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": "test",
            "model_type": "llm",
            "official_name": "Update Feed",
            "cleaned_name": "update-feed",
            "files": [{"name": "model.safetensors"}],
            "runtime_engine_hints": ["transformers"]
        }))
        .unwrap(),
    )
    .unwrap();

    let api = PumasApi::builder(temp_dir.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();

    let initial = api
        .list_model_library_updates_since(None, 100)
        .await
        .unwrap();
    assert!(!initial.stale_cursor);
    assert!(initial.events.is_empty());

    let snapshot = api
        .model_package_facts_summary_snapshot(100, 0)
        .await
        .unwrap();
    let item = snapshot
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("model should appear in summary snapshot");
    assert_eq!(item.status, ModelPackageFactsSummaryStatus::Missing);

    let summary = api
        .resolve_model_package_facts_summary(model_id)
        .await
        .unwrap();
    assert_eq!(summary.model_id, model_id);
    assert_eq!(summary.status, ModelPackageFactsSummaryStatus::Regenerated);
    assert!(summary.summary.is_some());

    let feed = api
        .list_model_library_updates_since(Some(&initial.cursor), 100)
        .await
        .unwrap();

    assert!(feed.events.iter().any(|event| {
        event.model_id == model_id
            && event.change_kind == ModelLibraryChangeKind::PackageFactsModified
            && event.fact_family == ModelFactFamily::PackageFacts
    }));

    let snapshot = api
        .model_package_facts_summary_snapshot(100, 0)
        .await
        .unwrap();
    let item = snapshot
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("model should appear in summary snapshot");
    assert_eq!(item.status, ModelPackageFactsSummaryStatus::Cached);
    assert!(item.summary.is_some());
}

#[tokio::test]
async fn test_model_library_selector_snapshot_is_direct_and_does_not_regenerate_facts() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let model_id = "llm/test/selector-direct";
    let model_dir = temp_dir
        .path()
        .join("shared-resources/models")
        .join(model_id);
    let entry_path = model_dir.join("model.safetensors");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("config.json"), r#"{"model_type":"llama"}"#).unwrap();
    std::fs::write(&entry_path, b"test").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": "test",
            "model_type": "llm",
            "official_name": "Selector Direct",
            "cleaned_name": "selector-direct",
            "repo_id": "example/selector-direct",
            "selected_artifact_id": "model.safetensors",
            "entry_path": entry_path.display().to_string(),
            "storage_kind": "library_owned",
            "validation_state": "valid",
            "task_type_primary": "text-generation",
            "runtime_engine_hints": ["transformers"],
            "files": [{"name": "model.safetensors"}]
        }))
        .unwrap(),
    )
    .unwrap();

    let api = PumasApi::builder(temp_dir.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();

    let selector = api
        .model_library_selector_snapshot(ModelLibrarySelectorSnapshotRequest {
            limit: Some(100),
            ..ModelLibrarySelectorSnapshotRequest::default()
        })
        .await
        .unwrap();
    let row = selector
        .rows
        .iter()
        .find(|row| row.model_id == model_id)
        .expect("model should appear in selector snapshot");

    assert_eq!(row.repo_id.as_deref(), Some("example/selector-direct"));
    assert_eq!(
        row.selected_artifact_id.as_deref(),
        Some("model.safetensors")
    );
    assert_eq!(row.entry_path_state, ModelEntryPathState::Ready);
    assert_eq!(row.artifact_state, ModelArtifactState::Ready);
    assert!(row.is_executable_reference_ready());
    assert_eq!(
        row.package_facts_summary_status,
        ModelPackageFactsSummaryStatus::Missing
    );

    let package_snapshot = api
        .model_package_facts_summary_snapshot(100, 0)
        .await
        .unwrap();
    let package_item = package_snapshot
        .items
        .iter()
        .find(|item| item.model_id == model_id)
        .expect("model should remain visible in package-facts snapshot");
    assert_eq!(package_item.status, ModelPackageFactsSummaryStatus::Missing);
}

#[tokio::test]
async fn test_list_models_reconciliation_advances_update_feed() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let api = PumasApi::builder(temp_dir.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();

    let initial = api
        .list_model_library_updates_since(None, 100)
        .await
        .unwrap();

    let model_id = "llm/llama/reconcile-feed";
    let model_dir = temp_dir
        .path()
        .join("shared-resources/models")
        .join(model_id);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(
        model_dir.join("config.json"),
        r#"{"model_type":"llama","architectures":["LlamaForCausalLM"]}"#,
    )
    .unwrap();
    std::fs::write(model_dir.join("model.safetensors"), b"test").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": "llama",
            "model_type": "llm",
            "official_name": "Reconcile Feed",
            "cleaned_name": "reconcile-feed",
            "files": [{"name": "model.safetensors"}],
            "runtime_engine_hints": ["transformers"]
        }))
        .unwrap(),
    )
    .unwrap();

    let models = api.list_models().await.unwrap();
    assert!(models.iter().any(|model| model.id == model_id));

    let feed = api
        .list_model_library_updates_since(Some(&initial.cursor), 100)
        .await
        .unwrap();
    assert!(feed.events.iter().any(|event| {
        event.model_id == model_id
            && event.change_kind == ModelLibraryChangeKind::ModelAdded
            && event.fact_family == ModelFactFamily::ModelRecord
            && event.refresh_scope == ModelLibraryRefreshScope::SummaryAndDetail
    }));
}

#[tokio::test]
async fn test_resolve_pumas_model_ref_api_surface() {
    let temp_dir = create_test_env();
    let _registry = RegistryTestGuard::new(temp_dir.path());
    let model_id = "llm/test/ref-api";
    let model_dir = temp_dir
        .path()
        .join("shared-resources/models")
        .join(model_id);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("config.json"), r#"{"model_type":"llama"}"#).unwrap();
    std::fs::write(model_dir.join("model.safetensors"), b"test").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": "test",
            "model_type": "llm",
            "official_name": "Ref API",
            "cleaned_name": "ref-api",
            "files": [{"name": "model.safetensors"}]
        }))
        .unwrap(),
    )
    .unwrap();

    let api = PumasApi::builder(temp_dir.path())
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap();

    let by_id = api.resolve_pumas_model_ref(model_id).await.unwrap();
    assert_eq!(by_id.model_id, model_id);
    assert!(by_id.migration_diagnostics.is_empty());

    let by_file = api
        .resolve_pumas_model_ref(
            model_dir
                .join("model.safetensors")
                .to_string_lossy()
                .as_ref(),
        )
        .await
        .unwrap();
    assert_eq!(by_file.model_id, model_id);
    assert!(by_file.migration_diagnostics.is_empty());

    let unresolved = api
        .resolve_pumas_model_ref("llm/test/missing")
        .await
        .unwrap();
    assert_eq!(unresolved.model_id, "");
    assert!(unresolved
        .migration_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unknown_model_id"));
}
