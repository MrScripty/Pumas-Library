//! Model-level dependency planning and validation.
//!
//! This module provides deterministic dependency profile/binding resolution
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
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Dependency lifecycle/check/install states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum DependencyState {
    Ready,
    Missing,
    Failed,
    UnknownProfile,
    ManualInterventionRequired,
    ProfileConflict,
}

/// Per-binding dependency pin summary.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyPinSummary {
    pub pinned: bool,
    pub required_count: u32,
    pub pinned_count: u32,
    pub missing_count: u32,
}

/// Per-binding required dependency pin with requirement provenance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyRequiredPin {
    pub name: String,
    pub reasons: Vec<String>,
}

/// Per-binding resolution/check/install row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyBindingPlan {
    pub binding_id: String,
    pub model_id: String,
    pub profile_id: String,
    pub profile_version: i64,
    pub profile_hash: Option<String>,
    pub environment_kind: String,
    pub binding_kind: String,
    pub backend_key: Option<String>,
    pub platform_selector: Option<String>,
    pub priority: i64,
    pub env_id: String,
    pub state: DependencyState,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub pin_summary: ModelDependencyPinSummary,
    pub required_pins: Vec<ModelDependencyRequiredPin>,
    pub missing_pins: Vec<String>,
}

/// Deterministic dependency plan for a model/context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyPlan {
    pub model_id: String,
    pub platform_key: String,
    pub backend_key: Option<String>,
    pub state: DependencyState,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub missing_pins: Vec<String>,
    pub bindings: Vec<ModelDependencyBindingPlan>,
}

/// Dependency check result for a model/context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyCheckResult {
    pub model_id: String,
    pub platform_key: String,
    pub backend_key: Option<String>,
    pub state: DependencyState,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub selected_binding_ids: Option<Vec<String>>,
    pub missing_pins: Vec<String>,
    pub bindings: Vec<ModelDependencyBindingPlan>,
}

/// Dependency install result for a model/context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDependencyInstallResult {
    pub model_id: String,
    pub platform_key: String,
    pub backend_key: Option<String>,
    pub state: DependencyState,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub selected_binding_ids: Option<Vec<String>>,
    pub attempted_binding_ids: Vec<String>,
    pub installed_binding_ids: Vec<String>,
    pub skipped_binding_ids: Vec<String>,
    pub missing_pins: Vec<String>,
    pub bindings: Vec<ModelDependencyBindingPlan>,
}

impl ModelLibrary {
    /// Return model dependency profiles/bindings for model + context.
    pub async fn get_model_dependency_profiles(
        &self,
        model_id: &str,
        platform_context: &str,
        backend_key: Option<&str>,
    ) -> Result<Vec<ModelDependencyBindingPlan>> {
        let plan = self
            .resolve_model_dependency_plan(model_id, platform_context, backend_key)
            .await?;
        Ok(plan.bindings)
    }

