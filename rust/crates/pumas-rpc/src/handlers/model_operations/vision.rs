//! Read-only feature readiness for the existing llama.cpp vision chat adapter.
//! This is backend declaration checking, not model-byte or semantic attestation.
use super::super::openai_gateway;
use crate::server::AppState;
use pumas_library::models::{RuntimeProviderId, RuntimeProviderMode, ServedModelStatus};
use serde_json::Value;
use std::{sync::OnceLock, time::Duration};

const MAX_PROPS_BYTES: usize = 64 * 1024;

pub(in crate::handlers) async fn supported_profile(
    state: &AppState,
    served: &ServedModelStatus,
) -> bool {
    if served.provider != RuntimeProviderId::LlamaCpp {
        return false;
    }
    state
        .api
        .get_runtime_profiles_snapshot()
        .await
        .is_ok_and(|snapshot| {
            snapshot.snapshot.profiles.iter().any(|profile| {
                profile.profile_id == served.profile_id
                    && profile.provider == RuntimeProviderId::LlamaCpp
                    && profile.provider_mode == RuntimeProviderMode::LlamaCppDedicated
                    && profile.enabled
            })
        })
}

pub(in crate::handlers) async fn ready(state: &AppState, served: &ServedModelStatus) -> bool {
    if !supported_profile(state, served).await
        || openai_gateway::generation_session_stop(state, served)
            .await
            .is_err()
    {
        return false;
    }
    let Some(endpoint) = served.endpoint_url.as_ref() else {
        return false;
    };
    // The existing provider transport appends API paths to the endpoint base.
    // A query/fragment cannot safely participate in that same path contract.
    let Ok(base) = reqwest::Url::parse(endpoint.as_str()) else {
        return false;
    };
    if base.query().is_some() || base.fragment().is_some() {
        return false;
    }
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    let client = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .expect("vision declaration client")
    });
    // Initial scope is dedicated sessions. The explicit flag also prevents a
    // compatible backend from loading a model during this read-only query.
    let read = async {
        let mut response = client
            .get(format!("{}/props", endpoint.as_str().trim_end_matches('/')))
            .query(&[("model", served.model_id.as_str()), ("autoload", "false")])
            .send()
            .await
            .ok()?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > MAX_PROPS_BYTES as u64)
        {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|n| n > MAX_PROPS_BYTES)
            {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice::<Value>(&bytes).ok()
    };
    match tokio::time::timeout(Duration::from_secs(5), read).await {
        Ok(Some(value)) => {
            value["modalities"]["vision"].as_bool() == Some(true)
                && value["is_sleeping"].as_bool() == Some(false)
                && value["build_info"]
                    .as_str()
                    .is_some_and(|v| !v.is_empty() && v.len() <= 256)
                && supported_profile(state, served).await
                && openai_gateway::generation_session_stop(state, served)
                    .await
                    .is_ok()
        }
        _ => false,
    }
}
