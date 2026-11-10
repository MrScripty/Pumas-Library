//! Model-type resolver for metadata v2 classification.
//!
//! Uses active SQLite rule tables (`model_type_arch_rules`, `model_type_config_rules`)
//! and deterministic scoring rules to classify model type from hard source signals.

use crate::index::{ModelIndex, ModelTypeArchRule, ModelTypeConfigRule};
use crate::model_library::types::{HuggingFaceEvidence, ModelType};
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
    has_sentence_transformers_config: bool,
    has_sentence_transformers_modules: bool,
    has_vision_config: bool,
    has_vlm_task_hint: bool,
    has_vision_task_hint: bool,
    /// A `*ForSequenceClassification` head with exactly one label: one
    /// relevance logit per input, as cross-encoder rerankers export.
    single_logit_classifier: bool,
}

/// Resolve model type from rule tables using source metadata signals.
pub fn resolve_model_type_with_rules(
    index: &ModelIndex,
    model_dir: &Path,
    pipeline_tag: Option<&str>,
    spec_model_type: Option<&str>,
    huggingface_evidence: Option<&HuggingFaceEvidence>,
) -> Result<ModelTypeResolution> {
    // A GGUF that declares a decision head is served by llama.cpp as a decision
    // model. That file fact outranks any task tag or name.
    let medium_hints =
        collect_medium_hints(index, pipeline_tag, spec_model_type, huggingface_evidence)?;
    if gguf_declares_decision_head(model_dir) {
        // A task tag that names another category still wins nothing, but the
        // disagreement is recorded so a person can confirm the category.
        let conflicting_hint = medium_hints.iter().any(|hint| *hint != ModelType::Decision);
        let (confidence, review_reasons) = if conflicting_hint {
            (
                0.75,
                vec![
                    "model-type-hint-conflict".to_string(),
                    "model-type-low-confidence".to_string(),
                ],
            )
        } else {
            (0.95, Vec::new())
        };
        return Ok(ModelTypeResolution {
            model_type: ModelType::Decision,
            source: "model-type-gguf-decision-head".to_string(),
            confidence,
            review_reasons,
        });
    }
    // A GGUF ranking head without a decision head is what llama.cpp serves
    // through its rerank endpoint; that file fact outranks task tags too.
    if gguf_declares_ranking_head(model_dir) {
        return Ok(head_resolution(
            ModelType::Reranker,
            "model-type-gguf-ranking-head",
            &medium_hints,
        ));
    }
    let name_hint = model_dir.file_name().and_then(|name| name.to_str());
    let signals = load_config_signals(model_dir, huggingface_evidence);
    if signals.single_logit_classifier {
        return Ok(head_resolution(
            ModelType::Reranker,
            "model-type-single-logit-classifier",
            &medium_hints,
        ));
    }
    resolve_model_type_from_inputs(index, signals, medium_hints, name_hint)
}

/// A resolution from the model's head. A task tag naming another category is
/// recorded for review but does not change the result.
fn head_resolution(
    model_type: ModelType,
    source: &str,
    medium_hints: &[ModelType],
) -> ModelTypeResolution {
    let (confidence, review_reasons) = if medium_hints.iter().any(|hint| *hint != model_type) {
        (
            0.75,
            vec![
                "model-type-hint-conflict".to_string(),
                "model-type-low-confidence".to_string(),
            ],
        )
    } else {
        (0.95, Vec::new())
    };
    ModelTypeResolution {
        model_type,
        source: source.to_string(),
        confidence,
        review_reasons,
    }
}

/// A top-level, non-projector GGUF of the package carries a ranking head
/// (`cls.output.weight`) in any of its shards.
fn gguf_declares_ranking_head(model_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(model_dir) else {
        return false;
    };
    entries.filter_map(|entry| entry.ok()).any(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        name.ends_with(".gguf")
            && !name.contains("mmproj")
            && entry.file_type().is_ok_and(|kind| kind.is_file())
            && crate::model_library::gguf_package_has_ranking_head(&entry.path()) == Some(true)
    })
}

/// The model's primary GGUF (a top-level `.gguf` that is not a projector)
/// declares `<arch>.decision.type`.
fn gguf_declares_decision_head(model_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(model_dir) else {
        return false;
    };
    entries.filter_map(|entry| entry.ok()).any(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        name.ends_with(".gguf")
            && !name.contains("mmproj")
            && entry.file_type().is_ok_and(|kind| kind.is_file())
            && crate::model_library::identify_model_type(entry.path())
                .is_ok_and(|info| info.model_type == ModelType::Decision)
    })
}

