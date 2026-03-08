//! External directory-root asset support for the model library.
//!
//! Milestone one supports diffusers-style text-to-image bundles that remain in
//! place on disk while Pumas stores only a metadata/registry artifact under the
//! library root.

use crate::error::{PumasError, Result};
use crate::model_library::naming::normalize_name;
use crate::models::{
    resolve_inference_settings, AssetValidationError, AssetValidationState, BundleFormat,
    ExternalDiffusersImportSpec, ImportState, ModelMetadata, StorageKind,
};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

pub const MODEL_EXECUTION_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub(crate) struct DiffusersValidationResult {
    pub entry_path: PathBuf,
    pub pipeline_class: Option<String>,
    pub validation_state: AssetValidationState,
    pub validation_errors: Vec<AssetValidationError>,
}

pub(crate) struct DiffusersBundleMetadataSpec<'a> {
    pub family: &'a str,
    pub official_name: &'a str,
    pub repo_id: Option<&'a str>,
    pub tags: Option<&'a [String]>,
    pub source_path: &'a Path,
    pub storage_kind: StorageKind,
    pub match_source: &'a str,
    pub classification_source: &'a str,
    pub expected_files: Option<&'a [String]>,
    pub pipeline_tag: Option<&'a str>,
}

pub(crate) fn is_external_reference(metadata: &ModelMetadata) -> bool {
    metadata.storage_kind == Some(StorageKind::ExternalReference)
}

pub(crate) fn is_diffusers_bundle(metadata: &ModelMetadata) -> bool {
    metadata.bundle_format == Some(BundleFormat::DiffusersDirectory)
}

pub(crate) fn is_external_diffusers_bundle(metadata: &ModelMetadata) -> bool {
    is_external_reference(metadata) && is_diffusers_bundle(metadata)
}

pub(crate) fn validate_diffusers_directory_for_import(source_path: &Path) -> DiffusersValidationResult {
    validate_diffusers_directory(source_path, false)
}

pub(crate) fn revalidate_diffusers_directory(entry_path: &Path) -> DiffusersValidationResult {
    validate_diffusers_directory(entry_path, true)
}

pub(crate) fn refresh_external_metadata_validation(metadata: &mut ModelMetadata) -> bool {
    if !is_external_diffusers_bundle(metadata) {
        return false;
    }

    let entry_path = metadata
        .entry_path
        .clone()
        .or_else(|| metadata.source_path.clone());
    let Some(entry_path) = entry_path else {
        let next_errors = vec![AssetValidationError {
            code: "missing_entry_path".to_string(),
            message: "external asset metadata is missing entry_path".to_string(),
            path: None,
        }];
        let changed = metadata.validation_state != Some(AssetValidationState::Invalid)
            || metadata.validation_errors.as_ref() != Some(&next_errors);
        metadata.validation_state = Some(AssetValidationState::Invalid);
        metadata.validation_errors = Some(next_errors);
        return changed;
    };

    let validation = revalidate_diffusers_directory(Path::new(&entry_path));
    apply_validation_result(metadata, &validation)
}

pub(crate) fn build_external_diffusers_metadata(
    spec: &ExternalDiffusersImportSpec,
    validation: &DiffusersValidationResult,
    model_id: &str,
) -> ModelMetadata {
    let source_path = Path::new(&spec.source_path);
    let metadata_spec = DiffusersBundleMetadataSpec {
        family: &spec.family,
        official_name: &spec.official_name,
        repo_id: spec.repo_id.as_deref(),
        tags: spec.tags.as_deref(),
        source_path,
        storage_kind: StorageKind::ExternalReference,
        match_source: "external_reference",
        classification_source: "external-diffusers-import",
        expected_files: None,
        pipeline_tag: Some("text-to-image"),
    };
    build_diffusers_bundle_metadata(&metadata_spec, validation, model_id)
}

