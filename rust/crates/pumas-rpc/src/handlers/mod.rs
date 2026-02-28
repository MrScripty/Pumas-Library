//! JSON-RPC request handlers, split by domain.

mod conversion;
mod custom_nodes;
mod links;
mod models;
mod ollama;
mod plugins;
mod process;
mod shared;
mod shortcuts;
mod status;
mod torch;
mod versions;

use crate::server::AppState;
use crate::wrapper::wrap_response;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, warn};

pub(crate) use shared::{
    detect_sandbox_environment, extract_safetensors_header, get_bool_param, get_i64_param,
    get_str_param, require_str_param, sync_version_paths_to_process_manager,
};

// ============================================================================
// JSON-RPC types
// ============================================================================

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

// ============================================================================
// HTTP endpoints
// ============================================================================

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
            Json(JsonRpcResponse::success(
                id,
                json!({"status": "shutting_down"}),
            )),
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
// Method dispatcher
// ============================================================================

/// Dispatch a method call to the appropriate domain handler.
async fn dispatch_method(
    state: &AppState,
    method: &str,
    params: &Value,
) -> pumas_library::Result<Value> {
    match method {
        // Status & System
        "get_status" => status::get_status(state, params).await,
        "get_disk_space" => status::get_disk_space(state, params).await,
        "get_system_resources" => status::get_system_resources(state, params).await,
        "get_launcher_version" => status::get_launcher_version(state, params).await,
        "check_launcher_updates" => status::check_launcher_updates(state, params).await,
        "apply_launcher_update" => status::apply_launcher_update(state, params).await,
        "restart_launcher" => status::restart_launcher(state, params).await,
        "get_sandbox_info" => status::get_sandbox_info(state, params).await,
        "check_git" => status::check_git(state, params).await,
        "check_brave" => status::check_brave(state, params).await,
        "check_setproctitle" => status::check_setproctitle(state, params).await,
        "get_network_status" => status::get_network_status(state, params).await,
        "get_library_status" => status::get_library_status(state, params).await,
        "get_app_status" => status::get_app_status(state, params).await,

        // Version Management
        "get_available_versions" => versions::get_available_versions(state, params).await,
        "get_installed_versions" => versions::get_installed_versions(state, params).await,
        "get_active_version" => versions::get_active_version(state, params).await,
        "get_default_version" => versions::get_default_version(state, params).await,
        "set_default_version" => versions::set_default_version(state, params).await,
        "switch_version" => versions::switch_version(state, params).await,
        "install_version" => versions::install_version(state, params).await,
        "remove_version" => versions::remove_version(state, params).await,
        "cancel_installation" => versions::cancel_installation(state, params).await,
        "get_installation_progress" => versions::get_installation_progress(state, params).await,
        "validate_installations" => versions::validate_installations(state, params).await,
        "get_version_status" => versions::get_version_status(state, params).await,
        "get_version_info" => versions::get_version_info(state, params).await,
        "get_release_size_info" => versions::get_release_size_info(state, params).await,
        "get_release_size_breakdown" => versions::get_release_size_breakdown(state, params).await,
        "calculate_release_size" => versions::calculate_release_size(state, params).await,
        "calculate_all_release_sizes" => versions::calculate_all_release_sizes(state, params).await,
        "has_background_fetch_completed" => {
            versions::has_background_fetch_completed(state, params).await
        }
        "reset_background_fetch_flag" => versions::reset_background_fetch_flag(state, params).await,
        "get_github_cache_status" => versions::get_github_cache_status(state, params).await,
        "check_version_dependencies" => versions::check_version_dependencies(state, params).await,
        "install_version_dependencies" => {
            versions::install_version_dependencies(state, params).await
        }
        "get_release_dependencies" => versions::get_release_dependencies(state, params).await,
        "is_patched" => versions::is_patched(state, params).await,
        "toggle_patch" => versions::toggle_patch(state, params).await,

        // Model Library
        "get_models" => models::get_models(state, params).await,
        "refresh_model_index" => models::refresh_model_index(state, params).await,
        "refresh_model_mappings" => models::refresh_model_mappings(state, params).await,
        "import_model" => models::import_model(state, params).await,
        "download_model_from_hf" => models::download_model_from_hf(state, params).await,
        "start_model_download_from_hf" => models::start_model_download_from_hf(state, params).await,
        "get_model_download_status" => models::get_model_download_status(state, params).await,
        "cancel_model_download" => models::cancel_model_download(state, params).await,
        "pause_model_download" => models::pause_model_download(state, params).await,
        "resume_model_download" => models::resume_model_download(state, params).await,
        "list_model_downloads" => models::list_model_downloads(state, params).await,
        "list_interrupted_downloads" => models::list_interrupted_downloads(state, params).await,
        "recover_download" => models::recover_download(state, params).await,
        "search_hf_models" => models::search_hf_models(state, params).await,
        "get_related_models" => models::get_related_models(state, params).await,
        "search_models_fts" => models::search_models_fts(state, params).await,
        "import_batch" => models::import_batch(state, params).await,
        "lookup_hf_metadata_for_file" => models::lookup_hf_metadata_for_file(state, params).await,
        "detect_sharded_sets" => models::detect_sharded_sets(state, params).await,
        "validate_file_type" => models::validate_file_type(state, params).await,
        "get_embedded_metadata" => models::get_embedded_metadata(state, params).await,
        "get_library_model_metadata" => models::get_library_model_metadata(state, params).await,
        "refetch_model_metadata_from_hf" => {
            models::refetch_model_metadata_from_hf(state, params).await
        }
        "adopt_orphan_models" => models::adopt_orphan_models(state, params).await,
        "import_model_in_place" => models::import_model_in_place(state, params).await,
        "scan_shared_storage" => models::scan_shared_storage(state, params).await,

        // Inference Settings
        "get_inference_settings" => models::get_inference_settings(state, params).await,
        "update_inference_settings" => models::update_inference_settings(state, params).await,
        "resolve_model_dependency_requirements" => {
            models::resolve_model_dependency_requirements(state, params).await
        }
        "audit_dependency_pin_compliance" => {
            models::audit_dependency_pin_compliance(state, params).await
        }
        "list_models_needing_review" => models::list_models_needing_review(state, params).await,
        "submit_model_review" => models::submit_model_review(state, params).await,
        "reset_model_review" => models::reset_model_review(state, params).await,
        "generate_model_migration_dry_run_report" => {
            models::generate_model_migration_dry_run_report(state, params).await
        }
        "execute_model_migration" => models::execute_model_migration(state, params).await,
        "list_model_migration_reports" => models::list_model_migration_reports(state, params).await,
        "delete_model_migration_report" => {
            models::delete_model_migration_report(state, params).await
        }
        "prune_model_migration_reports" => {
            models::prune_model_migration_reports(state, params).await
        }

        // HuggingFace Authentication
        "set_hf_token" => models::set_hf_token(state, params).await,
        "clear_hf_token" => models::clear_hf_token(state, params).await,
        "get_hf_auth_status" => models::get_hf_auth_status(state, params).await,

        // Process Management
        "is_comfyui_running" => process::is_comfyui_running(state, params).await,
        "stop_comfyui" => process::stop_comfyui(state, params).await,
        "launch_comfyui" => process::launch_comfyui(state, params).await,
        "launch_ollama" => process::launch_ollama(state, params).await,
        "stop_ollama" => process::stop_ollama(state, params).await,
        "is_ollama_running" => process::is_ollama_running(state, params).await,
        "launch_torch" => process::launch_torch(state, params).await,
        "stop_torch" => process::stop_torch(state, params).await,
        "is_torch_running" => process::is_torch_running(state, params).await,
        "open_path" => process::open_path(state, params).await,
        "open_url" => process::open_url(state, params).await,
        "open_active_install" => process::open_active_install(state, params).await,

        // Ollama Model Management
        "ollama_list_models" => ollama::ollama_list_models(state, params).await,
        "ollama_create_model" => ollama::ollama_create_model(state, params).await,
        "ollama_delete_model" => ollama::ollama_delete_model(state, params).await,
        "ollama_load_model" => ollama::ollama_load_model(state, params).await,
        "ollama_unload_model" => ollama::ollama_unload_model(state, params).await,
        "ollama_list_running" => ollama::ollama_list_running(state, params).await,

        // Torch Inference Server
        "torch_list_slots" => torch::torch_list_slots(state, params).await,
        "torch_load_model" => torch::torch_load_model(state, params).await,
        "torch_unload_model" => torch::torch_unload_model(state, params).await,
        "torch_get_status" => torch::torch_get_status(state, params).await,
        "torch_list_devices" => torch::torch_list_devices(state, params).await,
        "torch_configure" => torch::torch_configure(state, params).await,

        // Link Management
        "get_link_health" => links::get_link_health(state, params).await,
        "clean_broken_links" => links::clean_broken_links(state, params).await,
        "remove_orphaned_links" => links::remove_orphaned_links(state, params).await,
        "get_links_for_model" => links::get_links_for_model(state, params).await,
        "delete_model_with_cascade" => links::delete_model_with_cascade(state, params).await,
        "preview_model_mapping" => links::preview_model_mapping(state, params).await,
        "apply_model_mapping" => links::apply_model_mapping(state, params).await,
        "sync_models_incremental" => links::sync_models_incremental(state, params).await,
        "sync_with_resolutions" => links::sync_with_resolutions(state, params).await,
        "get_cross_filesystem_warning" => links::get_cross_filesystem_warning(state, params).await,
        "get_file_link_count" => links::get_file_link_count(state, params).await,
        "check_files_writable" => links::check_files_writable(state, params).await,
        "set_model_link_exclusion" => links::set_model_link_exclusion(state, params).await,
        "get_link_exclusions" => links::get_link_exclusions(state, params).await,

        // Shortcuts
        "get_version_shortcuts" => shortcuts::get_version_shortcuts(state, params).await,
        "get_all_shortcut_states" => shortcuts::get_all_shortcut_states(state, params).await,
        "toggle_menu" => shortcuts::toggle_menu(state, params).await,
        "toggle_desktop" => shortcuts::toggle_desktop(state, params).await,
        "menu_exists" => shortcuts::menu_exists(state, params).await,
        "desktop_exists" => shortcuts::desktop_exists(state, params).await,
        "install_icon" => shortcuts::install_icon(state, params).await,
        "create_menu_shortcut" => shortcuts::create_menu_shortcut(state, params).await,
        "create_desktop_shortcut" => shortcuts::create_desktop_shortcut(state, params).await,
        "remove_menu_shortcut" => shortcuts::remove_menu_shortcut(state, params).await,
        "remove_desktop_shortcut" => shortcuts::remove_desktop_shortcut(state, params).await,

        // Conversion
        "start_model_conversion" => conversion::start_model_conversion(state, params).await,
        "get_conversion_progress" => conversion::get_conversion_progress(state, params).await,
        "cancel_model_conversion" => conversion::cancel_model_conversion(state, params).await,
        "list_model_conversions" => conversion::list_model_conversions(state, params).await,
        "check_conversion_environment" => {
            conversion::check_conversion_environment(state, params).await
        }
        "setup_conversion_environment" => {
            conversion::setup_conversion_environment(state, params).await
        }
        "get_supported_quant_types" => conversion::get_supported_quant_types(state, params).await,
        "get_backend_status" => conversion::get_backend_status(state, params).await,
        "setup_quantization_backend" => conversion::setup_quantization_backend(state, params).await,

        // Plugins
        "get_plugins" => plugins::get_plugins(state, params).await,
        "get_plugin" => plugins::get_plugin(state, params).await,
        "check_plugin_health" => plugins::check_plugin_health(state, params).await,

        // Custom Nodes
        "get_custom_nodes" => custom_nodes::get_custom_nodes(state, params).await,
        "install_custom_node" => custom_nodes::install_custom_node(state, params).await,
        "update_custom_node" => custom_nodes::update_custom_node(state, params).await,
        "remove_custom_node" => custom_nodes::remove_custom_node(state, params).await,

        // Unknown method
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
// Tests
// ============================================================================

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
        assert!(!is_sandboxed || ["flatpak", "snap", "docker", "appimage"].contains(&sandbox_type));
    }
}
