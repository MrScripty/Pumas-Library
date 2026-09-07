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
    pub async fn is_conversion_environment_ready(&self) -> Result<bool> {
        self.primary()
            .conversion_manager
            .is_environment_ready_async()
            .await
    }

    /// Ensure the Python conversion environment is set up.
    pub async fn ensure_conversion_environment(&self) -> Result<()> {
        self.primary().conversion_manager.ensure_environment().await
    }

    /// Start base Python setup, or inspect the latest retained operation.
    /// `None` never retries a retained operation. Only a matching terminal ID
    /// admits a successor; stale IDs return the current snapshot. IDs must be
    /// canonical lower-case hyphenated UUIDs, and a retry without an owner-local
    /// record is invalid. Records are not persisted across owner/process restart.
    pub async fn start_conversion_setup(
        &self,
        expected_previous_operation_id: Option<&str>,
    ) -> Result<conversion::ConversionSetupSnapshot> {
        self.primary()
            .conversion_manager
            .start_conversion_setup(expected_previous_operation_id)
            .await
    }

    /// Read the latest owner-local setup snapshot without starting work or disk I/O.
    pub fn get_conversion_setup(&self) -> Option<conversion::ConversionSetupSnapshot> {
        self.primary().conversion_manager.get_conversion_setup()
    }

    /// Close base Python setup admission and await owned process cleanup.
    /// Invoke before stopping the hosting runtime. Successful cancellation and
    /// cleanup return success; repeated calls preserve actual setup failures.
    pub async fn shutdown_conversion_setup(&self) -> Result<()> {
        self.primary().conversion_manager.shutdown_setup().await
    }

    /// Get the list of supported quantization types for conversion.
    pub async fn supported_quant_types(&self) -> Result<Vec<conversion::QuantOption>> {
        self.primary()
            .conversion_manager
            .supported_quant_types_async()
            .await
    }

    /// Get the readiness status of all quantization backends.
    pub async fn backend_status(&self) -> Result<Vec<conversion::BackendStatus>> {
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
        self.primary()
            .conversion_manager
            .ensure_backend_environment(backend)
            .await
    }
}
