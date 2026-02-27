//! Dependency pin schema parsing, validation, canonicalization, and hashing.

use crate::error::{PumasError, Result};
use crate::model_library::task_signature::CANONICAL_MODALITY_TOKENS;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static EXACT_PIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^==[A-Za-z0-9]+[A-Za-z0-9._+\-]*$").expect("dependency pin regex must compile")
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct PythonPackagePin {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markers: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) struct PinPolicyRequiredPackage {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) struct PinPolicySpec {
    #[serde(default)]
    pub required_packages: Vec<PinPolicyRequiredPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) struct BindingModalityOverride {
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub output_modalities: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedDependencyPinSpec {
    pub python_packages: Vec<PythonPackagePin>,
    pub required_policy_packages: Vec<String>,
    pub binding_modality_overrides: HashMap<String, BindingModalityOverride>,
    pub canonical_json: String,
    pub profile_hash: String,
}

pub(crate) fn parse_and_canonicalize_profile_spec(
    spec_json: &str,
    environment_kind: &str,
    field_context: &str,
) -> Result<ParsedDependencyPinSpec> {
    let mut value: Value =
        serde_json::from_str(spec_json).map_err(|err| PumasError::Validation {
            field: field_context.to_string(),
            message: format!("invalid_dependency_pin: invalid JSON: {}", err),
        })?;
    let root = value
        .as_object_mut()
        .ok_or_else(|| PumasError::Validation {
            field: field_context.to_string(),
            message: "invalid_dependency_pin: profile spec must be a JSON object".to_string(),
        })?;

    let mut python_packages =
        parse_python_packages(root.get("python_packages"), field_context, environment_kind)?;
    python_packages.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));
    let package_names: HashSet<String> =
        python_packages.iter().map(|pin| pin.name.clone()).collect();

    if environment_kind.trim().to_lowercase().starts_with("python") && python_packages.is_empty() {
        return Err(PumasError::Validation {
            field: format!("{}.python_packages", field_context),
            message:
                "invalid_dependency_pin: python environments require at least one exact package pin"
                    .to_string(),
        });
    }

    let required_policy_packages =
        parse_required_policy_packages(root.get("pin_policy"), field_context)?;
    for package in &required_policy_packages {
        if !package_names.contains(package) {
            return Err(PumasError::Validation {
                field: format!("{}.pin_policy.required_packages", field_context),
                message: format!(
                    "invalid_dependency_pin: required package '{}' is missing from python_packages",
                    package
                ),
            });
        }
    }

    let binding_modality_overrides =
        parse_binding_modality_overrides(root.get("binding_modality_overrides"), field_context)?;

    root.insert(
        "python_packages".to_string(),
        serde_json::to_value(&python_packages)?,
    );
    if !required_policy_packages.is_empty() || root.get("pin_policy").is_some() {
        let policy = PinPolicySpec {
            required_packages: required_policy_packages
                .iter()
                .cloned()
                .map(|name| PinPolicyRequiredPackage { name })
                .collect(),
        };
        root.insert("pin_policy".to_string(), serde_json::to_value(&policy)?);
    }
    if !binding_modality_overrides.is_empty() || root.get("binding_modality_overrides").is_some() {
        root.insert(
            "binding_modality_overrides".to_string(),
            serde_json::to_value(&binding_modality_overrides)?,
        );
    }

    let canonical_value = canonicalize_value(&value);
    let canonical_json = serde_json::to_string(&canonical_value)?;
    let profile_hash = compute_canonical_profile_hash(&canonical_json);

    Ok(ParsedDependencyPinSpec {
        python_packages,
        required_policy_packages,
        binding_modality_overrides,
        canonical_json,
        profile_hash,
    })
}

