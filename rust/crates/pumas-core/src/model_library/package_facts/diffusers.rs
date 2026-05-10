use crate::error::{PumasError, Result};
use crate::model_library::external_assets::normalized_component_relative_path;
use crate::models::{
    DiffusersComponentFacts, DiffusersComponentRole, DiffusersPackageEvidence,
    ImageGenerationFamilyEvidence, ImageGenerationFamilyEvidenceSource, ImageGenerationFamilyLabel,
    ModelPackageDiagnostic, PackageFactStatus, PackageFactValueSource, TaskEvidence,
};
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const MODEL_INDEX_PATH: &str = "model_index.json";
const MAX_DIFFUSERS_JSON_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiffusersPackageExtraction {
    pub evidence: Option<DiffusersPackageEvidence>,
    pub diagnostics: Vec<ModelPackageDiagnostic>,
}

pub(crate) async fn diffusers_package_evidence(
    bundle_root: &Path,
) -> Result<DiffusersPackageExtraction> {
    let model_index_path = bundle_root.join(MODEL_INDEX_PATH);
    if !tokio::fs::try_exists(&model_index_path).await? {
        return Ok(DiffusersPackageExtraction {
            evidence: None,
            diagnostics: Vec::new(),
        });
    }

    let parse_result = read_json(model_index_path).await;
    let model_index = match parse_result {
        Ok(value) => value,
        Err(err) => {
            let message = format!("failed to parse model_index.json: {}", err);
            return Ok(DiffusersPackageExtraction {
                evidence: Some(DiffusersPackageEvidence {
                    status: PackageFactStatus::Invalid,
                    ..DiffusersPackageEvidence::default()
                }),
                diagnostics: vec![ModelPackageDiagnostic {
                    code: "diffusers_model_index_invalid".to_string(),
                    message,
                    path: Some(MODEL_INDEX_PATH.to_string()),
                }],
            });
        }
    };

    if !model_index.is_object() {
        return Ok(DiffusersPackageExtraction {
            evidence: Some(DiffusersPackageEvidence {
                status: PackageFactStatus::Invalid,
                ..DiffusersPackageEvidence::default()
            }),
            diagnostics: vec![ModelPackageDiagnostic {
                code: "diffusers_model_index_invalid".to_string(),
                message: "model_index.json must be a JSON object".to_string(),
                path: Some(MODEL_INDEX_PATH.to_string()),
            }],
        });
    }

    let pipeline_class = string_field(&model_index, "_class_name");
    let diffusers_version = string_field(&model_index, "_diffusers_version");
    let name_or_path = string_field(&model_index, "_name_or_path");
    let mut components = vec![DiffusersComponentFacts {
        role: DiffusersComponentRole::PipelineIndex,
        status: PackageFactStatus::Present,
        relative_path: Some(MODEL_INDEX_PATH.to_string()),
        source_library: None,
        class_name: pipeline_class.clone(),
        config_path: Some(MODEL_INDEX_PATH.to_string()),
        config_model_type: None,
    }];

    for component in component_specs() {
        let Some(value) = model_index.get(component.key) else {
            continue;
        };
        let (source_library, class_name) = component_reference(value);
        let config =
            read_component_config(bundle_root, component.key, component.config_candidates).await?;

        components.push(DiffusersComponentFacts {
            role: component.role,
            status: PackageFactStatus::Present,
            relative_path: Some(component.key.to_string()),
            source_library,
            class_name,
            config_path: config
                .as_ref()
                .map(|config| format!("{}/{}", component.key, config.relative_file)),
            config_model_type: config.and_then(|config| config.model_type),
        });
    }

    Ok(DiffusersPackageExtraction {
        evidence: Some(DiffusersPackageEvidence {
            status: PackageFactStatus::Present,
            pipeline_class: pipeline_class.clone(),
            diffusers_version,
            name_or_path,
            task: task_for_pipeline(pipeline_class.as_deref()),
            family_evidence: family_evidence(&model_index, pipeline_class.as_deref()),
            components,
        }),
        diagnostics: Vec::new(),
    })
}

