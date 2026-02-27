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
        self.primary()
            .conversion_manager
            .get_progress(conversion_id)
    }

    /// Cancel a running conversion.
    pub async fn cancel_conversion(&self, conversion_id: &str) -> Result<bool> {
        self.primary()
            .conversion_manager
            .cancel_conversion(conversion_id)
            .await
    }

    /// List all tracked conversions (active and recently completed).
    pub fn list_conversions(&self) -> Vec<conversion::ConversionProgress> {
        self.primary().conversion_manager.list_conversions()
    }

    /// Check if the Python conversion environment is ready.
    pub fn is_conversion_environment_ready(&self) -> bool {
        self.primary().conversion_manager.is_environment_ready()
    }

    /// Ensure the Python conversion environment is set up.
    pub async fn ensure_conversion_environment(&self) -> Result<()> {
        self.primary().conversion_manager.ensure_environment().await
    }

    /// Get the list of supported quantization types for conversion.
    pub fn supported_quant_types(&self) -> Vec<conversion::QuantOption> {
        self.primary().conversion_manager.supported_quant_types()
    }

    /// Get the readiness status of all quantization backends.
    pub fn backend_status(&self) -> Vec<conversion::BackendStatus> {
        self.primary().conversion_manager.backend_status()
    }

    /// Ensure a specific quantization backend's environment is set up.
    pub async fn ensure_backend_environment(
        &self,
        backend: conversion::QuantBackend,
    ) -> Result<()> {
        self.primary()
            .conversion_manager
            .ensure_backend_environment(backend)
            .await
    }
}
