//! Integration tests for the PumasApi public interface.
//!
//! These tests verify that the main API struct works correctly with
//! all its components initialized properly.

use pumas_core::{PumasApi, AppId};
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

    temp_dir
}

#[tokio::test]
async fn test_api_creation_succeeds() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await;
    assert!(api.is_ok());
}

#[tokio::test]
async fn test_api_creation_fails_for_nonexistent_path() {
    let result = PumasApi::new("/nonexistent/path/that/does/not/exist").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_api_paths() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

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
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    let status = api.get_status().await;
    assert!(status.is_ok());

    let status = status.unwrap();
    assert!(status.success);
    assert!(!status.version.is_empty());
    assert_eq!(status.message, "Rust backend running");
}

#[tokio::test]
async fn test_get_disk_space() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

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
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

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
async fn test_version_methods_with_no_versions() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // With fresh install, there should be no versions
    let installed = api.get_installed_versions(None).await.unwrap();
    assert!(installed.is_empty());

    let active = api.get_active_version(None).await.unwrap();
    assert!(active.is_none());

    let default = api.get_default_version(None).await.unwrap();
    assert!(default.is_none());
}

#[tokio::test]
async fn test_background_fetch_flag() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Initially should be false
    assert!(!api.has_background_fetch_completed().await);

    // Reset should work
    api.reset_background_fetch_flag().await;
    assert!(!api.has_background_fetch_completed().await);
}

#[tokio::test]
async fn test_process_methods() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // ComfyUI should not be running in test environment
    let running = api.is_comfyui_running().await;
    assert!(!running);

    // Getting running processes should return empty
    let processes = api.get_running_processes().await;
    assert!(processes.is_empty());
}

#[tokio::test]
async fn test_shortcut_state_for_nonexistent_version() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    let state = api.get_version_shortcut_state("v0.0.0").await;
    assert_eq!(state.tag, "v0.0.0");
    assert!(!state.menu);
    assert!(!state.desktop);
}

#[tokio::test]
async fn test_open_path_with_invalid_path() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Opening a non-existent path should fail gracefully
    let result = api.open_path("/nonexistent/path/that/does/not/exist");
    // This may succeed or fail depending on the system,
    // but it should not panic
    let _ = result;
}

#[tokio::test]
async fn test_open_url_with_invalid_url() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Opening an invalid URL should fail gracefully
    let result = api.open_url("not-a-valid-url");
    // This may succeed or fail depending on the system,
    // but it should not panic
    let _ = result;
}

#[tokio::test]
async fn test_open_active_install_with_no_active_version() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // With no active version, this should return an error
    let result = api.open_active_install().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_validate_installations_empty() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    let result = api.validate_installations(None).await;
    assert!(result.is_ok());

    let validation = result.unwrap();
    assert_eq!(validation.valid_count, 0);
    assert!(validation.removed_tags.is_empty());
    assert!(validation.orphaned_dirs.is_empty());
}

#[tokio::test]
async fn test_cancel_installation_when_none_running() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Should return false when no installation is running
    let result = api.cancel_installation(None).await.unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_get_installation_progress_when_none_running() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    let progress = api.get_installation_progress(None).await;
    assert!(progress.is_none());
}

#[tokio::test]
async fn test_set_active_version_for_nonexistent_version() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Setting active version for non-installed version should fail
    let result = api.set_active_version("v999.999.999", None).await;
    // This should either fail or return false
    match result {
        Ok(success) => assert!(!success),
        Err(_) => {} // Expected
    }
}

#[tokio::test]
async fn test_remove_version_for_nonexistent_version() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Removing non-existent version should fail
    let result = api.remove_version("v999.999.999", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_launch_version_for_nonexistent_version() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Launching non-installed version should fail
    let result = api.launch_version("v999.999.999", None).await;
    assert!(result.is_ok());
    let launch = result.unwrap();
    assert!(!launch.success);
    assert!(launch.error.is_some());
}

#[tokio::test]
async fn test_toggle_menu_shortcut_for_nonexistent_version() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Toggling menu shortcut for non-installed version should fail
    let result = api.toggle_menu_shortcut("v999.999.999").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_toggle_desktop_shortcut_for_nonexistent_version() {
    let temp_dir = create_test_env();
    let api = PumasApi::new(temp_dir.path()).await.unwrap();

    // Toggling desktop shortcut for non-installed version should fail
    let result = api.toggle_desktop_shortcut("v999.999.999").await;
    assert!(result.is_err());
}
