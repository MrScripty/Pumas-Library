//! Metadata v2 normalization and validation helpers.

use crate::error::{PumasError, Result};
use crate::index::ModelIndex;
use crate::model_library::dependencies::evaluate_binding_pin_requirements;
use crate::model_library::task_signature::CANONICAL_MODALITY_TOKENS;
use crate::models::{DependencyBindingRef, ModelMetadata};

/// Add a review reason and keep canonical lowercase + sorted + deduplicated storage.
pub fn push_review_reason(metadata: &mut ModelMetadata, reason: &str) {
    let mut reasons = metadata.review_reasons.take().unwrap_or_default();
    reasons.push(reason.to_string());
    normalize_review_reasons(&mut reasons);
    metadata.review_reasons = Some(reasons);
}

/// Canonical review-reasons normalization: lowercase, dedupe, lexicographic order.
pub fn normalize_review_reasons(reasons: &mut Vec<String>) {
    for reason in reasons.iter_mut() {
        *reason = reason.trim().to_lowercase();
    }
    reasons.retain(|r| !r.is_empty());
    reasons.sort();
    reasons.dedup();
}

/// Validate core metadata v2 constraints used by importer path.
pub fn validate_metadata_v2(metadata: &ModelMetadata) -> Result<()> {
    validate_metadata_v2_internal(metadata, None)
}

/// Validate core metadata v2 constraints with index-backed dependency reference checks.
pub fn validate_metadata_v2_with_index(metadata: &ModelMetadata, index: &ModelIndex) -> Result<()> {
    validate_metadata_v2_internal(metadata, Some(index))
}

fn validate_metadata_v2_internal(
    metadata: &ModelMetadata,
    index: Option<&ModelIndex>,
) -> Result<()> {
    validate_confidence(
        "task_classification_confidence",
        metadata.task_classification_confidence,
    )?;
    validate_confidence(
        "model_type_resolution_confidence",
        metadata.model_type_resolution_confidence,
    )?;
    validate_dependency_binding_refs(metadata.dependency_bindings.as_ref(), index, metadata)?;

    let strict_v2 = metadata.schema_version.unwrap_or(1) >= 2;
    if !strict_v2 {
        return Ok(());
    }

    let task_type_primary =
        require_non_empty_str("task_type_primary", metadata.task_type_primary.as_deref())?;
    if !is_canonical_task_type(task_type_primary) {
        return Err(PumasError::Validation {
            field: "task_type_primary".to_string(),
            message: "must be a canonical lowercase task tag (or unknown)".to_string(),
        });
    }

    let model_type = require_non_empty_str("model_type", metadata.model_type.as_deref())?;
    if !is_canonical_model_type(model_type) {
        return Err(PumasError::Validation {
            field: "model_type".to_string(),
            message: "must be one of llm, diffusion, embedding, audio, vision, unknown".to_string(),
        });
    }

    require_non_empty_str(
        "task_classification_source",
        metadata.task_classification_source.as_deref(),
    )?;
    require_non_empty_str(
        "model_type_resolution_source",
        metadata.model_type_resolution_source.as_deref(),
    )?;

    if metadata.task_classification_confidence.is_none() {
        return Err(PumasError::Validation {
            field: "task_classification_confidence".to_string(),
            message: "is required for schema_version >= 2".to_string(),
        });
    }
    if metadata.model_type_resolution_confidence.is_none() {
        return Err(PumasError::Validation {
            field: "model_type_resolution_confidence".to_string(),
            message: "is required for schema_version >= 2".to_string(),
        });
    }

    validate_modalities("input_modalities", metadata.input_modalities.as_ref())?;
    validate_modalities("output_modalities", metadata.output_modalities.as_ref())?;
    validate_review_reasons(metadata.review_reasons.as_ref())?;
    validate_custom_code_requirements(metadata)?;

    if task_type_primary == "unknown" {
        validate_unknown_review_bundle(
            metadata,
            "task_type_primary",
            "task_classification_confidence",
            metadata.task_classification_confidence,
        )?;
    }

    if model_type == "unknown" {
        validate_unknown_review_bundle(
            metadata,
            "model_type",
            "model_type_resolution_confidence",
            metadata.model_type_resolution_confidence,
        )?;
    }

    Ok(())
}

