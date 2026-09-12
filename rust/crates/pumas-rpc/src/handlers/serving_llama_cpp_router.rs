//! llama.cpp router serving adapter helpers.

use super::serving::{
    current_serving_snapshot, decorate_serving_snapshot, effective_gateway_alias_from_config,
    non_critical_failure_response, serving_error,
};
use super::serving_llama_cpp_shared::{
    active_llama_cpp_runtime, llama_cpp_runtime_support_error, provider_request_model_id,
};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeError, ModelServeErrorCode, RuntimeManagementMode, RuntimeProfileConfig,
    RuntimeProviderId, ServeModelRequest, ServeModelResponse, ServedModelLoadState,
    ServedModelStatus,
};
use pumas_library::runtime_profiles::{
    OwnedRuntimeProfileObservation, RuntimeProfileLaunchOverrides,
};
use serde_json::Value;
use std::time::Duration;
use tracing::warn;

pub(super) async fn serve_llama_cpp_router_model(
    state: &AppState,
    request: ServeModelRequest,
) -> pumas_library::Result<Value> {
    let profile = state
        .api
        .get_runtime_profiles_snapshot()
        .await?
        .snapshot
        .profiles
        .into_iter()
        .find(|profile| profile.profile_id == request.config.profile_id);
    let Some(profile) = profile else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ProfileNotFound,
                "selected llama.cpp router profile was not found",
                &request,
            ),
        )
        .await;
    };
    let endpoint = match state
        .api
        .resolve_model_runtime_profile_endpoint(
            RuntimeProviderId::LlamaCpp,
            &request.model_id,
            Some(request.config.profile_id.clone()),
        )
        .await
    {
        Ok(endpoint) => endpoint,
        Err(_) => {
            warn!("failed to resolve llama.cpp router serving endpoint");
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::EndpointUnavailable,
                    "selected llama.cpp router profile is not available",
                    &request,
                ),
            )
            .await;
        }
    };

    let Some((_tag, version_dir)) = active_llama_cpp_runtime(state, &request).await? else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::MissingRuntime,
                "llama.cpp runtime versions are not available",
                &request,
            ),
        )
        .await;
    };
    if let Some(error) = llama_cpp_runtime_support_error(&version_dir, &request) {
        return non_critical_failure_response(state, error).await;
    }

    let router_model_id = provider_request_model_id(&request, &state.provider_registry);
    let owned = match managed_router_session(state, &request, &profile, &endpoint).await {
        Ok(owned) => owned,
        Err(error) => return non_critical_failure_response(state, error).await,
    };
    let ownership_matches = || match &owned {
        Some(owned) => state
            .api
            .observe_owned_runtime_profile(&request.config.profile_id)
            .map(|current| current.as_ref() == Some(owned))
            .map_err(|_| "llama.cpp router ownership could not be observed".to_string()),
        None => Ok(true),
    };
    let owns_listener = || match &owned {
        Some(owned) => state
            .api
            .owned_runtime_profile_has_listener(&request.config.profile_id, owned)
            .map_err(|_| {
                "llama.cpp router listener ownership could not be established".to_string()
            }),
        None => Ok(true),
    };
    let preparation_deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let readiness = RouterReadiness {
        model_id: &router_model_id,
        deadline: preparation_deadline,
        require_loaded: false,
    };
    if let Err(error) = router_endpoint_after_launch(
        &state.llama_cpp_router_client,
        endpoint.as_str(),
        &request,
        readiness,
        ownership_matches,
        owns_listener,
    )
    .await
    {
        return non_critical_failure_response(state, error).await;
    }
    let mut operation = match &owned {
        Some(owned) => match state
            .api
            .begin_owned_router_model_operation(&request.config.profile_id, owned)
        {
            Ok(operation) => Some(operation),
            Err(error) => {
                return non_critical_failure_response(
                    state,
                    serving_error(
                        ModelServeErrorCode::ProviderLoadFailed,
                        router_operation_admission_message(&error),
                        &request,
                    ),
                )
                .await
            }
        },
        None => None,
    };
    if let Some(context) = request.config.context_size {
        let preparation = prepare_router_context(
            &state.llama_cpp_router_client,
            endpoint.as_str(),
            context,
            RouterReadiness {
                model_id: &router_model_id,
                deadline: preparation_deadline,
                require_loaded: false,
            },
            ownership_matches,
            owns_listener,
            || async {
                let operation = operation
                    .as_mut()
                    .ok_or("external llama.cpp router context cannot be changed")?;
                if owned
                    .as_ref()
                    .is_some_and(|owned| owned.context_size.is_some())
                {
                    return Err("llama.cpp router process has a fixed context override".into());
                }
                let snapshot = current_serving_snapshot(state).await
                    .map_err(|_| "llama.cpp router serving state could not be observed")?;
                if snapshot.served_models.iter().any(|model| model.profile_id == request.config.profile_id) {
                    return Err("Clear or stop the existing llama.cpp serving session before changing context size".into());
                }
                operation
                    .set_model_context(&router_model_id, context)
                    .await
                    .map_err(|_| {
                        "llama.cpp router model context could not be prepared".to_string()
                    })?;
                operation.mark_mutating().map_err(|_| "llama.cpp router ownership changed before reload")?;
                Ok(())
            },
        )
        .await;
        if let Err(message) = preparation {
            return non_critical_failure_response(
                state,
                serving_error(ModelServeErrorCode::ProviderLoadFailed, message, &request),
            )
            .await;
        }
    }
    if let Some(operation) = operation.as_mut() {
        if operation.mark_mutating().is_err() {
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "llama.cpp router ownership changed before model load",
                    &request,
                ),
            )
            .await;
        }
    }
    let load_deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    if let Err(error) = load_router_model(
        &state.llama_cpp_router_client,
        endpoint.as_str(),
        &request,
        RouterReadiness {
            model_id: &router_model_id,
            deadline: load_deadline,
            require_loaded: true,
        },
        ownership_matches,
        owns_listener,
    )
    .await
    {
        return non_critical_failure_response(state, error).await;
    }
    if let Some(context) = request.config.context_size {
        let proof = async {
            if !ownership_matches()? || !owns_listener()? {
                return Err(
                    "llama.cpp router ownership changed before context verification".to_string(),
                );
            }
            let catalog = state
                .llama_cpp_router_client
                .router_context_catalog(endpoint.as_str(), &router_model_id, false, load_deadline)
                .await?;
            if !ownership_matches()? || !owns_listener()? {
                return Err(
                    "llama.cpp router ownership changed during context verification".into(),
                );
            }
            if !catalog.selected_loaded || catalog.context_size != Some(context) {
                return Err(
                    "llama.cpp router loaded model context does not match the request".into(),
                );
            }
            state
                .llama_cpp_router_client
                .verify_router_runtime_context(
                    endpoint.as_str(),
                    &router_model_id,
                    context,
                    load_deadline,
                )
                .await?;
            if !ownership_matches()? || !owns_listener()? {
                return Err(
                    "llama.cpp router ownership changed during runtime context verification".into(),
                );
            }
            Ok(())
        }
        .await;
        if let Err(message) = proof {
            return non_critical_failure_response(
                state,
                serving_error(ModelServeErrorCode::ProviderLoadFailed, message, &request),
            )
            .await;
        }
    }
    let gateway_alias = effective_gateway_alias_from_config(&request);
    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(gateway_alias.clone()),
        provider: RuntimeProviderId::LlamaCpp,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: request.config.gpu_layers,
        tensor_split: request.config.tensor_split.clone(),
        context_size: request.config.context_size,
        keep_loaded: request.config.keep_loaded,
        endpoint_url: Some(endpoint),
        memory_bytes: None,
        loaded_at: None,
        last_error: None,
    };
    let publication = match &owned {
        Some(owned) => {
            state
                .api
                .record_served_model_for_owned_profile(status.clone(), owned)
                .await
        }
        None => state.api.record_served_model(status.clone()).await,
    };
    let mut snapshot = match publication {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "llama.cpp router ownership changed before serving publication",
                    &request,
                ),
            )
            .await
        }
    };
    if let Some(operation) = operation {
        operation.finish()?;
    }
    decorate_serving_snapshot(state, &mut snapshot);

    Ok(serde_json::to_value(ServeModelResponse {
        success: true,
        error: None,
        loaded: true,
        loaded_models_unchanged: false,
        status: Some(status),
        load_error: None,
        snapshot: Some(snapshot),
    })?)
}

