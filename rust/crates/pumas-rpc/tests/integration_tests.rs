//! Integration tests for the pumas-rpc JSON-RPC server.
//!
//! These tests verify that the RPC server correctly handles all API methods
//! and returns responses that match the expected TypeScript types.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;

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
    let json = rpc_call_raw(port, method, params).await?;
    if let Some(error) = json.get("error") {
        return Err(error.to_string());
    }
    Ok(json.get("result").cloned().unwrap_or(Value::Null))
}

/// Make an RPC call and return the full JSON-RPC payload.
async fn rpc_call_raw(port: u16, method: &str, params: Value) -> Result<Value, String> {
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

    response.json::<Value>().await.map_err(|e| e.to_string())
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

struct RpcServerHandle {
    child: tokio::process::Child,
    port: u16,
    stdout_drain: Option<tokio::task::JoinHandle<()>>,
}

impl RpcServerHandle {
    async fn stop(mut self) {
        if let Some(drain) = self.stdout_drain.take() {
            drain.abort();
        }
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

impl Drop for RpcServerHandle {
    fn drop(&mut self) {
        if let Some(drain) = self.stdout_drain.take() {
            drain.abort();
        }
        let _ = self.child.start_kill();
    }
}

/// Start the RPC binary and wait until `/health` is ready.
async fn start_rpc_server(launcher_root: &std::path::Path) -> Result<RpcServerHandle, String> {
    let binary = if let Ok(path) = std::env::var("CARGO_BIN_EXE_pumas-rpc") {
        PathBuf::from(path)
    } else {
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("failed to resolve current_exe for fallback: {e}"))?;
        let target_debug_dir = current_exe
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| "failed to resolve target/debug directory for fallback".to_string())?;

        let mut fallback = target_debug_dir.join("pumas-rpc");
        if cfg!(target_os = "windows") {
            fallback.set_extension("exe");
        }
        if !fallback.exists() {
            return Err(format!(
                "CARGO_BIN_EXE_pumas-rpc not set and fallback binary not found at {}",
                fallback.display()
            ));
        }
        fallback
    };

    let mut child = tokio::process::Command::new(&binary)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .arg("--launcher-root")
        .arg(launcher_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn pumas-rpc: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture stdout".to_string())?;
    let mut lines = tokio::io::BufReader::new(stdout).lines();

    let mut discovered_port: Option<u16> = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(250), lines.next_line()).await {
            Ok(Ok(Some(line))) => {
                if let Some(value) = line.strip_prefix("RPC_PORT=") {
                    let parsed = value
                        .trim()
                        .parse::<u16>()
                        .map_err(|e| format!("invalid RPC_PORT value '{value}': {e}"))?;
                    discovered_port = Some(parsed);
                    break;
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(err)) => return Err(format!("failed to read pumas-rpc stdout: {err}")),
            Err(_) => continue,
        }
    }

    let port =
        discovered_port.ok_or_else(|| "RPC_PORT line not emitted by pumas-rpc".to_string())?;
    if !wait_for_server(port, 15).await {
        return Err(format!("pumas-rpc failed health check on port {port}"));
    }

    let stdout_drain =
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });

    Ok(RpcServerHandle {
        child,
        port,
        stdout_drain: Some(stdout_drain),
    })
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
        "version",
        "deps_ready",
        "patched",
        "menu_shortcut",
        "desktop_shortcut",
        "message",
        "comfyui_running",
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

    let resources = response
        .get("resources")
        .ok_or("Missing 'resources' field")?;

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
    if response
        .get("isGitRepo")
        .and_then(|v| v.as_bool())
        .is_none()
    {
        return Err("Missing 'isGitRepo' field".into());
    }

    Ok(())
}

/// Validate SandboxInfoResponse structure
fn validate_sandbox_info_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    if response
        .get("is_sandboxed")
        .and_then(|v| v.as_bool())
        .is_none()
    {
        return Err("Missing 'is_sandboxed' field".into());
    }
    if response
        .get("sandbox_type")
        .and_then(|v| v.as_str())
        .is_none()
    {
        return Err("Missing 'sandbox_type' field".into());
    }
    if response
        .get("limitations")
        .and_then(|v| v.as_array())
        .is_none()
    {
        return Err("Missing 'limitations' field".into());
    }

    Ok(())
}