    /// Resolve deterministic model dependency plan.
    pub async fn resolve_model_dependency_plan(
        &self,
        model_id: &str,
        platform_context: &str,
        backend_key: Option<&str>,
    ) -> Result<ModelDependencyPlan> {
        ensure_model_exists(self, model_id)?;

        let platform_key = normalize_platform_key(platform_context);
        let model_metadata = self.get_effective_metadata(model_id)?;
        let mut bindings = Vec::new();
        for binding in self
            .index()
            .list_active_model_dependency_bindings(model_id, backend_key)?
            .into_iter()
            .filter(|b| platform_selector_matches(b.platform_selector.as_deref(), &platform_key))
        {
            let profile_hash = binding.profile_hash.clone();
            let environment_kind = binding
                .environment_kind
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let resolved_backend_key = binding
                .backend_key
                .clone()
                .or_else(|| backend_key.map(String::from));
            let env_id = build_env_id(
                &environment_kind,
                &binding.profile_id,
                binding.profile_version,
                profile_hash.as_deref(),
                &platform_key,
                resolved_backend_key.as_deref(),
            );

            let mut plan = ModelDependencyBindingPlan {
                binding_id: binding.binding_id.clone(),
                model_id: binding.model_id,
                profile_id: binding.profile_id.clone(),
                profile_version: binding.profile_version,
                profile_hash,
                environment_kind: environment_kind.clone(),
                binding_kind: binding.binding_kind.clone(),
                backend_key: binding.backend_key.clone(),
                platform_selector: binding.platform_selector,
                priority: binding.priority,
                env_id,
                state: DependencyState::Ready,
                error_code: None,
                message: None,
                pin_summary: ModelDependencyPinSummary::default(),
                required_pins: Vec::new(),
                missing_pins: Vec::new(),
            };

            if binding.spec_json.is_none() || plan.profile_hash.is_none() {
                plan.state = DependencyState::UnknownProfile;
                plan.error_code = Some("unknown_profile".to_string());
                plan.message =
                    Some("Dependency profile is missing or incomplete in SQLite".to_string());
            } else if let Some(spec_json) = binding.spec_json.as_deref() {
                let pin_eval = evaluate_binding_pin_requirements(
                    &binding.binding_id,
                    &binding.binding_kind,
                    resolved_backend_key.as_deref(),
                    &binding.profile_id,
                    binding.profile_version,
                    &environment_kind,
                    spec_json,
                    model_metadata.as_ref(),
                );
                plan.pin_summary = pin_eval.pin_summary;
                plan.required_pins = pin_eval.required_pins;
                plan.missing_pins = pin_eval.missing_pins;
                if let Some(code) = pin_eval.error_code {
                    plan.state = DependencyState::ManualInterventionRequired;
                    plan.error_code = Some(code);
                    plan.message = pin_eval.message;
                }
            }

            bindings.push(plan);
        }

        if bindings.is_empty() {
            let declared_refs = load_declared_binding_refs(self, model_id)?;
            if declared_refs {
                return Ok(ModelDependencyPlan {
                    model_id: model_id.to_string(),
                    platform_key,
                    backend_key: backend_key.map(String::from),
                    state: DependencyState::UnknownProfile,
                    error_code: Some("unknown_profile".to_string()),
                    message: Some(
                        "Model metadata references dependency bindings, but no active SQLite bindings were resolved"
                            .to_string(),
                    ),
                    missing_pins: Vec::new(),
                    bindings,
                });
            }

            return Ok(ModelDependencyPlan {
                model_id: model_id.to_string(),
                platform_key,
                backend_key: backend_key.map(String::from),
                state: DependencyState::Ready,
                error_code: None,
                message: Some("No dependency bindings declared for model".to_string()),
                missing_pins: Vec::new(),
                bindings,
            });
        }

        let conflicting_binding_ids = detect_profile_conflicts(&bindings, &platform_key);
        if !conflicting_binding_ids.is_empty() {
            for binding in &mut bindings {
                if conflicting_binding_ids.contains(&binding.binding_id) {
                    binding.state = DependencyState::ProfileConflict;
                    binding.error_code = Some("profile_conflict".to_string());
                    binding.message = Some(
                        "Different profile hashes resolved to the same deterministic environment id"
                            .to_string(),
                    );
                }
            }
            return Ok(ModelDependencyPlan {
                model_id: model_id.to_string(),
                platform_key,
                backend_key: backend_key.map(String::from),
                state: DependencyState::ProfileConflict,
                error_code: Some("profile_conflict".to_string()),
                message: Some(
                    "Conflicting profile hashes detected for identical env_id".to_string(),
                ),
                missing_pins: aggregate_missing_required_pins(&bindings),
                bindings,
            });
        }

        if bindings
            .iter()
            .any(|b| b.state == DependencyState::UnknownProfile)
        {
            return Ok(ModelDependencyPlan {
                model_id: model_id.to_string(),
                platform_key,
                backend_key: backend_key.map(String::from),
                state: DependencyState::UnknownProfile,
                error_code: Some("unknown_profile".to_string()),
                message: Some(
                    "One or more dependency bindings reference unknown profiles".to_string(),
                ),
                missing_pins: aggregate_missing_required_pins(&bindings),
                bindings,
            });
        }

        let plan_state = aggregate_dependency_state(DependencyState::Ready, &bindings);
        let (plan_error_code, plan_message) = dependency_state_summary(&plan_state, &bindings);

        Ok(ModelDependencyPlan {
            model_id: model_id.to_string(),
            platform_key,
            backend_key: backend_key.map(String::from),
            state: plan_state.clone(),
            error_code: if plan_state == DependencyState::Ready {
                None
            } else {
                plan_error_code
            },
            message: if plan_state == DependencyState::Ready {
                None
            } else {
                plan_message
            },
            missing_pins: aggregate_missing_required_pins(&bindings),
            bindings,
        })
    }

