//! Backend-owned user-directed serving service.
//!
//! This module owns served-model status snapshots and validation helpers for
//! model-row/modal serving requests. Provider-specific load/unload behavior is
//! added in later slices behind this service boundary.

use tokio::sync::RwLock;

use crate::models::{
    ModelServeError, ModelServeErrorCode, ModelServeValidationResponse, RuntimeLifecycleState,
    RuntimeProviderId, ServeModelRequest, ServingStatusResponse, ServingStatusSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServingValidationProfile {
    pub provider: RuntimeProviderId,
    pub state: RuntimeLifecycleState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn validate_request(
        request: &ServeModelRequest,
        context: &ServingValidationContext,
    ) -> ModelServeValidationResponse {
        validate_model_serving_request(request, context)
    }
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

    match context.profile {
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

            if !matches!(
                profile.state,
                RuntimeLifecycleState::Running | RuntimeLifecycleState::External
            ) {
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
                context_size: Some(4096),
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
                state: RuntimeLifecycleState::Running,
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
            state: RuntimeLifecycleState::Stopped,
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
}