#[derive(Clone, Copy)]
struct ComponentSpec {
    key: &'static str,
    role: DiffusersComponentRole,
    config_candidates: &'static [&'static str],
}

struct ComponentConfig {
    relative_file: &'static str,
    model_type: Option<String>,
}

fn component_specs() -> &'static [ComponentSpec] {
    &[
        ComponentSpec {
            key: "scheduler",
            role: DiffusersComponentRole::Scheduler,
            config_candidates: &["scheduler_config.json", "config.json"],
        },
        ComponentSpec {
            key: "unet",
            role: DiffusersComponentRole::Unet,
            config_candidates: &["config.json"],
        },
        ComponentSpec {
            key: "transformer",
            role: DiffusersComponentRole::Transformer,
            config_candidates: &["config.json"],
        },
        ComponentSpec {
            key: "vae",
            role: DiffusersComponentRole::Vae,
            config_candidates: &["config.json"],
        },
        ComponentSpec {
            key: "text_encoder",
            role: DiffusersComponentRole::TextEncoder,
            config_candidates: &["config.json"],
        },
        ComponentSpec {
            key: "text_encoder_2",
            role: DiffusersComponentRole::TextEncoder2,
            config_candidates: &["config.json"],
        },
        ComponentSpec {
            key: "text_encoder_3",
            role: DiffusersComponentRole::TextEncoder3,
            config_candidates: &["config.json"],
        },
        ComponentSpec {
            key: "tokenizer",
            role: DiffusersComponentRole::Tokenizer,
            config_candidates: &["tokenizer_config.json", "config.json"],
        },
        ComponentSpec {
            key: "tokenizer_2",
            role: DiffusersComponentRole::Tokenizer2,
            config_candidates: &["tokenizer_config.json", "config.json"],
        },
        ComponentSpec {
            key: "image_processor",
            role: DiffusersComponentRole::ImageProcessor,
            config_candidates: &["preprocessor_config.json", "config.json"],
        },
        ComponentSpec {
            key: "feature_extractor",
            role: DiffusersComponentRole::ImageProcessor,
            config_candidates: &["preprocessor_config.json", "config.json"],
        },
        ComponentSpec {
            key: "processor",
            role: DiffusersComponentRole::Processor,
            config_candidates: &[
                "processor_config.json",
                "preprocessor_config.json",
                "config.json",
            ],
        },
        ComponentSpec {
            key: "controlnet",
            role: DiffusersComponentRole::Controlnet,
            config_candidates: &["config.json"],
        },
        ComponentSpec {
            key: "adapter",
            role: DiffusersComponentRole::Adapter,
            config_candidates: &["config.json"],
        },
    ]
}

async fn read_component_config(
    bundle_root: &Path,
    component_key: &'static str,
    candidates: &'static [&'static str],
) -> Result<Option<ComponentConfig>> {
    for candidate in candidates {
        let relative_path = bounded_component_config_path(component_key, candidate)?;
        let path = bundle_root.join(&relative_path);
        if !tokio::fs::try_exists(&path).await? {
            continue;
        }
        let model_type = read_json(path)
            .await
            .ok()
            .and_then(|config| string_field(&config, "model_type"));
        return Ok(Some(ComponentConfig {
            relative_file: candidate,
            model_type,
        }));
    }
    Ok(None)
}

fn bounded_component_config_path(component_key: &str, config_file: &str) -> Result<PathBuf> {
    let component_path = normalized_component_relative_path(component_key)?;
    let config_path = PathBuf::from(config_file);
    if config_path.is_absolute()
        || config_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(PumasError::Validation {
            field: "config_file".to_string(),
            message: "component config path must remain inside the bundle root".to_string(),
        });
    }
    Ok(component_path.join(config_path))
}

