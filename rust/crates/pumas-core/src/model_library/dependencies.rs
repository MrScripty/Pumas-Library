//! Model-level dependency requirement resolution and validation.
//!
//! This module provides deterministic dependency requirement resolution
//! using SQLite dependency tables.

use crate::error::{PumasError, Result};
use crate::model_library::dependency_pins::{
    parse_and_canonicalize_profile_spec, ParsedDependencyPinSpec,
};
use crate::model_library::library::ModelLibrary;
use crate::model_library::normalize_task_signature;
use crate::models::ModelMetadata;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub const DEPENDENCY_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DependencyValidationState {
    Resolved,
    UnknownProfile,
    InvalidProfile,
    ProfileConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DependencyValidationErrorScope {
    TopLevel,
    Binding,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DependencyValidationError {
    pub code: String,
    pub scope: DependencyValidationErrorScope,
    pub binding_id: Option<String>,
    pub field: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyRequirement {
    pub kind: String,
    pub name: String,
    pub exact_pin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_index_urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python_requires: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platform_constraints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hashes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyBindingRequirements {
    pub binding_id: String,
    pub profile_id: String,
    pub profile_version: i64,
    pub profile_hash: Option<String>,
    pub backend_key: Option<String>,
    pub platform_selector: Option<String>,
    pub environment_kind: Option<String>,
    pub env_id: Option<String>,
    pub validation_state: DependencyValidationState,
    pub validation_errors: Vec<DependencyValidationError>,
    pub requirements: Vec<ModelDependencyRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyRequirementsResolution {
    pub model_id: String,
    pub platform_key: String,
    pub backend_key: Option<String>,
    pub dependency_contract_version: u32,
    pub validation_state: DependencyValidationState,
    pub validation_errors: Vec<DependencyValidationError>,
    pub bindings: Vec<ModelDependencyBindingRequirements>,
}

/// Per-binding required dependency pin with requirement provenance.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyRequiredPin {
    pub name: String,
    pub reasons: Vec<String>,
}

/// Per-binding audit finding for dependency pin compliance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DependencyPinAuditBindingIssue {
    pub model_id: String,
    pub binding_id: String,
    pub profile_id: String,
    pub profile_version: i64,
    pub binding_kind: String,
    pub backend_key: Option<String>,
    pub error_code: String,
    pub message: Option<String>,
    pub missing_pins: Vec<String>,
    pub required_pins: Vec<ModelDependencyRequiredPin>,
}

/// Per-profile audit rollup for missing required pins.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DependencyPinAuditProfileIssue {
    pub profile_id: String,
    pub profile_version: i64,
    pub missing_pins: Vec<String>,
    pub suggested_backfill_pins: Vec<String>,
    pub affected_binding_ids: Vec<String>,
}

/// Report of dependency pin compliance across active bindings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DependencyPinAuditReport {
    pub generated_at: String,
    pub total_models_scanned: u32,
    pub total_bindings_scanned: u32,
    pub issue_count: u32,
    pub binding_issues: Vec<DependencyPinAuditBindingIssue>,
    pub profile_issues: Vec<DependencyPinAuditProfileIssue>,
}

impl ModelLibrary {
    /// Resolve deterministic dependency requirements for a model/context.
    pub async fn resolve_model_dependency_requirements(
        &self,
        model_id: &str,
        platform_context: &str,
        backend_key: Option<&str>,
    ) -> Result<ModelDependencyRequirementsResolution> {
        ensure_model_exists(self, model_id)?;

        let platform_key = normalize_platform_key(platform_context);
        let requested_backend_key = normalize_optional_token(backend_key);
        let model_metadata = self.get_effective_metadata(model_id)?;

        let binding_rows = self
            .index()
            .list_active_model_dependency_bindings(model_id, requested_backend_key.as_deref())?
            .into_iter()
            .filter(|binding| {
                platform_selector_matches(binding.platform_selector.as_deref(), &platform_key)
            })
            .collect::<Vec<_>>();

        if binding_rows.is_empty() {
            let has_declared_refs = load_declared_binding_refs(self, model_id)?;
            if has_declared_refs {
                let error = top_level_error(
                    "declared_bindings_unresolved",
                    Some("dependency_bindings"),
                    "Model declares dependency bindings, but no active bindings resolved for the requested context",
                );
                return Ok(ModelDependencyRequirementsResolution {
                    model_id: model_id.to_string(),
                    platform_key,
                    backend_key: requested_backend_key,
                    dependency_contract_version: DEPENDENCY_CONTRACT_VERSION,
                    validation_state: DependencyValidationState::UnknownProfile,
                    validation_errors: vec![error],
                    bindings: Vec::new(),
                });
            }

            return Ok(ModelDependencyRequirementsResolution {
                model_id: model_id.to_string(),
                platform_key,
                backend_key: requested_backend_key,
                dependency_contract_version: DEPENDENCY_CONTRACT_VERSION,
                validation_state: DependencyValidationState::Resolved,
                validation_errors: Vec::new(),
                bindings: Vec::new(),
            });
        }

        let mut bindings = Vec::with_capacity(binding_rows.len());
        for row in binding_rows {
            let normalized_backend = normalize_optional_token(
                row.backend_key
                    .as_deref()
                    .or(requested_backend_key.as_deref()),
            );
            let normalized_selector = normalize_optional_token(row.platform_selector.as_deref());
            let normalized_environment_kind =
                normalize_optional_token(row.environment_kind.as_deref());
            let mut binding = ModelDependencyBindingRequirements {
                binding_id: row.binding_id.clone(),
                profile_id: row.profile_id.clone(),
                profile_version: row.profile_version,
                profile_hash: row.profile_hash.clone(),
                backend_key: normalized_backend,
                platform_selector: normalized_selector,
                environment_kind: normalized_environment_kind,
                env_id: None,
                validation_state: DependencyValidationState::Resolved,
                validation_errors: Vec::new(),
                requirements: Vec::new(),
            };

            let Some(spec_json) = row.spec_json.as_deref() else {
                binding.validation_state = DependencyValidationState::UnknownProfile;
                push_binding_error(
                    &mut binding,
                    "unknown_profile",
                    Some("spec_json"),
                    "Dependency profile spec_json is missing for binding",
                );
                bindings.push(binding);
                continue;
            };

            if binding.profile_hash.is_none() {
                binding.validation_state = DependencyValidationState::UnknownProfile;
                push_binding_error(
                    &mut binding,
                    "unknown_profile",
                    Some("profile_hash"),
                    "Dependency profile hash is missing for binding",
                );
                bindings.push(binding);
                continue;
            }

            if binding.environment_kind.is_none() {
                binding.validation_state = DependencyValidationState::UnknownProfile;
                push_binding_error(
                    &mut binding,
                    "unknown_profile",
                    Some("environment_kind"),
                    "Dependency profile environment_kind is missing for binding",
                );
                bindings.push(binding);
                continue;
            }

            let field_context = format!(
                "dependency_profiles.{}:{}",
                row.profile_id, row.profile_version
            );
            let parsed = match parse_and_canonicalize_profile_spec(
                spec_json,
                binding.environment_kind.as_deref().unwrap_or("unknown"),
                &field_context,
            ) {
                Ok(parsed) => parsed,
                Err(err) => {
                    binding.validation_state = DependencyValidationState::InvalidProfile;
                    push_binding_error(
                        &mut binding,
                        "invalid_profile",
                        Some("spec_json"),
                        &err.to_string(),
                    );
                    bindings.push(binding);
                    continue;
                }
            };

            let requirements = build_requirements(&parsed);
            if let Err(err) = validate_requirement_duplicates(&requirements) {
                binding.validation_state = DependencyValidationState::InvalidProfile;
                push_binding_error(&mut binding, "invalid_profile", Some("requirements"), &err);
                bindings.push(binding);
                continue;
            }

            let pin_eval = evaluate_binding_pin_requirements(
                &row.binding_id,
                &row.binding_kind,
                binding.backend_key.as_deref(),
                &row.profile_id,
                row.profile_version,
                binding.environment_kind.as_deref().unwrap_or("unknown"),
                spec_json,
                model_metadata.as_ref(),
            );

            if let Some(code) = pin_eval.error_code {
                binding.validation_state = DependencyValidationState::InvalidProfile;
                push_binding_error(
                    &mut binding,
                    &code,
                    Some("requirements"),
                    pin_eval
                        .message
                        .as_deref()
                        .unwrap_or("Dependency profile requirements are invalid"),
                );
            }

            binding.requirements = requirements;
            if binding.validation_state == DependencyValidationState::Resolved {
                binding.env_id = Some(build_env_id(
                    binding.environment_kind.as_deref().unwrap_or("unknown"),
                    &binding.profile_id,
                    binding.profile_version,
                    binding.profile_hash.as_deref().unwrap_or("unknown"),
                    &platform_key,
                    binding.backend_key.as_deref(),
                ));
            }

            bindings.push(binding);
        }

        mark_profile_conflicts(&mut bindings, &platform_key);

        let mut top_level_errors = collect_validation_errors_union(&bindings);
        sort_validation_errors(&mut top_level_errors);

        let validation_state = aggregate_validation_state(&bindings);

        Ok(ModelDependencyRequirementsResolution {
            model_id: model_id.to_string(),
            platform_key,
            backend_key: requested_backend_key,
            dependency_contract_version: DEPENDENCY_CONTRACT_VERSION,
            validation_state,
            validation_errors: top_level_errors,
            bindings,
        })
    }

    /// Audit dependency pin compliance for all active bindings.
    ///
    /// Returns deterministic findings for unpinned dependency profiles and
    /// modality-resolution ambiguity before/after enforcement rollout.
    pub async fn audit_dependency_pin_compliance(&self) -> Result<DependencyPinAuditReport> {
        let models = self.list_models().await?;
        let mut binding_issues = Vec::new();
        let mut profile_issues: BTreeMap<(String, i64), DependencyPinAuditProfileIssue> =
            BTreeMap::new();
        let mut total_bindings_scanned = 0_u32;

        for model in &models {
            let model_metadata = self.get_effective_metadata(&model.id)?;
            let bindings = self
                .index()
                .list_active_model_dependency_bindings(&model.id, None)?;
            for binding in bindings {
                total_bindings_scanned += 1;
                let Some(spec_json) = binding.spec_json.as_deref() else {
                    continue;
                };
                let environment_kind = binding
                    .environment_kind
                    .as_deref()
                    .unwrap_or("unknown")
                    .to_string();

                let pin_eval = evaluate_binding_pin_requirements(
                    &binding.binding_id,
                    &binding.binding_kind,
                    binding.backend_key.as_deref(),
                    &binding.profile_id,
                    binding.profile_version,
                    &environment_kind,
                    spec_json,
                    model_metadata.as_ref(),
                );
                let Some(error_code) = pin_eval.error_code.clone() else {
                    continue;
                };

                binding_issues.push(DependencyPinAuditBindingIssue {
                    model_id: model.id.clone(),
                    binding_id: binding.binding_id.clone(),
                    profile_id: binding.profile_id.clone(),
                    profile_version: binding.profile_version,
                    binding_kind: binding.binding_kind.clone(),
                    backend_key: binding.backend_key.clone(),
                    error_code: error_code.clone(),
                    message: pin_eval.message.clone(),
                    missing_pins: pin_eval.missing_pins.clone(),
                    required_pins: pin_eval.required_pins.clone(),
                });

                if error_code == PIN_ERROR_UNPINNED_DEPENDENCY && !pin_eval.missing_pins.is_empty()
                {
                    let key = (binding.profile_id.clone(), binding.profile_version);
                    let issue = profile_issues.entry(key).or_insert_with(|| {
                        DependencyPinAuditProfileIssue {
                            profile_id: binding.profile_id.clone(),
                            profile_version: binding.profile_version,
                            missing_pins: Vec::new(),
                            suggested_backfill_pins: Vec::new(),
                            affected_binding_ids: Vec::new(),
                        }
                    });
                    for pin in &pin_eval.missing_pins {
                        if !issue.missing_pins.contains(pin) {
                            issue.missing_pins.push(pin.clone());
                        }
                        let suggested = format!("{}==<pin-me>", pin);
                        if !issue.suggested_backfill_pins.contains(&suggested) {
                            issue.suggested_backfill_pins.push(suggested);
                        }
                    }
                    if !issue.affected_binding_ids.contains(&binding.binding_id) {
                        issue.affected_binding_ids.push(binding.binding_id.clone());
                    }
                }
            }
        }

        binding_issues.sort_by(|a, b| {
            a.model_id
                .cmp(&b.model_id)
                .then_with(|| a.binding_id.cmp(&b.binding_id))
        });

        let mut profile_issue_rows = profile_issues.into_values().collect::<Vec<_>>();
        for issue in &mut profile_issue_rows {
            issue.missing_pins.sort();
            issue.suggested_backfill_pins.sort();
            issue.affected_binding_ids.sort();
        }
        profile_issue_rows.sort_by(|a, b| {
            a.profile_id
                .cmp(&b.profile_id)
                .then_with(|| a.profile_version.cmp(&b.profile_version))
        });

        Ok(DependencyPinAuditReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_models_scanned: models.len() as u32,
            total_bindings_scanned,
            issue_count: binding_issues.len() as u32,
            binding_issues,
            profile_issues: profile_issue_rows,
        })
    }
}

