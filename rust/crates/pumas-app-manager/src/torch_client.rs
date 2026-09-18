//! HTTP client for interacting with the Pumas Torch inference server.
//!
//! Provides model slot management (list, load, unload), device discovery,
//! and server configuration via the Torch server's REST API. The Torch server
//! is a Python-based inference engine managed as a subprocess by Pumas.
//!
//! ## API Surface
//!
//! - `/health`      — Server health check
//! - `/api/slots`   — List loaded model slots
//! - `/api/load`    — Load a model into a slot on a specific device
//! - `/api/unload`  — Unload a model slot
//! - `/api/status`  — Server status with resource usage
//! - `/api/devices` — Available compute devices
//! - `/api/configure` — Update server configuration

use futures::StreamExt;
use pumas_library::config::AppId;
use pumas_library::{PumasError, Result};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;
use tracing::{debug, info};

/// Default Torch server API base URL — delegates to [`AppId::Torch`].
fn default_base_url() -> &'static str {
    AppId::Torch.default_base_url()
}

/// Timeout for short API calls (list, status, devices).
const API_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for model loading (can take minutes for large models).
const LOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// Timeout for image generation. The sidecar deadline is 600 seconds and the
/// gateway transport allowance is 615 seconds, so this stays inside both.
const IMAGE_TIMEOUT: Duration = Duration::from_secs(600);

/// Single owner of the Torch sidecar protocol version spoken by this build.
/// The installer recipe check and the runtime handshake both use this value.
pub const SUPPORTED_TORCH_PROTOCOL: u32 = 2;

/// Capability advertised by image-capable Torch sidecars.
pub const TORCH_IMAGE_GENERATION_CAPABILITY: &str = "image_generation";

/// Upper bound for a sidecar image-generation response body (12 MiB gateway JSON).
const MAX_IMAGE_RESPONSE_BYTES: usize = 12 * 1024 * 1024;

/// Helper to create a network error.
fn net_err(msg: String) -> PumasError {
    PumasError::Network {
        message: msg,
        cause: None,
    }
}

// =============================================================================
// Types
// =============================================================================

/// Compute device identifier for model placement.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeDevice {
    /// CPU inference.
    Cpu,
    /// NVIDIA CUDA GPU (by index).
    Cuda(u32),
    /// Apple Metal Performance Shaders.
    Mps,
    /// Let the server choose the best available device.
    Auto,
}

impl ComputeDevice {
    /// Serialize to the string format used by the Python server.
    pub fn to_server_string(&self) -> String {
        match self {
            ComputeDevice::Cpu => "cpu".to_string(),
            ComputeDevice::Cuda(idx) => format!("cuda:{}", idx),
            ComputeDevice::Mps => "mps".to_string(),
            ComputeDevice::Auto => "auto".to_string(),
        }
    }

    /// Parse from the string format used by the Python server.
    pub fn from_server_string(s: &str) -> Self {
        match s {
            "cpu" => ComputeDevice::Cpu,
            "mps" => ComputeDevice::Mps,
            "auto" => ComputeDevice::Auto,
            s if s.starts_with("cuda:") => {
                let idx = s[5..].parse::<u32>().unwrap_or(0);
                ComputeDevice::Cuda(idx)
            }
            _ => ComputeDevice::Auto,
        }
    }
}

/// State of a model slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotState {
    /// Slot registered but model not loaded.
    Unloaded,
    /// Model is currently being loaded.
    Loading,
    /// Model is loaded and ready for inference.
    Ready,
    /// Model is being unloaded.
    Unloading,
    /// Error occurred during load/unload.
    Error,
}

/// A model slot in the Torch inference server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSlot {
    /// Unique slot identifier.
    pub slot_id: String,
    /// Display name for the model.
    pub model_name: String,
    /// Path to the model files on disk.
    pub model_path: String,
    /// Assigned compute device.
    pub device: String,
    /// Current slot state.
    pub state: SlotState,
    /// GPU memory usage in bytes (if on a GPU device).
    #[serde(default)]
    pub gpu_memory_bytes: Option<u64>,
    /// RAM usage in bytes (if on CPU).
    #[serde(default)]
    pub ram_memory_bytes: Option<u64>,
    /// Model type (e.g., "text-generation", "dllm", "sherry").
    #[serde(default)]
    pub model_type: Option<String>,
}

