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
    let result = dispatch_method(&state.api, method, &params).await;

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

/// Dispatch a method call to the appropriate API handler.
async fn dispatch_method(
    api: &pumas_core::PumasApi,
    method: &str,
    params: &Value,
) -> pumas_core::Result<Value> {
    match method {
        // ========================================
        // Status & System
        // ========================================
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

        // ========================================
        // Version Management
        // ========================================
        "get_available_versions" => {
            let force_refresh = params
                .get("force_refresh")
                .or_else(|| params.get("forceRefresh"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let app_id = params
                .get("app_id")
                .or_else(|| params.get("appId"))
                .and_then(|v| v.as_str())
                .and_then(pumas_core::AppId::from_str);

            let versions = api.get_available_versions(force_refresh, app_id).await?;
            Ok(serde_json::to_value(versions)?)
        }

        "get_installed_versions" => {
            let app_id = params
                .get("app_id")
                .or_else(|| params.get("appId"))
                .and_then(|v| v.as_str())
                .and_then(pumas_core::AppId::from_str);

            let versions = api.get_installed_versions(app_id).await?;
            Ok(serde_json::to_value(versions)?)
        }

        "get_active_version" => {
            let app_id = params
                .get("app_id")
                .or_else(|| params.get("appId"))
                .and_then(|v| v.as_str())
                .and_then(pumas_core::AppId::from_str);

            let version = api.get_active_version(app_id).await?;
            Ok(serde_json::to_value(version)?)
        }

        // ========================================
        // Background Fetch
        // ========================================
        "has_background_fetch_completed" => {
            let completed = api.has_background_fetch_completed().await;
            Ok(serde_json::to_value(completed)?)
        }

        "reset_background_fetch_flag" => {
            api.reset_background_fetch_flag().await;
            Ok(json!(true))
        }

        // ========================================
        // Not Yet Implemented
        // ========================================
        _ => {
            warn!("Method not found: {}", method);
            Err(pumas_core::PumasError::Other(format!(
                "Method not found: {}",
                method
            )))
        }
    }
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
}