const PIN_REASON_BACKEND_REQUIRED: &str = "backend_required";
const PIN_REASON_MODALITY_REQUIRED: &str = "modality_required";
const PIN_REASON_PROFILE_POLICY_REQUIRED: &str = "profile_policy_required";

const PIN_ERROR_UNPINNED_DEPENDENCY: &str = "unpinned_dependency";
const PIN_ERROR_MODALITY_RESOLUTION_UNKNOWN: &str = "modality_resolution_unknown";

pub(super) struct BindingPinEvaluation {
    pub required_pins: Vec<ModelDependencyRequiredPin>,
    pub missing_pins: Vec<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

enum ModalityResolution {
    Known(BTreeSet<String>),
    Unknown(String),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_binding_pin_requirements(
    binding_id: &str,
    binding_kind: &str,
    backend_key: Option<&str>,
    profile_id: &str,
    profile_version: i64,
    environment_kind: &str,
    spec_json: &str,
    model_metadata: Option<&ModelMetadata>,
) -> BindingPinEvaluation {
    let field_context = format!("dependency_profiles.{}:{}", profile_id, profile_version);
    let parsed =
        match parse_and_canonicalize_profile_spec(spec_json, environment_kind, &field_context) {
            Ok(parsed) => parsed,
            Err(err) => {
                return BindingPinEvaluation {
                    required_pins: Vec::new(),
                    missing_pins: Vec::new(),
                    error_code: Some(PIN_ERROR_UNPINNED_DEPENDENCY.to_string()),
                    message: Some(format!(
                        "Dependency pin validation failed for {}:{}: {}",
                        profile_id, profile_version, err
                    )),
                };
            }
        };

    let mut required_pin_reasons: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for package in &parsed.required_policy_packages {
        register_required_pin(
            &mut required_pin_reasons,
            package,
            PIN_REASON_PROFILE_POLICY_REQUIRED,
        );
    }

    let normalized_backend = normalize_optional_token(backend_key);
    let mut modality_unknown_message = None;
    if normalized_backend.as_deref() == Some("pytorch") {
        register_required_pin(
            &mut required_pin_reasons,
            "torch",
            PIN_REASON_BACKEND_REQUIRED,
        );

        match resolve_effective_modalities(binding_id, &parsed, model_metadata) {
            ModalityResolution::Known(modalities) => {
                if modalities.contains("image") {
                    register_required_pin(
                        &mut required_pin_reasons,
                        "torchvision",
                        PIN_REASON_MODALITY_REQUIRED,
                    );
                }
                if modalities.contains("audio") {
                    register_required_pin(
                        &mut required_pin_reasons,
                        "torchaudio",
                        PIN_REASON_MODALITY_REQUIRED,
                    );
                }
            }
            ModalityResolution::Unknown(message) => {
                modality_unknown_message = Some(message);
            }
        }
    }

    let declared_packages: HashSet<String> = parsed
        .python_packages
        .iter()
        .map(|pin| pin.name.clone())
        .collect();

    let required_pins = required_pin_reasons
        .iter()
        .map(|(name, reasons)| ModelDependencyRequiredPin {
            name: name.clone(),
            reasons: reasons.iter().cloned().collect(),
        })
        .collect::<Vec<_>>();
    let mut missing_pins = required_pin_reasons
        .keys()
        .filter(|name| !declared_packages.contains(*name))
        .cloned()
        .collect::<Vec<_>>();
    missing_pins.sort();

    let mut error_code = None;
    let mut message = None;
    if let Some(modality_message) = modality_unknown_message {
        if is_required_binding_kind(binding_kind) {
            error_code = Some(PIN_ERROR_MODALITY_RESOLUTION_UNKNOWN.to_string());
            message = Some(modality_message);
        }
    } else if !missing_pins.is_empty() {
        error_code = Some(PIN_ERROR_UNPINNED_DEPENDENCY.to_string());
        message = Some(format!(
            "Required dependency pins are missing for binding {}: {}",
            binding_id,
            missing_pins.join(",")
        ));
    }

    BindingPinEvaluation {
        required_pins,
        missing_pins,
        error_code,
        message,
    }
}

fn register_required_pin(
    required_pin_reasons: &mut BTreeMap<String, BTreeSet<String>>,
    package_name: &str,
    reason: &str,
) {
    let normalized = package_name.trim().to_lowercase();
    if normalized.is_empty() {
        return;
    }
    required_pin_reasons
        .entry(normalized)
        .or_default()
        .insert(reason.to_string());
}

fn resolve_effective_modalities(
    binding_id: &str,
    parsed: &ParsedDependencyPinSpec,
    model_metadata: Option<&ModelMetadata>,
) -> ModalityResolution {
    if let Some(override_modalities) = parsed.binding_modality_overrides.get(binding_id) {
        return classify_modalities(
            &override_modalities.input_modalities,
            &override_modalities.output_modalities,
            format!(
                "binding modality override '{}' could not resolve canonical modalities",
                binding_id
            ),
        );
    }

    if let Some(metadata) = model_metadata {
        let has_metadata_modalities = metadata
            .input_modalities
            .as_ref()
            .map(|modalities| !modalities.is_empty())
            .unwrap_or(false)
            || metadata
                .output_modalities
                .as_ref()
                .map(|modalities| !modalities.is_empty())
                .unwrap_or(false);

        if has_metadata_modalities {
            return classify_modalities(
                metadata.input_modalities.as_deref().unwrap_or(&[]),
                metadata.output_modalities.as_deref().unwrap_or(&[]),
                "model metadata could not resolve canonical modalities".to_string(),
            );
        }

        if let Some(task_type_primary) = metadata.task_type_primary.as_deref() {
            let signature = normalize_task_signature(task_type_primary);
            return classify_modalities(
                &signature.input_modalities,
                &signature.output_modalities,
                "task fallback could not resolve canonical modalities".to_string(),
            );
        }
    }

    ModalityResolution::Unknown(
        "unable to resolve modalities via binding override, model metadata, or task fallback"
            .to_string(),
    )
}

fn classify_modalities(
    input_modalities: &[String],
    output_modalities: &[String],
    unknown_message: String,
) -> ModalityResolution {
    let mut combined = BTreeSet::new();
    for token in input_modalities.iter().chain(output_modalities.iter()) {
        let normalized = token.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        combined.insert(normalized);
    }

    if combined.is_empty() {
        return ModalityResolution::Unknown(unknown_message);
    }
    if combined.contains("unknown") || combined.contains("any") {
        return ModalityResolution::Unknown(unknown_message);
    }

    ModalityResolution::Known(combined)
}

fn build_requirements(parsed: &ParsedDependencyPinSpec) -> Vec<ModelDependencyRequirement> {
    let mut requirements = parsed
        .python_packages
        .iter()
        .map(|package| ModelDependencyRequirement {
            kind: "python_package".to_string(),
            name: package.name.clone(),
            exact_pin: package.version.clone(),
            index_url: normalize_url(package.index_url.clone()),
            extra_index_urls: normalize_urls(package.extra_index_urls.clone()),
            markers: normalize_optional_owned(package.markers.clone()),
            python_requires: normalize_optional_owned(package.python_requires.clone()),
            platform_constraints: normalize_string_vec(package.platform_constraints.clone()),
            hashes: normalize_hashes(package.hashes.clone()),
            source: normalize_optional_owned(package.source.clone()),
        })
        .collect::<Vec<_>>();

    requirements.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.exact_pin.cmp(&b.exact_pin))
    });
    requirements
}

