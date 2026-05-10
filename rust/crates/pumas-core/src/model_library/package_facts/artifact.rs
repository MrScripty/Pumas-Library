use crate::error::{PumasError, Result};
use crate::model_library::external_assets::is_diffusers_bundle;
use crate::model_library::types::ModelMetadata;
use crate::models::{
    PackageArtifactKind, PackageClassReference, PackageFactStatus, ProcessorComponentFacts,
    ProcessorComponentKind, TransformersPackageEvidence,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) async fn package_artifact_kind(
    model_dir: &Path,
    metadata: &ModelMetadata,
    selected_files: &[String],
) -> Result<PackageArtifactKind> {
    if is_diffusers_bundle(metadata) {
        return Ok(PackageArtifactKind::DiffusersBundle);
    }
    if tokio::fs::try_exists(model_dir.join("adapter_config.json")).await? {
        return Ok(PackageArtifactKind::Adapter);
    }
    if selected_files
        .iter()
        .any(|file| file.to_lowercase().ends_with(".gguf"))
    {
        return Ok(PackageArtifactKind::Gguf);
    }
    if selected_files
        .iter()
        .any(|file| file.to_lowercase().ends_with(".onnx"))
    {
        return Ok(PackageArtifactKind::Onnx);
    }
    if selected_files
        .iter()
        .any(|file| file.to_lowercase().ends_with(".safetensors"))
    {
        if tokio::fs::try_exists(model_dir.join("config.json")).await? {
            return Ok(PackageArtifactKind::HfCompatibleDirectory);
        }
        return Ok(PackageArtifactKind::Safetensors);
    }
    if tokio::fs::try_exists(model_dir.join("config.json")).await? {
        return Ok(PackageArtifactKind::HfCompatibleDirectory);
    }
    Ok(PackageArtifactKind::Unknown)
}