pub(crate) fn build_diffusers_bundle_metadata(
    spec: &DiffusersBundleMetadataSpec<'_>,
    validation: &DiffusersValidationResult,
    model_id: &str,
) -> ModelMetadata {
    let now = chrono::Utc::now().to_rfc3339();
    let cleaned_name = normalize_name(spec.official_name);
    let validation_errors = if validation.validation_errors.is_empty() {
        None
    } else {
        Some(validation.validation_errors.clone())
    };
    let mut metadata = ModelMetadata {
        schema_version: Some(2),
        model_id: Some(model_id.to_string()),
        family: Some(spec.family.to_string()),
        model_type: Some("diffusion".to_string()),
        official_name: Some(spec.official_name.to_string()),
        cleaned_name: Some(cleaned_name),
        tags: spec.tags.map(|tags| tags.to_vec()),
        repo_id: spec.repo_id.map(str::to_string),
        source_path: Some(spec.source_path.display().to_string()),
        entry_path: Some(validation.entry_path.display().to_string()),
        storage_kind: Some(spec.storage_kind),
        bundle_format: Some(BundleFormat::DiffusersDirectory),
        pipeline_class: validation.pipeline_class.clone(),
        import_state: Some(match validation.validation_state {
            AssetValidationState::Valid => ImportState::Ready,
            _ => ImportState::Failed,
        }),
        validation_state: Some(validation.validation_state),
        validation_errors,
        expected_files: spec.expected_files.map(|files| files.to_vec()),
        pipeline_tag: spec.pipeline_tag.map(str::to_string),
        task_type_primary: Some("text-to-image".to_string()),
        input_modalities: Some(vec!["text".to_string()]),
        output_modalities: Some(vec!["image".to_string()]),
        task_classification_source: Some(spec.classification_source.to_string()),
        task_classification_confidence: Some(1.0),
        model_type_resolution_source: Some(spec.classification_source.to_string()),
        model_type_resolution_confidence: Some(1.0),
        recommended_backend: Some("diffusers".to_string()),
        runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
        requires_custom_code: Some(false),
        metadata_needs_review: Some(false),
        review_reasons: Some(Vec::new()),
        review_status: Some("pending".to_string()),
        match_source: Some(spec.match_source.to_string()),
        added_date: Some(now.clone()),
        updated_date: Some(now),
        size_bytes: Some(calculate_directory_size(&validation.entry_path)),
        license_status: Some("license_unknown".to_string()),
        ..Default::default()
    };

    metadata.inference_settings = resolve_inference_settings(&metadata, "diffusers");
    metadata
}

fn apply_validation_result(metadata: &mut ModelMetadata, validation: &DiffusersValidationResult) -> bool {
    let next_errors = if validation.validation_errors.is_empty() {
        None
    } else {
        Some(validation.validation_errors.clone())
    };
    let next_pipeline_class = validation.pipeline_class.clone();
    let next_entry_path = Some(validation.entry_path.display().to_string());

    let changed = metadata.validation_state != Some(validation.validation_state)
        || metadata.validation_errors != next_errors
        || metadata.pipeline_class != next_pipeline_class
        || metadata.entry_path != next_entry_path;

    metadata.validation_state = Some(validation.validation_state);
    metadata.validation_errors = next_errors;
    metadata.pipeline_class = next_pipeline_class;
    metadata.entry_path = next_entry_path;

    if changed {
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
    }

    changed
}

