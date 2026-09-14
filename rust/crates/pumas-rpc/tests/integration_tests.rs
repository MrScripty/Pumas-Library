//! Integration tests for the pumas-rpc JSON-RPC server.
//!
//! These tests verify that the RPC server correctly handles all API methods
//! and returns responses that match the expected TypeScript types.

use futures::StreamExt;
use serde_json::{json, Value};
use std::fmt::Display;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncRead};
use tokio::sync::Mutex;

const MAX_CAPTURED_DIAGNOSTIC_BYTES: usize = 64 * 1024;

/// Create a temporary directory with launcher-data structure.
fn create_test_env() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create required directories
    std::fs::create_dir_all(temp_dir.path().join("launcher-data")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("shared-resources")).unwrap();

    temp_dir
}

fn create_indexable_test_model(root: &std::path::Path, model_id: &str, official_name: &str) {
    let model_dir = root.join("shared-resources/models").join(model_id);
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
            "official_name": official_name,
            "cleaned_name": official_name.to_ascii_lowercase().replace(' ', "-"),
            "files": [{"name": "model.safetensors"}],
            "runtime_engine_hints": ["transformers"]
        }))
        .unwrap(),
    )
    .unwrap();
}

fn create_partial_test_model(root: &std::path::Path) -> &'static str {
    const MODEL_ID: &str = "llm/acme/partial-model";
    let model_dir = root.join("shared-resources/models").join(MODEL_ID);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("weights.gguf.part"), b"partial").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 2,
            "model_id": MODEL_ID,
            "family": "acme",
            "model_type": "llm",
            "official_name": "Partial Model",
            "cleaned_name": "partial-model",
            "repo_id": "acme/model",
            "match_source": "download_partial",
            "expected_files": ["weights.gguf"],
            "selected_artifact_id": "acme--model__weights-gguf",
            "selected_artifact_files": ["weights.gguf"],
            "selected_artifact_quant": "Q4_K_M",
            "size_bytes": 100
        }))
        .unwrap(),
    )
    .unwrap();
    let cache_dir = root.join("launcher-data/cache/hf");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(
        cache_dir.join("hf_acme_model_files.json"),
        serde_json::to_string_pretty(&json!({
            "repo_id": "acme/model",
            "lfs_files": [{
                "filename": "weights.gguf",
                "size": 100,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }],
            "regular_files": [],
            "cached_at": "2026-09-03T00:00:00Z",
            "last_modified": null,
            "cache_version": 2
        }))
        .unwrap(),
    )
    .unwrap();
    MODEL_ID
}

fn create_tracked_partial_test_model(root: &std::path::Path, status: &str) -> &'static str {
    let model_id = create_partial_test_model(root);
    let snapshot = serde_json::from_value(json!({
        "download_id": "tracked-partial-1",
        "repo_id": "acme/model",
        "revision": null,
        "filename": "weights.gguf",
        "filenames": ["weights.gguf"],
        "dest_dir": root.join("shared-resources/models").join(model_id),
        "total_bytes": 100,
        "status": status,
        "download_request": {
            "repo_id": "acme/model", "family": "acme", "official_name": "Partial Model",
            "model_type": "llm", "filenames": ["weights.gguf"]
        },
        "created_at": "2026-09-03T00:00:00Z"
    }))
    .unwrap();
    pumas_library::model_library::test_support::admit_paused_download(root, &snapshot).unwrap();
    model_id
}

fn create_untracked_partial_test_model(root: &std::path::Path) -> &'static str {
    let model_id = create_partial_test_model(root);
    let cache_dir = root.join("launcher-data/cache/hf");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(
        cache_dir.join("hf_acme_model_files.json"),
        serde_json::to_string_pretty(&json!({
            "repo_id": "acme/model",
            "lfs_files": [{
                "filename": "weights.gguf",
                "size": 100,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }],
            "regular_files": ["config.json"],
            "cached_at": "2026-09-03T00:00:00Z",
            "last_modified": null,
            "cache_version": 2
        }))
        .unwrap(),
    )
    .unwrap();
    model_id
}

fn create_untracked_partial_with_missing_remote_member(root: &std::path::Path) -> &'static str {
    let model_id = create_untracked_partial_test_model(root);
    let model_dir = root.join("shared-resources/models").join(model_id);
    std::fs::remove_file(model_dir.join("weights.gguf.part")).unwrap();
    std::fs::write(model_dir.join("weights-1.gguf.part"), b"partial").unwrap();
    let metadata_path = model_dir.join("metadata.json");
    let mut metadata: Value =
        serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
    metadata["expected_files"] = json!(["weights-1.gguf", "weights-2.gguf"]);
    metadata["selected_artifact_files"] = json!(["weights-1.gguf", "weights-2.gguf"]);
    std::fs::write(metadata_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
    let cache_path = root.join("launcher-data/cache/hf/hf_acme_model_files.json");
    let mut tree: Value = serde_json::from_slice(&std::fs::read(&cache_path).unwrap()).unwrap();
    tree["lfs_files"] = json!([{
        "filename": "weights-1.gguf",
        "size": 100,
        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }]);
    std::fs::write(cache_path, serde_json::to_vec_pretty(&tree).unwrap()).unwrap();
    model_id
}

