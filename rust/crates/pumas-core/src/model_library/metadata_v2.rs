//! Metadata v2 normalization and validation helpers.

use crate::error::{PumasError, Result};
use crate::models::ModelMetadata;

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
    validate_confidence(
        "task_classification_confidence",
        metadata.task_classification_confidence,
    )?;
    validate_confidence(
        "model_type_resolution_confidence",
        metadata.model_type_resolution_confidence,
    )?;

    if metadata.task_type_primary.as_deref() == Some("unknown") {
        if metadata.task_classification_confidence != Some(0.0) {
            return Err(PumasError::Validation {
                field: "task_classification_confidence".to_string(),
                message: "must be 0.0 when task_type_primary is unknown".to_string(),
            });
        }
    }

    if metadata.model_type.as_deref() == Some("unknown")
        && metadata.model_type_resolution_confidence != Some(0.0)
    {
        return Err(PumasError::Validation {
            field: "model_type_resolution_confidence".to_string(),
            message: "must be 0.0 when model_type is unknown".to_string(),
        });
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut metadata = ModelMetadata::default();
        metadata.task_type_primary = Some("unknown".to_string());
        metadata.task_classification_confidence = Some(0.3);

        let err = validate_metadata_v2(&metadata).unwrap_err();
        match err {
            PumasError::Validation { field, .. } => {
                assert_eq!(field, "task_classification_confidence")
            }
            _ => panic!("expected validation error"),
        }
    }
}