fn validate_diffusers_directory(path: &Path, allow_degraded: bool) -> DiffusersValidationResult {
    let fallback_entry_path = path.to_path_buf();
    let canonical_entry_path = match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(err) => {
            return DiffusersValidationResult {
                entry_path: fallback_entry_path,
                pipeline_class: None,
                validation_state: if allow_degraded {
                    AssetValidationState::Degraded
                } else {
                    AssetValidationState::Invalid
                },
                validation_errors: vec![AssetValidationError {
                    code: "path_not_found".to_string(),
                    message: format!("could not access external bundle root: {}", err),
                    path: Some(path.display().to_string()),
                }],
            };
        }
    };

    if !canonical_entry_path.is_dir() {
        return DiffusersValidationResult {
            entry_path: canonical_entry_path.clone(),
            pipeline_class: None,
            validation_state: if allow_degraded {
                AssetValidationState::Degraded
            } else {
                AssetValidationState::Invalid
            },
            validation_errors: vec![AssetValidationError {
                code: "path_not_directory".to_string(),
                message: "external bundle root must be a directory".to_string(),
                path: Some(canonical_entry_path.display().to_string()),
            }],
        };
    }

    let model_index_path = canonical_entry_path.join("model_index.json");
    let model_index_data = match std::fs::read_to_string(&model_index_path) {
        Ok(data) => data,
        Err(err) => {
            return DiffusersValidationResult {
                entry_path: canonical_entry_path.clone(),
                pipeline_class: None,
                validation_state: if allow_degraded {
                    AssetValidationState::Degraded
                } else {
                    AssetValidationState::Invalid
                },
                validation_errors: vec![AssetValidationError {
                    code: "missing_model_index".to_string(),
                    message: format!("missing model_index.json: {}", err),
                    path: Some(model_index_path.display().to_string()),
                }],
            };
        }
    };

    let model_index: Value = match serde_json::from_str(&model_index_data) {
        Ok(json) => json,
        Err(err) => {
            return DiffusersValidationResult {
                entry_path: canonical_entry_path.clone(),
                pipeline_class: None,
                validation_state: if allow_degraded {
                    AssetValidationState::Degraded
                } else {
                    AssetValidationState::Invalid
                },
                validation_errors: vec![AssetValidationError {
                    code: "invalid_model_index_json".to_string(),
                    message: format!("model_index.json is not valid JSON: {}", err),
                    path: Some(model_index_path.display().to_string()),
                }],
            };
        }
    };

    let pipeline_class = model_index
        .get("_class_name")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let Some(pipeline_class) = pipeline_class else {
        return DiffusersValidationResult {
            entry_path: canonical_entry_path.clone(),
            pipeline_class: None,
            validation_state: if allow_degraded {
                AssetValidationState::Degraded
            } else {
                AssetValidationState::Invalid
            },
            validation_errors: vec![AssetValidationError {
                code: "missing_pipeline_class".to_string(),
                message: "model_index.json is missing _class_name".to_string(),
                path: Some(model_index_path.display().to_string()),
            }],
        };
    };

    if !is_supported_text_to_image_pipeline(&pipeline_class) {
        return DiffusersValidationResult {
            entry_path: canonical_entry_path.clone(),
            pipeline_class: Some(pipeline_class),
            validation_state: if allow_degraded {
                AssetValidationState::Degraded
            } else {
                AssetValidationState::Invalid
            },
            validation_errors: vec![AssetValidationError {
                code: "unsupported_pipeline".to_string(),
                message: "bundle is not a supported text-to-image diffusers pipeline".to_string(),
                path: Some(model_index_path.display().to_string()),
            }],
        };
    }

    let mut validation_errors = Vec::new();
    if let Some(components) = model_index.as_object() {
        for (component_name, component_value) in components {
            if component_name.starts_with('_')
                || !is_diffusers_component_entry(component_value)
                || is_optional_component_marker(component_value)
            {
                continue;
            }

            let relative_path = match normalized_component_relative_path(component_name) {
                Ok(path) => path,
                Err(err) => {
                    validation_errors.push(AssetValidationError {
                        code: "path_escape".to_string(),
                        message: err.to_string(),
                        path: Some(component_name.clone()),
                    });
                    continue;
                }
            };

            let candidate_path = canonical_entry_path.join(&relative_path);
            if !candidate_path.exists() {
                validation_errors.push(AssetValidationError {
                    code: "missing_component".to_string(),
                    message: format!("referenced component '{}' is missing", component_name),
                    path: Some(candidate_path.display().to_string()),
                });
                continue;
            }

            match candidate_path.canonicalize() {
                Ok(canonical_component) => {
                    if !canonical_component.starts_with(&canonical_entry_path) {
                        validation_errors.push(AssetValidationError {
                            code: "path_escape".to_string(),
                            message: format!(
                                "component '{}' resolves outside the bundle root",
                                component_name
                            ),
                            path: Some(canonical_component.display().to_string()),
                        });
                    }
                }
                Err(err) => {
                    validation_errors.push(AssetValidationError {
                        code: "component_unreadable".to_string(),
                        message: format!(
                            "could not access referenced component '{}': {}",
                            component_name, err
                        ),
                        path: Some(candidate_path.display().to_string()),
                    });
                }
            }
        }
    }

    let validation_state = if validation_errors.is_empty() {
        AssetValidationState::Valid
    } else if allow_degraded {
        AssetValidationState::Degraded
    } else {
        AssetValidationState::Invalid
    };

    DiffusersValidationResult {
        entry_path: canonical_entry_path,
        pipeline_class: Some(pipeline_class),
        validation_state,
        validation_errors,
    }
}

pub(crate) fn normalized_component_relative_path(component_name: &str) -> Result<PathBuf> {
    let candidate = PathBuf::from(component_name);
    if candidate.is_absolute() {
        return Err(PumasError::Validation {
            field: "component_name".to_string(),
            message: "component path must be relative to the bundle root".to_string(),
        });
    }

    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err(PumasError::Validation {
            field: "component_name".to_string(),
            message: "component path must remain inside the bundle root".to_string(),
        });
    }

    Ok(candidate)
}

pub(crate) fn is_optional_component_marker(value: &Value) -> bool {
    value.is_null()
        || value
            .as_array()
            .is_some_and(|entries| entries.iter().all(Value::is_null))
}

