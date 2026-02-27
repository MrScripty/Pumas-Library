//! HTTP client for interacting with a running Ollama instance.
//!
//! Provides model management operations (list, create, delete) via Ollama's
//! REST API. Used by the RPC handler to load library GGUF models into Ollama.
//!
//! The create flow uses the v0.5+ blob-based API:
//! 1. Compute SHA256 of the GGUF file
//! 2. Check if blob exists via `HEAD /api/blobs/sha256:{digest}`
//! 3. Upload blob via `POST /api/blobs/sha256:{digest}` if missing
//! 4. Create model via `POST /api/create` with `files` mapping

use futures::stream;
use pumas_library::config::AppId;
use pumas_library::{PumasError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tracing::{debug, info};

/// Default Ollama API base URL — delegates to [`AppId::Ollama`].
fn default_base_url() -> &'static str {
    AppId::Ollama.default_base_url()
}

/// Timeout for short API calls (list, delete, blob check).
const API_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for model creation (after blob is uploaded).
const CREATE_TIMEOUT: Duration = Duration::from_secs(300);

/// Chunk size for streaming blob uploads (8 MB).
const UPLOAD_CHUNK_SIZE: usize = 8 * 1024 * 1024;

/// Helper to create a network error.
fn net_err(msg: String) -> PumasError {
    PumasError::Network {
        message: msg,
        cause: None,
    }
}

/// A model registered in Ollama, as returned by `GET /api/tags`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: String,
}

/// Response from `GET /api/tags`.
#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Option<Vec<OllamaModel>>,
}

/// A model currently loaded in Ollama memory, as returned by `GET /api/ps`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningModel {
    pub name: String,
    pub size: u64,
    pub digest: String,
    #[serde(default)]
    pub size_vram: u64,
    pub expires_at: String,
}

/// Response from `GET /api/ps`.
#[derive(Debug, Deserialize)]
struct PsResponse {
    models: Option<Vec<RunningModel>>,
}

/// A single progress line from the streamed `POST /api/create` response.
#[derive(Debug, Deserialize)]
struct CreateProgressLine {
    status: String,
    #[serde(default)]
    error: Option<String>,
}

/// HTTP client for a running Ollama instance.
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
    /// Client with no total timeout for blob uploads (large files).
    upload_client: reqwest::Client,
    create_client: reqwest::Client,
}

impl OllamaClient {
    /// Create a new client targeting the given base URL.
    ///
    /// If `base_url` is `None`, defaults to `http://127.0.0.1:11434`.
    pub fn new(base_url: Option<&str>) -> Self {
        let base_url = base_url
            .unwrap_or(default_base_url())
            .trim_end_matches('/')
            .to_string();

        let client = reqwest::Client::builder()
            .timeout(API_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .expect("failed to build reqwest client");

        let upload_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // No total timeout -- large blob uploads can take a while.
            .user_agent("pumas-library")
            .build()
            .expect("failed to build reqwest upload client");

        let create_client = reqwest::Client::builder()
            .timeout(CREATE_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .expect("failed to build reqwest create client");

        Self {
            base_url,
            client,
            upload_client,
            create_client,
        }
    }

    /// List models registered in the running Ollama instance.
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>> {
        let url = format!("{}/api/tags", self.base_url);
        debug!("Listing Ollama models from {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to connect to Ollama at {}: {}", url, e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!("Ollama API returned {}: {}", status, body)));
        }

        let tags: TagsResponse = response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Ollama tags response: {}", e)))?;

        let models = tags.models.unwrap_or_default();
        info!("Ollama has {} registered models", models.len());
        Ok(models)
    }