    /// Check dependency readiness for model/context.
    ///
    /// Current implementation validates plan consistency and required-binding closure.
    /// Environment probing/installation is not yet implemented, so resolved bindings
    /// are reported as `missing`.
    pub async fn check_model_dependencies(
        &self,
        model_id: &str,
        platform_context: &str,
        backend_key: Option<&str>,
        selected_binding_ids: Option<Vec<String>>,
    ) -> Result<ModelDependencyCheckResult> {
        let plan = self
            .resolve_model_dependency_plan(model_id, platform_context, backend_key)
            .await?;

        let mut bindings = plan.bindings.clone();
        if let Some(ref selected) = selected_binding_ids {
            let selected_set: HashSet<&str> = selected.iter().map(|s| s.as_str()).collect();
            let missing_required = missing_required_binding_ids(&bindings, &selected_set);
            if !missing_required.is_empty() {
                for binding in &mut bindings {
                    if missing_required.contains(&binding.binding_id) {
                        binding.state = DependencyState::Failed;
                        binding.error_code = Some("required_binding_omitted".to_string());
                        binding.message =
                            Some("Caller selection omitted a required binding".to_string());
                    }
                }
                return Ok(ModelDependencyCheckResult {
                    model_id: plan.model_id,
                    platform_key: plan.platform_key,
                    backend_key: plan.backend_key,
                    state: DependencyState::Failed,
                    error_code: Some("required_binding_omitted".to_string()),
                    message: Some(format!(
                        "Required bindings missing from selection: {}",
                        missing_required.join(",")
                    )),
                    selected_binding_ids,
                    missing_pins: aggregate_missing_required_pins(&bindings),
                    bindings,
                });
            }
        }

        if plan.state == DependencyState::Ready {
            let runtime_specs = load_runtime_specs(self, model_id, platform_context, backend_key)?;
            let model_dir = self.library_root().join(model_id);
            for binding in &mut bindings {
                let Some(runtime_spec) = runtime_specs.get(&binding.binding_id) else {
                    binding.state = DependencyState::UnknownProfile;
                    binding.error_code = Some("unknown_profile".to_string());
                    binding.message =
                        Some("Dependency binding runtime profile could not be loaded".to_string());
                    continue;
                };

                let outcome = probe_binding_readiness(runtime_spec, &model_dir).await;
                binding.state = outcome.state;
                binding.error_code = outcome.error_code;
                binding.message = outcome.message;
            }
        }

        let check_state = aggregate_dependency_state(plan.state.clone(), &bindings);
        let (check_error_code, check_message) = dependency_state_summary(&check_state, &bindings);

        Ok(ModelDependencyCheckResult {
            model_id: plan.model_id,
            platform_key: plan.platform_key,
            backend_key: plan.backend_key,
            state: check_state,
            error_code: check_error_code.or(plan.error_code),
            message: check_message.or(plan.message),
            selected_binding_ids,
            missing_pins: aggregate_missing_required_pins(&bindings),
            bindings,
        })
    }

