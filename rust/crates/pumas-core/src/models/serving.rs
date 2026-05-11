//! User-directed model serving contract types.
//!
//! These DTOs describe the renderer/backend contract for loading a model into a
//! local serving endpoint. They intentionally contain no provider lifecycle or
//! memory scheduling behavior; serving services and provider adapters consume
//! these contracts in later implementation slices.

use serde::{Deserialize, Serialize};

use super::{RuntimeDeviceMode, RuntimeEndpointUrl, RuntimeProfileId, RuntimeProviderId};

const SERVING_CURSOR_ZERO: &str = "serving:0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingEndpointMode {
    NotConfigured,
    ProviderEndpoint,
    PumasGateway,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServedModelLoadState {
    Requested,
    Loading,
    Loaded,
    Unloading,
    Unloaded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelServeErrorSeverity {
    NonCritical,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelServeErrorCode {
    InvalidRequest,
    ModelNotFound,
    ModelNotExecutable,
    ProfileNotFound,
    ProfileStopped,
    UnsupportedProvider,
    UnsupportedPlacement,
    DeviceUnavailable,
    InsufficientMemory,
    ProviderLoadFailed,
    MissingRuntime,
    InvalidFormat,
    EndpointUnavailable,
    DuplicateModelAlias,
    AmbiguousModelRouting,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelServeError {
    pub code: ModelServeErrorCode,
    pub severity: ModelServeErrorSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<RuntimeProfileId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<RuntimeProviderId>,
}

impl ModelServeError {
    pub fn non_critical(code: ModelServeErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: ModelServeErrorSeverity::NonCritical,
            message: message.into(),
            model_id: None,
            profile_id: None,
            provider: None,
        }
    }

    pub fn for_model(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self
    }

    pub fn for_profile(mut self, profile_id: RuntimeProfileId) -> Self {
        self.profile_id = Some(profile_id);
        self
    }

    pub fn for_provider(mut self, provider: RuntimeProviderId) -> Self {
        self.provider = Some(provider);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelServingConfig {
    pub provider: RuntimeProviderId,
    pub profile_id: RuntimeProfileId,
    pub device_mode: RuntimeDeviceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_layers: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tensor_split: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_size: Option<u32>,
    #[serde(default)]
    pub keep_loaded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_alias: Option<String>,
}

impl ModelServingConfig {
    pub fn validate_numeric_bounds(&self, model_id: &str) -> Vec<ModelServeError> {
        let mut errors = Vec::new();

        if self
            .device_id
            .as_ref()
            .is_some_and(|device_id| device_id.trim().is_empty())
        {
            errors.push(
                ModelServeError::non_critical(
                    ModelServeErrorCode::InvalidRequest,
                    "device_id cannot be empty when provided",
                )
                .for_model(model_id)
                .for_profile(self.profile_id.clone())
                .for_provider(self.provider),
            );
        }

        if self.gpu_layers.is_some_and(|gpu_layers| gpu_layers < -1) {
            errors.push(
                ModelServeError::non_critical(
                    ModelServeErrorCode::InvalidRequest,
                    "gpu_layers must be -1 or greater",
                )
                .for_model(model_id)
                .for_profile(self.profile_id.clone())
                .for_provider(self.provider),
            );
        }

        if let Some(tensor_split) = &self.tensor_split {
            if tensor_split.is_empty()
                || tensor_split
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
            {
                errors.push(
                    ModelServeError::non_critical(
                        ModelServeErrorCode::InvalidRequest,
                        "tensor_split values must be finite positive numbers",
                    )
                    .for_model(model_id)
                    .for_profile(self.profile_id.clone())
                    .for_provider(self.provider),
                );
            }
        }

        if self.context_size == Some(0) {
            errors.push(
                ModelServeError::non_critical(
                    ModelServeErrorCode::InvalidRequest,
                    "context_size must be greater than zero",
                )
                .for_model(model_id)
                .for_profile(self.profile_id.clone())
                .for_provider(self.provider),
            );
        }

        if self
            .model_alias
            .as_ref()
            .is_some_and(|model_alias| model_alias.trim().is_empty())
        {
            errors.push(
                ModelServeError::non_critical(
                    ModelServeErrorCode::InvalidRequest,
                    "model_alias cannot be empty when provided",
                )
                .for_model(model_id)
                .for_profile(self.profile_id.clone())
                .for_provider(self.provider),
            );
        }

        errors
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServeModelRequest {
    pub model_id: String,
    pub config: ModelServingConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UnserveModelRequest {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<RuntimeProviderId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<RuntimeProfileId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelServeValidationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub valid: bool,
    pub errors: Vec<ModelServeError>,
    pub warnings: Vec<ModelServeError>,
}

impl ModelServeValidationResponse {
    pub fn from_errors(errors: Vec<ModelServeError>) -> Self {
        Self {
            success: true,
            error: None,
            valid: errors.is_empty(),
            errors,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServingEndpointStatus {
    pub endpoint_mode: ServingEndpointMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<RuntimeEndpointUrl>,
    #[serde(default)]
    pub model_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ServingEndpointStatus {
    pub fn not_configured() -> Self {
        Self {
            endpoint_mode: ServingEndpointMode::NotConfigured,
            endpoint_url: None,
            model_count: 0,
            message: Some("Serving endpoint is not configured".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServedModelStatus {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_alias: Option<String>,
    pub provider: RuntimeProviderId,
    pub profile_id: RuntimeProfileId,
    pub load_state: ServedModelLoadState,
    pub device_mode: RuntimeDeviceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_layers: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tensor_split: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_size: Option<u32>,
    #[serde(default)]
    pub keep_loaded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<RuntimeEndpointUrl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ModelServeError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServingStatusSnapshot {
    pub schema_version: u32,
    pub cursor: String,
    pub endpoint: ServingEndpointStatus,
    pub served_models: Vec<ServedModelStatus>,
    pub last_errors: Vec<ModelServeError>,
}

impl ServingStatusSnapshot {
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            cursor: SERVING_CURSOR_ZERO.to_string(),
            endpoint: ServingEndpointStatus::not_configured(),
            served_models: Vec::new(),
            last_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServingStatusResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub snapshot: ServingStatusSnapshot,
}

impl ServingStatusResponse {
    pub fn empty_success() -> Self {
        Self {
            success: true,
            error: None,
            snapshot: ServingStatusSnapshot::empty(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingStatusEventKind {
    ModelLoaded,
    ModelUnloaded,
    LoadFailed,
    SnapshotRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServingStatusEvent {
    pub cursor: String,
    pub event_kind: ServingStatusEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<RuntimeProfileId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<RuntimeProviderId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServingStatusUpdateFeed {
    pub cursor: String,
    pub events: Vec<ServingStatusEvent>,
    pub stale_cursor: bool,
    pub snapshot_required: bool,
}

impl ServingStatusUpdateFeed {
    pub fn empty(cursor: Option<&str>) -> Self {
        Self {
            cursor: cursor.unwrap_or(SERVING_CURSOR_ZERO).to_string(),
            events: Vec::new(),
            stale_cursor: false,
            snapshot_required: false,
        }
    }

    pub fn snapshot_required(cursor: String) -> Self {
        Self {
            cursor,
            events: Vec::new(),
            stale_cursor: true,
            snapshot_required: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServingStatusUpdateFeedResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub feed: ServingStatusUpdateFeed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServeModelResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub loaded: bool,
    pub loaded_models_unchanged: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ServedModelStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_error: Option<ModelServeError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ServingStatusSnapshot>,
}

impl ServeModelResponse {
    pub fn non_critical_failure(error: ModelServeError) -> Self {
        Self {
            success: true,
            error: None,
            loaded: false,
            loaded_models_unchanged: true,
            status: None,
            load_error: Some(error),
            snapshot: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UnserveModelResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub unloaded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ServingStatusSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn serving_config() -> ModelServingConfig {
        ModelServingConfig {
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
            device_mode: RuntimeDeviceMode::Hybrid,
            device_id: Some("cuda:0".to_string()),
            gpu_layers: Some(32),
            tensor_split: Some(vec![1.0, 2.0]),
            context_size: Some(4096),
            keep_loaded: true,
            model_alias: Some("local-model".to_string()),
        }
    }

    #[test]
    fn serving_contract_serializes_with_snake_case_fields() {
        let request = ServeModelRequest {
            model_id: "models/example".to_string(),
            config: serving_config(),
        };
        let encoded = serde_json::to_value(&request).unwrap();

        assert_eq!(encoded["model_id"], json!("models/example"));
        assert_eq!(encoded["config"]["provider"], json!("llama_cpp"));
        assert_eq!(encoded["config"]["profile_id"], json!("llama-router"));
        assert_eq!(encoded["config"]["device_mode"], json!("hybrid"));
        assert_eq!(encoded["config"]["gpu_layers"], json!(32));
        assert_eq!(encoded["config"]["tensor_split"], json!([1.0, 2.0]));
        assert_eq!(encoded["config"]["context_size"], json!(4096));
        assert_eq!(encoded["config"]["keep_loaded"], json!(true));
    }

    #[test]
    fn unserve_contract_serializes_provider_scoped_identity() {
        let request = UnserveModelRequest {
            model_id: "models/example".to_string(),
            provider: Some(RuntimeProviderId::LlamaCpp),
            profile_id: Some(RuntimeProfileId::parse("llama-router").unwrap()),
            model_alias: Some("local-model".to_string()),
        };
        let encoded = serde_json::to_value(&request).unwrap();

        assert_eq!(encoded["model_id"], json!("models/example"));
        assert_eq!(encoded["provider"], json!("llama_cpp"));
        assert_eq!(encoded["profile_id"], json!("llama-router"));
        assert_eq!(encoded["model_alias"], json!("local-model"));
    }

    #[test]
    fn serving_config_numeric_validation_returns_non_critical_errors() {
        let config = ModelServingConfig {
            gpu_layers: Some(-2),
            tensor_split: Some(vec![1.0, 0.0]),
            context_size: Some(0),
            model_alias: Some("   ".to_string()),
            device_id: Some("".to_string()),
            ..serving_config()
        };

        let errors = config.validate_numeric_bounds("models/example");

        assert_eq!(errors.len(), 5);
        assert!(errors
            .iter()
            .all(|error| error.severity == ModelServeErrorSeverity::NonCritical));
        assert!(errors
            .iter()
            .all(|error| error.code == ModelServeErrorCode::InvalidRequest));
        assert!(errors
            .iter()
            .all(|error| error.model_id.as_deref() == Some("models/example")));
    }

    #[test]
    fn validation_response_marks_invalid_when_errors_exist() {
        let error = ModelServeError::non_critical(
            ModelServeErrorCode::ProfileStopped,
            "selected profile is not running",
        )
        .for_model("models/example")
        .for_profile(RuntimeProfileId::parse("ollama-default").unwrap())
        .for_provider(RuntimeProviderId::Ollama);

        let response = ModelServeValidationResponse::from_errors(vec![error.clone()]);
        let encoded = serde_json::to_value(&response).unwrap();

        assert!(response.success);
        assert!(!response.valid);
        assert_eq!(encoded["errors"][0]["severity"], json!("non_critical"));
        assert_eq!(encoded["errors"][0]["code"], json!("profile_stopped"));
        assert_eq!(encoded["errors"][0]["profile_id"], json!("ollama-default"));
    }

    #[test]
    fn non_critical_load_failure_preserves_existing_models_by_default() {
        let response = ServeModelResponse::non_critical_failure(
            ModelServeError::non_critical(
                ModelServeErrorCode::InsufficientMemory,
                "selected placement does not fit available memory",
            )
            .for_model("models/example"),
        );
        let encoded = serde_json::to_value(&response).unwrap();

        assert!(response.success);
        assert!(!response.loaded);
        assert!(response.loaded_models_unchanged);
        assert_eq!(encoded["load_error"]["severity"], json!("non_critical"));
        assert_eq!(encoded["load_error"]["code"], json!("insufficient_memory"));
    }

    #[test]
    fn empty_serving_snapshot_reports_not_configured_endpoint() {
        let snapshot = ServingStatusSnapshot::empty();

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.cursor, "serving:0");
        assert!(snapshot.served_models.is_empty());
        assert_eq!(
            snapshot.endpoint.endpoint_mode,
            ServingEndpointMode::NotConfigured
        );
    }
}
