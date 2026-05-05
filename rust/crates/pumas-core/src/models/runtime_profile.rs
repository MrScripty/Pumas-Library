//! Runtime profile contract types.
//!
//! These DTOs describe backend-owned local runtime profiles and model routes.
//! They intentionally contain no process lifecycle behavior; provider adapters
//! and services consume these contracts in later implementation slices.

use serde::{Deserialize, Serialize};

const RUNTIME_PROFILE_CURSOR_ZERO: &str = "runtime-profiles:0";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeProfileId(String);

impl RuntimeProfileId {
    pub fn parse(value: impl AsRef<str>) -> std::result::Result<Self, String> {
        let trimmed = value.as_ref().trim();
        if trimmed.is_empty() {
            return Err("runtime profile id is required".to_string());
        }
        if !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            return Err(
                "runtime profile id may only contain ASCII letters, numbers, '.', '-', or '_'"
                    .to_string(),
            );
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeEndpointUrl(String);

impl RuntimeEndpointUrl {
    pub fn parse(value: impl AsRef<str>) -> std::result::Result<Self, String> {
        let trimmed = value.as_ref().trim();
        if trimmed.is_empty() {
            return Err("runtime endpoint URL is required".to_string());
        }
        let parsed = url::Url::parse(trimmed)
            .map_err(|err| format!("runtime endpoint URL is invalid: {err}"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("runtime endpoint URL must use http or https".to_string());
        }
        if parsed.host().is_none() {
            return Err("runtime endpoint URL must include a host".to_string());
        }
        Ok(Self(parsed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimePort(u16);

impl RuntimePort {
    pub fn parse(value: u16) -> std::result::Result<Self, String> {
        if value == 0 {
            return Err("runtime port must be greater than zero".to_string());
        }
        Ok(Self(value))
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProviderId {
    Ollama,
    LlamaCpp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProviderMode {
    OllamaServe,
    LlamaCppRouter,
    LlamaCppDedicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeManagementMode {
    Managed,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDeviceMode {
    Auto,
    Cpu,
    Gpu,
    Hybrid,
    SpecificDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlamaCppProfileMode {
    Router,
    Dedicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLifecycleState {
    Unknown,
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
    External,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeDeviceSettings {
    pub mode: RuntimeDeviceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_layers: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tensor_split: Option<Vec<f32>>,
}

impl Default for RuntimeDeviceSettings {
    fn default() -> Self {
        Self {
            mode: RuntimeDeviceMode::Auto,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeSchedulerSettings {
    #[serde(default)]
    pub auto_load: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_models: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_alive_seconds: Option<u64>,
}

impl Default for RuntimeSchedulerSettings {
    fn default() -> Self {
        Self {
            auto_load: true,
            max_concurrent_models: None,
            keep_alive_seconds: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfileConfig {
    pub profile_id: RuntimeProfileId,
    pub provider: RuntimeProviderId,
    pub provider_mode: RuntimeProviderMode,
    pub management_mode: RuntimeManagementMode,
    pub name: String,
    #[serde(default = "default_profile_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<RuntimeEndpointUrl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<RuntimePort>,
    #[serde(default)]
    pub device: RuntimeDeviceSettings,
    #[serde(default)]
    pub scheduler: RuntimeSchedulerSettings,
}

impl RuntimeProfileConfig {
    pub fn default_ollama() -> Self {
        Self {
            profile_id: RuntimeProfileId("ollama-default".to_string()),
            provider: RuntimeProviderId::Ollama,
            provider_mode: RuntimeProviderMode::OllamaServe,
            management_mode: RuntimeManagementMode::Managed,
            name: "Ollama Default".to_string(),
            enabled: true,
            endpoint_url: RuntimeEndpointUrl::parse("http://127.0.0.1:11434").ok(),
            port: RuntimePort::parse(11434).ok(),
            device: RuntimeDeviceSettings::default(),
            scheduler: RuntimeSchedulerSettings::default(),
        }
    }
}

fn default_profile_enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelRuntimeRoute {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<RuntimeProfileId>,
    #[serde(default)]
    pub auto_load: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfileStatus {
    pub profile_id: RuntimeProfileId,
    pub state: RuntimeLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<RuntimeEndpointUrl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfilesSnapshot {
    pub schema_version: u32,
    pub cursor: String,
    pub profiles: Vec<RuntimeProfileConfig>,
    pub routes: Vec<ModelRuntimeRoute>,
    pub statuses: Vec<RuntimeProfileStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile_id: Option<RuntimeProfileId>,
}

impl RuntimeProfilesSnapshot {
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            cursor: RUNTIME_PROFILE_CURSOR_ZERO.to_string(),
            profiles: Vec::new(),
            routes: Vec::new(),
            statuses: Vec::new(),
            default_profile_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProfileEventKind {
    ProfileCreated,
    ProfileUpdated,
    ProfileDeleted,
    RouteUpdated,
    RouteDeleted,
    StatusChanged,
    SnapshotRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfileEvent {
    pub cursor: String,
    pub event_kind: RuntimeProfileEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<RuntimeProfileId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfileUpdateFeed {
    pub cursor: String,
    pub events: Vec<RuntimeProfileEvent>,
    pub stale_cursor: bool,
    pub snapshot_required: bool,
}

impl RuntimeProfileUpdateFeed {
    pub fn empty(cursor: Option<&str>) -> Self {
        Self {
            cursor: cursor.unwrap_or(RUNTIME_PROFILE_CURSOR_ZERO).to_string(),
            events: Vec::new(),
            stale_cursor: false,
            snapshot_required: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfilesSnapshotResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub snapshot: RuntimeProfilesSnapshot,
}

impl RuntimeProfilesSnapshotResponse {
    pub fn empty_success() -> Self {
        Self {
            success: true,
            error: None,
            snapshot: RuntimeProfilesSnapshot::empty(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfileUpdateFeedResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub feed: RuntimeProfileUpdateFeed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProfileMutationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<RuntimeProfileId>,
    #[serde(default)]
    pub snapshot_required: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_profile_id_rejects_empty_or_unsafe_values() {
        assert!(RuntimeProfileId::parse("ollama-default").is_ok());
        assert!(RuntimeProfileId::parse("").is_err());
        assert!(RuntimeProfileId::parse("../ollama").is_err());
        assert!(RuntimeProfileId::parse("ollama default").is_err());
    }

    #[test]
    fn runtime_endpoint_url_accepts_http_urls_only() {
        let endpoint = RuntimeEndpointUrl::parse("http://127.0.0.1:11434").unwrap();
        assert_eq!(endpoint.as_str(), "http://127.0.0.1:11434/");
        assert!(RuntimeEndpointUrl::parse("https://runtime.example.test").is_ok());
        assert!(RuntimeEndpointUrl::parse("file:///tmp/socket").is_err());
    }

    #[test]
    fn runtime_contract_serializes_with_snake_case_fields() {
        let profile = RuntimeProfileConfig::default_ollama();
        let encoded = serde_json::to_value(&profile).unwrap();

        assert_eq!(encoded["profile_id"], json!("ollama-default"));
        assert_eq!(encoded["provider"], json!("ollama"));
        assert_eq!(encoded["provider_mode"], json!("ollama_serve"));
        assert_eq!(encoded["management_mode"], json!("managed"));
        assert_eq!(encoded["endpoint_url"], json!("http://127.0.0.1:11434/"));
        assert_eq!(encoded["port"], json!(11434));
    }

    #[test]
    fn empty_runtime_update_feed_preserves_cursor() {
        let feed = RuntimeProfileUpdateFeed::empty(Some("runtime-profiles:42"));
        assert_eq!(feed.cursor, "runtime-profiles:42");
        assert!(feed.events.is_empty());
        assert!(!feed.stale_cursor);
        assert!(!feed.snapshot_required);
    }
}
