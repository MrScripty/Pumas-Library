//! Default inference parameter schemas by model type.
//!
//! Provides sensible defaults based on the model's type, file format,
//! and subtype. Used both during import (to populate metadata.json) and
//! at read time (lazy fallback for pre-existing models without settings).

use serde_json::json;

use super::model::{InferenceParamSchema, ModelMetadata, ParamConstraints, ParamType};

/// Return default inference parameter schemas for a model.
///
/// Returns `None` for model types where inference settings are not applicable.
///
/// # Arguments
///
/// * `model_type` - e.g. "llm", "diffusion", "embedding"
/// * `file_format` - e.g. "gguf", "safetensors", "pickle"
/// * `subtype` - e.g. Some("dllm") for diffusion-decoding LLMs
pub fn default_inference_settings(
    model_type: &str,
    file_format: &str,
    subtype: Option<&str>,
) -> Option<Vec<InferenceParamSchema>> {
    match model_type {
        "llm" => {
            let is_dllm = subtype == Some("dllm");
            let is_gguf = file_format == "gguf";

            let mut params = Vec::new();

            // GGUF-specific: GPU offloading and context size
            if is_gguf {
                params.push(InferenceParamSchema {
                    key: "gpu_layers".into(),
                    label: "GPU Layers".into(),
                    param_type: ParamType::Integer,
                    default: json!(-1),
                    description: Some("Layers to offload to GPU (-1 = all)".into()),
                    constraints: Some(ParamConstraints {
                        min: Some(-1.0),
                        max: None,
                        allowed_values: None,
                    }),
                });
                params.push(InferenceParamSchema {
                    key: "context_length".into(),
                    label: "Context Length".into(),
                    param_type: ParamType::Integer,
                    default: json!(8192),
                    description: Some("Maximum context window size in tokens".into()),
                    constraints: Some(ParamConstraints {
                        min: Some(512.0),
                        max: Some(131072.0),
                        allowed_values: None,
                    }),
                });
            }

            // Common LLM sampling parameters
            params.extend(common_llm_params());

            // dLLM-specific: diffusion decoding parameters
            if is_dllm {
                params.push(InferenceParamSchema {
                    key: "denoising_steps".into(),
                    label: "Denoising Steps".into(),
                    param_type: ParamType::Integer,
                    default: json!(8),
                    description: Some("Number of refinement iterations per block".into()),
                    constraints: Some(ParamConstraints {
                        min: Some(1.0),
                        max: Some(64.0),
                        allowed_values: None,
                    }),
                });
                params.push(InferenceParamSchema {
                    key: "block_length".into(),
                    label: "Block Length".into(),
                    param_type: ParamType::Integer,
                    default: json!(8),
                    description: Some("Tokens generated per diffusion block".into()),
                    constraints: Some(ParamConstraints {
                        min: Some(1.0),
                        max: Some(64.0),
                        allowed_values: None,
                    }),
                });
            }

            Some(params)
        }
        "diffusion" => Some(vec![
            InferenceParamSchema {
                key: "num_inference_steps".into(),
                label: "Inference Steps".into(),
                param_type: ParamType::Integer,
                default: json!(20),
                description: Some("Number of diffusion steps".into()),
                constraints: Some(ParamConstraints {
                    min: Some(1.0),
                    max: Some(150.0),
                    allowed_values: None,
                }),
            },
            InferenceParamSchema {
                key: "guidance_scale".into(),
                label: "Guidance Scale".into(),
                param_type: ParamType::Number,
                default: json!(7.0),
                description: Some("Classifier-free guidance scale".into()),
                constraints: Some(ParamConstraints {
                    min: Some(1.0),
                    max: Some(30.0),
                    allowed_values: None,
                }),
            },
            InferenceParamSchema {
                key: "width".into(),
                label: "Width".into(),
                param_type: ParamType::Integer,
                default: json!(1024),
                description: Some("Output image width in pixels".into()),
                constraints: Some(ParamConstraints {
                    min: Some(64.0),
                    max: Some(4096.0),
                    allowed_values: None,
                }),
            },
            InferenceParamSchema {
                key: "height".into(),
                label: "Height".into(),
                param_type: ParamType::Integer,
                default: json!(1024),
                description: Some("Output image height in pixels".into()),
                constraints: Some(ParamConstraints {
                    min: Some(64.0),
                    max: Some(4096.0),
                    allowed_values: None,
                }),
            },
            InferenceParamSchema {
                key: "seed".into(),
                label: "Seed".into(),
                param_type: ParamType::Integer,
                default: json!(-1),
                description: Some("Random seed (-1 = random)".into()),
                constraints: Some(ParamConstraints {
                    min: Some(-1.0),
                    max: None,
                    allowed_values: None,
                }),
            },
        ]),
        _ => None,
    }
}

/// Resolve inference settings with metadata-aware overrides.
pub fn resolve_inference_settings(
    metadata: &ModelMetadata,
    file_format: &str,
) -> Option<Vec<InferenceParamSchema>> {
    let model_type = metadata.model_type.as_deref().unwrap_or("");
    let subtype = metadata.subtype.as_deref();
    let mut params = default_inference_settings(model_type, file_format, subtype)?;

    if model_type == "diffusion" && is_qwen_image_text_to_image(metadata) {
        replace_param(
            &mut params,
            &["guidance_scale", "cfg_scale"],
            InferenceParamSchema {
                key: "true_cfg_scale".into(),
                label: "True CFG Scale".into(),
                param_type: ParamType::Number,
                default: json!(4.0),
                description: Some(
                    "QwenImage true CFG scale. guidance_scale is ineffective for CFG.".into(),
                ),
                constraints: Some(ParamConstraints {
                    min: Some(1.0),
                    max: Some(30.0),
                    allowed_values: None,
                }),
            },
        );
    }

    Some(params)
}