    /// Return dependency install guidance for model/context.
    ///
    /// Pumas Core does not execute installers. It returns deterministic
    /// readiness/installability data for consumer-managed environments.
    pub async fn install_model_dependencies(
        &self,
        model_id: &str,
        platform_context: &str,
        backend_key: Option<&str>,
        selected_binding_ids: Option<Vec<String>>,
    ) -> Result<ModelDependencyInstallResult> {
        let check = self
            .check_model_dependencies(
                model_id,
                platform_context,
                backend_key,
                selected_binding_ids.clone(),
            )
            .await?;

        let runtime_specs = load_runtime_specs(self, model_id, platform_context, backend_key)?;
        let mut bindings = check.bindings.clone();

        let selected_set = selected_binding_ids
            .as_ref()
            .map(|v| v.iter().cloned().collect::<HashSet<_>>());
        let attempted = Vec::new();
        let installed = Vec::new();
        let mut skipped = Vec::new();
        for binding in &mut bindings {
            let selected = match selected_set.as_ref() {
                Some(set) => set.contains(&binding.binding_id),
                None => true,
            };
            if selected {
                if matches!(
                    binding.error_code.as_deref(),
                    Some("unpinned_dependency" | "modality_resolution_unknown")
                ) {
                    // Guardrail: never execute install commands while required pin semantics
                    // are unresolved for this binding.
                    continue;
                }

                if binding.state != DependencyState::Missing {
                    continue;
                }

                let Some(runtime_spec) = runtime_specs.get(&binding.binding_id) else {
                    binding.state = DependencyState::UnknownProfile;
                    binding.error_code = Some("unknown_profile".to_string());
                    binding.message =
                        Some("Dependency binding runtime profile could not be loaded".to_string());
                    continue;
                };

                let install_commands = runtime_spec.install_commands.clone().unwrap_or_default();
                if install_commands.is_empty() {
                    binding.state = DependencyState::ManualInterventionRequired;
                    binding.error_code = Some("manual_intervention_required".to_string());
                    binding.message = Some(
                        "No install commands are defined; consumer must install manually"
                            .to_string(),
                    );
                    continue;
                }

                binding.state = DependencyState::ManualInterventionRequired;
                binding.error_code = Some("installation_delegated_to_consumer".to_string());
                let first = &install_commands[0];
                let source = first.source_url.as_deref().unwrap_or("unknown-source");
                let source_ref = first.source_ref.as_deref().unwrap_or("unknown-ref");
                binding.message = Some(format!(
                    "Pumas does not execute install commands; consumer should run '{}' {:?} (and {} total command(s), source={} ref={})",
                    first.program,
                    first.args,
                    install_commands.len(),
                    source,
                    source_ref
                ));
            } else {
                skipped.push(binding.binding_id.clone());
            }
        }

        let aggregate_seed = if check.state == DependencyState::Missing {
            DependencyState::Ready
        } else {
            check.state.clone()
        };
        let final_state = aggregate_dependency_state(aggregate_seed, &bindings);
        let (final_error_code, final_message) = dependency_state_summary(&final_state, &bindings);

        Ok(ModelDependencyInstallResult {
            model_id: check.model_id,
            platform_key: check.platform_key,
            backend_key: check.backend_key,
            state: final_state,
            error_code: final_error_code.or(check.error_code),
            message: final_message.or(check.message),
            selected_binding_ids: check.selected_binding_ids,
            attempted_binding_ids: attempted,
            installed_binding_ids: installed,
            skipped_binding_ids: skipped,
            missing_pins: aggregate_missing_required_pins(&bindings),
            bindings,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct DependencyProfileSpec {
    #[serde(default)]
    probes: Vec<DependencyProbeSpec>,
    #[serde(default)]
    install: Option<DependencyInstallSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct DependencyInstallSpec {
    #[serde(default)]
    commands: Vec<DependencyCommandSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DependencyProbeSpec {
    Command {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        success_exit_codes: Option<Vec<i32>>,
    },
    PathExists {
        path: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct DependencyCommandSpec {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    source_ref: Option<String>,
}

#[derive(Debug, Clone)]
struct BindingRuntimeSpec {
    binding_id: String,
    profile_id: String,
    profile_version: i64,
    probes: Vec<DependencyProbeSpec>,
    install_commands: Option<Vec<DependencyCommandSpec>>,
}

struct BindingProbeOutcome {
    state: DependencyState,
    error_code: Option<String>,
    message: Option<String>,
}

const PIN_REASON_BACKEND_REQUIRED: &str = "backend_required";
const PIN_REASON_MODALITY_REQUIRED: &str = "modality_required";
const PIN_REASON_PROFILE_POLICY_REQUIRED: &str = "profile_policy_required";

const PIN_ERROR_UNPINNED_DEPENDENCY: &str = "unpinned_dependency";
const PIN_ERROR_MODALITY_RESOLUTION_UNKNOWN: &str = "modality_resolution_unknown";

pub(super) struct BindingPinEvaluation {
    pub pin_summary: ModelDependencyPinSummary,
    pub required_pins: Vec<ModelDependencyRequiredPin>,
    pub missing_pins: Vec<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

enum ModalityResolution {
    Known(BTreeSet<String>),
    Unknown(String),
}

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
                    pin_summary: ModelDependencyPinSummary {
                        pinned: false,
                        required_count: 0,
                        pinned_count: 0,
                        missing_count: 0,
                    },
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

    let normalized_backend = backend_key
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_lowercase);
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

    let required_count = required_pins.len() as u32;
    let missing_count = missing_pins.len() as u32;
    let pinned_count = required_count.saturating_sub(missing_count);
    let pinned = missing_pins.is_empty() && modality_unknown_message.is_none();

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
        pin_summary: ModelDependencyPinSummary {
            pinned,
            required_count,
            pinned_count,
            missing_count,
        },
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

fn load_runtime_specs(
    library: &ModelLibrary,
    model_id: &str,
    platform_context: &str,
    backend_key: Option<&str>,
) -> Result<HashMap<String, BindingRuntimeSpec>> {
    let platform_key = normalize_platform_key(platform_context);
    let records = library
        .index()
        .list_active_model_dependency_bindings(model_id, backend_key)?;

    let mut specs = HashMap::new();
    for record in records.into_iter().filter(|record| {
        platform_selector_matches(record.platform_selector.as_deref(), &platform_key)
    }) {
        let Some(spec_json) = record.spec_json.as_ref() else {
            continue;
        };

        let parsed: DependencyProfileSpec =
            serde_json::from_str(spec_json).map_err(|err| PumasError::Validation {
                field: format!(
                    "dependency_profiles.{}:{}",
                    record.profile_id, record.profile_version
                ),
                message: format!("invalid spec_json: {}", err),
            })?;

        specs.insert(
            record.binding_id.clone(),
            BindingRuntimeSpec {
                binding_id: record.binding_id,
                profile_id: record.profile_id,
                profile_version: record.profile_version,
                probes: parsed.probes,
                install_commands: parsed.install.map(|install| install.commands),
            },
        );
    }

    Ok(specs)
}

async fn probe_binding_readiness(
    runtime_spec: &BindingRuntimeSpec,
    model_dir: &Path,
) -> BindingProbeOutcome {
    if runtime_spec.probes.is_empty() {
        return BindingProbeOutcome {
            state: DependencyState::Missing,
            error_code: Some("probe_not_defined".to_string()),
            message: Some("No dependency probes are defined for this profile".to_string()),
        };
    }

    for probe in &runtime_spec.probes {
        match run_probe(probe, model_dir).await {
            Ok(true) => {}
            Ok(false) => {
                return BindingProbeOutcome {
                    state: DependencyState::Missing,
                    error_code: Some("probe_failed".to_string()),
                    message: Some(format!(
                        "Dependency probe failed for {}:{} (binding {})",
                        runtime_spec.profile_id,
                        runtime_spec.profile_version,
                        runtime_spec.binding_id
                    )),
                };
            }
            Err(message) => {
                return BindingProbeOutcome {
                    state: DependencyState::ManualInterventionRequired,
                    error_code: Some("manual_intervention_required".to_string()),
                    message: Some(message),
                };
            }
        }
    }

    BindingProbeOutcome {
        state: DependencyState::Ready,
        error_code: None,
        message: Some("Dependency probes passed".to_string()),
    }
}

async fn run_probe(
    probe: &DependencyProbeSpec,
    model_dir: &Path,
) -> std::result::Result<bool, String> {
    match probe {
        DependencyProbeSpec::PathExists { path } => {
            let resolved = resolve_probe_path(model_dir, path);
            Ok(resolved.exists())
        }
        DependencyProbeSpec::Command {
            program,
            args,
            success_exit_codes,
        } => run_command_probe(program, args, success_exit_codes.as_ref()).await,
    }
}

async fn run_command_probe(
    program: &str,
    args: &[String],
    success_exit_codes: Option<&Vec<i32>>,
) -> std::result::Result<bool, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|err| format!("failed to execute probe command {}: {}", program, err))?;

    let code = output.status.code().unwrap_or(-1);
    let allowed_codes = success_exit_codes.cloned().unwrap_or_else(|| vec![0]);
    Ok(allowed_codes.contains(&code))
}

fn resolve_probe_path(model_dir: &Path, path: &str) -> PathBuf {
    let raw = PathBuf::from(path);
    if raw.is_absolute() {
        raw
    } else {
        model_dir.join(raw)
    }
}

fn aggregate_dependency_state(
    initial_state: DependencyState,
    bindings: &[ModelDependencyBindingPlan],
) -> DependencyState {
    if initial_state != DependencyState::Ready {
        return initial_state;
    }

    if bindings
        .iter()
        .any(|binding| binding.state == DependencyState::Failed)
    {
        return DependencyState::Failed;
    }
    if bindings
        .iter()
        .any(|binding| binding.state == DependencyState::ManualInterventionRequired)
    {
        return DependencyState::ManualInterventionRequired;
    }
    if bindings
        .iter()
        .any(|binding| binding.state == DependencyState::UnknownProfile)
    {
        return DependencyState::UnknownProfile;
    }
    if bindings
        .iter()
        .any(|binding| binding.state == DependencyState::ProfileConflict)
    {
        return DependencyState::ProfileConflict;
    }
    if bindings
        .iter()
        .any(|binding| binding.state == DependencyState::Missing)
    {
        return DependencyState::Missing;
    }

    DependencyState::Ready
}

fn dependency_state_summary(
    state: &DependencyState,
    bindings: &[ModelDependencyBindingPlan],
) -> (Option<String>, Option<String>) {
    match state {
        DependencyState::Ready => (None, Some("All dependency bindings are ready".to_string())),
        DependencyState::Missing => (
            Some("missing_dependencies".to_string()),
            Some(format!(
                "{} dependency bindings are missing",
                bindings
                    .iter()
                    .filter(|binding| binding.state == DependencyState::Missing)
                    .count()
            )),
        ),
        DependencyState::Failed => (
            Some("dependency_install_failed".to_string()),
            Some("One or more dependency bindings failed".to_string()),
        ),
        DependencyState::UnknownProfile => (
            Some("unknown_profile".to_string()),
            Some("One or more dependency profiles are unknown".to_string()),
        ),
        DependencyState::ManualInterventionRequired => {
            if bindings.iter().any(|binding| {
                is_required_binding_kind(&binding.binding_kind)
                    && binding.error_code.as_deref() == Some(PIN_ERROR_MODALITY_RESOLUTION_UNKNOWN)
            }) {
                return (
                    Some(PIN_ERROR_MODALITY_RESOLUTION_UNKNOWN.to_string()),
                    Some(
                        "Modality requirements could not be resolved for one or more required dependency bindings"
                            .to_string(),
                    ),
                );
            }

            if bindings.iter().any(|binding| {
                is_required_binding_kind(&binding.binding_kind)
                    && binding.error_code.as_deref() == Some(PIN_ERROR_UNPINNED_DEPENDENCY)
            }) {
                return (
                    Some(PIN_ERROR_UNPINNED_DEPENDENCY.to_string()),
                    Some(
                        "One or more required dependency bindings are missing exact required pins"
                            .to_string(),
                    ),
                );
            }

            (
                Some("manual_intervention_required".to_string()),
                Some("Dependency configuration requires manual intervention".to_string()),
            )
        }
        DependencyState::ProfileConflict => (
            Some("profile_conflict".to_string()),
            Some("Dependency profile conflict detected".to_string()),
        ),
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
    profile_hash: Option<&str>,
    platform_key: &str,
    backend_key: Option<&str>,
) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}",
        environment_kind,
        profile_id,
        profile_version,
        profile_hash.unwrap_or("unknown"),
        platform_key,
        backend_key.unwrap_or("any"),
    )
}

fn detect_profile_conflicts(
    bindings: &[ModelDependencyBindingPlan],
    platform_key: &str,
) -> HashSet<String> {
    let mut by_target: HashMap<String, String> = HashMap::new();
    let mut conflicts = HashSet::new();
    for binding in bindings {
        let hash = binding.profile_hash.as_deref().unwrap_or("unknown");
        let target_key = format!(
            "{}:{}:{}",
            binding.environment_kind,
            platform_key,
            binding.backend_key.as_deref().unwrap_or("any")
        );

        if let Some(existing_hash) = by_target.get(&target_key) {
            if existing_hash != hash {
                conflicts.insert(binding.binding_id.clone());
                for prior in bindings {
                    let prior_target_key = format!(
                        "{}:{}:{}",
                        prior.environment_kind,
                        platform_key,
                        prior.backend_key.as_deref().unwrap_or("any")
                    );
                    if prior_target_key == target_key {
                        conflicts.insert(prior.binding_id.clone());
                    }
                }
            }
        } else {
            by_target.insert(target_key, hash.to_string());
        }
    }
    conflicts
}

fn missing_required_binding_ids(
    bindings: &[ModelDependencyBindingPlan],
    selected_binding_ids: &HashSet<&str>,
) -> Vec<String> {
    let mut missing = bindings
        .iter()
        .filter(|binding| is_required_binding_kind(&binding.binding_kind))
        .filter(|binding| !selected_binding_ids.contains(binding.binding_id.as_str()))
        .map(|binding| binding.binding_id.clone())
        .collect::<Vec<_>>();
    missing.sort();
    missing
}

fn aggregate_missing_required_pins(bindings: &[ModelDependencyBindingPlan]) -> Vec<String> {
    let mut missing = BTreeSet::new();
    for binding in bindings
        .iter()
        .filter(|binding| is_required_binding_kind(&binding.binding_kind))
    {
        for pin in &binding.missing_pins {
            missing.insert(pin.clone());
        }
    }
    missing.into_iter().collect()
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
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("llama".to_string()),
            official_name: Some("Test".to_string()),
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
    async fn resolve_plan_is_ready_when_no_bindings_declared() {
        let (_tmp, library) = setup_library().await;
        create_model(&library, "llm/llama/no-bindings").await;

        let plan = library
            .resolve_model_dependency_plan("llm/llama/no-bindings", "linux-x86_64", None)
            .await
            .unwrap();

        assert_eq!(plan.state, DependencyState::Ready);
        assert!(plan.bindings.is_empty());
    }

    #[tokio::test]
    async fn resolve_plan_flags_profile_conflict_on_env_collision() {
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
                profile_id: "p2".to_string(),
                profile_version: 1,
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
                profile_id: "p2".to_string(),
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

        let plan = library
            .resolve_model_dependency_plan(model_id, "linux-x86_64", Some("transformers"))
            .await
            .unwrap();

        assert_eq!(plan.state, DependencyState::ProfileConflict);
        assert_eq!(plan.error_code.as_deref(), Some("profile_conflict"));
    }

    #[tokio::test]
    async fn check_requires_selected_required_bindings() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/selection";
        create_model(&library, model_id).await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p1".to_string(),
                profile_version: 1,
                profile_hash: Some("h1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1"),
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
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let result = library
            .check_model_dependencies(model_id, "linux-x86_64", Some("transformers"), Some(vec![]))
            .await
            .unwrap();

        assert_eq!(result.state, DependencyState::Failed);
        assert_eq!(
            result.error_code.as_deref(),
            Some("required_binding_omitted")
        );
    }

    #[tokio::test]
    async fn check_marks_binding_ready_when_probe_passes() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/probe-ready";
        create_model(&library, model_id).await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p-ready".to_string(),
                profile_version: 1,
                profile_hash: Some("h-ready".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1"}
                    ],
                    "probes": [
                        { "kind": "command", "program": "true" }
                    ]
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-ready".to_string(),
                model_id: model_id.to_string(),
                profile_id: "p-ready".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("transformers".to_string()),
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

        let check = library
            .check_model_dependencies(model_id, "linux-x86_64", Some("transformers"), None)
            .await
            .unwrap();

        assert_eq!(check.state, DependencyState::Ready);
        assert_eq!(check.bindings.len(), 1);
        assert_eq!(check.bindings[0].state, DependencyState::Ready);
    }

    #[tokio::test]
    async fn install_is_informational_only_and_does_not_execute_commands() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/install-flow";
        create_model(&library, model_id).await;

        let model_dir = library.library_root().join(model_id);
        let marker = model_dir.join("deps").join("ok.flag");
        assert!(!marker.exists());

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "p-install".to_string(),
                profile_version: 1,
                profile_hash: Some("h-install".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1"}
                    ],
                    "probes": [
                        { "kind": "path_exists", "path": "deps/ok.flag" }
                    ],
                    "install": {
                        "commands": [
                            { "program": "sh", "args": ["-c", "mkdir -p deps && touch deps/ok.flag"], "source_url": "https://example.com/install", "source_ref": "README.md" }
                        ]
                    }
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-install".to_string(),
                model_id: model_id.to_string(),
                profile_id: "p-install".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("transformers".to_string()),
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

        let pre_check = library
            .check_model_dependencies(model_id, "linux-x86_64", Some("transformers"), None)
            .await
            .unwrap();
        assert_eq!(pre_check.state, DependencyState::Missing);
        assert_eq!(pre_check.bindings[0].state, DependencyState::Missing);

        let install = library
            .install_model_dependencies(model_id, "linux-x86_64", Some("transformers"), None)
            .await
            .unwrap();
        assert_eq!(install.state, DependencyState::ManualInterventionRequired);
        assert!(install.attempted_binding_ids.is_empty());
        assert!(install.installed_binding_ids.is_empty());
        assert_eq!(
            install.bindings[0].state,
            DependencyState::ManualInterventionRequired
        );
        assert_eq!(
            install.bindings[0].error_code.as_deref(),
            Some("installation_delegated_to_consumer")
        );
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn resolve_plan_flags_unpinned_required_pytorch_binding() {
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

        let plan = library
            .resolve_model_dependency_plan(model_id, "linux-x86_64", Some("pytorch"))
            .await
            .unwrap();

        assert_eq!(plan.state, DependencyState::ManualInterventionRequired);
        assert_eq!(plan.error_code.as_deref(), Some("unpinned_dependency"));
        assert_eq!(plan.missing_pins, vec!["torch".to_string()]);
        assert_eq!(
            plan.bindings[0].error_code.as_deref(),
            Some("unpinned_dependency")
        );
        assert_eq!(plan.bindings[0].missing_pins, vec!["torch".to_string()]);
        assert!(!plan.bindings[0].pin_summary.pinned);
    }

    #[tokio::test]
    async fn resolve_plan_uses_binding_modality_override_precedence() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/pytorch-override";
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
                profile_id: "pt-override".to_string(),
                profile_version: 1,
                profile_hash: Some("h-pt-2".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1"},
                        {"name": "torchvision", "version": "==0.20.1"}
                    ],
                    "binding_modality_overrides": {
                        "b-pt-2": {
                            "input_modalities": ["image"],
                            "output_modalities": ["text"]
                        }
                    }
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-pt-2".to_string(),
                model_id: model_id.to_string(),
                profile_id: "pt-override".to_string(),
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

        let plan = library
            .resolve_model_dependency_plan(model_id, "linux-x86_64", Some("pytorch"))
            .await
            .unwrap();

        assert_eq!(plan.state, DependencyState::Ready);
        assert!(plan.bindings[0].pin_summary.pinned);
        assert_eq!(plan.bindings[0].required_pins.len(), 2);
        assert_eq!(plan.bindings[0].required_pins[0].name, "torch");
        assert_eq!(plan.bindings[0].required_pins[1].name, "torchvision");
    }

    #[tokio::test]
    async fn resolve_plan_marks_required_binding_when_modality_unknown() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/pytorch-modality-unknown";
        create_model(&library, model_id).await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "pt-core".to_string(),
                profile_version: 1,
                profile_hash: Some("h-pt-3".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1"),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-pt-3".to_string(),
                model_id: model_id.to_string(),
                profile_id: "pt-core".to_string(),
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

        let plan = library
            .resolve_model_dependency_plan(model_id, "linux-x86_64", Some("pytorch"))
            .await
            .unwrap();

        assert_eq!(plan.state, DependencyState::ManualInterventionRequired);
        assert_eq!(
            plan.error_code.as_deref(),
            Some("modality_resolution_unknown")
        );
        assert_eq!(
            plan.bindings[0].error_code.as_deref(),
            Some("modality_resolution_unknown")
        );
        assert!(!plan.bindings[0].pin_summary.pinned);
    }

    #[tokio::test]
    async fn top_level_missing_pins_only_include_required_bindings() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/pin-union";
        create_model_with_modalities(
            &library,
            model_id,
            vec!["audio"],
            vec!["text"],
            Some("automatic-speech-recognition"),
        )
        .await;

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "pt-required".to_string(),
                profile_version: 1,
                profile_hash: Some("h-pt-4".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1"),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "pt-optional".to_string(),
                profile_version: 1,
                profile_hash: Some("h-pt-5".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: pinned_profile_spec("torch", "==2.5.1"),
                created_at: now.clone(),
            })
            .unwrap();

        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-pt-4".to_string(),
                model_id: model_id.to_string(),
                profile_id: "pt-required".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("pytorch".to_string()),
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
                binding_id: "b-pt-5".to_string(),
                model_id: model_id.to_string(),
                profile_id: "pt-optional".to_string(),
                profile_version: 1,
                binding_kind: "optional_feature".to_string(),
                backend_key: Some("pytorch".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 200,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let plan = library
            .resolve_model_dependency_plan(model_id, "linux-x86_64", Some("pytorch"))
            .await
            .unwrap();

        assert_eq!(plan.missing_pins, vec!["torchaudio".to_string()]);
        assert_eq!(plan.bindings.len(), 2);
        assert_eq!(
            plan.bindings[0].missing_pins,
            vec!["torchaudio".to_string()]
        );
        assert_eq!(
            plan.bindings[1].missing_pins,
            vec!["torchaudio".to_string()]
        );
    }

    #[tokio::test]
    async fn required_pin_reasons_include_backend_and_policy() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/pin-reasons";
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
                profile_id: "pt-policy".to_string(),
                profile_version: 1,
                profile_hash: Some("h-pt-6".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1"}
                    ],
                    "pin_policy": {
                        "required_packages": [
                            {"name": "torch"}
                        ]
                    }
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                binding_id: "b-pt-6".to_string(),
                model_id: model_id.to_string(),
                profile_id: "pt-policy".to_string(),
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

        let plan = library
            .resolve_model_dependency_plan(model_id, "linux-x86_64", Some("pytorch"))
            .await
            .unwrap();

        let torch = plan.bindings[0]
            .required_pins
            .iter()
            .find(|pin| pin.name == "torch")
            .unwrap();
        assert_eq!(
            torch.reasons,
            vec![
                "backend_required".to_string(),
                "profile_policy_required".to_string()
            ]
        );
    }
}
