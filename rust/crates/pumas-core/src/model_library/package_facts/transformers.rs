use crate::error::{PumasError, Result};
use crate::model_library::types::ModelMetadata;
use crate::models::{
    BackendHintFacts, BackendHintLabel, PackageFactStatus, TransformersPackageEvidence,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

pub(crate) async fn transformers_package_evidence(
    model_dir: &Path,
    metadata: &ModelMetadata,
    selected_files: &[String],
) -> Result<Option<TransformersPackageEvidence>> {
    let config_path = model_dir.join("config.json");
    let config_exists = tokio::fs::try_exists(&config_path).await?;
    let generation_config_exists =
        tokio::fs::try_exists(model_dir.join("generation_config.json")).await?;
    if !config_exists && !generation_config_exists {
        return Ok(None);
    }

    let config = if config_exists {
        let config_path = config_path.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::read_to_string(config_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        })
        .await
        .map_err(|err| PumasError::Other(format!("Failed to join config parse: {}", err)))?
    } else {
        None
    };

    let architectures = config
        .as_ref()
        .and_then(|value| value.get("architectures"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .or_else(|| {
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.architectures.clone())
        })
        .unwrap_or_default();

    Ok(Some(TransformersPackageEvidence {
        config_status: if config_exists {
            PackageFactStatus::Present
        } else {
            PackageFactStatus::Missing
        },
        config_model_type: config
            .as_ref()
            .and_then(|value| value.get("model_type"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                metadata
                    .huggingface_evidence
                    .as_ref()
                    .and_then(|evidence| evidence.config_model_type.clone())
            }),
        architectures,
        dtype: config
            .as_ref()
            .and_then(|value| value.get("dtype"))
            .and_then(Value::as_str)
            .map(str::to_string),
        torch_dtype: config
            .as_ref()
            .and_then(|value| value.get("torch_dtype"))
            .and_then(Value::as_str)
            .map(str::to_string),
        auto_map: config
            .as_ref()
            .and_then(|value| value.get("auto_map"))
            .and_then(Value::as_object)
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default(),
        processor_class: config
            .as_ref()
            .and_then(|value| value.get("processor_class"))
            .and_then(Value::as_str)
            .map(str::to_string),
        generation_config_status: if generation_config_exists {
            PackageFactStatus::Present
        } else {
            PackageFactStatus::Missing
        },
        source_repo_id: metadata.repo_id.clone().or_else(|| {
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.repo_id.clone())
        }),
        source_revision: None,
        selected_files: selected_files
            .iter()
            .filter(|file| {
                file.as_str() == "config.json" || file.as_str() == "generation_config.json"
            })
            .cloned()
            .collect(),
    }))
}

pub(crate) async fn auto_map_sources_from_config(model_dir: &Path) -> Result<Vec<String>> {
    let config_path = model_dir.join("config.json");
    if !tokio::fs::try_exists(&config_path).await? {
        return Ok(Vec::new());
    }

    tokio::task::spawn_blocking(move || {
        let Some(config) = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(Vec::new());
        };

        let mut sources = BTreeSet::new();
        if let Some(auto_map) = config.get("auto_map").and_then(Value::as_object) {
            for value in auto_map.values() {
                match value {
                    Value::String(source) => {
                        sources.insert(source.clone());
                    }
                    Value::Array(values) => {
                        sources.extend(
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(std::string::ToString::to_string),
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok::<_, PumasError>(sources.into_iter().collect())
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join auto_map parse: {}", err)))?
}

pub(crate) async fn custom_generate_sources(model_dir: &Path) -> Result<Vec<String>> {
    let relative_path = "custom_generate/generate.py";
    if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
        return Ok(vec![relative_path.to_string()]);
    }
    Ok(Vec::new())
}

pub(crate) async fn custom_generate_dependency_manifests(model_dir: &Path) -> Result<Vec<String>> {
    let relative_path = "custom_generate/requirements.txt";
    if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
        return Ok(vec![relative_path.to_string()]);
    }
    Ok(Vec::new())
}

pub(crate) fn merge_string_lists(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    left.into_iter()
        .chain(right)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn backend_hint_facts(
    recommended_backend: Option<&str>,
    runtime_engine_hints: &[String],
) -> BackendHintFacts {
    let mut raw = BTreeSet::new();
    if let Some(backend) = recommended_backend.filter(|backend| !backend.trim().is_empty()) {
        raw.insert(backend.trim().to_string());
    }
    raw.extend(
        runtime_engine_hints
            .iter()
            .filter(|hint| !hint.trim().is_empty())
            .map(|hint| hint.trim().to_string()),
    );

    let mut accepted = Vec::new();
    let mut unsupported = Vec::new();
    for hint in &raw {
        match hint.to_lowercase().as_str() {
            "transformers" => accepted.push(BackendHintLabel::Transformers),
            "llama.cpp" | "llamacpp" | "llama-cpp" => accepted.push(BackendHintLabel::LlamaCpp),
            "vllm" => accepted.push(BackendHintLabel::Vllm),
            "mlx" => accepted.push(BackendHintLabel::Mlx),
            "candle" => accepted.push(BackendHintLabel::Candle),
            "diffusers" => accepted.push(BackendHintLabel::Diffusers),
            "onnx-runtime" | "onnxruntime" => accepted.push(BackendHintLabel::OnnxRuntime),
            _ => unsupported.push(hint.clone()),
        }
    }
    accepted.sort_by_key(|hint| serde_json::to_string(hint).unwrap_or_default());
    accepted.dedup();

    BackendHintFacts {
        accepted,
        raw: raw.into_iter().collect(),
        unsupported,
    }
}
