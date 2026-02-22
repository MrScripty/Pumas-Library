//! JSON-RPC request handlers.

use crate::server::AppState;
use crate::wrapper::wrap_response;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// JSON-RPC 2.0 request structure.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 response structure.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 error structure.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

/// Health check endpoint.
pub async fn handle_health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

/// Main JSON-RPC handler.
pub async fn handle_rpc(
    State(state): State<Arc<AppState>>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let method = &request.method;
    let params = request.params.unwrap_or(Value::Object(Default::default()));
    let id = request.id.clone();

    debug!("RPC call: {}({:?})", method, params);

    // Handle built-in methods
    if method == "health_check" {
        return (
            StatusCode::OK,
            Json(JsonRpcResponse::success(id, json!({"status": "ok"}))),
        );
    }

    if method == "shutdown" {
        // In production, this would trigger a graceful shutdown
        return (
            StatusCode::OK,
            Json(JsonRpcResponse::success(id, json!({"status": "shutting_down"}))),
        );
    }

    // Dispatch to API methods
    let result = dispatch_method(&state, method, &params).await;

    match result {
        Ok(value) => {
            let wrapped = wrap_response(method, value);
            (StatusCode::OK, Json(JsonRpcResponse::success(id, wrapped)))
        }
        Err(e) => {
            error!("RPC error for {}: {}", method, e);
            let code = e.to_rpc_error_code();
            (
                StatusCode::OK,
                Json(JsonRpcResponse::error(id, code, e.to_string())),
            )
        }
    }
}

// ============================================================================
// Helper macros for extracting parameters
// ============================================================================

/// Extract a required string parameter, supporting both snake_case and camelCase.
macro_rules! get_str_param {
    ($params:expr, $snake:literal, $camel:literal) => {
        $params
            .get($snake)
            .or_else(|| $params.get($camel))
            .and_then(|v| v.as_str())
    };
}

/// Extract a required string parameter or return an error.
macro_rules! require_str_param {
    ($params:expr, $snake:literal, $camel:literal) => {
        match get_str_param!($params, $snake, $camel) {
            Some(s) => s.to_string(),
            None => {
                return Err(pumas_library::PumasError::InvalidParams {
                    message: format!("Missing required parameter: {}", $snake),
                });
            }
        }
    };
}

/// Extract an optional bool parameter, supporting both snake_case and camelCase.
macro_rules! get_bool_param {
    ($params:expr, $snake:literal, $camel:literal) => {
        $params
            .get($snake)
            .or_else(|| $params.get($camel))
            .and_then(|v| v.as_bool())
    };
}

/// Extract an optional i64 parameter.
macro_rules! get_i64_param {
    ($params:expr, $snake:literal, $camel:literal) => {
        $params
            .get($snake)
            .or_else(|| $params.get($camel))
            .and_then(|v| v.as_i64())
    };
}

/// Extract an app_id parameter and convert to AppId.
macro_rules! get_app_id {
    ($params:expr) => {
        $params
            .get("app_id")
            .or_else(|| $params.get("appId"))
            .and_then(|v| v.as_str())
            .and_then(pumas_library::AppId::from_str)
    };
}

// ============================================================================
// Helper functions
// ============================================================================

/// Extract the JSON header from a safetensors file.
///
/// Safetensors format: 8-byte header size (little-endian u64) followed by JSON header.
fn extract_safetensors_header(path: &str) -> std::result::Result<Value, String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;

    // Read header size (8 bytes, little-endian)
    let mut size_buf = [0u8; 8];
    file.read_exact(&mut size_buf).map_err(|e| e.to_string())?;
    let header_size = u64::from_le_bytes(size_buf) as usize;

    // Sanity check
    if header_size > 100_000_000 {
        return Err("Header size too large".to_string());
    }

    // Read JSON header
    let mut header_buf = vec![0u8; header_size];
    file.read_exact(&mut header_buf).map_err(|e| e.to_string())?;

    // Parse JSON - the header contains tensor metadata, not model metadata
    // Safetensors stores tensor shapes/dtypes, not general metadata like GGUF
    let header: Value = serde_json::from_slice(&header_buf).map_err(|e| e.to_string())?;

    // Extract __metadata__ field if present (some safetensors files include this)
    if let Some(metadata) = header.get("__metadata__") {
        Ok(metadata.clone())
    } else {
        // Return tensor info as metadata
        Ok(header)
    }
}

// ============================================================================
// Method dispatcher
// ============================================================================