pub fn resolve_model_type_from_huggingface_evidence(
    index: &ModelIndex,
    model_name: Option<&str>,
    pipeline_tag: Option<&str>,
    spec_model_type: Option<&str>,
    huggingface_evidence: Option<&HuggingFaceEvidence>,
) -> Result<ModelTypeResolution> {
    let signals = load_remote_config_signals(huggingface_evidence);
    let medium_hints =
        collect_medium_hints(index, pipeline_tag, spec_model_type, huggingface_evidence)?;
    let name_hint = model_name
        .or_else(|| huggingface_evidence.and_then(|evidence| evidence.repo_id.as_deref()));
    resolve_model_type_from_inputs(index, signals, medium_hints, name_hint)
}

fn resolve_model_type_from_inputs(
    index: &ModelIndex,
    signals: ConfigSignals,
    medium_hints: Vec<ModelType>,
    name_hint: Option<&str>,
) -> Result<ModelTypeResolution> {
    let arch_rules = index.list_active_model_type_arch_rules()?;
    let config_rules = index.list_active_model_type_config_rules()?;
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

    if should_apply_reranker_disambiguation_guard(&hard_types, name_hint, &signals, &medium_hints) {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Reranker,
            source: "model-type-reranker-disambiguation-guard".to_string(),
            confidence: 0.90,
            review_reasons: Vec::new(),
        });
    }

    if should_apply_audio_disambiguation_guard(&hard_types, name_hint, &signals, &medium_hints) {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Audio,
            source: "model-type-audio-disambiguation-guard".to_string(),
            confidence: 0.90,
            review_reasons: Vec::new(),
        });
    }

    if should_apply_vlm_disambiguation_guard(&hard_types, name_hint, &signals, &medium_hints) {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Vlm,
            source: "model-type-vlm-disambiguation-guard".to_string(),
            confidence: 0.90,
            review_reasons: Vec::new(),
        });
    }

    if should_apply_vision_disambiguation_guard(&hard_types, name_hint, &signals, &medium_hints) {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Vision,
            source: "model-type-vision-disambiguation-guard".to_string(),
            confidence: 0.90,
            review_reasons: Vec::new(),
        });
    }

    if should_apply_diffusion_disambiguation_guard(&hard_types, name_hint, &medium_hints) {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Diffusion,
            source: "model-type-diffusion-disambiguation-guard".to_string(),
            confidence: 0.85,
            review_reasons: Vec::new(),
        });
    }

    if should_apply_embedding_disambiguation_guard(&hard_types, name_hint, &signals, &medium_hints)
    {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Embedding,
            source: "model-type-embedding-disambiguation-guard".to_string(),
            confidence: 0.90,
            review_reasons: Vec::new(),
        });
    }

    if hard_types.len() > 1 {
        return Ok(ModelTypeResolution {
            model_type: ModelType::Unknown,
            source: "model-type-resolver-hard-conflict".to_string(),
            confidence: 0.0,
            review_reasons: vec!["model-type-conflict".to_string()],
        });
    }

    let Some(resolved_type) = hard_types.into_iter().next() else {
        if let Some(hint_resolved) = resolve_hint_only_model_type(&medium_hints) {
            return Ok(ModelTypeResolution {
                model_type: hint_resolved,
                source: "model-type-resolver-medium-hints".to_string(),
                confidence: 0.65,
                review_reasons: vec!["model-type-low-confidence".to_string()],
            });
        }

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

    let mut hint_conflict = false;
    for hint in &medium_hints {
        if *hint == resolved_type {
            score += 0.10;
        } else {
            score -= 0.20;
            hint_conflict = true;
        }
    }
    // A hard source signal (architecture, config) is never outvoted by a task
    // tag: a disagreeing tag lowers confidence and asks for review instead.
    score = score.clamp(if hint_conflict { 0.60 } else { 0.0 }, 1.0);

    let source = if !arch_votes.is_empty() && config_vote.is_some() {
        "model-type-resolver-arch-config-rules".to_string()
    } else if !arch_votes.is_empty() {
        "model-type-resolver-arch-rules".to_string()
    } else {
        "model-type-resolver-config-rules".to_string()
    };

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
    if hint_conflict {
        review_reasons.push("model-type-hint-conflict".to_string());
    }

    Ok(ModelTypeResolution {
        model_type: resolved_type,
        source,
        confidence: score,
        review_reasons,
    })
}

fn resolve_hint_only_model_type(medium_hints: &[ModelType]) -> Option<ModelType> {
    // Preserve hard-signal-first behavior, but accept a single unambiguous
    // task-derived hint as a low-confidence classification. HF pipeline tags
    // are often the only available source signal for remote metadata audits.
    if medium_hints.len() == 1 {
        let hint = medium_hints[0];
        if hint != ModelType::Unknown {
            return Some(hint);
        }
    }
    None
}

