use std::path::Path;

use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::models::{
    ModelRuntimeRoute, RuntimeProfileConfig, RuntimeProfileId, RuntimeProfilesConfigFile,
    RUNTIME_PROFILES_SCHEMA_VERSION,
};
use crate::{PumasError, Result};

pub(super) fn load_or_initialize_config(path: &Path) -> Result<RuntimeProfilesConfigFile> {
    let raw_config: Option<Value> = atomic_read_json(path)?;
    match raw_config {
        Some(raw_config) => {
            let (config, migrated) = migrate_runtime_profiles_config(raw_config)?;
            if migrated {
                atomic_write_json(path, &config, true)?;
            }
            Ok(config)
        }
        None => {
            let config = RuntimeProfilesConfigFile::default_seed();
            atomic_write_json(path, &config, true)?;
            Ok(config)
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacyRuntimeProfilesConfigFile {
    cursor: String,
    profiles: Vec<RuntimeProfileConfig>,
    #[serde(default)]
    routes: Vec<LegacyModelRuntimeRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_profile_id: Option<RuntimeProfileId>,
}

#[derive(Debug, Deserialize)]
struct LegacyModelRuntimeRoute {
    model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile_id: Option<RuntimeProfileId>,
    #[serde(default)]
    auto_load: bool,
}

fn migrate_runtime_profiles_config(raw_config: Value) -> Result<(RuntimeProfilesConfigFile, bool)> {
    let schema_version = raw_config
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    if schema_version >= u64::from(RUNTIME_PROFILES_SCHEMA_VERSION) {
        return serde_json::from_value(raw_config)
            .map(|config| (config, false))
            .map_err(Into::into);
    }

    let legacy: LegacyRuntimeProfilesConfigFile = serde_json::from_value(raw_config)?;
    let mut routes = Vec::new();
    let mut dropped_ambiguous_routes = Vec::new();
    for route in legacy.routes {
        let model_id = route.model_id;
        let Some(provider) = route.profile_id.as_ref().and_then(|profile_id| {
            legacy
                .profiles
                .iter()
                .find(|profile| &profile.profile_id == profile_id)
                .map(|profile| profile.provider)
        }) else {
            dropped_ambiguous_routes.push(model_id);
            continue;
        };
        routes.push(ModelRuntimeRoute {
            provider,
            model_id,
            profile_id: route.profile_id,
            auto_load: route.auto_load,
        });
    }
    if !dropped_ambiguous_routes.is_empty() {
        warn!(
            dropped_routes = ?dropped_ambiguous_routes,
            "dropped ambiguous legacy runtime profile routes during provider-scope migration"
        );
    }

    Ok((
        RuntimeProfilesConfigFile {
            schema_version: RUNTIME_PROFILES_SCHEMA_VERSION,
            cursor: legacy.cursor,
            profiles: legacy.profiles,
            routes,
            default_profile_id: legacy.default_profile_id,
        },
        true,
    ))
}

pub(super) fn validate_model_route(route: &ModelRuntimeRoute) -> Result<()> {
    if route.model_id.trim().is_empty() {
        return Err(PumasError::InvalidParams {
            message: "model_id is required".to_string(),
        });
    }
    Ok(())
}