fn validate_confidence(field: &str, value: Option<f64>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };

    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(PumasError::Validation {
            field: field.to_string(),
            message: "must be between 0.0 and 1.0".to_string(),
        })
    }
}

fn require_non_empty_str<'a>(field: &str, value: Option<&'a str>) -> Result<&'a str> {
    let value = value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| PumasError::Validation {
            field: field.to_string(),
            message: "is required for schema_version >= 2".to_string(),
        })?;
    Ok(value)
}

fn is_canonical_model_type(value: &str) -> bool {
    matches!(
        value,
        "llm" | "diffusion" | "embedding" | "audio" | "vision" | "unknown"
    )
}

fn is_canonical_task_type(value: &str) -> bool {
    if value == "unknown" {
        return true;
    }
    if value.is_empty() {
        return false;
    }

    let mut prev_dash = false;
    for (idx, ch) in value.chars().enumerate() {
        if ch == '-' {
            if idx == 0 || prev_dash {
                return false;
            }
            prev_dash = true;
            continue;
        }

        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() {
            return false;
        }
        prev_dash = false;
    }
    !prev_dash
}

fn validate_modalities(field: &str, modalities: Option<&Vec<String>>) -> Result<()> {
    let modalities = modalities.ok_or_else(|| PumasError::Validation {
        field: field.to_string(),
        message: "must be non-empty for schema_version >= 2".to_string(),
    })?;

    if modalities.is_empty() {
        return Err(PumasError::Validation {
            field: field.to_string(),
            message: "must be non-empty for schema_version >= 2".to_string(),
        });
    }

    for token in modalities {
        let normalized = token.trim().to_lowercase();
        if normalized != *token {
            return Err(PumasError::Validation {
                field: field.to_string(),
                message: "modality tokens must be lowercase, trimmed canonical values".to_string(),
            });
        }

        if !CANONICAL_MODALITY_TOKENS.contains(&normalized.as_str()) {
            return Err(PumasError::Validation {
                field: field.to_string(),
                message: format!("contains unsupported modality token: {}", token),
            });
        }
    }

    Ok(())
}

fn validate_review_reasons(review_reasons: Option<&Vec<String>>) -> Result<()> {
    let Some(review_reasons) = review_reasons else {
        return Ok(());
    };

    let mut normalized = review_reasons.clone();
    normalize_review_reasons(&mut normalized);
    if *review_reasons != normalized {
        return Err(PumasError::Validation {
            field: "review_reasons".to_string(),
            message: "must be lowercase, deduplicated, and lexicographically sorted".to_string(),
        });
    }

    Ok(())
}

fn validate_custom_code_requirements(metadata: &ModelMetadata) -> Result<()> {
    if metadata.requires_custom_code != Some(true) {
        return Ok(());
    }

    let has_sources = metadata
        .custom_code_sources
        .as_ref()
        .map(|sources| sources.iter().any(|value| !value.trim().is_empty()))
        .unwrap_or(false);

    if has_sources {
        Ok(())
    } else {
        Err(PumasError::Validation {
            field: "custom_code_sources".to_string(),
            message: "must be non-empty when requires_custom_code=true".to_string(),
        })
    }
}

