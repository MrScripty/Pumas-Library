use crate::error::{PumasError, Result};
use crate::index::ModelDependencyBindingRecord;
use crate::model_library::external_assets::normalized_component_relative_path;
use crate::model_library::types::ModelMetadata;
use crate::models::{
    ModelExecutionDescriptor, PackageFactStatus, PackageFactValueSource,
    PackageInspectionManifest as ContractPackageInspectionManifest,
    PackageInspectionManifestEntry as ContractPackageInspectionManifestEntry,
    PACKAGE_FACTS_CONTRACT_VERSION,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};
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
    size_bytes: Option<u64>,
    status: PackageFactStatus,
    value_source: PackageFactValueSource,
}

impl PackageInspectionManifest {
    pub(crate) async fn build(model_dir: &Path, metadata: &ModelMetadata) -> Result<Self> {
        let selected_files = package_selected_files(model_dir, metadata).await?;
        let entries = package_manifest_entries(model_dir, metadata, &selected_files).await?;
        Ok(Self {
            selected_files,
            entries,
        })
    }

    pub(crate) fn selected_files(&self) -> &[String] {
        &self.selected_files
    }

    pub(crate) fn entries(&self) -> &[PackageInspectionManifestEntry] {
        &self.entries
    }

    pub(crate) fn to_contract(&self) -> ContractPackageInspectionManifest {
        ContractPackageInspectionManifest {
            entries: self
                .entries
                .iter()
                .map(PackageInspectionManifestEntry::to_contract)
                .collect(),
        }
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
    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(crate) fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub(crate) fn status(&self) -> PackageFactStatus {
        self.status
    }

    pub(crate) fn value_source(&self) -> PackageFactValueSource {
        self.value_source
    }

    fn to_contract(&self) -> ContractPackageInspectionManifestEntry {
        ContractPackageInspectionManifestEntry {
            relative_path: self.relative_path.clone(),
            size_bytes: self.size_bytes,
            status: self.status,
            value_source: self.value_source,
        }
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

const STANDARD_DIFFUSERS_PACKAGE_WEIGHT_PATHS: &[&str] = &[
    "transformer/diffusion_pytorch_model.safetensors",
    "transformer/diffusion_pytorch_model.bin",
    "transformer/model.safetensors",
    "transformer/pytorch_model.bin",
    "unet/diffusion_pytorch_model.safetensors",
    "unet/diffusion_pytorch_model.bin",
    "unet/model.safetensors",
    "unet/pytorch_model.bin",
    "vae/diffusion_pytorch_model.safetensors",
    "vae/diffusion_pytorch_model.bin",
    "vae/model.safetensors",
    "vae/pytorch_model.bin",
    "text_encoder/model.safetensors",
    "text_encoder/pytorch_model.bin",
    "text_encoder_2/model.safetensors",
    "text_encoder_2/pytorch_model.bin",
    "text_encoder_3/model.safetensors",
    "text_encoder_3/pytorch_model.bin",
    "controlnet/diffusion_pytorch_model.safetensors",
    "controlnet/diffusion_pytorch_model.bin",
    "adapter/adapter_model.safetensors",
    "adapter/adapter_model.bin",
];

const STANDARD_DIFFUSERS_WEIGHT_DIRECTORIES: &[&str] = &[
    "transformer",
    "unet",
    "vae",
    "text_encoder",
    "text_encoder_2",
    "text_encoder_3",
    "controlnet",
    "adapter",
];

const DIFFUSERS_MODEL_INDEX_PATH: &str = "model_index.json";
const MAX_DIFFUSERS_MODEL_INDEX_BYTES: u64 = 16 * 1024 * 1024;

async fn package_manifest_entries(
    model_dir: &Path,
    metadata: &ModelMetadata,
    selected_files: &[String],
) -> Result<Vec<PackageInspectionManifestEntry>> {
    let mut paths = selected_files.iter().cloned().collect::<BTreeSet<_>>();
    extend_metadata_package_paths(metadata, &mut paths);
    for relative_path in STANDARD_DIFFUSERS_PACKAGE_FACT_PATHS {
        if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            paths.insert((*relative_path).to_string());
        }
    }
    for relative_path in STANDARD_DIFFUSERS_PACKAGE_WEIGHT_PATHS {
        if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            paths.insert((*relative_path).to_string());
        }
    }
    paths.extend(diffusers_component_weight_paths(model_dir).await?);
    paths.extend(weight_index_declared_shard_paths(model_dir, &paths).await?);

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

    let mut entries = Vec::new();
    for relative_path in paths {
        entries.push(package_manifest_entry(model_dir, relative_path).await?);
    }

    Ok(entries)
}

fn extend_metadata_package_paths(metadata: &ModelMetadata, paths: &mut BTreeSet<String>) {
    if let Some(files) = metadata.files.as_ref() {
        paths.extend(
            files
                .iter()
                .filter_map(|file| normalize_package_relative_path(&file.name)),
        );
    }
    paths.extend(
        metadata
            .expected_files
            .iter()
            .flatten()
            .filter_map(|path| normalize_package_relative_path(path)),
    );
}

async fn diffusers_component_weight_paths(model_dir: &Path) -> Result<BTreeSet<String>> {
    let mut component_dirs = STANDARD_DIFFUSERS_WEIGHT_DIRECTORIES
        .iter()
        .map(|relative_dir| (*relative_dir).to_string())
        .collect::<BTreeSet<_>>();
    component_dirs.extend(model_index_component_weight_directories(model_dir).await?);

    let mut paths = BTreeSet::new();
    for relative_dir in component_dirs {
        paths.extend(scan_diffusers_component_weight_dir(model_dir, &relative_dir).await?);
    }

    Ok(paths)
}

async fn model_index_component_weight_directories(model_dir: &Path) -> Result<BTreeSet<String>> {
    let model_index_path = model_dir.join(DIFFUSERS_MODEL_INDEX_PATH);
    let metadata = match tokio::fs::metadata(&model_index_path).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(err) => return Err(PumasError::io_with_path(err, &model_index_path)),
    };
    if !metadata.is_file() {
        return Ok(BTreeSet::new());
    }