fn load_config_signals(
    model_dir: &Path,
    huggingface_evidence: Option<&HuggingFaceEvidence>,
) -> ConfigSignals {
    let config_path = model_dir.join("config.json");
    let Ok(config_str) = std::fs::read_to_string(&config_path) else {
        let mut signals = ConfigSignals::default();
        merge_huggingface_evidence_into_signals(&mut signals, huggingface_evidence);
        return signals;
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&config_str) else {
        let mut signals = ConfigSignals::default();
        merge_huggingface_evidence_into_signals(&mut signals, huggingface_evidence);
        return signals;
    };

    let architectures: Vec<String> = config
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

    let has_sentence_transformers_config = model_dir
        .join("config_sentence_transformers.json")
        .is_file();
    let has_sentence_transformers_modules = detect_sentence_transformers_modules(model_dir);
    let single_logit_classifier = architectures
        .iter()
        .any(|architecture: &String| architecture.ends_with("ForSequenceClassification"))
        && (config
            .get("id2label")
            .and_then(|labels| labels.as_object())
            .is_some_and(|labels| labels.len() == 1)
            || config.get("num_labels").and_then(|labels| labels.as_u64()) == Some(1));
    let has_vision_config = config.get("vision_config").is_some()
        || config.get("vision_encoder").is_some()
        || config.get("image_encoder").is_some()
        || config.get("vision_tower").is_some();

    let mut signals = ConfigSignals {
        architectures,
        config_model_type,
        has_sentence_transformers_config,
        has_sentence_transformers_modules,
        has_vision_config,
        has_vlm_task_hint: false,
        has_vision_task_hint: false,
        single_logit_classifier,
    };
    merge_huggingface_evidence_into_signals(&mut signals, huggingface_evidence);
    signals
}

fn load_remote_config_signals(huggingface_evidence: Option<&HuggingFaceEvidence>) -> ConfigSignals {
    let mut signals = ConfigSignals::default();
    merge_huggingface_evidence_into_signals(&mut signals, huggingface_evidence);
    signals
}

fn merge_huggingface_evidence_into_signals(
    signals: &mut ConfigSignals,
    huggingface_evidence: Option<&HuggingFaceEvidence>,
) {
    let Some(evidence) = huggingface_evidence else {
        return;
    };

    let mut architectures = std::mem::take(&mut signals.architectures);
    for architecture in evidence
        .architectures
        .as_ref()
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        if !architectures
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&architecture))
        {
            architectures.push(architecture);
        }
    }
    signals.architectures = architectures;

    if signals.config_model_type.is_none() {
        signals.config_model_type = evidence
            .config_model_type
            .as_deref()
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());
    }

    signals.has_vlm_task_hint |= evidence
        .pipeline_tag
        .as_deref()
        .is_some_and(is_vlm_task_hint)
        || evidence
            .remote_kind
            .as_deref()
            .is_some_and(is_vlm_task_hint)
        || evidence
            .tags
            .as_ref()
            .into_iter()
            .flatten()
            .any(|tag| is_vlm_task_hint(tag));

    signals.has_vision_task_hint |= evidence
        .pipeline_tag
        .as_deref()
        .is_some_and(is_vision_task_hint)
        || evidence
            .remote_kind
            .as_deref()
            .is_some_and(is_vision_task_hint)
        || evidence
            .tags
            .as_ref()
            .into_iter()
            .flatten()
            .any(|tag| is_vision_task_hint(tag) || tag.eq_ignore_ascii_case("vision"));
}

fn detect_sentence_transformers_modules(model_dir: &Path) -> bool {
    let modules_path = model_dir.join("modules.json");
    let Ok(contents) = std::fs::read_to_string(modules_path) else {
        return false;
    };
    let lower = contents.to_lowercase();
    lower.contains("sentence_transformers.models.")
        && (lower.contains("sentence_transformers.models.pooling")
            || lower.contains("sentence_transformers.models.normalize")
            || lower.contains("sentence_transformers.models.dense"))
}

fn name_looks_like_embedding(name_hint: Option<&str>) -> bool {
    name_hint
        .map(|name| {
            let lower = name.to_lowercase();
            lower.contains("embedding") || lower.contains("embed-")
        })
        .unwrap_or(false)
}

fn name_looks_like_reranker(name_hint: Option<&str>) -> bool {
    name_hint
        .map(|name| {
            let lower = name.to_lowercase();
            lower.contains("reranker")
                || lower.contains("re-ranker")
                || lower.contains("text-ranking")
                || lower.starts_with("rank")
                || lower.contains("-rank")
                || lower.contains("rank-")
                || lower.contains("_rank")
        })
        .unwrap_or(false)
}

