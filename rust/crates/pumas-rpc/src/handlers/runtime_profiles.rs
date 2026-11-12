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
    // Remote providers are supported for External profiles; plain http to
    // another host sends prompts and outputs over the network unencrypted.
    if command
        .profile
        .endpoint_url
        .as_ref()
        .is_some_and(|endpoint| endpoint.is_plaintext_remote())
    {
        tracing::warn!(
            profile_id = command.profile.profile_id.as_str(),
            "Runtime profile endpoint is a remote host over plain http; requests are not encrypted"
        );
    }
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
        if profile.provider == RuntimeProviderId::OnnxRuntime {
            if let Some(refusal) =
                onnx_runtime_launch_refusal(state, command.tag.as_deref()).await?
            {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": refusal,
                    "ready": false
                }));
            }
        }
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
    let _torch_lifecycle_lease = if app_id == "torch" {
        Some(version_manager.torch_lifecycle_lease().await?)
    } else {
        None
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
    if app_id == "torch" {
        version_manager.verify_torch_identity(&tag).await?;
    }
    let version_dir = version_manager.version_path(&tag)?;
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

/// ONNX Runtime executes in-process with the library of the active installed
/// version, so a profile can only run when that library is available. Returns
/// the reason a launch is refused, or `None` when it may proceed.
async fn onnx_runtime_launch_refusal(
    state: &AppState,
    requested_tag: Option<&str>,
) -> pumas_library::Result<Option<String>> {
    let library = super::serving_onnx::active_onnx_runtime_library(state).await?;
    if let Some(requested) = requested_tag {
        if library.as_ref().map(|library| library.version_tag()) != Some(requested) {
            return Ok(Some(format!(
                "ONNX Runtime runs the active installed version; make {requested} the active \
                 ONNX Runtime version to use it"
            )));
        }
    }
    Ok(
        pumas_library::onnx_runtime::ensure_runtime_library_available(library.as_ref())
            .err()
            .map(|error| error.message),
    )
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
    use pumas_library::AppId;
    use tempfile::TempDir;

    async fn onnx_profile(state: &AppState) -> RuntimeProfileId {
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
        profile_id
    }

    /// Register an ONNX Runtime version manager with one installed, active
    /// version whose library is a fixture file. Nothing is downloaded.
    async fn install_active_onnx_runtime_fixture(state: &AppState, root: &std::path::Path) {
        let tag = "v1.24.2+cpu";
        let version_dir = root.join(AppId::OnnxRuntime.versions_dir_name()).join(tag);
        let library =
            version_dir.join(pumas_library::onnx_runtime_distribution::library_relative_path());
        std::fs::create_dir_all(library.parent().unwrap()).unwrap();
        std::fs::write(&library, b"installed library fixture").unwrap();
        pumas_library::metadata::MetadataManager::new(root)
            .update_installed_version(
                tag,
                pumas_library::metadata::InstalledVersionMetadata {
                    path: tag.into(),
                    installed_date: "2026-11-12T00:00:00Z".into(),
                    release_tag: tag.into(),
                    python_version: None,
                    git_commit: None,
                    release_date: None,
                    release_notes: None,
                    download_url: None,
                    size: None,
                    requirements_hash: None,
                    dependencies_installed: Some(true),
                },
                Some(AppId::OnnxRuntime),
            )
            .unwrap();
        let manager = pumas_app_manager::VersionManager::new(root, AppId::OnnxRuntime)
            .await
            .unwrap();
        assert!(manager.set_active_version(tag).await.unwrap());
        state
            .version_managers
            .write()
            .await
            .insert(AppId::OnnxRuntime.as_str().to_owned(), manager);
    }

    #[tokio::test]
    async fn launch_onnx_profile_without_an_installed_runtime_is_refused_with_a_reason() {
        if std::env::var_os("ORT_DYLIB_PATH").is_some() {
            return; // A developer override would satisfy the launch.
        }
        let temp_dir = TempDir::new().unwrap();
        let state = crate::handlers::test_support::build_test_app_state(temp_dir.path()).await;
        let profile_id = onnx_profile(&state).await;

        let response = launch_runtime_profile(
            &state,
            &serde_json::json!({ "profile_id": profile_id.as_str() }),
        )
        .await
        .unwrap();

        assert_eq!(response["success"], false);
        let error = response["error"].as_str().unwrap();
        assert!(
            error.contains("no ONNX Runtime version is installed and active"),
            "{error}"
        );
        assert!(
            error.contains("install a compatible ONNX Runtime version"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn launch_onnx_profile_uses_the_active_installed_runtime() {
        let temp_dir = TempDir::new().unwrap();
        let state = crate::handlers::test_support::build_test_app_state(temp_dir.path()).await;
        let profile_id = onnx_profile(&state).await;
        install_active_onnx_runtime_fixture(&state, temp_dir.path()).await;

        let other_version = launch_runtime_profile(
            &state,
            &serde_json::json!({ "profile_id": profile_id.as_str(), "tag": "v1.25.0+cpu" }),
        )
        .await
        .unwrap();
        assert_eq!(other_version["success"], false);
        assert!(other_version["error"]
            .as_str()
            .unwrap()
            .contains("make v1.25.0+cpu the active ONNX Runtime version"));

        let response = launch_runtime_profile(
            &state,
            &serde_json::json!({ "profile_id": profile_id.as_str() }),
        )
        .await
        .unwrap();

        assert_eq!(response["success"], true, "{response}");
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
