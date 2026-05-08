//! User-directed model serving API methods.

use crate::models::{
    ModelServeValidationResponse, RuntimeLifecycleState, ServeModelRequest, ServingStatusResponse,
};
use crate::serving::{ServingValidationContext, ServingValidationProfile};
use crate::{PumasApi, Result};

impl PumasApi {
    pub async fn get_serving_status(&self) -> Result<ServingStatusResponse> {
        Ok(self.primary().serving_service.status().await)
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
                    state,
                }
            });

        let context = ServingValidationContext {
            model_exists: model.is_some(),
            primary_artifact_extension,
            profile,
        };

        Ok(crate::serving::ServingService::validate_request(
            &request, &context,
        ))
    }
}