fn validate_dependency_binding_refs(
    bindings: Option<&Vec<DependencyBindingRef>>,
    index: Option<&ModelIndex>,
    metadata: &ModelMetadata,
) -> Result<()> {
    let Some(bindings) = bindings else {
        return Ok(());
    };

    if bindings.is_empty() {
        return Ok(());
    }

    let Some(index) = index else {
        return Err(PumasError::Validation {
            field: "dependency_bindings".to_string(),
            message:
                "index-backed validation context is required for dependency binding references"
                    .to_string(),
        });
    };

    for (idx, binding) in bindings.iter().enumerate() {
        let profile_id = binding
            .profile_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| PumasError::Validation {
                field: format!("dependency_bindings[{idx}].profile_id"),
                message: "is required".to_string(),
            })?;

        let profile_version = binding
            .profile_version
            .ok_or_else(|| PumasError::Validation {
                field: format!("dependency_bindings[{idx}].profile_version"),
                message: "is required".to_string(),
            })?;

        if profile_version <= 0 {
            return Err(PumasError::Validation {
                field: format!("dependency_bindings[{idx}].profile_version"),
                message: "must be a positive integer".to_string(),
            });
        }

        let Some(profile) = index.get_dependency_profile(profile_id, profile_version)? else {
            return Err(PumasError::Validation {
                field: format!("dependency_bindings[{idx}]"),
                message: format!(
                    "references missing dependency profile: {}:{}",
                    profile_id, profile_version
                ),
            });
        };

        let binding_id = binding
            .binding_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        let binding_kind = binding
            .binding_kind
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("required_core");

        let pin_eval = evaluate_binding_pin_requirements(
            binding_id,
            binding_kind,
            binding.backend_key.as_deref(),
            &profile.profile_id,
            profile.profile_version,
            &profile.environment_kind,
            &profile.spec_json,
            Some(metadata),
        );
        if let Some(error_code) = pin_eval.error_code {
            return Err(PumasError::Validation {
                field: format!("dependency_bindings[{idx}]"),
                message: format!(
                    "{}: {}",
                    error_code,
                    pin_eval.message.unwrap_or_else(|| {
                        "dependency binding does not satisfy required pin policy".to_string()
                    })
                ),
            });
        }
    }

    Ok(())
}

