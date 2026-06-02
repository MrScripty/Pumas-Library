use crate::model_library::package_facts::manifest::PackageInspectionManifest;
use crate::model_library::types::ModelMetadata;
use crate::models::{
    ModelPackageDiagnostic, PackageArtifactKind, PackageFactStatus, PackageFactValueSource,
    PackageFileSizeFact, PackageLogicalSizeFacts, PackageSizeRole, ProcessorComponentFacts,
    ProcessorComponentKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

pub(crate) fn artifact_logical_size_facts(
    metadata: &ModelMetadata,
    artifact_kind: PackageArtifactKind,
    selected_files: &[String],
    companion_artifacts: &[String],
    components: &[ProcessorComponentFacts],
    manifest: &PackageInspectionManifest,
) -> PackageLogicalSizeFacts {
    let selected_files = normalized_path_set(selected_files.iter().map(String::as_str));
    let companion_artifacts = normalized_path_set(companion_artifacts.iter().map(String::as_str));
    let upstream_sizes = upstream_file_sizes(metadata);
    let mut diagnostics = Vec::new();
    let mut files = BTreeMap::new();

    for entry in manifest.entries() {
        let relative_path = entry.relative_path();
        let role = size_role_for_path(
            relative_path,
            artifact_kind,
            &selected_files,
            &companion_artifacts,
            None,
        );
        if !is_logical_size_role(role) {
            continue;
        }
        files.insert(
            relative_path.to_string(),
            PackageFileSizeFact {
                relative_path: relative_path.to_string(),
                size_bytes: entry.size_bytes(),
                status: entry.status(),
                value_source: entry.value_source(),
                role: Some(role),
            },
        );
    }

    for component in components {
        let Some(relative_path) = component.relative_path.as_deref() else {
            continue;
        };
        let Some(relative_path) = normalize_package_relative_path(relative_path) else {
            continue;
        };
        let role = size_role_for_path(
            &relative_path,
            artifact_kind,
            &selected_files,
            &companion_artifacts,
            Some(component.kind),
        );
        if !is_logical_size_role(role) || files.contains_key(&relative_path) {
            continue;
        }
        files.insert(
            relative_path.clone(),
            PackageFileSizeFact {
                relative_path,
                size_bytes: None,
                status: component.status,
                value_source: PackageFactValueSource::Unavailable,
                role: Some(role),
            },
        );
    }

    for relative_path in selected_files
        .iter()
        .chain(companion_artifacts.iter())
        .chain(
            metadata
                .expected_files
                .iter()
                .flatten()
                .filter_map(|path| normalize_package_relative_path(path))
                .collect::<Vec<_>>()
                .iter(),
        )
    {
        if files.contains_key(relative_path) {
            continue;
        }
        let role = size_role_for_path(
            relative_path,
            artifact_kind,
            &selected_files,
            &companion_artifacts,
            None,
        );
        if !is_logical_size_role(role) {
            continue;
        }
        files.insert(
            relative_path.clone(),
            PackageFileSizeFact {
                relative_path: relative_path.clone(),
                size_bytes: None,
                status: PackageFactStatus::Missing,
                value_source: PackageFactValueSource::Unavailable,
                role: Some(role),
            },
        );
    }

    for (relative_path, upstream_size) in &upstream_sizes {
        let Some(file) = files.get_mut(relative_path) else {
            continue;
        };
        match file.size_bytes {
            Some(local_size) if local_size != *upstream_size => diagnostics.push(
                ModelPackageDiagnostic {
                    code: "logical_size_mismatch".to_string(),
                    message: format!(
                        "local file size for {relative_path} ({local_size}) differs from upstream metadata ({upstream_size})"
                    ),
                    path: Some(relative_path.clone()),
                },
            ),
            Some(_) => {}
            None => {
                file.size_bytes = Some(*upstream_size);
                file.value_source = PackageFactValueSource::UpstreamMetadata;
            }
        }
    }

    let mut file_facts = files.into_values().collect::<Vec<_>>();
    file_facts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    for file in &file_facts {
        if file.status == PackageFactStatus::Missing {
            diagnostics.push(ModelPackageDiagnostic {
                code: "logical_size_file_missing".to_string(),
                message: if file.size_bytes.is_some() {
                    format!(
                        "{} is missing locally; size is sourced from upstream metadata",
                        file.relative_path
                    )
                } else {
                    format!("{} is missing and has no known size", file.relative_path)
                },
                path: Some(file.relative_path.clone()),
            });
        }
    }

    let (total_size_bytes, value_source) =
        logical_total_size(metadata, &file_facts, &mut diagnostics);

    PackageLogicalSizeFacts {
        total_size_bytes,
        value_source,
        files: file_facts,
        diagnostics,
    }
}

fn logical_total_size(
    metadata: &ModelMetadata,
    files: &[PackageFileSizeFact],
    diagnostics: &mut Vec<ModelPackageDiagnostic>,
) -> (Option<u64>, PackageFactValueSource) {
    if files.is_empty() {
        if let Some(size_bytes) = metadata.size_bytes {
            diagnostics.push(ModelPackageDiagnostic {
                code: "logical_size_from_metadata_total".to_string(),
                message: "logical size uses model metadata total because no bounded file sizes were available".to_string(),
                path: None,
            });
            return (Some(size_bytes), PackageFactValueSource::UpstreamMetadata);
        }
        diagnostics.push(ModelPackageDiagnostic {
            code: "logical_size_unavailable".to_string(),
            message: "logical size is unavailable from bounded package evidence".to_string(),
            path: None,
        });
        return (None, PackageFactValueSource::Unavailable);
    }

    let mut total = 0_u64;
    let mut saw_upstream = false;
    for file in files {
        let Some(size_bytes) = file.size_bytes else {
            return (None, PackageFactValueSource::Unavailable);
        };
        saw_upstream |= file.value_source == PackageFactValueSource::UpstreamMetadata;
        let Some(next_total) = total.checked_add(size_bytes) else {
            diagnostics.push(ModelPackageDiagnostic {
                code: "logical_size_overflow".to_string(),
                message: "logical size overflowed u64 while summing package file sizes".to_string(),
                path: Some(file.relative_path.clone()),
            });
            return (None, PackageFactValueSource::Unavailable);
        };
        total = next_total;
    }

    if let Some(metadata_size) = metadata.size_bytes {
        if metadata_size != total {
            diagnostics.push(ModelPackageDiagnostic {
                code: "logical_size_total_mismatch".to_string(),
                message: format!(
                    "summed logical size ({total}) differs from model metadata total ({metadata_size})"
                ),
                path: None,
            });
        }
    }

    let value_source = if saw_upstream {
        PackageFactValueSource::UpstreamMetadata
    } else if files.len() > 1 {
        PackageFactValueSource::ComponentLayout
    } else {
        PackageFactValueSource::FilesystemMetadata
    };

    (Some(total), value_source)
}

fn upstream_file_sizes(metadata: &ModelMetadata) -> BTreeMap<String, u64> {
    metadata
        .files
        .iter()
        .flatten()
        .filter_map(|file| Some((normalize_package_relative_path(&file.name)?, file.size?)))
        .collect()
}

fn normalized_path_set<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    paths.filter_map(normalize_package_relative_path).collect()
}