fn validate_requirement_duplicates(
    requirements: &[ModelDependencyRequirement],
) -> std::result::Result<(), String> {
    let mut pins_by_key = HashMap::<(String, String), String>::new();
    for requirement in requirements {
        let key = (requirement.kind.clone(), requirement.name.clone());
        if let Some(existing_pin) = pins_by_key.get(&key) {
            if existing_pin != &requirement.exact_pin {
                return Err(format!(
                    "duplicate requirement '{}' has conflicting exact pins ('{}' vs '{}')",
                    requirement.name, existing_pin, requirement.exact_pin
                ));
            }
        } else {
            pins_by_key.insert(key, requirement.exact_pin.clone());
        }
    }
    Ok(())
}

fn ensure_model_exists(library: &ModelLibrary, model_id: &str) -> Result<()> {
    let model_dir = library.library_root().join(model_id);
    if model_dir.exists() {
        Ok(())
    } else {
        Err(PumasError::ModelNotFound {
            model_id: model_id.to_string(),
        })
    }
}

fn load_declared_binding_refs(library: &ModelLibrary, model_id: &str) -> Result<bool> {
    let model_dir = library.library_root().join(model_id);
    let metadata = library.load_metadata(&model_dir)?;
    Ok(metadata
        .as_ref()
        .and_then(|m| m.dependency_bindings.as_ref())
        .map(|bindings| !bindings.is_empty())
        .unwrap_or(false))
}

