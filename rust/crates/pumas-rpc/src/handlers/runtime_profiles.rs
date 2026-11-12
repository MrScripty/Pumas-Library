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
    if let Some(version) = command.profile.runtime_version.as_deref().filter(|_| {
        command.profile.management_mode == pumas_library::models::RuntimeManagementMode::Managed
    }) {
        ensure_runtime_version_installed(state, &command.profile, version).await?;
    }
    Ok(serde_json::to_value(
        state.api.upsert_runtime_profile(command.profile).await?,
    )?)
}

/// A profile may only pin a version its runtime has installed. Core validates
/// the tag's shape; the installed set is known here, beside the managers.
async fn ensure_runtime_version_installed(
    state: &AppState,
    profile: &RuntimeProfileConfig,
    version: &str,
) -> pumas_library::Result<()> {
    let Some(behavior) = state.provider_registry.get(profile.provider) else {
        return Ok(()); // Core rejects an unregistered provider with its reason.
    };
    let app_id = behavior.managed_runtime_app_id.as_str();
    let Some(version_manager) = super::get_version_manager(state, app_id).await else {
        return Err(PumasError::InvalidParams {
            message: format!(
                "runtime_version {version} cannot be checked: the {app_id} version manager is \
                 not available"
            ),
        });
    };
    let mut installed = version_manager.get_installed_versions().await?;
    installed.sort();
    if installed.iter().any(|tag| tag == version) {
        return Ok(());
    }
    Err(PumasError::InvalidParams {
        message: format!(
            "runtime_version {version} is not an installed {app_id} version; installed versions: \
             {}",
            if installed.is_empty() {
                "none".to_string()
            } else {
                installed.join(", ")
            }
        ),
    })
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
    let _torch_lifecycle_lease = if app_id == "torch" {
        Some(version_manager.torch_lifecycle_lease().await?)
    } else {
        None
    };
    let (tag, version_dir) = match super::resolve_profile_runtime_version(
        state,
        &version_manager,
        &command.profile_id,
        command.tag.as_deref(),
    )
    .await?
    {
        Ok(resolved) => resolved,
        Err(refusal) => {
            let error = match refusal {
                super::ProfileRuntimeVersionRefusal::NoActiveVersion => {
                    behavior.managed_runtime_no_active_version_message.clone()
                }
                super::ProfileRuntimeVersionRefusal::NotInstalled(reason) => reason,
            };
            return Ok(serde_json::json!({
                "success": false,
                "error": error,
                "ready": false
            }));
        }
    };
    if app_id == "torch" {
        version_manager.verify_torch_identity(&tag).await?;
    }
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

    fn llama_cpp_router_profile(profile_id: &str, version: Option<&str>) -> RuntimeProfileConfig {
        RuntimeProfileConfig {
            profile_id: RuntimeProfileId::parse(profile_id).unwrap(),
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::Managed,
            name: profile_id.to_string(),
            enabled: true,
            endpoint_url: None,
            port: None,
            device: RuntimeDeviceSettings::default(),
            scheduler: Default::default(),
            runtime_version: version.map(str::to_string),
        }
    }

    /// A llama.cpp manager whose installed versions are fixture directories:
    /// metadata plus a placeholder server file. Nothing is downloaded or run.
    async fn llama_cpp_manager_with_installed(
        state: &AppState,
        root: &std::path::Path,
        tags: &[&str],
    ) -> pumas_app_manager::VersionManager {
        use pumas_library::AppId;
        let metadata = pumas_library::metadata::MetadataManager::new(root);
        for tag in tags {
            let version_dir = root.join(AppId::LlamaCpp.versions_dir_name()).join(tag);
            std::fs::create_dir_all(version_dir.join("bin")).unwrap();
            std::fs::write(version_dir.join("bin/llama-server"), b"placeholder").unwrap();
            metadata
                .update_installed_version(
                    tag,
                    pumas_library::metadata::InstalledVersionMetadata {
                        path: (*tag).into(),
                        installed_date: "2026-11-12T00:00:00Z".into(),
                        release_tag: (*tag).into(),
                        python_version: None,
                        git_commit: None,
                        release_date: None,
                        release_notes: None,
                        download_url: None,
                        size: None,
                        requirements_hash: None,
                        dependencies_installed: Some(true),
                    },
                    Some(AppId::LlamaCpp),
                )
                .unwrap();
        }
        let manager = pumas_app_manager::VersionManager::new(root, AppId::LlamaCpp)
            .await
            .unwrap();
        state
            .version_managers
            .write()
            .await
            .insert(AppId::LlamaCpp.as_str().to_owned(), manager.clone());
        manager
    }

    #[tokio::test]
    async fn profile_runtime_version_must_be_installed_to_save_and_wins_over_active() {
        let temp_dir = TempDir::new().unwrap();
        let state = crate::handlers::test_support::build_test_app_state(temp_dir.path()).await;
        let manager =
            llama_cpp_manager_with_installed(&state, temp_dir.path(), &["b1000+cpu", "b2000+cpu"])
                .await;
        assert!(manager.set_active_version("b2000+cpu").await.unwrap());

        let refused = upsert_runtime_profile(
            &state,
            &serde_json::json!({
                "profile": llama_cpp_router_profile("pinned", Some("b3000+cpu"))
            }),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(
            refused.contains("not an installed llama-cpp version"),
            "{refused}"
        );
        assert!(refused.contains("b1000+cpu, b2000+cpu"), "{refused}");

        upsert_runtime_profile(
            &state,
            &serde_json::json!({
                "profile": llama_cpp_router_profile("pinned", Some("b1000+cpu"))
            }),
        )
        .await
        .unwrap();
        let profile_id = RuntimeProfileId::parse("pinned").unwrap();
        let (tag, version_dir) =
            crate::handlers::resolve_profile_runtime_version(&state, &manager, &profile_id, None)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(tag, "b1000+cpu", "the pin wins over the active version");
        assert!(version_dir.ends_with("llama-cpp-versions/b1000+cpu"));

        // An unpinned profile follows the active version.
        state
            .api
            .upsert_runtime_profile(llama_cpp_router_profile("follows", None))
            .await
            .unwrap();
        let (tag, _) = crate::handlers::resolve_profile_runtime_version(
            &state,
            &manager,
            &RuntimeProfileId::parse("follows").unwrap(),
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(tag, "b2000+cpu");
    }

    #[tokio::test]
    async fn launching_a_profile_pinned_to_a_removed_version_is_refused_with_the_reason() {
        let temp_dir = TempDir::new().unwrap();
        let state = crate::handlers::test_support::build_test_app_state(temp_dir.path()).await;
        llama_cpp_manager_with_installed(&state, temp_dir.path(), &["b2000+cpu"]).await;
        // Saved while installed elsewhere, or edited on disk; core checks only the shape.
        state
            .api
            .upsert_runtime_profile(llama_cpp_router_profile("stale-pin", Some("b1000+cpu")))
            .await
            .unwrap();

        let response =
            launch_runtime_profile(&state, &serde_json::json!({ "profile_id": "stale-pin" }))
                .await
                .unwrap();

        assert_eq!(response["success"], false);
        let error = response["error"].as_str().unwrap();
        assert!(
            error.contains("version b1000+cpu, which is not installed"),
            "{error}"
        );
        assert!(
            error.contains("change the profile's runtime version"),
            "{error}"
        );
    }

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
                runtime_version: None,
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