fn parse_python_packages(
    value: Option<&Value>,
    field_context: &str,
    environment_kind: &str,
) -> Result<Vec<PythonPackagePin>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value.as_array().ok_or_else(|| PumasError::Validation {
        field: format!("{}.python_packages", field_context),
        message: "invalid_dependency_pin: must be an array".to_string(),
    })?;

    let mut pins = Vec::new();
    let mut seen_packages = HashMap::<String, String>::new();
    for (idx, item) in array.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| PumasError::Validation {
            field: format!("{}.python_packages[{}]", field_context, idx),
            message: "invalid_dependency_pin: package entry must be an object".to_string(),
        })?;

        let name_raw = obj
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| PumasError::Validation {
                field: format!("{}.python_packages[{}].name", field_context, idx),
                message: "invalid_dependency_pin: package name is required".to_string(),
            })?;
        let name = normalize_package_name(name_raw);

        let version = obj
            .get("version")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| PumasError::Validation {
                field: format!("{}.python_packages[{}].version", field_context, idx),
                message: "invalid_dependency_pin: exact version is required".to_string(),
            })?
            .to_string();

        if !is_exact_pin_version(&version) {
            return Err(PumasError::Validation {
                field: format!("{}.python_packages[{}].version", field_context, idx),
                message: format!(
                    "invalid_dependency_pin: '{}' must use exact '==' syntax",
                    version
                ),
            });
        }

        if let Some(prior_version) = seen_packages.get(&name) {
            if prior_version != &version {
                return Err(PumasError::Validation {
                    field: format!("{}.python_packages[{}].name", field_context, idx),
                    message: format!(
                        "invalid_dependency_pin: package '{}' appears with conflicting versions ('{}' and '{}')",
                        name, prior_version, version
                    ),
                });
            }
            continue;
        }
        seen_packages.insert(name.clone(), version.clone());

        pins.push(PythonPackagePin {
            name,
            version,
            index: normalize_optional_string(obj.get("index")),
            markers: normalize_optional_string(obj.get("markers")),
        });
    }

    if environment_kind.trim().to_lowercase().starts_with("python") && pins.is_empty() {
        return Err(PumasError::Validation {
            field: format!("{}.python_packages", field_context),
            message:
                "invalid_dependency_pin: python environments require at least one exact package pin"
                    .to_string(),
        });
    }

    Ok(pins)
}

fn parse_required_policy_packages(
    value: Option<&Value>,
    field_context: &str,
) -> Result<Vec<String>> {
    let Some(policy) = value else {
        return Ok(Vec::new());
    };
    let policy_obj = policy.as_object().ok_or_else(|| PumasError::Validation {
        field: format!("{}.pin_policy", field_context),
        message: "invalid_dependency_pin: pin_policy must be an object".to_string(),
    })?;
    let Some(required_raw) = policy_obj.get("required_packages") else {
        return Ok(Vec::new());
    };
    let required_array = required_raw
        .as_array()
        .ok_or_else(|| PumasError::Validation {
            field: format!("{}.pin_policy.required_packages", field_context),
            message: "invalid_dependency_pin: required_packages must be an array".to_string(),
        })?;

    let mut required = Vec::new();
    for (idx, item) in required_array.iter().enumerate() {
        let entry_obj = item.as_object().ok_or_else(|| PumasError::Validation {
            field: format!("{}.pin_policy.required_packages[{}]", field_context, idx),
            message: "invalid_dependency_pin: required package entry must be an object".to_string(),
        })?;
        let name = entry_obj
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(normalize_package_name)
            .ok_or_else(|| PumasError::Validation {
                field: format!(
                    "{}.pin_policy.required_packages[{}].name",
                    field_context, idx
                ),
                message: "invalid_dependency_pin: required package name is required".to_string(),
            })?;
        required.push(name);
    }

    required.sort();
    required.dedup();
    Ok(required)
}

fn parse_binding_modality_overrides(
    value: Option<&Value>,
    field_context: &str,
) -> Result<HashMap<String, BindingModalityOverride>> {
    let Some(raw_overrides) = value else {
        return Ok(HashMap::new());
    };
    let overrides_obj = raw_overrides
        .as_object()
        .ok_or_else(|| PumasError::Validation {
            field: format!("{}.binding_modality_overrides", field_context),
            message: "invalid_dependency_pin: binding_modality_overrides must be an object"
                .to_string(),
        })?;

    let mut overrides = HashMap::new();
    for (binding_id, entry) in overrides_obj {
        let binding_id = binding_id.trim();
        if binding_id.is_empty() {
            return Err(PumasError::Validation {
                field: format!("{}.binding_modality_overrides", field_context),
                message: "invalid_dependency_pin: binding id keys must be non-empty".to_string(),
            });
        }
        let entry_obj = entry.as_object().ok_or_else(|| PumasError::Validation {
            field: format!(
                "{}.binding_modality_overrides.{}",
                field_context, binding_id
            ),
            message: "invalid_dependency_pin: override entry must be an object".to_string(),
        })?;

        let input_modalities = parse_modalities_array(
            entry_obj.get("input_modalities"),
            &format!(
                "{}.binding_modality_overrides.{}.input_modalities",
                field_context, binding_id
            ),
        )?;
        let output_modalities = parse_modalities_array(
            entry_obj.get("output_modalities"),
            &format!(
                "{}.binding_modality_overrides.{}.output_modalities",
                field_context, binding_id
            ),
        )?;

        overrides.insert(
            binding_id.to_string(),
            BindingModalityOverride {
                input_modalities,
                output_modalities,
            },
        );
    }

    Ok(overrides)
}

