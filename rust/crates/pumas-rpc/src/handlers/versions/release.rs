//! Release metadata, sizing, and cache handlers.

use crate::handlers::{get_bool_param, get_i64_param, get_version_manager, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};
use tracing::warn;

pub async fn get_available_versions(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<crate::contract::AvailableVersionsOutcome> {
    let force_refresh = get_bool_param(params, "force_refresh", "forceRefresh").unwrap_or(false);
    let app_id_str = require_str_param(params, "app_id", "appId")?;

    if let Some(vm) = get_version_manager(state, app_id_str).await {
        // Handle rate limit errors specially to return structured response
        match vm.get_available_releases(force_refresh).await {
            Ok(releases) => {
                let versions: Vec<pumas_library::models::VersionReleaseInfo> = releases
                    .into_iter()
                    .map(pumas_library::models::VersionReleaseInfo::from)
                    .collect();
                crate::contract::AvailableVersionsOutcome::available(versions)
            }
            Err(pumas_library::PumasError::RateLimited {
                service,
                retry_after_secs,
            }) => {
                warn!("Rate limited by {} when fetching versions", service);
                crate::contract::AvailableVersionsOutcome::rate_limited(retry_after_secs)
            }
            Err(e) => Err(e),
        }
    } else {
        crate::contract::AvailableVersionsOutcome::available(vec![])
    }
}

pub async fn get_version_status(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<crate::contract::VersionStatusOutcome> {
    use crate::contract::{
        RuntimeVersionDependencies, RuntimeVersionEntry, RuntimeVersionStatus, VersionStatusOutcome,
    };

    let app_id_str = require_str_param(params, "app_id", "appId")?;
    let Some(vm) = get_version_manager(state, app_id_str).await else {
        return VersionStatusOutcome::new(RuntimeVersionStatus::default());
    };
    let active = vm.get_active_version().await?;
    let default = vm.get_default_version().await?;
    let installed = vm.get_installed_versions().await?;
    let mut versions = std::collections::BTreeMap::new();
    for tag in &installed {
        // Preserve the existing empty dependency lists when a check fails.
        let deps = vm.check_dependencies(tag).await.ok().unwrap_or_default();
        versions.insert(
            tag.clone(),
            RuntimeVersionEntry {
                is_active: active.as_ref() == Some(tag),
                dependencies: RuntimeVersionDependencies {
                    installed: deps.installed,
                    missing: deps.missing,
                },
            },
        );
    }
    VersionStatusOutcome::new(RuntimeVersionStatus {
        installed_count: installed.len(),
        active_version: active,
        default_version: default,
        versions,
    })
}

pub async fn get_version_info(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<crate::contract::VersionInfoOutcome> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    if let Some(vm) = get_version_manager(state, app_id_str).await {
        let installed = vm.get_installed_versions().await?;
        let is_installed = installed.contains(&tag);
        Ok(crate::contract::VersionInfoOutcome::new(tag, is_installed))
    } else {
        Ok(crate::contract::VersionInfoOutcome::new(tag, false))
    }
}

pub async fn get_release_size_info(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let archive_size = get_i64_param(params, "archive_size", "archiveSize").unwrap_or(0) as u64;

    // Calculate release size using size_calculator from state
    let mut calc = state.size_calculator.lock().await;
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
    let calc = state.size_calculator.lock().await;
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

    let mut calc = state.size_calculator.lock().await;
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
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    let versions = if let Some(vm) = get_version_manager(state, app_id_str).await {
        let releases = vm.get_available_releases(false).await?;
        releases
            .into_iter()
            .map(pumas_library::models::VersionReleaseInfo::from)
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let mut results = serde_json::Map::new();
    let mut calc = state.size_calculator.lock().await;

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
) -> pumas_library::Result<crate::contract::GithubCacheStatusOutcome> {
    let app_id_str = require_str_param(params, "app_id", "appId")?;
    // Return cache status in format expected by frontend
    if let Some(vm) = get_version_manager(state, app_id_str).await {
        let cache_status = vm.get_github_cache_status().await;
        crate::contract::GithubCacheStatusOutcome::snapshot(cache_status)
    } else {
        Ok(crate::contract::GithubCacheStatusOutcome::no_manager())
    }
}
