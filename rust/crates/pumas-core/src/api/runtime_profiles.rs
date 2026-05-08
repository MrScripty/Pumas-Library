//! Local runtime profile API methods.

use crate::models::{
    LaunchResponse, ModelRuntimeRoute, RuntimeEndpointUrl, RuntimeProfileConfig, RuntimeProfileId,
    RuntimeProfileMutationResponse, RuntimeProfileUpdateFeed, RuntimeProfileUpdateFeedResponse,
    RuntimeProfilesSnapshotResponse, RuntimeProviderId,
};
use crate::runtime_profiles::RuntimeProfileLaunchOverrides;
use crate::{PumasApi, Result};
use std::path::Path;

impl PumasApi {
    pub async fn get_runtime_profiles_snapshot(&self) -> Result<RuntimeProfilesSnapshotResponse> {
        self.primary().runtime_profile_service.snapshot().await
    }

    pub async fn list_runtime_profile_updates_since(
        &self,
        cursor: Option<&str>,
    ) -> Result<RuntimeProfileUpdateFeedResponse> {
        self.primary()
            .runtime_profile_service
            .list_updates_since(cursor)
            .await
    }

    pub fn subscribe_runtime_profile_updates(
        &self,
    ) -> tokio::sync::broadcast::Receiver<RuntimeProfileUpdateFeed> {
        self.primary().runtime_profile_service.subscribe_updates()
    }

    pub async fn upsert_runtime_profile(
        &self,
        profile: RuntimeProfileConfig,
    ) -> Result<RuntimeProfileMutationResponse> {
        self.primary()
            .runtime_profile_service
            .upsert_profile(profile)
            .await
    }

    pub async fn delete_runtime_profile(
        &self,
        profile_id: RuntimeProfileId,
    ) -> Result<RuntimeProfileMutationResponse> {
        self.primary()
            .runtime_profile_service
            .delete_profile(profile_id)
            .await
    }

    pub async fn set_model_runtime_route(
        &self,
        route: ModelRuntimeRoute,
    ) -> Result<RuntimeProfileMutationResponse> {
        self.primary()
            .runtime_profile_service
            .set_model_route(route)
            .await
    }

    pub async fn clear_model_runtime_route(
        &self,
        model_id: String,
    ) -> Result<RuntimeProfileMutationResponse> {
        self.primary()
            .runtime_profile_service
            .clear_model_route(model_id)
            .await
    }

    pub async fn resolve_runtime_profile_endpoint(
        &self,
        provider: RuntimeProviderId,
        profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        self.primary()
            .runtime_profile_service
            .resolve_profile_endpoint(provider, profile_id)
            .await
    }

    pub async fn resolve_runtime_profile_endpoint_for_operation(
        &self,
        provider: RuntimeProviderId,
        profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        self.primary()
            .runtime_profile_service
            .resolve_profile_endpoint_for_operation(provider, profile_id)
            .await
    }

    pub async fn resolve_model_runtime_profile_endpoint(
        &self,
        provider: RuntimeProviderId,
        model_id: &str,
        profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        self.primary()
            .runtime_profile_service
            .resolve_model_endpoint(provider, model_id, profile_id)
            .await
    }

    pub async fn resolve_model_runtime_profile_endpoint_for_operation(
        &self,
        provider: RuntimeProviderId,
        model_id: &str,
        profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        self.primary()
            .runtime_profile_service
            .resolve_model_endpoint_for_operation(provider, model_id, profile_id)
            .await
    }

    pub async fn model_runtime_route_auto_load(&self, model_id: &str) -> Result<Option<bool>> {
        self.primary()
            .runtime_profile_service
            .model_route_auto_load(model_id)
            .await
    }

    pub async fn launch_runtime_profile(
        &self,
        profile_id: RuntimeProfileId,
        tag: &str,
        version_dir: &Path,
    ) -> Result<LaunchResponse> {
        self.launch_runtime_profile_for_model(profile_id, tag, version_dir, None)
            .await
    }

    pub async fn launch_runtime_profile_for_model(
        &self,
        profile_id: RuntimeProfileId,
        tag: &str,
        version_dir: &Path,
        model_id: Option<&str>,
    ) -> Result<LaunchResponse> {
        self.launch_runtime_profile_for_model_with_overrides(
            profile_id,
            tag,
            version_dir,
            model_id,
            None,
        )
        .await
    }

    pub async fn launch_runtime_profile_for_model_with_overrides(
        &self,
        profile_id: RuntimeProfileId,
        tag: &str,
        version_dir: &Path,
        model_id: Option<&str>,
        overrides: Option<RuntimeProfileLaunchOverrides>,
    ) -> Result<LaunchResponse> {
        super::state_runtime_profiles::launch_runtime_profile(
            self.primary(),
            profile_id,
            tag,
            version_dir,
            model_id.map(ToOwned::to_owned),
            overrides,
        )
        .await
    }

    pub async fn stop_runtime_profile(&self, profile_id: RuntimeProfileId) -> Result<bool> {
        super::state_runtime_profiles::stop_runtime_profile(self.primary(), profile_id).await
    }

    pub async fn refresh_default_ollama_profile_status(&self) -> Result<()> {
        let is_running = self.is_ollama_running().await;
        self.primary()
            .runtime_profile_service
            .record_default_ollama_status(is_running)
            .await?;
        Ok(())
    }
}