fn router_operation_admission_message(error: &pumas_library::PumasError) -> &'static str {
    match error {
        pumas_library::PumasError::Other(message)
            if message
                == "Router model state is uncertain; explicitly stop and restart the profile" =>
        {
            "llama.cpp router model state is uncertain; explicitly stop and restart the profile"
        }
        pumas_library::PumasError::Other(message)
            if message == "Router model operation is busy" =>
        {
            "llama.cpp router model operation is busy"
        }
        _ => "llama.cpp router model operation ownership is unavailable",
    }
}

async fn prepare_router_context<F, Fut>(
    client: &crate::provider_clients::LlamaCppRouterClient,
    endpoint: &str,
    context: u32,
    readiness: RouterReadiness<'_>,
    mut ownership_matches: impl FnMut() -> Result<bool, String>,
    mut owns_listener: impl FnMut() -> Result<bool, String>,
    set_context: F,
) -> Result<(), String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let model_id = readiness.model_id;
    let deadline = readiness.deadline;
    if !ownership_matches()? || !owns_listener()? {
        return Err("llama.cpp router ownership changed before context preparation".into());
    }
    let catalog = client
        .router_context_catalog(endpoint, model_id, false, deadline)
        .await?;
    if !ownership_matches()? || !owns_listener()? {
        return Err("llama.cpp router ownership changed during context preparation".into());
    }
    if catalog.context_size == Some(context) {
        return Ok(());
    }
    if !catalog.all_unloaded {
        return Err("Unload all llama.cpp router models before changing model context size".into());
    }
    tokio::time::timeout_at(deadline, set_context())
        .await
        .map_err(|_| "llama.cpp router context preparation exceeded its deadline")??;
    if !ownership_matches()? || !owns_listener()? {
        return Err("llama.cpp router ownership changed before preset reload".into());
    }
    let catalog = client
        .router_context_catalog(endpoint, model_id, true, deadline)
        .await?;
    if !ownership_matches()? || !owns_listener()? {
        return Err("llama.cpp router ownership changed during preset reload".into());
    }
    if !catalog.all_unloaded || !catalog.selected_unloaded || catalog.context_size != Some(context)
    {
        return Err("llama.cpp router reloaded model context does not match the request".into());
    }
    Ok(())
}

