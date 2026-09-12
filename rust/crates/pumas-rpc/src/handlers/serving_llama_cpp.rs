//! llama.cpp serving adapter used by the serving RPC boundary.

use super::serving::{
    current_serving_snapshot, decorate_serving_snapshot, effective_gateway_alias_from_config,
    non_critical_failure_response, serving_error,
};
use super::serving_llama_cpp_router::{
    serve_llama_cpp_router_model, unserve_llama_cpp_router_model,
};
use super::serving_llama_cpp_shared::{
    active_llama_cpp_runtime, llama_cpp_launch_overrides, llama_cpp_runtime_support_error,
};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeErrorCode, RuntimeProfileId, RuntimeProviderId, RuntimeProviderMode,
    ServeModelRequest, ServeModelResponse, ServedModelLoadState, ServedModelStatus,
    UnserveModelRequest, UnserveModelResponse,
};
use serde_json::Value;
use std::time::Duration;
use tracing::warn;

pub(super) async fn serve_llama_cpp_model(
    state: &AppState,
    request: ServeModelRequest,
    operation: &pumas_library::serving::ServingLoadOperation,
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
                "selected llama.cpp runtime profile was not found",
                &request,
            ),
        )
        .await;
    };

    if profile.provider_mode == RuntimeProviderMode::LlamaCppRouter {
        return serve_llama_cpp_router_model(state, request, operation).await;
    }

    if profile.provider_mode != RuntimeProviderMode::LlamaCppDedicated {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::UnsupportedProvider,
                "selected llama.cpp runtime profile mode is not supported for serving",
                &request,
            ),
        )
        .await;
    }

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
            warn!("failed to resolve llama.cpp serving endpoint");
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::EndpointUnavailable,
                    "selected llama.cpp runtime profile has no serving endpoint",
                    &request,
                ),
            )
            .await;
        }
    };

    let Some((tag, version_dir)) = active_llama_cpp_runtime(state, &request).await? else {
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
    let launch_response = state
        .api
        .launch_runtime_profile_for_model_with_receipt(
            request.config.profile_id.clone(),
            &tag,
            &version_dir,
            Some(&request.model_id),
            Some(llama_cpp_launch_overrides(&request)),
        )
        .await;
    let owned = match launch_response {
        Ok(receipt) if receipt.response.success => receipt.observation,
        Ok(receipt) => {
            let message = receipt.response.error.unwrap_or_else(|| {
                "llama.cpp runtime profile did not start for the selected model".to_string()
            });
            warn!("llama.cpp model serve launch failed");
            return non_critical_failure_response(
                state,
                serving_error(ModelServeErrorCode::ProviderLoadFailed, message, &request),
            )
            .await;
        }
        Err(_) => {
            warn!("llama.cpp model serve launch failed");
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::MissingRuntime,
                    "llama.cpp runtime could not be launched for the selected model",
                    &request,
                ),
            )
            .await;
        }
    };

    let readiness = async {
        let owned = owned
            .as_ref()
            .ok_or("llama.cpp launch has no owned process".to_string())?;
        if owned.state != pumas_library::models::RuntimeLifecycleState::Running
            || owned.pid.is_none()
        {
            return Err("llama.cpp owned process is not running".to_string());
        }
        if owned.endpoint_url != endpoint {
            return Err("llama.cpp owned process endpoint changed during launch".to_string());
        }
        let model_path = owned
            .model_path
            .as_ref()
            .and_then(|path| path.to_str())
            .ok_or("llama.cpp owned launch has no valid model path".to_string())?;
        wait_for_dedicated_readiness(
            &state.llama_cpp_router_client,
            owned.endpoint_url.as_str(),
            model_path,
            Duration::from_secs(60),
            || {
                state
                    .api
                    .observe_owned_runtime_profile(&request.config.profile_id)
                    .map(|current| current.as_ref() == Some(owned))
                    .map_err(|_| "llama.cpp process ownership could not be observed".to_string())
            },
            || {
                state
                    .api
                    .owned_runtime_profile_has_listener(&request.config.profile_id, owned)
                    .map_err(|_| {
                        "llama.cpp listener ownership could not be established".to_string()
                    })
            },
        )
        .await?;
        Ok(owned)
    }
    .await;
    let owned = match readiness {
        Ok(owned) => owned,
        Err(message) => {
            return non_critical_failure_response(
                state,
                serving_error(ModelServeErrorCode::ProviderLoadFailed, message, &request),
            )
            .await;
        }
    };

    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(effective_gateway_alias_from_config(&request)),
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
    let mut snapshot = match state
        .api
        .record_served_model_for_operation_and_owned_profile(operation, status.clone(), owned)
    {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "llama.cpp owned process exited or changed before serving publication",
                    &request,
                ),
            )
            .await;
        }
    };
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

