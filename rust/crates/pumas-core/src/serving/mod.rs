//! Backend-owned user-directed serving service.
//!
//! This module owns served-model status snapshots and validation helpers for
//! model-row/modal serving requests. Provider-specific load/unload behavior is
//! added in later slices behind this service boundary.

use tokio::sync::RwLock;

use crate::models::{
    ModelServeError, ModelServeErrorCode, ModelServeValidationResponse, RuntimeDeviceMode,
    RuntimeLifecycleState, RuntimeManagementMode, RuntimeProfileId, RuntimeProviderId,
    RuntimeProviderMode, ServeModelRequest, ServedModelStatus, ServingEndpointMode,
    ServingEndpointStatus, ServingStatusResponse, ServingStatusSnapshot,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ServingValidationProfile {
    pub provider: RuntimeProviderId,
    pub provider_mode: RuntimeProviderMode,
    pub management_mode: RuntimeManagementMode,
    pub state: RuntimeLifecycleState,
    pub device_mode: RuntimeDeviceMode,
    pub device_id: Option<String>,
    pub gpu_layers: Option<i32>,
    pub tensor_split: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServingValidationContext {
    pub model_exists: bool,
    pub primary_artifact_extension: Option<String>,
    pub profile: Option<ServingValidationProfile>,
}

#[derive(Debug)]
pub struct ServingService {
    snapshot: RwLock<ServingStatusSnapshot>,
}

impl ServingService {
    pub fn new() -> Self {
        Self {
            snapshot: RwLock::new(ServingStatusSnapshot::empty()),
        }
    }

    pub async fn status(&self) -> ServingStatusResponse {
        ServingStatusResponse {
            success: true,
            error: None,
            snapshot: self.snapshot.read().await.clone(),
        }
    }

    pub async fn record_loaded_model(&self, status: ServedModelStatus) -> ServingStatusSnapshot {
        let mut snapshot = self.snapshot.write().await;
        snapshot
            .served_models
            .retain(|model| !same_served_model(model, &status));
        snapshot.served_models.push(status.clone());
        snapshot.endpoint = ServingEndpointStatus {
            endpoint_mode: ServingEndpointMode::ProviderEndpoint,
            endpoint_url: status.endpoint_url,
            model_count: snapshot.served_models.len() as u32,
            message: None,
        };
        bump_snapshot_cursor(&mut snapshot);
        snapshot.clone()
    }

    pub async fn record_load_error(&self, error: ModelServeError) -> ServingStatusSnapshot {
        let mut snapshot = self.snapshot.write().await;
        snapshot.last_errors.push(error);
        bump_snapshot_cursor(&mut snapshot);
        snapshot.clone()
    }

    pub async fn record_unloaded_model(
        &self,
        model_id: &str,
        profile_id: Option<&RuntimeProfileId>,
        model_alias: Option<&str>,
    ) -> ServingStatusSnapshot {
        let mut snapshot = self.snapshot.write().await;
        snapshot.served_models.retain(|status| {
            if status.model_id != model_id {
                return true;
            }
            if let Some(profile_id) = profile_id {
                if &status.profile_id != profile_id {
                    return true;
                }
            }
            if let Some(model_alias) = model_alias {
                if status.model_alias.as_deref() != Some(model_alias) {
                    return true;
                }
            }
            false
        });
        snapshot.endpoint.model_count = snapshot.served_models.len() as u32;
        if snapshot.served_models.is_empty() {
            snapshot.endpoint = ServingEndpointStatus::not_configured();
        }
        bump_snapshot_cursor(&mut snapshot);
        snapshot.clone()
    }

    pub async fn find_served_model(
        &self,
        model_id: &str,
        profile_id: Option<&RuntimeProfileId>,
    ) -> Option<ServedModelStatus> {
        self.snapshot
            .read()
            .await
            .served_models
            .iter()
            .find(|status| {
                status.model_id == model_id
                    && profile_id.is_none_or(|profile_id| &status.profile_id == profile_id)
            })
            .cloned()
    }

    pub fn validate_request(
        request: &ServeModelRequest,
        context: &ServingValidationContext,
    ) -> ModelServeValidationResponse {
        validate_model_serving_request(request, context)
    }
}

fn same_served_model(left: &ServedModelStatus, right: &ServedModelStatus) -> bool {
    left.model_id == right.model_id
        && left.profile_id == right.profile_id
        && left.model_alias == right.model_alias
}

fn bump_snapshot_cursor(snapshot: &mut ServingStatusSnapshot) {
    let next = snapshot
        .cursor
        .strip_prefix("serving:")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    snapshot.cursor = format!("serving:{next}");
}

pub fn validate_model_serving_request(
    request: &ServeModelRequest,
    context: &ServingValidationContext,
) -> ModelServeValidationResponse {
    let mut errors = Vec::new();
    let model_id = request.model_id.trim();

    if model_id.is_empty() {
        errors.push(ModelServeError::non_critical(
            ModelServeErrorCode::InvalidRequest,
            "model_id is required",
        ));
    } else if !context.model_exists {
        errors.push(
            ModelServeError::non_critical(
                ModelServeErrorCode::ModelNotFound,
                "model was not found in the Pumas library",
            )
            .for_model(model_id),
        );
    }

    errors.extend(request.config.validate_numeric_bounds(model_id));

    match &context.profile {
        Some(profile) => {
            if profile.provider != request.config.provider {
                errors.push(
                    ModelServeError::non_critical(
                        ModelServeErrorCode::UnsupportedProvider,
                        "selected profile provider does not match the serving request",
                    )
                    .for_model(model_id)
                    .for_profile(request.config.profile_id.clone())
                    .for_provider(request.config.provider),
                );
            }

            if !profile_accepts_serving_operation(profile) {
                errors.push(
                    ModelServeError::non_critical(
                        ModelServeErrorCode::ProfileStopped,
                        "selected runtime profile is not running",
                    )
                    .for_model(model_id)
                    .for_profile(request.config.profile_id.clone())
                    .for_provider(request.config.provider),
                );
            }

            errors.extend(validate_provider_placement(model_id, request, profile));
        }
        None => {
            errors.push(
                ModelServeError::non_critical(
                    ModelServeErrorCode::ProfileNotFound,
                    "selected runtime profile was not found",
                )
                .for_model(model_id)
                .for_profile(request.config.profile_id.clone())
                .for_provider(request.config.provider),
            );
        }
    }

    if context.model_exists {
        match context.primary_artifact_extension.as_deref() {
            Some("gguf") => {}
            Some(_) => errors.push(
                ModelServeError::non_critical(
                    ModelServeErrorCode::InvalidFormat,
                    "selected provider requires a GGUF model artifact",
                )
                .for_model(model_id)
                .for_profile(request.config.profile_id.clone())
                .for_provider(request.config.provider),
            ),
            None => errors.push(
                ModelServeError::non_critical(
                    ModelServeErrorCode::ModelNotExecutable,
                    "model has no executable primary artifact",
                )
                .for_model(model_id)
                .for_profile(request.config.profile_id.clone())
                .for_provider(request.config.provider),
            ),
        }
    }

    ModelServeValidationResponse::from_errors(errors)
}

fn profile_accepts_serving_operation(profile: &ServingValidationProfile) -> bool {
    if matches!(
        profile.state,
        RuntimeLifecycleState::Running | RuntimeLifecycleState::External
    ) {
        return true;
    }

    profile.provider == RuntimeProviderId::LlamaCpp
        && profile.provider_mode == RuntimeProviderMode::LlamaCppDedicated
        && profile.management_mode == RuntimeManagementMode::Managed
        && matches!(
            profile.state,
            RuntimeLifecycleState::Stopped
                | RuntimeLifecycleState::Failed
                | RuntimeLifecycleState::Unknown
        )
}

fn validate_provider_placement(
    model_id: &str,
    request: &ServeModelRequest,
    profile: &ServingValidationProfile,
) -> Vec<ModelServeError> {
    match profile.provider {
        RuntimeProviderId::Ollama => validate_ollama_placement(model_id, request, profile),
        RuntimeProviderId::LlamaCpp => validate_llama_cpp_placement(model_id, request, profile),
    }
}

fn validate_llama_cpp_placement(
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
            "llama.cpp serving uses the selected runtime profile device mode; choose a matching profile or use Auto",
        ));
    }

    if let Some(requested_device_id) = request.config.device_id.as_deref() {
        if profile.device_id.as_deref() != Some(requested_device_id) {
            errors.push(unsupported_placement(
                model_id,
                request,
                "llama.cpp serving uses the selected runtime profile device ID; choose a matching profile",
            ));
        }
    }

    if let Some(requested_gpu_layers) = request.config.gpu_layers {
        if profile.gpu_layers != Some(requested_gpu_layers) {
            errors.push(unsupported_placement(
                model_id,
                request,
                "llama.cpp GPU layers must match the selected runtime profile because per-load overrides are not applied yet",
            ));
        }
    }

    if let Some(requested_tensor_split) = &request.config.tensor_split {
        if profile.tensor_split.as_ref() != Some(requested_tensor_split) {
            errors.push(unsupported_placement(
                model_id,
                request,
                "llama.cpp tensor split must match the selected runtime profile because per-load overrides are not applied yet",
            ));
        }
    }

    if request.config.context_size.is_some() {
        errors.push(unsupported_placement(
            model_id,
            request,
            "llama.cpp context size is not applied by this serving path yet",
        ));
    }

    errors
}