fn replace_param(
    params: &mut Vec<InferenceParamSchema>,
    replaced_keys: &[&str],
    replacement: InferenceParamSchema,
) {
    if let Some(index) = params
        .iter()
        .position(|param| replaced_keys.contains(&param.key.as_str()))
    {
        params[index] = replacement;
    } else {
        params.push(replacement);
    }
}

fn is_qwen_image_text_to_image(metadata: &ModelMetadata) -> bool {
    let family_is_qwen = metadata
        .family
        .as_deref()
        .is_some_and(|family| family.eq_ignore_ascii_case("qwen"));

    if !family_is_qwen {
        return false;
    }

    let names = [
        metadata.official_name.as_deref(),
        metadata.cleaned_name.as_deref(),
        metadata.repo_id.as_deref(),
    ];

    names.iter().flatten().any(|value| {
        let lower = value.to_ascii_lowercase();
        lower.contains("qwen-image") && !lower.contains("edit")
    })
}

/// Common LLM sampling parameters shared across all LLM formats.
fn common_llm_params() -> Vec<InferenceParamSchema> {
    vec![
        InferenceParamSchema {
            key: "temperature".into(),
            label: "Temperature".into(),
            param_type: ParamType::Number,
            default: json!(0.7),
            description: Some("Sampling temperature (higher = more creative)".into()),
            constraints: Some(ParamConstraints {
                min: Some(0.0),
                max: Some(5.0),
                allowed_values: None,
            }),
        },
        InferenceParamSchema {
            key: "top_p".into(),
            label: "Top P".into(),
            param_type: ParamType::Number,
            default: json!(0.9),
            description: Some("Nucleus sampling threshold".into()),
            constraints: Some(ParamConstraints {
                min: Some(0.0),
                max: Some(1.0),
                allowed_values: None,
            }),
        },
        InferenceParamSchema {
            key: "top_k".into(),
            label: "Top K".into(),
            param_type: ParamType::Integer,
            default: json!(40),
            description: Some("Top-K sampling (0 = disabled)".into()),
            constraints: Some(ParamConstraints {
                min: Some(0.0),
                max: Some(1000.0),
                allowed_values: None,
            }),
        },
        InferenceParamSchema {
            key: "repeat_penalty".into(),
            label: "Repeat Penalty".into(),
            param_type: ParamType::Number,
            default: json!(1.1),
            description: Some("Penalty for repeated tokens".into()),
            constraints: Some(ParamConstraints {
                min: Some(0.0),
                max: Some(5.0),
                allowed_values: None,
            }),
        },
        InferenceParamSchema {
            key: "seed".into(),
            label: "Seed".into(),
            param_type: ParamType::Integer,
            default: json!(-1),
            description: Some("Random seed (-1 = random)".into()),
            constraints: Some(ParamConstraints {
                min: Some(-1.0),
                max: None,
                allowed_values: None,
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gguf_llm_defaults() {
        let settings = default_inference_settings("llm", "gguf", None).unwrap();
        let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
        assert!(keys.contains(&"gpu_layers"));
        assert!(keys.contains(&"context_length"));
        assert!(keys.contains(&"temperature"));
        assert!(keys.contains(&"top_p"));
        assert!(!keys.contains(&"denoising_steps"));
    }

    #[test]
    fn test_safetensors_llm_defaults() {
        let settings = default_inference_settings("llm", "safetensors", None).unwrap();
        let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
        assert!(!keys.contains(&"gpu_layers"));
        assert!(!keys.contains(&"context_length"));
        assert!(keys.contains(&"temperature"));
    }

    #[test]
    fn test_dllm_gets_both_llm_and_diffusion_params() {
        let settings = default_inference_settings("llm", "safetensors", Some("dllm")).unwrap();
        let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
        assert!(keys.contains(&"temperature"));
        assert!(keys.contains(&"denoising_steps"));
        assert!(keys.contains(&"block_length"));
    }

    #[test]
    fn test_diffusion_defaults() {
        let settings = default_inference_settings("diffusion", "safetensors", None).unwrap();
        let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
        assert!(keys.contains(&"num_inference_steps"));
        assert!(keys.contains(&"guidance_scale"));
        assert!(keys.contains(&"width"));
        assert!(keys.contains(&"height"));
        assert!(keys.contains(&"seed"));
    }

    #[test]
    fn test_resolve_inference_settings_for_qwen_image_replaces_guidance_scale() {
        let metadata = ModelMetadata {
            model_type: Some("diffusion".into()),
            family: Some("Qwen".into()),
            official_name: Some("Qwen-Image-2512".into()),
            cleaned_name: Some("qwen-image-2512".into()),
            repo_id: Some("Qwen/Qwen-Image-2512".into()),
            ..Default::default()
        };

        let settings = resolve_inference_settings(&metadata, "safetensors").unwrap();
        let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();

        assert!(keys.contains(&"num_inference_steps"));
        assert!(keys.contains(&"true_cfg_scale"));
        assert!(keys.contains(&"width"));
        assert!(keys.contains(&"height"));
        assert!(!keys.contains(&"guidance_scale"));
    }

    #[test]
    fn test_unknown_type_returns_none() {
        assert!(default_inference_settings("embedding", "safetensors", None).is_none());
        assert!(default_inference_settings("audio", "gguf", None).is_none());
    }
}
