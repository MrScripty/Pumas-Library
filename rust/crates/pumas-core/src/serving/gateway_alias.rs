use crate::models::{
    ModelServeError, ModelServeErrorCode, RuntimeProfileId, RuntimeProviderId, ServeModelRequest,
    ServedModelStatus,
};

use super::{served_model_reserves_gateway_alias, ServingValidationContext};

const MAX_GATEWAY_ALIAS_LEN: usize = 128;

pub(super) fn validate_gateway_alias_contract(
    model_id: &str,
    profile_id: &RuntimeProfileId,
    provider: RuntimeProviderId,
    model_alias: Option<&str>,
) -> Vec<ModelServeError> {
    let Some(model_alias) = model_alias else {
        return Vec::new();
    };
    let alias = model_alias.trim();
    if alias.is_empty() {
        return Vec::new();
    }

    let mut message = None;
    if alias.len() > MAX_GATEWAY_ALIAS_LEN {
        message = Some(format!(
            "model_alias must be {MAX_GATEWAY_ALIAS_LEN} characters or fewer"
        ));
    } else if alias.starts_with('/') || alias.ends_with('/') || alias.contains("//") {
        message = Some("model_alias cannot begin or end with '/', or contain '//'".to_string());
    } else if alias
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        message = Some("model_alias cannot contain path traversal segments".to_string());
    } else if !alias.chars().all(is_allowed_gateway_alias_char) {
        message = Some(
            "model_alias may contain only lowercase ASCII letters, digits, '.', '_', '-', and '/'"
                .to_string(),
        );
    }

    message
        .map(|message| {
            vec![
                ModelServeError::non_critical(ModelServeErrorCode::InvalidRequest, message)
                    .for_model(model_id)
                    .for_profile(profile_id.clone())
                    .for_provider(provider),
            ]
        })
        .unwrap_or_default()
}

fn is_allowed_gateway_alias_char(character: char) -> bool {
    character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || matches!(character, '.' | '_' | '-' | '/')
}

pub fn effective_gateway_model_alias(request: &ServeModelRequest) -> String {
    request
        .config
        .model_alias
        .as_deref()
        .map(str::trim)
        .filter(|model_alias| !model_alias.is_empty())
        .unwrap_or_else(|| request.model_id.trim())
        .to_string()
}

pub(super) fn validate_gateway_alias_is_unique(
    model_id: &str,
    effective_alias: &str,
    request: &ServeModelRequest,
    context: &ServingValidationContext,
) -> Vec<ModelServeError> {
    let Some(effective_alias_key) = gateway_alias_key(effective_alias) else {
        return Vec::new();
    };

    context
        .served_models
        .iter()
        .find(|status| {
            served_model_reserves_gateway_alias(status)
                && gateway_alias_key(served_status_effective_gateway_alias(status)).as_deref()
                    == Some(effective_alias_key.as_str())
                && !same_requested_served_instance(
                    model_id,
                    effective_alias_key.as_str(),
                    request,
                    status,
                )
        })
        .map(|status| {
            vec![
                ModelServeError::non_critical(
                    ModelServeErrorCode::DuplicateModelAlias,
                    format!(
                        "gateway model alias '{effective_alias}' is already served by model '{}' on profile '{}'; choose a unique alias",
                        status.model_id,
                        status.profile_id.as_str()
                    ),
                )
                .for_model(model_id)
                .for_profile(request.config.profile_id.clone())
                .for_provider(request.config.provider),
            ]
        })
        .unwrap_or_default()
}

fn gateway_alias_key(alias: &str) -> Option<String> {
    let alias = alias.trim();
    if alias.is_empty() {
        return None;
    }

    let mut key = String::with_capacity(alias.len());
    let mut previous_was_separator = false;
    for character in alias.chars() {
        let normalized = character.to_ascii_lowercase();
        let is_separator = matches!(normalized, '.' | '_' | '-' | '/');
        if is_separator {
            if !previous_was_separator {
                key.push('-');
            }
            previous_was_separator = true;
        } else {
            key.push(normalized);
            previous_was_separator = false;
        }
    }

    Some(key.trim_matches('-').to_string()).filter(|key| !key.is_empty())
}

fn served_status_effective_gateway_alias(status: &ServedModelStatus) -> &str {
    status
        .model_alias
        .as_deref()
        .map(str::trim)
        .filter(|model_alias| !model_alias.is_empty())
        .unwrap_or(status.model_id.as_str())
}

fn same_requested_served_instance(
    model_id: &str,
    effective_alias_key: &str,
    request: &ServeModelRequest,
    status: &ServedModelStatus,
) -> bool {
    status.model_id == model_id
        && status.provider == request.config.provider
        && status.profile_id == request.config.profile_id
        && gateway_alias_key(served_status_effective_gateway_alias(status)).as_deref()
            == Some(effective_alias_key)
}