/// Library-resolved components for a concrete image adapter.
pub struct ImageModelComponents<'a> {
    /// Complete pipeline directory, or the standalone text encoder directory.
    pub pipeline_path: &'a str,
    /// Separate VAE checkpoint when the adapter uses standalone components.
    pub vae_path: Option<&'a str>,
}

/// Information about an available compute device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device identifier (e.g., "cpu", "cuda:0", "mps").
    pub device_id: String,
    /// Human-readable name (e.g., "NVIDIA RTX 4090").
    pub name: String,
    /// Total memory in bytes.
    pub memory_total: u64,
    /// Available memory in bytes.
    pub memory_available: u64,
    /// Whether the device is currently usable.
    pub is_available: bool,
}

/// Server configuration for the Torch inference process.
///
/// Local-only loopback binding is the default. Non-loopback binding is supported only when
/// `lan_access` is explicitly enabled here, and the Python sidecar separately requires both
/// `PUMAS_TORCH_ALLOW_LAN=1` and `PUMAS_TORCH_API_TOKEN` before accepting LAN traffic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorchServerConfig {
    /// Port for the OpenAI-compatible API.
    pub api_port: u16,
    /// Host to bind to ("127.0.0.1" for local only, "0.0.0.0" for LAN).
    pub host: String,
    /// Maximum models to keep loaded simultaneously.
    pub max_loaded_models: usize,
    /// Whether LAN access is enabled (user opt-in).
    pub lan_access: bool,
}

impl Default for TorchServerConfig {
    fn default() -> Self {
        Self {
            api_port: 8400,
            host: "127.0.0.1".to_string(),
            max_loaded_models: 4,
            lan_access: false,
        }
    }
}

impl TorchServerConfig {
    /// Validate host/LAN policy before sending it to the Torch server.
    pub fn validate(&self) -> Result<()> {
        let host: IpAddr = self.host.parse().map_err(|_| PumasError::InvalidParams {
            message: format!(
                "Torch server host must be an IP address, got '{}'",
                self.host
            ),
        })?;

        if !self.lan_access && !host.is_loopback() {
            return Err(PumasError::InvalidParams {
                message: format!(
                    "Torch LAN access must be enabled for non-loopback host '{}'",
                    self.host
                ),
            });
        }

        Ok(())
    }
}

/// Server status including all loaded slots and device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorchServerStatus {
    /// Whether the server is running and healthy.
    pub running: bool,
    /// All model slots.
    pub slots: Vec<ModelSlot>,
    /// Available compute devices.
    pub devices: Vec<DeviceInfo>,
    /// Current server configuration.
    #[serde(default)]
    pub config: Option<TorchServerConfig>,
}

// =============================================================================
// Response types (internal, for deserializing server responses)
// =============================================================================

#[derive(Debug, Deserialize)]
struct SlotsResponse {
    slots: Vec<ModelSlot>,
}

#[derive(Debug, Deserialize)]
struct DevicesResponse {
    devices: Vec<DeviceInfo>,
}

#[derive(Debug, Deserialize)]
struct LoadResponse {
    slot: ModelSlot,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
}

/// Sidecar handshake: liveness, protocol version and capability advertisement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorchHandshake {
    /// Liveness marker reported by the sidecar (`ok` when healthy).
    #[serde(default)]
    pub status: String,
    /// Sidecar protocol version; must equal [`SUPPORTED_TORCH_PROTOCOL`].
    #[serde(default)]
    pub protocol: u32,
    /// Capability names advertised by the sidecar.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl TorchHandshake {
    /// Returns true when the sidecar speaks the supported protocol and
    /// advertises image generation.
    pub fn is_image_compatible(&self) -> bool {
        self.status == "ok"
            && self.protocol == SUPPORTED_TORCH_PROTOCOL
            && self
                .capabilities
                .iter()
                .any(|capability| capability == TORCH_IMAGE_GENERATION_CAPABILITY)
    }
}