fn validate_unknown_review_bundle(
    metadata: &ModelMetadata,
    unknown_field: &str,
    confidence_field: &str,
    confidence: Option<f64>,
) -> Result<()> {
    if metadata.metadata_needs_review != Some(true) {
        return Err(PumasError::Validation {
            field: "metadata_needs_review".to_string(),
            message: format!("must be true when {} is unknown", unknown_field),
        });
    }

    if metadata
        .review_reasons
        .as_ref()
        .map(|reasons| reasons.is_empty())
        .unwrap_or(true)
    {
        return Err(PumasError::Validation {
            field: "review_reasons".to_string(),
            message: format!("must be non-empty when {} is unknown", unknown_field),
        });
    }

    if confidence != Some(0.0) {
        return Err(PumasError::Validation {
            field: confidence_field.to_string(),
            message: format!("must be 0.0 when {} is unknown", unknown_field),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{DependencyProfileRecord, ModelIndex};
    use crate::models::DependencyBindingRef;
    use tempfile::TempDir;

    fn metadata_v2_base() -> ModelMetadata {
        ModelMetadata {
            schema_version: Some(2),
            model_type: Some("llm".to_string()),
            task_type_primary: Some("text-generation".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["text".to_string()]),
            task_classification_source: Some("task-signature-mapping".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("model-type-resolver-arch-rules".to_string()),
            model_type_resolution_confidence: Some(0.7),
            metadata_needs_review: Some(false),
            review_reasons: Some(Vec::new()),
            requires_custom_code: Some(false),
            ..Default::default()
        }
    }

    #[test]
    fn review_reasons_normalization_is_deterministic() {
        let mut reasons = vec![
            " Unknown-Task-Signature ".to_string(),
            "model-type-unresolved".to_string(),
            "unknown-task-signature".to_string(),
        ];
        normalize_review_reasons(&mut reasons);
        assert_eq!(
            reasons,
            vec![
                "model-type-unresolved".to_string(),
                "unknown-task-signature".to_string(),
            ]
        );
    }

    #[test]
    fn validation_rejects_out_of_range_confidence() {
        let mut metadata = ModelMetadata::default();
        metadata.task_classification_confidence = Some(1.2);

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => {
                assert_eq!(field, "task_classification_confidence")
            }
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn unknown_task_requires_zero_confidence() {
        let mut metadata = metadata_v2_base();
        metadata.task_type_primary = Some("unknown".to_string());
        metadata.task_classification_confidence = Some(0.3);
        metadata.metadata_needs_review = Some(true);
        metadata.review_reasons = Some(vec!["unknown-task-signature".to_string()]);

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => {
                assert_eq!(field, "task_classification_confidence")
            }
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn validation_rejects_non_canonical_model_type() {
        let mut metadata = metadata_v2_base();
        metadata.model_type = Some("checkpoint".to_string());

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => assert_eq!(field, "model_type"),
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn validation_rejects_missing_modalities_for_v2() {
        let mut metadata = metadata_v2_base();
        metadata.input_modalities = Some(Vec::new());

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => assert_eq!(field, "input_modalities"),
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn validation_requires_custom_code_sources_when_flagged() {
        let mut metadata = metadata_v2_base();
        metadata.requires_custom_code = Some(true);
        metadata.custom_code_sources = Some(Vec::new());

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => assert_eq!(field, "custom_code_sources"),
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn validation_rejects_non_normalized_review_reasons() {
        let mut metadata = metadata_v2_base();
        metadata.review_reasons = Some(vec![
            "Unknown-Task-Signature".to_string(),
            "unknown-task-signature".to_string(),
        ]);

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => assert_eq!(field, "review_reasons"),
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn unknown_model_requires_review_bundle() {
        let mut metadata = metadata_v2_base();
        metadata.model_type = Some("unknown".to_string());
        metadata.model_type_resolution_confidence = Some(0.0);
        metadata.metadata_needs_review = Some(false);

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => assert_eq!(field, "metadata_needs_review"),
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn dependency_binding_refs_require_existing_profile() {
        let temp = TempDir::new().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();

        let mut metadata = metadata_v2_base();
        metadata.dependency_bindings = Some(vec![DependencyBindingRef {
            profile_id: Some("torch-cu121".to_string()),
            profile_version: Some(1),
            ..Default::default()
        }]);

        let err = validate_metadata_v2_with_index(&metadata, &index).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => {
                assert_eq!(field, "dependency_bindings[0]")
            }
            _ => panic!("expected validation error"),
        }

        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "torch-cu121".to_string(),
                profile_version: 1,
                profile_hash: Some("hash-1".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1+cu121"}
                    ]
                })
                .to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .unwrap();

        validate_metadata_v2_with_index(&metadata, &index).unwrap();
    }

    #[test]
    fn dependency_binding_refs_are_validated_for_schema_v1() {
        let temp = TempDir::new().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();

        let mut metadata = ModelMetadata::default();
        metadata.schema_version = Some(1);
        metadata.dependency_bindings = Some(vec![DependencyBindingRef {
            profile_id: Some("torch-core".to_string()),
            profile_version: Some(1),
            ..Default::default()
        }]);

        let err = validate_metadata_v2_with_index(&metadata, &index).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => assert_eq!(field, "dependency_bindings[0]"),
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn dependency_binding_refs_enforce_pin_compliance() {
        let temp = TempDir::new().unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();

        index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: "torch-core".to_string(),
                profile_version: 1,
                profile_hash: Some("hash-2".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "xformers", "version": "==0.0.30"}
                    ]
                })
                .to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .unwrap();

        let mut metadata = metadata_v2_base();
        metadata.dependency_bindings = Some(vec![DependencyBindingRef {
            binding_id: Some("binding.torch.core".to_string()),
            profile_id: Some("torch-core".to_string()),
            profile_version: Some(1),
            backend_key: Some("pytorch".to_string()),
            binding_kind: Some("required_core".to_string()),
            ..Default::default()
        }]);

        let err = validate_metadata_v2_with_index(&metadata, &index).unwrap_err();
        match err {
            PumasError::Validation { field, message } => {
                assert_eq!(field, "dependency_bindings[0]");
                assert!(message.contains("unpinned_dependency"));
            }
            _ => panic!("expected validation error"),
        }
    }
}