/// Validate NetworkStatusResponse structure
fn validate_network_status_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    let required_fields = [
        "total_requests",
        "successful_requests",
        "failed_requests",
        "circuit_breaker_rejections",
        "retries",
        "success_rate",
        "circuit_states",
        "is_offline",
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
    if response
        .get("deep_scan_in_progress")
        .and_then(|v| v.as_bool())
        .is_none()
    {
        return Err("Missing 'deep_scan_in_progress' field".into());
    }
    if response
        .get("model_count")
        .and_then(|v| v.as_i64())
        .is_none()
    {
        return Err("Missing 'model_count' field".into());
    }

    Ok(())
}

/// Validate LinkHealthResponse structure
fn validate_link_health_response(response: &Value) -> Result<(), String> {
    validate_base_response(response)?;

    let required_fields = [
        "status",
        "total_links",
        "healthy_links",
        "broken_links",
        "orphaned_links",
        "warnings",
        "errors",
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

    #[tokio::test]
    async fn test_migration_report_rpc_lifecycle() {
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;

        let dry_run = rpc_call(port, "generate_model_migration_dry_run_report", json!({}))
            .await
            .unwrap();
        assert_eq!(dry_run.get("success").and_then(|v| v.as_bool()), Some(true));
        let dry_run_report = dry_run.get("report").expect("missing report");
        assert!(dry_run_report
            .get("generated_at")
            .and_then(|v| v.as_str())
            .is_some());
        assert!(dry_run_report
            .get("machine_readable_report_path")
            .and_then(|v| v.as_str())
            .is_some());
        assert!(dry_run_report
            .get("human_readable_report_path")
            .and_then(|v| v.as_str())
            .is_some());

        let execution = rpc_call(port, "execute_model_migration", json!({}))
            .await
            .unwrap();
        assert_eq!(
            execution.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );
        let execution_report = execution.get("report").expect("missing execution report");
        assert!(execution_report
            .get("referential_integrity_ok")
            .and_then(|v| v.as_bool())
            .is_some());
        assert!(execution_report
            .get("referential_integrity_errors")
            .and_then(|v| v.as_array())
            .is_some());
        assert!(execution_report
            .get("reindexed_model_count")
            .and_then(|v| v.as_u64())
            .is_some());

        let listed = rpc_call(port, "list_model_migration_reports", json!({}))
            .await
            .unwrap();
        assert_eq!(listed.get("success").and_then(|v| v.as_bool()), Some(true));
        let reports = listed
            .get("reports")
            .and_then(|v| v.as_array())
            .expect("reports array missing");
        assert!(
            !reports.is_empty(),
            "expected at least one report artifact after dry-run/execution"
        );
        let first_report = reports[0].clone();
        let report_path = first_report
            .get("json_report_path")
            .and_then(|v| v.as_str())
            .expect("json_report_path missing")
            .to_string();

        let deleted = rpc_call(
            port,
            "delete_model_migration_report",
            json!({"reportPath": report_path}),
        )
        .await
        .unwrap();
        assert_eq!(deleted.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(deleted.get("removed").and_then(|v| v.as_bool()), Some(true));

        let pruned = rpc_call(
            port,
            "prune_model_migration_reports",
            json!({"keepLatest": 0}),
        )
        .await
        .unwrap();
        assert_eq!(pruned.get("success").and_then(|v| v.as_bool()), Some(true));
        assert!(pruned.get("removed").and_then(|v| v.as_u64()).is_some());
        assert_eq!(pruned.get("kept").and_then(|v| v.as_u64()), Some(0));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_migration_report_prune_rejects_negative_keep_latest() {
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;

        let payload = rpc_call_raw(
            port,
            "prune_model_migration_reports",
            json!({"keep_latest": -1}),
        )
        .await
        .unwrap();
        let error = payload
            .get("error")
            .expect("expected JSON-RPC error payload");
        assert_eq!(error.get("code").and_then(|v| v.as_i64()), Some(-32602));
        assert!(error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .contains("keep_latest must be >= 0"));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_sync_with_resolutions_rejects_invalid_action() {
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;

        let response = rpc_call(
            port,
            "sync_with_resolutions",
            json!({
                "versionTag": "v-test",
                "resolutions": {
                    "checkpoints/model.safetensors": "invalid_action"
                }
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            response.get("success").and_then(|v| v.as_bool()),
            Some(false)
        );
        let error = response
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(error.contains("Invalid conflict resolution action"));
        assert!(error.contains("skip, overwrite, rename"));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_get_cross_filesystem_warning_returns_structured_response() {
        let env = create_test_env();
        // Ensure the version path exists so cross-filesystem check can resolve metadata cleanly.
        std::fs::create_dir_all(env.path().join("comfyui-versions/v-test/models")).unwrap();

        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;

        let response = rpc_call(
            port,
            "get_cross_filesystem_warning",
            json!({
                "versionTag": "v-test"
            }),
        )
        .await
        .unwrap();

        assert!(response.get("success").and_then(|v| v.as_bool()).is_some());
        assert!(response
            .get("cross_filesystem")
            .and_then(|v| v.as_bool())
            .is_some());
        assert!(response
            .get("library_path")
            .and_then(|v| v.as_str())
            .is_some());
        assert!(response.get("app_path").and_then(|v| v.as_str()).is_some());

        server.stop().await;
    }

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
        let response = rpc_call(port, "get_system_resources", json!({}))
            .await
            .unwrap();
        validate_system_resources_response(&response)
            .expect("SystemResourcesResponse contract violation");

        // get_launcher_version
        let response = rpc_call(port, "get_launcher_version", json!({}))
            .await
            .unwrap();
        validate_launcher_version_response(&response)
            .expect("LauncherVersionResponse contract violation");

        // get_sandbox_info
        let response = rpc_call(port, "get_sandbox_info", json!({})).await.unwrap();
        validate_sandbox_info_response(&response).expect("SandboxInfoResponse contract violation");

        // get_network_status
        let response = rpc_call(port, "get_network_status", json!({}))
            .await
            .unwrap();
        validate_network_status_response(&response)
            .expect("NetworkStatusResponse contract violation");

        // get_library_status
        let response = rpc_call(port, "get_library_status", json!({}))
            .await
            .unwrap();
        validate_library_status_response(&response)
            .expect("LibraryStatusResponse contract violation");

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
        let response = rpc_call(port, "get_available_versions", json!({}))
            .await
            .unwrap();
        // Response is wrapped: { success: true, versions: [...] }
        assert!(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        assert!(response
            .get("versions")
            .and_then(|v| v.as_array())
            .is_some());

        // get_installed_versions should return an array
        let response = rpc_call(port, "get_installed_versions", json!({}))
            .await
            .unwrap();
        assert!(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        assert!(response
            .get("versions")
            .and_then(|v| v.as_array())
            .is_some());

        // get_active_version
        let response = rpc_call(port, "get_active_version", json!({}))
            .await
            .unwrap();
        assert!(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        // version can be null or a string

        // get_default_version
        let response = rpc_call(port, "get_default_version", json!({}))
            .await
            .unwrap();
        assert!(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
    }

    /// Test process management methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_process_methods() {
        let port = 9999;

        // is_comfyui_running should return a boolean
        let response = rpc_call(port, "is_comfyui_running", json!({}))
            .await
            .unwrap();
        assert!(response.is_boolean());
    }

    /// Test shortcut methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_shortcut_methods() {
        let port = 9999;

        // get_version_shortcuts needs a tag parameter
        let response = rpc_call(port, "get_version_shortcuts", json!({"tag": "v0.4.0"}))
            .await
            .unwrap();
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
        let response = rpc_call(
            port,
            "search_models_fts",
            json!({
                "query": "llama",
                "limit": 10
            }),
        )
        .await
        .unwrap();

        assert!(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        assert!(response.get("models").and_then(|v| v.as_array()).is_some());
        assert!(response
            .get("total_count")
            .and_then(|v| v.as_i64())
            .is_some());
        assert!(response
            .get("query_time_ms")
            .and_then(|v| v.as_i64())
            .is_some());
    }

    /// Test utility methods
    #[tokio::test]
    #[ignore] // Requires running server
    async fn test_utility_methods() {
        let port = 9999;

        // get_file_link_count
        let response = rpc_call(
            port,
            "get_file_link_count",
            json!({
                "file_path": "/tmp/nonexistent"
            }),
        )
        .await
        .unwrap();

        assert!(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
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
        let response1 = rpc_call(
            port,
            "get_available_versions",
            json!({
                "force_refresh": true
            }),
        )
        .await
        .unwrap();

        // Test with camelCase
        let response2 = rpc_call(
            port,
            "get_available_versions",
            json!({
                "forceRefresh": true
            }),
        )
        .await
        .unwrap();

        // Both should work
        assert!(response1
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        assert!(response2
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
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