fn normalize_package_relative_path(raw_path: &str) -> Option<String> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty() {
        return None;
    }

    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        return None;
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_str()?.to_string()),
            _ => return None,
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn size_role_for_path(
    relative_path: &str,
    artifact_kind: PackageArtifactKind,
    selected_files: &BTreeSet<String>,
    companion_artifacts: &BTreeSet<String>,
    component_kind: Option<ProcessorComponentKind>,
) -> PackageSizeRole {
    if companion_artifacts.contains(relative_path) {
        return PackageSizeRole::CompanionArtifact;
    }
    if direct_selected_artifact_path(relative_path, artifact_kind, selected_files) {
        return PackageSizeRole::SelectedArtifact;
    }
    if let Some(kind) = component_kind.and_then(size_role_for_component) {
        return kind;
    }

    let lower = relative_path.to_lowercase();
    if is_single_file_artifact_kind(artifact_kind) && is_weight_file(&lower) {
        return PackageSizeRole::Other;
    }
    if is_dependency_manifest_path(&lower) {
        PackageSizeRole::DependencyManifest
    } else if is_tokenizer_path(&lower) {
        PackageSizeRole::Tokenizer
    } else if is_component_config_path(&lower) {
        PackageSizeRole::ComponentConfig
    } else if is_transformers_shard_file(&lower) {
        PackageSizeRole::Shard
    } else if is_weight_file(&lower) {
        PackageSizeRole::Weight
    } else {
        PackageSizeRole::Other
    }
}

