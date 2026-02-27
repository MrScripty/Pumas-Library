//! Primary instance state and IPC dispatch.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::conversion;
use crate::error::PumasError;
use crate::ipc;
use crate::model_library;
use crate::network;
use crate::process;
use crate::registry;
use crate::system;

/// All state owned by a primary instance.
///
/// This is the full set of subsystems that were previously fields on `PumasApi`.
/// Wrapped in `Arc` so it can be shared with the IPC server dispatch.
pub(crate) struct PrimaryState {
    pub(crate) _state: Arc<RwLock<ApiState>>,
    pub(crate) network_manager: Arc<network::NetworkManager>,
    pub(crate) process_manager: Arc<RwLock<Option<process::ProcessManager>>>,
    pub(crate) system_utils: Arc<system::SystemUtils>,
    pub(crate) model_library: Arc<model_library::ModelLibrary>,
    pub(crate) model_mapper: model_library::ModelMapper,
    pub(crate) hf_client: Option<model_library::HuggingFaceClient>,
    pub(crate) model_importer: model_library::ModelImporter,
    pub(crate) conversion_manager: Arc<conversion::ConversionManager>,
    /// IPC server handle (Primary only). Protected by async Mutex for shutdown.
    pub(crate) server_handle: tokio::sync::Mutex<Option<ipc::IpcServerHandle>>,
    /// Global registry connection (best-effort, None if unavailable).
    pub(crate) registry: Option<registry::LibraryRegistry>,
}

/// IPC dispatch implementation for the primary state.
///
/// Routes incoming JSON-RPC method calls to the appropriate PrimaryState methods.
/// Each method deserializes params, calls the real implementation, and serializes the result.
#[async_trait::async_trait]
impl ipc::server::IpcDispatch for PrimaryState {
    async fn dispatch(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, PumasError> {
        match method {
            "list_models" => {
                let models = self.model_library.list_models().await?;
                Ok(serde_json::to_value(models)?)
            }
            "search_models" => {
                let query = params["query"].as_str().unwrap_or("");
                let limit = params["limit"].as_u64().unwrap_or(50) as usize;
                let offset = params["offset"].as_u64().unwrap_or(0) as usize;
                let result = self
                    .model_library
                    .search_models(query, limit, offset)
                    .await?;
                Ok(serde_json::to_value(result)?)
            }
            "get_model" => {
                let model_id =
                    params["model_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "model_id is required".to_string(),
                        })?;
                let model = self.model_library.get_model(model_id).await?;
                Ok(serde_json::to_value(model)?)
            }
            "get_status" => Ok(serde_json::json!({
                "success": true,
                "version": env!("CARGO_PKG_VERSION"),
                "is_primary": true,
            })),
            "ping" => Ok(serde_json::json!("pong")),
            // Conversion methods
            "start_conversion" => {
                let request: conversion::ConversionRequest = serde_json::from_value(params)
                    .map_err(|e| PumasError::InvalidParams {
                        message: format!("Invalid conversion request: {e}"),
                    })?;
                let id = self.conversion_manager.start_conversion(request).await?;
                Ok(serde_json::json!({ "conversion_id": id }))
            }
            "get_conversion_progress" => {
                let id =
                    params["conversion_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "conversion_id is required".to_string(),
                        })?;
                let progress = self.conversion_manager.get_progress(id);
                Ok(serde_json::to_value(progress)?)
            }
            "cancel_conversion" => {
                let id =
                    params["conversion_id"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "conversion_id is required".to_string(),
                        })?;
                let cancelled = self.conversion_manager.cancel_conversion(id).await?;
                Ok(serde_json::json!({ "cancelled": cancelled }))
            }
            "list_conversions" => {
                let conversions = self.conversion_manager.list_conversions();
                Ok(serde_json::to_value(conversions)?)
            }
            "is_conversion_environment_ready" => {
                let ready = self.conversion_manager.is_environment_ready();
                Ok(serde_json::json!({ "ready": ready }))
            }
            "ensure_conversion_environment" => {
                self.conversion_manager.ensure_environment().await?;
                Ok(serde_json::json!({ "success": true }))
            }
            "supported_quant_types" => {
                let types = self.conversion_manager.supported_quant_types();
                Ok(serde_json::to_value(types)?)
            }
            // Quantization backend methods
            "backend_status" => {
                let status = self.conversion_manager.backend_status();
                Ok(serde_json::to_value(status)?)
            }
            "ensure_backend_environment" => {
                let backend_str =
                    params["backend"]
                        .as_str()
                        .ok_or_else(|| PumasError::InvalidParams {
                            message: "backend is required".to_string(),
                        })?;
                let backend: conversion::QuantBackend =
                    serde_json::from_value(serde_json::json!(backend_str)).map_err(|e| {
                        PumasError::InvalidParams {
                            message: format!("Invalid backend: {e}"),
                        }
                    })?;
                self.conversion_manager
                    .ensure_backend_environment(backend)
                    .await?;
                Ok(serde_json::json!({ "success": true }))
            }
            _ => Err(PumasError::InvalidParams {
                message: format!("Unknown IPC method: {}", method),
            }),
        }
    }
}

/// Internal state for the API.
pub(crate) struct ApiState {
    /// Whether background fetch has completed
    pub(crate) background_fetch_completed: bool,
}