/// Dispatch a method call to the appropriate API handler.
async fn dispatch_method(
    state: &AppState,
    method: &str,
    params: &Value,
) -> pumas_library::Result<Value> {
    let api = &state.api;
    match method {
        // ====================================================================
        // Status & System
        // ====================================================================
        "get_status" => {
            // Ensure process manager has current version paths for accurate running detection
            sync_version_paths_to_process_manager(state).await;
            let mut response = api.get_status().await?;

            // Enrich with version-specific data from ComfyUI version manager
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get("comfyui") {
                // Get active version
                let active_version = vm.get_active_version().await.ok().flatten();

                // Check dependencies for active version
                if let Some(ref tag) = active_version {
                    if let Ok(deps) = vm.check_dependencies(tag).await {
                        response.deps_ready = deps.missing.is_empty();
                    }
                }

                // Check if active version is patched
                if let Some(ref tag) = active_version {
                    response.patched = api.is_patched(Some(tag));
                }

                // Get shortcut states for active version
                if let Some(ref tag) = active_version {
                    response.shortcut_version = Some(tag.clone());
                    let sm_lock = state.shortcut_manager.read().await;
                    if let Some(ref sm) = *sm_lock {
                        let shortcut_state = sm.get_version_shortcut_state(tag);
                        response.menu_shortcut = shortcut_state.menu;
                        response.desktop_shortcut = shortcut_state.desktop;
                    }
                }
            }
            drop(managers);

            Ok(serde_json::to_value(response)?)
        }

        "get_disk_space" => {
            let response = api.get_disk_space().await?;
            Ok(serde_json::to_value(response)?)
        }

        "get_system_resources" => {
            let response = api.get_system_resources().await?;
            Ok(serde_json::to_value(response)?)
        }

        "get_launcher_version" => {
            let version_info = api.get_launcher_version();
            Ok(version_info)
        }

        "check_launcher_updates" => {
            let force_refresh = get_bool_param!(params, "force_refresh", "forceRefresh").unwrap_or(false);
            let result = api.check_launcher_updates(force_refresh).await;
            Ok(serde_json::to_value(result)?)
        }

        "apply_launcher_update" => {
            let result = api.apply_launcher_update().await;
            Ok(serde_json::to_value(result)?)
        }

        "restart_launcher" => {
            match api.restart_launcher() {
                Ok(success) => Ok(json!({
                    "success": success
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                }))
            }
        }

        "get_sandbox_info" => {
            // Detect sandbox environment
            let (is_sandboxed, sandbox_type, limitations) = detect_sandbox_environment();
            Ok(json!({
                "success": true,
                "is_sandboxed": is_sandboxed,
                "sandbox_type": sandbox_type,
                "limitations": limitations
            }))
        }

        // ====================================================================
        // Version Management (uses pumas-app-manager)
        // ====================================================================
        "get_available_versions" => {
            let force_refresh = get_bool_param!(params, "force_refresh", "forceRefresh").unwrap_or(false);
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");

            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                // Handle rate limit errors specially to return structured response
                match vm.get_available_releases(force_refresh).await {
                    Ok(releases) => {
                        let versions: Vec<pumas_library::models::VersionReleaseInfo> = releases
                            .into_iter()
                            .map(pumas_library::models::VersionReleaseInfo::from)
                            .collect();
                        Ok(json!({
                            "success": true,
                            "versions": versions
                        }))
                    }
                    Err(pumas_library::PumasError::RateLimited { service, retry_after_secs }) => {
                        warn!("Rate limited by {} when fetching versions", service);
                        Ok(json!({
                            "success": false,
                            "error": format!("Rate limited by {}", service),
                            "rate_limited": true,
                            "retry_after_secs": retry_after_secs
                        }))
                    },
                    Err(e) => Err(e)
                }
            } else {
                Ok(json!({
                    "success": true,
                    "versions": []
                }))
            }
        }

        "get_installed_versions" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let versions = vm.get_installed_versions().await?;
                // Return raw array - wrapper.rs will add {success, versions} wrapper
                Ok(serde_json::to_value(versions)?)
            } else {
                Ok(json!([]))
            }
        }

        "get_active_version" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let version = vm.get_active_version().await?;
                // Return raw value - wrapper.rs will add {success, version} wrapper
                Ok(serde_json::to_value(version)?)
            } else {
                Ok(Value::Null)
            }
        }

        "get_default_version" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let version = vm.get_default_version().await?;
                // Return raw value - wrapper.rs will add {success, version} wrapper
                Ok(serde_json::to_value(version)?)
            } else {
                Ok(Value::Null)
            }
        }

        "set_default_version" => {
            let tag = get_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let result = vm.set_default_version(tag).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_library::PumasError::Config {
                    message: format!("Version manager not initialized for app: {}", app_id_str),
                })
            }
        }

        "switch_version" => {
            let tag = require_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let result = vm.set_active_version(&tag).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_library::PumasError::Config {
                    message: format!("Version manager not initialized for app: {}", app_id_str),
                })
            }
        }

        "install_version" => {
            let tag = require_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");

            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                // Start the installation (returns a progress receiver)
                match vm.install_version(&tag).await {
                    Ok(_rx) => {
                        // Installation started successfully
                        // Progress can be monitored via get_installation_progress
                        Ok(json!({
                            "success": true,
                            "message": format!("Installation of {} started", tag)
                        }))
                    }
                    Err(e) => {
                        warn!("Failed to start installation of {}: {}", tag, e);
                        Ok(json!({
                            "success": false,
                            "error": e.to_string()
                        }))
                    }
                }
            } else {
                Ok(json!({
                    "success": false,
                    "error": format!("Version manager not initialized for app: {}", app_id_str)
                }))
            }
        }

        "remove_version" => {
            let tag = require_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let result = vm.remove_version(&tag).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_library::PumasError::Config {
                    message: format!("Version manager not initialized for app: {}", app_id_str),
                })
            }
        }

        "cancel_installation" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let result = vm.cancel_installation().await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Ok(serde_json::to_value(false)?)
            }
        }

        "get_installation_progress" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let progress = vm.get_installation_progress().await;
                Ok(serde_json::to_value(progress)?)
            } else {
                Ok(serde_json::to_value::<Option<pumas_library::models::InstallationProgress>>(None)?)
            }
        }

        "validate_installations" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let result = vm.validate_installations().await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Ok(json!({
                    "removed_tags": [],
                    "orphaned_dirs": [],
                    "valid_count": 0
                }))
            }
        }

        "get_version_status" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                // Return version status combining active/default/installed
                let active = vm.get_active_version().await?;
                let default = vm.get_default_version().await?;
                let installed = vm.get_installed_versions().await?;

                // Build versions map with isActive and dependencies for each installed version
                let mut versions_map = serde_json::Map::new();
                for tag in &installed {
                    let is_active = active.as_ref() == Some(tag);
                    // Get dependency status if available
                    let deps = vm.check_dependencies(tag).await.ok();
                    versions_map.insert(tag.clone(), json!({
                        "isActive": is_active,
                        "dependencies": {
                            "installed": deps.as_ref().map(|d| &d.installed).unwrap_or(&vec![]),
                            "missing": deps.as_ref().map(|d| &d.missing).unwrap_or(&vec![])
                        }
                    }));
                }

                // Return raw status object - wrapper.rs will add {success, status} wrapper
                Ok(json!({
                    "installedCount": installed.len(),
                    "activeVersion": active,
                    "defaultVersion": default,
                    "versions": versions_map
                }))
            } else {
                Ok(json!({
                    "installedCount": 0,
                    "activeVersion": null,
                    "defaultVersion": null,
                    "versions": {}
                }))
            }
        }

        "get_version_info" => {
            let tag = require_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let installed = vm.get_installed_versions().await?;
                let is_installed = installed.contains(&tag);
                Ok(json!({
                    "tag": tag,
                    "installed": is_installed,
                    "size": null
                }))
            } else {
                Ok(json!({
                    "tag": tag,
                    "installed": false,
                    "size": null
                }))
            }
        }

        "get_release_size_info" => {
            let tag = require_str_param!(params, "tag", "tag");
            let archive_size = get_i64_param!(params, "archive_size", "archiveSize").unwrap_or(0) as u64;

            // Calculate release size using size_calculator from state
            let mut calc = state.size_calculator.write().await;
            let result = calc.calculate_release_size(&tag, archive_size, None).await?;
            Ok(serde_json::to_value(result)?)
        }

        "get_release_size_breakdown" => {
            let tag = require_str_param!(params, "tag", "tag");

            // Get cached size breakdown
            let calc = state.size_calculator.read().await;
            if let Some(breakdown) = calc.get_size_breakdown(&tag) {
                Ok(serde_json::to_value(breakdown)?)
            } else {
                Ok(json!({
                    "tag": tag,
                    "error": "No cached size data available"
                }))
            }
        }

        "calculate_release_size" => {
            let tag = require_str_param!(params, "tag", "tag");
            let archive_size = get_i64_param!(params, "archive_size", "archiveSize").unwrap_or(0) as u64;

            // Parse optional requirements array
            let requirements: Option<Vec<String>> = params
                .get("requirements")
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            let mut calc = state.size_calculator.write().await;
            let result = calc.calculate_release_size(
                &tag,
                archive_size,
                requirements.as_deref(),
            ).await?;
            Ok(serde_json::to_value(result)?)
        }

        "calculate_all_release_sizes" => {
            // Get all available versions and calculate sizes
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            let versions = if let Some(vm) = managers.get(app_id_str) {
                let releases = vm.get_available_releases(false).await?;
                releases
                    .into_iter()
                    .map(pumas_library::models::VersionReleaseInfo::from)
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            drop(managers);

            let mut results = serde_json::Map::new();
            let mut calc = state.size_calculator.write().await;

            for version in versions.iter().take(20) {
                // Limit to avoid too many calculations
                let archive_size = version.archive_size.unwrap_or(0);
                if let Ok(size_info) = calc.calculate_release_size(&version.tag_name, archive_size, None).await {
                    if let Ok(value) = serde_json::to_value(&size_info) {
                        results.insert(version.tag_name.clone(), value);
                    }
                }
            }

            Ok(json!(results))
        }

        // ====================================================================
        // Background Fetch
        // ====================================================================
        "has_background_fetch_completed" => {
            let completed = api.has_background_fetch_completed().await;
            Ok(serde_json::to_value(completed)?)
        }

        "reset_background_fetch_flag" => {
            api.reset_background_fetch_flag().await;
            Ok(json!(true))
        }

        "get_github_cache_status" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            // Return cache status in format expected by frontend
            // CacheStatusResponse expects: has_cache, is_valid, is_fetching, age_seconds?, last_fetched?, releases_count?
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let cache_status = vm.get_github_cache_status();
                Ok(json!({
                    "has_cache": cache_status.has_cache,
                    "is_valid": cache_status.is_valid,
                    "is_fetching": cache_status.is_fetching,
                    "age_seconds": cache_status.age_seconds,
                    "last_fetched": cache_status.last_fetched,
                    "releases_count": cache_status.releases_count
                }))
            } else {
                Ok(json!({
                    "has_cache": false,
                    "is_valid": false,
                    "is_fetching": false
                }))
            }
        }

        // ====================================================================
        // Dependency Management (uses pumas-app-manager)
        // ====================================================================
        "check_version_dependencies" => {
            let tag = require_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let status = vm.check_dependencies(&tag).await?;
                Ok(serde_json::to_value(status)?)
            } else {
                Err(pumas_library::PumasError::Config {
                    message: format!("Version manager not initialized for app: {}", app_id_str),
                })
            }
        }

        "install_version_dependencies" => {
            let tag = require_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let result = vm.install_dependencies(&tag, None).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_library::PumasError::Config {
                    message: format!("Version manager not initialized for app: {}", app_id_str),
                })
            }
        }

        "get_release_dependencies" => {
            let tag = require_str_param!(params, "tag", "tag");
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                let version_path = vm.version_path(&tag);
                let requirements_path = version_path.join("requirements.txt");

                if !requirements_path.exists() {
                    return Ok(serde_json::to_value::<Vec<String>>(vec![])?);
                }

                let content = std::fs::read_to_string(&requirements_path).map_err(|e| {
                    pumas_library::PumasError::Io {
                        message: format!("Failed to read requirements.txt: {}", e),
                        path: Some(requirements_path),
                        source: Some(e),
                    }
                })?;

                // Parse requirements (simple extraction of package names)
                let packages: Vec<String> = content
                    .lines()
                    .filter(|line| {
                        let line = line.trim();
                        !line.is_empty() && !line.starts_with('#') && !line.starts_with('-')
                    })
                    .filter_map(|line| {
                        let name = line
                            .split(|c| c == '=' || c == '>' || c == '<' || c == '[' || c == ';')
                            .next()?
                            .trim();
                        if !name.is_empty() {
                            Some(name.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                Ok(serde_json::to_value(packages)?)
            } else {
                Err(pumas_library::PumasError::Config {
                    message: format!("Version manager not initialized for app: {}", app_id_str),
                })
            }
        }

        // ====================================================================
        // Patching
        // ====================================================================
        "is_patched" => {
            let tag = get_str_param!(params, "tag", "tag");
            let is_patched = api.is_patched(tag);
            Ok(json!(is_patched))
        }

        "toggle_patch" => {
            let tag = get_str_param!(params, "tag", "tag");
            match api.toggle_patch(tag) {
                Ok(is_now_patched) => Ok(json!(is_now_patched)),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                }))
            }
        }

        // ====================================================================
        // Process Management
        // ====================================================================
        "is_comfyui_running" => {
            // Ensure process manager has current version paths for accurate detection
            sync_version_paths_to_process_manager(state).await;
            let running = api.is_comfyui_running().await;
            Ok(serde_json::to_value(running)?)
        }

        "stop_comfyui" => {
            // Ensure process manager has current version paths for proper PID file cleanup
            sync_version_paths_to_process_manager(state).await;
            let result = api.stop_comfyui().await?;
            Ok(json!({ "success": result }))
        }

        "launch_comfyui" => {
            // Ensure process manager has current version paths
            sync_version_paths_to_process_manager(state).await;
            // Get the active version from comfyui version_manager and launch it
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get("comfyui") {
                let active = vm.get_active_version().await?;
                if let Some(tag) = active {
                    let version_dir = vm.version_path(&tag);
                    drop(managers);  // Release lock before calling api
                    let response = api.launch_version(&tag, &version_dir).await?;
                    Ok(serde_json::to_value(response)?)
                } else {
                    Ok(json!({
                        "success": false,
                        "error": "No active version set"
                    }))
                }
            } else {
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized for comfyui"
                }))
            }
        }

        "launch_ollama" => {
            // Get the active version from ollama version_manager and launch it
            let managers = state.version_managers.read().await;
            info!("launch_ollama: checking for ollama version manager");
            if let Some(vm) = managers.get("ollama") {
                let installed = vm.get_installed_versions().await?;
                info!("launch_ollama: installed versions: {:?}", installed);
                let active = vm.get_active_version().await?;
                info!("launch_ollama: active version: {:?}", active);
                if let Some(tag) = active {
                    let version_dir = vm.version_path(&tag);
                    info!("launch_ollama: launching tag={} from {:?}", tag, version_dir);
                    drop(managers);  // Release lock before calling api
                    let response = api.launch_ollama(&tag, &version_dir).await?;
                    info!("launch_ollama: result success={}", response.success);
                    Ok(serde_json::to_value(response)?)
                } else {
                    warn!("launch_ollama: no active version set");
                    Ok(json!({
                        "success": false,
                        "error": "No active Ollama version set"
                    }))
                }
            } else {
                warn!("launch_ollama: version manager not initialized");
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized for ollama"
                }))
            }
        }

        "stop_ollama" => {
            let result = api.stop_ollama().await?;
            Ok(json!({ "success": result }))
        }

        "is_ollama_running" => {
            let running = api.is_ollama_running().await;
            Ok(serde_json::to_value(running)?)
        }

        // ====================================================================
        // Ollama Model Management (load library models into Ollama)
        // ====================================================================
        "ollama_list_models" => {
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");
            let client = pumas_app_manager::OllamaClient::new(connection_url);
            let models = client.list_models().await?;
            Ok(json!({
                "success": true,
                "models": models
            }))
        }

        "ollama_create_model" => {
            let model_id = require_str_param!(params, "model_id", "modelId");
            let model_name = get_str_param!(params, "model_name", "modelName");
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            // Resolve GGUF path from library
            let library = api.model_library();
            let primary_file = library.get_primary_model_file(&model_id);
            let gguf_path = match primary_file {
                Some(path) => {
                    let ext = path.extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    if ext != "gguf" {
                        return Ok(json!({
                            "success": false,
                            "error": format!("Model file is not GGUF format (found .{})", ext)
                        }));
                    }
                    path
                }
                None => {
                    return Ok(json!({
                        "success": false,
                        "error": format!("No model file found for '{}'", model_id)
                    }));
                }
            };

            // Look up model record for name derivation and cached SHA256
            let model_record = library.get_model(&model_id).await?;

            let ollama_name = match model_name {
                Some(name) => name.to_string(),
                None => {
                    let display = model_record.as_ref()
                        .map(|r| r.cleaned_name.clone())
                        .unwrap_or_else(|| model_id.split('/').last().unwrap_or(&model_id).to_string());
                    pumas_app_manager::derive_ollama_name(&display)
                }
            };

            // Use cached SHA256 from library metadata if available
            let known_sha256 = model_record.as_ref()
                .and_then(|r| r.hashes.get("sha256"))
                .cloned();

            let client = pumas_app_manager::OllamaClient::new(connection_url);
            client.create_model(&ollama_name, &gguf_path, known_sha256.as_deref()).await?;

            // Auto-load the model into VRAM/RAM so it's ready for inference.
            client.load_model(&ollama_name).await?;

            Ok(json!({
                "success": true,
                "model_name": ollama_name
            }))
        }

        "ollama_delete_model" => {
            let model_name = require_str_param!(params, "model_name", "modelName");
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            let client = pumas_app_manager::OllamaClient::new(connection_url);
            client.delete_model(&model_name).await?;

            Ok(json!({ "success": true }))
        }

        "ollama_load_model" => {
            let model_name = require_str_param!(params, "model_name", "modelName");
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            let client = pumas_app_manager::OllamaClient::new(connection_url);
            client.load_model(&model_name).await?;

            Ok(json!({ "success": true }))
        }

        "ollama_unload_model" => {
            let model_name = require_str_param!(params, "model_name", "modelName");
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            let client = pumas_app_manager::OllamaClient::new(connection_url);
            client.unload_model(&model_name).await?;

            Ok(json!({ "success": true }))
        }

        "ollama_list_running" => {
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            let client = pumas_app_manager::OllamaClient::new(connection_url);
            let models = client.list_running_models().await?;

            Ok(json!({ "success": true, "models": models }))
        }

        // ====================================================================
        // Torch Inference Server
        // ====================================================================
        "torch_list_slots" => {
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");
            let client = pumas_app_manager::TorchClient::new(connection_url);
            let slots = client.list_slots().await?;
            Ok(json!({
                "success": true,
                "slots": slots
            }))
        }

        "torch_load_model" => {
            let model_id = require_str_param!(params, "model_id", "modelId");
            let device = get_str_param!(params, "device", "device").unwrap_or("auto");
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            // Resolve model path from library
            let library = api.model_library();
            let primary_file = library.get_primary_model_file(&model_id);
            let model_path = match primary_file {
                Some(path) => path,
                None => {
                    // For safetensors, the model directory itself may be the path
                    let model_dir = library.library_root().join(&model_id);
                    if model_dir.exists() {
                        model_dir
                    } else {
                        return Ok(json!({
                            "success": false,
                            "error": format!("No model found for '{}'", model_id)
                        }));
                    }
                }
            };

            let model_record = library.get_model(&model_id).await?;
            let model_name = model_record
                .as_ref()
                .map(|r| r.cleaned_name.clone())
                .unwrap_or_else(|| model_id.split('/').last().unwrap_or(&model_id).to_string());

            let compute_device = pumas_app_manager::ComputeDevice::from_server_string(device);
            let client = pumas_app_manager::TorchClient::new(connection_url);
            let slot = client.load_model(
                &model_path.to_string_lossy(),
                &model_name,
                &compute_device,
                None,
            ).await?;

            Ok(json!({
                "success": true,
                "slot": slot
            }))
        }

        "torch_unload_model" => {
            let slot_id = require_str_param!(params, "slot_id", "slotId");
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            let client = pumas_app_manager::TorchClient::new(connection_url);
            client.unload_model(&slot_id).await?;

            Ok(json!({ "success": true }))
        }

        "torch_get_status" => {
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            let client = pumas_app_manager::TorchClient::new(connection_url);
            let status = client.get_status().await?;

            Ok(json!({
                "success": true,
                "status": status
            }))
        }

        "torch_list_devices" => {
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");

            let client = pumas_app_manager::TorchClient::new(connection_url);
            let devices = client.list_devices().await?;

            Ok(json!({
                "success": true,
                "devices": devices
            }))
        }

        "torch_configure" => {
            let connection_url = get_str_param!(params, "connection_url", "connectionUrl");
            let config: pumas_app_manager::TorchServerConfig = serde_json::from_value(
                params.get("config").cloned().unwrap_or_default()
            ).map_err(|e| pumas_library::PumasError::InvalidParams {
                message: format!("Invalid torch config: {}", e),
            })?;

            let client = pumas_app_manager::TorchClient::new(connection_url);
            client.configure(&config).await?;

            Ok(json!({ "success": true }))
        }

        "launch_torch" => {
            let managers = state.version_managers.read().await;
            info!("launch_torch: checking for torch version manager");
            if let Some(vm) = managers.get("torch") {
                let active = vm.get_active_version().await?;
                info!("launch_torch: active version: {:?}", active);
                if let Some(tag) = active {
                    let version_dir = vm.version_path(&tag);
                    info!("launch_torch: launching tag={} from {:?}", tag, version_dir);
                    drop(managers);
                    let response = api.launch_torch(&tag, &version_dir).await?;
                    info!("launch_torch: result success={}", response.success);
                    Ok(serde_json::to_value(response)?)
                } else {
                    warn!("launch_torch: no active version set");
                    Ok(json!({
                        "success": false,
                        "error": "No active Torch version set"
                    }))
                }
            } else {
                warn!("launch_torch: version manager not initialized");
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized for torch"
                }))
            }
        }

        "stop_torch" => {
            let result = api.stop_torch().await?;
            Ok(json!({ "success": result }))
        }

        "is_torch_running" => {
            let running = api.is_torch_running().await;
            Ok(serde_json::to_value(running)?)
        }

        // ====================================================================
        // Plugin System
        // ====================================================================
        "get_plugins" => {
            let plugins = state.plugin_loader.get_enabled();
            Ok(json!({
                "success": true,
                "plugins": serde_json::to_value(plugins)?
            }))
        }

        "get_plugin" => {
            let app_id = require_str_param!(params, "app_id", "appId");
            let plugin = state.plugin_loader.get(&app_id);
            match plugin {
                Some(config) => Ok(json!({
                    "success": true,
                    "plugin": serde_json::to_value(config)?
                })),
                None => Ok(json!({
                    "success": false,
                    "error": format!("Plugin not found: {}", app_id)
                })),
            }
        }

        "check_plugin_health" => {
            let app_id = require_str_param!(params, "app_id", "appId");
            let plugin = state.plugin_loader.get(&app_id);
            match plugin {
                Some(config) => {
                    if let Some(conn) = &config.connection {
                        let health_endpoint = conn.health_endpoint.as_deref().unwrap_or("/health");
                        let url = format!("{}://localhost:{}{}", conn.protocol, conn.default_port, health_endpoint);
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(3))
                            .build()
                            .unwrap_or_default();
                        let healthy = client.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false);
                        Ok(json!({
                            "success": true,
                            "healthy": healthy
                        }))
                    } else {
                        Ok(json!({
                            "success": true,
                            "healthy": false
                        }))
                    }
                }
                None => Ok(json!({
                    "success": false,
                    "error": format!("Plugin not found: {}", app_id),
                    "healthy": false
                })),
            }
        }

        "get_app_status" => {
            let app_id = require_str_param!(params, "app_id", "appId");
            let running = match app_id.as_str() {
                "comfyui" => api.is_comfyui_running().await,
                "ollama" => api.is_ollama_running().await,
                "torch" => api.is_torch_running().await,
                _ => false,
            };
            Ok(json!({
                "success": true,
                "running": running
            }))
        }

        // ====================================================================
        // Shortcuts (uses version_manager for version_dir)
        // ====================================================================
        "get_version_shortcuts" => {
            let tag = require_str_param!(params, "tag", "tag");
            let sm_lock = state.shortcut_manager.read().await;
            if let Some(ref sm) = *sm_lock {
                let shortcut_state = sm.get_version_shortcut_state(&tag);
                Ok(json!({
                    "tag": shortcut_state.tag,
                    "menu": shortcut_state.menu,
                    "desktop": shortcut_state.desktop
                }))
            } else {
                Ok(json!({
                    "tag": tag,
                    "menu": false,
                    "desktop": false
                }))
            }
        }

        "get_all_shortcut_states" => {
            let sm_lock = state.shortcut_manager.read().await;
            if let Some(ref sm) = *sm_lock {
                let states = sm.get_all_shortcut_states();
                let result: HashMap<String, serde_json::Value> = states
                    .into_iter()
                    .map(|(tag, state)| (tag, json!({
                        "tag": state.tag,
                        "menu": state.menu,
                        "desktop": state.desktop
                    })))
                    .collect();
                Ok(json!(result))
            } else {
                Ok(json!({}))
            }
        }

        "toggle_menu" => {
            let tag = get_str_param!(params, "tag", "tag");
            if let Some(t) = tag {
                let managers = state.version_managers.read().await;
                if let Some(vm) = managers.get("comfyui") {
                    let version_dir = vm.version_path(t);
                    drop(managers);
                    let sm_lock = state.shortcut_manager.read().await;
                    if let Some(ref sm) = *sm_lock {
                        match sm.toggle_menu_shortcut(t, &version_dir) {
                            Ok(result) => Ok(json!(result.success)),
                            Err(e) => Ok(json!({"success": false, "error": e.to_string()}))
                        }
                    } else {
                        Ok(json!(false))
                    }
                } else {
                    Ok(json!(false))
                }
            } else {
                Ok(json!(false))
            }
        }

        "toggle_desktop" => {
            let tag = get_str_param!(params, "tag", "tag");
            if let Some(t) = tag {
                let managers = state.version_managers.read().await;
                if let Some(vm) = managers.get("comfyui") {
                    let version_dir = vm.version_path(t);
                    drop(managers);
                    let sm_lock = state.shortcut_manager.read().await;
                    if let Some(ref sm) = *sm_lock {
                        match sm.toggle_desktop_shortcut(t, &version_dir) {
                            Ok(result) => Ok(json!(result.success)),
                            Err(e) => Ok(json!({"success": false, "error": e.to_string()}))
                        }
                    } else {
                        Ok(json!(false))
                    }
                } else {
                    Ok(json!(false))
                }
            } else {
                Ok(json!(false))
            }
        }

        // Legacy shortcut methods (deprecated but still supported)
        "menu_exists" => {
            let sm_lock = state.shortcut_manager.read().await;
            if let Some(ref sm) = *sm_lock {
                Ok(json!(sm.menu_exists()))
            } else {
                Ok(json!(false))
            }
        }

        "desktop_exists" => {
            let sm_lock = state.shortcut_manager.read().await;
            if let Some(ref sm) = *sm_lock {
                Ok(json!(sm.desktop_exists()))
            } else {
                Ok(json!(false))
            }
        }

        "install_icon" => {
            // Legacy method - icons are installed with shortcuts now
            Ok(json!(true))
        }

        "create_menu_shortcut" => {
            // Legacy method - use toggle_menu instead
            Ok(json!(false))
        }

        "create_desktop_shortcut" => {
            // Legacy method - use toggle_desktop instead
            Ok(json!(false))
        }

        "remove_menu_shortcut" => {
            // Legacy method - use toggle_menu instead
            Ok(json!(false))
        }

        "remove_desktop_shortcut" => {
            // Legacy method - use toggle_desktop instead
            Ok(json!(false))
        }

        // ====================================================================
        // System Utilities
        // ====================================================================
        "open_path" => {
            let path = require_str_param!(params, "path", "path");
            match api.open_path(&path) {
                Ok(()) => Ok(json!({"success": true})),
                Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
            }
        }

        "open_url" => {
            let url = require_str_param!(params, "url", "url");
            match api.open_url(&url) {
                Ok(()) => Ok(json!({"success": true})),
                Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
            }
        }

        "open_active_install" => {
            let app_id_str = get_str_param!(params, "app_id", "appId").unwrap_or("comfyui");
            // Get the active version from version_manager and open its directory
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get(app_id_str) {
                if let Some(tag) = vm.get_active_version().await? {
                    let version_dir = vm.version_path(&tag);
                    drop(managers);
                    if version_dir.exists() {
                        match api.open_directory(&version_dir) {
                            Ok(()) => Ok(json!({"success": true})),
                            Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
                        }
                    } else {
                        Ok(json!({"success": false, "error": "Version directory not found"}))
                    }
                } else {
                    Ok(json!({"success": false, "error": "No active version set"}))
                }
            } else {
                Ok(json!({"success": false, "error": format!("Version manager not initialized for app: {}", app_id_str)}))
            }
        }

        // ====================================================================
        // Model Library
        // ====================================================================
        "get_models" => {
            let models = api.list_models().await?;
            // Convert to a format with model_id as keys for frontend compatibility
            let mut result = serde_json::Map::new();
            for model in models {
                result.insert(model.id.clone(), serde_json::to_value(&model)?);
            }
            Ok(json!(result))
        }

        "refresh_model_index" => {
            let count = api.rebuild_model_index().await?;
            Ok(json!({
                "success": true,
                "indexed_count": count
            }))
        }

        "refresh_model_mappings" => {
            let _app_id = get_str_param!(params, "app_id", "appId");
            // TODO: Implement model mapping refresh
            Ok(json!({}))
        }

        "import_model" => {
            let local_path = require_str_param!(params, "local_path", "localPath");
            let family = require_str_param!(params, "family", "family");
            let official_name = require_str_param!(params, "official_name", "officialName");
            let repo_id = get_str_param!(params, "repo_id", "repoId").map(String::from);
            let model_type = get_str_param!(params, "model_type", "modelType").map(String::from);
            let subtype = get_str_param!(params, "subtype", "subtype").map(String::from);
            let security_acknowledged = get_bool_param!(params, "security_acknowledged", "securityAcknowledged");

            let spec = pumas_library::model_library::ModelImportSpec {
                path: local_path,
                family,
                official_name,
                repo_id,
                model_type,
                subtype,
                tags: None,
                security_acknowledged,
            };

            let result = api.import_model(&spec).await?;
            Ok(serde_json::to_value(result)?)
        }

        "download_model_from_hf" => {
            let repo_id = require_str_param!(params, "repo_id", "repoId");
            let family = require_str_param!(params, "family", "family");
            let official_name = require_str_param!(params, "official_name", "officialName");
            let model_type = get_str_param!(params, "model_type", "modelType").map(String::from);
            let quant = get_str_param!(params, "quant", "quant").map(String::from);
            let filename = get_str_param!(params, "filename", "filename").map(String::from);

            let request = pumas_library::DownloadRequest {
                repo_id,
                family,
                official_name,
                model_type,
                quant,
                filename,
            };

            match api.start_hf_download(&request).await {
                Ok(download_id) => Ok(json!({
                    "success": true,
                    "download_id": download_id
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                })),
            }
        }

        "start_model_download_from_hf" => {
            let repo_id = require_str_param!(params, "repo_id", "repoId");
            let family = require_str_param!(params, "family", "family");
            let official_name = require_str_param!(params, "official_name", "officialName");
            let model_type = get_str_param!(params, "model_type", "modelType").map(String::from);
            let quant = get_str_param!(params, "quant", "quant").map(String::from);
            let filename = get_str_param!(params, "filename", "filename").map(String::from);

            let request = pumas_library::DownloadRequest {
                repo_id,
                family,
                official_name,
                model_type,
                quant,
                filename,
            };

            match api.start_hf_download(&request).await {
                Ok(download_id) => Ok(json!({
                    "success": true,
                    "download_id": download_id
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                })),
            }
        }

        "get_model_download_status" => {
            let download_id = require_str_param!(params, "download_id", "downloadId");
            match api.get_hf_download_progress(&download_id).await {
                Some(progress) => {
                    let mut response = serde_json::to_value(progress)?;
                    if let Some(obj) = response.as_object_mut() {
                        obj.insert("success".to_string(), json!(true));
                    }
                    Ok(response)
                }
                None => Ok(json!({
                    "success": false,
                    "error": "Download not found"
                })),
            }
        }

        "cancel_model_download" => {
            let download_id = require_str_param!(params, "download_id", "downloadId");
            match api.cancel_hf_download(&download_id).await {
                Ok(cancelled) => Ok(json!({
                    "success": cancelled
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                })),
            }
        }

        "pause_model_download" => {
            let download_id = require_str_param!(params, "download_id", "downloadId");
            match api.pause_hf_download(&download_id).await {
                Ok(paused) => Ok(json!({
                    "success": paused
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                })),
            }
        }

        "resume_model_download" => {
            let download_id = require_str_param!(params, "download_id", "downloadId");
            match api.resume_hf_download(&download_id).await {
                Ok(resumed) => Ok(json!({
                    "success": resumed
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                })),
            }
        }

        "list_model_downloads" => {
            let downloads = api.list_hf_downloads().await;
            Ok(json!({
                "success": true,
                "downloads": downloads
            }))
        }

        "search_hf_models" => {
            let query = require_str_param!(params, "query", "query");
            let kind = get_str_param!(params, "kind", "kind");
            let limit = get_i64_param!(params, "limit", "limit").unwrap_or(25) as usize;

            match api.search_hf_models(&query, kind, limit).await {
                Ok(models) => Ok(json!({
                    "success": true,
                    "models": models
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "models": [],
                    "error": e.to_string()
                })),
            }
        }

        "get_related_models" => {
            let model_id = require_str_param!(params, "model_id", "modelId");
            let limit = get_i64_param!(params, "limit", "limit").unwrap_or(25) as usize;
            // Use the model's name to search for related models on HuggingFace
            let models = match api.get_model(&model_id).await {
                Ok(Some(model)) => {
                    api.search_hf_models(&model.official_name, None, limit).await.unwrap_or_default()
                }
                _ => vec![],
            };
            Ok(json!({
                "success": true,
                "models": models
            }))
        }

        "search_models_fts" => {
            let query = require_str_param!(params, "query", "query");
            let limit = get_i64_param!(params, "limit", "limit").unwrap_or(100) as usize;
            let offset = get_i64_param!(params, "offset", "offset").unwrap_or(0) as usize;

            match api.search_models(&query, limit, offset).await {
                Ok(result) => Ok(json!({
                    "success": true,
                    "models": result.models,
                    "total_count": result.total_count,
                    "query_time_ms": result.query_time_ms,
                    "query": result.query
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "models": [],
                    "total_count": 0,
                    "query_time_ms": 0,
                    "query": query,
                    "error": e.to_string()
                })),
            }
        }

        "import_batch" => {
            // Parse the imports array from params
            let imports: Vec<pumas_library::model_library::ModelImportSpec> = params
                .get("imports")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            let results = api.import_models_batch(imports).await;
            let imported = results.iter().filter(|r| r.success).count();
            let failed = results.iter().filter(|r| !r.success).count();

            Ok(json!({
                "success": true,
                "imported": imported,
                "failed": failed,
                "results": results
            }))
        }

        "lookup_hf_metadata_for_file" => {
            let file_path = require_str_param!(params, "file_path", "filePath");

            match api.lookup_hf_metadata_for_file(&file_path).await {
                Ok(Some(metadata)) => Ok(json!({
                    "success": true,
                    "metadata": metadata
                })),
                Ok(None) => Ok(json!({
                    "success": false,
                    "error": "No metadata found"
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                })),
            }
        }

        "detect_sharded_sets" => {
            // Get files array from params
            let files: Vec<String> = params
                .get("files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            // Convert to PathBuf
            let paths: Vec<std::path::PathBuf> = files.iter().map(std::path::PathBuf::from).collect();

            // Detect sharded sets
            let sets = pumas_library::sharding::detect_sharded_sets(&paths);

            // Convert to serializable format
            let result: std::collections::HashMap<String, Vec<String>> = sets
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(|p| p.to_string_lossy().to_string()).collect()))
                .collect();

            Ok(json!({
                "success": true,
                "sets": result
            }))
        }

        "validate_file_type" => {
            let file_path = require_str_param!(params, "file_path", "filePath");
            // TODO: Implement file type validation
            Ok(json!({
                "success": true,
                "valid": true,
                "detected_type": null
            }))
        }

        "mark_metadata_as_manual" => {
            let model_id = require_str_param!(params, "model_id", "modelId");
            match api.mark_model_metadata_as_manual(&model_id).await {
                Ok(()) => Ok(json!({
                    "success": true
                })),
                Err(e) => Ok(json!({
                    "success": false,
                    "error": e.to_string()
                })),
            }
        }

        "get_embedded_metadata" => {
            let file_path = require_str_param!(params, "file_path", "filePath");
            let path = std::path::Path::new(&file_path);

            // Detect file type from extension
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            match extension.as_str() {
                "gguf" => {
                    match pumas_library::model_library::extract_gguf_metadata(&file_path) {
                        Ok(metadata) => {
                            // Convert HashMap<String, String> to Value
                            let metadata_value: serde_json::Map<String, Value> = metadata
                                .into_iter()
                                .map(|(k, v)| (k, Value::String(v)))
                                .collect();
                            Ok(json!({
                                "success": true,
                                "file_type": "gguf",
                                "metadata": metadata_value
                            }))
                        }
                        Err(e) => Ok(json!({
                            "success": false,
                            "file_type": "gguf",
                            "error": e.to_string(),
                            "metadata": null
                        }))
                    }
                }
                "safetensors" => {
                    // Read safetensors JSON header
                    match extract_safetensors_header(&file_path) {
                        Ok(header) => Ok(json!({
                            "success": true,
                            "file_type": "safetensors",
                            "metadata": header
                        })),
                        Err(e) => Ok(json!({
                            "success": false,
                            "file_type": "safetensors",
                            "error": e,
                            "metadata": null
                        }))
                    }
                }
                _ => Ok(json!({
                    "success": false,
                    "file_type": "unsupported",
                    "error": format!("Unsupported file type: {}", extension),
                    "metadata": null
                }))
            }
        }

        "get_library_model_metadata" => {
            let model_id = require_str_param!(params, "model_id", "modelId");

            // Get the library
            let library = api.model_library();

            // Get stored metadata from metadata.json
            let model_dir = library.library_root().join(&model_id);
            let stored_metadata = library.load_metadata(&model_dir)?;

            // Find primary model file and get embedded metadata
            let primary_file = library.get_primary_model_file(&model_id);
            let embedded_metadata: Option<Value> = if let Some(ref file_path) = primary_file {
                let extension = file_path
                    .extension()
                    .and_then(|e: &std::ffi::OsStr| e.to_str())
                    .map(|s: &str| s.to_lowercase())
                    .unwrap_or_default();

                match extension.as_str() {
                    "gguf" => {
                        match pumas_library::model_library::extract_gguf_metadata(file_path) {
                            Ok(metadata) => {
                                let metadata_value: serde_json::Map<String, Value> = metadata
                                    .into_iter()
                                    .map(|(k, v)| (k, Value::String(v)))
                                    .collect();
                                Some(json!({
                                    "file_type": "gguf",
                                    "metadata": metadata_value
                                }))
                            }
                            Err(_) => None
                        }
                    }
                    "safetensors" => {
                        match extract_safetensors_header(&file_path.to_string_lossy()) {
                            Ok(header) => Some(json!({
                                "file_type": "safetensors",
                                "metadata": header
                            })),
                            Err(_) => None
                        }
                    }
                    _ => None
                }
            } else {
                None
            };

            let primary_file_str = primary_file.map(|p: std::path::PathBuf| p.to_string_lossy().to_string());

            Ok(json!({
                "success": true,
                "model_id": model_id,
                "stored_metadata": stored_metadata,
                "embedded_metadata": embedded_metadata,
                "primary_file": primary_file_str
            }))
        }

        "refetch_model_metadata_from_hf" => {
            let model_id = require_str_param!(params, "model_id", "modelId");

            let updated = api.refetch_metadata_from_hf(&model_id).await?;
            Ok(json!({
                "success": true,
                "model_id": model_id,
                "metadata": serde_json::to_value(&updated)?
            }))
        }

        // ====================================================================
        // Model Overrides
        // ====================================================================
        "get_model_overrides" => {
            let rel_path = require_str_param!(params, "rel_path", "relPath");
            // TODO: Implement model overrides
            Ok(json!({}))
        }

        "update_model_overrides" => {
            let rel_path = require_str_param!(params, "rel_path", "relPath");
            // TODO: Implement model overrides update
            Ok(json!(false))
        }

        // ====================================================================
        // Link Management
        // ====================================================================
        "get_link_health" => {
            let version_tag = get_str_param!(params, "version_tag", "versionTag");
            let response = api.get_link_health(version_tag).await?;
            Ok(serde_json::to_value(response)?)
        }

        "clean_broken_links" => {
            let response = api.clean_broken_links().await?;
            Ok(serde_json::to_value(response)?)
        }

        "remove_orphaned_links" => {
            let _version_tag = require_str_param!(params, "version_tag", "versionTag");
            // Orphaned links are handled as part of clean_broken_links
            let response = api.clean_broken_links().await?;
            Ok(json!({
                "success": response.success,
                "removed": response.cleaned
            }))
        }

        "get_links_for_model" => {
            let model_id = require_str_param!(params, "model_id", "modelId");
            let response = api.get_links_for_model(&model_id).await?;
            Ok(serde_json::to_value(response)?)
        }

        "delete_model_with_cascade" => {
            let model_id = require_str_param!(params, "model_id", "modelId");
            let response = api.delete_model_with_cascade(&model_id).await?;
            Ok(serde_json::to_value(response)?)
        }

        "preview_model_mapping" => {
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            // Get the models path from comfyui version_manager
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get("comfyui") {
                let version_path = vm.version_path(&version_tag);
                let models_path = version_path.join("models");
                drop(managers);
                let response = api.preview_model_mapping(&version_tag, &models_path).await?;
                Ok(serde_json::to_value(response)?)
            } else {
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized for comfyui"
                }))
            }
        }

        "apply_model_mapping" => {
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            // Get the models path from comfyui version_manager
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get("comfyui") {
                let version_path = vm.version_path(&version_tag);
                let models_path = version_path.join("models");
                drop(managers);
                let response = api.apply_model_mapping(&version_tag, &models_path).await?;
                Ok(serde_json::to_value(response)?)
            } else {
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized for comfyui"
                }))
            }
        }

        "sync_models_incremental" => {
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            // Get the models path from comfyui version_manager
            let managers = state.version_managers.read().await;
            if let Some(vm) = managers.get("comfyui") {
                let version_path = vm.version_path(&version_tag);
                let models_path = version_path.join("models");
                drop(managers);
                let response = api.sync_models_incremental(&version_tag, &models_path).await?;
                Ok(serde_json::to_value(response)?)
            } else {
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized for comfyui"
                }))
            }
        }

        "sync_with_resolutions" => {
            // TODO: Implement sync with resolutions
            Ok(json!({
                "success": true,
                "synced": 0
            }))
        }

        "get_cross_filesystem_warning" => {
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            // TODO: Check for cross-filesystem issues
            Ok(json!({
                "success": true,
                "warning": null,
                "affected_models": []
            }))
        }

        "get_file_link_count" => {
            let file_path = require_str_param!(params, "file_path", "filePath");
            // Count hard links to a file
            let path = std::path::Path::new(&file_path);
            if path.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(metadata) = std::fs::metadata(path) {
                        return Ok(json!({
                            "success": true,
                            "count": metadata.nlink()
                        }));
                    }
                }
            }
            Ok(json!({
                "success": true,
                "count": 1
            }))
        }

        "check_files_writable" => {
            // Check if files can be written/modified
            let file_paths: Vec<String> = params
                .get("file_paths")
                .or_else(|| params.get("filePaths"))
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            let results: Vec<_> = file_paths
                .iter()
                .map(|p| {
                    let path = std::path::Path::new(p);
                    let writable = if path.exists() {
                        std::fs::metadata(path)
                            .map(|m| !m.permissions().readonly())
                            .unwrap_or(false)
                    } else {
                        // Check if parent directory is writable
                        path.parent()
                            .map(|parent| {
                                std::fs::metadata(parent)
                                    .map(|m| !m.permissions().readonly())
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    };
                    json!({
                        "path": p,
                        "writable": writable
                    })
                })
                .collect();

            Ok(json!({
                "success": true,
                "results": results
            }))
        }

        // ====================================================================
        // Custom Nodes (uses pumas-app-manager)
        // ====================================================================
        "get_custom_nodes" => {
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            let nodes = state.custom_nodes_manager.list_custom_nodes(&version_tag)?;
            Ok(serde_json::to_value(nodes)?)
        }

        "install_custom_node" => {
            let repo_url = require_str_param!(params, "repo_url", "repoUrl");
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            let result = state.custom_nodes_manager.install_from_git(&repo_url, &version_tag).await?;
            Ok(serde_json::to_value(result)?)
        }

        "update_custom_node" => {
            let node_name = require_str_param!(params, "node_name", "nodeName");
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            let result = state.custom_nodes_manager.update(&node_name, &version_tag).await?;
            Ok(serde_json::to_value(result)?)
        }

        "remove_custom_node" => {
            let node_name = require_str_param!(params, "node_name", "nodeName");
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            let result = state.custom_nodes_manager.remove(&node_name, &version_tag)?;
            Ok(json!({"success": result}))
        }

        // ====================================================================
        // Scan & Discovery
        // ====================================================================
        "scan_shared_storage" => {
            // Rebuild the model index from metadata files on disk
            let count = api.rebuild_model_index().await?;
            Ok(json!({
                "modelsFound": count,
                "scanned": count,
                "indexed": count
            }))
        }

        // ====================================================================
        // Network Status
        // ====================================================================
        "get_network_status" => {
            // TODO: Implement network status from network layer
            Ok(json!({
                "success": true,
                "total_requests": 0,
                "successful_requests": 0,
                "failed_requests": 0,
                "circuit_breaker_rejections": 0,
                "retries": 0,
                "success_rate": 1.0,
                "circuit_states": {},
                "is_offline": false
            }))
        }

        // ====================================================================
        // Library Status
        // ====================================================================
        "get_library_status" => {
            // TODO: Implement library status
            Ok(json!({
                "success": true,
                "indexing": false,
                "deep_scan_in_progress": false,
                "model_count": 0,
                "pending_lookups": null,
                "deep_scan_progress": null
            }))
        }

        // ====================================================================
        // Library Maintenance
        // ====================================================================
        "adopt_orphan_models" => {
            let result = api.adopt_orphan_models().await?;
            Ok(serde_json::to_value(result)?)
        }

        "import_model_in_place" => {
            let model_dir = require_str_param!(params, "model_dir", "modelDir");
            let official_name = require_str_param!(params, "official_name", "officialName");
            let family = require_str_param!(params, "family", "family");
            let model_type = get_str_param!(params, "model_type", "modelType").map(String::from);
            let repo_id = get_str_param!(params, "repo_id", "repoId").map(String::from);
            let known_sha256 = get_str_param!(params, "known_sha256", "knownSha256").map(String::from);
            let compute_hashes = get_bool_param!(params, "compute_hashes", "computeHashes").unwrap_or(false);

            let expected_files: Option<Vec<String>> = params
                .get("expected_files")
                .or_else(|| params.get("expectedFiles"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

            let spec = pumas_library::model_library::InPlaceImportSpec {
                model_dir: std::path::PathBuf::from(model_dir),
                official_name,
                family,
                model_type,
                repo_id,
                known_sha256,
                compute_hashes,
                expected_files,
            };

            let result = api.import_model_in_place(&spec).await?;
            Ok(serde_json::to_value(result)?)
        }

        // ====================================================================
        // System Checks
        // ====================================================================
        "check_git" => {
            let result = api.check_git();
            Ok(serde_json::to_value(result)?)
        }

        "check_brave" => {
            let result = api.check_brave();
            Ok(serde_json::to_value(result)?)
        }

        "check_setproctitle" => {
            let result = api.check_setproctitle();
            Ok(serde_json::to_value(result)?)
        }

        // ====================================================================
        // Model Format Conversion
        // ====================================================================
        "start_model_conversion" => {
            let model_id = require_str_param!(params, "model_id", "modelId");
            let direction = require_str_param!(params, "direction", "direction");
            let target_quant = get_str_param!(params, "target_quant", "targetQuant").map(String::from);
            let output_name = get_str_param!(params, "output_name", "outputName").map(String::from);
            let imatrix_calibration_file = get_str_param!(params, "imatrix_calibration_file", "imatrixCalibrationFile").map(String::from);
            let force_imatrix = get_bool_param!(params, "force_imatrix", "forceImatrix");

            let direction = match direction.as_str() {
                "gguf_to_safetensors" | "GgufToSafetensors" => {
                    pumas_library::conversion::ConversionDirection::GgufToSafetensors
                }
                "safetensors_to_gguf" | "SafetensorsToGguf" => {
                    pumas_library::conversion::ConversionDirection::SafetensorsToGguf
                }
                "safetensors_to_quantized_gguf" | "SafetensorsToQuantizedGguf" => {
                    pumas_library::conversion::ConversionDirection::SafetensorsToQuantizedGguf
                }
                "gguf_to_quantized_gguf" | "GgufToQuantizedGguf" => {
                    pumas_library::conversion::ConversionDirection::GgufToQuantizedGguf
                }
                "safetensors_to_nvfp4" | "SafetensorsToNvfp4" => {
                    pumas_library::conversion::ConversionDirection::SafetensorsToNvfp4
                }
                "safetensors_to_sherry_qat" | "SafetensorsToSherryQat" => {
                    pumas_library::conversion::ConversionDirection::SafetensorsToSherryQat
                }
                _ => {
                    return Err(pumas_library::PumasError::InvalidParams {
                        message: format!("Invalid conversion direction: {}", direction),
                    });
                }
            };

            let request = pumas_library::conversion::ConversionRequest {
                model_id,
                direction,
                target_quant,
                output_name,
                imatrix_calibration_file,
                force_imatrix,
            };

            let conversion_id = api.start_conversion(request).await?;
            Ok(json!({
                "success": true,
                "conversion_id": conversion_id
            }))
        }

        "get_conversion_progress" => {
            let conversion_id = require_str_param!(params, "conversion_id", "conversionId");
            let progress = api.get_conversion_progress(&conversion_id);
            Ok(json!({
                "success": true,
                "progress": progress
            }))
        }

        "cancel_model_conversion" => {
            let conversion_id = require_str_param!(params, "conversion_id", "conversionId");
            let cancelled = api.cancel_conversion(&conversion_id).await?;
            Ok(json!({
                "success": true,
                "cancelled": cancelled
            }))
        }

        "list_model_conversions" => {
            let conversions = api.list_conversions();
            Ok(json!({
                "success": true,
                "conversions": conversions
            }))
        }

        "check_conversion_environment" => {
            let ready = api.is_conversion_environment_ready();
            Ok(json!({
                "success": true,
                "ready": ready
            }))
        }

        "setup_conversion_environment" => {
            api.ensure_conversion_environment().await?;
            Ok(json!({
                "success": true
            }))
        }

        "get_supported_quant_types" => {
            let types = api.supported_quant_types();
            Ok(json!({
                "success": true,
                "quant_types": types
            }))
        }

        "get_backend_status" => {
            let status = api.backend_status();
            Ok(json!({
                "success": true,
                "backends": status
            }))
        }

        "setup_quantization_backend" => {
            let backend = require_str_param!(params, "backend", "backend");
            let backend = match backend.as_str() {
                "llama_cpp" | "LlamaCpp" => pumas_library::conversion::QuantBackend::LlamaCpp,
                "nvfp4" | "Nvfp4" => pumas_library::conversion::QuantBackend::Nvfp4,
                "sherry" | "Sherry" => pumas_library::conversion::QuantBackend::Sherry,
                "python_conversion" | "PythonConversion" => {
                    pumas_library::conversion::QuantBackend::PythonConversion
                }
                _ => {
                    return Err(pumas_library::PumasError::InvalidParams {
                        message: format!("Unknown quantization backend: {}", backend),
                    });
                }
            };

            api.ensure_backend_environment(backend).await?;
            Ok(json!({
                "success": true
            }))
        }

        // ====================================================================
        // Not Yet Implemented / Unknown
        // ====================================================================
        _ => {
            warn!("Method not found: {}", method);
            Err(pumas_library::PumasError::Other(format!(
                "Method not found: {}",
                method
            )))
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Synchronize version paths from ComfyUI version_manager to process_manager.
///
/// This ensures the process manager knows about all installed version directories
/// so it can properly detect and clean up PID files.
async fn sync_version_paths_to_process_manager(state: &AppState) {
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get("comfyui") {
        // Get installed versions
        if let Ok(installed) = vm.get_installed_versions().await {
            let version_paths: HashMap<String, PathBuf> = installed
                .into_iter()
                .map(|tag| {
                    let path = vm.version_path(&tag);
                    (tag, path)
                })
                .collect();

            // Update process manager
            drop(managers);  // Release version_managers lock first
            state.api.set_process_version_paths(version_paths).await;
        }
    }
}

/// Detect if running in a sandbox environment.
fn detect_sandbox_environment() -> (bool, &'static str, Vec<&'static str>) {
    // Check for Flatpak
    if std::path::Path::new("/.flatpak-info").exists() {
        return (
            true,
            "flatpak",
            vec![
                "Limited filesystem access",
                "May need portal permissions for some operations",
            ],
        );
    }

    // Check for Snap
    if std::env::var("SNAP").is_ok() {
        return (
            true,
            "snap",
            vec![
                "Limited filesystem access",
                "Strict confinement may limit features",
            ],
        );
    }

    // Check for Docker
    if std::path::Path::new("/.dockerenv").exists() {
        return (
            true,
            "docker",
            vec![
                "Running in container",
                "GPU access may require --gpus flag",
            ],
        );
    }

    // Check for AppImage
    if std::env::var("APPIMAGE").is_ok() {
        return (
            true,
            "appimage",
            vec!["Running as AppImage bundle"],
        );
    }

    (false, "none", vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_response_success() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!({"data": "test"}));
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let response = JsonRpcResponse::error(Some(json!(1)), -32600, "Test error".into());
        assert!(response.error.is_some());
        assert!(response.result.is_none());
        assert_eq!(response.error.unwrap().code, -32600);
    }

    #[test]
    fn test_detect_sandbox() {
        let (is_sandboxed, sandbox_type, _) = detect_sandbox_environment();
        // In normal development, we're not sandboxed
        // This test verifies the function runs without error
        assert!(
            !is_sandboxed || ["flatpak", "snap", "docker", "appimage"].contains(&sandbox_type)
        );
    }
}
