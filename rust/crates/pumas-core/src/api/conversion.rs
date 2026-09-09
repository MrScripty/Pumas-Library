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
    /// Managed execution excludes setup and other managed conversions at the
    /// same stable root through cleanup, publication and indexing. Contention
    /// becomes terminal failed progress, not an automatic retry. Direct backend
    /// calls, independent readiness probes and external tools are caller-coordinated.
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

    /// Request cooperative cancellation of a running conversion.
    /// `true` acknowledges the request, not completed cleanup. Observe conversion
    /// progress for its terminal outcome; unknown or finished workers return false.
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
    /// Overlapping reads share a retained probe; later reads refresh its result.
    /// Missing interpreter or normal nonzero import exit returns false. Probe
    /// infrastructure, signal, deadline or cleanup failure returns `ConversionFailed`;
    /// closed/cancelled admission returns `ConversionCancelled`. Dropped callers
    /// do not detach work; `shutdown_conversion_setup` drains it before runtime
    /// shutdown. The five-second command budget does not bound cleanup time.
    /// Reads do not install and callers must coordinate them with setup.
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

    /// Start or inspect setup for a built-in backend, including PythonConversion.
    /// `None` never retries a retained operation. Only a matching terminal ID
    /// admits a successor; stale IDs return the selected backend's current record.
    /// Malformed IDs or retry without an owner-local record return `InvalidParams`.
    /// Records are process-local and shared with the corresponding ensure method.
    /// Setup excludes managed conversions at the same stable root; contention
    /// fails the setup operation without automatic retry. Callers must still
    /// exclude direct backend execution, independent probes and external tools:
    /// native repair may clean/rebuild generated outputs.
    /// Dropped callers do not cancel setup; drain `shutdown_conversion_setup`
    /// before stopping the runtime. Closed admission returns `InstallationCancelled`.
    /// See [`conversion::ConversionManager::start_backend_setup`] for the contract.
    pub async fn start_backend_setup(
        &self,
        backend: conversion::QuantBackend,
        expected_previous_operation_id: Option<&str>,
    ) -> Result<conversion::ConversionSetupSnapshot> {
        self.primary()
            .conversion_manager
            .start_backend_setup(backend, expected_previous_operation_id)
            .await
    }

    /// Inspect a built-in backend's retained setup without disk I/O or starting work.
    /// `None` means no owner-local record, not a readiness result. Includes
    /// PythonConversion and remains readable after setup shutdown.
    pub fn get_backend_setup(
        &self,
        backend: conversion::QuantBackend,
    ) -> Result<Option<conversion::ConversionSetupSnapshot>> {
        self.primary().conversion_manager.get_backend_setup(backend)
    }

    /// Close base Python and built-in quantization setup/probe admission,
    /// then await cleanup. Finish caller-owned synchronous readiness calls first.
    /// Invoke before stopping the hosting runtime. Successful cancellation and
    /// cleanup return success; repeated calls preserve setup/probe failures.
    pub async fn shutdown_conversion_setup(&self) -> Result<()> {
        self.primary().conversion_manager.shutdown_setup().await
    }

    /// Close conversion admission, request cancellation and observe retained workers.
    /// Call before stopping the hosting runtime. Dropping this waiter does not
    /// release worker ownership; another caller can resume observing shutdown.
    /// Native process-tree cleanup remains governed by each conversion backend.
    pub async fn shutdown_conversions(&self) -> Result<()> {
        self.primary().conversion_manager.shutdown().await
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
    /// Setup and managed conversions share root-level exclusion. Callers must
    /// exclude direct backend execution, independent probes and external tools:
    /// native repair may clean/rebuild generated CMake outputs.
    /// Dropping this waiter does not release installer ownership; use
    /// `shutdown_conversion_setup` before stopping the host runtime.
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
