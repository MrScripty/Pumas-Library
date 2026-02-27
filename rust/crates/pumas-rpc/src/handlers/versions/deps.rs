//! Version dependency handlers.

use crate::handlers::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::Value;

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
