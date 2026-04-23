//! Version dependency handlers.

use crate::handlers::{
    get_str_param, get_version_manager, path_exists, read_utf8_file, require_str_param,
    require_version_manager,
};
use crate::server::AppState;
use serde_json::Value;

pub async fn check_version_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let vm = require_version_manager(state, app_id_str).await?;
    let status = vm.check_dependencies(&tag).await?;
    Ok(serde_json::to_value(status)?)
}

pub async fn install_version_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    let vm = require_version_manager(state, app_id_str).await?;
    let result = vm.install_dependencies(&tag, None).await?;
    Ok(serde_json::to_value(result)?)
}

pub async fn get_release_dependencies(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let app_id_str = get_str_param(params, "app_id", "appId").unwrap_or("comfyui");
    if let Some(vm) = get_version_manager(state, app_id_str).await {
        let version_path = vm.version_path(&tag);
        let requirements_path = version_path.join("requirements.txt");

        if !path_exists(&requirements_path).await? {
            return Ok(serde_json::to_value::<Vec<String>>(vec![])?);
        }

        let content = read_utf8_file(&requirements_path).await?;

        // Parse requirements (simple extraction of package names)
        let packages: Vec<String> = content
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('#') && !line.starts_with('-')
            })
            .filter_map(|line| {
                let name = line.split(['=', '>', '<', '[', ';']).next()?.trim();
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
