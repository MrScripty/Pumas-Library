use crate::models::{
    ModelServeError, ModelServeErrorCode, RuntimeDeviceMode, RuntimeProviderMode, ServeModelRequest,
};
use crate::providers::{ProviderBehavior, ProviderServingPlacementPolicy};

use super::{
    served_model_reserves_gateway_alias, ServingValidationContext, ServingValidationProfile,
};

pub(super) fn validate_provider_placement(
    model_id: &str,
    request: &ServeModelRequest,
    profile: &ServingValidationProfile,
    context: &ServingValidationContext,
    behavior: &ProviderBehavior,
) -> Vec<ModelServeError> {
    match behavior.serving_placement_policy {
        ProviderServingPlacementPolicy::ProfileOnly => {
            validate_profile_only_placement(model_id, request, profile)
        }
        ProviderServingPlacementPolicy::LlamaCppRuntime => {
            validate_llama_cpp_placement(model_id, request, profile, context)
        }
    }
}

fn validate_llama_cpp_placement(
    model_id: &str,
    request: &ServeModelRequest,
    profile: &ServingValidationProfile,
    context: &ServingValidationContext,
) -> Vec<ModelServeError> {
    if profile.provider_mode == RuntimeProviderMode::LlamaCppDedicated {
        return Vec::new();
    }

    let mut errors = Vec::new();

    if request.config.device_mode != RuntimeDeviceMode::Auto
        && request.config.device_mode != profile.device_mode
    {
        errors.push(unsupported_placement(
            model_id,
            request,
            "llama.cpp router serving uses the selected runtime profile device mode; choose a matching profile or use Auto",
        ));
    }

    if let Some(requested_device_id) = request.config.device_id.as_deref() {
        if profile.device_id.as_deref() != Some(requested_device_id) {
            errors.push(unsupported_placement(
                model_id,
                request,
                "llama.cpp router serving uses the selected runtime profile device ID; choose a matching profile",
            ));
        }
    }

    if let Some(requested_gpu_layers) = request.config.gpu_layers {
        if profile.gpu_layers != Some(requested_gpu_layers) {
            errors.push(unsupported_placement(
                model_id,
                request,
                "llama.cpp router GPU layers must match the selected runtime profile because router per-load overrides are not applied",
            ));
        }
    }

    if let Some(requested_tensor_split) = &request.config.tensor_split {
        if profile.tensor_split.as_ref() != Some(requested_tensor_split) {
            errors.push(unsupported_placement(
                model_id,
                request,
                "llama.cpp router tensor split must match the selected runtime profile because router per-load overrides are not applied",
            ));
        }
    }

    if let Some(requested_context_size) = request.config.context_size {
        if context
            .served_models
            .iter()
            .filter(|status| {
                status.provider == request.config.provider
                    && status.profile_id == request.config.profile_id
                    && served_model_reserves_gateway_alias(status)
            })
            .any(|status| status.context_size != Some(requested_context_size))
        {
            errors.push(unsupported_placement(
                model_id,
                request,
                "llama.cpp router context size cannot be changed while models are loaded on the selected profile",
            ));
        }
    }

    errors
}

fn validate_profile_only_placement(
    model_id: &str,
    request: &ServeModelRequest,
    profile: &ServingValidationProfile,
) -> Vec<ModelServeError> {
    let mut errors = Vec::new();

    if request.config.device_mode != RuntimeDeviceMode::Auto
        && request.config.device_mode != profile.device_mode
    {
        errors.push(unsupported_placement(
            model_id,
            request,
            "Ollama per-model device mode is not supported; choose a runtime profile with matching device placement or use Auto",
        ));
    }

    if let Some(requested_device_id) = request.config.device_id.as_deref() {
        if profile.device_id.as_deref() != Some(requested_device_id) {
            errors.push(unsupported_placement(
                model_id,
                request,
                "Ollama per-model device IDs are not supported; choose a runtime profile for that device",
            ));
        }
    }

    if request.config.gpu_layers.is_some() {
        errors.push(unsupported_placement(
            model_id,
            request,
            "Ollama does not support per-model GPU layer settings through this serving path",
        ));
    }

    if request.config.tensor_split.is_some() {
        errors.push(unsupported_placement(
            model_id,
            request,
            "Ollama does not support per-model tensor split settings through this serving path",
        ));
    }

    if request.config.context_size.is_some() {
        errors.push(unsupported_placement(
            model_id,
            request,
            "Ollama context size is not applied by this serving path yet",
        ));
    }

    errors
}

fn unsupported_placement(
    model_id: &str,
    request: &ServeModelRequest,
    message: &'static str,
) -> ModelServeError {
    ModelServeError::non_critical(ModelServeErrorCode::UnsupportedPlacement, message)
        .for_model(model_id)
        .for_profile(request.config.profile_id.clone())
        .for_provider(request.config.provider)
}
