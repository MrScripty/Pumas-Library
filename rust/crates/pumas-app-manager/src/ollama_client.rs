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
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tracing::{debug, info, warn};

use crate::provider_http::{bypass_proxy_for_loopback, is_loopback_url, parse_provider_url};

mod naming;

pub use naming::derive_ollama_name;

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

/// Bytes read from an Ollama error response before the rest is discarded,
/// matching the RPC gateway's bound for a provider's own error reason.
const ERROR_BODY_READ_BYTES: usize = 64 * 1024;

/// Characters of an Ollama error reason embedded in a Pumas error: enough for
/// Ollama's one-line `{"error": "..."}` messages, short enough that a large or
/// hostile upstream body cannot flood logs and RPC replies.
const ERROR_REASON_CHARS: usize = 512;

/// Ollama's reason for an unsuccessful response: its `error` field when the
/// body is JSON, otherwise the body text, read and embedded within bounds.
async fn error_reason(response: reqwest::Response) -> String {
    use futures::StreamExt;
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut body = response.bytes_stream();
    while let Some(Ok(chunk)) = body.next().await {
        let room = ERROR_BODY_READ_BYTES - bytes.len();
        if chunk.len() > room {
            bytes.extend_from_slice(&chunk[..room]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| value.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
    let mut reason: String = text
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(ERROR_REASON_CHARS)
        .collect();
    if truncated || text.trim().chars().count() > ERROR_REASON_CHARS {
        reason.push_str("...");
    }
    reason
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
#[derive(Clone)]
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
    /// Client with no total timeout for blob uploads (large files).
    upload_client: reqwest::Client,
    create_client: reqwest::Client,
}

#[derive(Clone)]
pub struct OllamaHttpClients {
    client: reqwest::Client,
    upload_client: reqwest::Client,
    create_client: reqwest::Client,
}

impl OllamaHttpClients {
    /// Build the shared clients. A loopback Ollama is always reached
    /// directly; a configured remote Ollama keeps the environment's proxies.
    pub fn new() -> Result<Self> {
        let builder = || bypass_proxy_for_loopback(reqwest::Client::builder());
        let client = builder()
            .timeout(API_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .map_err(|err| net_err(format!("failed to build Ollama API client: {err}")))?;

        let upload_client = builder()
            .connect_timeout(Duration::from_secs(10))
            // No total timeout -- large blob uploads can take a while.
            .user_agent("pumas-library")
            .build()
            .map_err(|err| net_err(format!("failed to build Ollama upload client: {err}")))?;

        let create_client = builder()
            .timeout(CREATE_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .map_err(|err| net_err(format!("failed to build Ollama create client: {err}")))?;

        Ok(Self {
            client,
            upload_client,
            create_client,
        })
    }
}

impl OllamaClient {
    /// Create a client for an Ollama on this machine.
    ///
    /// If `base_url` is `None`, defaults to `http://127.0.0.1:11434`. The URL
    /// must be http(s) on a loopback host; a remote Ollama is reached through
    /// a configured runtime profile endpoint with [`Self::with_http_clients`].
    pub fn new(base_url: Option<&str>) -> Result<Self> {
        let url = parse_provider_url(base_url.unwrap_or(default_base_url()), "Ollama")?;
        if !is_loopback_url(&url) {
            return Err(PumasError::Config {
                message: format!(
                    "Ollama base URL host {} is not loopback; configure a remote Ollama as a runtime profile endpoint",
                    url.host_str().unwrap_or_default()
                ),
            });
        }
        Ok(Self::with_http_clients(base_url, OllamaHttpClients::new()?))
    }

    /// Create a client for a configured endpoint, which may be a remote
    /// Ollama (a supported runtime profile configuration). Callers pass a
    /// validated endpoint such as a `RuntimeEndpointUrl`. A plain-http remote
    /// endpoint is accepted but logged, since its traffic is unencrypted.
    pub fn with_http_clients(base_url: Option<&str>, http_clients: OllamaHttpClients) -> Self {
        let base_url = base_url
            .unwrap_or(default_base_url())
            .trim_end_matches('/')
            .to_string();
        // Clients are built per request, so each endpoint is reported once.
        static WARNED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
            std::sync::OnceLock::new();
        if let Ok(url) = reqwest::Url::parse(&base_url) {
            if url.scheme() == "http"
                && !is_loopback_url(&url)
                && WARNED
                    .get_or_init(Default::default)
                    .lock()
                    .is_ok_and(|mut warned| warned.insert(base_url.clone()))
            {
                warn!(
                    "Ollama endpoint {} is a non-loopback host over plain http; prompts and models are sent unencrypted",
                    url.host_str().unwrap_or_default()
                );
            }
        }

        Self {
            base_url,
            client: http_clients.client,
            upload_client: http_clients.upload_client,
            create_client: http_clients.create_client,
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
            let body = error_reason(response).await;
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
            let body = error_reason(response).await;
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
            let body = error_reason(response).await;
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
        self.load_model_with_keep_alive(name, true).await
    }

    /// Load a model into Ollama memory, optionally pinning it until explicit unload.
    pub async fn load_model_with_keep_alive(&self, name: &str, keep_loaded: bool) -> Result<()> {
        self.load_with_keep_alive(name, keep_loaded, false).await
    }

    /// Load an embedding model. Ollama refuses `/api/generate` for models that
    /// cannot generate, so embedding models are loaded through `/api/embed`.
    pub async fn load_embedding_model_with_keep_alive(
        &self,
        name: &str,
        keep_loaded: bool,
    ) -> Result<()> {
        self.load_with_keep_alive(name, keep_loaded, true).await
    }

    async fn load_with_keep_alive(
        &self,
        name: &str,
        keep_loaded: bool,
        embedding: bool,
    ) -> Result<()> {
        let (url, mut body) = if embedding {
            (
                format!("{}/api/embed", self.base_url),
                // A short input makes Ollama start the runner; the vector is discarded.
                serde_json::json!({ "model": name, "input": "load" }),
            )
        } else {
            (
                format!("{}/api/generate", self.base_url),
                serde_json::json!({ "model": name, "prompt": "", "stream": false }),
            )
        };
        info!("Loading Ollama model '{}' into memory", name);
        if keep_loaded {
            body["keep_alive"] = serde_json::json!(-1);
        }

        let response = self
            .create_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to load model in Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = error_reason(response).await;
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
            let body = error_reason(response).await;
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
            let body = error_reason(response).await;
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
            let body = error_reason(response).await;
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
async fn compute_sha256_async(path: &Path) -> Result<String> {
    let path = path.to_path_buf();
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn new_requires_a_loopback_http_endpoint() {
        assert!(OllamaClient::new(None).is_ok());
        assert!(OllamaClient::new(Some("http://localhost:11434")).is_ok());
        assert!(OllamaClient::new(Some("http://[::1]:11434/")).is_ok());
        for refused in [
            "http://192.168.1.20:11434",
            "https://ollama.example.com",
            "ftp://127.0.0.1",
            "not a url",
        ] {
            assert!(OllamaClient::new(Some(refused)).is_err(), "{refused}");
        }
        let error = OllamaClient::new(Some("http://192.168.1.20:11434"))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("not loopback"), "{error}");
    }

    #[test]
    fn configured_remote_endpoints_remain_supported() {
        let client = OllamaClient::with_http_clients(
            Some("http://192.168.1.20:11434/"),
            OllamaHttpClients::new().unwrap(),
        );
        assert_eq!(client.base_url, "http://192.168.1.20:11434");
    }

    #[tokio::test]
    async fn upstream_error_bodies_are_bounded_in_errors() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for body in [
                serde_json::json!({ "error": "model \"x\" not found" }).to_string(),
                "e\n".repeat(1 << 20),
            ] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request).await;
                let _ = stream
                    .write_all(
                        format!(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
                let _ = stream.write_all(body.as_bytes()).await;
            }
        });
        let client = OllamaClient::new(Some(&base)).unwrap();

        let error = client.list_models().await.unwrap_err().to_string();
        assert!(
            error.ends_with("500 Internal Server Error: model \"x\" not found"),
            "{error}"
        );

        let error = client.list_models().await.unwrap_err().to_string();
        assert!(error.len() < 1024, "{} bytes", error.len());
        assert!(error.ends_with("..."), "{error}");
        assert!(!error.contains('\n'));
        server.await.unwrap();
    }
}
