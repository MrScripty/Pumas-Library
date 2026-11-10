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
//!
//! ## API token
//!
//! Every Torch sidecar this backend launches shares one per-process API token
//! ([`pumas_library::runtime_profiles::torch_api_token`]) and refuses requests
//! without it. Only [`TorchClient::for_owned_sidecar`] sends that token, in the
//! `x-pumas-torch-token` header; the caller must have proved the endpoint is a
//! sidecar this backend owns. [`TorchClient::new`] targets any other Torch
//! server (external, remote, or caller-named) and never sends the token, so a
//! server the caller chose cannot learn the secret protecting owned sidecars.
//!
//! Clients never follow redirects, so a request cannot be forwarded to another
//! origin, and loopback servers are reached without any proxy.

use futures::StreamExt;
use pumas_library::config::AppId;
use pumas_library::runtime_profiles::{torch_api_token, TORCH_API_TOKEN_HEADER};
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

/// Connection-establishment budget for image generation requests.
///
/// Generation itself is duration-unbounded (see
/// `docs/contracts/generation-lifetime.md`): once connected, an admitted
/// request may remain silent until its terminal result. Only establishing the
/// transport connection is bounded here.
pub const GENERATION_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Single owner of the Torch sidecar protocol version spoken by this build.
/// The installer recipe check and the runtime handshake both use this value.
pub const SUPPORTED_TORCH_PROTOCOL: u32 = 3;

/// Capability advertised by image-capable Torch sidecars.
pub const TORCH_IMAGE_GENERATION_CAPABILITY: &str = "image_generation";

/// Capability advertised by Torch sidecars that serve audio transcription.
pub const TORCH_AUDIO_TRANSCRIPTION_CAPABILITY: &str = "audio_transcription";

/// Capability advertised by Torch sidecars that can load text embedders.
pub const TORCH_TEXT_EMBEDDING_CAPABILITY: &str = "text_embedding";

/// Capability advertised by Torch sidecars that can load text rerankers.
pub const TORCH_RERANK_CAPABILITY: &str = "rerank";

/// Sidecar model type of a text embedder slot.
pub const TORCH_TEXT_EMBEDDING_MODEL_TYPE: &str = "text-embedding";

/// Sidecar model type of a text reranker slot.
pub const TORCH_TEXT_RERANK_MODEL_TYPE: &str = "text-rerank";

/// Upper bound for a sidecar embeddings or rerank response body, the model
/// operations response bound.
const MAX_TEXT_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

/// Upper bound for a sidecar transcription response body.
const MAX_TRANSCRIPTION_RESPONSE_BYTES: usize = 256 * 1024;

/// Upper bound for a sidecar image-generation response body (12 MiB gateway JSON).
const MAX_IMAGE_RESPONSE_BYTES: usize = 12 * 1024 * 1024;

/// Helper to create a network error.
fn net_err(msg: String) -> PumasError {
    PumasError::Network {
        message: msg,
        cause: None,
    }
}

/// Whether a sidecar base URL names a loopback host (`localhost`, 127/8, `::1`).
fn is_loopback_base_url(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url) else {
        return false;
    };
    match url.host_str() {
        Some(host) if host.eq_ignore_ascii_case("localhost") => true,
        Some(host) => host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback()),
        None => false,
    }
}

/// Whether a client presents this backend's Torch API token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenScope {
    /// The destination is a sidecar this backend launched and owns.
    OwnedSidecar,
    /// Any other server; the token must never reach it.
    Unowned,
}

/// Shared transport policy for every Torch client.
fn sidecar_client_builder(base_url: &str, scope: TokenScope) -> reqwest::ClientBuilder {
    let mut builder = reqwest::Client::builder()
        .user_agent("pumas-library")
        // A redirect would forward the request (and any token) to another origin.
        .redirect(reqwest::redirect::Policy::none());
    if scope == TokenScope::OwnedSidecar {
        let mut token = reqwest::header::HeaderValue::from_str(torch_api_token())
            .expect("hex Torch API token is a valid header value");
        token.set_sensitive(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::HeaderName::from_static(TORCH_API_TOKEN_HEADER),
            token,
        );
        builder = builder.default_headers(headers);
    }
    if is_loopback_base_url(base_url) {
        // A system proxy must never see loopback sidecar traffic or its token.
        builder.no_proxy()
    } else {
        builder
    }
}

/// Reason text for a failed sidecar response.
///
/// Typed sidecar refusals carry `detail: {code, message}`; their message and
/// code are surfaced. Any other body is passed through unchanged.
fn sidecar_reason(body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("detail").cloned());
    match detail {
        Some(serde_json::Value::Object(detail)) => {
            match (
                detail.get("message").and_then(serde_json::Value::as_str),
                detail.get("code").and_then(serde_json::Value::as_str),
            ) {
                (Some(message), Some(code)) => format!("{message} ({code})"),
                (Some(message), None) => message.to_string(),
                _ => body.to_string(),
            }
        }
        Some(serde_json::Value::String(message)) => message,
        _ => body.to_string(),
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
///
/// `status` and `protocol` are required: a body that omits them or reports
/// them with the wrong JSON type is malformed, not merely incompatible.
/// `capabilities` is additive and defaults to empty, so a legacy body without
/// it parses as incompatible (missing capability) rather than malformed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorchHandshake {
    /// Liveness marker reported by the sidecar (`ok` when healthy).
    pub status: String,
    /// Sidecar protocol version; must equal [`SUPPORTED_TORCH_PROTOCOL`].
    pub protocol: u32,
    /// Capability names advertised by the sidecar.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// One owned incompatibility reason for a Torch sidecar handshake.
///
/// Each variant maps to a distinct boundary outcome so callers can tell
/// malformed handshakes, protocol mismatches, missing capabilities, and
/// unreachable sidecars apart instead of sharing one generic error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorchCompatibilityError {
    /// The sidecar process is not alive (`status` is not `"ok"`).
    NotLive { status: String },
    /// The sidecar speaks a different protocol than this build requires.
    ProtocolMismatch { actual: u32, expected: u32 },
    /// The sidecar protocol matches but it does not advertise image work.
    MissingCapability { capability: &'static str },
}

impl TorchCompatibilityError {
    /// Stable code identifying the incompatibility reason.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotLive { .. } => "torch_not_live",
            Self::ProtocolMismatch { .. } => "torch_protocol_mismatch",
            Self::MissingCapability { .. } => "torch_missing_capability",
        }
    }
}

