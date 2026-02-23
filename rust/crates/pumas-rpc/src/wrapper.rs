//! Response wrapping for frontend compatibility.
//!
//! The frontend expects responses in specific formats. This module transforms
//! raw API responses to match the expected format, mirroring the Python
//! `wrap_response()` function in `rpc_server.py`.

use serde_json::{json, Value};

/// Wrap API responses to match the frontend's expected format.
///
/// The frontend expects responses in the format: `{success: bool, ...data, error?: string}`
/// but the core API returns raw data. This function wraps responses appropriately.
pub fn wrap_response(method: &str, result: Value) -> Value {
    // Methods that return lists and need {success, versions/nodes/etc} wrapping
    match method {
        // List wrappers
        "get_installed_versions" => {
            json!({
                "success": true,
                "versions": if result.is_null() { json!([]) } else { result }
            })
        }

        // get_available_versions is handled entirely by handler due to rate limit complexity
        "get_available_versions" => result,

        "get_custom_nodes" => {
            json!({
                "success": true,
                "nodes": if result.is_null() { json!([]) } else { result }
            })
        }

        "get_release_dependencies" => {
            json!({
                "success": true,
                "dependencies": if result.is_null() { json!([]) } else { result }
            })
        }

        // Dict wrappers
        "get_version_status" => {
            json!({
                "success": true,
                "status": if result.is_null() { json!({}) } else { result }
            })
        }

        "get_version_info" => {
            json!({
                "success": true,
                "info": if result.is_null() { json!({}) } else { result }
            })
        }

        "get_release_size_info" => {
            json!({
                "success": true,
                "info": if result.is_null() { json!({}) } else { result }
            })
        }

        "get_release_size_breakdown" => {
            json!({
                "success": true,
                "breakdown": if result.is_null() { json!({}) } else { result }
            })
        }

        // get_github_cache_status returns CacheStatusResponse which doesn't need wrapping
        "get_github_cache_status" => result,

        "get_version_shortcuts" => {
            json!({
                "success": true,
                "state": if result.is_null() { json!({}) } else { result }
            })
        }

        "get_all_shortcut_states" => {
            json!({
                "success": true,
                "states": if result.is_null() { json!({}) } else { result }
            })
        }

        // Passthrough methods (already in correct format)
        "get_status"
        | "get_disk_space"
        | "get_system_resources"
        | "get_launcher_version"
        | "check_launcher_updates"
        | "apply_launcher_update"
        | "restart_launcher"
        | "get_network_status"
        | "get_library_status"
        | "get_link_health"
        | "import_model"
        | "download_model_from_hf"
        | "start_model_download_from_hf"
        | "get_model_download_status"
        | "cancel_model_download"
        | "pause_model_download"
        | "resume_model_download"
        | "list_model_downloads"
        | "search_hf_models"
        | "get_related_models"
        | "search_models_fts"
        | "import_batch"
        | "lookup_hf_metadata_for_file"
        | "detect_sharded_sets"
        | "validate_file_type"
        | "mark_metadata_as_manual"
        | "get_file_link_count"
        | "check_files_writable"
        | "open_path"
        | "open_url"
        | "open_active_install"
        | "preview_model_mapping"
        | "apply_model_mapping"
        | "sync_models_incremental"
        | "sync_with_resolutions"
        | "get_cross_filesystem_warning"
        | "clean_broken_links"
        | "remove_orphaned_links"
        | "get_links_for_model"
        | "delete_model_with_cascade"
        | "get_sandbox_info"
        | "validate_installations"
        | "ollama_list_models"
        | "ollama_create_model"
        | "ollama_delete_model"
        | "ollama_load_model"
        | "ollama_unload_model"
        | "ollama_list_running"
        | "start_model_conversion"
        | "get_conversion_progress"
        | "cancel_model_conversion"
        | "list_model_conversions"
        | "check_conversion_environment"
        | "setup_conversion_environment"
        | "get_supported_quant_types" => result,

        // Structured response methods (handler returns {success, ...} directly)
        "install_version" => result,

        // Bool methods
        "remove_version"
        | "switch_version"
        | "cancel_installation"
        | "install_version_dependencies"
        | "install_custom_node"
        | "update_custom_node"
        | "remove_custom_node"
        | "toggle_patch"
        | "toggle_menu"
        | "toggle_desktop"
        | "refresh_model_index"
        | "set_default_version" => {
            json!({
                "success": result.as_bool().unwrap_or(false)
            })
        }

        // Optional dict methods (can return null)
        "get_installation_progress" | "calculate_release_size" => result,

        // Special cases
        "get_active_version" => {
            json!({
                "success": true,
                "version": if result.is_null() { json!("") } else { result }
            })
        }

        "get_default_version" => {
            json!({
                "success": true,
                "version": if result.is_null() { json!("") } else { result }
            })
        }

        "get_models" => {
            json!({
                "success": true,
                "models": if result.is_null() { json!({}) } else { result }
            })
        }

        "refresh_model_mappings" => {
            json!({
                "success": true,
                "results": if result.is_null() { json!({}) } else { result }
            })
        }

        "get_model_overrides" => {
            json!({
                "success": true,
                "overrides": if result.is_null() { json!({}) } else { result }
            })
        }

        "update_model_overrides" => {
            json!({
                "success": result.as_bool().unwrap_or(false)
            })
        }

        "scan_shared_storage" => {
            json!({
                "success": true,
                "result": if result.is_null() { json!({}) } else { result }
            })
        }

        "check_version_dependencies" => {
            json!({
                "success": true,
                "dependencies": if result.is_null() {
                    json!({"installed": [], "missing": []})
                } else {
                    result
                }
            })
        }

        "calculate_all_release_sizes" => {
            if result.is_null() {
                json!({})
            } else {
                result
            }
        }

        "has_background_fetch_completed" => {
            json!({
                "success": true,
                "completed": result.as_bool().unwrap_or(false)
            })
        }

        "reset_background_fetch_flag" => {
            json!({
                "success": true
            })
        }

        // Default: return as-is (for methods not explicitly handled)
        _ => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_versions() {
        // get_installed_versions gets wrapped
        let versions = json!(["v1.0.0", "v1.1.0"]);
        let wrapped = wrap_response("get_installed_versions", versions);

        assert!(wrapped.get("success").unwrap().as_bool().unwrap());
        assert_eq!(
            wrapped.get("versions").unwrap(),
            &json!(["v1.0.0", "v1.1.0"])
        );
    }

    #[test]
    fn test_available_versions_passthrough() {
        // get_available_versions is passed through (handler handles wrapping)
        let data = json!({"success": true, "versions": ["v1.0.0"]});
        let wrapped = wrap_response("get_available_versions", data.clone());
        assert_eq!(wrapped, data);
    }

    #[test]
    fn test_wrap_null_versions() {
        let wrapped = wrap_response("get_installed_versions", Value::Null);

        assert!(wrapped.get("success").unwrap().as_bool().unwrap());
        assert_eq!(wrapped.get("versions").unwrap(), &json!([]));
    }

    #[test]
    fn test_wrap_bool_method() {
        let wrapped = wrap_response("install_version", json!(true));
        assert!(wrapped.get("success").unwrap().as_bool().unwrap());

        let wrapped = wrap_response("install_version", json!(false));
        assert!(!wrapped.get("success").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_passthrough_method() {
        let data = json!({"success": true, "version": "1.0.0"});
        let wrapped = wrap_response("get_status", data.clone());
        assert_eq!(wrapped, data);
    }

    #[test]
    fn test_wrap_active_version() {
        let wrapped = wrap_response("get_active_version", json!("v1.0.0"));
        assert!(wrapped.get("success").unwrap().as_bool().unwrap());
        assert_eq!(wrapped.get("version").unwrap(), &json!("v1.0.0"));

        let wrapped_null = wrap_response("get_active_version", Value::Null);
        assert_eq!(wrapped_null.get("version").unwrap(), &json!(""));
    }
}
