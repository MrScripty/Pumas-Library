//! Integration tests for the pumas-rpc JSON-RPC server.
//!
//! These tests verify that the RPC server correctly handles all API methods
//! and returns responses that match the expected TypeScript types.

use serde_json::{json, Value};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

/// Find an available port for testing.
fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to port")
        .local_addr()
        .expect("Failed to get local address")
        .port()
}

/// Create a temporary directory with launcher-data structure.
fn create_test_env() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("launcher-data")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("comfyui-versions")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("shared-resources")).unwrap();

    temp_dir
}

/// Make an RPC call to the server.
async fn rpc_call(port: u16, method: &str, params: Value) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/rpc", port))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: Value = response.json().await.map_err(|e| e.to_string())?;

    if let Some(error) = json.get("error") {
        return Err(error.to_string());
    }

    Ok(json.get("result").cloned().unwrap_or(Value::Null))
}

/// Check health endpoint.
async fn check_health(port: u16) -> bool {
    let client = reqwest::Client::new();
    if let Ok(response) = client
        .get(format!("http://127.0.0.1:{}/health", port))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        if let Ok(json) = response.json::<Value>().await {
            return json.get("status").and_then(|v| v.as_str()) == Some("ok");
        }
    }
    false
}

/// Wait for server to be ready.
async fn wait_for_server(port: u16, timeout_secs: u64) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(timeout_secs) {
        if check_health(port).await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

// =============================================================================
// Response Structure Validators
// These functions verify that responses match the expected TypeScript types
// =============================================================================

/// Validate BaseResponse structure: { success: bool, error?: string }
fn validate_base_response(response: &Value) -> Result<(), String> {
    if response.get("success").and_then(|v| v.as_bool()).is_none() {
        return Err("Missing 'success' field".into());
    }
    // 'error' is optional
    Ok(())
}

/// Validate StatusResponse structure
fn validate_status_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    let required_fields = [
        "version", "deps_ready", "patched", "menu_shortcut",
        "desktop_shortcut", "message", "comfyui_running"
    ];

    for field in required_fields {
        if response.get(field).is_none() {
            return Err(format!("Missing field: {}", field));
        }
    }

    Ok(())
}

/// Validate DiskSpaceResponse structure
fn validate_disk_space_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    let required_fields = ["total", "used", "free", "percent"];
    for field in required_fields {
        if response.get(field).is_none() {
            return Err(format!("Missing field: {}", field));
        }
    }

    // Verify types
    if response.get("total").and_then(|v| v.as_u64()).is_none() {
        return Err("'total' must be a number".into());
    }
    if response.get("percent").and_then(|v| v.as_f64()).is_none() {
        return Err("'percent' must be a number".into());
    }

    Ok(())
}

/// Validate SystemResourcesResponse structure
fn validate_system_resources_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    let resources = response.get("resources").ok_or("Missing 'resources' field")?;

    // Check CPU
    let cpu = resources.get("cpu").ok_or("Missing 'resources.cpu'")?;
    if cpu.get("usage").and_then(|v| v.as_f64()).is_none() {
        return Err("Missing 'cpu.usage'".into());
    }

    // Check GPU
    let gpu = resources.get("gpu").ok_or("Missing 'resources.gpu'")?;
    if gpu.get("usage").and_then(|v| v.as_f64()).is_none() {
        return Err("Missing 'gpu.usage'".into());
    }

    // Check RAM
    let ram = resources.get("ram").ok_or("Missing 'resources.ram'")?;
    if ram.get("usage").and_then(|v| v.as_f64()).is_none() {
        return Err("Missing 'ram.usage'".into());
    }

    // Check Disk
    let disk = resources.get("disk").ok_or("Missing 'resources.disk'")?;
    if disk.get("usage").and_then(|v| v.as_f64()).is_none() {
        return Err("Missing 'disk.usage'".into());
    }

    Ok(())
}

/// Validate LauncherVersionResponse structure
fn validate_launcher_version_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    if response.get("version").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'version' field".into());
    }
    if response.get("branch").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'branch' field".into());
    }
    if response.get("isGitRepo").and_then(|v| v.as_bool()).is_none() {
        return Err("Missing 'isGitRepo' field".into());
    }

    Ok(())
}