impl std::fmt::Display for TorchCompatibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLive { status } => {
                write!(f, "Torch sidecar is not live (status {status:?})")
            }
            Self::ProtocolMismatch { actual, expected } => write!(
                f,
                "Torch sidecar protocol {actual} is incompatible; this build requires protocol {expected}"
            ),
            Self::MissingCapability { capability } => write!(
                f,
                "Torch sidecar does not advertise the '{capability}' capability"
            ),
        }
    }
}

impl std::error::Error for TorchCompatibilityError {}

/// Typed failure for fetching and checking a Torch sidecar handshake.
///
/// Unreachable, malformed, and incompatible sidecars fail with distinct
/// reasons so serving, listing, and admission boundaries can map each one to
/// its owning outcome instead of sharing a generic error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorchHandshakeFailure {
    /// The sidecar could not be reached or did not answer successfully.
    Unavailable(String),
    /// The sidecar answered, but not with the handshake shape.
    Malformed(String),
    /// The handshake parsed but is not live, current-protocol, or capable.
    Incompatible(TorchCompatibilityError),
}

impl TorchHandshakeFailure {
    /// Stable code identifying the failure reason.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable(_) => "torch_unavailable",
            Self::Malformed(_) => "torch_malformed_handshake",
            Self::Incompatible(error) => error.code(),
        }
    }
}

impl std::fmt::Display for TorchHandshakeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) | Self::Malformed(message) => f.write_str(message),
            Self::Incompatible(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TorchHandshakeFailure {}

impl TorchHandshake {
    /// Liveness check, independent of protocol and capability state.
    pub fn is_live(&self) -> bool {
        self.status == "ok"
    }

    /// Exact protocol agreement with this build, independent of liveness
    /// and capability state.
    pub fn protocol_matches(&self) -> bool {
        self.protocol == SUPPORTED_TORCH_PROTOCOL
    }

    /// Image-capability advertisement, independent of liveness, protocol,
    /// and ready-slot state. A ready slot is checked separately through
    /// [`TorchClient::list_slots`]; this advertisement alone never admits
    /// a model for image work.
    pub fn has_image_capability(&self) -> bool {
        self.has_capability(TORCH_IMAGE_GENERATION_CAPABILITY)
    }

    /// Whether the sidecar advertises `capability`.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|name| name == capability)
    }

    /// Require liveness, the supported protocol, and the image-generation
    /// capability, reporting the first failure with its owning reason.
    pub fn check_image_compatible(&self) -> std::result::Result<(), TorchCompatibilityError> {
        self.check_compatible(TORCH_IMAGE_GENERATION_CAPABILITY)
    }

    /// Require liveness, the supported protocol, and `capability`, reporting
    /// the first failure with its owning reason.
    pub fn check_compatible(
        &self,
        capability: &'static str,
    ) -> std::result::Result<(), TorchCompatibilityError> {
        if !self.is_live() {
            return Err(TorchCompatibilityError::NotLive {
                status: self.status.clone(),
            });
        }
        if !self.protocol_matches() {
            return Err(TorchCompatibilityError::ProtocolMismatch {
                actual: self.protocol,
                expected: SUPPORTED_TORCH_PROTOCOL,
            });
        }
        if !self.has_capability(capability) {
            return Err(TorchCompatibilityError::MissingCapability { capability });
        }
        Ok(())
    }

    /// Returns true when the sidecar speaks the supported protocol and
    /// advertises image generation.
    pub fn is_image_compatible(&self) -> bool {
        self.check_image_compatible().is_ok()
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
    /// The requested dimensions exceed the adapter's supported maximum,
    /// refused at admission before any generation. Carries that maximum in
    /// output pixels when the provider reports it.
    DimensionsExceedLimit { max_pixels: Option<u64> },
    /// The sidecar ran out of GPU memory.
    OutOfMemory,
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
            Self::DimensionsExceedLimit { .. } => "dimensions_exceed_limit",
            Self::OutOfMemory => "out_of_memory",
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

/// Transcript returned by the sidecar's private audio operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorchTranscription {
    pub text: String,
    /// Generation reached the adapter's token bound instead of ending.
    pub length_limited: bool,
}

/// Decoded failures of a Torch sidecar operation (transcription, embeddings,
/// rerank).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorchOperationError {
    /// The sidecar is busy with another operation on the model's device.
    RuntimeBusy,
    /// No ready slot holds the requested model.
    ModelUnavailable,
    /// The slot's model does not perform the requested operation.
    UnsupportedModel,
    /// The sidecar refused the request, with its reason.
    InvalidRequest(String),
    /// The sidecar ran out of GPU memory.
    OutOfMemory,
    /// The operation stopped (client disconnect or cancellation).
    Cancelled,
    /// The sidecar reported an unclassified failure.
    BackendFailure,
    /// The sidecar response was not the operation's typed result.
    InvalidBackendResult,
    /// The sidecar could not be reached or its body could not be read.
    Transport,
}