fn name_looks_like_audio(name_hint: Option<&str>) -> bool {
    name_hint
        .map(|name| {
            let lower = name.to_lowercase();
            lower.contains("audio")
                || lower.contains("speech")
                || lower.contains("whisper")
                || lower.contains("tts")
                || lower.contains("musicgen")
                || lower.contains("encodec")
                || lower.contains("wav2vec")
                || lower.contains("hubert")
                || lower.contains("canary")
        })
        .unwrap_or(false)
}

fn name_looks_like_vlm(name_hint: Option<&str>) -> bool {
    name_hint
        .map(|name| {
            let lower = name.to_lowercase();
            lower.contains("florence")
                || lower.contains("paligemma")
                || lower.contains("idefics")
                || lower.contains("blip")
                || lower.contains("llava")
                || lower.contains("vlm")
                || lower.contains("qwen-vl")
                || lower.contains("-vl")
                || lower.contains("_vl")
        })
        .unwrap_or(false)
}

fn name_looks_like_diffusion(name_hint: Option<&str>) -> bool {
    name_hint
        .map(|name| {
            let lower = name.to_lowercase();
            lower.contains("qwen-image")
                || lower.contains("image-turbo")
                || lower.contains("image_turbo")
                || lower.contains("sd-turbo")
                || lower.contains("sd_turbo")
                || lower.contains("stable-diffusion")
                || lower.contains("stable_diffusion")
                || lower.contains("sdxl")
                || lower.contains("flux")
                || lower.contains("glm-image")
                || lower.contains("image-edit")
                || lower.contains("diffusion")
                || lower.contains("inpaint")
        })
        .unwrap_or(false)
}

fn has_reward_model_architecture(signals: &ConfigSignals) -> bool {
    signals.architectures.iter().any(|arch| {
        let lower = arch.to_lowercase();
        lower.contains("forrewardmodel") || lower.contains("rewardmodel")
    })
}

fn should_apply_reranker_disambiguation_guard(
    hard_types: &HashSet<ModelType>,
    name_hint: Option<&str>,
    signals: &ConfigSignals,
    medium_hints: &[ModelType],
) -> bool {
    // Respect explicit non-reranker hints.
    if medium_hints.iter().any(|hint| *hint != ModelType::Reranker) {
        return false;
    }

    let has_reward_arch = has_reward_model_architecture(signals);
    let has_reranker_name = name_looks_like_reranker(name_hint);
    let has_reranker_config = signals
        .config_model_type
        .as_deref()
        .is_some_and(|value| value.contains("rerank"));

    if medium_hints.contains(&ModelType::Reranker) && hard_types.contains(&ModelType::Llm) {
        return true;
    }

    if has_reranker_name && hard_types.len() == 1 && hard_types.contains(&ModelType::Llm) {
        return true;
    }

    if has_reward_arch
        && (hard_types.contains(&ModelType::Llm)
            || medium_hints.contains(&ModelType::Reranker)
            || has_reranker_name
            || has_reranker_config)
    {
        return true;
    }

    medium_hints.contains(&ModelType::Reranker) && (has_reranker_name || has_reranker_config)
}

fn should_apply_diffusion_disambiguation_guard(
    hard_types: &HashSet<ModelType>,
    name_hint: Option<&str>,
    medium_hints: &[ModelType],
) -> bool {
    if medium_hints
        .iter()
        .any(|hint| *hint != ModelType::Diffusion)
    {
        return false;
    }

    let has_diffusion_name = name_looks_like_diffusion(name_hint);

    if medium_hints.contains(&ModelType::Diffusion)
        && (hard_types.contains(&ModelType::Llm) || hard_types.contains(&ModelType::Vlm))
    {
        return true;
    }

    !hard_types.is_empty()
        && hard_types
            .iter()
            .all(|model_type| matches!(model_type, ModelType::Llm | ModelType::Vlm))
        && has_diffusion_name
}

fn has_vlm_architecture(signals: &ConfigSignals) -> bool {
    signals.architectures.iter().any(|arch| {
        let lower = arch.to_lowercase();
        lower.contains("florence")
            || lower.contains("paligemma")
            || lower.contains("idefics")
            || lower.contains("blip")
            || lower.contains("llava")
            || lower.contains("vlm")
            || lower.contains("_vl")
            || lower.contains("visionencoderdecoder")
    })
}

fn has_vlm_config(signals: &ConfigSignals) -> bool {
    signals.config_model_type.as_deref().is_some_and(|value| {
        value.contains("florence")
            || value.contains("paligemma")
            || value.contains("idefics")
            || value.contains("blip")
            || value.contains("llava")
            || value.contains("vlm")
            || value.contains("_vl")
            || value.contains("vision-encoder-decoder")
    })
}

