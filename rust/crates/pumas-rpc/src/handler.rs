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
use std::sync::Arc;
use tracing::{debug, error, warn};

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
                return Err(pumas_core::PumasError::InvalidParams {
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
            .and_then(pumas_core::AppId::from_str)
    };
}

// ============================================================================
// Method dispatcher
// ============================================================================

/// Dispatch a method call to the appropriate API handler.
async fn dispatch_method(
    state: &AppState,
    method: &str,
    params: &Value,
) -> pumas_core::Result<Value> {
    let api = &state.api;
    match method {
        // ====================================================================
        // Status & System
        // ====================================================================
        "get_status" => {
            let response = api.get_status().await?;
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
            let _app_id = get_app_id!(params);

            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                // Handle rate limit errors specially to return structured response
                match vm.get_available_releases(force_refresh).await {
                    Ok(releases) => {
                        let versions: Vec<pumas_core::models::VersionReleaseInfo> = releases
                            .into_iter()
                            .map(pumas_core::models::VersionReleaseInfo::from)
                            .collect();
                        Ok(json!({
                            "success": true,
                            "versions": versions
                        }))
                    }
                    Err(pumas_core::PumasError::RateLimited { service, retry_after_secs }) => {
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
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let versions = vm.get_installed_versions().await?;
                Ok(serde_json::to_value(versions)?)
            } else {
                Ok(serde_json::to_value::<Vec<String>>(vec![])?)
            }
        }

        "get_active_version" => {
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let version = vm.get_active_version().await?;
                Ok(serde_json::to_value(version)?)
            } else {
                Ok(serde_json::to_value::<Option<String>>(None)?)
            }
        }

        "get_default_version" => {
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let version = vm.get_default_version().await?;
                Ok(serde_json::to_value(version)?)
            } else {
                Ok(serde_json::to_value::<Option<String>>(None)?)
            }
        }

        "set_default_version" => {
            let tag = get_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let result = vm.set_default_version(tag).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_core::PumasError::Config {
                    message: "Version manager not initialized".to_string(),
                })
            }
        }

        "switch_version" => {
            let tag = require_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let result = vm.set_active_version(&tag).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_core::PumasError::Config {
                    message: "Version manager not initialized".to_string(),
                })
            }
        }

        "install_version" => {
            let tag = require_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);

            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
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
                    "error": "Version manager not initialized"
                }))
            }
        }

        "remove_version" => {
            let tag = require_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let result = vm.remove_version(&tag).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_core::PumasError::Config {
                    message: "Version manager not initialized".to_string(),
                })
            }
        }

        "cancel_installation" => {
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let result = vm.cancel_installation().await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Ok(serde_json::to_value(false)?)
            }
        }

        "get_installation_progress" => {
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let progress = vm.get_installation_progress().await;
                Ok(serde_json::to_value(progress)?)
            } else {
                Ok(serde_json::to_value::<Option<pumas_core::models::InstallationProgress>>(None)?)
            }
        }

        "validate_installations" => {
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
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
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                // Return version status combining active/default/installed
                let active = vm.get_active_version().await?;
                let default = vm.get_default_version().await?;
                let installed = vm.get_installed_versions().await?;
                Ok(json!({
                    "active": active,
                    "default": default,
                    "installed": installed
                }))
            } else {
                Ok(json!({
                    "active": null,
                    "default": null,
                    "installed": []
                }))
            }
        }

        "get_version_info" => {
            let tag = require_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
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
            let vm_lock = state.version_manager.read().await;
            let versions = if let Some(ref vm) = *vm_lock {
                let releases = vm.get_available_releases(false).await?;
                releases
                    .into_iter()
                    .map(pumas_core::models::VersionReleaseInfo::from)
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            drop(vm_lock);

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
            let _app_id = get_app_id!(params);
            // TODO: Implement cache status
            Ok(json!({
                "cached": false,
                "last_fetch": null,
                "cache_age_seconds": null
            }))
        }

        // ====================================================================
        // Dependency Management (uses pumas-app-manager)
        // ====================================================================
        "check_version_dependencies" => {
            let tag = require_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let status = vm.check_dependencies(&tag).await?;
                Ok(serde_json::to_value(status)?)
            } else {
                Err(pumas_core::PumasError::Config {
                    message: "Version manager not initialized".to_string(),
                })
            }
        }

        "install_version_dependencies" => {
            let tag = require_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let result = vm.install_dependencies(&tag, None).await?;
                Ok(serde_json::to_value(result)?)
            } else {
                Err(pumas_core::PumasError::Config {
                    message: "Version manager not initialized".to_string(),
                })
            }
        }

        "get_release_dependencies" => {
            let tag = require_str_param!(params, "tag", "tag");
            let _app_id = get_app_id!(params);
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let version_path = vm.version_path(&tag);
                let requirements_path = version_path.join("requirements.txt");

                if !requirements_path.exists() {
                    return Ok(serde_json::to_value::<Vec<String>>(vec![])?);
                }

                let content = std::fs::read_to_string(&requirements_path).map_err(|e| {
                    pumas_core::PumasError::Io {
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
                Err(pumas_core::PumasError::Config {
                    message: "Version manager not initialized".to_string(),
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
            let running = api.is_comfyui_running().await;
            Ok(serde_json::to_value(running)?)
        }

        "stop_comfyui" => {
            let result = api.stop_comfyui().await?;
            Ok(serde_json::to_value(result)?)
        }

        "launch_comfyui" => {
            // Get the active version from version_manager and launch it
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let active = vm.get_active_version().await?;
                if let Some(tag) = active {
                    let version_dir = vm.version_path(&tag);
                    drop(vm_lock);  // Release lock before calling api
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
                    "error": "Version manager not initialized"
                }))
            }
        }

        // ====================================================================
        // Shortcuts (uses version_manager for version_dir)
        // ====================================================================
        "get_version_shortcuts" => {
            let tag = require_str_param!(params, "tag", "tag");
            let shortcut_state = api.get_version_shortcut_state(&tag).await;
            Ok(serde_json::to_value(shortcut_state)?)
        }

        "get_all_shortcut_states" => {
            // TODO: Implement getting all shortcut states
            Ok(json!({}))
        }

        "toggle_menu" => {
            let tag = get_str_param!(params, "tag", "tag");
            if let Some(t) = tag {
                let vm_lock = state.version_manager.read().await;
                if let Some(ref vm) = *vm_lock {
                    let version_dir = vm.version_path(t);
                    drop(vm_lock);
                    let result = api.toggle_menu_shortcut(t, &version_dir).await?;
                    Ok(serde_json::to_value(result)?)
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
                let vm_lock = state.version_manager.read().await;
                if let Some(ref vm) = *vm_lock {
                    let version_dir = vm.version_path(t);
                    drop(vm_lock);
                    let result = api.toggle_desktop_shortcut(t, &version_dir).await?;
                    Ok(serde_json::to_value(result)?)
                } else {
                    Ok(json!(false))
                }
            } else {
                Ok(json!(false))
            }
        }

        // Legacy shortcut methods (deprecated but still supported)
        "menu_exists" => {
            // Check if any menu shortcut exists
            Ok(json!(false))
        }

        "desktop_exists" => {
            // Check if any desktop shortcut exists
            Ok(json!(false))
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
            let _app_id = get_app_id!(params);
            // Get the active version from version_manager and open its directory
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                if let Some(tag) = vm.get_active_version().await? {
                    let version_dir = vm.version_path(&tag);
                    drop(vm_lock);
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
                Ok(json!({"success": false, "error": "Version manager not initialized"}))
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

            let spec = pumas_core::model_library::ModelImportSpec {
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

            let request = pumas_core::DownloadRequest {
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

            let request = pumas_core::DownloadRequest {
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
                Some(progress) => Ok(serde_json::to_value(progress)?),
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
            let imports: Vec<pumas_core::model_library::ModelImportSpec> = params
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
            let sets = pumas_core::sharding::detect_sharded_sets(&paths);

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
            // TODO: Implement embedded metadata extraction
            Ok(json!({
                "success": false,
                "error": "Not yet implemented"
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
            // Get the models path from version_manager
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let version_path = vm.version_path(&version_tag);
                let models_path = version_path.join("models");
                drop(vm_lock);
                let response = api.preview_model_mapping(&version_tag, &models_path).await?;
                Ok(serde_json::to_value(response)?)
            } else {
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized"
                }))
            }
        }

        "apply_model_mapping" => {
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            // Get the models path from version_manager
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let version_path = vm.version_path(&version_tag);
                let models_path = version_path.join("models");
                drop(vm_lock);
                let response = api.apply_model_mapping(&version_tag, &models_path).await?;
                Ok(serde_json::to_value(response)?)
            } else {
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized"
                }))
            }
        }

        "sync_models_incremental" => {
            let version_tag = require_str_param!(params, "version_tag", "versionTag");
            // Get the models path from version_manager
            let vm_lock = state.version_manager.read().await;
            if let Some(ref vm) = *vm_lock {
                let version_path = vm.version_path(&version_tag);
                let models_path = version_path.join("models");
                drop(vm_lock);
                let response = api.sync_models_incremental(&version_tag, &models_path).await?;
                Ok(serde_json::to_value(response)?)
            } else {
                Ok(json!({
                    "success": false,
                    "error": "Version manager not initialized"
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
        // Not Yet Implemented / Unknown
        // ====================================================================
        _ => {
            warn!("Method not found: {}", method);
            Err(pumas_core::PumasError::Other(format!(
                "Method not found: {}",
                method
            )))
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

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
