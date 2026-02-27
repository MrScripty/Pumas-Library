//! Task signature normalization for metadata v2 classification.
//!
//! Normalizes heterogeneous task labels into canonical `inputs->outputs`
//! signatures with deterministic modality ordering.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Normalization outcome status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskNormalizationStatus {
    Ok,
    Warning,
    Error,
}

/// Canonical task-signature normalization result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NormalizedTaskSignature {
    pub signature_key: String,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub normalization_status: TaskNormalizationStatus,
    pub normalization_warnings: Vec<String>,
}

impl NormalizedTaskSignature {
    fn error_invalid_signature() -> Self {
        Self {
            signature_key: "unknown->unknown".to_string(),
            input_modalities: vec!["unknown".to_string()],
            output_modalities: vec!["unknown".to_string()],
            normalization_status: TaskNormalizationStatus::Error,
            normalization_warnings: vec!["invalid-task-signature".to_string()],
        }
    }
}

const MODALITY_ORDER: [&str; 15] = [
    "text",
    "image",
    "audio",
    "video",
    "document",
    "mask",
    "keypoints",
    "action",
    "3d",
    "embedding",
    "tabular",
    "timeseries",
    "rl-state",
    "any",
    "unknown",
];

/// Normalize a task signature or source task label into canonical shape.
pub fn normalize_task_signature(raw: &str) -> NormalizedTaskSignature {
    let normalized = raw.trim().to_lowercase();
    if normalized.is_empty() {
        return NormalizedTaskSignature::error_invalid_signature();
    }

    let normalized = hf_task_to_signature(&normalized).unwrap_or(normalized);
    let normalized = normalize_direction_separator(&normalized);

    let Some((lhs, rhs)) = split_directional(&normalized) else {
        return NormalizedTaskSignature::error_invalid_signature();
    };

    let mut warnings = Vec::new();
    let (input_modalities, input_unknown) = normalize_modalities(lhs);
    if !input_unknown.is_empty() {
        warnings.push(format!(
            "unresolved-input-tokens:{}",
            input_unknown.join(",")
        ));
    }

    let (output_modalities, output_unknown) = normalize_modalities(rhs);
    if !output_unknown.is_empty() {
        warnings.push(format!(
            "unresolved-output-tokens:{}",
            output_unknown.join(",")
        ));
    }

    let mut status = TaskNormalizationStatus::Ok;
    let inputs = if input_modalities.is_empty() {
        status = TaskNormalizationStatus::Warning;
        warnings.push("missing-input-modalities".to_string());
        vec!["unknown".to_string()]
    } else {
        input_modalities
    };
    let outputs = if output_modalities.is_empty() {
        status = TaskNormalizationStatus::Warning;
        warnings.push("missing-output-modalities".to_string());
        vec!["unknown".to_string()]
    } else {
        output_modalities
    };

    if !warnings.is_empty() && status == TaskNormalizationStatus::Ok {
        status = TaskNormalizationStatus::Warning;
    }

    let signature_key = format!("{}->{}", inputs.join("+"), outputs.join("+"));

    NormalizedTaskSignature {
        signature_key,
        input_modalities: inputs,
        output_modalities: outputs,
        normalization_status: status,
        normalization_warnings: warnings,
    }
}

fn normalize_direction_separator(input: &str) -> String {
    let mut out = input
        .replace('→', "->")
        .replace("=>", "->")
        .replace(" to ", "->");

    // Support compact forms like "speech2text"
    if let Ok(compact_two) = Regex::new(r"([a-z])2([a-z])") {
        out = compact_two.replace_all(&out, "$1->$2").to_string();
    }

    // Collapse whitespace around explicit arrows
    if let Ok(arrow_spaces) = Regex::new(r"\s*->\s*") {
        out = arrow_spaces.replace_all(&out, "->").to_string();
    }

    out.trim().to_string()
}

fn split_directional(input: &str) -> Option<(&str, &str)> {
    let idx = input.find("->")?;
    let (lhs, rhs_with_arrow) = input.split_at(idx);
    let rhs = rhs_with_arrow.strip_prefix("->")?;
    Some((lhs.trim(), rhs.trim()))
}

fn normalize_modalities(side: &str) -> (Vec<String>, Vec<String>) {
    let mut normalized = side.replace(" and ", "+");
    normalized = normalized
        .replace(',', "+")
        .replace('&', "+")
        .replace('/', "+");

    let mut known = HashSet::new();
    let mut unknown = Vec::new();
    for token in normalized.split('+') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if let Some(modality) = normalize_modality_token(token) {
            known.insert(modality.to_string());
        } else {
            unknown.push(token.to_string());
        }
    }

    let mut known_vec: Vec<String> = known.into_iter().collect();
    known_vec.sort_by_key(|m| modality_rank(m.as_str()));

    (known_vec, unknown)
}

fn modality_rank(token: &str) -> usize {
    MODALITY_ORDER
        .iter()
        .position(|m| *m == token)
        .unwrap_or(MODALITY_ORDER.len())
}