#[cfg(feature = "inference-plugins")]
fn create_indexable_gguf_test_model(root: &std::path::Path, model_id: &str, official_name: &str) {
    let model_dir = root.join("shared-resources/models").join(model_id);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.gguf"), b"test").unwrap();
    std::fs::write(
        model_dir.join("metadata.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model_id": model_id,
            "family": "llama",
            "model_type": "llm",
            "official_name": official_name,
            "cleaned_name": official_name.to_ascii_lowercase().replace(' ', "-"),
            "files": [{"name": "model.gguf"}],
            "runtime_engine_hints": ["llama.cpp"]
        }))
        .unwrap(),
    )
    .unwrap();
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

async fn recovery_token_for_model(port: u16, model_id: &str) -> String {
    let pointer = format!(
        "/models/{}/artifact/recovery/recoveryToken",
        model_id.replace('~', "~0").replace('/', "~1")
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let models = rpc_call(port, "get_models", json!({})).await.unwrap();
        if let Some(token) = models.pointer(&pointer).and_then(Value::as_str) {
            return token.to_string();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "partial model never appeared with a recovery ticket: {models}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Send an exact request body to the real producer adapter.
async fn rpc_body_raw(port: u16, body: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/rpc"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("unexpected HTTP status {}", response.status()));
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())
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

async fn read_stream_until_contains<S, B, E>(
    stream: &mut S,
    pattern: &str,
    timeout: Duration,
) -> Result<String, String>
where
    S: futures::Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: Display,
{
    let deadline = tokio::time::Instant::now() + timeout;
    let mut body = String::new();

    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(chunk))) => {
                body.push_str(&String::from_utf8_lossy(chunk.as_ref()));
                if body.contains(pattern) {
                    return Ok(body);
                }
            }
            Ok(Some(Err(error))) => return Err(error.to_string()),
            Ok(None) => return Err("event stream ended before expected payload".to_string()),
            Err(_) => break,
        }
    }

    Err(format!(
        "timed out waiting for '{pattern}' in event stream body: {body}"
    ))
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
    stderr_drain: Option<tokio::task::JoinHandle<()>>,
    stdout: Arc<Mutex<String>>,
    stderr: Arc<Mutex<String>>,
}

impl RpcServerHandle {
    async fn stop(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        if let Some(drain) = self.stdout_drain.take() {
            let _ = drain.await;
        }
        if let Some(drain) = self.stderr_drain.take() {
            let _ = drain.await;
        }
    }

    async fn diagnostics(&self) -> String {
        let stdout = self.stdout.lock().await.clone();
        let stderr = self.stderr.lock().await.clone();
        format!("{stdout}\n{stderr}")
    }
}

impl Drop for RpcServerHandle {
    fn drop(&mut self) {
        if let Some(drain) = self.stdout_drain.take() {
            drain.abort();
        }
        if let Some(drain) = self.stderr_drain.take() {
            drain.abort();
        }
        let _ = self.child.start_kill();
    }
}

/// Start the RPC binary and wait until `/health` is ready.
async fn start_rpc_server(launcher_root: &std::path::Path) -> Result<RpcServerHandle, String> {
    start_rpc_server_with_debug(launcher_root, false).await
}

fn rpc_binary_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_pumas-rpc") {
        return Ok(PathBuf::from(path));
    }

    let current_exe = std::env::current_exe()
        .map_err(|error| format!("failed to resolve current_exe for fallback: {error}"))?;
    let target_debug_dir = current_exe
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| "failed to resolve target/debug directory for fallback".to_string())?;
    let mut fallback = target_debug_dir.join("pumas-rpc");
    if cfg!(target_os = "windows") {
        fallback.set_extension("exe");
    }
    if fallback.exists() {
        Ok(fallback)
    } else {
        Err(format!(
            "CARGO_BIN_EXE_pumas-rpc not set and fallback binary not found at {}",
            fallback.display()
        ))
    }
}

async fn append_bounded(capture: &Arc<Mutex<String>>, line: &str) {
    let mut captured = capture.lock().await;
    if captured.len() >= MAX_CAPTURED_DIAGNOSTIC_BYTES {
        return;
    }
    let remaining = MAX_CAPTURED_DIAGNOSTIC_BYTES - captured.len();
    let mut take = line.len().min(remaining);
    while !line.is_char_boundary(take) {
        take -= 1;
    }
    captured.push_str(&line[..take]);
    if captured.len() < MAX_CAPTURED_DIAGNOSTIC_BYTES {
        captured.push('\n');
    }
}

fn spawn_diagnostic_drain<R>(reader: R, capture: Arc<Mutex<String>>) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            append_bounded(&capture, &line).await;
        }
    })
}

