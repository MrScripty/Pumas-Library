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

/// The registered runtime providers, each with the runtime app a managed
/// profile launches and whether this server version-manages that app.
pub async fn get_runtime_providers(state: &AppState) -> crate::contract::RuntimeProvidersOutcome {
    let managers = state.version_managers.read().await;
    crate::contract::RuntimeProvidersOutcome::from_registry(&state.provider_registry, |app_id| {
        managers.contains_key(app_id)
    })
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
    // Held until the new process records its PID, so its version cannot be
    // removed between resolution and launch.
    let _lifecycle_lease = version_manager.lifecycle_lease().await;
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
    let response = state
        .api
        .launch_runtime_profile_for_model(
            command.profile_id.clone(),
            &tag,
            &version_dir,
            command.model_id.as_deref(),
        )
        .await?;
    if profile.provider == RuntimeProviderId::OnnxRuntime && response.success {
        if let Err(reason) = confirm_onnx_host_ready(state, &command.profile_id).await? {
            // The host could not load its library or execution provider;
            // stop it so a corrected profile can launch again.
            state.api.stop_runtime_profile(command.profile_id).await?;
            return Ok(serde_json::json!({
                "success": false,
                "error": reason,
                "ready": false
            }));
        }
    }
    Ok(serde_json::to_value(response)?)
}

/// The endpoint of an owned, running runtime profile process.
fn running_profile_endpoint(
    state: &AppState,
    profile_id: &RuntimeProfileId,
) -> pumas_library::Result<Option<String>> {
    Ok(state
        .api
        .observe_owned_runtime_profile(profile_id)?
        .filter(|owned| owned.state == pumas_library::models::RuntimeLifecycleState::Running)
        .map(|owned| owned.endpoint_url.as_str().to_string()))
}

/// Ask a just-launched ONNX host whether it loaded its runtime library and
/// execution provider. `Err` carries the host's reason.
async fn confirm_onnx_host_ready(
    state: &AppState,
    profile_id: &RuntimeProfileId,
) -> pumas_library::Result<Result<(), String>> {
    use pumas_library::onnx_runtime::host_protocol::{OnnxHostHealth, HEALTH_PATH};
    let Some(endpoint) = running_profile_endpoint(state, profile_id)? else {
        return Ok(Err(format!(
            "ONNX Runtime profile {} stopped right after launch; see its log",
            profile_id.as_str()
        )));
    };
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(ONNX_HOST_HEALTH_POLL)
        .build()
        .map_err(|error| PumasError::Other(format!("ONNX host health client: {error}")))?;
    let url = format!("{}{HEALTH_PATH}", endpoint.trim_end_matches('/'));
    // The host listens only after loading its library and execution provider,
    // which can take a while for GPU builds; poll until it answers.
    let deadline = tokio::time::Instant::now() + ONNX_HOST_HEALTH_TIMEOUT;
    let health = loop {
        let attempt = async {
            client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json::<OnnxHostHealth>()
                .await
        }
        .await;
        match attempt {
            Ok(health) => break Ok(health),
            Err(error) if tokio::time::Instant::now() >= deadline => break Err(error),
            Err(_) => {
                if running_profile_endpoint(state, profile_id)?.is_none() {
                    return Ok(Err(format!(
                        "ONNX Runtime profile {} exited while starting; see its log",
                        profile_id.as_str()
                    )));
                }
                tokio::time::sleep(ONNX_HOST_HEALTH_POLL).await;
            }
        }
    };
    Ok(match health {
        Ok(health) if health.ready => Ok(()),
        Ok(health) => Err(health
            .error
            .unwrap_or_else(|| "ONNX host is not ready".to_string())),
        Err(error) => Err(format!(
            "ONNX host for profile {} did not report its health: {error}",
            profile_id.as_str()
        )),
    })
}

/// How long a just-launched ONNX host may take to start answering its health
/// check. It answers only after its runtime initialized; this bounds startup,
/// not model loads.
const ONNX_HOST_HEALTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
/// Interval between health polls of a starting ONNX host.
const ONNX_HOST_HEALTH_POLL: std::time::Duration = std::time::Duration::from_millis(250);

