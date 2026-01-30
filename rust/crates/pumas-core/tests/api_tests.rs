//! Integration tests for the PumasApi public interface.
//!
//! These tests verify that the main API struct works correctly with
//! all its components initialized properly.
//!
//! Note: Version management tests have been moved to pumas-app-manager.
//! PumasApi now focuses on model library and system utilities.
//! Shortcut tests have been moved to pumas-rpc.

use pumas_library::{PumasApi, AppId};
use std::path::Path;
use tempfile::TempDir;

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
    let api = PumasApi::builder(temp_dir.path()).build().await;
    assert!(api.is_ok());
}

#[tokio::test]
async fn test_api_creation_with_auto_create_dirs() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    // Don't create directories manually - test auto_create_dirs
    let api = PumasApi::builder(temp_dir.path())
        .auto_create_dirs(true)
        .build()
        .await;
    assert!(api.is_ok());
}

#[tokio::test]
async fn test_api_creation_fails_for_nonexistent_path() {
    let result = PumasApi::builder("/nonexistent/path/that/does/not/exist")
        .build()
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_api_paths() {
    let temp_dir = create_test_env();
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Test path methods
    assert_eq!(api.launcher_root(), temp_dir.path());
    assert!(api.launcher_data_dir().ends_with("launcher-data"));
    assert!(api.metadata_dir().ends_with("metadata"));
    assert!(api.cache_dir().ends_with("cache"));
    assert!(api.versions_dir(AppId::ComfyUI).ends_with("comfyui-versions"));
    assert!(api.versions_dir(AppId::Ollama).ends_with("ollama-versions"));
}

#[tokio::test]
async fn test_get_status() {
    let temp_dir = create_test_env();
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    let status = api.get_status().await;
    assert!(status.is_ok());

    let status = status.unwrap();
    assert!(status.success);
    assert!(!status.version.is_empty());
    assert_eq!(status.message, "Ready");
}

#[tokio::test]
async fn test_get_disk_space() {
    let temp_dir = create_test_env();
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
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Opening a non-existent path should fail gracefully
    let result = api.open_path("/nonexistent/path/that/does/not/exist");
    // This may succeed or fail depending on the system,
    // but it should not panic
    let _ = result;
}

#[tokio::test]
async fn test_open_url_with_invalid_url() {
    let temp_dir = create_test_env();
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Opening an invalid URL should fail gracefully
    let result = api.open_url("not-a-valid-url");
    // This may succeed or fail depending on the system,
    // but it should not panic
    let _ = result;
}

#[tokio::test]
async fn test_open_directory_for_nonexistent_path() {
    let temp_dir = create_test_env();
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Opening non-existent directory should fail
    let result = api.open_directory(Path::new("/nonexistent/path"));
    assert!(result.is_err());
}

#[tokio::test]
async fn test_launch_version_for_nonexistent_version() {
    let temp_dir = create_test_env();
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
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // With no models, list should return empty
    let models = api.list_models().await;
    assert!(models.is_ok());
    assert!(models.unwrap().is_empty());
}

#[tokio::test]
async fn test_model_search_with_empty_library() {
    let temp_dir = create_test_env();
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Searching empty library should return empty results
    let result = api.search_models("test", 10, 0).await;
    assert!(result.is_ok());
    let search = result.unwrap();
    assert!(search.models.is_empty());
    assert_eq!(search.total_count, 0);
}

#[tokio::test]
async fn test_get_model_nonexistent() {
    let temp_dir = create_test_env();
    let api = PumasApi::builder(temp_dir.path()).build().await.unwrap();

    // Getting non-existent model should return None
    let model = api.get_model("nonexistent-model-id").await;
    assert!(model.is_ok());
    assert!(model.unwrap().is_none());
}