/// Typed image-generation result from the Torch sidecar (private boundary).
/// The gateway projects this into the public response shape.
#[derive(Debug, Clone, PartialEq)]
pub struct TorchImageResult {
    /// Base64-encoded PNG bytes.
    pub png_base64: String,
    /// Seed used for the generation.
    pub seed: u32,
    /// Denoising steps reported by the adapter.
    pub steps: u32,
    /// Guidance scale reported by the adapter.
    pub guidance: f64,
    /// Memory policy reported by the adapter.
    pub memory_policy: String,
    /// Generation duration in seconds reported by the sidecar.
    pub duration_seconds: f64,
}

/// Decoded image-generation failures from the Torch sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorchImageError {
    /// Sidecar is busy with another image operation.
    RuntimeBusy,
    /// No slot holds the requested image model.
    ModelUnavailable,
    /// The slot model does not support image generation.
    UnsupportedModel,
    /// The sidecar ran out of GPU memory.
    OutOfMemory,
    /// Generation exceeded the sidecar deadline (or the client timeout).
    DeadlineExceeded,
    /// Generation was cancelled (client disconnect or explicit stop).
    Cancelled,
    /// The sidecar reported an unclassified failure.
    BackendFailure,
    /// The sidecar response was not the typed image result.
    InvalidBackendResult,
    /// The sidecar could not be reached or its body could not be read.
    Transport,
}

impl TorchImageError {
    /// Stable provider error code for this failure.
    pub fn code(&self) -> &'static str {
        match self {
            Self::RuntimeBusy => "runtime_busy",
            Self::ModelUnavailable => "model_unavailable",
            Self::UnsupportedModel => "unsupported_model",
            Self::OutOfMemory => "out_of_memory",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::Cancelled => "cancelled",
            Self::BackendFailure => "backend_failure",
            Self::InvalidBackendResult => "invalid_backend_result",
            Self::Transport => "transport",
        }
    }
}

impl std::fmt::Display for TorchImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for TorchImageError {}

/// Private sidecar success body for image generation.
#[derive(Debug, Deserialize)]
struct ImageSuccessBody {
    #[serde(default)]
    png_base64: Option<String>,
    #[serde(default)]
    seed: Option<u32>,
    #[serde(default)]
    steps: Option<u32>,
    #[serde(default)]
    guidance: Option<f64>,
    #[serde(default)]
    memory_policy: Option<String>,
    #[serde(default)]
    duration_seconds: Option<f64>,
}

/// Decode a sidecar error body (`detail.code`) into the typed failure.
/// Only known runtime codes are admitted; anything else is a backend failure
/// so backend paths and tracebacks are never forwarded.
fn decode_image_error(body: &[u8]) -> TorchImageError {
    let code = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/detail/code")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    match code.as_deref() {
        Some("runtime_busy") => TorchImageError::RuntimeBusy,
        Some("model_unavailable") => TorchImageError::ModelUnavailable,
        Some("unsupported_model") => TorchImageError::UnsupportedModel,
        Some("out_of_memory") => TorchImageError::OutOfMemory,
        Some("deadline_exceeded") => TorchImageError::DeadlineExceeded,
        Some("cancelled") => TorchImageError::Cancelled,
        Some("invalid_backend_result") | Some("invalid_backend_response") => {
            TorchImageError::InvalidBackendResult
        }
        _ => TorchImageError::BackendFailure,
    }
}

// =============================================================================
// Client
// =============================================================================

/// HTTP client for a running Torch inference server.
pub struct TorchClient {
    base_url: String,
    client: reqwest::Client,
    /// Client with extended timeout for model loading operations.
    load_client: reqwest::Client,
    /// Client with the image-generation timeout (600s class).
    image_client: reqwest::Client,
}