pub(crate) fn is_diffusers_component_entry(value: &Value) -> bool {
    matches!(value, Value::Array(entries) if !entries.is_empty())
}

pub(crate) fn is_supported_text_to_image_pipeline(class_name: &str) -> bool {
    let normalized = class_name.trim().to_lowercase();
    if !normalized.contains("pipeline") {
        return false;
    }

    for rejected in [
        "img2img",
        "image2image",
        "inpaint",
        "controlnet",
        "refiner",
        "audio",
        "speech",
        "video",
    ] {
        if normalized.contains(rejected) {
            return false;
        }
    }

    true
}

fn calculate_directory_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_valid_bundle() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let bundle_root = temp_dir.path().join("tiny-sd-turbo");
        fs::create_dir_all(bundle_root.join("unet")).unwrap();
        fs::create_dir_all(bundle_root.join("vae")).unwrap();
        fs::create_dir_all(bundle_root.join("text_encoder")).unwrap();
        fs::create_dir_all(bundle_root.join("tokenizer")).unwrap();
        fs::write(
            bundle_root.join("model_index.json"),
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"],
  "text_encoder": ["transformers", "CLIPTextModel"],
  "tokenizer": ["transformers", "CLIPTokenizer"]
}"#,
        )
        .unwrap();

        (temp_dir, bundle_root)
    }

    #[test]
    fn validates_supported_diffusers_bundle() {
        let (_temp_dir, bundle_root) = create_valid_bundle();

        let result = validate_diffusers_directory_for_import(&bundle_root);

        assert_eq!(result.validation_state, AssetValidationState::Valid);
        assert!(result.validation_errors.is_empty());
        assert_eq!(
            result.pipeline_class.as_deref(),
            Some("StableDiffusionPipeline")
        );
    }

    #[test]
    fn rejects_missing_model_index() {
        let temp_dir = TempDir::new().unwrap();
        let bundle_root = temp_dir.path().join("bundle");
        fs::create_dir_all(&bundle_root).unwrap();

        let result = validate_diffusers_directory_for_import(&bundle_root);

        assert_eq!(result.validation_state, AssetValidationState::Invalid);
        assert_eq!(result.validation_errors[0].code, "missing_model_index");
    }

    #[test]
    fn rejects_missing_referenced_component() {
        let (_temp_dir, bundle_root) = create_valid_bundle();
        fs::remove_dir_all(bundle_root.join("vae")).unwrap();

        let result = validate_diffusers_directory_for_import(&bundle_root);

        assert_eq!(result.validation_state, AssetValidationState::Invalid);
        assert!(
            result
                .validation_errors
                .iter()
                .any(|error| error.code == "missing_component")
        );
    }

    #[test]
    fn ignores_non_component_model_index_fields() {
        let temp_dir = TempDir::new().unwrap();
        let bundle_root = temp_dir.path().join("tiny-sd-turbo");
        fs::create_dir_all(bundle_root.join("scheduler")).unwrap();
        fs::create_dir_all(bundle_root.join("text_encoder")).unwrap();
        fs::create_dir_all(bundle_root.join("tokenizer")).unwrap();
        fs::create_dir_all(bundle_root.join("unet")).unwrap();
        fs::create_dir_all(bundle_root.join("vae")).unwrap();
        fs::write(
            bundle_root.join("model_index.json"),
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "_diffusers_version": "0.32.0",
  "_name_or_path": "stabilityai/sd-turbo",
  "feature_extractor": [null, null],
  "image_encoder": [null, null],
  "requires_safety_checker": true,
  "safety_checker": [null, null],
  "scheduler": ["diffusers", "EulerDiscreteScheduler"],
  "text_encoder": ["transformers", "CLIPTextModel"],
  "tokenizer": ["transformers", "CLIPTokenizer"],
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderTiny"]
}"#,
        )
        .unwrap();

        let result = validate_diffusers_directory_for_import(&bundle_root);

        assert_eq!(result.validation_state, AssetValidationState::Valid);
        assert!(result.validation_errors.is_empty());
    }

    #[test]
    fn revalidation_marks_missing_bundle_as_degraded() {
        let temp_dir = TempDir::new().unwrap();
        let bundle_root = temp_dir.path().join("missing-bundle");

        let result = revalidate_diffusers_directory(&bundle_root);

        assert_eq!(result.validation_state, AssetValidationState::Degraded);
        assert_eq!(result.validation_errors[0].code, "path_not_found");
    }
}
