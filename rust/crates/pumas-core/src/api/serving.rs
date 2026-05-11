//! User-directed model serving API methods.

use crate::models::{
    ModelServeError, ModelServeValidationResponse, RuntimeLifecycleState, RuntimeProfileId,
    ServeModelRequest, ServedModelStatus, ServingStatusResponse, ServingStatusSnapshot,
    ServingStatusUpdateFeed, ServingStatusUpdateFeedResponse,
};
use crate::serving::{ServingValidationContext, ServingValidationProfile};
use crate::{PumasApi, Result};

impl PumasApi {
    pub async fn get_serving_status(&self) -> Result<ServingStatusResponse> {
        Ok(self.primary().serving_service.status().await)
    }

    pub async fn list_serving_status_updates_since(
        &self,
        cursor: Option<&str>,
    ) -> Result<ServingStatusUpdateFeedResponse> {
        Ok(self
            .primary()
            .serving_service
            .list_updates_since(cursor)
            .await)
    }

    pub fn subscribe_serving_status_updates(
        &self,
    ) -> tokio::sync::broadcast::Receiver<ServingStatusUpdateFeed> {
        self.primary().serving_service.subscribe_updates()
    }

    pub async fn validate_model_serving_config(
        &self,
        request: ServeModelRequest,
    ) -> Result<ModelServeValidationResponse> {
        let primary = self.primary();
        let model_id = request.model_id.trim();
        let model = if model_id.is_empty() {
            None
        } else {
            primary.model_library.get_model(model_id).await?
        };
        let primary_artifact_extension = if model.is_some() {
            primary
                .model_library
                .get_primary_model_file(model_id)
                .and_then(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase())
                })
        } else {
            None
        };

        let runtime_snapshot = self.get_runtime_profiles_snapshot().await?.snapshot;
        let profile = runtime_snapshot
            .profiles
            .iter()
            .find(|profile| profile.profile_id == request.config.profile_id)
            .map(|profile| {
                let state = runtime_snapshot
                    .statuses
                    .iter()
                    .find(|status| status.profile_id == profile.profile_id)
                    .map(|status| status.state)
                    .unwrap_or(RuntimeLifecycleState::Unknown);
                ServingValidationProfile {
                    provider: profile.provider,
                    provider_mode: profile.provider_mode,
                    management_mode: profile.management_mode,
                    state,
                    device_mode: profile.device.mode,
                    device_id: profile.device.device_id.clone(),
                    gpu_layers: profile.device.gpu_layers,
                    tensor_split: profile.device.tensor_split.clone(),
                }
            });

        let context = ServingValidationContext {
            model_exists: model.is_some(),
            primary_artifact_extension,
            profile,
            served_models: primary
                .serving_service
                .status()
                .await
                .snapshot
                .served_models,
        };

        Ok(crate::serving::ServingService::validate_request(
            &request, &context,
        ))
    }

    pub async fn record_served_model(
        &self,
        status: ServedModelStatus,
    ) -> Result<ServingStatusSnapshot> {
        Ok(self
            .primary()
            .serving_service
            .record_loaded_model(status)
            .await)
    }

    pub async fn record_serving_load_error(
        &self,
        error: ModelServeError,
    ) -> Result<ServingStatusSnapshot> {
        Ok(self
            .primary()
            .serving_service
            .record_load_error(error)
            .await)
    }

    pub async fn record_unserved_model(
        &self,
        model_id: &str,
        provider: Option<crate::models::RuntimeProviderId>,
        profile_id: Option<&RuntimeProfileId>,
        model_alias: Option<&str>,
    ) -> Result<ServingStatusSnapshot> {
        Ok(self
            .primary()
            .serving_service
            .record_unloaded_model(model_id, provider, profile_id, model_alias)
            .await)
    }

    pub async fn find_served_model(
        &self,
        model_id: &str,
        provider: Option<crate::models::RuntimeProviderId>,
        profile_id: Option<&RuntimeProfileId>,
    ) -> Result<Option<ServedModelStatus>> {
        Ok(self
            .primary()
            .serving_service
            .find_served_model(model_id, provider, profile_id)
            .await)
    }
}