fn validate_ollama_placement(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ModelServeErrorSeverity, ModelServingConfig, RuntimeDeviceMode, RuntimeProfileId,
    };

    fn request() -> ServeModelRequest {
        ServeModelRequest {
            model_id: "models/example".to_string(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::Ollama,
                profile_id: RuntimeProfileId::parse("ollama-default").unwrap(),
                device_mode: RuntimeDeviceMode::Auto,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: None,
                keep_loaded: false,
                model_alias: None,
            },
        }
    }

    fn valid_context() -> ServingValidationContext {
        ServingValidationContext {
            model_exists: true,
            primary_artifact_extension: Some("gguf".to_string()),
            profile: Some(ServingValidationProfile {
                provider: RuntimeProviderId::Ollama,
                provider_mode: RuntimeProviderMode::OllamaServe,
                management_mode: RuntimeManagementMode::Managed,
                state: RuntimeLifecycleState::Running,
                device_mode: RuntimeDeviceMode::Auto,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
            }),
        }
    }

    #[tokio::test]
    async fn serving_service_starts_with_not_configured_snapshot() {
        let service = ServingService::new();

        let status = service.status().await;

        assert!(status.success);
        assert_eq!(status.snapshot.schema_version, 1);
        assert!(status.snapshot.served_models.is_empty());
    }

    #[tokio::test]
    async fn serving_service_records_loaded_and_unloaded_models() {
        let service = ServingService::new();
        let status = ServedModelStatus {
            model_id: "models/example".to_string(),
            model_alias: Some("example".to_string()),
            provider: RuntimeProviderId::Ollama,
            profile_id: RuntimeProfileId::parse("ollama-default").unwrap(),
            load_state: crate::models::ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Auto,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: None,
            keep_loaded: true,
            endpoint_url: None,
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        };

        let loaded = service.record_loaded_model(status.clone()).await;

        assert_eq!(loaded.served_models.len(), 1);
        assert_eq!(
            loaded.endpoint.endpoint_mode,
            ServingEndpointMode::ProviderEndpoint
        );
        assert_eq!(
            service
                .find_served_model("models/example", Some(&status.profile_id))
                .await
                .as_ref()
                .and_then(|status| status.model_alias.as_deref()),
            Some("example")
        );

        let unloaded = service
            .record_unloaded_model("models/example", Some(&status.profile_id), Some("example"))
            .await;

        assert!(unloaded.served_models.is_empty());
        assert_eq!(
            unloaded.endpoint.endpoint_mode,
            ServingEndpointMode::NotConfigured
        );
    }

    #[test]
    fn validation_accepts_existing_gguf_on_running_profile() {
        let response = validate_model_serving_request(&request(), &valid_context());

        assert!(response.success);
        assert!(response.valid);
        assert!(response.errors.is_empty());
    }

    #[test]
    fn validation_returns_non_critical_domain_errors() {
        let mut context = valid_context();
        context.model_exists = false;
        context.primary_artifact_extension = None;
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Stopped,
            device_mode: RuntimeDeviceMode::Auto,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });

        let response = validate_model_serving_request(&request(), &context);

        assert!(response.success);
        assert!(!response.valid);
        assert!(response
            .errors
            .iter()
            .all(|error| error.severity == ModelServeErrorSeverity::NonCritical));
        assert!(response
            .errors
            .iter()
            .any(|error| error.code == ModelServeErrorCode::ModelNotFound));
        assert!(response
            .errors
            .iter()
            .any(|error| error.code == ModelServeErrorCode::UnsupportedProvider));
        assert!(response
            .errors
            .iter()
            .any(|error| error.code == ModelServeErrorCode::ProfileStopped));
    }

    #[test]
    fn validation_rejects_ollama_per_model_placement_fields() {
        let mut request = request();
        request.config.device_mode = RuntimeDeviceMode::Gpu;
        request.config.device_id = Some("cuda:0".to_string());
        request.config.gpu_layers = Some(32);
        request.config.tensor_split = Some(vec![1.0, 2.0]);
        request.config.context_size = Some(4096);

        let response = validate_model_serving_request(&request, &valid_context());

        assert!(!response.valid);
        assert!(response
            .errors
            .iter()
            .all(|error| error.severity == ModelServeErrorSeverity::NonCritical));
        assert!(response
            .errors
            .iter()
            .any(|error| error.code == ModelServeErrorCode::UnsupportedPlacement));
    }

    #[test]
    fn validation_accepts_ollama_profile_matching_device_request() {
        let mut request = request();
        request.config.device_mode = RuntimeDeviceMode::Cpu;
        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::Ollama,
            provider_mode: RuntimeProviderMode::OllamaServe,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Running,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });

        let response = validate_model_serving_request(&request, &context);

        assert!(response.valid);
    }

    #[test]
    fn validation_accepts_stopped_managed_llama_cpp_dedicated_profile() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-dedicated").unwrap();
        request.config.device_mode = RuntimeDeviceMode::Hybrid;
        request.config.gpu_layers = Some(24);
        request.config.tensor_split = Some(vec![1.0, 1.0]);

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppDedicated,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Stopped,
            device_mode: RuntimeDeviceMode::Hybrid,
            device_id: None,
            gpu_layers: Some(24),
            tensor_split: Some(vec![1.0, 1.0]),
        });

        let response = validate_model_serving_request(&request, &context);

        assert!(response.valid);
    }

    #[test]
    fn validation_rejects_llama_cpp_per_load_placement_overrides() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-dedicated").unwrap();
        request.config.device_mode = RuntimeDeviceMode::Gpu;
        request.config.device_id = Some("cuda:1".to_string());
        request.config.gpu_layers = Some(36);
        request.config.tensor_split = Some(vec![3.0, 1.0]);
        request.config.context_size = Some(8192);

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppDedicated,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Running,
            device_mode: RuntimeDeviceMode::Hybrid,
            device_id: Some("cuda:0".to_string()),
            gpu_layers: Some(24),
            tensor_split: Some(vec![1.0, 1.0]),
        });

        let response = validate_model_serving_request(&request, &context);

        assert!(!response.valid);
        assert!(response
            .errors
            .iter()
            .all(|error| error.severity == ModelServeErrorSeverity::NonCritical));
        assert_eq!(
            response
                .errors
                .iter()
                .filter(|error| error.code == ModelServeErrorCode::UnsupportedPlacement)
                .count(),
            5
        );
    }
}