/// The running ONNX host of an ONNX Runtime profile, launching the profile
/// (with its pinned or the active version) when it is not running. `Err`
/// carries the reason it cannot run.
pub(super) async fn running_onnx_host(
    state: &AppState,
    profile_id: &RuntimeProfileId,
) -> pumas_library::Result<Result<pumas_library::OnnxHostEndpoint, String>> {
    let endpoint = match running_profile_endpoint(state, profile_id)? {
        Some(endpoint) => endpoint,
        None => {
            let launched = launch_runtime_profile(
                state,
                &serde_json::json!({ "profile_id": profile_id.as_str() }),
            )
            .await?;
            if launched["success"] != true {
                return Ok(Err(launched["error"]
                    .as_str()
                    .unwrap_or("ONNX Runtime profile did not start")
                    .to_string()));
            }
            match running_profile_endpoint(state, profile_id)? {
                Some(endpoint) => endpoint,
                None => {
                    return Ok(Err(format!(
                        "ONNX Runtime profile {} stopped right after launch; see its log",
                        profile_id.as_str()
                    )))
                }
            }
        }
    };
    Ok(pumas_library::OnnxHostEndpoint::parse(&endpoint).map_err(|error| error.message))
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
    async fn runtime_providers_report_registry_facts_and_held_version_managers() {
        let temp_dir = TempDir::new().unwrap();
        let state = crate::handlers::test_support::build_test_app_state(temp_dir.path()).await;

        let before = serde_json::to_value(get_runtime_providers(&state).await).unwrap();
        assert_eq!(before["success"], true);
        let providers = before["providers"].as_array().unwrap();
        let pairs: Vec<_> = providers
            .iter()
            .map(|provider| {
                (
                    provider["provider"].as_str().unwrap(),
                    provider["app_id"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            [
                ("ollama", "ollama"),
                ("llama_cpp", "llama-cpp"),
                ("onnx_runtime", "onnx-runtime"),
                ("torch", "torch"),
            ]
        );
        assert!(providers
            .iter()
            .all(|provider| provider["version_managed"] == false));
        let onnx = &providers[2];
        assert_eq!(
            onnx["device_modes"],
            serde_json::json!(["auto", "cpu", "gpu"])
        );
        assert_eq!(onnx["provider_modes"], serde_json::json!(["onnx_serve"]));
        assert_eq!(onnx["supports_managed_profiles"], true);
        assert_eq!(onnx["supports_external_profiles"], false);

        llama_cpp_manager_with_installed(&state, temp_dir.path(), &[]).await;
        let after = serde_json::to_value(get_runtime_providers(&state).await).unwrap();
        let managed: Vec<_> = after["providers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|provider| provider["version_managed"] == true)
            .map(|provider| provider["app_id"].as_str().unwrap())
            .collect();
        assert_eq!(managed, ["llama-cpp"]);
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
    async fn launching_an_onnx_profile_without_an_installed_runtime_is_refused_with_the_reason() {
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

        // ONNX Runtime runs in a host process launched from an installed
        // version, so without the version manager there is nothing to launch.
        assert_eq!(response["success"], false);
        assert_eq!(
            response["error"],
            "Version manager not initialized for ONNX Runtime"
        );

        // With a manager but no installed version, the refusal says how to
        // install one.
        state.version_managers.write().await.insert(
            pumas_library::AppId::OnnxRuntime.as_str().to_owned(),
            pumas_app_manager::VersionManager::new(
                temp_dir.path(),
                pumas_library::AppId::OnnxRuntime,
            )
            .await
            .unwrap(),
        );
        let response = launch_runtime_profile(
            &state,
            &serde_json::json!({ "profile_id": profile_id.as_str() }),
        )
        .await
        .unwrap();
        assert_eq!(response["success"], false);
        assert!(response["error"]
            .as_str()
            .unwrap()
            .contains("install one from the ONNX Runtime versions"));

        let snapshot = state.api.get_runtime_profiles_snapshot().await.unwrap();
        let status = snapshot
            .snapshot
            .statuses
            .iter()
            .find(|status| status.profile_id == profile_id)
            .unwrap();
        assert_ne!(status.state, RuntimeLifecycleState::Running);
    }
}
