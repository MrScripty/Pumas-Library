//! Plugin system handlers.

use super::require_str_param;
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn get_plugins(state: &AppState, _params: &Value) -> pumas_library::Result<Value> {
    let plugins = state.plugin_loader.get_enabled();
    Ok(json!({
        "success": true,
        "plugins": serde_json::to_value(plugins)?
    }))
}

pub async fn get_plugin(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id = require_str_param(params, "app_id", "appId")?;
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

pub async fn check_plugin_health(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let app_id = require_str_param(params, "app_id", "appId")?;
    let plugin = state.plugin_loader.get(&app_id);
    match plugin {
        Some(config) => {
            if let Some(conn) = &config.connection {
                let health_endpoint = conn.health_endpoint.as_deref().unwrap_or("/health");
                let url = format!(
                    "{}://localhost:{}{}",
                    conn.protocol, conn.default_port, health_endpoint
                );
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(3))
                    .build()
                    .unwrap_or_default();
                let healthy = client
                    .get(&url)
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);
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