struct RouterReadiness<'a> {
    model_id: &'a str,
    deadline: tokio::time::Instant,
    require_loaded: bool,
}

async fn load_router_model(
    client: &crate::provider_clients::LlamaCppRouterClient,
    endpoint: &str,
    request: &ServeModelRequest,
    readiness: RouterReadiness<'_>,
    mut ownership_matches: impl FnMut() -> Result<bool, String>,
    mut owns_listener: impl FnMut() -> Result<bool, String>,
) -> Result<(), ModelServeError> {
    let load = async {
        if !ownership_matches()? || !owns_listener()? {
            return Err("llama.cpp router ownership changed before model load".to_string());
        }
        client
            .load_model(endpoint, readiness.model_id, readiness.deadline)
            .await
    };
    if tokio::time::timeout_at(readiness.deadline, load)
        .await
        .map_or(true, |result| result.is_err())
    {
        return Err(serving_error(
            ModelServeErrorCode::ProviderLoadFailed,
            "llama.cpp router could not load the selected model",
            request,
        ));
    }
    router_endpoint_after_launch(
        client,
        endpoint,
        request,
        readiness,
        ownership_matches,
        owns_listener,
    )
    .await
}

async fn router_endpoint_after_launch(
    client: &crate::provider_clients::LlamaCppRouterClient,
    endpoint: &str,
    request: &ServeModelRequest,
    readiness: RouterReadiness<'_>,
    mut ownership_matches: impl FnMut() -> Result<bool, String>,
    mut owns_listener: impl FnMut() -> Result<bool, String>,
) -> Result<(), ModelServeError> {
    let deadline = readiness.deadline;
    let result = tokio::time::timeout_at(deadline, async {
        loop {
            if !ownership_matches()? {
                return Err(
                    "llama.cpp router owned process exited or changed during startup".to_string(),
                );
            }
            if owns_listener()? {
                let ready = client
                    .router_catalog_ready(
                        endpoint,
                        readiness.model_id,
                        readiness.require_loaded,
                        deadline,
                    )
                    .await?;
                if !ownership_matches()? || !owns_listener()? {
                    return Err(
                        "llama.cpp router owned process exited or changed during startup"
                            .to_string(),
                    );
                }
                if ready {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        Err(if readiness.require_loaded {
            "llama.cpp router selected model did not become loaded before the deadline"
        } else {
            "llama.cpp router profile started but its model endpoint is not reachable"
        }
        .into())
    });
    result
        .map_err(|message| serving_error(ModelServeErrorCode::ProviderLoadFailed, message, request))
}

pub(super) async fn unserve_llama_cpp_router_model(
    state: &AppState,
    request_model_id: &str,
    profile_id: &pumas_library::models::RuntimeProfileId,
    model_alias: &str,
) -> pumas_library::Result<Option<Value>> {
    let endpoint = match state
        .api
        .resolve_model_runtime_profile_endpoint(
            RuntimeProviderId::LlamaCpp,
            request_model_id,
            Some(profile_id.clone()),
        )
        .await
    {
        Ok(endpoint) => endpoint,
        Err(_) => {
            warn!("failed to resolve llama.cpp router unload endpoint");
            return Ok(Some(serde_json::to_value(
                pumas_library::models::UnserveModelResponse {
                    success: true,
                    error: Some("llama.cpp router endpoint is not available".to_string()),
                    unloaded: false,
                    snapshot: Some(current_serving_snapshot(state).await?),
                },
            )?));
        }
    };
    let owned = state.api.observe_owned_runtime_profile(profile_id)?;
    let managed = state
        .api
        .get_runtime_profiles_snapshot()
        .await?
        .snapshot
        .profiles
        .iter()
        .any(|profile| {
            &profile.profile_id == profile_id
                && profile.management_mode == RuntimeManagementMode::Managed
        });
    let mut operation = if managed {
        let owned = owned.as_ref().ok_or_else(|| {
            pumas_library::PumasError::Other(
                "llama.cpp router unload has no owned process identity".into(),
            )
        })?;
        if owned.endpoint_url != endpoint {
            return Err(pumas_library::PumasError::Other(
                "llama.cpp router unload endpoint differs from the owned listener".into(),
            ));
        }
        match state
            .api
            .begin_owned_router_model_operation(profile_id, owned)
        {
            Ok(operation) => Some(operation),
            Err(error) => {
                return Ok(Some(serde_json::to_value(
                    pumas_library::models::UnserveModelResponse {
                        success: true,
                        error: Some(router_operation_admission_message(&error).into()),
                        unloaded: false,
                        snapshot: Some(current_serving_snapshot(state).await?),
                    },
                )?))
            }
        }
    } else {
        None
    };
    if let Some(operation) = operation.as_mut() {
        operation.mark_mutating()?;
    }
    let router_model_id = request_model_id.trim();
    if state
        .llama_cpp_router_client
        .unload_model(endpoint.as_str(), router_model_id)
        .await
        .is_err()
    {
        warn!("llama.cpp router model unload failed");
        return Ok(Some(serde_json::to_value(
            pumas_library::models::UnserveModelResponse {
                success: true,
                error: Some("llama.cpp router could not unload the selected model".to_string()),
                unloaded: false,
                snapshot: Some(current_serving_snapshot(state).await?),
            },
        )?));
    }
    if let Some(owned) = owned.as_ref().filter(|_| managed) {
        if state
            .api
            .observe_owned_runtime_profile(profile_id)?
            .as_ref()
            != Some(owned)
            || !state
                .api
                .owned_runtime_profile_has_listener(profile_id, owned)?
        {
            return Err(pumas_library::PumasError::Other(
                "llama.cpp router ownership changed during unload".into(),
            ));
        }
    }
    let mut snapshot = match owned.as_ref().filter(|_| managed) {
        Some(owned) => {
            state
                .api
                .record_unserved_model_for_owned_profile(
                    request_model_id,
                    RuntimeProviderId::LlamaCpp,
                    profile_id,
                    model_alias,
                    owned,
                )
                .await?
        }
        None => {
            state
                .api
                .record_unserved_model(
                    request_model_id,
                    Some(RuntimeProviderId::LlamaCpp),
                    Some(profile_id),
                    Some(model_alias),
                )
                .await?
        }
    };
    if let Some(operation) = operation {
        operation.finish()?;
    }
    decorate_serving_snapshot(state, &mut snapshot);
    Ok(Some(serde_json::to_value(
        pumas_library::models::UnserveModelResponse {
            success: true,
            error: None,
            unloaded: true,
            snapshot: Some(snapshot),
        },
    )?))
}

async fn managed_router_session(
    state: &AppState,
    request: &ServeModelRequest,
    profile: &RuntimeProfileConfig,
    endpoint: &pumas_library::models::RuntimeEndpointUrl,
) -> Result<Option<OwnedRuntimeProfileObservation>, ModelServeError> {
    if profile.management_mode != RuntimeManagementMode::Managed {
        return Ok(None);
    }
    let failure =
        |message| serving_error(ModelServeErrorCode::ProviderLoadFailed, message, request);
    let current = state
        .api
        .observe_owned_runtime_profile(&request.config.profile_id)
        .map_err(|_| failure("llama.cpp router process ownership could not be observed"))?;
    let active_unowned = if current.is_none() {
        let snapshot = state
            .api
            .get_runtime_profiles_snapshot()
            .await
            .map_err(|_| failure("llama.cpp router profile status could not be observed"))?;
        snapshot.snapshot.statuses.iter().any(|status| {
            status.profile_id == request.config.profile_id
                && !matches!(
                    status.state,
                    pumas_library::models::RuntimeLifecycleState::Stopped
                        | pumas_library::models::RuntimeLifecycleState::Failed
                )
        })
    } else {
        false
    };
    select_router_session(current, active_unowned, request, endpoint, || {
        launch_llama_cpp_router_profile(state, request, profile)
    })
    .await
    .map(Some)
}

async fn select_router_session<F, Fut>(
    current: Option<OwnedRuntimeProfileObservation>,
    active_unowned: bool,
    request: &ServeModelRequest,
    endpoint: &pumas_library::models::RuntimeEndpointUrl,
    launch: F,
) -> Result<OwnedRuntimeProfileObservation, ModelServeError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<OwnedRuntimeProfileObservation, ModelServeError>>,
{
    let failure =
        |message| serving_error(ModelServeErrorCode::ProviderLoadFailed, message, request);
    let owned = match current {
        Some(owned)
            if owned.pid.is_none()
                && matches!(
                    owned.state,
                    pumas_library::models::RuntimeLifecycleState::Stopped
                        | pumas_library::models::RuntimeLifecycleState::Failed
                ) =>
        {
            launch().await?
        }
        Some(owned) => owned,
        None if active_unowned => {
            return Err(failure(
                "llama.cpp router has an active process without owned launch identity",
            ))
        }
        None => launch().await?,
    };
    if owned.state != pumas_library::models::RuntimeLifecycleState::Running
        || owned.pid.is_none()
        || &owned.endpoint_url != endpoint
    {
        return Err(failure(
            "llama.cpp router owned process is not running at the selected endpoint",
        ));
    }
    if owned.context_size.is_some()
        && request.config.context_size.is_some()
        && request.config.context_size != owned.context_size
    {
        return Err(failure(
            "Stop the llama.cpp router profile before changing its context size",
        ));
    }
    Ok(owned)
}

async fn launch_llama_cpp_router_profile(
    state: &AppState,
    request: &ServeModelRequest,
    profile: &RuntimeProfileConfig,
) -> Result<OwnedRuntimeProfileObservation, ModelServeError> {
    let failure =
        |message| serving_error(ModelServeErrorCode::ProviderLoadFailed, message, request);
    let runtime = active_llama_cpp_runtime(state, request)
        .await
        .map_err(|_| failure("llama.cpp router runtime could not be resolved"))?;
    let Some((tag, version_dir)) = runtime else {
        return Err(failure("llama.cpp runtime versions are not available"));
    };
    let receipt = state
        .api
        .launch_runtime_profile_for_model_with_receipt(
            request.config.profile_id.clone(),
            &tag,
            &version_dir,
            Some(&request.model_id),
            Some(llama_cpp_router_launch_overrides(request, profile)),
        )
        .await
        .map_err(|_| failure("llama.cpp router runtime could not be launched"))?;
    if !receipt.response.success {
        return Err(failure(
            "llama.cpp router profile did not start for the selected model",
        ));
    }
    receipt
        .observation
        .ok_or_else(|| failure("llama.cpp router launch has no owned process identity"))
}

fn llama_cpp_router_launch_overrides(
    _request: &ServeModelRequest,
    profile: &RuntimeProfileConfig,
) -> RuntimeProfileLaunchOverrides {
    RuntimeProfileLaunchOverrides {
        device: Some(profile.device.clone()),
        context_size: None,
    }
}

#[cfg(test)]
#[path = "serving_llama_cpp_router_tests.rs"]
mod tests;
