//! Read-only detection of native decision models served by llama.cpp.
//!
//! llama.cpp reports `decisions` in a model's `architecture.output_modalities`
//! (from GGUF metadata, without loading it). That runtime fact decides whether
//! the model answers `/v1/systemone`; prompted classifiers do not get it.
use super::super::openai_gateway;
use crate::server::AppState;
use pumas_library::models::{RuntimeProviderId, ServedModelStatus};
use serde_json::Value;
use std::{sync::OnceLock, time::Duration};

const MAX_MODELS_BYTES: usize = 256 * 1024;

/// Library tasks under which decision models are published. Only these are
/// probed, so other models never pay for the extra request.
pub(super) fn candidate_task(task: Option<&str>) -> bool {
    matches!(
        task,
        Some(
            "text-classification"
                | "zero-shot-classification"
                | "image-text-to-text"
                | "text+image->text"
        )
    )
}

pub(super) async fn is_decision_model(state: &AppState, served: &ServedModelStatus) -> bool {
    if served.provider != RuntimeProviderId::LlamaCpp {
        return false;
    }
    let Some(endpoint) = served.endpoint_url.as_ref() else {
        return false;
    };
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
            .expect("decision declaration client")
    });
    let read = async {
        let mut response = client
            .get(format!(
                "{}/v1/models",
                endpoint.as_str().trim_end_matches('/')
            ))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len().checked_add(chunk.len())? > MAX_MODELS_BYTES {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice::<Value>(&bytes).ok()
    };
    let Ok(Some(models)) = tokio::time::timeout(Duration::from_secs(5), read).await else {
        return false;
    };
    let provider_id = openai_gateway::provider_request_model_id(served, &state.provider_registry);
    let entries = models["data"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    // A router lists every model; a dedicated server lists its one model.
    let entry = entries
        .iter()
        .find(|entry| entry["id"].as_str() == Some(provider_id.as_str()))
        .or(match entries {
            [only] => Some(only),
            _ => None,
        });
    entry.is_some_and(|entry| {
        entry["architecture"]["output_modalities"]
            .as_array()
            .is_some_and(|modalities| modalities.iter().any(|m| m == "decisions"))
    })
}
