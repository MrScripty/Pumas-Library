use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::external_assets::{
    is_diffusers_component_entry, is_optional_component_marker,
    is_supported_text_to_image_pipeline, normalized_component_relative_path,
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
        self.classify_repo_bundle_at_revision(repo_id, &DownloadRevision::legacy_main())
            .await
    }

    pub(crate) async fn classify_repo_bundle_at_revision(
        &self,
        repo_id: &str,
        revision: &DownloadRevision,
    ) -> Result<Option<HfRepoBundleClassification>> {
        let tree = self.get_repo_files_at_revision(repo_id, revision).await?;
        if !tree
            .regular_files
            .iter()
            .any(|path| path == "model_index.json")
        {
            return Ok(None);
        }

        let model_index = self
            .fetch_repo_text_file(repo_id, "model_index.json", revision)
            .await?;
        Ok(classify_repo_bundle_from_parts(&tree, &model_index))
    }

    async fn fetch_repo_text_file(
        &self,
        repo_id: &str,
        path: &str,
        revision: &DownloadRevision,
    ) -> Result<String> {
        let url = format!(
            "{}/{}/resolve/{}/{}",
            self.hub_base_url(),
            repo_id,
            revision.as_str(),
            path
        );
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
        if component_name.starts_with('_')
            || !is_diffusers_component_entry(component_value)
            || is_optional_component_marker(component_value)
        {
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
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
            regular_files: regular_files
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
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

        assert_eq!(
            classification.bundle_format,
            BundleFormat::DiffusersDirectory
        );
        assert_eq!(classification.pipeline_class, "StableDiffusionPipeline");
    }

    #[test]
    fn does_not_classify_repo_with_missing_component() {
        let tree = repo_tree(
            &["model_index.json"],
            &["unet/diffusion_pytorch_model.safetensors"],
        );

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

    async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            assert!(
                bytes.len() < 8 * 1024,
                "fixture request exceeded header limit"
            );
            bytes.push(socket.read_u8().await.unwrap());
        }
        String::from_utf8(bytes).unwrap()
    }

    async fn write_response(socket: &mut tokio::net::TcpStream, content_type: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    }

    #[tokio::test]
    async fn pinned_bundle_fetches_tree_and_model_index_at_same_commit() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut tree_socket, _) = listener.accept().await.unwrap();
            let tree_request = read_request(&mut tree_socket).await;
            write_response(
                &mut tree_socket,
                "application/json",
                r#"[
                    {"path":"model_index.json","type":"file"},
                    {"path":"unet/diffusion_pytorch_model.safetensors","type":"file","lfs":{"oid":"abc","size":1}}
                ]"#,
            )
            .await;

            let (mut index_socket, _) = listener.accept().await.unwrap();
            let index_request = read_request(&mut index_socket).await;
            write_response(
                &mut index_socket,
                "application/json",
                r#"{
                    "_class_name":"StableDiffusionPipeline",
                    "unet":["diffusers","UNet2DConditionModel"]
                }"#,
            )
            .await;
            (tree_request, index_request)
        });
        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        client.set_test_download_base_url(base_url);
        let revision = DownloadRevision::from_commit(commit).unwrap();

        let classification = client
            .classify_repo_bundle_at_revision("acme/diffusers", &revision)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            classification.bundle_format,
            BundleFormat::DiffusersDirectory
        );
        let (tree_request, index_request) = server.await.unwrap();
        assert!(tree_request.starts_with(&format!(
            "GET /api/models/acme/diffusers/tree/{commit}?recursive=true HTTP/1.1"
        )));
        assert!(index_request.starts_with(&format!(
            "GET /acme/diffusers/resolve/{commit}/model_index.json HTTP/1.1"
        )));
    }
}