impl TorchOperationError {
    /// Stable provider error code for this failure.
    pub fn code(&self) -> &'static str {
        match self {
            Self::RuntimeBusy => "runtime_busy",
            Self::ModelUnavailable => "model_unavailable",
            Self::UnsupportedModel => "unsupported_model",
            Self::InvalidRequest(_) => "invalid_request",
            Self::OutOfMemory => "out_of_memory",
            Self::Cancelled => "cancelled",
            Self::BackendFailure => "backend_failure",
            Self::InvalidBackendResult => "invalid_backend_result",
            Self::Transport => "transport",
        }
    }
}

impl std::fmt::Display for TorchOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest(reason) => write!(f, "invalid_request: {reason}"),
            other => f.write_str(other.code()),
        }
    }
}

impl std::error::Error for TorchOperationError {}

fn decode_operation_error(body: &[u8]) -> TorchOperationError {
    let detail = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("detail").cloned());
    let field = |name: &str| {
        detail
            .as_ref()
            .and_then(|detail| detail.get(name))
            .and_then(serde_json::Value::as_str)
    };
    match field("code") {
        Some("runtime_busy") => TorchOperationError::RuntimeBusy,
        Some("model_unavailable") => TorchOperationError::ModelUnavailable,
        Some("unsupported_model") => TorchOperationError::UnsupportedModel,
        // The sidecar's refusal reasons name request fields, never paths.
        Some("invalid_request") => TorchOperationError::InvalidRequest(
            field("message")
                .unwrap_or("audio request refused")
                .chars()
                .take(512)
                .collect(),
        ),
        Some("out_of_memory") => TorchOperationError::OutOfMemory,
        Some("cancelled") => TorchOperationError::Cancelled,
        _ => TorchOperationError::BackendFailure,
    }
}

#[derive(Debug, Deserialize)]
struct TranscriptionBody {
    text: String,
    finish_reason: String,
}

fn decode_transcription(
    body: &[u8],
) -> std::result::Result<TorchTranscription, TorchOperationError> {
    let parsed = serde_json::from_slice::<TranscriptionBody>(body)
        .map_err(|_| TorchOperationError::InvalidBackendResult)?;
    let length_limited = match parsed.finish_reason.as_str() {
        "stop" => false,
        "length" => true,
        _ => return Err(TorchOperationError::InvalidBackendResult),
    };
    Ok(TorchTranscription {
        text: parsed.text,
        length_limited,
    })
}

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
///
/// There is no generation-deadline outcome: admitted generation has no total,
/// read, idle, or elapsed deadline. A legacy `deadline_exceeded` code is
/// therefore decoded as an unclassified backend failure, never as proof that
/// work stopped on time.
fn decode_image_error(body: &[u8]) -> TorchImageError {
    let detail = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("detail").cloned());
    let code = detail
        .as_ref()
        .and_then(|detail| detail.get("code"))
        .and_then(serde_json::Value::as_str);
    match code {
        Some("runtime_busy") => TorchImageError::RuntimeBusy,
        Some("model_unavailable") => TorchImageError::ModelUnavailable,
        Some("unsupported_model") => TorchImageError::UnsupportedModel,
        // Only the typed limit crosses; the provider's message text does not.
        Some("dimensions_exceed_limit") => TorchImageError::DimensionsExceedLimit {
            max_pixels: detail
                .as_ref()
                .and_then(|detail| detail.get("max_pixels"))
                .and_then(serde_json::Value::as_u64)
                .filter(|max_pixels| *max_pixels > 0),
        },
        Some("out_of_memory") => TorchImageError::OutOfMemory,
        Some("cancelled") => TorchImageError::Cancelled,
        Some("invalid_backend_result") | Some("invalid_backend_response") => {
            TorchImageError::InvalidBackendResult
        }
        _ => TorchImageError::BackendFailure,
    }
}

/// Decoded PNG payload limit: the sidecar must not return more than 8 MiB of
/// PNG bytes for one image.
const MAX_PNG_BYTES: usize = 8 * 1024 * 1024;

/// Decode one base64 character into its 6-bit value. Rejects whitespace and
/// non-alphabet characters so sidecar payloads are checked strictly.
fn base64_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some((byte - b'A') as u32),
        b'a'..=b'z' => Some((byte - b'a' + 26) as u32),
        b'0'..=b'9' => Some((byte - b'0' + 52) as u32),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Strictly decode standard base64 without whitespace. Returns `None` for any
/// malformed input, including incorrect padding or trailing bits.
fn decode_base64_strict(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    let total = bytes.len() / 4;
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let pad = chunk.iter().rev().take_while(|&&b| b == b'=').count();
        if pad > 2 {
            return None;
        }
        // Padding is only valid on the final quantum.
        if pad > 0 && index + 1 != total {
            return None;
        }
        let mut sextets = [0u32; 4];
        for (position, &byte) in chunk.iter().enumerate() {
            if byte == b'=' {
                // Padding may only fill the trailing pad slots.
                if position < 4 - pad {
                    return None;
                }
                sextets[position] = 0;
            } else {
                sextets[position] = base64_value(byte)?;
            }
        }
        let triple = (sextets[0] << 18) | (sextets[1] << 12) | (sextets[2] << 6) | sextets[3];
        output.push((triple >> 16) as u8);
        if pad < 2 {
            output.push((triple >> 8) as u8);
        }
        if pad == 0 {
            output.push(triple as u8);
        }
        // Non-zero trailing bits indicate a non-canonical encoding.
        if pad == 1 && (triple & 0xFF) != 0 {
            return None;
        }
        if pad == 2 && (triple & 0xFFFF) != 0 {
            return None;
        }
    }
    Some(output)
}