    /// Create a model in Ollama from a local GGUF file.
    ///
    /// Uses the blob-based API (Ollama v0.5+):
    /// 1. Compute SHA256 of the GGUF file (or use `known_sha256` if provided)
    /// 2. Upload the file as a blob if not already present
    /// 3. Create the model with the `files` mapping
    pub async fn create_model(
        &self,
        name: &str,
        gguf_path: &Path,
        known_sha256: Option<&str>,
    ) -> Result<()> {
        info!(
            "Creating Ollama model '{}' from {}",
            name,
            gguf_path.display()
        );

        // Step 1: Get or compute SHA256 digest.
        let digest = match known_sha256 {
            Some(hash) => {
                debug!("Using pre-computed SHA256: {}", hash);
                hash.to_string()
            }
            None => {
                info!(
                    "Computing SHA256 for {} (this may take a moment for large files)",
                    gguf_path.display()
                );
                let path = gguf_path.to_path_buf();
                compute_sha256_async(&path).await?
            }
        };

        let digest_ref = format!("sha256:{}", digest);

        // Step 2: Check if the blob already exists in Ollama.
        if !self.blob_exists(&digest_ref).await? {
            // Step 3: Upload the GGUF file as a blob.
            self.upload_blob(&digest_ref, gguf_path).await?;
        } else {
            debug!(
                "Blob {} already exists in Ollama, skipping upload",
                digest_ref
            );
        }

        // Step 4: Create the model using the files mapping.
        let filename = gguf_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("model.gguf");

        let mut files = HashMap::new();
        files.insert(filename.to_string(), digest_ref);

        let url = format!("{}/api/create", self.base_url);
        let body = serde_json::json!({
            "model": name,
            "files": files,
        });

        debug!("Creating Ollama model with body: {}", body);

        let response = self
            .create_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to send create request to Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Ollama create API returned {}: {}",
                status, body
            )));
        }

        // Read the streamed NDJSON response to completion.
        let response_text: String = response
            .text()
            .await
            .map_err(|e| net_err(format!("Failed to read Ollama create response: {}", e)))?;

        // Check each line for errors.
        for line in response_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(progress) = serde_json::from_str::<CreateProgressLine>(trimmed) {
                if let Some(err) = progress.error {
                    return Err(net_err(format!("Ollama model creation failed: {}", err)));
                }
                debug!("Ollama create progress: {}", progress.status);
            }
        }

        info!("Successfully created Ollama model '{}'", name);
        Ok(())
    }

    /// Check if a blob exists in Ollama.
    async fn blob_exists(&self, digest: &str) -> Result<bool> {
        let url = format!("{}/api/blobs/{}", self.base_url, digest);
        debug!("Checking blob existence: {}", url);

        let response = self
            .client
            .head(&url)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to check Ollama blob: {}", e)))?;

        Ok(response.status().is_success())
    }

    /// Upload a file as a blob to Ollama, streaming the content.
    async fn upload_blob(&self, digest: &str, path: &Path) -> Result<()> {
        let url = format!("{}/api/blobs/{}", self.base_url, digest);
        let file_size = tokio::fs::metadata(path)
            .await
            .map_err(|e| PumasError::io_with_path(e, path))?
            .len();

        info!(
            "Uploading blob {} ({:.1} GB) to Ollama",
            digest,
            file_size as f64 / 1e9
        );

        // Stream the file in chunks to avoid loading it all into memory.
        let path_owned = path.to_path_buf();
        let file = tokio::fs::File::open(&path_owned)
            .await
            .map_err(|e| PumasError::io_with_path(e, &path_owned))?;

        let file_stream = stream::unfold(file, |mut file| async move {
            let mut buf = vec![0u8; UPLOAD_CHUNK_SIZE];
            match file.read(&mut buf).await {
                Ok(0) => None,
                Ok(n) => {
                    buf.truncate(n);
                    Some((Ok::<_, std::io::Error>(bytes::Bytes::from(buf)), file))
                }
                Err(e) => Some((Err(e), file)),
            }
        });

        let body = reqwest::Body::wrap_stream(file_stream);

        let response = self
            .upload_client
            .post(&url)
            .header("Content-Length", file_size)
            .body(body)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to upload blob to Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Ollama blob upload returned {}: {}",
                status, body
            )));
        }

        info!("Blob upload complete for {}", digest);
        Ok(())
    }

    /// Load a model into Ollama's memory (VRAM/RAM) for inference.
    ///
    /// Sends a generate request with an empty prompt to trigger model loading.
    /// Uses `keep_alive: -1` to keep the model loaded until explicitly unloaded.
    pub async fn load_model(&self, name: &str) -> Result<()> {
        let url = format!("{}/api/generate", self.base_url);
        info!("Loading Ollama model '{}' into memory", name);

        let body = serde_json::json!({
            "model": name,
            "prompt": "",
            "stream": false,
            "keep_alive": -1
        });

        let response = self
            .create_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to load model in Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Ollama load returned {}: {}",
                status, body
            )));
        }

        info!("Model '{}' loaded into memory", name);
        Ok(())
    }

    /// Unload a model from Ollama's memory.
    ///
    /// Sends a generate request with `keep_alive: 0` to immediately free VRAM/RAM.
    pub async fn unload_model(&self, name: &str) -> Result<()> {
        let url = format!("{}/api/generate", self.base_url);
        info!("Unloading Ollama model '{}' from memory", name);

        let body = serde_json::json!({
            "model": name,
            "prompt": "",
            "stream": false,
            "keep_alive": 0
        });

        let response = self
            .create_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to unload model from Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Ollama unload returned {}: {}",
                status, body
            )));
        }

        info!("Model '{}' unloaded from memory", name);
        Ok(())
    }

    /// List models currently loaded in Ollama's memory.
    pub async fn list_running_models(&self) -> Result<Vec<RunningModel>> {
        let url = format!("{}/api/ps", self.base_url);
        debug!("Listing running Ollama models from {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to connect to Ollama at {}: {}", url, e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!("Ollama API returned {}: {}", status, body)));
        }

        let ps: PsResponse = response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Ollama ps response: {}", e)))?;

        let models = ps.models.unwrap_or_default();
        debug!("Ollama has {} models loaded in memory", models.len());
        Ok(models)
    }

    /// Delete a model from the running Ollama instance.
    pub async fn delete_model(&self, name: &str) -> Result<()> {
        let url = format!("{}/api/delete", self.base_url);
        info!("Deleting Ollama model '{}'", name);

        let body = serde_json::json!({ "model": name });

        let response = self
            .client
            .delete(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to send delete request to Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Ollama delete API returned {}: {}",
                status, body
            )));
        }

        info!("Successfully deleted Ollama model '{}'", name);
        Ok(())
    }
}

