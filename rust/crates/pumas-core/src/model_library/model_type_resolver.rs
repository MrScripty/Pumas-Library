//! Model-type resolver for metadata v2 classification.
//!
//! Uses active SQLite rule tables (`model_type_arch_rules`, `model_type_config_rules`)
//! and deterministic scoring rules to classify model type from hard source signals.

use crate::index::{ModelIndex, ModelTypeArchRule, ModelTypeConfigRule};
use crate::model_library::types::ModelType;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ModelTypeResolution {
    pub model_type: ModelType,
    pub source: String,
    pub confidence: f64,
    pub review_reasons: Vec<String>,
}

#[derive(Debug, Default)]
struct ConfigSignals {
    architectures: Vec<String>,
    config_model_type: Option<String>,
}

/// Resolve model type from rule tables using source metadata signals.
pub fn resolve_model_type_with_rules(
    index: &ModelIndex,
    model_dir: &Path,
    pipeline_tag: Option<&str>,
    spec_model_type: Option<&str>,
) -> Result<ModelTypeResolution> {
    let arch_rules = index.list_active_model_type_arch_rules()?;
    let config_rules = index.list_active_model_type_config_rules()?;
    let signals = load_config_signals(model_dir);

    let arch_votes = resolve_architecture_votes(&signals.architectures, &arch_rules);
    let config_vote = resolve_config_vote(signals.config_model_type.as_deref(), &config_rules);

    let mut hard_types = HashSet::new();
    for (_, mt) in &arch_votes {
        if *mt != ModelType::Unknown {
            hard_types.insert(*mt);
        }
    }
    if let Some(mt) = config_vote {
        if mt != ModelType::Unknown {
            hard_types.insert(mt);
        }
    }

    if hard_types.len() > 1 {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Unknown,
            source: "model-type-resolver-hard-conflict".to_string(),
            confidence: 0.0,
            review_reasons: vec!["model-type-conflict".to_string()],
        });
    }

    let Some(resolved) = hard_types.into_iter().next() else {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Unknown,
            source: "unresolved".to_string(),
            confidence: 0.0,
            review_reasons: vec!["model-type-unresolved".to_string()],
        });
    };

    let mut score: f64 = 0.70;
    let hard_signal_count = arch_votes.len() + usize::from(config_vote.is_some());
    if hard_signal_count >= 2 {
        score += 0.20;
    }

    let medium_hints = collect_medium_hints(pipeline_tag, spec_model_type);
    for hint in medium_hints {
        if hint == resolved {
            score += 0.10;
        } else {
            score -= 0.20;
        }
    }
    score = score.clamp(0.0, 1.0);

    if score < 0.60 {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Unknown,
            source: "unresolved".to_string(),
            confidence: 0.0,
            review_reasons: vec!["model-type-unresolved".to_string()],
        });
    }

    let mut review_reasons = Vec::new();
    if score < 0.85 {
        review_reasons.push("model-type-low-confidence".to_string());
    }

    let source = if !arch_votes.is_empty() && config_vote.is_some() {
        "model-type-resolver-arch-config-rules"
    } else if !arch_votes.is_empty() {
        "model-type-resolver-arch-rules"
    } else {
        "model-type-resolver-config-rules"
    };

    Ok(ModelTypeResolution {
        model_type: resolved,
        source: source.to_string(),
        confidence: score,
        review_reasons,
    })
}

fn load_config_signals(model_dir: &Path) -> ConfigSignals {
    let config_path = model_dir.join("config.json");
    let Ok(config_str) = std::fs::read_to_string(&config_path) else {
        return ConfigSignals::default();
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_str) else {
        return ConfigSignals::default();
    };

    let architectures = config
        .get("architectures")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let config_model_type = config
        .get("model_type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    ConfigSignals {
        architectures,
        config_model_type,
    }
}

fn resolve_architecture_votes(
    architectures: &[String],
    rules: &[ModelTypeArchRule],
) -> Vec<(String, ModelType)> {
    let mut votes = Vec::new();
    for arch in architectures {
        let arch_norm = arch.trim().to_lowercase();
        if arch_norm.is_empty() {
            continue;
        }

        let mut matches: Vec<&ModelTypeArchRule> = rules
            .iter()
            .filter(|rule| arch_matches_rule(&arch_norm, rule))
            .collect();
        matches.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| b.pattern.len().cmp(&a.pattern.len()))
                .then_with(|| a.pattern.cmp(&b.pattern))
        });

        if let Some(rule) = matches.first() {
            votes.push((arch.clone(), parse_model_type(&rule.model_type)));
        }
    }
    votes
}

fn resolve_config_vote(
    config_model_type: Option<&str>,
    rules: &[ModelTypeConfigRule],
) -> Option<ModelType> {
    let value = config_model_type?;
    let value = value.trim().to_lowercase();
    if value.is_empty() {
        return None;
    }

    rules
        .iter()
        .find(|rule| rule.config_model_type.eq_ignore_ascii_case(&value))
        .map(|rule| parse_model_type(&rule.model_type))
}