fn normalize_platform_key(platform_context: &str) -> String {
    let normalized = platform_context.trim().to_lowercase();
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

fn normalize_optional_token(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_lowercase)
}

fn normalize_optional_owned(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn normalize_string_vec(values: Vec<String>) -> Vec<String> {
    let mut normalized = values
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_urls(values: Vec<String>) -> Vec<String> {
    let mut normalized = values
        .into_iter()
        .filter_map(|v| normalize_url(Some(v)))
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_hashes(values: Vec<String>) -> Vec<String> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.starts_with("sha256:") || value.contains(':') {
                value
            } else {
                format!("sha256:{}", value)
            }
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_url(value: Option<String>) -> Option<String> {
    let raw = normalize_optional_owned(value)?;

    let Some((scheme, remainder)) = raw.split_once("://") else {
        return Some(raw.trim_end_matches('/').to_string());
    };

    let scheme = scheme.to_lowercase();
    let mut split_idx = remainder.len();
    for delimiter in ['/', '?', '#'] {
        if let Some(idx) = remainder.find(delimiter) {
            split_idx = split_idx.min(idx);
        }
    }

    let (authority, suffix) = remainder.split_at(split_idx);
    let authority = normalize_authority(authority);

    let normalized_suffix = if suffix.is_empty() {
        String::new()
    } else if let Some(query_or_fragment_start) = suffix.find(['?', '#']) {
        let (path_part, query_fragment) = suffix.split_at(query_or_fragment_start);
        format!("{}{}", path_part.trim_end_matches('/'), query_fragment)
    } else {
        suffix.trim_end_matches('/').to_string()
    };

    Some(format!("{}://{}{}", scheme, authority, normalized_suffix))
}

fn normalize_authority(authority: &str) -> String {
    let Some((userinfo, host_port)) = authority.rsplit_once('@') else {
        return authority.to_lowercase();
    };
    format!("{}@{}", userinfo, host_port.to_lowercase())
}

fn top_level_error(code: &str, field: Option<&str>, message: &str) -> DependencyValidationError {
    DependencyValidationError {
        code: code.to_string(),
        scope: DependencyValidationErrorScope::TopLevel,
        binding_id: None,
        field: field.map(String::from),
        message: message.to_string(),
    }
}

fn push_binding_error(
    binding: &mut ModelDependencyBindingRequirements,
    code: &str,
    field: Option<&str>,
    message: &str,
) {
    let error = DependencyValidationError {
        code: code.to_string(),
        scope: DependencyValidationErrorScope::Binding,
        binding_id: Some(binding.binding_id.clone()),
        field: field.map(String::from),
        message: message.to_string(),
    };

    if !binding.validation_errors.iter().any(|existing| {
        existing.code == error.code
            && existing.scope == error.scope
            && existing.binding_id == error.binding_id
            && existing.field == error.field
            && existing.message == error.message
    }) {
        binding.validation_errors.push(error);
    }

    sort_validation_errors(&mut binding.validation_errors);
}

fn sort_validation_errors(errors: &mut [DependencyValidationError]) {
    errors.sort_by(|a, b| {
        a.binding_id
            .as_deref()
            .unwrap_or("")
            .cmp(b.binding_id.as_deref().unwrap_or(""))
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| {
                a.field
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.field.as_deref().unwrap_or(""))
            })
            .then_with(|| a.message.cmp(&b.message))
    });
}

fn collect_validation_errors_union(
    bindings: &[ModelDependencyBindingRequirements],
) -> Vec<DependencyValidationError> {
    let mut seen = HashSet::<(
        String,
        DependencyValidationErrorScope,
        String,
        String,
        String,
    )>::new();
    let mut errors = Vec::new();

    for binding in bindings {
        for error in &binding.validation_errors {
            let key = (
                error.binding_id.clone().unwrap_or_default(),
                error.scope,
                error.code.clone(),
                error.field.clone().unwrap_or_default(),
                error.message.clone(),
            );
            if seen.insert(key) {
                errors.push(error.clone());
            }
        }
    }

    errors
}

fn aggregate_validation_state(
    bindings: &[ModelDependencyBindingRequirements],
) -> DependencyValidationState {
    let mut state = DependencyValidationState::Resolved;
    for binding in bindings {
        state = match (
            state_precedence(state),
            state_precedence(binding.validation_state),
        ) {
            (current, next) if next > current => binding.validation_state,
            _ => state,
        };
    }
    state
}

fn state_precedence(state: DependencyValidationState) -> u8 {
    match state {
        DependencyValidationState::Resolved => 0,
        DependencyValidationState::UnknownProfile => 1,
        DependencyValidationState::InvalidProfile => 2,
        DependencyValidationState::ProfileConflict => 3,
    }
}

fn mark_profile_conflicts(bindings: &mut [ModelDependencyBindingRequirements], platform_key: &str) {
    let mut by_target: HashMap<String, String> = HashMap::new();
    let mut conflict_binding_ids = HashSet::new();

    for binding in bindings.iter() {
        let Some(profile_hash) = binding.profile_hash.as_deref() else {
            continue;
        };
        let target_key = format!(
            "{}:{}:{}",
            binding.environment_kind.as_deref().unwrap_or("unknown"),
            platform_key,
            binding.backend_key.as_deref().unwrap_or("any")
        );

        if let Some(existing_hash) = by_target.get(&target_key) {
            if existing_hash != profile_hash {
                conflict_binding_ids.insert(binding.binding_id.clone());
                for prior in bindings.iter() {
                    let prior_target_key = format!(
                        "{}:{}:{}",
                        prior.environment_kind.as_deref().unwrap_or("unknown"),
                        platform_key,
                        prior.backend_key.as_deref().unwrap_or("any")
                    );
                    if prior_target_key == target_key {
                        conflict_binding_ids.insert(prior.binding_id.clone());
                    }
                }
            }
        } else {
            by_target.insert(target_key, profile_hash.to_string());
        }
    }

    if conflict_binding_ids.is_empty() {
        return;
    }

    for binding in bindings.iter_mut() {
        if conflict_binding_ids.contains(&binding.binding_id) {
            binding.validation_state = DependencyValidationState::ProfileConflict;
            binding.env_id = None;
            push_binding_error(
                binding,
                "profile_conflict",
                Some("profile_hash"),
                "Conflicting profile hashes resolved to the same target environment",
            );
        }
    }
}

fn platform_selector_matches(selector: Option<&str>, platform_key: &str) -> bool {
    let Some(selector) = selector else {
        return true;
    };
    let selector = selector.trim().to_lowercase();
    if selector.is_empty() || selector == "*" {
        return true;
    }
    selector
        .split([',', '|'])
        .map(|s| s.trim())
        .any(|token| token == "*" || token == platform_key)
}

fn build_env_id(
    environment_kind: &str,
    profile_id: &str,
    profile_version: i64,
    profile_hash: &str,
    platform_key: &str,
    backend_key: Option<&str>,
) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}",
        environment_kind,
        profile_id,
        profile_version,
        profile_hash,
        platform_key,
        backend_key.unwrap_or("any"),
    )
}

