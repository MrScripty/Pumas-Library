//! Backend-owned user-directed serving service.
//!
//! This module owns served-model status snapshots and validation helpers for
//! model-row/modal serving requests. Provider-specific load/unload behavior is
//! added in later slices behind this service boundary.

mod gateway_alias;
mod placement;

use tokio::sync::{broadcast, RwLock};

pub use gateway_alias::effective_gateway_model_alias;
use gateway_alias::{validate_gateway_alias_contract, validate_gateway_alias_is_unique};
use placement::validate_provider_placement;

use crate::models::{
    ModelServeError, ModelServeErrorCode, ModelServeValidationResponse, RuntimeDeviceMode,
    RuntimeLifecycleState, RuntimeManagementMode, RuntimeProfileId, RuntimeProviderId,
    RuntimeProviderMode, ServeModelRequest, ServedModelStatus, ServingEndpointMode,
    ServingEndpointStatus, ServingStatusEvent, ServingStatusEventKind, ServingStatusResponse,
    ServingStatusSnapshot, ServingStatusUpdateFeed, ServingStatusUpdateFeedResponse,
};
use crate::providers::{ExecutableArtifactFormat, ProviderRegistry};

const SERVING_STATUS_UPDATE_CHANNEL_CAPACITY: usize = 64;
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
    pub primary_artifact_format: Option<ExecutableArtifactFormat>,
    pub profile: Option<ServingValidationProfile>,
    pub served_models: Vec<ServedModelStatus>,
}

#[derive(Debug)]
pub struct ServingService {
    snapshot: RwLock<ServingStatusSnapshot>,
    updates: broadcast::Sender<ServingStatusUpdateFeed>,
    provider_registry: ProviderRegistry,
}

impl ServingService {
    pub fn with_provider_registry(provider_registry: ProviderRegistry) -> Self {
        Self {
            snapshot: RwLock::new(ServingStatusSnapshot::empty()),
            updates: broadcast::channel(SERVING_STATUS_UPDATE_CHANNEL_CAPACITY).0,
            provider_registry,
        }
    }

    pub async fn status(&self) -> ServingStatusResponse {
        ServingStatusResponse {
            success: true,
            error: None,
            snapshot: self.snapshot.read().await.clone(),
        }
    }

    pub async fn list_updates_since(
        &self,
        cursor: Option<&str>,
    ) -> ServingStatusUpdateFeedResponse {
        let current_cursor = self.snapshot.read().await.cursor.clone();
        ServingStatusUpdateFeedResponse {
            success: true,
            error: None,
            feed: build_update_feed(cursor, &current_cursor),
        }
    }

    pub fn subscribe_updates(&self) -> broadcast::Receiver<ServingStatusUpdateFeed> {
        self.updates.subscribe()
    }

    pub async fn record_loaded_model(&self, status: ServedModelStatus) -> ServingStatusSnapshot {
        let mut snapshot = self.snapshot.write().await;
        snapshot
            .served_models
            .retain(|model| !same_served_model(model, &status));
        snapshot.served_models.push(status.clone());
        snapshot.endpoint = ServingEndpointStatus {
            endpoint_mode: ServingEndpointMode::PumasGateway,
            endpoint_url: None,
            model_count: snapshot.served_models.len() as u32,
            message: Some("Use the Pumas /v1 serving gateway for loaded models".to_string()),
        };
        bump_snapshot_cursor(&mut snapshot);
        let event = serving_status_event(
            snapshot.cursor.clone(),
            ServingStatusEventKind::ModelLoaded,
            Some(status.model_id.clone()),
            Some(status.profile_id.clone()),
            Some(status.provider),
        );
        self.publish_event(event);
        snapshot.clone()
    }

    pub async fn record_load_error(&self, error: ModelServeError) -> ServingStatusSnapshot {
        let mut snapshot = self.snapshot.write().await;
        snapshot.last_errors.push(error);
        bump_snapshot_cursor(&mut snapshot);
        let event = serving_status_event(
            snapshot.cursor.clone(),
            ServingStatusEventKind::LoadFailed,
            snapshot
                .last_errors
                .last()
                .and_then(|error| error.model_id.clone()),
            snapshot
                .last_errors
                .last()
                .and_then(|error| error.profile_id.clone()),
            snapshot.last_errors.last().and_then(|error| error.provider),
        );
        self.publish_event(event);
        snapshot.clone()
    }