/// Start the real RPC binary and retain bounded stdout/stderr diagnostics.
async fn start_rpc_server_with_debug(
    launcher_root: &std::path::Path,
    debug: bool,
) -> Result<RpcServerHandle, String> {
    let binary = rpc_binary_path()?;

    let mut command = tokio::process::Command::new(&binary);
    command
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .arg("--launcher-root")
        .arg(launcher_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if debug {
        command.arg("--debug");
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to spawn pumas-rpc: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture stdout".to_string())?;
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture stderr".to_string())?;
    let stdout_capture = Arc::new(Mutex::new(String::new()));
    let stderr_capture = Arc::new(Mutex::new(String::new()));
    let stderr_drain = spawn_diagnostic_drain(stderr, stderr_capture.clone());

    let mut discovered_port: Option<u16> = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(250), lines.next_line()).await {
            Ok(Ok(Some(line))) => {
                append_bounded(&stdout_capture, &line).await;
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

    let remaining_stdout_capture = stdout_capture.clone();
    let stdout_drain = tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            append_bounded(&remaining_stdout_capture, &line).await;
        }
    });

    Ok(RpcServerHandle {
        child,
        port,
        stdout_drain: Some(stdout_drain),
        stderr_drain: Some(stderr_drain),
        stdout: stdout_capture,
        stderr: stderr_capture,
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

    let required_fields = ["version", "message", "ollama_running", "torch_running"];

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
    use std::io::ErrorKind;

    fn can_bind_local_tcp_for_tests() -> bool {
        match std::net::TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => {
                drop(listener);
                true
            }
            Err(err)
                if err.kind() == ErrorKind::PermissionDenied || err.raw_os_error() == Some(1) =>
            {
                eprintln!(
                    "Skipping RPC integration test: local TCP bind not permitted ({})",
                    err
                );
                false
            }
            Err(err) => panic!("Unexpected TCP bind failure while preflighting tests: {err}"),
        }
    }

    #[cfg(feature = "inference-plugins")]
    fn missing_serving_request() -> Value {
        json!({
            "request": {
                "model_id": "llm/missing/serving-fixture",
                "config": {
                    "provider": "ollama",
                    "profile_id": "ollama-default",
                    "device_mode": "auto",
                    "keep_loaded": false
                }
            }
        })
    }

    #[cfg(feature = "inference-plugins")]
    #[tokio::test]
    async fn test_serving_rpc_methods_return_non_critical_domain_errors() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;

        let status = rpc_call(port, "get_serving_status", json!({}))
            .await
            .unwrap();
        assert_eq!(status.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            status
                .pointer("/snapshot/endpoint/endpoint_mode")
                .and_then(|v| v.as_str()),
            Some("not_configured")
        );
        let initial_cursor = status
            .pointer("/snapshot/cursor")
            .and_then(|v| v.as_str())
            .expect("serving snapshot cursor missing");

        let initial_feed = rpc_call(
            port,
            "list_serving_status_updates_since",
            json!({"cursor": initial_cursor}),
        )
        .await
        .unwrap();
        assert_eq!(
            initial_feed
                .pointer("/feed/snapshot_required")
                .and_then(|v| v.as_bool()),
            Some(false)
        );

        let validation = rpc_call(
            port,
            "validate_model_serving_config",
            missing_serving_request(),
        )
        .await
        .unwrap();
        assert_eq!(
            validation.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            validation.get("valid").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(validation
            .get("errors")
            .and_then(|v| v.as_array())
            .is_some_and(|errors| errors
                .iter()
                .any(
                    |error| error.get("code").and_then(|v| v.as_str()) == Some("model_not_found")
                )));

        let serve = rpc_call(port, "serve_model", missing_serving_request())
            .await
            .unwrap();
        assert_eq!(serve.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(serve.get("loaded").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            serve
                .get("loaded_models_unchanged")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            serve
                .pointer("/load_error/severity")
                .and_then(|v| v.as_str()),
            Some("non_critical")
        );
        let updated_feed = rpc_call(
            port,
            "list_serving_status_updates_since",
            json!({"cursor": initial_cursor}),
        )
        .await
        .unwrap();
        assert_eq!(
            updated_feed
                .pointer("/feed/snapshot_required")
                .and_then(|v| v.as_bool()),
            Some(true)
        );

        let client = reqwest::Client::new();
        let models = client
            .get(format!("http://127.0.0.1:{}/v1/models", port))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(models.get("object").and_then(|v| v.as_str()), Some("list"));
        assert!(models
            .get("data")
            .and_then(|v| v.as_array())
            .is_some_and(Vec::is_empty));

        let proxy_error = client
            .post(format!("http://127.0.0.1:{}/v1/chat/completions", port))
            .json(&json!({"model": "not-served", "messages": []}))
            .send()
            .await
            .unwrap();
        assert_eq!(proxy_error.status(), reqwest::StatusCode::NOT_FOUND);

        server.stop().await;
    }

    #[cfg(feature = "inference-plugins")]
    #[tokio::test]
    async fn test_serving_llama_cpp_missing_runtime_is_non_critical() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let model_id = "llm/llama/serving-gguf-fixture";
        create_indexable_gguf_test_model(env.path(), model_id, "Serving GGUF Fixture");
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;

        let listed = rpc_call(port, "get_models", json!({})).await.unwrap();
        assert!(listed
            .get("models")
            .and_then(|value| value.as_object())
            .is_some_and(|models| models.contains_key(model_id)));

        let profile_id = "llama-dedicated-serving-test";
        let upserted = rpc_call(
            port,
            "upsert_runtime_profile",
            json!({
                "profile": {
                    "profile_id": profile_id,
                    "provider": "llama_cpp",
                    "provider_mode": "llama_cpp_dedicated",
                    "management_mode": "managed",
                    "name": "llama.cpp Dedicated Serving Test",
                    "enabled": true,
                    "endpoint_url": "http://127.0.0.1:39191",
                    "port": 39191,
                    "device": {"mode": "auto"},
                    "scheduler": {"auto_load": true}
                }
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            upserted.get("success").and_then(|value| value.as_bool()),
            Some(true)
        );

        let served = rpc_call(
            port,
            "serve_model",
            json!({
                "request": {
                    "model_id": model_id,
                    "config": {
                        "provider": "llama_cpp",
                        "profile_id": profile_id,
                        "device_mode": "auto",
                        "keep_loaded": false
                    }
                }
            }),
        )
        .await
        .unwrap();

        assert_eq!(served.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(served.get("loaded").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            served
                .get("loaded_models_unchanged")
                .and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            served
                .pointer("/load_error/severity")
                .and_then(|v| v.as_str()),
            Some("non_critical")
        );
        assert!(matches!(
            served
                .pointer("/load_error/code")
                .and_then(|value| value.as_str()),
            Some("missing_runtime" | "provider_load_failed")
        ));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_migration_report_rpc_lifecycle() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
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

    #[cfg(feature = "inference-plugins")]
    #[tokio::test]
    async fn test_serving_status_update_event_stream_emits_initial_snapshot_required() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;
        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "http://127.0.0.1:{}/events/serving-status-updates",
                port
            ))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        assert!(response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")));
        let mut stream = response.bytes_stream();

        let body = read_stream_until_contains(
            &mut stream,
            "serving-status-update",
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert!(body.contains("event: serving-status-update"));
        assert!(body.contains("\"snapshot_required\":true"));
        assert!(body.contains("\"cursor\":\"serving:0\""));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_model_library_update_event_stream_emits_after_reconcile() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;
        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "http://127.0.0.1:{}/events/model-library-updates",
                port
            ))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        assert!(response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")));
        let mut stream = response.bytes_stream();

        // Allow the stream to establish its durable cursor before mutating the
        // library, otherwise an initial snapshot cursor could legitimately skip
        // the test event.
        tokio::time::sleep(Duration::from_millis(1200)).await;

        let model_id = "llm/llama/sse-reconcile-feed";
        create_indexable_test_model(env.path(), model_id, "SSE Reconcile Feed");

        let listed = rpc_call(port, "get_models", json!({})).await.unwrap();
        assert!(listed
            .get("models")
            .and_then(|value| value.as_object())
            .is_some_and(|models| models.contains_key(model_id)));

        let body = read_stream_until_contains(&mut stream, model_id, Duration::from_secs(10)).await;
        let body = body.unwrap();
        assert!(body.contains("event: model-library-update"));
        assert!(body.contains("\"change_kind\":\"model_added\""));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_model_library_update_event_stream_recovers_from_cursor() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;

        let initial_feed = rpc_call(
            port,
            "list_model_library_updates_since",
            json!({"cursor": null, "limit": 100}),
        )
        .await
        .unwrap();
        let cursor = initial_feed
            .get("cursor")
            .and_then(|value| value.as_str())
            .expect("initial update cursor missing")
            .to_string();

        let model_id = "llm/llama/sse-recovered-feed";
        create_indexable_test_model(env.path(), model_id, "SSE Recovered Feed");
        let listed = rpc_call(port, "get_models", json!({})).await.unwrap();
        assert!(listed
            .get("models")
            .and_then(|value| value.as_object())
            .is_some_and(|models| models.contains_key(model_id)));

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "http://127.0.0.1:{}/events/model-library-updates?cursor={}",
                port, cursor
            ))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let mut stream = response.bytes_stream();

        let body = read_stream_until_contains(&mut stream, model_id, Duration::from_secs(10)).await;
        let body = body.unwrap();
        assert!(body.contains("event: model-library-update"));
        assert!(body.contains("\"change_kind\":\"model_added\""));
        assert!(body.contains("\"snapshot_required\":false"));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_model_download_update_event_stream_emits_initial_snapshot() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();
        let port = server.port;
        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "http://127.0.0.1:{}/events/model-download-updates",
                port
            ))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        assert!(response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")));
        let mut stream = response.bytes_stream();

        let body = read_stream_until_contains(
            &mut stream,
            "model-download-update",
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert!(body.contains("event: model-download-update"));
        assert!(body.contains("\"snapshot\""));
        assert!(body.contains("\"downloads\""));

        server.stop().await;
    }

    #[tokio::test]
    async fn test_migration_report_prune_rejects_negative_keep_latest() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
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
        assert_eq!(
            error.get("message").and_then(Value::as_str),
            Some("Request parameters are invalid.")
        );
        assert_eq!(
            error.pointer("/data/class").and_then(Value::as_str),
            Some("invalid_request")
        );

        server.stop().await;
    }

    #[tokio::test]
    async fn desktop_rpc_producer_rejects_invalid_envelopes_methods_and_typed_params() {
        const SENTINEL_TOKEN: &str = "hf_rpc_admission_secret_do_not_disclose";

        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let server = start_rpc_server(env.path()).await.unwrap();

        let cases = [
            ("{", -32700, None),
            (
                r#"{"jsonrpc":"1.0","method":"get_status","params":{},"id":41}"#,
                -32600,
                Some(41),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"get_status","params":{},"id":42,"extra":true}"#,
                -32600,
                Some(42),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"unknown_operation","params":{},"id":43}"#,
                -32601,
                Some(43),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"get_status","params":null,"id":44}"#,
                -32602,
                Some(44),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"check_launcher_updates","params":{"force_refresh":-1},"id":45}"#,
                -32602,
                Some(45),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"get_links_for_model","params":{},"id":48}"#,
                -32602,
                Some(48),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"check_files_writable","params":{"file_paths":[]},"id":49}"#,
                -32602,
                Some(49),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"set_model_link_exclusion","params":{"model_id":"model","app_id":"app","excluded":1},"id":50}"#,
                -32602,
                Some(50),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"start_model_conversion","params":{"model_id":"model","direction":"unknown"},"id":52}"#,
                -32602,
                Some(52),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"get_conversion_progress","params":{"conversion_id":"one","conversionId":"two"},"id":53}"#,
                -32602,
                Some(53),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"list_model_conversions","params":{"unexpected":true},"id":54}"#,
                -32602,
                Some(54),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"open_url","params":{"url":"file:///tmp/private"},"id":56}"#,
                -32602,
                Some(56),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"open_path","params":{"path":"/definitely/missing/rpc-open-path-sentinel"},"id":57}"#,
                -32602,
                Some(57),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"start_model_download_from_hf","params":{"repo_id":"acme/model","family":"acme","official_name":"Model","filenames":[]},"id":58}"#,
                -32602,
                Some(58),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"get_model_download_status","params":{"download_id":"one","downloadId":"two"},"id":59}"#,
                -32602,
                Some(59),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"list_model_downloads","params":{"unexpected":true},"id":60}"#,
                -32602,
                Some(60),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"get_models","params":{"unexpected":true},"id":63}"#,
                -32602,
                Some(63),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"refresh_model_index","params":null,"id":64}"#,
                -32602,
                Some(64),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"list_interrupted_downloads","params":{},"id":67}"#,
                -32601,
                Some(67),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"recover_download","params":{"repo_id":"acme/model","dest_dir":"/tmp/model"},"id":68}"#,
                -32601,
                Some(68),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"resume_partial_download","params":{"repo_id":"acme/model","dest_dir":"/tmp/model"},"id":69}"#,
                -32602,
                Some(69),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"resume_partial_download","params":{"modelId":"llm/acme/model","recoveryToken":"v1:short"},"id":70}"#,
                -32602,
                Some(70),
            ),
        ];

        for (body, expected_code, expected_id) in cases {
            let response = rpc_body_raw(server.port, body).await.unwrap();
            assert_eq!(
                response.pointer("/error/code").and_then(Value::as_i64),
                Some(expected_code)
            );
            assert_eq!(response.get("id").and_then(Value::as_i64), expected_id);
        }

        let secret_body = json!({
            "jsonrpc": "2.0",
            "method": "set_hf_token",
            "params": {"token": SENTINEL_TOKEN, "extra": SENTINEL_TOKEN},
            "id": 46
        })
        .to_string();
        let secret_response = rpc_body_raw(server.port, &secret_body).await.unwrap();
        assert_eq!(
            secret_response
                .pointer("/error/code")
                .and_then(Value::as_i64),
            Some(-32602)
        );
        assert!(!secret_response.to_string().contains(SENTINEL_TOKEN));

        let health = rpc_body_raw(
            server.port,
            r#"{"jsonrpc":"2.0","method":"health_check","id":47}"#,
        )
        .await
        .unwrap();
        assert_eq!(
            health.pointer("/result/status").and_then(Value::as_str),
            Some("ok")
        );

        let link_health = rpc_body_raw(
            server.port,
            r#"{"jsonrpc":"2.0","method":"get_link_health","id":51}"#,
        )
        .await
        .unwrap();
        assert_eq!(
            link_health
                .pointer("/result/success")
                .and_then(Value::as_bool),
            Some(true)
        );

        let conversion_progress = rpc_body_raw(
            server.port,
            r#"{"jsonrpc":"2.0","method":"get_conversion_progress","params":{"conversionId":"missing"},"id":55}"#,
        )
        .await
        .unwrap();
        assert_eq!(
            conversion_progress.pointer("/result"),
            Some(&json!({"success": true, "progress": null}))
        );

        let download_status = rpc_body_raw(
            server.port,
            r#"{"jsonrpc":"2.0","method":"get_model_download_status","params":{"downloadId":"missing"},"id":61}"#,
        )
        .await
        .unwrap();
        assert_eq!(
            download_status.pointer("/result"),
            Some(&json!({"success": false, "error": "Download not found"}))
        );

        let downloads = rpc_body_raw(
            server.port,
            r#"{"jsonrpc":"2.0","method":"list_model_downloads","id":62}"#,
        )
        .await
        .unwrap();
        assert_eq!(
            downloads.pointer("/result"),
            Some(&json!({"success": true, "downloads": []}))
        );

        let missing_recovery = rpc_body_raw(
            server.port,
            &json!({
                "jsonrpc": "2.0",
                "method": "resume_partial_download",
                "params": {
                    "modelId": "llm/acme/missing",
                    "recoveryToken": format!("v1:{}", "a".repeat(64))
                },
                "id": 71
            })
            .to_string(),
        )
        .await
        .unwrap();
        assert_eq!(
            missing_recovery.pointer("/result"),
            Some(&json!({
                "success": false,
                "action": "none",
                "download_id": null,
                "status": null,
                "reason_code": "model_not_found",
                "error": "The partial download could not be resumed."
            }))
        );
        assert!(!missing_recovery.to_string().contains("repo_id"));
        assert!(!missing_recovery.to_string().contains("dest_dir"));
        assert!(!missing_recovery.to_string().contains("message"));

        let models = rpc_body_raw(
            server.port,
            r#"{"jsonrpc":"2.0","method":"get_models","id":65}"#,
        )
        .await
        .unwrap();
        assert_eq!(
            models.pointer("/result/success").and_then(Value::as_bool),
            Some(true)
        );
        assert!(models
            .pointer("/result/models")
            .and_then(Value::as_object)
            .is_some());

        let refresh = rpc_body_raw(
            server.port,
            r#"{"jsonrpc":"2.0","method":"refresh_model_index","params":{},"id":66}"#,
        )
        .await
        .unwrap();
        assert_eq!(
            refresh.pointer("/result/success").and_then(Value::as_bool),
            Some(true)
        );
        assert!(refresh
            .pointer("/result/indexed_count")
            .and_then(Value::as_u64)
            .is_some());

        server.stop().await;
    }

    #[tokio::test]
    async fn desktop_rpc_partial_recovery_uses_issued_ticket_for_tracked_download() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let model_id = create_tracked_partial_test_model(env.path(), "paused");
        let server = start_rpc_server(env.path()).await.unwrap();

        let recovery_token = recovery_token_for_model(server.port, model_id).await;

        let catalog = rpc_call(server.port, "get_models", json!({}))
            .await
            .unwrap();
        let search = rpc_call(
            server.port,
            "search_models_fts",
            json!({"query":"partial", "limit":10, "offset":0}),
        )
        .await
        .unwrap();
        assert_eq!(search["success"], true);
        assert_eq!(search["models"].as_array().unwrap().len(), 1);
        assert_eq!(search["models"][0], catalog["models"][model_id]);
        assert_eq!(
            search["models"][0]["artifact"]["recovery"]["recoveryToken"],
            recovery_token
        );

        let stale = rpc_call(
            server.port,
            "resume_partial_download",
            json!({
                "modelId": model_id,
                "recoveryToken": format!("v1:{}", "b".repeat(64))
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            stale,
            json!({
                "success": false,
                "action": "none",
                "download_id": null,
                "status": null,
                "reason_code": "recovery_context_stale",
                "error": "The partial download could not be resumed."
            })
        );

        let resumed = rpc_call(
            server.port,
            "resume_partial_download",
            json!({"modelId": model_id, "recoveryToken": recovery_token}),
        )
        .await
        .unwrap();
        assert_eq!(resumed["success"], true);
        assert_eq!(resumed["action"], "resume");
        assert_eq!(resumed["download_id"], "tracked-partial-1");
        assert_eq!(resumed["status"], "queued");
        assert!(resumed.get("repoId").is_none());
        assert!(resumed.get("modelDir").is_none());
        assert!(resumed.get("message").is_none());

        server.stop().await;
    }

    #[tokio::test]
    async fn desktop_rpc_partial_recovery_attaches_to_active_exact_context() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let model_id = create_tracked_partial_test_model(env.path(), "paused");
        let server = start_rpc_server(env.path()).await.unwrap();
        let recovery_token = recovery_token_for_model(server.port, model_id).await;

        let first = rpc_call(
            server.port,
            "resume_partial_download",
            json!({"modelId": model_id, "recoveryToken": recovery_token}),
        );
        let second = rpc_call(
            server.port,
            "resume_partial_download",
            json!({"modelId": model_id, "recoveryToken": recovery_token}),
        );
        let (first, second) = tokio::join!(first, second);
        let outcomes = [first.unwrap(), second.unwrap()];
        assert!(
            outcomes.iter().all(|outcome| outcome["success"] == true),
            "{outcomes:?}"
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| outcome["action"] == "resume")
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| outcome["action"] == "attach")
                .count(),
            1
        );
        assert!(outcomes.iter().all(|outcome| {
            outcome["download_id"] == "tracked-partial-1"
                && if outcome["action"] == "resume" {
                    outcome["status"] == "queued"
                } else {
                    matches!(
                        outcome["status"].as_str(),
                        Some("queued" | "downloading" | "pausing" | "cancelling")
                    )
                }
        }));

        server.stop().await;
    }

    #[tokio::test]
    async fn desktop_rpc_partial_recovery_starts_untracked_download_from_issued_ticket() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let model_id = create_untracked_partial_test_model(env.path());
        let server = start_rpc_server(env.path()).await.unwrap();
        let recovery_token = recovery_token_for_model(server.port, model_id).await;

        let recovered = rpc_call(
            server.port,
            "resume_partial_download",
            json!({"modelId": model_id, "recoveryToken": recovery_token}),
        )
        .await
        .unwrap();
        assert_eq!(recovered["success"], true);
        assert_eq!(recovered["action"], "recover");
        assert_eq!(recovered["status"], "queued");
        assert!(recovered["download_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(recovered.get("repoId").is_none());
        assert!(recovered.get("modelDir").is_none());
        assert!(recovered.get("message").is_none());
        // Fresh-owner reconciliation may publish an empty current-format store.
        // Recovery must not create an unowned ordinary resumable snapshot.
        let store: Value = serde_json::from_slice(
            &std::fs::read(env.path().join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(store["downloads"], json!([]));
        let model_dir = env.path().join("shared-resources/models").join(model_id);
        assert!(!model_dir.join("config.json").exists());
        assert!(!model_dir.join("config.json.part").exists());

        server.stop().await;
    }

    #[tokio::test]
    async fn desktop_rpc_partial_recovery_refuses_missing_bound_remote_file_without_task_or_write()
    {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let model_id = create_untracked_partial_with_missing_remote_member(env.path());
        let model_dir = env.path().join("shared-resources/models").join(model_id);
        let server = start_rpc_server(env.path()).await.unwrap();
        let recovery_token = recovery_token_for_model(server.port, model_id).await;
        let store_path = env.path().join("launcher-data/downloads.json");
        let store_before = std::fs::read(&store_path).unwrap();

        let refused = rpc_call(
            server.port,
            "resume_partial_download",
            json!({"modelId": model_id, "recoveryToken": recovery_token}),
        )
        .await
        .unwrap();
        assert_eq!(
            refused,
            json!({
                "success": false,
                "action": "none",
                "download_id": null,
                "status": null,
                "reason_code": "recovery_context_stale",
                "error": "The partial download could not be resumed."
            })
        );
        assert_eq!(
            std::fs::read(model_dir.join("weights-1.gguf.part")).unwrap(),
            b"partial"
        );
        assert!(!model_dir.join("weights-2.gguf.part").exists());
        assert!(!model_dir.join(".pumas_download").exists());
        assert_eq!(std::fs::read(store_path).unwrap(), store_before);

        server.stop().await;
    }

    #[tokio::test]
    async fn desktop_rpc_partial_recovery_maps_repo_lookup_failure_to_closed_outcome() {
        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let model_id = create_untracked_partial_test_model(env.path());
        let server = start_rpc_server(env.path()).await.unwrap();
        let recovery_token = recovery_token_for_model(server.port, model_id).await;
        std::fs::write(
            env.path()
                .join("launcher-data/cache/hf/hf_acme_model_files.json"),
            b"not-json",
        )
        .unwrap();

        let refused = rpc_call(
            server.port,
            "resume_partial_download",
            json!({"modelId": model_id, "recoveryToken": recovery_token}),
        )
        .await
        .unwrap();
        assert_eq!(
            refused,
            json!({
                "success": false,
                "action": "none",
                "download_id": null,
                "status": null,
                "reason_code": "recover_failed",
                "error": "The partial download could not be resumed."
            })
        );

        server.stop().await;
    }

    #[tokio::test]
    async fn rpc_cli_rejects_remote_host_and_removed_lan_flag_before_startup() {
        let env = create_test_env();
        let binary = rpc_binary_path().unwrap();

        let remote = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::process::Command::new(&binary)
                .args(["--host", "0.0.0.0", "--port", "0", "--launcher-root"])
                .arg(env.path())
                .output(),
        )
        .await
        .expect("remote-host process did not terminate")
        .expect("remote-host process did not start");
        let remote_stdout = String::from_utf8_lossy(&remote.stdout);
        let remote_stderr = String::from_utf8_lossy(&remote.stderr);
        assert!(!remote.status.success());
        assert!(!remote_stdout.contains("RPC_PORT="));
        assert!(remote_stderr.contains("loopback"), "{remote_stderr}");

        let removed_flag = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::process::Command::new(binary)
                .args([
                    "--host",
                    "127.0.0.1",
                    "--allow-lan",
                    "--port",
                    "0",
                    "--launcher-root",
                ])
                .arg(env.path())
                .output(),
        )
        .await
        .expect("removed-flag process did not terminate")
        .expect("removed-flag process did not start");
        let flag_stdout = String::from_utf8_lossy(&removed_flag.stdout);
        let flag_stderr = String::from_utf8_lossy(&removed_flag.stderr);
        assert!(!removed_flag.status.success());
        assert!(!flag_stdout.contains("RPC_PORT="));
        assert!(flag_stderr.contains("--allow-lan"), "{flag_stderr}");
    }

    #[tokio::test]
    async fn debug_rpc_process_does_not_disclose_credentials_or_private_locators() {
        const SENTINEL_TOKEN: &str = "hf_test_rpc_secret_do_not_disclose";
        const SENTINEL_PATH_FRAGMENT: &str = "private-rpc-path-sentinel";
        const SENTINEL_URL_FRAGMENT: &str = "private-rpc-url-sentinel";

        if !can_bind_local_tcp_for_tests() {
            return;
        }
        let env = create_test_env();
        let server = start_rpc_server_with_debug(env.path(), true).await.unwrap();

        let token_response = match rpc_call_raw(
            server.port,
            "set_hf_token",
            json!({"token": SENTINEL_TOKEN}),
        )
        .await
        {
            Ok(response) => response,
            Err(_) => panic!("credential request did not return a JSON-RPC response"),
        };
        assert!(token_response.get("error").is_none());

        let private_path = env.path().join(SENTINEL_PATH_FRAGMENT);
        let path_response = match rpc_call_raw(
            server.port,
            "open_path",
            json!({"path": private_path.to_string_lossy()}),
        )
        .await
        {
            Ok(response) => response,
            Err(_) => panic!("private-path request did not return a JSON-RPC response"),
        };
        let path_error = path_response
            .get("error")
            .expect("private-path failure must be a typed JSON-RPC error");
        assert_eq!(path_error.get("code").and_then(Value::as_i64), Some(-32602));
        assert_eq!(
            path_error.pointer("/data/class").and_then(Value::as_str),
            Some("invalid_request")
        );

        let url_response = match rpc_call_raw(
            server.port,
            "open_url",
            json!({"url": format!("file:///{SENTINEL_URL_FRAGMENT}")}),
        )
        .await
        {
            Ok(response) => response,
            Err(_) => panic!("private-URL request did not return a JSON-RPC response"),
        };
        let url_error = url_response
            .get("error")
            .expect("private-URL failure must be a typed JSON-RPC error");
        assert_eq!(url_error.get("code").and_then(Value::as_i64), Some(-32602));
        assert_eq!(
            url_error.pointer("/data/class").and_then(Value::as_str),
            Some("invalid_request")
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        let diagnostics = server.diagnostics().await;
        let responses = format!("{token_response}{path_response}{url_response}");

        assert!(!diagnostics.contains(SENTINEL_TOKEN));
        assert!(!diagnostics.contains(SENTINEL_PATH_FRAGMENT));
        assert!(!diagnostics.contains(SENTINEL_URL_FRAGMENT));
        assert!(!responses.contains(SENTINEL_TOKEN));
        assert!(!responses.contains(SENTINEL_PATH_FRAGMENT));
        assert!(!responses.contains(SENTINEL_URL_FRAGMENT));

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
        let response = rpc_call(port, "get_active_version", json!({"app_id": "ollama"}))
            .await
            .unwrap();
        assert!(response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
        // version can be null or a string

        // get_default_version
        let response = rpc_call(port, "get_default_version", json!({"app_id": "ollama"}))
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

        let response = rpc_call(port, "is_ollama_running", json!({}))
            .await
            .unwrap();
        assert!(response.is_boolean());
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
        ("get_available_versions", json!({"app_id": "ollama"})),
        ("get_installed_versions", json!({"app_id": "ollama"})),
        ("get_active_version", json!({"app_id": "ollama"})),
        ("get_default_version", json!({"app_id": "ollama"})),
        ("is_ollama_running", json!({})),
        ("has_background_fetch_completed", json!({})),
        ("get_github_cache_status", json!({"app_id": "ollama"})),
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