    tokio::task::spawn_blocking(move || {
        let Some(model_index) = read_bounded_utf8_file(&model_index_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(BTreeSet::new());
        };

        Ok::<_, PumasError>(model_index_component_directories(&model_index))
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join Diffusers model index component scan: {}",
            err
        ))
    })?
}

fn model_index_component_directories(model_index: &Value) -> BTreeSet<String> {
    let Some(object) = model_index.as_object() else {
        return BTreeSet::new();
    };

    object
        .iter()
        .filter(|(component_name, value)| {
            !component_name.starts_with('_') && is_model_index_component_reference(value)
        })
        .filter_map(|(component_name, _value)| {
            let component_path = normalized_component_relative_path(component_name).ok()?;
            normalize_package_relative_path(component_path.to_str()?)
        })
        .collect()
}

fn is_model_index_component_reference(value: &Value) -> bool {
    let Some(entries) = value.as_array() else {
        return false;
    };
    if entries.len() < 2 {
        return false;
    }
    entries
        .first()
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
        && entries
            .get(1)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
}

async fn scan_diffusers_component_weight_dir(
    model_dir: &Path,
    relative_dir: &str,
) -> Result<BTreeSet<String>> {
    let component_dir = model_dir.join(relative_dir);
    let metadata = match tokio::fs::metadata(&component_dir).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(err) => return Err(PumasError::io_with_path(err, &component_dir)),
    };
    if !metadata.is_dir() {
        return Ok(BTreeSet::new());
    }

    let relative_dir = relative_dir.to_string();
    tokio::task::spawn_blocking(move || {
        let mut paths = BTreeSet::new();
        for entry in std::fs::read_dir(&component_dir).map_err(|err| PumasError::Io {
            message: "Failed to read Diffusers component directory".to_string(),
            path: Some(component_dir.clone()),
            source: Some(err),
        })? {
            let entry = entry.map_err(|err| PumasError::Io {
                message: "Failed to read Diffusers component entry".to_string(),
                path: Some(component_dir.clone()),
                source: Some(err),
            })?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if is_package_weight_file(name) {
                paths.insert(format!("{relative_dir}/{name}"));
            }
        }
        Ok::<_, PumasError>(paths)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join Diffusers component weight scan: {}",
            err
        ))
    })?
}

fn read_bounded_utf8_file(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path).map_err(|err| PumasError::io_with_path(err, path))?;
    if metadata.len() > MAX_DIFFUSERS_MODEL_INDEX_BYTES {
        return Err(PumasError::Other(format!(
            "{} exceeds bounded Diffusers model index limit of {} bytes",
            path.display(),
            MAX_DIFFUSERS_MODEL_INDEX_BYTES
        )));
    }

    let mut file = File::open(path).map_err(|err| PumasError::io_with_path(err, path))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_DIFFUSERS_MODEL_INDEX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| PumasError::io_with_path(err, path))?;
    if bytes.len() as u64 > MAX_DIFFUSERS_MODEL_INDEX_BYTES {
        return Err(PumasError::Other(format!(
            "{} exceeds bounded Diffusers model index limit of {} bytes",
            path.display(),
            MAX_DIFFUSERS_MODEL_INDEX_BYTES
        )));
    }

    String::from_utf8(bytes)
        .map_err(|_| PumasError::Other(format!("{} is not valid UTF-8", path.display())))
}