impl TorchClient {
    /// Create a new client targeting the given base URL.
    ///
    /// If `base_url` is `None`, defaults to `http://127.0.0.1:8400`.
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

        let load_client = reqwest::Client::builder()
            .timeout(LOAD_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .expect("failed to build reqwest load client");

        let image_client = reqwest::Client::builder()
            .timeout(IMAGE_TIMEOUT)
            .user_agent("pumas-library")
            .build()
            .expect("failed to build reqwest image client");

        Self {
            base_url,
            client,
            load_client,
            image_client,
        }
    }

    /// Check if the server is responding.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        debug!("Torch health check: {}", url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let health: HealthResponse = response.json().await.map_err(|e| {
                        net_err(format!("Failed to parse Torch health response: {}", e))
                    })?;
                    Ok(health.status == "ok")
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// Fetch the sidecar handshake (`status`, `protocol`, `capabilities`).
    pub async fn handshake(&self) -> Result<TorchHandshake> {
        let url = format!("{}/health", self.base_url);
        debug!("Torch handshake: {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            net_err(format!(
                "Failed to connect to Torch server at {}: {}",
                url, e
            ))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Torch handshake API returned {}: {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Torch handshake response: {}", e)))
    }

    /// Require the supported protocol plus the image-generation capability.
    pub async fn verify_image_runtime(&self) -> Result<TorchHandshake> {
        let handshake = self.handshake().await?;
        if handshake.is_image_compatible() {
            Ok(handshake)
        } else {
            Err(PumasError::TorchInference {
                message: format!(
                    "Torch sidecar protocol {} is incompatible; this build requires protocol {} with the '{TORCH_IMAGE_GENERATION_CAPABILITY}' capability",
                    handshake.protocol, SUPPORTED_TORCH_PROTOCOL
                ),
            })
        }
    }

    /// Generate one image through the sidecar's private image boundary.
    ///
    /// Posts normalized values as private JSON (`model_id`, not the public
    /// `model` contract) and returns the typed result. Awaiting inline keeps
    /// disconnect propagation: dropping the future closes the sidecar request.
    pub async fn generate_image(
        &self,
        model_id: &str,
        prompt: &str,
        width: u32,
        height: u32,
        seed: Option<u32>,
    ) -> std::result::Result<TorchImageResult, TorchImageError> {
        let url = format!("{}/api/images/generate", self.base_url);
        debug!("Torch image generation for model '{}' at {}", model_id, url);

        let body = serde_json::json!({
            "model_id": model_id,
            "prompt": prompt,
            "width": width,
            "height": height,
            "seed": seed,
        });

        let response = self
            .image_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    TorchImageError::DeadlineExceeded
                } else {
                    TorchImageError::Transport
                }
            })?;

        let status = response.status();
        // Bound the body while streaming, mirroring the gateway JSON limit,
        // instead of buffering an unbounded sidecar response.
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| TorchImageError::Transport)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_IMAGE_RESPONSE_BYTES {
                return Err(TorchImageError::InvalidBackendResult);
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            return Err(decode_image_error(&bytes));
        }
        match serde_json::from_slice::<ImageSuccessBody>(&bytes).ok() {
            Some(ImageSuccessBody {
                png_base64: Some(png_base64),
                seed: Some(seed),
                steps: Some(steps),
                guidance: Some(guidance),
                memory_policy: Some(memory_policy),
                duration_seconds: Some(duration_seconds),
            }) if !png_base64.is_empty() => Ok(TorchImageResult {
                png_base64,
                seed,
                steps,
                guidance,
                memory_policy,
                duration_seconds,
            }),
            _ => Err(TorchImageError::InvalidBackendResult),
        }
    }

    /// List all model slots (loaded and unloaded).
    pub async fn list_slots(&self) -> Result<Vec<ModelSlot>> {
        let url = format!("{}/api/slots", self.base_url);
        debug!("Listing Torch model slots from {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            net_err(format!(
                "Failed to connect to Torch server at {}: {}",
                url, e
            ))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!("Torch API returned {}: {}", status, body)));
        }

        let slots_response: SlotsResponse = response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Torch slots response: {}", e)))?;

        info!(
            "Torch server has {} model slots",
            slots_response.slots.len()
        );
        Ok(slots_response.slots)
    }

    /// Load a model into a slot on a specific compute device.
    pub async fn load_model(
        &self,
        model_path: &str,
        model_name: &str,
        device: &ComputeDevice,
        model_type: Option<&str>,
    ) -> Result<ModelSlot> {
        self.load_with_pipeline(model_path, model_name, device, model_type, None)
            .await
    }

    /// Load an image checkpoint with library-resolved local pipeline components.
    pub async fn load_image_model(
        &self,
        model_path: &str,
        model_name: &str,
        device: &ComputeDevice,
        adapter: &str,
        components: &ImageModelComponents<'_>,
    ) -> Result<ModelSlot> {
        self.load_with_pipeline(
            model_path,
            model_name,
            device,
            Some(adapter),
            Some(components),
        )
        .await
    }

    async fn load_with_pipeline(
        &self,
        model_path: &str,
        model_name: &str,
        device: &ComputeDevice,
        model_type: Option<&str>,
        components: Option<&ImageModelComponents<'_>>,
    ) -> Result<ModelSlot> {
        let url = format!("{}/api/load", self.base_url);
        info!(
            "Loading model '{}' on {} from {}",
            model_name,
            device.to_server_string(),
            model_path
        );

        let mut body = serde_json::json!({
            "model_path": model_path,
            "model_name": model_name,
            "device": device.to_server_string(),
        });

        if let Some(mt) = model_type {
            body["model_type"] = serde_json::Value::String(mt.to_string());
        }
        if let Some(components) = components {
            body["pipeline_path"] = serde_json::Value::String(components.pipeline_path.to_string());
            if let Some(path) = components.vae_path {
                body["vae_path"] = serde_json::Value::String(path.to_string());
            }
        }

        let response = self
            .load_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                net_err(format!(
                    "Failed to send load request to Torch server: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(PumasError::TorchInference {
                message: format!("Load returned {}: {}", status, body),
            });
        }

        let load_response: LoadResponse = response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Torch load response: {}", e)))?;

        info!(
            "Model '{}' loaded into slot '{}' on {}",
            model_name, load_response.slot.slot_id, load_response.slot.device
        );
        Ok(load_response.slot)
    }

    /// Unload a model from a slot.
    pub async fn unload_model(&self, slot_id: &str) -> Result<()> {
        let url = format!("{}/api/unload", self.base_url);
        info!("Unloading Torch model slot '{}'", slot_id);

        let body = serde_json::json!({ "slot_id": slot_id });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                net_err(format!(
                    "Failed to send unload request to Torch server: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(PumasError::TorchInference {
                message: format!("Unload returned {}: {}", status, body),
            });
        }

        info!("Slot '{}' unloaded", slot_id);
        Ok(())
    }

    /// Get full server status including all slots and device info.
    pub async fn get_status(&self) -> Result<TorchServerStatus> {
        let url = format!("{}/api/status", self.base_url);
        debug!("Getting Torch server status from {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            net_err(format!(
                "Failed to connect to Torch server at {}: {}",
                url, e
            ))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Torch status API returned {}: {}",
                status, body
            )));
        }

        let status: TorchServerStatus = response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Torch status response: {}", e)))?;

        Ok(status)
    }

    /// List available compute devices.
    pub async fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        let url = format!("{}/api/devices", self.base_url);
        debug!("Listing Torch compute devices from {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            net_err(format!(
                "Failed to connect to Torch server at {}: {}",
                url, e
            ))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Torch devices API returned {}: {}",
                status, body
            )));
        }

        let devices_response: DevicesResponse = response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Torch devices response: {}", e)))?;

        Ok(devices_response.devices)
    }

    /// Update server configuration.
    ///
    /// Note: changes to `host` or `api_port` require a server restart to take effect.
    ///
    /// Non-loopback binding is intentionally gated twice: this client rejects it unless
    /// `lan_access` is enabled, and the Python sidecar still requires explicit LAN env opt-in
    /// plus an API token before serving non-loopback traffic.
    pub async fn configure(&self, config: &TorchServerConfig) -> Result<()> {
        config.validate()?;

        let url = format!("{}/api/configure", self.base_url);
        info!("Configuring Torch server: {:?}", config);

        let response = self
            .client
            .post(&url)
            .json(config)
            .send()
            .await
            .map_err(|e| {
                net_err(format!(
                    "Failed to send configure request to Torch server: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Torch configure API returned {}: {}",
                status, body
            )));
        }

        info!("Torch server configuration updated");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_device_roundtrip() {
        let devices = vec![
            (ComputeDevice::Cpu, "cpu"),
            (ComputeDevice::Cuda(0), "cuda:0"),
            (ComputeDevice::Cuda(1), "cuda:1"),
            (ComputeDevice::Mps, "mps"),
            (ComputeDevice::Auto, "auto"),
        ];

        for (device, expected_str) in devices {
            assert_eq!(device.to_server_string(), expected_str);
            assert_eq!(ComputeDevice::from_server_string(expected_str), device);
        }
    }

    #[test]
    fn test_compute_device_unknown_string() {
        let device = ComputeDevice::from_server_string("unknown");
        assert_eq!(device, ComputeDevice::Auto);
    }

    #[test]
    fn test_default_server_config() {
        let config = TorchServerConfig::default();
        assert_eq!(config.api_port, 8400);
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.max_loaded_models, 4);
        assert!(!config.lan_access);
    }

    #[test]
    fn test_torch_server_config_allows_loopback_without_lan_access() {
        let config = TorchServerConfig {
            host: "127.0.0.1".to_string(),
            lan_access: false,
            ..TorchServerConfig::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_torch_server_config_rejects_non_loopback_without_lan_access() {
        let config = TorchServerConfig {
            host: "0.0.0.0".to_string(),
            lan_access: false,
            ..TorchServerConfig::default()
        };

        let error = config.validate().unwrap_err();
        assert!(
            error.to_string().contains("LAN access must be enabled"),
            "{error}"
        );
    }

    #[test]
    fn test_torch_server_config_allows_non_loopback_with_lan_access() {
        let config = TorchServerConfig {
            host: "0.0.0.0".to_string(),
            lan_access: true,
            ..TorchServerConfig::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_slot_state_serialization() {
        let state = SlotState::Ready;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, r#""ready""#);

        let deserialized: SlotState = serde_json::from_str(r#""loading""#).unwrap();
        assert_eq!(deserialized, SlotState::Loading);
    }

    #[test]
    fn test_model_slot_deserialization() {
        let json = r#"{
            "slot_id": "slot-1",
            "model_name": "Llama 3 8B",
            "model_path": "/models/llama3-8b",
            "device": "cuda:0",
            "state": "ready",
            "gpu_memory_bytes": 8589934592,
            "model_type": "text-generation"
        }"#;

        let slot: ModelSlot = serde_json::from_str(json).unwrap();
        assert_eq!(slot.slot_id, "slot-1");
        assert_eq!(slot.model_name, "Llama 3 8B");
        assert_eq!(slot.device, "cuda:0");
        assert_eq!(slot.state, SlotState::Ready);
        assert_eq!(slot.gpu_memory_bytes, Some(8589934592));
        assert_eq!(slot.model_type.as_deref(), Some("text-generation"));
    }

    #[test]
    fn supported_protocol_is_two() {
        assert_eq!(SUPPORTED_TORCH_PROTOCOL, 2);
    }

    #[test]
    fn handshake_compat_requires_status_protocol_and_capability() {
        let compatible = TorchHandshake {
            status: "ok".to_string(),
            protocol: SUPPORTED_TORCH_PROTOCOL,
            capabilities: vec![TORCH_IMAGE_GENERATION_CAPABILITY.to_string()],
        };
        assert!(compatible.is_image_compatible());

        for handshake in [
            TorchHandshake {
                status: "starting".to_string(),
                protocol: SUPPORTED_TORCH_PROTOCOL,
                capabilities: vec![TORCH_IMAGE_GENERATION_CAPABILITY.to_string()],
            },
            TorchHandshake {
                status: "ok".to_string(),
                protocol: 1,
                capabilities: vec![TORCH_IMAGE_GENERATION_CAPABILITY.to_string()],
            },
            TorchHandshake {
                status: "ok".to_string(),
                protocol: SUPPORTED_TORCH_PROTOCOL,
                capabilities: vec![],
            },
        ] {
            assert!(!handshake.is_image_compatible(), "{handshake:?}");
        }
    }

    #[test]
    fn handshake_parses_legacy_health_without_capabilities_as_incompatible() {
        let handshake: TorchHandshake =
            serde_json::from_str(r#"{"status":"ok","protocol":1}"#).unwrap();
        assert!(!handshake.is_image_compatible());
    }

    #[test]
    fn image_error_decoding_admits_only_known_codes() {
        for (body, expected) in [
            (
                r#"{"detail":{"code":"runtime_busy"}}"#,
                TorchImageError::RuntimeBusy,
            ),
            (
                r#"{"detail":{"code":"model_unavailable"}}"#,
                TorchImageError::ModelUnavailable,
            ),
            (
                r#"{"detail":{"code":"unsupported_model"}}"#,
                TorchImageError::UnsupportedModel,
            ),
            (
                r#"{"detail":{"code":"out_of_memory"}}"#,
                TorchImageError::OutOfMemory,
            ),
            (
                r#"{"detail":{"code":"deadline_exceeded"}}"#,
                TorchImageError::DeadlineExceeded,
            ),
            (
                r#"{"detail":{"code":"cancelled"}}"#,
                TorchImageError::Cancelled,
            ),
            (
                r#"{"detail":{"code":"backend_failure"}}"#,
                TorchImageError::BackendFailure,
            ),
            (
                r#"{"detail":{"code":"invalid_backend_result"}}"#,
                TorchImageError::InvalidBackendResult,
            ),
            (
                r#"{"detail":{"code":"invalid_backend_response"}}"#,
                TorchImageError::InvalidBackendResult,
            ),
        ] {
            assert_eq!(decode_image_error(body.as_bytes()), expected, "{body}");
        }
        // Unknown codes, tracebacks and non-JSON bodies never forward backend detail.
        for body in [
            r#"{"detail":{"code":"totally_new","trace":"/srv/x.py"}}"#,
            r#"{"detail":"plain string"}"#,
            r#"not json"#,
            r#""#,
        ] {
            assert_eq!(
                decode_image_error(body.as_bytes()),
                TorchImageError::BackendFailure,
                "{body}"
            );
        }
    }

    #[test]
    fn image_success_body_requires_every_typed_field() {
        let valid = serde_json::json!({
            "png_base64": "aGVsbG8=",
            "seed": 7,
            "steps": 8,
            "guidance": 0.0,
            "memory_policy": "sequential_cpu_offload",
            "duration_seconds": 1.5
        });
        let parsed: ImageSuccessBody = serde_json::from_value(valid).unwrap();
        assert_eq!(parsed.png_base64.as_deref(), Some("aGVsbG8="));

        let missing = serde_json::json!({
            "png_base64": "aGVsbG8=",
            "seed": 7
        });
        let parsed: ImageSuccessBody = serde_json::from_value(missing).unwrap();
        assert!(parsed.steps.is_none());
    }

    #[test]
    fn test_device_info_deserialization() {
        let json = r#"{
            "device_id": "cuda:0",
            "name": "NVIDIA RTX 4090",
            "memory_total": 25769803776,
            "memory_available": 20000000000,
            "is_available": true
        }"#;

        let device: DeviceInfo = serde_json::from_str(json).unwrap();
        assert_eq!(device.device_id, "cuda:0");
        assert_eq!(device.name, "NVIDIA RTX 4090");
        assert!(device.is_available);
    }
}
