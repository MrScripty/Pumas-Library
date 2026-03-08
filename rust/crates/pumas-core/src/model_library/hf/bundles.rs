use super::types::HF_HUB_BASE;
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::external_assets::{
    is_optional_component_marker, is_supported_text_to_image_pipeline,
    normalized_component_relative_path,
};
use crate::model_library::types::RepoFileTree;
use crate::models::BundleFormat;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HfRepoBundleClassification {
    pub bundle_format: BundleFormat,
    pub pipeline_class: String,
}

impl HuggingFaceClient {
    pub(crate) async fn classify_repo_bundle(
        &self,
        repo_id: &str,
    ) -> Result<Option<HfRepoBundleClassification>> {
        let tree = self.get_repo_files(repo_id).await?;
        if !tree.regular_files.iter().any(|path| path == "model_index.json") {
            return Ok(None);
        }

        let model_index = self.fetch_repo_text_file(repo_id, "model_index.json").await?;
        Ok(classify_repo_bundle_from_parts(&tree, &model_index))
    }

    async fn fetch_repo_text_file(&self, repo_id: &str, path: &str) -> Result<String> {
        let url = format!("{}/{}/resolve/main/{}", HF_HUB_BASE, repo_id, path);
        let mut request = self.client.get(&url);
        if let Some(auth) = self.auth_header_value().await {
            request = request.header("Authorization", auth);
        }

        let response = request.send().await.map_err(|e| PumasError::Network {
            message: format!("Failed to fetch {} from {}: {}", path, repo_id, e),
            cause: Some(e.to_string()),
        })?;

        if !response.status().is_success() {
            return Err(PumasError::Network {
                message: format!(
                    "HuggingFace Hub returned {} for {}/{}",
                    response.status(),
                    repo_id,
                    path
                ),
                cause: None,
            });
        }

        response.text().await.map_err(|e| PumasError::Network {
            message: format!("Failed to read {} from {}: {}", path, repo_id, e),
            cause: Some(e.to_string()),
        })
    }
}

pub(crate) fn classify_repo_bundle_from_parts(
    tree: &RepoFileTree,
    model_index_data: &str,
) -> Option<HfRepoBundleClassification> {
    let model_index: Value = serde_json::from_str(model_index_data).ok()?;
    let pipeline_class = model_index
        .get("_class_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    if !is_supported_text_to_image_pipeline(pipeline_class) {
        return None;
    }

    let repo_paths: HashSet<&str> = tree
        .regular_files
        .iter()
        .map(String::as_str)
        .chain(tree.lfs_files.iter().map(|file| file.filename.as_str()))
        .collect();

    let components = model_index.as_object()?;
    for (component_name, component_value) in components {
        if component_name.starts_with('_') || is_optional_component_marker(component_value) {
            continue;
        }

        let relative_path = normalized_component_relative_path(component_name).ok()?;
        let relative_path = relative_path.to_string_lossy().replace('\\', "/");
        let dir_prefix = format!("{}/", relative_path);
        let exists = repo_paths.contains(relative_path.as_str())
            || repo_paths
                .iter()
                .any(|path| path.starts_with(dir_prefix.as_str()));
        if !exists {
            return None;
        }
    }

    Some(HfRepoBundleClassification {
        bundle_format: BundleFormat::DiffusersDirectory,
        pipeline_class: pipeline_class.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::types::{LfsFileInfo, RepoFileTree, REPO_FILE_TREE_VERSION};

    fn repo_tree(regular_files: &[&str], lfs_files: &[&str]) -> RepoFileTree {
        RepoFileTree {
            repo_id: "hf-internal-testing/tiny-sd-turbo".to_string(),
            lfs_files: lfs_files
                .iter()
                .map(|filename| LfsFileInfo {
                    filename: (*filename).to_string(),
                    size: 1024,
                    sha256: "sha256".to_string(),
                })
                .collect(),
            regular_files: regular_files.iter().map(|path| (*path).to_string()).collect(),
            cached_at: chrono::Utc::now().to_rfc3339(),
            last_modified: None,
            cache_version: REPO_FILE_TREE_VERSION,
        }
    }

    #[test]
    fn classifies_supported_diffusers_repo_as_single_bundle() {
        let tree = repo_tree(
            &[
                "model_index.json",
                "tokenizer/tokenizer.json",
                "tokenizer/tokenizer_config.json",
            ],
            &[
                "unet/diffusion_pytorch_model.safetensors",
                "vae/diffusion_pytorch_model.safetensors",
                "text_encoder/model.safetensors",
            ],
        );

        let classification = classify_repo_bundle_from_parts(
            &tree,
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"],
  "text_encoder": ["transformers", "CLIPTextModel"],
  "tokenizer": ["transformers", "CLIPTokenizer"]
}"#,
        )
        .unwrap();

        assert_eq!(classification.bundle_format, BundleFormat::DiffusersDirectory);
        assert_eq!(classification.pipeline_class, "StableDiffusionPipeline");
    }

    #[test]
    fn does_not_classify_repo_with_missing_component() {
        let tree = repo_tree(&["model_index.json"], &["unet/diffusion_pytorch_model.safetensors"]);

        let classification = classify_repo_bundle_from_parts(
            &tree,
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"]
}"#,
        );

        assert!(classification.is_none());
    }

    #[test]
    fn does_not_classify_unsupported_pipeline_repo() {
        let tree = repo_tree(
            &["model_index.json"],
            &[
                "unet/diffusion_pytorch_model.safetensors",
                "vae/diffusion_pytorch_model.safetensors",
            ],
        );

        let classification = classify_repo_bundle_from_parts(
            &tree,
            r#"{
  "_class_name": "StableDiffusionControlNetPipeline",
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"]
}"#,
        );

        assert!(classification.is_none());
    }
}
