//! Shortcut management handlers.

use super::{get_str_param, require_str_param};
use crate::server::AppState;
use serde_json::{json, Value};
use std::collections::HashMap;

pub async fn get_version_shortcuts(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let tag = require_str_param(params, "tag", "tag")?;
    let shortcut_manager = state.shortcut_manager.read().await.clone();
    if let Some(sm) = shortcut_manager {
        let shortcut_state = sm.get_version_shortcut_state_async(&tag).await?;
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

pub async fn get_all_shortcut_states(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let shortcut_manager = state.shortcut_manager.read().await.clone();
    if let Some(sm) = shortcut_manager {
        let states = sm.get_all_shortcut_states_async().await?;
        let result: HashMap<String, serde_json::Value> = states
            .into_iter()
            .map(|(tag, state)| {
                (
                    tag,
                    json!({
                        "tag": state.tag,
                        "menu": state.menu,
                        "desktop": state.desktop
                    }),
                )
            })
            .collect();
        Ok(json!(result))
    } else {
        Ok(json!({}))
    }
}

pub async fn toggle_menu(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    if let Some(t) = tag {
        let managers = state.version_managers.read().await;
        if let Some(vm) = managers.get("comfyui") {
            let version_dir = vm.version_path(t);
            drop(managers);
            let shortcut_manager = state.shortcut_manager.read().await.clone();
            if let Some(sm) = shortcut_manager {
                let tag = t.to_string();
                let result = tokio::task::spawn_blocking(move || {
                    sm.toggle_menu_shortcut(&tag, &version_dir)
                })
                .await
                .map_err(|e| pumas_library::PumasError::Config {
                    message: format!("Shortcut toggle task failed: {}", e),
                })?;

                match result {
                    Ok(result) => Ok(json!(result.success)),
                    Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
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

pub async fn toggle_desktop(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let tag = get_str_param(params, "tag", "tag");
    if let Some(t) = tag {
        let managers = state.version_managers.read().await;
        if let Some(vm) = managers.get("comfyui") {
            let version_dir = vm.version_path(t);
            drop(managers);
            let shortcut_manager = state.shortcut_manager.read().await.clone();
            if let Some(sm) = shortcut_manager {
                let tag = t.to_string();
                let result = tokio::task::spawn_blocking(move || {
                    sm.toggle_desktop_shortcut(&tag, &version_dir)
                })
                .await
                .map_err(|e| pumas_library::PumasError::Config {
                    message: format!("Shortcut toggle task failed: {}", e),
                })?;

                match result {
                    Ok(result) => Ok(json!(result.success)),
                    Err(e) => Ok(json!({"success": false, "error": e.to_string()})),
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

pub async fn menu_exists(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let shortcut_manager = state.shortcut_manager.read().await.clone();
    if let Some(sm) = shortcut_manager {
        Ok(json!(sm.menu_exists_async().await?))
    } else {
        Ok(json!(false))
    }
}

pub async fn desktop_exists(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let shortcut_manager = state.shortcut_manager.read().await.clone();
    if let Some(sm) = shortcut_manager {
        Ok(json!(sm.desktop_exists_async().await?))
    } else {
        Ok(json!(false))
    }
}

pub async fn install_icon(_state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    // Legacy method - icons are installed with shortcuts now
    Ok(json!(true))
}

pub async fn create_menu_shortcut(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // Legacy method - use toggle_menu instead
    Ok(json!(false))
}

pub async fn create_desktop_shortcut(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // Legacy method - use toggle_desktop instead
    Ok(json!(false))
}

pub async fn remove_menu_shortcut(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // Legacy method - use toggle_menu instead
    Ok(json!(false))
}

pub async fn remove_desktop_shortcut(
    _state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    // Legacy method - use toggle_desktop instead
    Ok(json!(false))
}
