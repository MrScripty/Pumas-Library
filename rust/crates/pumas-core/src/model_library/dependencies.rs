//! Model-level dependency planning and validation.
//!
//! This module provides deterministic dependency profile/binding resolution
//! using SQLite dependency tables.

use crate::error::{PumasError, Result};
use crate::model_library::library::ModelLibrary;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
        let mut bindings = self
            .index()
            .list_active_model_dependency_bindings(model_id, backend_key)?
            .into_iter()
            .filter(|b| platform_selector_matches(b.platform_selector.as_deref(), &platform_key))
            .map(|binding| {
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
                    binding_id: binding.binding_id,
                    model_id: binding.model_id,
                    profile_id: binding.profile_id,
                    profile_version: binding.profile_version,
                    profile_hash,
                    environment_kind,
                    binding_kind: binding.binding_kind,
                    backend_key: binding.backend_key,
                    platform_selector: binding.platform_selector,
                    priority: binding.priority,
                    env_id,
                    state: DependencyState::Ready,
                    error_code: None,
                    message: None,
                };

                if binding.spec_json.is_none() || plan.profile_hash.is_none() {
                    plan.state = DependencyState::UnknownProfile;
                    plan.error_code = Some("unknown_profile".to_string());
                    plan.message =
                        Some("Dependency profile is missing or incomplete in SQLite".to_string());
                }
                plan
            })
            .collect::<Vec<_>>();

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
                bindings,
            });
        }

        Ok(ModelDependencyPlan {
            model_id: model_id.to_string(),
            platform_key,
            backend_key: backend_key.map(String::from),
            state: DependencyState::Ready,
            error_code: None,
            message: None,
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
        if plan.state == DependencyState::Ready {
            for binding in &mut bindings {
                binding.state = DependencyState::Missing;
                binding.error_code = None;
                binding.message = Some(
                    "Dependency presence probing is not implemented yet; treating as missing"
                        .to_string(),
                );
            }
        }

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
                    bindings,
                });
            }
        }

        Ok(ModelDependencyCheckResult {
            model_id: plan.model_id,
            platform_key: plan.platform_key,
            backend_key: plan.backend_key,
            state: plan.state,
            error_code: plan.error_code,
            message: plan.message,
            selected_binding_ids,
            bindings,
        })
    }

    /// Install dependencies for model/context.
    ///
    /// This currently returns deterministic planning/check results and does not
    /// execute installation commands.
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

        let selected_set = selected_binding_ids
            .as_ref()
            .map(|v| v.iter().cloned().collect::<HashSet<_>>());
        let mut attempted = Vec::new();
        let mut skipped = Vec::new();
        for binding in &check.bindings {
            let selected = match selected_set.as_ref() {
                Some(set) => set.contains(&binding.binding_id),
                None => true,
            };
            if selected {
                attempted.push(binding.binding_id.clone());
            } else {
                skipped.push(binding.binding_id.clone());
            }
        }

        Ok(ModelDependencyInstallResult {
            model_id: check.model_id,
            platform_key: check.platform_key,
            backend_key: check.backend_key,
            state: check.state,
            error_code: check.error_code,
            message: Some(check.message.unwrap_or_else(|| {
                "Installation executor not implemented; returning deterministic plan/check only"
                    .to_string()
            })),
            selected_binding_ids: check.selected_binding_ids,
            attempted_binding_ids: attempted,
            installed_binding_ids: Vec::new(),
            skipped_binding_ids: skipped,
            bindings: check.bindings,
        })
    }
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
                spec_json: "{}".to_string(),
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
                spec_json: "{}".to_string(),
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
                spec_json: "{}".to_string(),
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
}
