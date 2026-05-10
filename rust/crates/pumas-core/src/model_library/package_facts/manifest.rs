use crate::error::{PumasError, Result};
use crate::index::ModelDependencyBindingRecord;
use crate::model_library::types::ModelMetadata;
use crate::models::{ModelExecutionDescriptor, PACKAGE_FACTS_CONTRACT_VERSION};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageInspectionManifest {
    selected_files: Vec<String>,
    entries: Vec<PackageInspectionManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageInspectionManifestEntry {
    relative_path: String,
}

impl PackageInspectionManifest {
    pub(crate) async fn build(model_dir: &Path, metadata: &ModelMetadata) -> Result<Self> {
        let selected_files = package_selected_files(model_dir, metadata).await?;
        let entries = package_manifest_entries(model_dir, &selected_files).await?;
        Ok(Self {
            selected_files,
            entries,
        })
    }

    pub(crate) fn selected_files(&self) -> &[String] {
        &self.selected_files
    }

    #[cfg(test)]
    pub(crate) fn entries(&self) -> &[PackageInspectionManifestEntry] {
        &self.entries
    }

    pub(crate) async fn source_fingerprint(
        &self,
        model_dir: &Path,
        descriptor: &ModelExecutionDescriptor,
        metadata: &ModelMetadata,
        dependency_bindings: &[ModelDependencyBindingRecord],
    ) -> Result<String> {
        let model_dir = model_dir.to_path_buf();
        let descriptor_json = serde_json::to_string(descriptor)?;
        let metadata_json = serde_json::to_string(metadata)?;
        let dependency_bindings_json = serde_json::to_string(dependency_bindings)?;
        let fingerprint_files = self
            .entries
            .iter()
            .map(|entry| entry.relative_path.clone())
            .collect::<BTreeSet<_>>();

        tokio::task::spawn_blocking(move || {
            let mut hasher = Sha256::new();

            update_package_facts_hash_part(
                &mut hasher,
                "contract_version",
                &PACKAGE_FACTS_CONTRACT_VERSION.to_string(),
            );
            update_package_facts_hash_part(&mut hasher, "descriptor", &descriptor_json);
            update_package_facts_hash_part(&mut hasher, "metadata", &metadata_json);
            update_package_facts_hash_part(
                &mut hasher,
                "dependency_bindings",
                &dependency_bindings_json,
            );

            for relative_path in fingerprint_files {
                update_package_facts_hash_part(&mut hasher, "file", &relative_path);
                let path = model_dir.join(&relative_path);
                match std::fs::metadata(&path) {
                    Ok(metadata) => {
                        update_package_facts_hash_part(&mut hasher, "file_state", "present");
                        update_package_facts_hash_part(
                            &mut hasher,
                            "file_len",
                            &metadata.len().to_string(),
                        );
                        let modified = metadata
                            .modified()
                            .ok()
                            .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
                        if let Some(modified) = modified {
                            update_package_facts_hash_part(
                                &mut hasher,
                                "file_mtime_secs",
                                &modified.as_secs().to_string(),
                            );
                            update_package_facts_hash_part(
                                &mut hasher,
                                "file_mtime_nanos",
                                &modified.subsec_nanos().to_string(),
                            );
                        }
                    }
                    Err(_) => {
                        update_package_facts_hash_part(&mut hasher, "file_state", "missing");
                    }
                }
            }

            Ok::<_, PumasError>(hex::encode(hasher.finalize()))
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join package facts fingerprint task: {}",
                err
            ))
        })?
    }
}

impl PackageInspectionManifestEntry {
    #[cfg(test)]
    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

fn update_package_facts_hash_part(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    hasher.update([0xff]);
}

const STANDARD_PACKAGE_FACT_FILENAMES: &[&str] = &[
    "config.json",
    "generation_config.json",
    "tokenizer.json",
    "vocab.json",
    "merges.txt",
    "vocab.txt",
    "spiece.model",
    "sentencepiece.bpe.model",
    "tokenizer.model",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "processor_config.json",
    "preprocessor_config.json",
    "image_processor_config.json",
    "video_processor_config.json",
    "feature_extractor_config.json",
    "chat_template.jinja",
    "model_index.json",
    "adapter_config.json",
    "adapter_model.safetensors",
    "adapter_model.bin",
    "model.safetensors.index.json",
    "pytorch_model.bin.index.json",
    "requirements.txt",
    "custom_generate/generate.py",
    "custom_generate/requirements.txt",
];

const STANDARD_DIFFUSERS_PACKAGE_FACT_PATHS: &[&str] = &[
    "scheduler/scheduler_config.json",
    "transformer/config.json",
    "unet/config.json",
    "vae/config.json",
    "text_encoder/config.json",
    "text_encoder_2/config.json",
    "text_encoder_3/config.json",
    "tokenizer/tokenizer_config.json",
    "tokenizer/tokenizer.json",
    "tokenizer_2/tokenizer_config.json",
    "tokenizer_2/tokenizer.json",
    "processor/processor_config.json",
    "image_processor/config.json",
    "image_processor/preprocessor_config.json",
];

async fn package_manifest_entries(
    model_dir: &Path,
    selected_files: &[String],
) -> Result<Vec<PackageInspectionManifestEntry>> {
    let mut paths = selected_files.iter().cloned().collect::<BTreeSet<_>>();
    for relative_path in STANDARD_DIFFUSERS_PACKAGE_FACT_PATHS {
        if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            paths.insert((*relative_path).to_string());
        }
    }