/// Validate SandboxInfoResponse structure
fn validate_sandbox_info_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    if response.get("is_sandboxed").and_then(|v| v.as_bool()).is_none() {
        return Err("Missing 'is_sandboxed' field".into());
    }
    if response.get("sandbox_type").and_then(|v| v.as_str()).is_none() {
        return Err("Missing 'sandbox_type' field".into());
    }
    if response.get("limitations").and_then(|v| v.as_array()).is_none() {
        return Err("Missing 'limitations' field".into());
    }

    Ok(())
}

/// Validate NetworkStatusResponse structure
fn validate_network_status_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    let required_fields = [
        "total_requests", "successful_requests", "failed_requests",
        "circuit_breaker_rejections", "retries", "success_rate",
        "circuit_states", "is_offline"
    ];

    for field in required_fields {
        if response.get(field).is_none() {
            return Err(format!("Missing field: {}", field));
        }
    }

    Ok(())
}

/// Validate LibraryStatusResponse structure
fn validate_library_status_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    if response.get("indexing").and_then(|v| v.as_bool()).is_none() {
        return Err("Missing 'indexing' field".into());
    }
    if response.get("deep_scan_in_progress").and_then(|v| v.as_bool()).is_none() {
        return Err("Missing 'deep_scan_in_progress' field".into());
    }
    if response.get("model_count").and_then(|v| v.as_i64()).is_none() {
        return Err("Missing 'model_count' field".into());
    }

    Ok(())
}

/// Validate LinkHealthResponse structure
fn validate_link_health_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    let required_fields = [
        "status", "total_links", "healthy_links", "broken_links",
        "orphaned_links", "warnings", "errors"
    ];

    for field in required_fields {
        if response.get(field).is_none() {
            return Err(format!("Missing field: {}", field));
        }
    }

    Ok(())
}

