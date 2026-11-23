//! Managed Basalt serving and model readiness.

use std::time::Duration;

use pumas_library::{
    models::{
        ModelServeErrorCode, RuntimeDeviceSettings, RuntimeLifecycleState, RuntimeProviderId,
        ServeModelRequest, ServeModelResponse, ServedModelLoadState, ServedModelStatus,
    },
    runtime_profiles::{OwnedRuntimeProfileObservation, RuntimeProfileLaunchOverrides},
    serving::ServingLoadOperation,
};
use serde::Deserialize;
use serde_json::Value;

use super::serving::{
    decorate_serving_snapshot, effective_gateway_alias_from_config, non_critical_failure_response,
    serving_error,
};
use crate::{provider_clients::read_bounded, server::AppState};

#[derive(Deserialize)]
pub(super) struct BasaltHealth {
    service: String,
    status: String,
    model: String,
    loaded: bool,
    pub(super) max_context: u32,
    pub(super) images: bool,
}

impl BasaltHealth {
    fn matches_model(&self, model: &str) -> bool {
        self.service == "basalt" && self.status == "ok" && self.model == model
    }
}

pub(super) async fn read_health(
    state: &AppState,
    owned: &OwnedRuntimeProfileObservation,
) -> Option<BasaltHealth> {
    let url = format!(
        "{}/health",
        owned.endpoint_url.as_str().trim_end_matches('/')
    );
    let response = state
        .gateway_http_client
        .for_url(&url)
        .get(&url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .ok()?;
    if response.status() != reqwest::StatusCode::OK
        || !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|mime| mime.to_str().ok())
            .and_then(|mime| mime.split(';').next())
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
    {
        return None;
    }
    let bytes = read_bounded(response, 16 * 1024).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(super) async fn serve_basalt_model(
    state: &AppState,
    request: ServeModelRequest,
    operation: &ServingLoadOperation,
) -> pumas_library::Result<Value> {
    let profile = &request.config.profile_id;
    let refusal = |code, message: String| serving_error(code, message, &request);
    let Some(manager) = super::get_version_manager(state, "basalt").await else {
        return non_critical_failure_response(
            state,
            refusal(
                ModelServeErrorCode::MissingRuntime,
                "Basalt version manager is unavailable".into(),
            ),
        )
        .await;
    };
    let lease = manager.lifecycle_lease().await;
    let (tag, version_dir) =
        match super::resolve_profile_runtime_version(state, &manager, profile, None).await? {
            Ok(version) => version,
            Err(reason) => {
                let message = match reason {
                    super::ProfileRuntimeVersionRefusal::NoActiveVersion => {
                        "Install and activate a Basalt version first".into()
                    }
                    super::ProfileRuntimeVersionRefusal::NotInstalled(message) => message,
                };
                return non_critical_failure_response(
                    state,
                    refusal(ModelServeErrorCode::MissingRuntime, message),
                )
                .await;
            }
        };
    let launch = state
        .api
        .launch_runtime_profile_for_model_with_receipt(
            profile.clone(),
            &tag,
            &version_dir,
            Some(&request.model_id),
            Some(RuntimeProfileLaunchOverrides {
                device: Some(RuntimeDeviceSettings {
                    mode: request.config.device_mode,
                    device_id: request.config.device_id.clone(),
                    gpu_layers: None,
                    tensor_split: None,
                }),
                context_size: request.config.context_size,
            }),
        )
        .await;
    drop(lease);

    let owned = match launch {
        Ok(receipt) if receipt.response.success => receipt.observation,
        Ok(receipt) => {
            let message = receipt
                .response
                .error
                .unwrap_or_else(|| "Could not start the Basalt profile".into());
            return non_critical_failure_response(
                state,
                refusal(ModelServeErrorCode::ProviderLoadFailed, message),
            )
            .await;
        }
        Err(pumas_library::PumasError::DeviceNotAvailable { device }) => {
            return non_critical_failure_response(
                state,
                refusal(ModelServeErrorCode::DeviceUnavailable, device),
            )
            .await;
        }
        Err(pumas_library::PumasError::InvalidParams { message }) => {
            return non_critical_failure_response(
                state,
                refusal(ModelServeErrorCode::ProviderLoadFailed, message),
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let Some(owned) = owned else {
        return non_critical_failure_response(
            state,
            refusal(
                ModelServeErrorCode::ProviderLoadFailed,
                "Basalt launch did not establish process ownership".into(),
            ),
        )
        .await;
    };

    let ready = tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let current = state.api.observe_owned_runtime_profile(profile)?;
            if current.as_ref() != Some(&owned) || owned.state != RuntimeLifecycleState::Running {
                return Ok::<_, pumas_library::PumasError>(None);
            }
            if state
                .api
                .owned_runtime_profile_has_listener(profile, &owned)?
            {
                let health = read_health(state, &owned).await.filter(|health| {
                    health.matches_model(&request.model_id)
                        && health.loaded
                        && health.max_context > 0
                });
                if health.is_some()
                    && state
                        .api
                        .owned_runtime_profile_has_listener(profile, &owned)?
                {
                    return Ok(health);
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await;

    let health = match ready {
        Ok(Ok(Some(health))) => health,
        failure => {
            let stopped = state
                .api
                .stop_runtime_profile_if_generation(profile.clone(), owned.generation)
                .await;
            if let Err(error) = stopped {
                tracing::warn!(%error, "Could not finish cleanup after Basalt readiness failure");
            }
            if let Ok(Err(error)) = failure {
                return Err(error);
            }
            return non_critical_failure_response(state, refusal(ModelServeErrorCode::ProviderLoadFailed, "The owned Basalt process did not load the selected model. Check its runtime log.".into())).await;
        }
    };
    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(effective_gateway_alias_from_config(&request)),
        provider: RuntimeProviderId::Basalt,
        profile_id: profile.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: None,
        tensor_split: None,
        context_size: Some(health.max_context),
        keep_loaded: request.config.keep_loaded,
        endpoint_url: Some(owned.endpoint_url.clone()),
        memory_bytes: None,
        loaded_at: None,
        last_error: None,
    };
    let mut snapshot = match state
        .api
        .record_served_model_for_operation_and_owned_profile(operation, status.clone(), &owned)
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            state
                .api
                .stop_runtime_profile_if_generation(profile.clone(), owned.generation)
                .await?;
            return Err(error);
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

pub(super) async fn model_health(
    state: &AppState,
    status: &ServedModelStatus,
) -> Option<BasaltHealth> {
    let owned = state
        .api
        .observe_owned_runtime_profile(&status.profile_id)
        .ok()??;
    if status.endpoint_url.as_ref() != Some(&owned.endpoint_url)
        || !state
            .api
            .owned_runtime_profile_has_listener(&status.profile_id, &owned)
            .ok()?
    {
        return None;
    }
    let health = read_health(state, &owned).await?;
    if !health.matches_model(&status.model_id)
        || !health.loaded
        || health.max_context == 0
        || !state
            .api
            .owned_runtime_profile_has_listener(&status.profile_id, &owned)
            .ok()?
    {
        return None;
    }
    Some(health)
}

pub(super) async fn apply_inference_defaults(
    state: &AppState,
    status: &ServedModelStatus,
    body: &mut Value,
) -> pumas_library::Result<()> {
    let options = body
        .as_object_mut()
        .ok_or_else(|| pumas_library::PumasError::InvalidParams {
            message: "Basalt chat options must be a JSON object".into(),
        })?;
    let settings = state.api.get_inference_settings(&status.model_id).await?;
    merge_inference_defaults(options, settings);
    Ok(())
}

fn merge_inference_defaults(
    options: &mut serde_json::Map<String, Value>,
    settings: Vec<pumas_library::models::InferenceParamSchema>,
) {
    for entry in settings {
        if !entry.default.is_null() && !has_explicit_option(options, &entry.key) {
            options.insert(entry.key, entry.default);
        }
    }
}

fn has_explicit_option(options: &serde_json::Map<String, Value>, key: &str) -> bool {
    let explicit = |key| options.get(key).is_some_and(|value| !value.is_null());
    let explicit_nested = |parent, key| {
        options
            .get(parent)
            .and_then(|value| value.get(key))
            .is_some_and(|value| !value.is_null())
    };
    explicit(key)
        || match key {
            "max_tokens" => explicit("max_completion_tokens"),
            "reasoning_effort" => {
                [
                    "reasoning_budget",
                    "reasoning_budget_tokens",
                    "thinking_budget_tokens",
                ]
                .into_iter()
                .any(explicit)
                    || explicit_nested("reasoning", "effort")
                    || explicit_nested("reasoning", "enabled")
                    || explicit_nested("thinking", "budget_tokens")
                    || explicit_nested("chat_template_kwargs", "reasoning_effort")
                    || explicit_nested("chat_template_kwargs", "enable_thinking")
            }
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_fill_only_omitted_options_and_respect_openai_aliases() {
        let mut settings = pumas_library::models::basalt_inference_settings();
        for entry in &mut settings {
            entry.default = match entry.key.as_str() {
                "reasoning_effort" => json!("high"),
                "max_tokens" => json!(4096),
                "temperature" => json!(0.7),
                "top_p" => json!(0.8),
                _ => Value::Null,
            };
        }
        let mut request = json!({
            "reasoning": {"enabled": false},
            "max_completion_tokens": 5,
            "temperature": 0,
            "top_p": null
        });
        let options = request.as_object_mut().unwrap();
        merge_inference_defaults(options, settings.clone());
        assert_eq!(options["temperature"], 0);
        assert_eq!(options["top_p"], 0.8);
        assert!(!options.contains_key("reasoning_effort"));
        assert!(!options.contains_key("max_tokens"));
        assert!(!options.contains_key("seed"));

        let mut omitted = serde_json::Map::new();
        merge_inference_defaults(&mut omitted, settings);
        assert_eq!(omitted["reasoning_effort"], "high");
        assert_eq!(omitted["max_tokens"], 4096);
    }

    #[test]
    fn preserves_basalt_chat_parameters_outside_the_saved_settings_schema() {
        let mut options = json!({"reasoning_effort": 4096, "temperature": 2.5})
            .as_object()
            .unwrap()
            .clone();
        merge_inference_defaults(
            &mut options,
            pumas_library::models::basalt_inference_settings(),
        );
        assert_eq!(options["reasoning_effort"], 4096);
        assert_eq!(options["temperature"], 2.5);
    }
}