async fn read_json(path: PathBuf) -> Result<Value> {
    tokio::task::spawn_blocking(move || {
        let raw = read_bounded_utf8_file(&path, MAX_DIFFUSERS_JSON_BYTES)?;
        serde_json::from_str::<Value>(&raw).map_err(|err| PumasError::Other(err.to_string()))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join diffusers JSON parse: {}", err)))?
}

fn read_bounded_utf8_file(path: &Path, max_bytes: u64) -> Result<String> {
    let metadata = std::fs::metadata(path).map_err(|err| PumasError::io_with_path(err, path))?;
    if metadata.len() > max_bytes {
        return Err(PumasError::Other(format!(
            "{} exceeds bounded Diffusers JSON limit of {} bytes",
            path.display(),
            max_bytes
        )));
    }

    let mut file = File::open(path).map_err(|err| PumasError::io_with_path(err, path))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| PumasError::io_with_path(err, path))?;
    if bytes.len() as u64 > max_bytes {
        return Err(PumasError::Other(format!(
            "{} exceeds bounded Diffusers JSON limit of {} bytes",
            path.display(),
            max_bytes
        )));
    }

    String::from_utf8(bytes)
        .map_err(|_| PumasError::Other(format!("{} is not valid UTF-8", path.display())))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn component_reference(value: &Value) -> (Option<String>, Option<String>) {
    match value.as_array() {
        Some(values) if values.len() >= 2 => (
            values
                .first()
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string),
            values
                .get(1)
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string),
        ),
        _ => (None, None),
    }
}

fn family_evidence(
    model_index: &Value,
    pipeline_class: Option<&str>,
) -> Vec<ImageGenerationFamilyEvidence> {
    let mut evidence = Vec::new();
    if let Some(pipeline_class) = pipeline_class {
        evidence.push(ImageGenerationFamilyEvidence {
            family: family_from_pipeline_class(pipeline_class),
            source: ImageGenerationFamilyEvidenceSource::PipelineClass,
            value_source: PackageFactValueSource::Config,
            source_path: Some(MODEL_INDEX_PATH.to_string()),
            message: Some(pipeline_class.to_string()),
        });
    }

    let mut component_families = Vec::new();
    for component in component_specs() {
        let Some(value) = model_index.get(component.key) else {
            continue;
        };
        let (_library, Some(class_name)) = component_reference(value) else {
            continue;
        };
        let Some(family) = family_from_component_class(&class_name) else {
            continue;
        };
        if !component_families.contains(&(family, class_name.clone())) {
            component_families.push((family, class_name));
        }
    }

    for (family, class_name) in component_families {
        evidence.push(ImageGenerationFamilyEvidence {
            family,
            source: ImageGenerationFamilyEvidenceSource::ModelIndexComponent,
            value_source: PackageFactValueSource::Config,
            source_path: Some(MODEL_INDEX_PATH.to_string()),
            message: Some(class_name),
        });
    }

    evidence
}

fn family_from_pipeline_class(pipeline_class: &str) -> ImageGenerationFamilyLabel {
    match pipeline_class {
        "StableDiffusionPipeline" => ImageGenerationFamilyLabel::StableDiffusion,
        "StableDiffusionXLPipeline" => ImageGenerationFamilyLabel::StableDiffusionXl,
        "FluxPipeline" => ImageGenerationFamilyLabel::Flux,
        "Flux2Pipeline" => ImageGenerationFamilyLabel::Flux2,
        "QwenImagePipeline" => ImageGenerationFamilyLabel::QwenImage,
        "LuminaPipeline" | "LuminaText2ImgPipeline" => ImageGenerationFamilyLabel::LuminaImage,
        "GlmImagePipeline" | "GLMImagePipeline" | "GLM4ImagePipeline" => {
            ImageGenerationFamilyLabel::GlmImage
        }
        "ZImagePipeline" => ImageGenerationFamilyLabel::ZImage,
        _ => ImageGenerationFamilyLabel::Unknown,
    }
}

fn family_from_component_class(class_name: &str) -> Option<ImageGenerationFamilyLabel> {
    match class_name {
        "FluxTransformer2DModel" => Some(ImageGenerationFamilyLabel::Flux),
        "QwenImageTransformer2DModel" => Some(ImageGenerationFamilyLabel::QwenImage),
        "LuminaNextDiT2DModel" | "Lumina2Transformer2DModel" => {
            Some(ImageGenerationFamilyLabel::LuminaImage)
        }
        "GlmImageTransformer2DModel" | "GLMImageTransformer2DModel" => {
            Some(ImageGenerationFamilyLabel::GlmImage)
        }
        "ZImageTransformer2DModel" => Some(ImageGenerationFamilyLabel::ZImage),
        _ => None,
    }
}