// =============================================================================
// Integration Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require the RPC server to be running.
    // In CI, you would start the server as part of the test setup.
    // For local development, run: cargo run --release -- --port <port> --launcher_root <path>

    /// Test that validates all response types match TypeScript definitions.
    /// This is a contract test that ensures compatibility with the frontend.
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_response_contracts() {
        let port = 9999; // Use a fixed port for manual testing

        // get_status
        let response = rpc_call(port, "get_status", json!({})).await.unwrap();
        validate_status_response(&response).expect("StatusResponse contract violation");

        // get_disk_space
        let response = rpc_call(port, "get_disk_space", json!({})).await.unwrap();
        validate_disk_space_response(&response).expect("DiskSpaceResponse contract violation");

        // get_system_resources
        let response = rpc_call(port, "get_system_resources", json!({})).await.unwrap();
        validate_system_resources_response(&response).expect("SystemResourcesResponse contract violation");

        // get_launcher_version
        let response = rpc_call(port, "get_launcher_version", json!({})).await.unwrap();
        validate_launcher_version_response(&response).expect("LauncherVersionResponse contract violation");

        // get_sandbox_info
        let response = rpc_call(port, "get_sandbox_info", json!({})).await.unwrap();
        validate_sandbox_info_response(&response).expect("SandboxInfoResponse contract violation");

        // get_network_status
        let response = rpc_call(port, "get_network_status", json!({})).await.unwrap();
        validate_network_status_response(&response).expect("NetworkStatusResponse contract violation");

        // get_library_status
        let response = rpc_call(port, "get_library_status", json!({})).await.unwrap();
        validate_library_status_response(&response).expect("LibraryStatusResponse contract violation");

        // get_link_health
        let response = rpc_call(port, "get_link_health", json!({})).await.unwrap();
        validate_link_health_response(&response).expect("LinkHealthResponse contract violation");
    }

    /// Test health check endpoint
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_health_check() {
        let port = 9999;

        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .send()
            .await
            .expect("Failed to make health check request");

        assert!(response.status().is_success());

        let json: Value = response.json().await.expect("Failed to parse response");
        assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("ok"));
    }

    /// Test health_check RPC method
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_health_check_rpc() {
        let port = 9999;

        let response = rpc_call(port, "health_check", json!({})).await.unwrap();
        assert_eq!(response.get("status").and_then(|v| v.as_str()), Some("ok"));
    }

    /// Test version management methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_version_methods() {
        let port = 9999;

        // get_available_versions should return an array
        let response = rpc_call(port, "get_available_versions", json!({})).await.unwrap();
        // Response is wrapped: { success: true, versions: [...] }
        assert!(response.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(response.get("versions").and_then(|v| v.as_array()).is_some());

        // get_installed_versions should return an array
        let response = rpc_call(port, "get_installed_versions", json!({})).await.unwrap();
        assert!(response.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(response.get("versions").and_then(|v| v.as_array()).is_some());

        // get_active_version
        let response = rpc_call(port, "get_active_version", json!({})).await.unwrap();
        assert!(response.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
        // version can be null or a string

        // get_default_version
        let response = rpc_call(port, "get_default_version", json!({})).await.unwrap();
        assert!(response.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    /// Test process management methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_process_methods() {
        let port = 9999;

        // is_comfyui_running should return a boolean
        let response = rpc_call(port, "is_comfyui_running", json!({})).await.unwrap();
        assert!(response.is_boolean());
    }

    /// Test shortcut methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_shortcut_methods() {
        let port = 9999;

        // get_version_shortcuts needs a tag parameter
        let response = rpc_call(port, "get_version_shortcuts", json!({"tag": "v0.4.0"})).await.unwrap();
        // Should have tag, menu, desktop fields
        assert!(response.get("tag").is_some());
        assert!(response.get("menu").is_some());
        assert!(response.get("desktop").is_some());
    }

    /// Test model library methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_model_methods() {
        let port = 9999;

        // search_models_fts
        let response = rpc_call(port, "search_models_fts", json!({
            "query": "llama",
            "limit": 10
        })).await.unwrap();

        assert!(response.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(response.get("models").and_then(|v| v.as_array()).is_some());
        assert!(response.get("total_count").and_then(|v| v.as_i64()).is_some());
        assert!(response.get("query_time_ms").and_then(|v| v.as_i64()).is_some());
    }

    /// Test utility methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_utility_methods() {
        let port = 9999;

        // get_file_link_count
        let response = rpc_call(port, "get_file_link_count", json!({
            "file_path": "/tmp/nonexistent"
        })).await.unwrap();

        assert!(response.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(response.get("count").and_then(|v| v.as_i64()).is_some());
    }

    /// Test JSON-RPC 2.0 error handling
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_error_handling() {
        let port = 9999;

        // Call a non-existent method
        let result = rpc_call(port, "nonexistent_method", json!({})).await;
        assert!(result.is_err());

        // Call with missing required parameter
        let result = rpc_call(port, "switch_version", json!({})).await;
        assert!(result.is_err());
    }

    /// Test parameter variations (snake_case vs camelCase)
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_parameter_variants() {
        let port = 9999;

        // Test with snake_case
        let response1 = rpc_call(port, "get_available_versions", json!({
            "force_refresh": true
        })).await.unwrap();

        // Test with camelCase
        let response2 = rpc_call(port, "get_available_versions", json!({
            "forceRefresh": true
        })).await.unwrap();

        // Both should work
        assert!(response1.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(response2.get("success").and_then(|v| v.as_bool()).unwrap_or(false));
    }
}

// =============================================================================
// Test Runner for Manual Testing
// =============================================================================

/// Run this with: cargo test --package pumas-rpc --test integration_tests -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn run_all_contract_tests() {
    let port = std::env::var("TEST_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9999);

    println!("Running contract tests against port {}", port);

    // Wait for server
    if !wait_for_server(port, 10).await {
        panic!("Server not available on port {}", port);
    }

    println!("Server is ready");

    // Run all validations
    let tests = [
        ("get_status", json!({})),
        ("get_disk_space", json!({})),
        ("get_system_resources", json!({})),
        ("get_launcher_version", json!({})),
        ("get_sandbox_info", json!({})),
        ("get_network_status", json!({})),
        ("get_library_status", json!({})),
        ("get_link_health", json!({})),
        ("get_available_versions", json!({})),
        ("get_installed_versions", json!({})),
        ("get_active_version", json!({})),
        ("get_default_version", json!({})),
        ("is_comfyui_running", json!({})),
        ("has_background_fetch_completed", json!({})),
        ("get_github_cache_status", json!({})),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (method, params) in tests {
        match rpc_call(port, method, params).await {
            Ok(response) => {
                println!("✓ {} returned: {:?}", method, response);
                passed += 1;
            }
            Err(e) => {
                println!("✗ {} failed: {}", method, e);
                failed += 1;
            }
        }
    }

    println!("\nResults: {} passed, {} failed", passed, failed);
    assert_eq!(failed, 0, "Some tests failed");
}