async fn weight_index_declared_shard_paths(
    model_dir: &Path,
    manifest_paths: &BTreeSet<String>,
) -> Result<BTreeSet<String>> {
    let mut shard_paths = BTreeSet::new();
    for relative_path in manifest_paths
        .iter()
        .filter(|path| is_transformers_weight_index_file(path))
    {
        shard_paths.extend(declared_shards_from_weight_index(model_dir.join(relative_path)).await?);
    }
    Ok(shard_paths)
}

async fn declared_shards_from_weight_index(path: PathBuf) -> Result<Vec<String>> {
    tokio::task::spawn_blocking(move || {
        let Some(index) = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        else {
            return Ok(Vec::new());
        };
        let Some(weight_map) = index
            .get("weight_map")
            .and_then(serde_json::Value::as_object)
        else {
            return Ok(Vec::new());
        };

        let shards = weight_map
            .values()
            .filter_map(serde_json::Value::as_str)
            .filter_map(normalize_package_relative_path)
            .collect::<BTreeSet<_>>();
        Ok::<_, PumasError>(shards.into_iter().collect())
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join weight index parse: {}", err)))?
}

async fn package_selected_files(model_dir: &Path, metadata: &ModelMetadata) -> Result<Vec<String>> {
    let mut names = BTreeSet::new();
    if let Some(selected_artifact_files) = metadata.selected_artifact_files.as_ref() {
        names.extend(
            selected_artifact_files
                .iter()
                .filter_map(|path| normalize_package_relative_path(path)),
        );
    } else if let Some(files) = metadata.files.as_ref() {
        names.extend(
            files
                .iter()
                .filter_map(|file| normalize_package_relative_path(&file.name)),
        );
        names.extend(
            metadata
                .expected_files
                .iter()
                .flatten()
                .filter_map(|path| normalize_package_relative_path(path)),
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
                    .file_name()?
                    .to_str()
                    .and_then(normalize_package_relative_path)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok::<_, PumasError>(files)
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join package file scan: {}", err)))?
}

async fn package_manifest_entry(
    model_dir: &Path,
    relative_path: String,
) -> Result<PackageInspectionManifestEntry> {
    match tokio::fs::metadata(model_dir.join(&relative_path)).await {
        Ok(metadata) if metadata.is_file() => Ok(PackageInspectionManifestEntry {
            relative_path,
            size_bytes: Some(metadata.len()),
            status: PackageFactStatus::Present,
            value_source: PackageFactValueSource::FilesystemMetadata,
        }),
        Ok(_) | Err(_) => Ok(PackageInspectionManifestEntry {
            relative_path,
            size_bytes: None,
            status: PackageFactStatus::Missing,
            value_source: PackageFactValueSource::Unavailable,
        }),
    }
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

fn is_transformers_weight_index_file(relative_path: &str) -> bool {
    let relative_path = relative_path.to_lowercase();
    relative_path.ends_with(".safetensors.index.json")
        || relative_path.ends_with(".bin.index.json")
        || relative_path.ends_with(".pt.index.json")
}

fn is_package_weight_file(file_name: &str) -> bool {
    let file_name = file_name.to_lowercase();
    ["safetensors", "bin", "pt", "pth", "ckpt", "gguf", "onnx"]
        .iter()
        .any(|extension| file_name.ends_with(&format!(".{extension}")))
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
        std::fs::create_dir_all(model_dir.join("image_encoder")).unwrap();
        let model_index_json = r#"{
  "image_encoder": ["transformers", "CLIPVisionModelWithProjection"]
}"#;
        std::fs::write(model_dir.join("model_index.json"), model_index_json).unwrap();
        std::fs::write(model_dir.join("scheduler/scheduler_config.json"), "{}").unwrap();
        std::fs::write(model_dir.join("transformer/config.json"), "{}").unwrap();
        std::fs::write(
            model_dir.join("image_encoder/model.fp16.safetensors"),
            "weights",
        )
        .unwrap();

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
        assert!(entries.contains(&"image_encoder/model.fp16.safetensors"));
        let model_index = manifest
            .entries()
            .iter()
            .find(|entry| entry.relative_path() == "model_index.json")
            .expect("model index manifest entry");
        assert_eq!(model_index.status(), PackageFactStatus::Present);
        assert_eq!(
            model_index.size_bytes(),
            Some(model_index_json.len() as u64)
        );
        assert_eq!(
            model_index.value_source(),
            PackageFactValueSource::FilesystemMetadata
        );
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