/// Read the `IHDR` dimensions of a PNG without image decoding. Returns
/// `(width, height)` or `None` when the bytes are not a PNG with a leading
/// `IHDR` chunk.
fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    // Signature (8) plus length (4), type (4), and IHDR data (13) = 29 bytes.
    if png.len() < 29 || &png[..8] != SIGNATURE {
        return None;
    }
    let length = u32::from_be_bytes([png[8], png[9], png[10], png[11]]);
    if length != 13 || &png[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

/// Validate a decoded sidecar success body against the private result
/// contract: every canonical required value must be present and well-formed,
/// the PNG must decode to the requested dimensions within the PNG bound, and
/// the whole-payload bound applies to the entire received body.
///
/// Unknown additive fields (for example `peak_vram_bytes`) are ignored here
/// and never projected publicly; invalid required data fails closed.
fn decode_image_result(
    body: &[u8],
    expected_width: u32,
    expected_height: u32,
) -> std::result::Result<TorchImageResult, TorchImageError> {
    let parsed = serde_json::from_slice::<ImageSuccessBody>(body).ok();
    let Some(ImageSuccessBody {
        png_base64: Some(png_base64),
        seed: Some(seed),
        steps: Some(steps),
        guidance: Some(guidance),
        memory_policy: Some(memory_policy),
        duration_seconds: Some(duration_seconds),
    }) = parsed
    else {
        return Err(TorchImageError::InvalidBackendResult);
    };
    if png_base64.is_empty()
        || memory_policy.trim().is_empty()
        || !guidance.is_finite()
        || !duration_seconds.is_finite()
        || duration_seconds < 0.0
    {
        return Err(TorchImageError::InvalidBackendResult);
    }
    let png = decode_base64_strict(&png_base64).ok_or(TorchImageError::InvalidBackendResult)?;
    if png.len() > MAX_PNG_BYTES {
        return Err(TorchImageError::InvalidBackendResult);
    }
    match png_dimensions(&png) {
        Some((width, height)) if width == expected_width && height == expected_height => {}
        _ => return Err(TorchImageError::InvalidBackendResult),
    }
    Ok(TorchImageResult {
        png_base64,
        seed,
        steps,
        guidance,
        memory_policy,
        duration_seconds,
    })
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
    /// Connection-bounded but duration-unbounded client for image generation
    /// and transcription: only connection establishment is bounded; an
    /// admitted request may stay connected and silent until its terminal result.
    image_client: reqwest::Client,
}

impl TorchClient {
    /// Create a client for a Torch server this backend does not own.
    ///
    /// The client never sends this backend's Torch API token, so it is safe
    /// for any caller-chosen, external or remote destination. A sidecar this
    /// backend launched refuses its requests; use [`Self::for_owned_sidecar`]
    /// once ownership of the endpoint is proven.
    ///
    /// If `base_url` is `None`, defaults to `http://127.0.0.1:8400`.
    pub fn new(base_url: Option<&str>) -> Self {
        Self::build(base_url.unwrap_or(default_base_url()), TokenScope::Unowned)
    }

    /// Create a client for a Torch sidecar this backend launched and owns.
    ///
    /// Every request except `/health` carries this backend's Torch API token.
    /// Pass only an endpoint taken from a current owned-process observation;
    /// any other destination would learn the secret that protects every
    /// sidecar this backend launches.
    pub fn for_owned_sidecar(endpoint_url: &str) -> Self {
        Self::build(endpoint_url, TokenScope::OwnedSidecar)
    }

    fn build(base_url: &str, scope: TokenScope) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();

        let client = sidecar_client_builder(&base_url, scope)
            .timeout(API_TIMEOUT)
            .build()
            .expect("failed to build reqwest client");

        let load_client = sidecar_client_builder(&base_url, scope)
            .timeout(LOAD_TIMEOUT)
            .build()
            .expect("failed to build reqwest load client");

        let image_client = sidecar_client_builder(&base_url, scope)
            .connect_timeout(GENERATION_CONNECT_TIMEOUT)
            .retry(reqwest::retry::never())
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
    ///
    /// A missing or mistyped body is malformed and fails here, before any
    /// protocol or capability classification.
    pub async fn handshake(&self) -> Result<TorchHandshake> {
        self.fetch_handshake()
            .await
            .map_err(|failure| match failure {
                TorchHandshakeFailure::Unavailable(message)
                | TorchHandshakeFailure::Malformed(message) => net_err(message),
                TorchHandshakeFailure::Incompatible(error) => PumasError::TorchInference {
                    message: error.to_string(),
                },
            })
    }

    /// Fetch the handshake with its owning failure reason: unreachable,
    /// malformed, or incompatible. Ready-slot state stays separate.
    async fn fetch_handshake(&self) -> std::result::Result<TorchHandshake, TorchHandshakeFailure> {
        let url = format!("{}/health", self.base_url);
        debug!("Torch handshake: {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            TorchHandshakeFailure::Unavailable(format!(
                "Failed to connect to Torch server at {url}: {e}"
            ))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(TorchHandshakeFailure::Unavailable(format!(
                "Torch handshake API returned {status}: {body}"
            )));
        }

        response.json().await.map_err(|e| {
            TorchHandshakeFailure::Malformed(format!(
                "Failed to parse Torch handshake response: {e}"
            ))
        })
    }

    /// Fetch the handshake and require the supported protocol plus the
    /// image-generation capability, keeping each failure reason distinct.
    pub async fn check_image_runtime(
        &self,
    ) -> std::result::Result<TorchHandshake, TorchHandshakeFailure> {
        self.check_runtime(TORCH_IMAGE_GENERATION_CAPABILITY).await
    }

    /// Fetch the handshake and require the supported protocol plus
    /// `capability`, keeping each failure reason distinct.
    pub async fn check_runtime(
        &self,
        capability: &'static str,
    ) -> std::result::Result<TorchHandshake, TorchHandshakeFailure> {
        let handshake = self.fetch_handshake().await?;
        handshake
            .check_compatible(capability)
            .map(|()| handshake)
            .map_err(TorchHandshakeFailure::Incompatible)
    }

    /// Require the supported protocol plus the image-generation capability.
    ///
    /// Each incompatibility keeps its owning reason: a malformed handshake
    /// body fails inside [`TorchClient::handshake`], while liveness, protocol
    /// mismatch, and missing capability fail here with distinct messages.
    /// An unreachable sidecar fails as a network error. Ready-slot state is
    /// checked separately through [`TorchClient::list_slots`]: compatibility
    /// alone never admits a model, and a ready slot never proves
    /// compatibility.
    pub async fn verify_image_runtime(&self) -> Result<TorchHandshake> {
        self.verify_runtime(TORCH_IMAGE_GENERATION_CAPABILITY).await
    }

    /// Require the supported protocol plus `capability`; see
    /// [`Self::verify_image_runtime`].
    pub async fn verify_runtime(&self, capability: &'static str) -> Result<TorchHandshake> {
        let handshake = self.handshake().await?;
        handshake
            .check_compatible(capability)
            .map(|()| handshake)
            .map_err(|error| PumasError::TorchInference {
                message: error.to_string(),
            })
    }

    /// Generate one image through the sidecar's private image boundary.
    ///
    /// Posts normalized values as private JSON (`model_id`, not the public
    /// `model` contract) and returns the typed result. The transport is
    /// connection-bounded but duration-unbounded: an admitted request may
    /// remain connected and silent until its terminal result, and elapsed
    /// time alone never fails it. Awaiting inline keeps disconnect
    /// propagation: dropping the future closes the sidecar request, which
    /// requests cancellation without proving that sidecar work stopped. A
    /// lost transport before a terminal result is an unknown outcome and is
    /// never replayed here.
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
            .map_err(|_| TorchImageError::Transport)?;

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
        decode_image_result(&bytes, width, height)
    }

    /// Transcribe one recording through the sidecar's private audio operation.
    ///
    /// `audio` is the typed PCM part (`encoding`, `sample_rate_hz`,
    /// `channels`, `sample_count`, `data_base64`). Like image generation the
    /// transport is connection-bounded but duration-unbounded, and dropping
    /// the future asks the sidecar to stop.
    pub async fn transcribe_audio(
        &self,
        model_id: &str,
        audio: &serde_json::Value,
        language: &str,
    ) -> std::result::Result<TorchTranscription, TorchOperationError> {
        let url = format!("{}/api/audio/transcriptions", self.base_url);
        debug!("Torch transcription for model '{}' at {}", model_id, url);
        let body = serde_json::json!({
            "model_id": model_id,
            "audio": audio,
            "language": language,
        });
        let response = self
            .image_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|_| TorchOperationError::Transport)?;
        let status = response.status();
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| TorchOperationError::Transport)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_TRANSCRIPTION_RESPONSE_BYTES {
                return Err(TorchOperationError::InvalidBackendResult);
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            return Err(decode_operation_error(&bytes));
        }
        decode_transcription(&bytes)
    }

    /// Embed texts with a loaded text embedder. The result is the sidecar's
    /// OpenAI-shaped body (`data[{index, embedding}]`, `usage`), for the
    /// caller's typed projection to check.
    pub async fn embed_text(
        &self,
        model_id: &str,
        input: &[String],
        dimensions: Option<u32>,
    ) -> std::result::Result<serde_json::Value, TorchOperationError> {
        let mut body = serde_json::json!({"model_id": model_id, "input": input});
        if let Some(dimensions) = dimensions {
            body["dimensions"] = dimensions.into();
        }
        self.text_operation("embeddings", &body).await
    }

    /// Score documents against a query with a loaded text reranker. The
    /// result is the sidecar's llama.cpp-shaped body (`results[{index,
    /// relevance_score}]` in document order, `usage`).
    pub async fn rerank_text(
        &self,
        model_id: &str,
        query: &str,
        documents: &[String],
    ) -> std::result::Result<serde_json::Value, TorchOperationError> {
        let body =
            serde_json::json!({"model_id": model_id, "query": query, "documents": documents});
        self.text_operation("rerank", &body).await
    }

    async fn text_operation(
        &self,
        operation: &str,
        body: &serde_json::Value,
    ) -> std::result::Result<serde_json::Value, TorchOperationError> {
        let url = format!("{}/api/text/{operation}", self.base_url);
        let response = self
            .image_client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|_| TorchOperationError::Transport)?;
        let status = response.status();
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| TorchOperationError::Transport)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_TEXT_RESPONSE_BYTES {
                return Err(TorchOperationError::InvalidBackendResult);
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            return Err(decode_operation_error(&bytes));
        }
        serde_json::from_slice(&bytes).map_err(|_| TorchOperationError::InvalidBackendResult)
    }

    /// Load a text embedder or reranker with the model's recorded
    /// sentence-transformers declarations, which decide what it computes.
    pub async fn load_text_encoder(
        &self,
        model_path: &str,
        model_name: &str,
        device: &ComputeDevice,
        model_type: &str,
        declarations: Option<&serde_json::Value>,
    ) -> Result<ModelSlot> {
        self.load_request(
            model_path,
            model_name,
            device,
            Some(model_type),
            None,
            declarations,
        )
        .await
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
            return Err(net_err(format!(
                "Torch API returned {}: {}",
                status,
                sidecar_reason(&body)
            )));
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
        self.load_request(model_path, model_name, device, model_type, components, None)
            .await
    }

    async fn load_request(
        &self,
        model_path: &str,
        model_name: &str,
        device: &ComputeDevice,
        model_type: Option<&str>,
        components: Option<&ImageModelComponents<'_>>,
        declarations: Option<&serde_json::Value>,
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
        if let Some(declarations) = declarations {
            body["declarations"] = declarations.clone();
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
                message: format!("Load returned {}: {}", status, sidecar_reason(&body)),
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
                message: format!("Unload returned {}: {}", status, sidecar_reason(&body)),
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
                status,
                sidecar_reason(&body)
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
                status,
                sidecar_reason(&body)
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
                status,
                sidecar_reason(&body)
            )));
        }

        info!("Torch server configuration updated");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn image_generation_does_not_follow_provider_redirects() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let redirect = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let target = format!(
            "http://{}/api/images/generate",
            redirect.local_addr().unwrap()
        );
        let backend = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut bytes = [0; 1024];
                let count = socket.read(&mut bytes).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&bytes[..count]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            assert!(request.starts_with(b"POST /api/images/generate "));
            socket.write_all(format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n").as_bytes()).await.unwrap();
        });
        let result = TorchClient::for_owned_sidecar(&endpoint)
            .generate_image("fixture", "synthetic", 1, 1, None)
            .await;
        assert!(result.is_err());
        backend.await.unwrap();
        assert!(futures::poll!(Box::pin(redirect.accept())).is_pending());
    }

    /// Accept one request, return its raw head, and answer with `response`.
    async fn serve_once(
        listener: tokio::net::TcpListener,
        response: &'static str,
    ) -> tokio::task::JoinHandle<String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let mut bytes = [0; 1024];
                let count = socket.read(&mut bytes).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&bytes[..count]);
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&request).to_ascii_lowercase()
        })
    }

    const EMPTY_SLOTS: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"slots\":[]}";

    #[tokio::test]
    async fn owned_sidecar_requests_carry_the_launch_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let backend = serve_once(listener, EMPTY_SLOTS).await;
        let slots = TorchClient::for_owned_sidecar(&endpoint)
            .list_slots()
            .await
            .unwrap();
        assert!(slots.is_empty());
        let request = backend.await.unwrap();
        assert!(request.starts_with("get /api/slots "));
        assert!(request.contains(&format!("{TORCH_API_TOKEN_HEADER}: {}", torch_api_token())));
    }

    #[tokio::test]
    async fn unowned_server_requests_never_carry_the_launch_token() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let backend = serve_once(listener, EMPTY_SLOTS).await;
        let slots = TorchClient::new(Some(&endpoint))
            .list_slots()
            .await
            .unwrap();
        assert!(slots.is_empty());
        let request = backend.await.unwrap();
        assert!(request.starts_with("get /api/slots "));
        assert!(!request.contains(TORCH_API_TOKEN_HEADER));
        assert!(!request.contains(torch_api_token()));
    }

    #[tokio::test]
    async fn control_requests_do_not_follow_redirects_and_report_typed_reasons() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let backend = serve_once(
            listener,
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://192.0.2.1/api/slots\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let result = TorchClient::for_owned_sidecar(&endpoint).list_slots().await;
        backend.await.unwrap();
        assert!(result.unwrap_err().to_string().contains("307"));

        let body = r#"{"detail":{"code":"unauthorized","message":"Missing or invalid x-pumas-torch-token header"}}"#;
        assert_eq!(
            sidecar_reason(body),
            "Missing or invalid x-pumas-torch-token header (unauthorized)"
        );
        assert_eq!(sidecar_reason(r#"{"detail":"legacy"}"#), "legacy");
        assert_eq!(sidecar_reason("plain"), "plain");
    }

    #[test]
    fn only_loopback_sidecars_bypass_proxies() {
        for url in [
            "http://127.0.0.1:8400",
            "http://localhost:8400",
            "http://LOCALHOST",
            "http://[::1]:8400",
            "http://127.1.2.3",
        ] {
            assert!(is_loopback_base_url(url), "{url}");
        }
        for url in ["http://192.168.1.4:8400", "http://example.com", "not a url"] {
            assert!(!is_loopback_base_url(url), "{url}");
        }
    }

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
    fn supported_protocol_is_three() {
        assert_eq!(SUPPORTED_TORCH_PROTOCOL, 3);
    }

    #[test]
    fn generation_transport_is_connect_bounded_without_total_timeout() {
        // Only connection establishment is bounded; there is no total, read,
        // idle, or elapsed generation deadline anywhere on this path.
        assert_eq!(GENERATION_CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(MAX_IMAGE_RESPONSE_BYTES, 12 * 1024 * 1024);
        assert_eq!(MAX_PNG_BYTES, 8 * 1024 * 1024);
    }

    #[test]
    fn handshake_compat_requires_status_protocol_and_capability() {
        let compatible = TorchHandshake {
            status: "ok".to_string(),
            protocol: SUPPORTED_TORCH_PROTOCOL,
            capabilities: vec![TORCH_IMAGE_GENERATION_CAPABILITY.to_string()],
        };
        assert!(compatible.is_image_compatible());
        assert!(compatible.check_image_compatible().is_ok());

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
    fn handshake_compat_distinguishes_each_incompatibility() {
        // A protocol-2 sidecar speaks the old lifetime: exact protocol 3 is
        // required, so mixed versions cannot be admitted.
        let protocol_two = TorchHandshake {
            status: "ok".to_string(),
            protocol: 2,
            capabilities: vec![TORCH_IMAGE_GENERATION_CAPABILITY.to_string()],
        };
        assert_eq!(
            protocol_two.check_image_compatible(),
            Err(TorchCompatibilityError::ProtocolMismatch {
                actual: 2,
                expected: SUPPORTED_TORCH_PROTOCOL,
            })
        );

        let not_live = TorchHandshake {
            status: "starting".to_string(),
            protocol: SUPPORTED_TORCH_PROTOCOL,
            capabilities: vec![TORCH_IMAGE_GENERATION_CAPABILITY.to_string()],
        };
        assert_eq!(
            not_live.check_image_compatible(),
            Err(TorchCompatibilityError::NotLive {
                status: "starting".to_string(),
            })
        );

        let missing_capability = TorchHandshake {
            status: "ok".to_string(),
            protocol: SUPPORTED_TORCH_PROTOCOL,
            capabilities: vec!["other".to_string()],
        };
        assert_eq!(
            missing_capability.check_image_compatible(),
            Err(TorchCompatibilityError::MissingCapability {
                capability: TORCH_IMAGE_GENERATION_CAPABILITY,
            })
        );

        // Liveness, protocol, and capability checks stay independent of the
        // ready-slot check: no handshake result admits a model by itself.
        assert!(!protocol_two.protocol_matches());
        assert!(missing_capability.protocol_matches());
        assert!(missing_capability.is_live());
        assert!(!missing_capability.has_image_capability());
    }

    #[test]
    fn handshake_parses_legacy_health_without_capabilities_as_incompatible() {
        let handshake: TorchHandshake =
            serde_json::from_str(r#"{"status":"ok","protocol":3}"#).unwrap();
        assert!(!handshake.is_image_compatible());
        assert_eq!(
            handshake.check_image_compatible(),
            Err(TorchCompatibilityError::MissingCapability {
                capability: TORCH_IMAGE_GENERATION_CAPABILITY,
            })
        );
    }

    #[test]
    fn handshake_rejects_malformed_bodies_before_compat_classification() {
        // Missing `status` or `protocol`, or a mistyped protocol, is
        // malformed: it fails parsing rather than classifying as a protocol
        // mismatch or a missing capability.
        for body in [
            r#"{"protocol":3,"capabilities":["image_generation"]}"#,
            r#"{"status":"ok","capabilities":["image_generation"]}"#,
            r#"{"status":"ok","protocol":"3","capabilities":["image_generation"]}"#,
            r#"{"status":"ok","protocol":null}"#,
            r#"not json"#,
        ] {
            assert!(
                serde_json::from_str::<TorchHandshake>(body).is_err(),
                "{body}"
            );
        }
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
            (
                r#"{"detail":{"code":"dimensions_exceed_limit","message":"/srv/x","max_pixels":4194304}}"#,
                TorchImageError::DimensionsExceedLimit {
                    max_pixels: Some(4_194_304),
                },
            ),
            (
                r#"{"detail":{"code":"dimensions_exceed_limit","max_pixels":"big"}}"#,
                TorchImageError::DimensionsExceedLimit { max_pixels: None },
            ),
        ] {
            assert_eq!(decode_image_error(body.as_bytes()), expected, "{body}");
        }
        // Unknown codes, tracebacks and non-JSON bodies never forward backend detail.
        // A legacy deadline code is unclassified: there is no deadline outcome.
        for body in [
            r#"{"detail":{"code":"totally_new","trace":"/srv/x.py"}}"#,
            r#"{"detail":{"code":"deadline_exceeded"}}"#,
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

    /// Minimal PNG with the given `IHDR` dimensions (signature plus `IHDR`
    /// header only; sufficient for the contract dimension check).
    fn fixture_png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&width.to_be_bytes());
        png.extend_from_slice(&height.to_be_bytes());
        png.extend_from_slice(&[8, 2, 0, 0, 0]);
        png
    }

    fn fixture_base64(data: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut output = String::new();
        for chunk in data.chunks(3) {
            let mut triple = 0u32;
            for &byte in chunk {
                triple = (triple << 8) | byte as u32;
            }
            triple <<= 8 * (3 - chunk.len());
            let quads = [18, 12, 6, 0].map(|shift| ALPHABET[((triple >> shift) & 63) as usize]);
            let pad = 3 - chunk.len();
            for (index, &quad) in quads.iter().enumerate() {
                output.push(if index >= 4 - pad { '=' } else { quad as char });
            }
        }
        output
    }

    fn fixture_result_body(png_base64: &str) -> Vec<u8> {
        serde_json::json!({
            "png_base64": png_base64,
            "seed": 7,
            "steps": 8,
            "guidance": 0.0,
            "memory_policy": "sequential_cpu_offload",
            "duration_seconds": 1.5
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn strict_base64_vectors_round_trip() {
        for (raw, encoded) in [
            (vec![0x68, 0x65, 0x6c, 0x6c, 0x6f], "aGVsbG8="),
            (vec![0x00], "AA=="),
            (vec![0xff, 0xff, 0xfe], "///+"),
            (vec![0x01, 0x02, 0x03], "AQID"),
            (vec![0x01, 0x02], "AQI="),
        ] {
            assert_eq!(fixture_base64(&raw), encoded, "{raw:?}");
            assert_eq!(decode_base64_strict(encoded), Some(raw), "{encoded}");
        }
        for bad in [
            "aGVsbG8",
            "aGVsbG8==",
            "a GVsbG8=",
            "aGVsbG8!",
            "====",
            "",
            "AB=C",
            "=ABC",
        ] {
            assert_eq!(decode_base64_strict(bad), None, "{bad}");
        }
    }

    #[test]
    fn image_result_accepts_canonical_fields_and_additive_private_data() {
        let png = fixture_base64(&fixture_png_bytes(512, 512));
        // Additive private fields (for example `peak_vram_bytes`) must not
        // fail decoding and must not appear in the typed result.
        let body = serde_json::json!({
            "png_base64": png,
            "seed": 7,
            "steps": 8,
            "guidance": 0.0,
            "memory_policy": "sequential_cpu_offload",
            "duration_seconds": 1.5,
            "peak_vram_bytes": 123456,
            "future_private_field": {"nested": true}
        })
        .to_string()
        .into_bytes();
        let result = decode_image_result(&body, 512, 512).unwrap();
        assert_eq!(result.seed, 7);
        assert_eq!(result.steps, 8);
        assert_eq!(result.png_base64, png);
        let projected = serde_json::to_value(&result.png_base64).unwrap();
        assert_eq!(projected, serde_json::Value::String(png));
    }

    #[test]
    fn image_result_rejects_invalid_required_data_and_dimension_mismatch() {
        let good_png = fixture_base64(&fixture_png_bytes(1280, 720));
        let good = fixture_result_body(&good_png);
        assert!(decode_image_result(&good, 1280, 720).is_ok());

        // Dimension mismatch, non-PNG bytes, and malformed base64 fail closed.
        assert_eq!(
            decode_image_result(&good, 512, 512),
            Err(TorchImageError::InvalidBackendResult)
        );
        let not_png = fixture_result_body(&fixture_base64(b"hello"));
        assert_eq!(
            decode_image_result(&not_png, 1280, 720),
            Err(TorchImageError::InvalidBackendResult)
        );
        let bad_b64 = fixture_result_body("!!!not-base64!!!");
        assert_eq!(
            decode_image_result(&bad_b64, 1280, 720),
            Err(TorchImageError::InvalidBackendResult)
        );
        // Missing required values, blank policy, and non-finite scalars fail.
        let missing_steps = serde_json::json!({
            "png_base64": good_png, "seed": 7,
            "guidance": 0.0, "memory_policy": "x", "duration_seconds": 1.5
        })
        .to_string()
        .into_bytes();
        assert_eq!(
            decode_image_result(&missing_steps, 1280, 720),
            Err(TorchImageError::InvalidBackendResult)
        );
        let blank_policy = serde_json::json!({
            "png_base64": good_png, "seed": 7, "steps": 8,
            "guidance": 0.0, "memory_policy": "  ", "duration_seconds": 1.5
        })
        .to_string()
        .into_bytes();
        assert_eq!(
            decode_image_result(&blank_policy, 1280, 720),
            Err(TorchImageError::InvalidBackendResult)
        );
        // A PNG beyond the 8 MiB decoded bound fails even with valid
        // dimensions (the whole-payload bound is enforced separately on the
        // transport while streaming).
        let mut oversized = fixture_png_bytes(64, 64);
        oversized.extend(vec![0u8; MAX_PNG_BYTES]);
        let oversized_body = fixture_result_body(&fixture_base64(&oversized));
        assert!(oversized_body.len() < MAX_IMAGE_RESPONSE_BYTES);
        assert_eq!(
            decode_image_result(&oversized_body, 64, 64),
            Err(TorchImageError::InvalidBackendResult)
        );
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

    #[tokio::test]
    async fn transcription_posts_to_the_private_route_and_decodes_results() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let backend = serve_once(
            listener,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 43\r\n\r\n{\"text\":\"ask not\",\"finish_reason\":\"length\"}",
        )
        .await;
        let audio = serde_json::json!({"encoding": "pcm_s16le"});
        let result = TorchClient::for_owned_sidecar(&endpoint)
            .transcribe_audio("asr", &audio, "en")
            .await
            .unwrap();
        assert_eq!(
            result,
            TorchTranscription {
                text: "ask not".into(),
                length_limited: true
            }
        );
        assert!(backend
            .await
            .unwrap()
            .starts_with("post /api/audio/transcriptions "));
    }

    #[test]
    fn transcription_refusals_keep_their_reasons() {
        let refusal = |code: &str, message: &str| {
            decode_operation_error(
                serde_json::json!({"detail": {"code": code, "message": message}})
                    .to_string()
                    .as_bytes(),
            )
        };
        assert_eq!(
            refusal("invalid_request", "language must be one of: en"),
            TorchOperationError::InvalidRequest("language must be one of: en".into())
        );
        assert_eq!(
            refusal("runtime_busy", ""),
            TorchOperationError::RuntimeBusy
        );
        assert_eq!(
            refusal("model_unavailable", ""),
            TorchOperationError::ModelUnavailable
        );
        assert_eq!(
            refusal("Traceback", "/secret/path"),
            TorchOperationError::BackendFailure
        );
        assert_eq!(
            decode_transcription(br#"{"text":"x","finish_reason":"other"}"#),
            Err(TorchOperationError::InvalidBackendResult)
        );
        let handshake = TorchHandshake {
            status: "ok".into(),
            protocol: SUPPORTED_TORCH_PROTOCOL,
            capabilities: vec![TORCH_IMAGE_GENERATION_CAPABILITY.into()],
        };
        assert_eq!(
            handshake.check_compatible(TORCH_AUDIO_TRANSCRIPTION_CAPABILITY),
            Err(TorchCompatibilityError::MissingCapability {
                capability: TORCH_AUDIO_TRANSCRIPTION_CAPABILITY
            })
        );
        assert!(handshake.check_image_compatible().is_ok());
    }
}