pub(crate) async fn package_component_facts(
    model_dir: &Path,
    selected_files: &[String],
) -> Result<Vec<ProcessorComponentFacts>> {
    struct ComponentCandidate {
        kind: ProcessorComponentKind,
        relative_path: &'static str,
        class_keys: &'static [&'static str],
    }

    let candidates = [
        ComponentCandidate {
            kind: ProcessorComponentKind::Config,
            relative_path: "config.json",
            class_keys: &["architectures"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Tokenizer,
            relative_path: "tokenizer.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::TokenizerConfig,
            relative_path: "tokenizer_config.json",
            class_keys: &["tokenizer_class"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::SpecialTokensMap,
            relative_path: "special_tokens_map.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Processor,
            relative_path: "processor_config.json",
            class_keys: &["processor_class"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Preprocessor,
            relative_path: "preprocessor_config.json",
            class_keys: &["processor_class", "feature_extractor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::ImageProcessor,
            relative_path: "image_processor_config.json",
            class_keys: &["image_processor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::VideoProcessor,
            relative_path: "video_processor_config.json",
            class_keys: &["video_processor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::AudioFeatureExtractor,
            relative_path: "feature_extractor_config.json",
            class_keys: &["feature_extractor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::ChatTemplate,
            relative_path: "chat_template.jinja",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::GenerationConfig,
            relative_path: "generation_config.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::ModelIndex,
            relative_path: "model_index.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Adapter,
            relative_path: "adapter_config.json",
            class_keys: &["peft_type"],
        },
    ];
    let mut facts = Vec::new();
    for candidate in candidates {
        let relative_path = candidate.relative_path;
        if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            let class_name =
                component_class_name(model_dir.join(relative_path), candidate.class_keys).await?;
            let message = if candidate.kind == ProcessorComponentKind::Tokenizer
                && relative_path == "tokenizer.json"
            {
                tokenizer_json_diagnostic_message(model_dir.join(relative_path)).await?
            } else {
                None
            };
            facts.push(ProcessorComponentFacts {
                kind: candidate.kind,
                status: PackageFactStatus::Present,
                relative_path: Some(relative_path.to_string()),
                class_name,
                message,
            });
        }
    }
    facts.extend(tokenizer_vocabulary_component_facts(model_dir).await?);
    facts.extend(chat_template_directory_facts(model_dir).await?);
    facts.extend(weight_component_facts(model_dir, selected_files).await?);
    facts.extend(quantization_component_facts(model_dir, selected_files).await?);
    Ok(facts)
}

async fn tokenizer_vocabulary_component_facts(
    model_dir: &Path,
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    for relative_path in TOKENIZER_VOCABULARY_FILENAMES {
        if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            facts.push(ProcessorComponentFacts {
                kind: ProcessorComponentKind::Tokenizer,
                status: PackageFactStatus::Present,
                relative_path: Some((*relative_path).to_string()),
                class_name: None,
                message: None,
            });
        }
    }

    if facts.is_empty()
        && !tokio::fs::try_exists(model_dir.join("tokenizer.json")).await?
        && tokio::fs::try_exists(model_dir.join("tokenizer_config.json")).await?
    {
        facts.push(ProcessorComponentFacts {
            kind: ProcessorComponentKind::Tokenizer,
            status: PackageFactStatus::Missing,
            relative_path: None,
            class_name: None,
            message: Some(
                "tokenizer_config.json is present without a known tokenizer vocabulary file"
                    .to_string(),
            ),
        });
    }

    Ok(facts)
}

async fn tokenizer_json_diagnostic_message(path: PathBuf) -> Result<Option<String>> {
    tokio::task::spawn_blocking(move || {
        let Some(tokenizer) = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(None);
        };

        let mut parts = Vec::new();
        if let Some(version) = tokenizer.get("version").and_then(Value::as_str) {
            parts.push(format!("version={version}"));
        }
        if let Some(model_type) = tokenizer
            .get("model")
            .and_then(Value::as_object)
            .and_then(|model| model.get("type"))
            .and_then(Value::as_str)
        {
            parts.push(format!("model={model_type}"));
        }
        if let Some(normalizer_type) = tokenizer
            .get("normalizer")
            .and_then(Value::as_object)
            .and_then(|normalizer| normalizer.get("type"))
            .and_then(Value::as_str)
        {
            parts.push(format!("normalizer={normalizer_type}"));
        }
        if let Some(pre_tokenizer_type) = tokenizer
            .get("pre_tokenizer")
            .and_then(Value::as_object)
            .and_then(|pre_tokenizer| pre_tokenizer.get("type"))
            .and_then(Value::as_str)
        {
            parts.push(format!("pre_tokenizer={pre_tokenizer_type}"));
        }

        Ok::<_, PumasError>(if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        })
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join tokenizer JSON parse: {}", err)))?
}

const TOKENIZER_VOCABULARY_FILENAMES: &[&str] = &[
    "vocab.json",
    "merges.txt",
    "vocab.txt",
    "spiece.model",
    "sentencepiece.bpe.model",
    "tokenizer.model",
];

async fn weight_component_facts(
    model_dir: &Path,
    selected_files: &[String],
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    let mut seen_paths = BTreeSet::new();
    for relative_path in selected_files {
        if !seen_paths.insert(relative_path.clone()) {
            continue;
        }
        if !tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            continue;
        }
        let lower = relative_path.to_lowercase();
        let kind = if is_transformers_weight_index_file(&lower) {
            Some(ProcessorComponentKind::WeightIndex)
        } else if is_transformers_shard_file(&lower) {
            Some(ProcessorComponentKind::Shard)
        } else if is_weight_file(&lower) {
            Some(ProcessorComponentKind::Weights)
        } else {
            None
        };

        if let Some(kind) = kind {
            facts.push(ProcessorComponentFacts {
                kind,
                status: PackageFactStatus::Present,
                relative_path: Some(relative_path.clone()),
                class_name: None,
                message: None,
            });
        }
    }
    facts.extend(weight_index_declared_shard_facts(model_dir, selected_files, &facts).await?);
    facts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(facts)
}

async fn weight_index_declared_shard_facts(
    model_dir: &Path,
    selected_files: &[String],
    existing_facts: &[ProcessorComponentFacts],
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    let mut seen_shards = existing_facts
        .iter()
        .filter(|fact| fact.kind == ProcessorComponentKind::Shard)
        .filter_map(|fact| fact.relative_path.clone())
        .collect::<BTreeSet<_>>();

    for index_path in selected_files
        .iter()
        .filter(|path| is_transformers_weight_index_file(&path.to_lowercase()))
    {
        let declared_shards = declared_shards_from_weight_index(model_dir.join(index_path)).await?;
        for shard_path in declared_shards {
            if !seen_shards.insert(shard_path.clone()) {
                continue;
            }
            let status = if tokio::fs::try_exists(model_dir.join(&shard_path)).await? {
                PackageFactStatus::Present
            } else {
                PackageFactStatus::Missing
            };
            let message = if status == PackageFactStatus::Missing {
                Some(format!("declared by {index_path} but file is missing"))
            } else {
                Some(format!("declared by {index_path}"))
            };
            facts.push(ProcessorComponentFacts {
                kind: ProcessorComponentKind::Shard,
                status,
                relative_path: Some(shard_path),
                class_name: None,
                message,
            });
        }
    }

    Ok(facts)
}

async fn declared_shards_from_weight_index(path: PathBuf) -> Result<Vec<String>> {
    tokio::task::spawn_blocking(move || {
        let Some(index) = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(Vec::new());
        };
        let Some(weight_map) = index.get("weight_map").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };

        let shards = weight_map
            .values()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        Ok::<_, PumasError>(shards.into_iter().collect())
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join weight index parse: {}", err)))?
}

fn is_transformers_weight_index_file(relative_path: &str) -> bool {
    relative_path.ends_with(".safetensors.index.json")
        || relative_path.ends_with(".bin.index.json")
        || relative_path.ends_with(".pt.index.json")
}

fn is_transformers_shard_file(relative_path: &str) -> bool {
    let Some(file_name) = Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
    else {
        return false;
    };
    let parts = file_name.split('-').collect::<Vec<_>>();
    parts.len() >= 3
        && parts.iter().enumerate().any(|(index, part)| {
            *part == "of"
                && index > 0
                && index + 1 < parts.len()
                && parts[index - 1].chars().all(|ch| ch.is_ascii_digit())
                && parts[index + 1]
                    .split('.')
                    .next()
                    .is_some_and(|total| total.chars().all(|ch| ch.is_ascii_digit()))
        })
}

fn is_weight_file(relative_path: &str) -> bool {
    ["safetensors", "bin", "pt", "pth", "ckpt", "gguf", "onnx"]
        .iter()
        .any(|extension| relative_path.ends_with(&format!(".{extension}")))
}

async fn quantization_component_facts(
    model_dir: &Path,
    selected_files: &[String],
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    if let Some(message) = quantization_message_from_config(model_dir).await? {
        facts.push(ProcessorComponentFacts {
            kind: ProcessorComponentKind::Quantization,
            status: PackageFactStatus::Present,
            relative_path: Some("config.json".to_string()),
            class_name: None,
            message: Some(message),
        });
    }

    let mut seen_messages = BTreeSet::new();
    for relative_path in selected_files {
        if !relative_path.to_lowercase().ends_with(".gguf")
            || !tokio::fs::try_exists(model_dir.join(relative_path)).await?
        {
            continue;
        }
        let Some(quant) = quantization_from_filename(relative_path) else {
            continue;
        };
        if !seen_messages.insert(quant.clone()) {
            continue;
        }
        facts.push(ProcessorComponentFacts {
            kind: ProcessorComponentKind::Quantization,
            status: PackageFactStatus::Present,
            relative_path: Some(relative_path.clone()),
            class_name: None,
            message: Some(quant),
        });
    }

    facts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(facts)
}

async fn quantization_message_from_config(model_dir: &Path) -> Result<Option<String>> {
    let config_path = model_dir.join("config.json");
    if !tokio::fs::try_exists(&config_path).await? {
        return Ok(None);
    }

    tokio::task::spawn_blocking(move || {
        let Some(config) = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(None);
        };
        let Some(quantization_config) =
            config.get("quantization_config").and_then(Value::as_object)
        else {
            return Ok(None);
        };
        if let Some(method) = quantization_config
            .get("quant_method")
            .and_then(Value::as_str)
        {
            return Ok(Some(method.to_string()));
        }
        if quantization_config
            .get("load_in_4bit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(Some("load_in_4bit".to_string()));
        }
        if quantization_config
            .get("load_in_8bit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(Some("load_in_8bit".to_string()));
        }
        Ok::<_, PumasError>(Some("config.quantization_config".to_string()))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join quantization parse: {}", err)))?
}

fn quantization_from_filename(relative_path: &str) -> Option<String> {
    let file_name = Path::new(relative_path).file_name()?.to_str()?;
    let upper = file_name.to_uppercase();
    KNOWN_GGUF_QUANTS
        .iter()
        .find(|quant| upper.contains(**quant))
        .map(|quant| (*quant).to_string())
}

const KNOWN_GGUF_QUANTS: &[&str] = &[
    "IQ2_XXS", "IQ3_XXS", "Q3_K_S", "Q3_K_M", "Q3_K_L", "Q4_K_S", "Q4_K_M", "Q5_K_S", "Q5_K_M",
    "IQ2_XS", "IQ3_XS", "IQ4_XS", "IQ4_NL", "IQ1_S", "IQ1_M", "IQ2_S", "IQ2_M", "IQ3_S", "IQ3_M",
    "Q2_K", "Q3_K", "Q4_0", "Q4_1", "Q4_K", "Q5_0", "Q5_1", "Q5_K", "Q6_K", "Q8_0",
];

async fn component_class_name(path: PathBuf, class_keys: &[&str]) -> Result<Option<String>> {
    if class_keys.is_empty() {
        return Ok(None);
    }
    let class_keys = class_keys
        .iter()
        .map(|key| (*key).to_string())
        .collect::<Vec<_>>();
    tokio::task::spawn_blocking(move || {
        let Some(config) = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(None);
        };
        for key in class_keys {
            match config.get(&key) {
                Some(Value::String(value)) => return Ok(Some(value.clone())),
                Some(Value::Array(values)) => {
                    if let Some(value) = values.iter().filter_map(Value::as_str).next() {
                        return Ok(Some(value.to_string()));
                    }
                }
                _ => {}
            }
        }
        Ok::<_, PumasError>(None)
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join component class parse: {}", err)))?
}

async fn chat_template_directory_facts(model_dir: &Path) -> Result<Vec<ProcessorComponentFacts>> {
    let template_dir = model_dir.join("chat_templates");
    if !tokio::fs::try_exists(&template_dir).await? {
        return Ok(Vec::new());
    }
    tokio::task::spawn_blocking(move || {
        let mut facts = Vec::new();
        for entry in std::fs::read_dir(template_dir).map_err(|err| PumasError::Io {
            message: "Failed to read chat_templates directory".to_string(),
            path: None,
            source: Some(err),
        })? {
            let entry = entry.map_err(|err| PumasError::Io {
                message: "Failed to read chat_templates entry".to_string(),
                path: None,
                source: Some(err),
            })?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("jinja") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            facts.push(ProcessorComponentFacts {
                kind: ProcessorComponentKind::ChatTemplate,
                status: PackageFactStatus::Present,
                relative_path: Some(format!("chat_templates/{}", name)),
                class_name: None,
                message: None,
            });
        }
        facts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok::<_, PumasError>(facts)
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join chat template scan: {}", err)))?
}

pub(crate) fn package_class_references(
    components: &[ProcessorComponentFacts],
    transformers: Option<&TransformersPackageEvidence>,
) -> Vec<PackageClassReference> {
    let mut references = Vec::new();
    let mut seen = BTreeSet::new();

    for component in components {
        let Some(class_name) = component.class_name.as_ref() else {
            continue;
        };
        push_package_class_reference(
            &mut references,
            &mut seen,
            component.kind,
            class_name.clone(),
            component.relative_path.clone(),
        );
    }

    if let Some(transformers) = transformers {
        for architecture in &transformers.architectures {
            push_package_class_reference(
                &mut references,
                &mut seen,
                ProcessorComponentKind::Config,
                architecture.clone(),
                Some("config.json".to_string()),
            );
        }
        if let Some(processor_class) = transformers.processor_class.as_ref() {
            push_package_class_reference(
                &mut references,
                &mut seen,
                ProcessorComponentKind::Processor,
                processor_class.clone(),
                Some("config.json".to_string()),
            );
        }
    }

    references.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
            .then_with(|| left.class_name.cmp(&right.class_name))
    });
    references
}

fn push_package_class_reference(
    references: &mut Vec<PackageClassReference>,
    seen: &mut BTreeSet<(String, String, String)>,
    kind: ProcessorComponentKind,
    class_name: String,
    source_path: Option<String>,
) {
    let key = (
        format!("{kind:?}"),
        source_path.clone().unwrap_or_default(),
        class_name.clone(),
    );
    if seen.insert(key) {
        references.push(PackageClassReference {
            kind,
            class_name,
            source_path,
        });
    }
}

pub(crate) fn companion_artifacts(selected_files: &[String]) -> Vec<String> {
    selected_files
        .iter()
        .filter(|file| file.to_lowercase().contains("mmproj"))
        .cloned()
        .collect()
}
