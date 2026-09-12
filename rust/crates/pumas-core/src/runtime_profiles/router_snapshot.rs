//! Strict pinned llama.cpp router model response decoding. No basename guessing.
use super::super::{OwnedRuntimeProfileObservation, RuntimeProfileLaunchSpec};
use super::catalog::{model_path, sections};
use crate::models::*;
use crate::{PumasError, Result};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

pub(super) fn decode(
    value: Value,
    preset: &[u8],
    spec: &RuntimeProfileLaunchSpec,
    receipt: &OwnedRuntimeProfileObservation,
) -> Result<Vec<ServedModelStatus>> {
    let stanzas = sections(preset)?;
    let paths: BTreeMap<_, _> = stanzas
        .iter()
        .filter_map(|(id, body)| model_path(body).map(|path| (id.as_str(), path)))
        .collect();
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| error("Router snapshot has no data array"))?;
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for value in data {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("Router model id missing"))?;
        if !seen.insert(id) {
            return Err(error("Duplicate router model identity"));
        }
        let expected_path = paths
            .get(id)
            .ok_or_else(|| error("Router catalog contains an unknown model identity"))?;
        let status = value
            .get("status")
            .ok_or_else(|| error("Router model status missing"))?;
        let state = status
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| error("Router model state missing"))?;
        let args = status
            .get("args")
            .and_then(Value::as_array)
            .ok_or_else(|| error("Router model arguments missing"))?;
        let args: Vec<&str> = args
            .iter()
            .map(|v| v.as_str().ok_or_else(|| error("Invalid router argument")))
            .collect::<Result<_>>()?;
        let path = unique_argument(&args, &["--model", "-m"])?;
        // Unloaded entries may not yet have child argv; their emitted preset
        // still carries the authoritative path.
        let emitted = status
            .get("preset")
            .and_then(Value::as_str)
            .and_then(model_path);
        if path.or(emitted) != Some(*expected_path)
            || emitted.is_some_and(|value| value != *expected_path)
        {
            return Err(error("Router model path does not match owned preset"));
        }
        if !matches!(state, "loaded" | "loading" | "unloading" | "unloaded") {
            return Err(error("Unknown router model state"));
        }
        let failed = match status.get("failed") {
            None => false,
            Some(value) => value
                .as_bool()
                .ok_or_else(|| error("Invalid router failure flag"))?,
        };
        let context_size = unique_argument(&args, &["--ctx-size", "-c"])?
            .map(|value| {
                value
                    .parse::<u32>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| error("Invalid router context"))
            })
            .transpose()?
            .or(receipt.context_size);
        let load_state = if failed {
            ServedModelLoadState::Failed
        } else {
            match state {
                "loaded" => ServedModelLoadState::Loaded,
                "loading" => ServedModelLoadState::Loading,
                "unloading" => ServedModelLoadState::Unloading,
                "unloaded" => continue,
                _ => return Err(error("Unknown router model state")),
            }
        };
        rows.push(ServedModelStatus {
            model_id: id.into(),
            model_alias: Some(id.into()),
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: spec.profile_id.clone(),
            load_state,
            device_mode: RuntimeDeviceMode::Auto,
            device_id: None,
            gpu_layers: argument(&args, "--n-gpu-layers").and_then(|s| s.parse().ok()),
            tensor_split: None,
            context_size,
            keep_loaded: false,
            endpoint_url: Some(receipt.endpoint_url.clone()),
            memory_bytes: None,
            loaded_at: None,
            last_error: failed.then(|| {
                ModelServeError::non_critical(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "Router child reported failure",
                )
                .for_model(id)
                .for_profile(spec.profile_id.clone())
                .for_provider(RuntimeProviderId::LlamaCpp)
            }),
        });
    }
    if paths.keys().any(|id| !seen.contains(id)) {
        return Err(error("Router snapshot is missing owned catalog entries"));
    }
    Ok(rows)
}
fn argument<'a>(args: &[&'a str], key: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == key)
        .map(|pair| pair[1])
}
fn error(message: &str) -> PumasError {
    PumasError::Other(message.into())
}

fn unique_argument<'a>(args: &[&'a str], keys: &[&str]) -> Result<Option<&'a str>> {
    let mut found = None;
    for (index, arg) in args.iter().enumerate() {
        if keys.contains(arg) {
            if found.is_some() {
                return Err(error("Duplicate router identity or context argument"));
            }
            found = Some(
                *args
                    .get(index + 1)
                    .ok_or_else(|| error("Missing router argument value"))?,
            );
        }
    }
    Ok(found)
}