fn parse_modalities_array(value: Option<&Value>, field: &str) -> Result<Vec<String>> {
    let Some(raw) = value else {
        return Ok(Vec::new());
    };
    let array = raw.as_array().ok_or_else(|| PumasError::Validation {
        field: field.to_string(),
        message: "invalid_dependency_pin: modalities must be an array of strings".to_string(),
    })?;

    let mut modalities = Vec::new();
    for (idx, modality) in array.iter().enumerate() {
        let normalized = modality
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_lowercase)
            .ok_or_else(|| PumasError::Validation {
                field: format!("{}[{}]", field, idx),
                message: "invalid_dependency_pin: modality token must be a non-empty string"
                    .to_string(),
            })?;
        if !CANONICAL_MODALITY_TOKENS.contains(&normalized.as_str()) {
            return Err(PumasError::Validation {
                field: format!("{}[{}]", field, idx),
                message: format!(
                    "invalid_dependency_pin: modality '{}' is not canonical",
                    normalized
                ),
            });
        }
        modalities.push(normalized);
    }
    Ok(modalities)
}

fn canonicalize_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let mut normalized = Map::new();
            for key in keys {
                if let Some(v) = map.get(&key) {
                    normalized.insert(key, canonicalize_value(v));
                }
            }
            Value::Object(normalized)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize_value).collect()),
        _ => value.clone(),
    }
}

fn normalize_optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
}

pub(crate) fn normalize_package_name(name: &str) -> String {
    name.trim().to_lowercase()
}

pub(crate) fn is_exact_pin_version(version: &str) -> bool {
    EXACT_PIN_RE.is_match(version.trim())
}

pub(crate) fn compute_canonical_profile_hash(canonical_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_profile_normalizes_and_hashes_stably() {
        let raw = serde_json::json!({
            "pin_policy": {
                "required_packages": [
                    {"name": "XFORMERS"}
                ]
            },
            "python_packages": [
                {"name": "XFORMERS", "version": "==0.0.30"},
                {"name": "Torch", "version": "==2.5.1+cu121"}
            ]
        })
        .to_string();
        let parsed = parse_and_canonicalize_profile_spec(&raw, "python-venv", "ctx").unwrap();
        assert_eq!(parsed.python_packages[0].name, "torch");
        assert_eq!(parsed.python_packages[1].name, "xformers");
        assert_eq!(parsed.required_policy_packages, vec!["xformers"]);
        assert_eq!(parsed.profile_hash.len(), 64);
    }

    #[test]
    fn parse_profile_rejects_non_exact_pin() {
        let raw = serde_json::json!({
            "python_packages": [
                {"name": "torch", "version": ">=2.5.1"}
            ]
        })
        .to_string();
        let err = parse_and_canonicalize_profile_spec(&raw, "python-venv", "ctx").unwrap_err();
        assert!(matches!(err, PumasError::Validation { .. }));
        assert!(err.to_string().contains("invalid_dependency_pin"));
    }

    #[test]
    fn parse_profile_rejects_unknown_modality_override_token() {
        let raw = serde_json::json!({
            "python_packages": [
                {"name": "torch", "version": "==2.5.1"}
            ],
            "binding_modality_overrides": {
                "b1": {
                    "input_modalities": ["foo"],
                    "output_modalities": ["text"]
                }
            }
        })
        .to_string();
        let err = parse_and_canonicalize_profile_spec(&raw, "python-venv", "ctx").unwrap_err();
        assert!(matches!(err, PumasError::Validation { .. }));
        assert!(err.to_string().contains("invalid_dependency_pin"));
    }
}
