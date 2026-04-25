//! Model format conversion methods on PumasApi.

use crate::conversion;
use crate::error::Result;
use crate::PumasApi;

impl PumasApi {
    // ========================================
    // Model Format Conversion Methods
    // ========================================

    /// Start a model format conversion (GGUF <-> Safetensors).
    ///
    /// Returns a conversion ID for tracking progress.
    pub async fn start_conversion(&self, request: conversion::ConversionRequest) -> Result<String> {
        if self.try_client().is_some() {
            let response: serde_json::Value = self
                .call_client_method("start_conversion", serde_json::to_value(request)?)
                .await?;
            return response["conversion_id"]
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    crate::error::PumasError::Other("Missing conversion_id".to_string())
                });
        }

        self.primary()
            .conversion_manager
            .start_conversion(request)
            .await
    }

    /// Get progress for a specific conversion.
    pub fn get_conversion_progress(
        &self,
        conversion_id: &str,
    ) -> Option<conversion::ConversionProgress> {
        if self.try_client().is_some() {
            return self.call_client_method_blocking_or_default(
                "get_conversion_progress",
                serde_json::json!({ "conversion_id": conversion_id }),
            );
        }

        self.primary()
            .conversion_manager
            .get_progress(conversion_id)
    }

    /// Cancel a running conversion.
    pub async fn cancel_conversion(&self, conversion_id: &str) -> Result<bool> {
        if self.try_client().is_some() {
            let response: serde_json::Value = self
                .call_client_method(
                    "cancel_conversion",
                    serde_json::json!({ "conversion_id": conversion_id }),
                )
                .await?;
            return response["cancelled"].as_bool().ok_or_else(|| {
                crate::error::PumasError::Other("Missing cancelled flag".to_string())
            });
        }

        self.primary()
            .conversion_manager
            .cancel_conversion(conversion_id)
            .await
    }

    /// List all tracked conversions (active and recently completed).
    pub fn list_conversions(&self) -> Vec<conversion::ConversionProgress> {
        if self.try_client().is_some() {
            return self
                .call_client_method_blocking_or_default("list_conversions", serde_json::json!({}));
        }

        self.primary().conversion_manager.list_conversions()
    }

    /// Check if the Python conversion environment is ready.
    pub async fn is_conversion_environment_ready(&self) -> Result<bool> {
        if self.try_client().is_some() {
            let response: serde_json::Value = self.call_client_method_blocking_or_default(
                "is_conversion_environment_ready",
                serde_json::json!({}),
            );
            return Ok(response["ready"].as_bool().unwrap_or(false));
        }

        self.primary()
            .conversion_manager
            .is_environment_ready_async()
            .await
    }

    /// Ensure the Python conversion environment is set up.
    pub async fn ensure_conversion_environment(&self) -> Result<()> {
        if self.try_client().is_some() {
            let _: serde_json::Value = self
                .call_client_method("ensure_conversion_environment", serde_json::json!({}))
                .await?;
            return Ok(());
        }

        self.primary().conversion_manager.ensure_environment().await
    }

    /// Get the list of supported quantization types for conversion.
    pub async fn supported_quant_types(&self) -> Result<Vec<conversion::QuantOption>> {
        if self.try_client().is_some() {
            return Ok(self.call_client_method_blocking_or_default(
                "supported_quant_types",
                serde_json::json!({}),
            ));
        }

        self.primary()
            .conversion_manager
            .supported_quant_types_async()
            .await
    }

    /// Get the readiness status of all quantization backends.
    pub async fn backend_status(&self) -> Result<Vec<conversion::BackendStatus>> {
        if self.try_client().is_some() {
            return Ok(self
                .call_client_method_blocking_or_default("backend_status", serde_json::json!({})));
        }

        self.primary()
            .conversion_manager
            .backend_status_async()
            .await
    }

    /// Ensure a specific quantization backend's environment is set up.
    pub async fn ensure_backend_environment(
        &self,
        backend: conversion::QuantBackend,
    ) -> Result<()> {
        if self.try_client().is_some() {
            let _: serde_json::Value = self
                .call_client_method(
                    "ensure_backend_environment",
                    serde_json::json!({ "backend": backend }),
                )
                .await?;
            return Ok(());
        }

        self.primary()
            .conversion_manager
            .ensure_backend_environment(backend)
            .await
    }
}
