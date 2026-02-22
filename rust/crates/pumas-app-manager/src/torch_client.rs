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

use pumas_library::{PumasError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// Default Torch server API base URL.
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8400";

/// Timeout for short API calls (list, status, devices).
const API_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for model loading (can take minutes for large models).
const LOAD_TIMEOUT: Duration = Duration::from_secs(600);

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

// =============================================================================
// Client
// =============================================================================

/// HTTP client for a running Torch inference server.
pub struct TorchClient {
    base_url: String,
    client: reqwest::Client,
    /// Client with extended timeout for model loading operations.
    load_client: reqwest::Client,
}

impl TorchClient {
    /// Create a new client targeting the given base URL.
    ///
    /// If `base_url` is `None`, defaults to `http://127.0.0.1:8400`.
    pub fn new(base_url: Option<&str>) -> Self {
        let base_url = base_url
            .unwrap_or(DEFAULT_BASE_URL)
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

        Self {
            base_url,
            client,
            load_client,
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

    /// List all model slots (loaded and unloaded).
    pub async fn list_slots(&self) -> Result<Vec<ModelSlot>> {
        let url = format!("{}/api/slots", self.base_url);
        debug!("Listing Torch model slots from {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to connect to Torch server at {}: {}", url, e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(net_err(format!(
                "Torch API returned {}: {}",
                status, body
            )));
        }

        let slots_response: SlotsResponse = response
            .json()
            .await
            .map_err(|e| net_err(format!("Failed to parse Torch slots response: {}", e)))?;

        info!("Torch server has {} model slots", slots_response.slots.len());
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

        let response = self
            .load_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to send load request to Torch server: {}", e)))?;

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

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to connect to Torch server at {}: {}", url, e)))?;

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

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| net_err(format!("Failed to connect to Torch server at {}: {}", url, e)))?;

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
    /// Note: Changes to `host` or `api_port` require a server restart to take effect.
    pub async fn configure(&self, config: &TorchServerConfig) -> Result<()> {
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
