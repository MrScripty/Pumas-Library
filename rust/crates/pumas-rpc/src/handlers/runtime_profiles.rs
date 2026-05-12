//! Runtime profile RPC contract handlers.

use super::parse_params;
use crate::server::AppState;
use pumas_library::models::{
    ModelRuntimeRoute, RuntimeProfileConfig, RuntimeProfileId, RuntimeProviderId,
};
use pumas_library::ProviderManagedLaunchTarget;
use pumas_library::PumasError;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct RuntimeProfileUpdateParams {
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertRuntimeProfileParams {
    profile: RuntimeProfileConfig,
}

#[derive(Debug, Deserialize)]
struct DeleteRuntimeProfileParams {
    profile_id: RuntimeProfileId,
}

#[derive(Debug, Deserialize)]
struct SetModelRuntimeRouteParams {
    route: ModelRuntimeRoute,
}

#[derive(Debug, Deserialize)]
struct ClearModelRuntimeRouteParams {
    provider: RuntimeProviderId,
    model_id: String,
}

#[derive(Debug, Deserialize)]
struct LaunchRuntimeProfileParams {
    profile_id: RuntimeProfileId,
    tag: Option<String>,
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StopRuntimeProfileParams {
    profile_id: RuntimeProfileId,
}

pub async fn get_runtime_profiles_snapshot(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    Ok(serde_json::to_value(
        state.api.get_runtime_profiles_snapshot().await?,
    )?)
}

pub async fn list_runtime_profile_updates_since(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: RuntimeProfileUpdateParams =
        parse_params("list_runtime_profile_updates_since", params)?;
    Ok(serde_json::to_value(
        state
            .api
            .list_runtime_profile_updates_since(command.cursor.as_deref())
            .await?,
    )?)
}

pub async fn upsert_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: UpsertRuntimeProfileParams = parse_params("upsert_runtime_profile", params)?;
    Ok(serde_json::to_value(
        state.api.upsert_runtime_profile(command.profile).await?,
    )?)
}

pub async fn delete_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: DeleteRuntimeProfileParams = parse_params("delete_runtime_profile", params)?;
    Ok(serde_json::to_value(
        state.api.delete_runtime_profile(command.profile_id).await?,
    )?)
}

pub async fn set_model_runtime_route(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: SetModelRuntimeRouteParams = parse_params("set_model_runtime_route", params)?;
    Ok(serde_json::to_value(
        state.api.set_model_runtime_route(command.route).await?,
    )?)
}

pub async fn clear_model_runtime_route(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: ClearModelRuntimeRouteParams = parse_params("clear_model_runtime_route", params)?;
    Ok(serde_json::to_value(
        state
            .api
            .clear_model_runtime_route(command.provider, command.model_id)
            .await?,
    )?)
}

pub async fn launch_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: LaunchRuntimeProfileParams = parse_params("launch_runtime_profile", params)?;
    let snapshot = state.api.get_runtime_profiles_snapshot().await?;
    let Some(profile) = snapshot
        .snapshot
        .profiles
        .iter()
        .find(|profile| profile.profile_id == command.profile_id)
    else {
        return Ok(serde_json::json!({
            "success": false,
            "error": format!("Runtime profile not found: {}", command.profile_id.as_str()),
            "ready": false
        }));
    };

    let Some(behavior) = state.provider_registry.get(profile.provider) else {
        return Err(PumasError::InvalidParams {
            message: "runtime profile provider is not registered".to_string(),
        });
    };
    if behavior.managed_launch_strategies.iter().any(|strategy| {
        strategy.provider_mode == profile.provider_mode
            && matches!(
                strategy.target,
                ProviderManagedLaunchTarget::InProcessRuntime(_)
            )
    }) {
        return Ok(serde_json::to_value(
            state
                .api
                .launch_runtime_profile_for_model(
                    command.profile_id,
                    "in-process",
                    Path::new(""),
                    command.model_id.as_deref(),
                )
                .await?,
        )?);
    }

    let app_id = behavior.managed_runtime_app_id.as_str();
    let Some(version_manager) = super::get_version_manager(state, app_id).await else {
        return Ok(serde_json::json!({
            "success": false,
            "error": behavior.managed_runtime_uninitialized_message.as_str(),
            "ready": false
        }));
    };
    let tag = match command.tag {
        Some(tag) => tag,
        None => match version_manager.get_active_version().await? {
            Some(tag) => tag,
            None => {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": behavior.managed_runtime_no_active_version_message.as_str(),
                    "ready": false
                }));
            }
        },
    };
    let version_dir = version_manager.version_path(&tag);
    Ok(serde_json::to_value(
        state
            .api
            .launch_runtime_profile_for_model(
                command.profile_id,
                &tag,
                &version_dir,
                command.model_id.as_deref(),
            )
            .await?,
    )?)
}

pub async fn stop_runtime_profile(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let command: StopRuntimeProfileParams = parse_params("stop_runtime_profile", params)?;
    let stopped = state.api.stop_runtime_profile(command.profile_id).await?;
    Ok(serde_json::json!({ "success": stopped }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::models::{
        RuntimeDeviceSettings, RuntimeLifecycleState, RuntimeManagementMode, RuntimeProviderMode,
    };
    use tempfile::TempDir;

    #[tokio::test]
    async fn launch_in_process_onnx_profile_does_not_require_version_manager() {
        let temp_dir = TempDir::new().unwrap();
        let state = crate::handlers::test_support::build_test_app_state(temp_dir.path()).await;
        let profile_id = RuntimeProfileId::parse("onnx-smoke").unwrap();

        state
            .api
            .upsert_runtime_profile(RuntimeProfileConfig {
                profile_id: profile_id.clone(),
                provider: RuntimeProviderId::OnnxRuntime,
                provider_mode: RuntimeProviderMode::OnnxServe,
                management_mode: RuntimeManagementMode::Managed,
                name: "ONNX Smoke".to_string(),
                enabled: true,
                endpoint_url: None,
                port: None,
                device: RuntimeDeviceSettings::default(),
                scheduler: Default::default(),
            })
            .await
            .unwrap();

        let response = launch_runtime_profile(
            &state,
            &serde_json::json!({
                "profile_id": profile_id.as_str()
            }),
        )
        .await
        .unwrap();

        assert_eq!(response["success"], true);
        assert_eq!(response["ready"], true);

        let snapshot = state.api.get_runtime_profiles_snapshot().await.unwrap();
        let status = snapshot
            .snapshot
            .statuses
            .iter()
            .find(|status| status.profile_id == profile_id)
            .unwrap();
        assert_eq!(status.state, RuntimeLifecycleState::Running);
    }
}