/// Compute SHA256 of a file asynchronously (offloaded to blocking thread pool).
async fn compute_sha256_async(path: &PathBuf) -> Result<String> {
    let path = path.clone();
    tokio::task::spawn_blocking(move || {
        let mut file =
            std::fs::File::open(&path).map_err(|e| PumasError::io_with_path(e, &path))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; UPLOAD_CHUNK_SIZE];
        loop {
            let n = std::io::Read::read(&mut file, &mut buffer)
                .map_err(|e| PumasError::io_with_path(e, &path))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let hash = format!("{:x}", hasher.finalize());
        Ok(hash)
    })
    .await
    .map_err(|e| net_err(format!("SHA256 computation task failed: {}", e)))?
}

/// Derive an Ollama-friendly model name from a library display name.
///
/// Lowercases, replaces spaces and special characters with hyphens,
/// and collapses consecutive hyphens.
pub fn derive_ollama_name(display_name: &str) -> String {
    let name: String = display_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens and trim.
    let mut result = String::with_capacity(name.len());
    let mut last_was_hyphen = false;
    for c in name.chars() {
        if c == '-' {
            if !last_was_hyphen && !result.is_empty() {
                result.push('-');
            }
            last_was_hyphen = true;
        } else {
            result.push(c);
            last_was_hyphen = false;
        }
    }

    result.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_ollama_name() {
        assert_eq!(derive_ollama_name("Llama 2 7B"), "llama-2-7b");
        assert_eq!(derive_ollama_name("Mistral 7B Q4_K_M"), "mistral-7b-q4_k_m");
        assert_eq!(derive_ollama_name("my-model"), "my-model");
        assert_eq!(
            derive_ollama_name("Model  With   Spaces"),
            "model-with-spaces"
        );
        assert_eq!(derive_ollama_name("model.v2"), "model.v2");
    }
}