fn size_role_for_component(kind: ProcessorComponentKind) -> Option<PackageSizeRole> {
    match kind {
        ProcessorComponentKind::Config
        | ProcessorComponentKind::Processor
        | ProcessorComponentKind::Preprocessor
        | ProcessorComponentKind::ImageProcessor
        | ProcessorComponentKind::VideoProcessor
        | ProcessorComponentKind::AudioFeatureExtractor
        | ProcessorComponentKind::FeatureExtractor
        | ProcessorComponentKind::ChatTemplate
        | ProcessorComponentKind::GenerationConfig
        | ProcessorComponentKind::ModelIndex
        | ProcessorComponentKind::WeightIndex
        | ProcessorComponentKind::Adapter
        | ProcessorComponentKind::Quantization => Some(PackageSizeRole::ComponentConfig),
        ProcessorComponentKind::Tokenizer
        | ProcessorComponentKind::TokenizerConfig
        | ProcessorComponentKind::SpecialTokensMap => Some(PackageSizeRole::Tokenizer),
        ProcessorComponentKind::Shard => Some(PackageSizeRole::Shard),
        ProcessorComponentKind::Weights => Some(PackageSizeRole::Weight),
        ProcessorComponentKind::Other => Some(PackageSizeRole::Other),
    }
}

fn direct_selected_artifact_path(
    relative_path: &str,
    artifact_kind: PackageArtifactKind,
    selected_files: &BTreeSet<String>,
) -> bool {
    if !selected_files.contains(relative_path) {
        return false;
    }
    let lower = relative_path.to_lowercase();
    match artifact_kind {
        PackageArtifactKind::Gguf => lower.ends_with(".gguf"),
        PackageArtifactKind::Safetensors => lower.ends_with(".safetensors"),
        PackageArtifactKind::Onnx => lower.ends_with(".onnx"),
        PackageArtifactKind::Adapter => {
            lower.ends_with("adapter_model.safetensors") || lower.ends_with("adapter_model.bin")
        }
        PackageArtifactKind::Shard => is_transformers_shard_file(&lower),
        PackageArtifactKind::Unknown => is_weight_file(&lower),
        PackageArtifactKind::HfCompatibleDirectory | PackageArtifactKind::DiffusersBundle => false,
    }
}

fn is_logical_size_role(role: PackageSizeRole) -> bool {
    !matches!(role, PackageSizeRole::Other)
}

fn is_single_file_artifact_kind(artifact_kind: PackageArtifactKind) -> bool {
    matches!(
        artifact_kind,
        PackageArtifactKind::Gguf
            | PackageArtifactKind::Safetensors
            | PackageArtifactKind::Onnx
            | PackageArtifactKind::Adapter
            | PackageArtifactKind::Shard
            | PackageArtifactKind::Unknown
    )
}

fn is_dependency_manifest_path(relative_path: &str) -> bool {
    relative_path == "requirements.txt" || relative_path.ends_with("/requirements.txt")
}

fn is_tokenizer_path(relative_path: &str) -> bool {
    relative_path.contains("tokenizer")
        || matches!(
            Path::new(relative_path)
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "vocab.json"
                    | "merges.txt"
                    | "vocab.txt"
                    | "spiece.model"
                    | "sentencepiece.bpe.model"
                    | "tokenizer.model"
                    | "special_tokens_map.json"
            )
        )
}

fn is_component_config_path(relative_path: &str) -> bool {
    matches!(
        Path::new(relative_path)
            .file_name()
            .and_then(|name| name.to_str()),
        Some(
            "config.json"
                | "generation_config.json"
                | "processor_config.json"
                | "preprocessor_config.json"
                | "image_processor_config.json"
                | "video_processor_config.json"
                | "feature_extractor_config.json"
                | "chat_template.jinja"
                | "model_index.json"
                | "adapter_config.json"
                | "scheduler_config.json"
        )
    ) || relative_path.ends_with(".index.json")
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
