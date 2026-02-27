//! Version management handlers.

use super::{get_bool_param, get_i64_param, get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};
use tracing::warn;

pub async fn get_available_versions(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let force_refresh = get_bool_param(params, "force_refresh", "forceRefresh").unwrap_or(false);
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");

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
            Err(pumas_library::PumasError::RateLimited {
                service,
                retry_after_secs,
            }) => {
                warn!("Rate limited by {} when fetching versions", service);
                Ok(json!({
                    "success": false,
                    "error": format!("Rate limited by {}", service),
                    "rate_limited": true,
                    "retry_after_secs": retry_after_secs
                }))
            }
            Err(e) => Err(e),
        }
    } else {
        Ok(json!({
            "success": true,
            "versions": []
        }))
    }
}

pub async fn get_installed_versions(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let versions = vm.get_installed_versions().await?;
        // Return raw array - wrapper.rs will add {success, versions} wrapper
        Ok(serde_json::to_value(versions)?)
    } else {
        Ok(json!([]))
    }
}

pub async fn get_active_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let version = vm.get_active_version().await?;
        // Return raw value - wrapper.rs will add {success, version} wrapper
        Ok(serde_json::to_value(version)?)
    } else {
        Ok(Value::Null)
    }
}

pub async fn get_default_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let version = vm.get_default_version().await?;
        // Return raw value - wrapper.rs will add {success, version} wrapper
        Ok(serde_json::to_value(version)?)
    } else {
        Ok(Value::Null)
    }
}

pub async fn set_default_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn switch_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn install_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");

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

pub async fn remove_version(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn cancel_installation(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let result = vm.cancel_installation().await?;
        Ok(serde_json::to_value(result)?)
    } else {
        Ok(serde_json::to_value(false)?)
    }
}

pub async fn get_installation_progress(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let managers = state.version_managers.read().await;
    if let Some(vm) = managers.get(app_id_str) {
        let progress = vm.get_installation_progress().await;
        Ok(serde_json::to_value(progress)?)
    } else {
        Ok(serde_json::to_value::<
            Option<pumas_library::models::InstallationProgress>,
        >(None)?)
    }
}

pub async fn validate_installations(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn get_version_status(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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
            versions_map.insert(
                tag.clone(),
                json!({
                    "isActive": is_active,
                    "dependencies": {
                        "installed": deps.as_ref().map(|d| &d.installed).unwrap_or(&vec![]),
                        "missing": deps.as_ref().map(|d| &d.missing).unwrap_or(&vec![])
                    }
                }),
            );
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

pub async fn get_version_info(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn get_release_size_info(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let archive_size = get_i64_param(params, "archive_size", "archiveSize").unwrap_or(0) as u64;

    // Calculate release size using size_calculator from state
    let mut calc = state.size_calculator.write().await;
    let result = calc
        .calculate_release_size(&tag, archive_size, None)
        .await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn get_release_size_breakdown(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;

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

pub async fn calculate_release_size(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let archive_size = get_i64_param(params, "archive_size", "archiveSize").unwrap_or(0) as u64;

    // Parse optional requirements array
    let requirements: Option<Vec<String>> = params
        .get("requirements")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let mut calc = state.size_calculator.write().await;
    let result = calc
        .calculate_release_size(&tag, archive_size, requirements.as_deref())
        .await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn calculate_all_release_sizes(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    // Get all available versions and calculate sizes
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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
        if let Ok(size_info) = calc
            .calculate_release_size(&version.tag_name, archive_size, None)
            .await
        {
            if let Ok(value) = serde_json::to_value(&size_info) {
                results.insert(version.tag_name.clone(), value);
            }
        }
    }

    Ok(json!(results))
}

pub async fn has_background_fetch_completed(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let completed = state.api.has_background_fetch_completed().await;
    Ok(serde_json::to_value(completed)?)
}

pub async fn reset_background_fetch_flag(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    state.api.reset_background_fetch_flag().await;
    Ok(json!(true))
}

pub async fn get_github_cache_status(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    // Return cache status in format expected by frontend
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

pub async fn check_version_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn install_version_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn get_release_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
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

pub async fn is_patched(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    let is_patched = state.api.is_patched(tag);
    Ok(json!(is_patched))
}

pub async fn toggle_patch(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    match state.api.toggle_patch(tag) {
        Ok(is_now_patched) => Ok(json!(is_now_patched)),
        Err(e) => Ok(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}