fn normalize_modality_token(token: &str) -> Option<&'static str> {
    match token.trim().to_lowercase().as_str() {
        "text" | "txt" | "natural-language" | "language" | "nlp" | "label" | "labels" => {
            Some("text")
        }
        "image" | "img" | "images" | "vision-image" | "picture" | "pictures" => Some("image"),
        "audio" | "speech" | "voice" | "sound" | "music" => Some("audio"),
        "video" | "vid" | "movie" | "movies" | "clip" => Some("video"),
        "document" | "doc" | "docs" | "document-image" | "pdf" => Some("document"),
        "mask" | "segmentation-mask" | "binary-mask" | "instance-mask" => Some("mask"),
        "keypoints" | "pose" | "skeleton" | "landmarks" => Some("keypoints"),
        "action" | "action-label" | "activity" => Some("action"),
        "3d" | "mesh" | "pointcloud" | "point-cloud" => Some("3d"),
        "embedding" | "embeddings" | "vector" | "vectors" | "feature" | "features" => {
            Some("embedding")
        }
        "tabular" | "table" | "tables" | "csv" | "structured" => Some("tabular"),
        "timeseries" | "time-series" | "series" | "temporal" => Some("timeseries"),
        "rl-state" | "reinforcement-learning" | "rl" | "state" => Some("rl-state"),
        "any" | "any-to-any" | "multi-any" => Some("any"),
        "unknown" => Some("unknown"),
        _ => None,
    }
}

fn hf_task_to_signature(input: &str) -> Option<String> {
    let signature = match input {
        "text-generation"
        | "text2text-generation"
        | "fill-mask"
        | "summarization"
        | "translation"
        | "text-classification"
        | "token-classification"
        | "question-answering"
        | "table-question-answering"
        | "conversational"
        | "sentence-similarity" => "text->text",
        "text-to-image" | "unconditional-image-generation" | "image-generation" => "text->image",
        "image-to-image" | "image-inpainting" => "image->image",
        "text-image-to-image" => "text+image->image",
        "image-to-text" => "image->text",
        "visual-question-answering" => "text+image->text",
        "document-question-answering" => "text+document->text",
        "video-text-to-text" | "video-question-answering" => "text+video->text",
        "text-to-video" => "text->video",
        "automatic-speech-recognition" | "speech-to-text" => "audio->text",
        "audio-to-audio" => "audio->audio",
        "audio-classification" => "audio->text",
        "text-to-audio" | "text-to-speech" => "text->audio",
        "image-classification" | "zero-shot-image-classification" => "image->text",
        "object-detection" | "zero-shot-object-detection" => "image->text",
        "image-segmentation" => "image->mask",
        "depth-estimation" => "image->image",
        "feature-extraction" => "text->embedding",
        "text-to-3d" => "text->3d",
        "image-to-3d" => "image->3d",
        "any-to-any" => "any->any",
        _ => return None,
    };

    Some(signature.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_direction_variants() {
        assert_eq!(
            normalize_task_signature("Text to Image").signature_key,
            "text->image"
        );
        assert_eq!(
            normalize_task_signature("text => image").signature_key,
            "text->image"
        );
        assert_eq!(
            normalize_task_signature("text → image").signature_key,
            "text->image"
        );
    }

    #[test]
    fn normalizes_multi_modality_separators() {
        assert_eq!(
            normalize_task_signature("text,image -> image").signature_key,
            "text+image->image"
        );
        assert_eq!(
            normalize_task_signature("video and text -> text").signature_key,
            "text+video->text"
        );
        assert_eq!(
            normalize_task_signature("text/image -> image").signature_key,
            "text+image->image"
        );
    }

    #[test]
    fn expands_aliases() {
        assert_eq!(
            normalize_task_signature("speech2text").signature_key,
            "audio->text"
        );
        assert_eq!(
            normalize_task_signature("doc -> labels").signature_key,
            "document->text"
        );
    }

    #[test]
    fn deduplicates_modalities() {
        assert_eq!(
            normalize_task_signature("text+text->image").signature_key,
            "text->image"
        );
    }

    #[test]
    fn unresolved_tokens_generate_warning() {
        let result = normalize_task_signature("text+weirdmodality->image");
        assert_eq!(
            result.normalization_status,
            TaskNormalizationStatus::Warning
        );
        assert!(result
            .normalization_warnings
            .iter()
            .any(|w| w.starts_with("unresolved-input-tokens:")));
    }

    #[test]
    fn invalid_signature_is_error() {
        let result = normalize_task_signature("unknownformat");
        assert_eq!(result.signature_key, "unknown->unknown");
        assert_eq!(result.normalization_status, TaskNormalizationStatus::Error);
    }

    #[test]
    fn idempotent_normalization() {
        let once = normalize_task_signature("text,image -> image");
        let twice = normalize_task_signature(&once.signature_key);
        assert_eq!(once.signature_key, twice.signature_key);
        assert_eq!(once.input_modalities, twice.input_modalities);
        assert_eq!(once.output_modalities, twice.output_modalities);
    }

    #[test]
    fn normalizes_any_and_rl_signatures() {
        assert_eq!(
            normalize_task_signature("any-to-any").signature_key,
            "any->any"
        );
        assert_eq!(
            normalize_task_signature("state->action").signature_key,
            "rl-state->action"
        );
    }
}