    let chat_template_dir = model_dir.join("chat_templates");
    if tokio::fs::try_exists(&chat_template_dir).await? {
        let chat_template_paths = tokio::task::spawn_blocking(move || {
            let mut paths = BTreeSet::new();
            for entry in std::fs::read_dir(chat_template_dir).map_err(|err| PumasError::Io {
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
                if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("jinja")
                {
                    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                        paths.insert(format!("chat_templates/{}", name));
                    }
                }
            }
            Ok::<_, PumasError>(paths)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!("Failed to join package manifest scan: {}", err))
        })??;
        paths.extend(chat_template_paths);
    }

    Ok(paths
        .into_iter()
        .map(|relative_path| PackageInspectionManifestEntry { relative_path })
        .collect())
}

async fn package_selected_files(model_dir: &Path, metadata: &ModelMetadata) -> Result<Vec<String>> {
    let mut names = BTreeSet::new();
    if let Some(files) = metadata.files.as_ref() {
        names.extend(files.iter().map(|file| file.name.clone()));
        names.extend(
            metadata
                .expected_files
                .iter()
                .flatten()
                .map(std::string::ToString::to_string),
        );
    }

    for filename in STANDARD_PACKAGE_FACT_FILENAMES {
        if tokio::fs::try_exists(model_dir.join(filename)).await? {
            names.insert((*filename).to_string());
        }
    }

    if !names.is_empty() {
        return Ok(names.into_iter().collect());
    }

    let model_dir = model_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let files = WalkDir::new(model_dir)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| {
                entry
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(std::string::ToString::to_string)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok::<_, PumasError>(files)
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join package file scan: {}", err)))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AssetValidationState, ModelExecutionDescriptor, StorageKind};

    fn test_descriptor(root: &Path) -> ModelExecutionDescriptor {
        ModelExecutionDescriptor {
            execution_contract_version: 1,
            model_id: "image/test".to_string(),
            entry_path: root.to_string_lossy().to_string(),
            model_type: "image".to_string(),
            task_type_primary: "image_generation".to_string(),
            recommended_backend: None,
            runtime_engine_hints: Vec::new(),
            storage_kind: StorageKind::LibraryOwned,
            validation_state: AssetValidationState::Valid,
            dependency_resolution: None,
        }
    }

    #[tokio::test]
    async fn manifest_preserves_nested_diffusers_relative_paths() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let model_dir = temp_dir.path();
        std::fs::create_dir_all(model_dir.join("scheduler")).unwrap();
        std::fs::create_dir_all(model_dir.join("transformer")).unwrap();
        std::fs::write(model_dir.join("model_index.json"), "{}").unwrap();
        std::fs::write(model_dir.join("scheduler/scheduler_config.json"), "{}").unwrap();
        std::fs::write(model_dir.join("transformer/config.json"), "{}").unwrap();

        let manifest = PackageInspectionManifest::build(model_dir, &ModelMetadata::default())
            .await
            .unwrap();
        let entries = manifest
            .entries()
            .iter()
            .map(PackageInspectionManifestEntry::relative_path)
            .collect::<Vec<_>>();

        assert!(entries.contains(&"model_index.json"));
        assert!(entries.contains(&"scheduler/scheduler_config.json"));
        assert!(entries.contains(&"transformer/config.json"));
        assert_eq!(manifest.selected_files(), &["model_index.json".to_string()]);
    }

    #[tokio::test]
    async fn fingerprint_changes_when_nested_manifest_file_changes() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let model_dir = temp_dir.path();
        std::fs::create_dir_all(model_dir.join("scheduler")).unwrap();
        std::fs::write(model_dir.join("model_index.json"), "{}").unwrap();
        std::fs::write(model_dir.join("scheduler/scheduler_config.json"), "{}").unwrap();

        let metadata = ModelMetadata::default();
        let descriptor = test_descriptor(model_dir);
        let first_manifest = PackageInspectionManifest::build(model_dir, &metadata)
            .await
            .unwrap();
        let first = first_manifest
            .source_fingerprint(model_dir, &descriptor, &metadata, &[])
            .await
            .unwrap();

        std::fs::write(
            model_dir.join("scheduler/scheduler_config.json"),
            "{\"x\":1}",
        )
        .unwrap();
        let second_manifest = PackageInspectionManifest::build(model_dir, &metadata)
            .await
            .unwrap();
        let second = second_manifest
            .source_fingerprint(model_dir, &descriptor, &metadata, &[])
            .await
            .unwrap();

        assert_ne!(first, second);
    }
}