    pub async fn record_unloaded_model(
        &self,
        model_id: &str,
        provider: Option<RuntimeProviderId>,
        profile_id: Option<&RuntimeProfileId>,
        model_alias: Option<&str>,
    ) -> ServingStatusSnapshot {
        let mut snapshot = self.snapshot.write().await;
        snapshot.served_models.retain(|status| {
            if status.model_id != model_id {
                return true;
            }
            if let Some(provider) = provider {
                if status.provider != provider {
                    return true;
                }
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
        let event = serving_status_event(
            snapshot.cursor.clone(),
            ServingStatusEventKind::ModelUnloaded,
            Some(model_id.to_string()),
            profile_id.cloned(),
            provider,
        );
        self.publish_event(event);
        snapshot.clone()
    }

    pub async fn record_profile_unavailable(
        &self,
        profile_id: &RuntimeProfileId,
    ) -> Option<ServingStatusSnapshot> {
        let mut snapshot = self.snapshot.write().await;
        let mut removed_models = Vec::new();
        snapshot.served_models.retain(|status| {
            if &status.profile_id == profile_id {
                removed_models.push(status.clone());
                return false;
            }
            true
        });

        if removed_models.is_empty() {
            return None;
        }

        snapshot.endpoint.model_count = snapshot.served_models.len() as u32;
        if snapshot.served_models.is_empty() {
            snapshot.endpoint = ServingEndpointStatus::not_configured();
        }
        bump_snapshot_cursor(&mut snapshot);
        let cursor = snapshot.cursor.clone();
        let events = removed_models
            .into_iter()
            .map(|status| {
                serving_status_event(
                    cursor.clone(),
                    ServingStatusEventKind::ModelUnloaded,
                    Some(status.model_id),
                    Some(status.profile_id),
                    Some(status.provider),
                )
            })
            .collect();
        self.publish_feed(ServingStatusUpdateFeed {
            cursor,
            events,
            stale_cursor: false,
            snapshot_required: false,
        });
        Some(snapshot.clone())
    }

    pub async fn find_served_model(
        &self,
        model_id: &str,
        provider: Option<RuntimeProviderId>,
        profile_id: Option<&RuntimeProfileId>,
    ) -> Option<ServedModelStatus> {
        self.snapshot
            .read()
            .await
            .served_models
            .iter()
            .find(|status| {
                status.model_id == model_id
                    && provider.is_none_or(|provider| status.provider == provider)
                    && profile_id.is_none_or(|profile_id| &status.profile_id == profile_id)
            })
            .cloned()
    }

    pub fn validate_request(
        &self,
        request: &ServeModelRequest,
        context: &ServingValidationContext,
    ) -> ModelServeValidationResponse {
        validate_model_serving_request(request, context, &self.provider_registry)
    }

    fn publish_event(&self, event: ServingStatusEvent) {
        self.publish_feed(ServingStatusUpdateFeed {
            cursor: event.cursor.clone(),
            events: vec![event],
            stale_cursor: false,
            snapshot_required: false,
        });
    }

    fn publish_feed(&self, feed: ServingStatusUpdateFeed) {
        let _ = self.updates.send(ServingStatusUpdateFeed {
            cursor: feed.cursor,
            events: feed.events,
            stale_cursor: feed.stale_cursor,
            snapshot_required: feed.snapshot_required,
        });
    }
}

fn same_served_model(left: &ServedModelStatus, right: &ServedModelStatus) -> bool {
    left.model_id == right.model_id
        && left.provider == right.provider
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

fn build_update_feed(cursor: Option<&str>, current_cursor: &str) -> ServingStatusUpdateFeed {
    let Some(requested_cursor) = cursor else {
        return ServingStatusUpdateFeed::snapshot_required(current_cursor.to_string());
    };
    let Some(requested) = parse_serving_cursor(requested_cursor) else {
        return ServingStatusUpdateFeed::snapshot_required(current_cursor.to_string());
    };
    let Some(current) = parse_serving_cursor(current_cursor) else {
        return ServingStatusUpdateFeed::snapshot_required(current_cursor.to_string());
    };
    if requested == current {
        return ServingStatusUpdateFeed::empty(Some(current_cursor));
    }
    ServingStatusUpdateFeed::snapshot_required(current_cursor.to_string())
}

fn parse_serving_cursor(cursor: &str) -> Option<u64> {
    cursor
        .strip_prefix("serving:")
        .and_then(|value| value.parse::<u64>().ok())
}

fn serving_status_event(
    cursor: String,
    event_kind: ServingStatusEventKind,
    model_id: Option<String>,
    profile_id: Option<RuntimeProfileId>,
    provider: Option<RuntimeProviderId>,
) -> ServingStatusEvent {
    ServingStatusEvent {
        cursor,
        event_kind,
        model_id,
        profile_id,
        provider,
    }
}

pub fn validate_model_serving_request(
    request: &ServeModelRequest,
    context: &ServingValidationContext,
    provider_registry: &ProviderRegistry,
) -> ModelServeValidationResponse {
    let mut errors = Vec::new();
    let model_id = request.model_id.trim();
    let effective_alias = effective_gateway_model_alias(request);

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
    errors.extend(validate_gateway_alias_contract(
        model_id,
        &request.config.profile_id,
        request.config.provider,
        request.config.model_alias.as_deref(),
    ));
    errors.extend(validate_gateway_alias_is_unique(
        model_id,
        effective_alias.as_str(),
        request,
        context,
    ));

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

            if !profile_accepts_serving_operation(profile, provider_registry) {
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

            if let Some(behavior) = provider_registry.get(request.config.provider) {
                errors.extend(validate_provider_placement(
                    model_id, request, profile, context, behavior,
                ));
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
        errors.extend(validate_provider_artifact_compatibility(
            model_id,
            request,
            context,
            provider_registry,
        ));
    }

    ModelServeValidationResponse::from_errors(errors)
}

pub(super) fn served_model_reserves_gateway_alias(status: &ServedModelStatus) -> bool {
    matches!(
        status.load_state,
        crate::models::ServedModelLoadState::Requested
            | crate::models::ServedModelLoadState::Loading
            | crate::models::ServedModelLoadState::Loaded
            | crate::models::ServedModelLoadState::Unloading
    )
}

fn validate_provider_artifact_compatibility(
    model_id: &str,
    request: &ServeModelRequest,
    context: &ServingValidationContext,
    provider_registry: &ProviderRegistry,
) -> Vec<ModelServeError> {
    let Some(format) = context.primary_artifact_format else {
        return vec![ModelServeError::non_critical(
            ModelServeErrorCode::ModelNotExecutable,
            "model has no executable primary artifact",
        )
        .for_model(model_id)
        .for_profile(request.config.profile_id.clone())
        .for_provider(request.config.provider)];
    };
    let Some(behavior) = provider_registry.get(request.config.provider) else {
        return vec![ModelServeError::non_critical(
            ModelServeErrorCode::UnsupportedProvider,
            "selected provider is not registered",
        )
        .for_model(model_id)
        .for_profile(request.config.profile_id.clone())
        .for_provider(request.config.provider)];
    };
    if behavior.supports_artifact_format(format) {
        return Vec::new();
    }

    vec![ModelServeError::non_critical(
        ModelServeErrorCode::InvalidFormat,
        "selected provider does not support this model artifact format",
    )
    .for_model(model_id)
    .for_profile(request.config.profile_id.clone())
    .for_provider(request.config.provider)]
}

fn profile_accepts_serving_operation(
    profile: &ServingValidationProfile,
    provider_registry: &ProviderRegistry,
) -> bool {
    if matches!(
        profile.state,
        RuntimeLifecycleState::Running | RuntimeLifecycleState::External
    ) {
        return true;
    }

    let Some(behavior) = provider_registry.get(profile.provider) else {
        return false;
    };

    behavior.supports_launch_on_serve(profile.provider_mode)
        && profile.management_mode == RuntimeManagementMode::Managed
        && matches!(
            profile.state,
            RuntimeLifecycleState::Stopped
                | RuntimeLifecycleState::Failed
                | RuntimeLifecycleState::Unknown
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ModelServeErrorSeverity, ModelServingConfig, RuntimeDeviceMode, RuntimeProfileId,
        ServedModelLoadState,
    };
    use crate::providers::ProviderBehavior;

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
            primary_artifact_format: Some(ExecutableArtifactFormat::Gguf),
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
            served_models: Vec::new(),
        }
    }

    fn onnx_request() -> ServeModelRequest {
        let mut request = request();
        request.config.provider = RuntimeProviderId::OnnxRuntime;
        request.config.profile_id = RuntimeProfileId::parse("onnx-default").unwrap();
        request
    }

    fn onnx_context() -> ServingValidationContext {
        ServingValidationContext {
            model_exists: true,
            primary_artifact_format: Some(ExecutableArtifactFormat::Onnx),
            profile: Some(ServingValidationProfile {
                provider: RuntimeProviderId::OnnxRuntime,
                provider_mode: RuntimeProviderMode::OnnxServe,
                management_mode: RuntimeManagementMode::Managed,
                state: RuntimeLifecycleState::Running,
                device_mode: RuntimeDeviceMode::Auto,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
            }),
            served_models: Vec::new(),
        }
    }

    fn validate(
        request: &ServeModelRequest,
        context: &ServingValidationContext,
    ) -> ModelServeValidationResponse {
        validate_model_serving_request(request, context, &ProviderRegistry::builtin())
    }

    fn service() -> ServingService {
        ServingService::with_provider_registry(ProviderRegistry::builtin())
    }

    fn loaded_status(
        model_id: &str,
        profile_id: &str,
        model_alias: Option<&str>,
    ) -> ServedModelStatus {
        ServedModelStatus {
            model_id: model_id.to_string(),
            model_alias: model_alias.map(str::to_string),
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse(profile_id).unwrap(),
            load_state: ServedModelLoadState::Loaded,
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
        }
    }

    #[tokio::test]
    async fn serving_service_starts_with_not_configured_snapshot() {
        let service = service();

        let status = service.status().await;

        assert!(status.success);
        assert_eq!(status.snapshot.schema_version, 1);
        assert!(status.snapshot.served_models.is_empty());
    }

    #[tokio::test]
    async fn serving_service_records_loaded_and_unloaded_models() {
        let service = service();
        let mut updates = service.subscribe_updates();
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
        let loaded_feed = updates.recv().await.unwrap();
        assert_eq!(loaded_feed.cursor, loaded.cursor);
        assert_eq!(
            loaded_feed.events[0].event_kind,
            ServingStatusEventKind::ModelLoaded
        );
        assert_eq!(
            loaded.endpoint.endpoint_mode,
            ServingEndpointMode::PumasGateway
        );
        assert!(service
            .list_updates_since(Some(loaded.cursor.as_str()))
            .await
            .feed
            .events
            .is_empty());
        assert_eq!(
            service
                .find_served_model(
                    "models/example",
                    Some(status.provider),
                    Some(&status.profile_id),
                )
                .await
                .as_ref()
                .and_then(|status| status.model_alias.as_deref()),
            Some("example")
        );

        let unloaded = service
            .record_unloaded_model(
                "models/example",
                Some(status.provider),
                Some(&status.profile_id),
                Some("example"),
            )
            .await;

        assert!(unloaded.served_models.is_empty());
        let unloaded_feed = updates.recv().await.unwrap();
        assert_eq!(unloaded_feed.cursor, unloaded.cursor);
        assert_eq!(
            unloaded_feed.events[0].event_kind,
            ServingStatusEventKind::ModelUnloaded
        );
        assert_eq!(
            unloaded.endpoint.endpoint_mode,
            ServingEndpointMode::NotConfigured
        );
        assert!(
            service
                .list_updates_since(Some(loaded.cursor.as_str()))
                .await
                .feed
                .snapshot_required
        );
    }

    #[tokio::test]
    async fn serving_service_unloads_only_matching_provider_instance() {
        let service = service();
        let mut llama = loaded_status("models/shared", "shared-profile", Some("shared"));
        llama.provider = RuntimeProviderId::LlamaCpp;
        let mut ollama = loaded_status("models/shared", "shared-profile", Some("shared"));
        ollama.provider = RuntimeProviderId::Ollama;
        let profile_id = llama.profile_id.clone();

        service.record_loaded_model(llama).await;
        let loaded = service.record_loaded_model(ollama).await;
        assert_eq!(loaded.served_models.len(), 2);

        let unloaded = service
            .record_unloaded_model(
                "models/shared",
                Some(RuntimeProviderId::Ollama),
                Some(&profile_id),
                Some("shared"),
            )
            .await;

        assert_eq!(unloaded.served_models.len(), 1);
        assert_eq!(
            unloaded.served_models[0].provider,
            RuntimeProviderId::LlamaCpp
        );
        assert!(service
            .find_served_model(
                "models/shared",
                Some(RuntimeProviderId::Ollama),
                Some(&profile_id),
            )
            .await
            .is_none());
        assert!(service
            .find_served_model(
                "models/shared",
                Some(RuntimeProviderId::LlamaCpp),
                Some(&profile_id),
            )
            .await
            .is_some());
    }

    #[tokio::test]
    async fn serving_service_removes_profile_models_when_profile_becomes_unavailable() {
        let service = service();
        let mut updates = service.subscribe_updates();
        let profile_id = RuntimeProfileId::parse("llama-gpu").unwrap();

        service
            .record_loaded_model(loaded_status("models/keep", "llama-cpu", Some("keep-cpu")))
            .await;
        service
            .record_loaded_model(loaded_status(
                "models/remove-a",
                "llama-gpu",
                Some("remove-gpu-a"),
            ))
            .await;
        service
            .record_loaded_model(loaded_status(
                "models/remove-b",
                "llama-gpu",
                Some("remove-gpu-b"),
            ))
            .await;
        for _ in 0..3 {
            updates.recv().await.unwrap();
        }

        let snapshot = service
            .record_profile_unavailable(&profile_id)
            .await
            .expect("profile-owned served models should be removed");

        assert_eq!(snapshot.served_models.len(), 1);
        assert_eq!(snapshot.served_models[0].model_id, "models/keep");
        assert_eq!(snapshot.endpoint.model_count, 1);
        assert_eq!(
            snapshot.endpoint.endpoint_mode,
            ServingEndpointMode::PumasGateway
        );

        let feed = updates.recv().await.unwrap();
        assert_eq!(feed.cursor, snapshot.cursor);
        assert_eq!(feed.events.len(), 2);
        assert!(feed.events.iter().all(|event| event.event_kind
            == ServingStatusEventKind::ModelUnloaded
            && event.profile_id.as_ref() == Some(&profile_id)
            && event.provider == Some(RuntimeProviderId::LlamaCpp)));
        assert!(feed
            .events
            .iter()
            .any(|event| event.model_id.as_deref() == Some("models/remove-a")));
        assert!(feed
            .events
            .iter()
            .any(|event| event.model_id.as_deref() == Some("models/remove-b")));
        assert!(service
            .record_profile_unavailable(&profile_id)
            .await
            .is_none());
        assert!(updates.try_recv().is_err());
    }

    #[test]
    fn validation_accepts_existing_gguf_on_running_profile() {
        let response = validate(&request(), &valid_context());

        assert!(response.success);
        assert!(response.valid);
        assert!(response.errors.is_empty());
    }

    #[test]
    fn validation_accepts_onnx_artifact_on_running_onnx_profile() {
        let response = validate(&onnx_request(), &onnx_context());

        assert!(response.success);
        assert!(response.valid);
        assert!(response.errors.is_empty());
    }

    #[test]
    fn validation_rejects_onnx_profile_for_unsupported_artifact_and_placement() {
        let mut request = onnx_request();
        request.config.gpu_layers = Some(8);
        let mut context = onnx_context();
        context.primary_artifact_format = Some(ExecutableArtifactFormat::Gguf);

        let response = validate(&request, &context);

        assert!(response.success);
        assert!(!response.valid);
        assert!(response
            .errors
            .iter()
            .any(|error| error.code == ModelServeErrorCode::InvalidFormat));
        assert!(response.errors.iter().any(|error| {
            error.code == ModelServeErrorCode::UnsupportedPlacement
                && error
                    .message
                    .contains("selected provider does not support per-model GPU layer")
        }));
    }

    #[test]
    fn serving_service_validation_uses_composed_provider_registry() {
        let service = ServingService::with_provider_registry(ProviderRegistry::from_behaviors([
            ProviderBehavior::llama_cpp(),
        ]));
        let response = service.validate_request(&request(), &valid_context());

        assert!(response.success);
        assert!(!response.valid);
        assert_eq!(
            response.errors[0].code,
            ModelServeErrorCode::UnsupportedProvider
        );
    }

    #[test]
    fn validation_uses_composed_provider_artifact_compatibility() {
        let mut behavior = ProviderBehavior::ollama();
        behavior.local_artifact_formats.clear();
        let service =
            ServingService::with_provider_registry(ProviderRegistry::from_behaviors([behavior]));

        let response = service.validate_request(&request(), &valid_context());

        assert!(response.success);
        assert!(!response.valid);
        assert!(response.errors.iter().any(|error| {
            error.code == ModelServeErrorCode::InvalidFormat
                && error
                    .message
                    .contains("selected provider does not support this model artifact format")
        }));
    }

    #[test]
    fn validation_returns_non_critical_domain_errors() {
        let mut context = valid_context();
        context.model_exists = false;
        context.primary_artifact_format = None;
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::External,
            state: RuntimeLifecycleState::Stopped,
            device_mode: RuntimeDeviceMode::Auto,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });

        let response = validate(&request(), &context);

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

        let response = validate(&request, &valid_context());

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

        let response = validate(&request, &context);

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

        let response = validate(&request, &context);

        assert!(response.valid);
    }

    #[test]
    fn validation_accepts_stopped_managed_llama_cpp_router_profile() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-router").unwrap();
        request.config.device_mode = RuntimeDeviceMode::Cpu;

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Stopped,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });

        let response = validate(&request, &context);

        assert!(response.valid);
    }

    #[test]
    fn validation_rejects_llama_cpp_router_per_load_placement_overrides() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-dedicated").unwrap();
        request.config.device_mode = RuntimeDeviceMode::Gpu;
        request.config.device_id = Some("cuda:1".to_string());
        request.config.gpu_layers = Some(36);
        request.config.tensor_split = Some(vec![3.0, 1.0]);

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Running,
            device_mode: RuntimeDeviceMode::Hybrid,
            device_id: Some("cuda:0".to_string()),
            gpu_layers: Some(24),
            tensor_split: Some(vec![1.0, 1.0]),
        });

        let response = validate(&request, &context);

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
            4
        );
    }

    #[test]
    fn validation_accepts_llama_cpp_router_context_size_before_load() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-router").unwrap();
        request.config.device_mode = RuntimeDeviceMode::Cpu;
        request.config.context_size = Some(8192);

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Running,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });

        let response = validate(&request, &context);

        assert!(response.valid);
    }

    #[test]
    fn validation_rejects_llama_cpp_router_context_change_with_loaded_models() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-router").unwrap();
        request.config.device_mode = RuntimeDeviceMode::Cpu;
        request.config.context_size = Some(8192);

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Running,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });
        let mut loaded = loaded_status("models/loaded", "llama-router", Some("loaded"));
        loaded.context_size = Some(4096);
        context.served_models = vec![loaded];

        let response = validate(&request, &context);

        assert!(!response.valid);
        assert!(response.errors.iter().any(|error| {
            error.code == ModelServeErrorCode::UnsupportedPlacement
                && error.message.contains("cannot be changed")
        }));
    }

    #[test]
    fn validation_rejects_duplicate_effective_gateway_alias() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-gpu").unwrap();
        request.config.model_alias = Some("shared_alias".to_string());

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppDedicated,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Stopped,
            device_mode: RuntimeDeviceMode::Auto,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });
        context.served_models = vec![loaded_status(
            "models/other",
            "llama-cpu",
            Some("shared.alias"),
        )];

        let response = validate(&request, &context);

        assert!(!response.valid);
        assert!(response
            .errors
            .iter()
            .any(|error| error.code == ModelServeErrorCode::DuplicateModelAlias));
    }

    #[test]
    fn validation_rejects_invalid_gateway_alias_characters() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-gpu").unwrap();
        request.config.model_alias = Some("Bad Alias".to_string());

        let response = validate(&request, &valid_context());

        assert!(!response.valid);
        assert!(response
            .errors
            .iter()
            .any(|error| error.code == ModelServeErrorCode::InvalidRequest
                && error.message.contains("model_alias")));
    }

    #[test]
    fn validation_allows_revalidating_same_served_instance_alias() {
        let mut request = request();
        request.config.provider = RuntimeProviderId::LlamaCpp;
        request.config.profile_id = RuntimeProfileId::parse("llama-cpu").unwrap();
        request.config.model_alias = Some("example-cpu".to_string());

        let mut context = valid_context();
        context.profile = Some(ServingValidationProfile {
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppDedicated,
            management_mode: RuntimeManagementMode::Managed,
            state: RuntimeLifecycleState::Stopped,
            device_mode: RuntimeDeviceMode::Auto,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        });
        context.served_models = vec![loaded_status(
            "models/example",
            "llama-cpu",
            Some("example-cpu"),
        )];

        let response = validate(&request, &context);

        assert!(response.valid);
    }
}