fn task_for_pipeline(pipeline_class: Option<&str>) -> TaskEvidence {
    let Some(pipeline_class) = pipeline_class else {
        return TaskEvidence::default();
    };
    if family_from_pipeline_class(pipeline_class) == ImageGenerationFamilyLabel::Unknown {
        return TaskEvidence::default();
    }
    TaskEvidence {
        pipeline_tag: Some("text-to-image".to_string()),
        task_type_primary: Some("image_generation".to_string()),
        input_modalities: vec!["text".to_string()],
        output_modalities: vec!["image".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn extracts_stable_diffusion_model_index_facts() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join(MODEL_INDEX_PATH),
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "_diffusers_version": "0.32.0",
  "_name_or_path": "runwayml/stable-diffusion-v1-5",
  "scheduler": ["diffusers", "EulerDiscreteScheduler"],
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"]
}"#,
        )
        .unwrap();

        let extraction = diffusers_package_evidence(temp.path()).await.unwrap();
        assert!(extraction.diagnostics.is_empty());
        let evidence = extraction.evidence.unwrap();
        assert_eq!(evidence.status, PackageFactStatus::Present);
        assert_eq!(
            evidence.pipeline_class.as_deref(),
            Some("StableDiffusionPipeline")
        );
        assert_eq!(evidence.diffusers_version.as_deref(), Some("0.32.0"));
        assert_eq!(
            evidence.name_or_path.as_deref(),
            Some("runwayml/stable-diffusion-v1-5")
        );
        assert_eq!(
            evidence.family_evidence[0].family,
            ImageGenerationFamilyLabel::StableDiffusion
        );
        assert!(evidence.components.iter().any(|component| {
            component.role == DiffusersComponentRole::Unet
                && component.source_library.as_deref() == Some("diffusers")
                && component.class_name.as_deref() == Some("UNet2DConditionModel")
        }));
    }

    #[tokio::test]
    async fn unknown_pipeline_does_not_guess_from_name_or_path() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join(MODEL_INDEX_PATH),
            r#"{
  "_class_name": "UnknownPipeline",
  "_name_or_path": "vendor/glm-image-looking-name",
  "transformer": ["diffusers", "PlainTransformer2DModel"]
}"#,
        )
        .unwrap();

        let extraction = diffusers_package_evidence(temp.path()).await.unwrap();
        let evidence = extraction.evidence.unwrap();
        assert_eq!(
            evidence.name_or_path.as_deref(),
            Some("vendor/glm-image-looking-name")
        );
        assert_eq!(evidence.family_evidence.len(), 1);
        assert_eq!(
            evidence.family_evidence[0].family,
            ImageGenerationFamilyLabel::Unknown
        );
        assert_eq!(
            evidence.family_evidence[0].source,
            ImageGenerationFamilyEvidenceSource::PipelineClass
        );
    }

    #[tokio::test]
    async fn invalid_model_index_returns_invalid_status_and_diagnostic() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join(MODEL_INDEX_PATH), "{ not json").unwrap();

        let extraction = diffusers_package_evidence(temp.path()).await.unwrap();
        let evidence = extraction.evidence.unwrap();
        assert_eq!(evidence.status, PackageFactStatus::Invalid);
        assert_eq!(extraction.diagnostics.len(), 1);
        assert_eq!(
            extraction.diagnostics[0].code,
            "diffusers_model_index_invalid"
        );
        assert_eq!(
            extraction.diagnostics[0].path.as_deref(),
            Some(MODEL_INDEX_PATH)
        );
    }

    #[tokio::test]
    async fn oversized_model_index_returns_invalid_status_and_diagnostic() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join(MODEL_INDEX_PATH),
            vec![b' '; (MAX_DIFFUSERS_JSON_BYTES + 1) as usize],
        )
        .unwrap();

        let extraction = diffusers_package_evidence(temp.path()).await.unwrap();
        let evidence = extraction.evidence.unwrap();
        assert_eq!(evidence.status, PackageFactStatus::Invalid);
        assert_eq!(extraction.diagnostics.len(), 1);
        assert_eq!(
            extraction.diagnostics[0].code,
            "diffusers_model_index_invalid"
        );
        assert!(extraction.diagnostics[0]
            .message
            .contains("bounded Diffusers JSON limit"));
    }

    #[tokio::test]
    async fn extracts_bounded_nested_component_config_model_types() {
        let temp = tempdir().unwrap();
        fs::write(
            temp.path().join(MODEL_INDEX_PATH),
            r#"{
  "_class_name": "FluxPipeline",
  "scheduler": ["diffusers", "FlowMatchEulerDiscreteScheduler"],
  "transformer": ["diffusers", "FluxTransformer2DModel"],
  "vae": ["diffusers", "AutoencoderKL"],
  "text_encoder": ["transformers", "CLIPTextModel"],
  "tokenizer": ["transformers", "CLIPTokenizer"],
  "image_processor": ["transformers", "CLIPImageProcessor"]
}"#,
        )
        .unwrap();
        write_json(
            temp.path(),
            "scheduler/scheduler_config.json",
            r#"{"model_type":"flow_match"}"#,
        );
        write_json(
            temp.path(),
            "transformer/config.json",
            r#"{"model_type":"flux"}"#,
        );
        write_json(
            temp.path(),
            "vae/config.json",
            r#"{"model_type":"autoencoder_kl"}"#,
        );
        write_json(
            temp.path(),
            "text_encoder/config.json",
            r#"{"model_type":"clip_text_model"}"#,
        );
        write_json(
            temp.path(),
            "tokenizer/tokenizer_config.json",
            r#"{"model_type":"clip_tokenizer"}"#,
        );
        write_json(
            temp.path(),
            "image_processor/preprocessor_config.json",
            r#"{"model_type":"clip_image_processor"}"#,
        );

        let extraction = diffusers_package_evidence(temp.path()).await.unwrap();
        let evidence = extraction.evidence.unwrap();
        assert_component_model_type(
            &evidence.components,
            DiffusersComponentRole::Scheduler,
            "scheduler/scheduler_config.json",
            "flow_match",
        );
        assert_component_model_type(
            &evidence.components,
            DiffusersComponentRole::Transformer,
            "transformer/config.json",
            "flux",
        );
        assert_component_model_type(
            &evidence.components,
            DiffusersComponentRole::Vae,
            "vae/config.json",
            "autoencoder_kl",
        );
        assert_component_model_type(
            &evidence.components,
            DiffusersComponentRole::TextEncoder,
            "text_encoder/config.json",
            "clip_text_model",
        );
        assert_component_model_type(
            &evidence.components,
            DiffusersComponentRole::Tokenizer,
            "tokenizer/tokenizer_config.json",
            "clip_tokenizer",
        );
        assert_component_model_type(
            &evidence.components,
            DiffusersComponentRole::ImageProcessor,
            "image_processor/preprocessor_config.json",
            "clip_image_processor",
        );
        assert!(evidence.family_evidence.iter().any(|item| {
            item.family == ImageGenerationFamilyLabel::Flux
                && item.source == ImageGenerationFamilyEvidenceSource::ModelIndexComponent
        }));
    }

    #[test]
    fn rejects_component_config_path_escapes() {
        let error = bounded_component_config_path("../escape", "config.json").unwrap_err();
        assert!(error
            .to_string()
            .contains("component path must remain inside the bundle root"));

        let error = bounded_component_config_path("scheduler", "../config.json").unwrap_err();
        assert!(error
            .to_string()
            .contains("component config path must remain inside the bundle root"));
    }

    fn write_json(root: &Path, relative_path: &str, contents: &str) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn assert_component_model_type(
        components: &[DiffusersComponentFacts],
        role: DiffusersComponentRole,
        config_path: &str,
        model_type: &str,
    ) {
        let component = components
            .iter()
            .find(|component| component.role == role)
            .unwrap();
        assert_eq!(component.config_path.as_deref(), Some(config_path));
        assert_eq!(component.config_model_type.as_deref(), Some(model_type));
    }
}
