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
}

impl PackageInspectionManifest {
    pub(crate) async fn build(model_dir: &Path, metadata: &ModelMetadata) -> Result<Self> {
        Ok(Self {
            selected_files: package_selected_files(model_dir, metadata).await?,
        })
    }

    pub(crate) fn selected_files(&self) -> &[String] {
        &self.selected_files
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
        let selected_files = self.selected_files.iter().cloned().collect::<BTreeSet<_>>();

        tokio::task::spawn_blocking(move || {
            let mut hasher = Sha256::new();
            let mut fingerprint_files = selected_files;
            if let Ok(entries) = std::fs::read_dir(model_dir.join("chat_templates")) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path.extension().and_then(|ext| ext.to_str()) == Some("jinja")
                    {
                        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                            fingerprint_files.insert(format!("chat_templates/{}", name));
                        }
                    }
                }
            }

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