fn should_apply_vlm_disambiguation_guard(
    hard_types: &HashSet<ModelType>,
    name_hint: Option<&str>,
    signals: &ConfigSignals,
    medium_hints: &[ModelType],
) -> bool {
    if medium_hints.iter().any(|hint| *hint != ModelType::Vlm) {
        let hard_conflict =
            hard_types.contains(&ModelType::Vision) && hard_types.contains(&ModelType::Llm);
        if !hard_conflict {
            return false;
        }
    }

    let has_vlm_specific_evidence = signals.has_vlm_task_hint
        || has_vlm_architecture(signals)
        || has_vlm_config(signals)
        || name_looks_like_vlm(name_hint);

    if hard_types.contains(&ModelType::Vision)
        && hard_types.contains(&ModelType::Llm)
        && has_vlm_specific_evidence
    {
        return true;
    }

    medium_hints.contains(&ModelType::Vlm)
        && (has_vlm_specific_evidence
            || signals.has_vision_config
            || hard_types.contains(&ModelType::Llm)
            || hard_types.contains(&ModelType::Vision))
}

fn should_apply_vision_disambiguation_guard(
    hard_types: &HashSet<ModelType>,
    _name_hint: Option<&str>,
    signals: &ConfigSignals,
    medium_hints: &[ModelType],
) -> bool {
    if medium_hints.iter().any(|hint| *hint != ModelType::Vision) {
        return false;
    }

    let has_vision_specific_evidence = signals.has_vision_config
        || signals.has_vision_task_hint
        || signals
            .architectures
            .iter()
            .any(|arch| arch.to_lowercase().contains("vision"))
        || signals
            .config_model_type
            .as_deref()
            .is_some_and(|value| value.contains("vision"));

    if hard_types.contains(&ModelType::Vision)
        && hard_types.contains(&ModelType::Llm)
        && has_vision_specific_evidence
    {
        return true;
    }

    medium_hints.contains(&ModelType::Vision)
        && (has_vision_specific_evidence || hard_types.contains(&ModelType::Llm))
}

fn has_audio_architecture(signals: &ConfigSignals) -> bool {
    signals.architectures.iter().any(|arch| {
        let lower = arch.to_lowercase();
        lower.contains("whisper")
            || lower.contains("speech")
            || lower.contains("audio")
            || lower.contains("asr")
            || lower.contains("wav2vec")
            || lower.contains("hubert")
            || lower.contains("wavlm")
            || lower.contains("encodec")
            || lower.contains("musicgen")
            || lower.contains("speecht5")
            || lower.contains("bark")
    })
}

fn should_apply_audio_disambiguation_guard(
    hard_types: &HashSet<ModelType>,
    name_hint: Option<&str>,
    signals: &ConfigSignals,
    medium_hints: &[ModelType],
) -> bool {
    if medium_hints.iter().any(|hint| *hint != ModelType::Audio) {
        return false;
    }

    let has_audio_config = signals.config_model_type.as_deref().is_some_and(|value| {
        value.contains("audio")
            || value.contains("speech")
            || value.contains("whisper")
            || value.contains("asr")
            || value.contains("tts")
            || value.contains("musicgen")
            || value.contains("encodec")
            || value.contains("wav2vec")
            || value.contains("hubert")
    });
    let has_audio_specific_evidence =
        has_audio_architecture(signals) || has_audio_config || name_looks_like_audio(name_hint);

    if hard_types.contains(&ModelType::Audio)
        && hard_types.contains(&ModelType::Llm)
        && has_audio_specific_evidence
    {
        return true;
    }

    medium_hints.contains(&ModelType::Audio)
        && (has_audio_specific_evidence || hard_types.contains(&ModelType::Llm))
}

fn should_apply_embedding_disambiguation_guard(
    hard_types: &HashSet<ModelType>,
    name_hint: Option<&str>,
    signals: &ConfigSignals,
    medium_hints: &[ModelType],
) -> bool {
    if hard_types.is_empty()
        || !hard_types
            .iter()
            .all(|model_type| matches!(model_type, ModelType::Llm | ModelType::Vlm))
    {
        return false;
    }

    // Respect explicit non-embedding hints.
    if medium_hints
        .iter()
        .any(|hint| *hint != ModelType::Embedding)
    {
        return false;
    }

    let mut evidence = 0u8;
    if medium_hints.contains(&ModelType::Embedding) {
        evidence += 2;
    }
    if signals.has_sentence_transformers_modules {
        evidence += 2;
    }
    if signals.has_sentence_transformers_config {
        evidence += 1;
    }
    if signals
        .config_model_type
        .as_deref()
        .is_some_and(|value| value.contains("embedding") || value.contains("sentence"))
    {
        evidence += 1;
    }
    if name_looks_like_embedding(name_hint) {
        evidence += 1;
    }

    evidence >= 2
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
    index: &ModelIndex,
    pipeline_tag: Option<&str>,
    spec_model_type: Option<&str>,
    huggingface_evidence: Option<&HuggingFaceEvidence>,
) -> Result<Vec<ModelType>> {
    // The source's own task tag outranks hints derived from it (search kinds,
    // download-dialog categories) whenever it names a known category.
    if let Some(tag) = huggingface_evidence
        .and_then(|evidence| evidence.pipeline_tag.as_deref())
        .or(pipeline_tag)
    {
        if let Some(mt) = map_medium_hint(index, tag)? {
            return Ok(vec![mt]);
        }
    }
    let mut hints = HashSet::new();
    let mut raw_hints = Vec::new();
    if let Some(evidence) = huggingface_evidence {
        for raw_hint in [
            evidence.pipeline_tag.as_deref(),
            evidence.remote_kind.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            raw_hints.push(raw_hint);
        }
    }
    if raw_hints.is_empty() {
        for raw_hint in [pipeline_tag, spec_model_type].into_iter().flatten() {
            raw_hints.push(raw_hint);
        }
    }

    for raw_hint in raw_hints {
        if let Some(mt) = map_medium_hint(index, raw_hint)? {
            hints.insert(mt);
        }
    }
    Ok(hints.into_iter().collect())
}