fn collect_medium_hints(
    pipeline_tag: Option<&str>,
    spec_model_type: Option<&str>,
) -> Vec<ModelType> {
    let mut hints = HashSet::new();
    if let Some(tag) = pipeline_tag {
        let mt = ModelType::from_pipeline_tag(tag);
        if mt != ModelType::Unknown {
            hints.insert(mt);
        }
    }
    if let Some(value) = spec_model_type {
        let mt = parse_model_type(value);
        if mt != ModelType::Unknown {
            hints.insert(mt);
        }
    }
    hints.into_iter().collect()
}

fn parse_model_type(value: &str) -> ModelType {
    value.trim().parse().unwrap_or(ModelType::Unknown)
}

fn arch_matches_rule(architecture: &str, rule: &ModelTypeArchRule) -> bool {
    let pattern = rule.pattern.trim().to_lowercase();
    match rule.match_style.as_str() {
        "exact" => architecture == pattern,
        "prefix" => architecture.starts_with(&pattern),
        "suffix" => architecture.ends_with(&pattern),
        "wildcard" => wildcard_match(architecture, &pattern),
        _ => false,
    }
}

fn wildcard_match(value: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return value == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut offset = 0usize;

    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if idx == 0 && !pattern.starts_with('*') {
            if !value.starts_with(part) {
                return false;
            }
            offset = part.len();
            continue;
        }

        if idx == parts.len() - 1 && !pattern.ends_with('*') {
            let suffix_start = value[offset..].rfind(part).map(|p| offset + p);
            let Some(start) = suffix_start else {
                return false;
            };
            if start + part.len() != value.len() {
                return false;
            }
            offset = start + part.len();
            continue;
        }

        let Some(found) = value[offset..].find(part) else {
            return false;
        };
        offset += found + part.len();
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::ModelIndex;
    use tempfile::TempDir;

    fn create_test_index() -> (TempDir, ModelIndex) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("models.db");
        let index = ModelIndex::new(&db_path).unwrap();
        (temp_dir, index)
    }

    fn write_config(model_dir: &Path, config: serde_json::Value) {
        let content = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(model_dir.join("config.json"), content).unwrap();
    }

    #[test]
    fn resolves_with_agreeing_hard_signals() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["LlamaForCausalLM"],
                "model_type": "llama"
            }),
        );

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("text-generation"),
            Some("llm"),
        )
        .unwrap();

        assert_eq!(resolved.model_type, ModelType::Llm);
        assert_eq!(resolved.source, "model-type-resolver-arch-config-rules");
        assert!((resolved.confidence - 1.0).abs() < f64::EPSILON);
        assert!(resolved.review_reasons.is_empty());
    }

    #[test]
    fn hard_signal_conflict_returns_unknown() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["UNet2DConditionModel"],
                "model_type": "llama"
            }),
        );

        let resolved = resolve_model_type_with_rules(&index, model_dir.path(), None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Unknown);
        assert_eq!(resolved.confidence, 0.0);
        assert_eq!(resolved.source, "model-type-resolver-hard-conflict");
        assert!(resolved
            .review_reasons
            .contains(&"model-type-conflict".to_string()));
    }

    #[test]
    fn single_hard_signal_marks_low_confidence_review() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["LlamaForCausalLM"]
            }),
        );

        let resolved = resolve_model_type_with_rules(&index, model_dir.path(), None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Llm);
        assert!((resolved.confidence - 0.7).abs() < f64::EPSILON);
        assert!(resolved
            .review_reasons
            .contains(&"model-type-low-confidence".to_string()));
    }

    #[test]
    fn unresolved_when_no_hard_signals() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["UnmappedArchitecture"]
            }),
        );

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("text-generation"),
            Some("llm"),
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Unknown);
        assert_eq!(resolved.confidence, 0.0);
        assert_eq!(resolved.source, "unresolved");
        assert!(resolved
            .review_reasons
            .contains(&"model-type-unresolved".to_string()));
    }

    #[test]
    fn wildcard_architecture_rule_matches() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["StableDiffusionXLPipeline"]
            }),
        );

        let resolved = resolve_model_type_with_rules(&index, model_dir.path(), None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Diffusion);
    }

    #[test]
    fn conflicting_medium_signal_can_drop_below_threshold() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["UNet2DConditionModel"]
            }),
        );

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), Some("text-generation"), None)
                .unwrap();
        assert_eq!(resolved.model_type, ModelType::Unknown);
        assert_eq!(resolved.confidence, 0.0);
        assert!(resolved
            .review_reasons
            .contains(&"model-type-unresolved".to_string()));
    }
}