fn is_required_binding_kind(binding_kind: &str) -> bool {
    matches!(
        binding_kind.to_lowercase().as_str(),
        "required_core" | "required_custom"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{DependencyProfileRecord, ModelDependencyBindingRecord};
    use crate::model_library::types::ModelMetadata;
    use tempfile::TempDir;

    fn pinned_profile_spec(package: &str, version: &str) -> String {
        serde_json::json!({
            "python_packages": [
                {"name": package, "version": version}
            ]
        })
        .to_string()
    }

    async fn setup_library() -> (TempDir, ModelLibrary) {
        let temp_dir = TempDir::new().unwrap();
        let library = ModelLibrary::new(temp_dir.path()).await.unwrap();
        (temp_dir, library)
    }

    async fn create_model(library: &ModelLibrary, model_id: &str) {
        let model_dir = library.library_root().join(model_id);
        std::fs::create_dir_all(&model_dir).unwrap();
        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("llama".to_string()),
            official_name: Some("Test".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();
    }

    async fn create_model_with_declared_bindings(library: &ModelLibrary, model_id: &str) {
        let model_dir = library.library_root().join(model_id);
        std::fs::create_dir_all(&model_dir).unwrap();
        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            dependency_bindings: Some(vec![crate::models::DependencyBindingRef {
                binding_id: Some("declared-only".to_string()),
                profile_id: Some("p1".to_string()),
                profile_version: Some(1),
                binding_kind: Some("required_core".to_string()),
                backend_key: Some("pytorch".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
            }]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();
    }

    async fn create_model_with_modalities(
        library: &ModelLibrary,
        model_id: &str,
        input_modalities: Vec<&str>,
        output_modalities: Vec<&str>,
        task_type_primary: Option<&str>,
    ) {
        let model_dir = library.library_root().join(model_id);
        std::fs::create_dir_all(&model_dir).unwrap();
        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("llama".to_string()),
            official_name: Some("Test".to_string()),
            input_modalities: Some(input_modalities.into_iter().map(String::from).collect()),
            output_modalities: Some(output_modalities.into_iter().map(String::from).collect()),
            task_type_primary: task_type_primary.map(String::from),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();
    }

    #[tokio::test]
    async fn resolve_requirements_is_resolved_when_no_bindings_declared() {
        let (_tmp, library) = setup_library().await;
        create_model(&library, "llm/llama/no-bindings").await;

        let result = library
            .resolve_model_dependency_requirements("llm/llama/no-bindings", "linux-x86_64", None)
            .await
            .unwrap();

        assert_eq!(
            result.dependency_contract_version,
            DEPENDENCY_CONTRACT_VERSION
        );
        assert_eq!(result.validation_state, DependencyValidationState::Resolved);
        assert!(result.validation_errors.is_empty());
        assert!(result.bindings.is_empty());
    }

    #[tokio::test]
    async fn resolve_requirements_marks_unknown_when_declared_bindings_unresolved() {
        let (_tmp, library) = setup_library().await;
        create_model_with_declared_bindings(&library, "llm/llama/declared-only").await;

        let result = library
            .resolve_model_dependency_requirements("llm/llama/declared-only", "linux-x86_64", None)
            .await
            .unwrap();

        assert_eq!(
            result.validation_state,
            DependencyValidationState::UnknownProfile
        );
        assert_eq!(result.bindings.len(), 0);
        assert_eq!(
            result.validation_errors[0].code,
            "declared_bindings_unresolved".to_string()
        );
    }

    #[tokio::test]
    async fn resolve_requirements_detects_conflicting_profiles_for_same_target() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/conflict";
        create_model(&library, model_id).await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p1".to_string(),
                profile_version: 1,
                profile_hash: Some("h1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.4.0"),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p1".to_string(),
                profile_version: 2,
                profile_hash: Some("h2".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.0"),
                created_at: now.clone(),
            })
            .unwrap();

        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b1".to_string(),
                model_id: model_id.to_string(),
                profile_id: "p1".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("transformers".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now.clone(),
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b2".to_string(),
                model_id: model_id.to_string(),
                profile_id: "p1".to_string(),
                profile_version: 2,
                binding_kind: "required_core".to_string(),
                backend_key: Some("transformers".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 110,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let result = library
            .resolve_model_dependency_requirements(model_id, "linux-x86_64", Some("transformers"))
            .await
            .unwrap();

        assert_eq!(
            result.validation_state,
            DependencyValidationState::ProfileConflict
        );
        assert!(result
            .bindings
            .iter()
            .all(|binding| binding.validation_state == DependencyValidationState::ProfileConflict));
    }

    #[tokio::test]
    async fn resolve_requirements_emits_deterministic_requirements_payload() {
        let (_tmp, library) = setup_library().await;
        let model_id = "audio/stable-audio/stable-audio-open";
        create_model_with_modalities(
            &library,
            model_id,
            vec!["audio"],
            vec!["audio"],
            Some("text-to-audio"),
        )
        .await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "stable-audio-profile".to_string(),
                profile_version: 1,
                profile_hash: Some("stable-audio-hash".to_string()),
                environment_kind: "Python-Venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {
                            "name": "stable-audio-tools",
                            "version": "==1.0.0",
                            "index_url": "HTTPS://PYPI.ORG/simple/",
                            "extra_index_urls": ["https://download.pytorch.org/whl/", " https://download.pytorch.org/whl/ "],
                            "hashes": ["ABC123", "sha256:abc123"],
                            "source": "stable-audio"
                        },
                        {
                            "name": "torch",
                            "version": "==2.5.1",
                            "index": "https://download.pytorch.org/whl/"
                        },
                        {
                            "name": "torchaudio",
                            "version": "==2.5.1"
                        }
                    ]
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();

        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "stable-audio-binding".to_string(),
                model_id: model_id.to_string(),
                profile_id: "stable-audio-profile".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("PyTorch".to_string()),
                platform_selector: Some("Linux-X86_64".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let result = library
            .resolve_model_dependency_requirements(model_id, "linux-x86_64", Some("PYTORCH"))
            .await
            .unwrap();

        assert_eq!(result.validation_state, DependencyValidationState::Resolved);
        assert_eq!(result.bindings.len(), 1);

        let binding = &result.bindings[0];
        assert_eq!(binding.backend_key.as_deref(), Some("pytorch"));
        assert_eq!(binding.environment_kind.as_deref(), Some("python-venv"));
        assert!(binding.env_id.is_some());

        assert!(binding
            .requirements
            .iter()
            .any(|req| req.name == "stable-audio-tools" && req.exact_pin == "==1.0.0"));

        let stable_audio_req = binding
            .requirements
            .iter()
            .find(|req| req.name == "stable-audio-tools")
            .unwrap();
        assert_eq!(
            stable_audio_req.index_url.as_deref(),
            Some("https://pypi.org/simple")
        );
        assert_eq!(stable_audio_req.extra_index_urls.len(), 1);
        assert_eq!(stable_audio_req.hashes, vec!["sha256:abc123".to_string()]);
    }

    #[tokio::test]
    async fn resolve_requirements_invalid_profile_for_missing_required_pin() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/pytorch-unpinned";
        create_model_with_modalities(
            &library,
            model_id,
            vec!["text"],
            vec!["text"],
            Some("text-generation"),
        )
        .await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "pt-missing-torch".to_string(),
                profile_version: 1,
                profile_hash: Some("h-pt-1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("xformers", "==0.0.30"),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-pt-1".to_string(),
                model_id: model_id.to_string(),
                profile_id: "pt-missing-torch".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("pytorch".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let result = library
            .resolve_model_dependency_requirements(model_id, "linux-x86_64", Some("pytorch"))
            .await
            .unwrap();

        assert_eq!(
            result.validation_state,
            DependencyValidationState::InvalidProfile
        );
        assert_eq!(
            result.bindings[0].validation_errors[0].code,
            "unpinned_dependency".to_string()
        );
    }

    #[tokio::test]
    async fn audit_reports_unpinned_bindings_and_profile_backfill_suggestions() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/audit-unpinned";
        create_model_with_modalities(
            &library,
            model_id,
            vec!["text"],
            vec!["text"],
            Some("text-generation"),
        )
        .await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "audit-pt".to_string(),
                profile_version: 1,
                profile_hash: Some("h-audit-1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("xformers", "==0.0.30"),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-audit-1".to_string(),
                model_id: model_id.to_string(),
                profile_id: "audit-pt".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("pytorch".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let report = library.audit_dependency_pin_compliance().await.unwrap();
        assert_eq!(report.total_models_scanned, 1);
        assert_eq!(report.total_bindings_scanned, 1);
        assert_eq!(report.issue_count, 1);
        assert_eq!(report.binding_issues.len(), 1);
        assert_eq!(report.binding_issues[0].error_code, "unpinned_dependency");
        assert_eq!(
            report.profile_issues[0].suggested_backfill_pins,
            vec!["torch==<pin-me>".to_string()]
        );
    }
}