/// A task tag or category name as a known category, or `None`.
fn map_medium_hint(index: &ModelIndex, raw_hint: &str) -> Result<Option<ModelType>> {
    let direct = raw_hint.parse::<ModelType>().unwrap_or(ModelType::Unknown);
    if direct != ModelType::Unknown {
        return Ok(Some(direct));
    }
    Ok(index
        .resolve_model_type_hint(raw_hint)?
        .map(|mapped| parse_model_type(&mapped))
        .filter(|mt| *mt != ModelType::Unknown))
}

fn is_vlm_task_hint(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "image-to-text"
            | "image-text-to-text"
            | "visual-question-answering"
            | "document-question-answering"
            | "video-text-to-text"
    )
}

fn is_vision_task_hint(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "zero-shot-object-detection"
            | "object-detection"
            | "image-classification"
            | "image-segmentation"
            | "depth-estimation"
            | "video-classification"
            | "zero-shot-image-classification"
    )
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

    /// A GGUF header with string keys and the named tensors.
    fn write_gguf(path: &Path, keys: &[(&str, &str)], tensors: &[&str]) {
        fn string(bytes: &mut Vec<u8>, value: &str) {
            bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
            bytes.extend_from_slice(value.as_bytes());
        }
        let mut bytes = b"GGUF".to_vec();
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(keys.len() as u64).to_le_bytes());
        for (key, value) in keys {
            string(&mut bytes, key);
            bytes.extend_from_slice(&8_u32.to_le_bytes());
            string(&mut bytes, value);
        }
        for name in tensors {
            string(&mut bytes, name);
            bytes.extend_from_slice(&2_u32.to_le_bytes());
            bytes.extend_from_slice(&1024_u64.to_le_bytes());
            bytes.extend_from_slice(&1_u64.to_le_bytes());
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            bytes.extend_from_slice(&0_u64.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_gguf_ranking_head_makes_a_reranker_despite_a_classification_tag() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_gguf(
            &model_dir.path().join("bge-reranker-v2-m3-Q8_0.gguf"),
            &[("general.architecture", "bert")],
            &["token_embd.weight", "cls.output.weight"],
        );
        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("text-classification"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Reranker);
        assert_eq!(resolved.source, "model-type-gguf-ranking-head");
        assert!(resolved
            .review_reasons
            .contains(&"model-type-hint-conflict".to_string()));
    }

    #[test]
    fn a_declared_decision_head_still_outranks_its_ranking_head() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_gguf(
            &model_dir.path().join("Laya-Q8_0.gguf"),
            &[
                ("general.architecture", "modern-bert"),
                ("modern-bert.decision.type", "laya"),
            ],
            &["token_embd.weight", "cls.output.weight"],
        );
        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Decision);
    }

    #[test]
    fn a_single_logit_sequence_classifier_is_a_reranker() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["XLMRobertaForSequenceClassification"],
                "model_type": "xlm-roberta",
                "id2label": {"0": "LABEL_0"}
            }),
        );
        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("text-classification"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Reranker);
        assert_eq!(resolved.source, "model-type-single-logit-classifier");

        let labels = TempDir::new().unwrap();
        write_config(
            labels.path(),
            serde_json::json!({
                "architectures": ["XLMRobertaForSequenceClassification"],
                "model_type": "xlm-roberta",
                "id2label": {"0": "negative", "1": "positive"}
            }),
        );
        let resolved =
            resolve_model_type_with_rules(&index, labels.path(), None, None, None).unwrap();
        assert_ne!(resolved.model_type, ModelType::Reranker);
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
            None,
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

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
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

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Llm);
        assert!((resolved.confidence - 0.7).abs() < f64::EPSILON);
        assert!(resolved
            .review_reasons
            .contains(&"model-type-low-confidence".to_string()));
    }

    #[test]
    fn embedding_disambiguation_guard_overrides_qwen_causal_signals() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["Qwen3ForCausalLM"],
                "model_type": "qwen3"
            }),
        );
        std::fs::write(
            model_dir.path().join("config_sentence_transformers.json"),
            "{}",
        )
        .unwrap();
        std::fs::write(
            model_dir.path().join("modules.json"),
            r#"[{"type":"sentence_transformers.models.Pooling"},{"type":"sentence_transformers.models.Normalize"}]"#,
        )
        .unwrap();

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Embedding);
        assert_eq!(resolved.source, "model-type-embedding-disambiguation-guard");
        assert!(resolved.confidence >= 0.9);
        assert!(resolved.review_reasons.is_empty());
    }

    #[test]
    fn embedding_disambiguation_guard_overrides_vlm_backbone_for_feature_extraction() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["Qwen3VLForConditionalGeneration"],
                "model_type": "qwen3_vl",
                "vision_config": {
                    "model_type": "qwen3_vl"
                }
            }),
        );

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("feature-extraction"),
            None,
            None,
        )
        .unwrap();

        assert_eq!(resolved.model_type, ModelType::Embedding);
        assert_eq!(resolved.source, "model-type-embedding-disambiguation-guard");
        assert!(resolved.confidence >= 0.9);
        assert!(resolved.review_reasons.is_empty());
    }

    #[test]
    fn direct_pipeline_tag_mapping_beats_stale_db_hint_for_vlm_tasks() {
        let (tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["Qwen3_5MoeForConditionalGeneration"],
                "model_type": "qwen3_5_moe",
                "vision_config": {
                    "model_type": "qwen3_5_moe"
                }
            }),
        );

        let conn = rusqlite::Connection::open(tmp.path().join("models.db")).unwrap();
        conn.execute(
            "UPDATE model_type_config_rules
             SET model_type = 'vision'
             WHERE config_model_type = 'image-text-to-text'",
            [],
        )
        .unwrap();

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("image-text-to-text"),
            None,
            None,
        )
        .unwrap();

        assert_eq!(resolved.model_type, ModelType::Vlm);
        assert_eq!(resolved.source, "model-type-vlm-disambiguation-guard");
        assert!(resolved.confidence >= 0.9);
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

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Unknown);
        assert_eq!(resolved.confidence, 0.0);
        assert_eq!(resolved.source, "unresolved");
        assert!(resolved
            .review_reasons
            .contains(&"model-type-unresolved".to_string()));
    }

    #[test]
    fn audio_guard_overrides_generic_conditional_generation_conflict() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["WhisperForConditionalGeneration"],
                "model_type": "whisper"
            }),
        );

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Audio);
        assert_eq!(resolved.source, "model-type-audio-disambiguation-guard");
        assert!(resolved.confidence >= 0.9);
        assert!(resolved.review_reasons.is_empty());
    }

    #[test]
    fn llm_medium_hint_resolves_without_hard_signals() {
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
            None,
            None,
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Llm);
        assert_eq!(resolved.source, "model-type-resolver-medium-hints");
        assert_eq!(resolved.confidence, 0.65);
        assert!(resolved
            .review_reasons
            .contains(&"model-type-low-confidence".to_string()));
    }

    #[test]
    fn reranker_medium_hint_resolves_without_hard_signals() {
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
            Some("text-ranking"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Reranker);
        assert_eq!(resolved.source, "model-type-resolver-medium-hints");
        assert_eq!(resolved.confidence, 0.65);
        assert!(resolved
            .review_reasons
            .contains(&"model-type-low-confidence".to_string()));
    }

    #[test]
    fn audio_medium_hint_resolves_without_hard_signals() {
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
            Some("text-to-speech"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Audio);
        assert_eq!(resolved.source, "model-type-resolver-medium-hints");
        assert_eq!(resolved.confidence, 0.65);
        assert!(resolved
            .review_reasons
            .contains(&"model-type-low-confidence".to_string()));
    }

    #[test]
    fn reranker_disambiguation_guard_overrides_qwen_llm_config() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["Qwen3ForRewardModel"],
                "model_type": "qwen3"
            }),
        );

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("text-ranking"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Reranker);
        assert_eq!(resolved.source, "model-type-reranker-disambiguation-guard");
        assert!(resolved.confidence >= 0.9);
        assert!(resolved.review_reasons.is_empty());
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

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Diffusion);
    }

    #[test]
    fn cohere_asr_resolves_to_audio_without_a_task_tag() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["CohereAsrForConditionalGeneration"],
                "model_type": "cohere_asr"
            }),
        );

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Audio);
    }

    #[test]
    fn moss_tts_delay_resolves_to_audio_with_seeded_rules() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["MossTTSDelayModel"],
                "model_type": "moss_tts_delay"
            }),
        );

        let resolved =
            resolve_model_type_with_rules(&index, model_dir.path(), None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Audio);
        assert!(resolved.source.starts_with("model-type-resolver-"));
        assert!(resolved.confidence >= 0.7);
    }

    #[test]
    fn conflicting_medium_signal_lowers_confidence_without_outvoting_hard_signal() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["UNet2DConditionModel"]
            }),
        );

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("text-generation"),
            None,
            None,
        )
        .unwrap();
        // The architecture rule wins; the disagreeing task tag only lowers
        // confidence (clamped at 0.60) and requests review.
        assert_eq!(resolved.model_type, ModelType::Diffusion);
        assert_eq!(resolved.confidence, 0.60);
        assert!(resolved
            .review_reasons
            .contains(&"model-type-hint-conflict".to_string()));
        assert!(resolved
            .review_reasons
            .contains(&"model-type-low-confidence".to_string()));
        assert!(!resolved
            .review_reasons
            .contains(&"model-type-unresolved".to_string()));
    }

    #[test]
    fn persisted_hf_evidence_overrides_stale_pipeline_hint() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["Qwen3ForRewardModel"],
                "model_type": "qwen3"
            }),
        );

        let evidence = HuggingFaceEvidence {
            repo_id: Some("QuantFactory/Qwen3-Reranker-4B-GGUF".to_string()),
            pipeline_tag: Some("text-ranking".to_string()),
            remote_kind: Some("text-ranking".to_string()),
            ..Default::default()
        };

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("text-generation"),
            Some("llm"),
            Some(&evidence),
        )
        .unwrap();
        assert_eq!(resolved.model_type, ModelType::Reranker);
        assert_eq!(resolved.source, "model-type-reranker-disambiguation-guard");
    }

    #[test]
    fn reranker_name_guard_overrides_qwen_llm_config_without_medium_hint() {
        let (_tmp, index) = create_test_index();

        let resolved = resolve_model_type_from_huggingface_evidence(
            &index,
            Some("Qwen3-Reranker-4B-NVFP4"),
            None,
            None,
            Some(&HuggingFaceEvidence {
                repo_id: Some("Forturne/Qwen3-Reranker-4B-NVFP4".to_string()),
                architectures: Some(vec!["Qwen3ForCausalLM".to_string()]),
                config_model_type: Some("qwen3".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.model_type, ModelType::Reranker);
        assert_eq!(resolved.source, "model-type-reranker-disambiguation-guard");
    }

    #[test]
    fn diffusion_name_guard_overrides_qwen_llm_config_without_medium_hint() {
        let (_tmp, index) = create_test_index();

        let resolved = resolve_model_type_from_huggingface_evidence(
            &index,
            Some("Qwen-Image-2512-Heretic"),
            None,
            None,
            Some(&HuggingFaceEvidence {
                repo_id: Some("catplusplus/Qwen-Image-2512-Heretic".to_string()),
                architectures: Some(vec!["Qwen2_5_VLForConditionalGeneration".to_string()]),
                config_model_type: Some("qwen2_5_vl".to_string()),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.model_type, ModelType::Diffusion);
        assert_eq!(resolved.source, "model-type-diffusion-disambiguation-guard");
        assert!(resolved.review_reasons.is_empty());
    }

    #[test]
    fn vlm_guard_overrides_conditional_generation_for_florence() {
        let (_tmp, index) = create_test_index();
        let model_dir = TempDir::new().unwrap();
        write_config(
            model_dir.path(),
            serde_json::json!({
                "architectures": ["Florence2ForConditionalGeneration"],
                "model_type": "florence2",
                "vision_config": {
                    "model_type": "davit"
                }
            }),
        );

        let resolved = resolve_model_type_with_rules(
            &index,
            model_dir.path(),
            Some("image-text-to-text"),
            None,
            Some(&HuggingFaceEvidence {
                repo_id: Some("microsoft/Florence-2-large".to_string()),
                pipeline_tag: Some("image-text-to-text".to_string()),
                remote_kind: Some("image-text-to-text".to_string()),
                architectures: Some(vec!["Florence2ForConditionalGeneration".to_string()]),
                config_model_type: Some("florence2".to_string()),
                tags: Some(vec!["vision".to_string(), "image-text-to-text".to_string()]),
                ..Default::default()
            }),
        )
        .unwrap();

        assert_eq!(resolved.model_type, ModelType::Vlm);
        assert_eq!(resolved.source, "model-type-vlm-disambiguation-guard");
        assert!(resolved.review_reasons.is_empty());
    }
}
