//! Private protocol between Pumas and an ONNX host process.
//!
//! ONNX Runtime keeps the first native library a process loads, and one
//! process cannot load two versions. Each managed ONNX Runtime profile
//! therefore runs its own host process (`pumas-rpc --onnx-host`) that loads
//! the library of the installed version the profile launches. Profiles pinned
//! to different versions run side by side; changing a profile's version
//! restarts only that host.
//!
//! The host listens on the profile's loopback endpoint. Every request except
//! [`HEALTH_PATH`] carries the per-backend token in [`TOKEN_HEADER`], which
//! reaches the host only through its environment, never argv. External clients
//! never call the host: the Pumas `/v1` gateway adapts it.

use std::path::PathBuf;
use std::sync::OnceLock;

use rand::RngExt;
use serde::{Deserialize, Serialize};

/// Command-line flag that runs `pumas-rpc` as an ONNX host.
pub const HOST_FLAG: &str = "--onnx-host";
/// Command-line option naming the profile's device mode (`auto`, `cpu`, `gpu`).
pub const DEVICE_MODE_FLAG: &str = "--onnx-device-mode";
/// Environment variable carrying the host's API token.
pub const TOKEN_ENV: &str = "PUMAS_ONNX_HOST_TOKEN";
/// Header every host request except health must carry.
pub const TOKEN_HEADER: &str = "x-pumas-onnx-host-token";
/// Environment variable naming the launching backend, which the host watches.
pub const PARENT_PID_ENV: &str = "PUMAS_ONNX_HOST_PARENT_PID";
/// Environment variable naming the installed version directory to load.
pub const RUNTIME_DIR_ENV: &str = "PUMAS_ONNX_HOST_RUNTIME_DIR";
/// Environment variable naming the installed version tag, such as `v1.31.1+cuda12`.
pub const RUNTIME_VERSION_ENV: &str = "PUMAS_ONNX_HOST_RUNTIME_VERSION";
/// ONNX Runtime's own telemetry opt-out. Pumas disables third-party
/// telemetry, so every host runs with it set to `1`. ORT reads it when its
/// environment initializes, before any API call could disable telemetry: on
/// Linux and macOS it stops the 1DS uploader and its device-id and event
/// database under the user cache (`Microsoft/DeveloperTools/.onnxruntime`).
pub const ORT_DISABLE_TELEMETRY_ENV: &str = "ORT_DISABLE_TELEMETRY";
/// Developer override naming an absolute ONNX Runtime library file. When the
/// backend has it set, its hosts load that library instead of the installed
/// version's.
pub const ORT_DYLIB_PATH_ENV: &str = "ORT_DYLIB_PATH";

/// Readiness and runtime identity; never needs the token.
pub const HEALTH_PATH: &str = "/health";
/// Load a model into a session.
pub const LOAD_PATH: &str = "/v1/sessions/load";
/// Unload a model's session.
pub const UNLOAD_PATH: &str = "/v1/sessions/unload";
/// List the sessions the host holds (`GET`, token required).
pub const SESSIONS_PATH: &str = "/v1/sessions";
/// Embed with a loaded model.
pub const EMBED_PATH: &str = "/v1/embeddings";
/// Score rerank pairs with a loaded cross-encoder.
pub const RERANK_PATH: &str = "/v1/rerank";

/// Random token for ONNX hosts launched by this backend process.
///
/// Generated once per backend process and never persisted. A host left by an
/// earlier backend, or any other local process, cannot drive a host without it.
pub fn host_api_token() -> &'static str {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN.get_or_init(|| hex::encode(rand::rng().random::<[u8; 32]>()))
}

/// What a host reports about its runtime. A host that could not load its
/// library or initialize its execution provider still answers, with `ready`
/// false and the reason, so Pumas can refuse the launch with that reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxHostHealth {
    pub ready: bool,
    /// Installed version tag the host loaded, such as `v1.31.1+cuda12`.
    pub runtime_version: String,
    /// Execution provider every session uses, when ready.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_provider: Option<super::OnnxExecutionProvider>,
    /// Why the host is not ready.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Load request sent to a host. The host validates the path against the root
/// itself before reading any file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxHostLoadRequest {
    pub model_id: String,
    pub model_root: PathBuf,
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_dimensions: Option<usize>,
    /// What the session computes (pooling, normalization, cross-encoder
    /// scoring and its activation), from the model's declarations.
    #[serde(default)]
    pub task: super::OnnxSessionTask,
}

/// Unload request sent to a host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxHostUnloadRequest {
    pub model_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_stable_per_process_and_hex() {
        let token = host_api_token();
        assert_eq!(token, host_api_token());
        assert_eq!(token.len(), 64);
        assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn health_round_trips_with_and_without_a_reason() {
        let failed = OnnxHostHealth {
            ready: false,
            runtime_version: "v1.31.1+cuda12".into(),
            execution_provider: None,
            error: Some("CUDA execution provider could not initialize".into()),
        };
        let json = serde_json::to_value(&failed).unwrap();
        assert_eq!(json["ready"], false);
        assert!(json.get("execution_provider").is_none());
        assert_eq!(
            serde_json::from_value::<OnnxHostHealth>(json).unwrap(),
            failed
        );
        let ready = OnnxHostHealth {
            ready: true,
            runtime_version: "v1.31.1+cpu".into(),
            execution_provider: Some(super::super::OnnxExecutionProvider::CoreMl),
            error: None,
        };
        assert_eq!(
            serde_json::to_value(&ready).unwrap()["execution_provider"],
            "coreml"
        );
    }
}
