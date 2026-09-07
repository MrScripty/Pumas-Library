//! Model format conversion handlers.

use crate::contract::{
    BackendStatusOutcome, ConversionCancelledOutcome, ConversionEnvironmentOutcome,
    ConversionListOutcome, ConversionProgressResponse, ConversionSetupStartedOutcome,
    ConversionSetupStatusOutcome, ConversionStartedOutcome, SupportedQuantTypesOutcome,
};
use crate::server::AppState;
use pumas_library::conversion::{ConversionRequest, QuantBackend};

pub async fn start_model_conversion(
    state: &AppState,
    request: ConversionRequest,
) -> pumas_library::Result<ConversionStartedOutcome> {
    let conversion_id = state.api.start_conversion(request).await?;
    ConversionStartedOutcome::new(conversion_id)
}

pub fn get_conversion_progress(
    state: &AppState,
    conversion_id: &str,
) -> pumas_library::Result<ConversionProgressResponse> {
    ConversionProgressResponse::new(state.api.get_conversion_progress(conversion_id))
}

pub async fn cancel_model_conversion(
    state: &AppState,
    conversion_id: &str,
) -> pumas_library::Result<ConversionCancelledOutcome> {
    let cancelled = state.api.cancel_conversion(conversion_id).await?;
    Ok(ConversionCancelledOutcome::new(cancelled))
}

pub fn list_model_conversions(state: &AppState) -> pumas_library::Result<ConversionListOutcome> {
    ConversionListOutcome::new(state.api.list_conversions())
}

pub async fn check_conversion_environment(
    state: &AppState,
) -> pumas_library::Result<ConversionEnvironmentOutcome> {
    let ready = state.api.is_conversion_environment_ready().await?;
    Ok(ConversionEnvironmentOutcome::new(ready))
}

pub async fn setup_conversion_environment(state: &AppState) -> pumas_library::Result<()> {
    state.api.ensure_conversion_environment().await
}

pub async fn start_conversion_setup(
    state: &AppState,
    expected_previous_operation_id: Option<&str>,
) -> pumas_library::Result<ConversionSetupStartedOutcome> {
    state
        .api
        .start_conversion_setup(expected_previous_operation_id)
        .await
        .and_then(ConversionSetupStartedOutcome::new)
}

pub fn get_conversion_setup(
    state: &AppState,
) -> pumas_library::Result<ConversionSetupStatusOutcome> {
    ConversionSetupStatusOutcome::new(state.api.get_conversion_setup())
}

pub async fn get_supported_quant_types(
    state: &AppState,
) -> pumas_library::Result<SupportedQuantTypesOutcome> {
    state
        .api
        .supported_quant_types()
        .await
        .and_then(SupportedQuantTypesOutcome::new)
}

pub async fn get_backend_status(state: &AppState) -> pumas_library::Result<BackendStatusOutcome> {
    state
        .api
        .backend_status()
        .await
        .map(BackendStatusOutcome::new)
}

pub async fn setup_quantization_backend(
    state: &AppState,
    backend: QuantBackend,
) -> pumas_library::Result<()> {
    state.api.ensure_backend_environment(backend).await
}