async fn wait_for_dedicated_readiness(
    client: &crate::provider_clients::LlamaCppRouterClient,
    endpoint: &str,
    model_path: &str,
    budget: Duration,
    mut ownership_matches: impl FnMut() -> Result<bool, String>,
    mut owns_listener: impl FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + budget;
    tokio::time::timeout_at(deadline, async {
        loop {
            if !ownership_matches()? {
                return Err("llama.cpp owned process exited or changed during startup".into());
            }
            if !owns_listener()? {
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
            let ready = client
                .dedicated_model_ready(endpoint, model_path, deadline)
                .await?;
            // HTTP success is valid only while the same child generation remains owned.
            if !ownership_matches()? {
                return Err("llama.cpp owned process exited or changed during startup".into());
            }
            if ready {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await
    .unwrap_or_else(|_| Err("llama.cpp model readiness timed out".into()))
}

pub(super) async fn unserve_llama_cpp_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let provider_mode = state
        .api
        .get_runtime_profiles_snapshot()
        .await?
        .snapshot
        .profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
        .map(|profile| profile.provider_mode);

    if provider_mode == Some(RuntimeProviderMode::LlamaCppRouter) {
        if let Some(response) =
            unserve_llama_cpp_router_model(state, &request.model_id, &profile_id, &model_alias)
                .await?
        {
            return Ok(response);
        }
    }

    if provider_mode != Some(RuntimeProviderMode::LlamaCppDedicated) {
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: Some("selected llama.cpp runtime profile mode is not supported".to_string()),
            unloaded: false,
            snapshot: Some(current_serving_snapshot(state).await?),
        })?);
    }

    if state
        .api
        .stop_runtime_profile(profile_id.clone())
        .await
        .is_err()
    {
        warn!("llama.cpp serving unload failed");
        return Ok(serde_json::to_value(UnserveModelResponse {
            success: true,
            error: Some("llama.cpp runtime profile could not be stopped".to_string()),
            unloaded: false,
            snapshot: Some(current_serving_snapshot(state).await?),
        })?);
    }

    // The process owner removes only the stopped generation's served state.
    let snapshot = current_serving_snapshot(state).await?;
    Ok(serde_json::to_value(UnserveModelResponse {
        success: true,
        error: None,
        unloaded: true,
        snapshot: Some(snapshot),
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_clients::LlamaCppRouterClient;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    async fn check_readiness_case(
        health: &'static str,
        models: &'static str,
        change_owner_on_models: bool,
        slow_health: bool,
    ) -> Result<(), String> {
        let owner = Arc::new(AtomicBool::new(true));
        let model_owner = owner.clone();
        let app = axum::Router::new()
            .route(
                "/health",
                axum::routing::get(move || async move {
                    if slow_health {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    health
                }),
            )
            .route(
                "/v1/models",
                axum::routing::get(move || {
                    let owner = model_owner.clone();
                    async move {
                        if change_owner_on_models {
                            owner.store(false, Ordering::SeqCst);
                        }
                        models
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = stopped.await;
        });
        let check = async {
            let client = LlamaCppRouterClient::new(reqwest::Client::new());
            let result = wait_for_dedicated_readiness(
                &client,
                &endpoint,
                "/test.gguf",
                if slow_health {
                    Duration::from_millis(50)
                } else {
                    Duration::from_secs(1)
                },
                || Ok(owner.load(Ordering::SeqCst)),
                || Ok(true),
            )
            .await;
            stop.send(()).unwrap();
            result
        };
        let (served, result) = tokio::join!(server, check);
        served.unwrap();
        result
    }

    #[tokio::test]
    async fn dedicated_readiness_rechecks_owner_after_successful_http() {
        let health = r#"{"status":"ok"}"#;
        let models = r#"{"data":[{"id":"/test.gguf","meta":{}}]}"#;
        assert_eq!(
            check_readiness_case(health, models, false, false).await,
            Ok(())
        );
        let error = check_readiness_case(health, models, true, false)
            .await
            .unwrap_err();
        assert!(error.contains("exited or changed"));
    }

    #[tokio::test]
    async fn dedicated_readiness_rejects_malformed_and_wrong_model() {
        for (health, models) in [
            ("not json", "{}"),
            (r#"{"status":"loading"}"#, "{}"),
            (r#"{"status":"ok"}"#, "not json"),
            (
                r#"{"status":"ok"}"#,
                r#"{"data":[{"id":"other","meta":{}}]}"#,
            ),
        ] {
            assert!(check_readiness_case(health, models, false, false)
                .await
                .is_err());
        }
    }

    #[tokio::test]
    async fn dedicated_readiness_deadline_bounds_loading_and_http_reads() {
        let health = r#"{"status":"ok"}"#;
        let loading = r#"{"data":[{"id":"/test.gguf","meta":null}]}"#;
        assert!(check_readiness_case(health, loading, false, false)
            .await
            .unwrap_err()
            .contains("timed out"));
        assert!(check_readiness_case(health, loading, false, true)
            .await
            .unwrap_err()
            .contains("timed out"));
    }

    #[tokio::test]
    async fn dedicated_readiness_missing_owner_fails_before_http() {
        let client = LlamaCppRouterClient::new(reqwest::Client::new());
        let result = wait_for_dedicated_readiness(
            &client,
            "http://127.0.0.1:1",
            "/test.gguf",
            Duration::from_secs(1),
            || Ok(false),
            || Ok(true),
        )
        .await;
        assert!(result.unwrap_err().contains("exited or changed"));
    }

    #[tokio::test]
    async fn dedicated_readiness_never_probes_an_unattributed_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let client = LlamaCppRouterClient::new(reqwest::Client::new());
        for listener_result in [Ok(false), Err("listener unavailable".to_string())] {
            let check = wait_for_dedicated_readiness(
                &client,
                &endpoint,
                "/test.gguf",
                Duration::from_millis(30),
                || Ok(true),
                || listener_result.clone(),
            );
            tokio::select! {
                _ = listener.accept() => panic!("readiness contacted an unattributed listener"),
                result = check => {
                    let error = result.unwrap_err();
                    assert!(error.contains("timed out") || error == "listener unavailable");
                }
            }
        }
    }
}
