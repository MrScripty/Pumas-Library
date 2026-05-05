//! Local runtime profile API methods.

use crate::models::{
    LaunchResponse, ModelRuntimeRoute, RuntimeEndpointUrl, RuntimeProfileConfig, RuntimeProfileId,
    RuntimeProfileMutationResponse, RuntimeProfileUpdateFeedResponse,
    RuntimeProfilesSnapshotResponse, RuntimeProviderId,
};
use crate::{PumasApi, Result};
use std::path::Path;

impl PumasApi {
    pub async fn get_runtime_profiles_snapshot(&self) -> Result<RuntimeProfilesSnapshotResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method("get_runtime_profiles_snapshot", serde_json::json!({}))
                .await;
        }

        self.refresh_default_ollama_profile_status().await?;
        self.primary().runtime_profile_service.snapshot().await
    }

    pub async fn list_runtime_profile_updates_since(
        &self,
        cursor: Option<&str>,
    ) -> Result<RuntimeProfileUpdateFeedResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "list_runtime_profile_updates_since",
                    serde_json::json!({ "cursor": cursor }),
                )
                .await;
        }

        self.refresh_default_ollama_profile_status().await?;
        self.primary()
            .runtime_profile_service
            .list_updates_since(cursor)
            .await
    }

    pub async fn upsert_runtime_profile(
        &self,
        profile: RuntimeProfileConfig,
    ) -> Result<RuntimeProfileMutationResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "upsert_runtime_profile",
                    serde_json::json!({ "profile": profile }),
                )
                .await;
        }

        self.primary()
            .runtime_profile_service
            .upsert_profile(profile)
            .await
    }

    pub async fn delete_runtime_profile(
        &self,
        profile_id: RuntimeProfileId,
    ) -> Result<RuntimeProfileMutationResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "delete_runtime_profile",
                    serde_json::json!({ "profile_id": profile_id }),
                )
                .await;
        }

        self.primary()
            .runtime_profile_service
            .delete_profile(profile_id)
            .await
    }

    pub async fn set_model_runtime_route(
        &self,
        route: ModelRuntimeRoute,
    ) -> Result<RuntimeProfileMutationResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "set_model_runtime_route",
                    serde_json::json!({ "route": route }),
                )
                .await;
        }

        self.primary()
            .runtime_profile_service
            .set_model_route(route)
            .await
    }

    pub async fn clear_model_runtime_route(
        &self,
        model_id: String,
    ) -> Result<RuntimeProfileMutationResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "clear_model_runtime_route",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await;
        }

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
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "resolve_runtime_profile_endpoint",
                    serde_json::json!({ "provider": provider, "profile_id": profile_id }),
                )
                .await;
        }

        self.primary()
            .runtime_profile_service
            .resolve_profile_endpoint(provider, profile_id)
            .await
    }

    pub async fn launch_runtime_profile(
        &self,
        profile_id: RuntimeProfileId,
        tag: &str,
        version_dir: &Path,
    ) -> Result<LaunchResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "launch_runtime_profile",
                    serde_json::json!({
                        "profile_id": profile_id,
                        "tag": tag,
                        "version_dir": version_dir,
                    }),
                )
                .await;
        }

        super::state_runtime_profiles::launch_runtime_profile(
            self.primary(),
            profile_id,
            tag,
            version_dir,
        )
        .await
    }

    async fn refresh_default_ollama_profile_status(&self) -> Result<()> {
        let is_running = self.is_ollama_running().await;
        self.primary()
            .runtime_profile_service
            .record_default_ollama_status(is_running)
            .await?;
        Ok(())
    }
}
