//! Provider-neutral runtime profile service contracts.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use crate::index::ModelRecord;
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::ModelLibrary;
use crate::models::{
    ModelRuntimeRoute, RuntimeDeviceMode, RuntimeDeviceSettings, RuntimeEndpointUrl,
    RuntimeLifecycleState, RuntimeManagementMode, RuntimePort, RuntimeProfileConfig,
    RuntimeProfileEvent, RuntimeProfileEventKind, RuntimeProfileId, RuntimeProfileMutationResponse,
    RuntimeProfileStatus, RuntimeProfileUpdateFeed, RuntimeProfileUpdateFeedResponse,
    RuntimeProfilesConfigFile, RuntimeProfilesSnapshot, RuntimeProfilesSnapshotResponse,
    RuntimeProviderId, RuntimeProviderMode,
};
use crate::providers::ProviderRegistry;
use crate::{PumasError, Result};
use tokio::sync::broadcast;

const RUNTIME_PROFILE_EVENT_RETAIN_LIMIT: usize = 256;
const RUNTIME_PROFILE_UPDATE_CHANNEL_CAPACITY: usize = 64;
const IMPLICIT_RUNTIME_PORT_SPAN: u16 = 10_000;
const OLLAMA_RUNTIME_BASE_PORT: u16 = 11_434;
const LLAMA_CPP_RUNTIME_BASE_PORT: u16 = 18_080;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeProviderCapabilities {
    pub provider: RuntimeProviderId,
    pub provider_modes: Vec<RuntimeProviderMode>,
    pub device_modes: Vec<RuntimeDeviceMode>,
    pub supports_managed_profiles: bool,
    pub supports_external_profiles: bool,
    pub supports_model_catalog: bool,
    pub supports_dedicated_model_processes: bool,
}

impl RuntimeProviderCapabilities {
    pub fn ollama() -> Self {
        Self {
            provider: RuntimeProviderId::Ollama,
            provider_modes: vec![RuntimeProviderMode::OllamaServe],
            device_modes: vec![
                RuntimeDeviceMode::Auto,
                RuntimeDeviceMode::Cpu,
                RuntimeDeviceMode::Gpu,
                RuntimeDeviceMode::Hybrid,
            ],
            supports_managed_profiles: true,
            supports_external_profiles: true,
            supports_model_catalog: false,
            supports_dedicated_model_processes: false,
        }
    }

    pub fn llama_cpp() -> Self {
        Self {
            provider: RuntimeProviderId::LlamaCpp,
            provider_modes: vec![
                RuntimeProviderMode::LlamaCppRouter,
                RuntimeProviderMode::LlamaCppDedicated,
            ],
            device_modes: vec![
                RuntimeDeviceMode::Auto,
                RuntimeDeviceMode::Cpu,
                RuntimeDeviceMode::Gpu,
                RuntimeDeviceMode::SpecificDevice,
            ],
            supports_managed_profiles: true,
            supports_external_profiles: true,
            supports_model_catalog: true,
            supports_dedicated_model_processes: true,
        }
    }
}

#[async_trait]
pub trait RuntimeProviderAdapter: Send + Sync {
    fn provider(&self) -> RuntimeProviderId;
    fn capabilities(&self) -> RuntimeProviderCapabilities;
    async fn validate_profile(&self, profile: &RuntimeProfileConfig) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LlamaCppRouterCatalog {
    pub entries: Vec<LlamaCppRouterCatalogEntry>,
    pub preset_ini: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LlamaCppRouterCatalogEntry {
    pub model_id: String,
    pub alias: String,
    pub model_type: String,
    pub model_path: PathBuf,
}

impl LlamaCppRouterCatalog {
    fn from_entries(mut entries: Vec<LlamaCppRouterCatalogEntry>) -> Self {
        entries.sort_by(|left, right| {
            left.model_id
                .cmp(&right.model_id)
                .then_with(|| left.model_path.cmp(&right.model_path))
        });
        let preset_ini = build_llama_cpp_router_preset_ini(&entries);
        Self {
            entries,
            preset_ini,
        }
    }
}

pub async fn generate_llama_cpp_router_catalog(
    library: Arc<ModelLibrary>,
) -> Result<LlamaCppRouterCatalog> {
    let records = library.list_models().await?;
    tokio::task::spawn_blocking(move || {
        let mut entries = Vec::new();
        for record in records {
            if let Some(entry) = llama_cpp_router_catalog_entry_for_record(&library, &record) {
                entries.push(entry);
            }
        }
        Ok(LlamaCppRouterCatalog::from_entries(entries))
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join llama.cpp router catalog generation task: {err}"
        ))
    })?
}

fn llama_cpp_router_catalog_entry_for_record(
    library: &ModelLibrary,
    record: &ModelRecord,
) -> Option<LlamaCppRouterCatalogEntry> {
    let model_path = library.get_primary_model_file(&record.id)?;
    if model_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| !extension.eq_ignore_ascii_case("gguf"))
        .unwrap_or(true)
    {
        return None;
    }

    Some(LlamaCppRouterCatalogEntry {
        model_id: record.id.clone(),
        alias: record.id.clone(),
        model_type: record.model_type.clone(),
        model_path,
    })
}

fn build_llama_cpp_router_preset_ini(entries: &[LlamaCppRouterCatalogEntry]) -> String {
    let mut output = String::from("version = 1\n\n[*]\nload-on-startup = false\n\n");
    for entry in entries {
        output.push('[');
        output.push_str(&sanitize_llama_cpp_preset_section(&entry.model_id));
        output.push_str("]\nmodel = ");
        output.push_str(&sanitize_llama_cpp_preset_value(
            entry.model_path.to_string_lossy().as_ref(),
        ));
        output.push_str("\nalias = ");
        output.push_str(&sanitize_llama_cpp_preset_value(&entry.alias));
        if entry.model_type.eq_ignore_ascii_case("embedding") {
            output.push_str("\nembedding = true");
        } else if entry.model_type.eq_ignore_ascii_case("reranker") {
            output.push_str("\nreranking = true");
        }
        output.push_str("\n\n");
    }
    output
}

fn sanitize_llama_cpp_preset_section(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '[' | ']' | '\r' | '\n' => '_',
            other => other,
        })
        .collect()
}

fn sanitize_llama_cpp_preset_value(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

pub struct OllamaRuntimeProviderAdapter;

#[async_trait]
impl RuntimeProviderAdapter for OllamaRuntimeProviderAdapter {
    fn provider(&self) -> RuntimeProviderId {
        RuntimeProviderId::Ollama
    }

    fn capabilities(&self) -> RuntimeProviderCapabilities {
        RuntimeProviderCapabilities::ollama()
    }

    async fn validate_profile(&self, profile: &RuntimeProfileConfig) -> Result<()> {
        if profile.provider != RuntimeProviderId::Ollama {
            return Err(PumasError::InvalidParams {
                message: "Ollama adapter received a non-Ollama profile".to_string(),
            });
        }
        if profile.provider_mode != RuntimeProviderMode::OllamaServe {
            return Err(PumasError::InvalidParams {
                message: "Ollama profiles must use provider_mode=ollama_serve".to_string(),
            });
        }
        if profile.management_mode == RuntimeManagementMode::External
            && profile.endpoint_url.is_none()
        {
            return Err(PumasError::InvalidParams {
                message: "external Ollama profiles require endpoint_url".to_string(),
            });
        }
        Ok(())
    }
}

pub struct LlamaCppRuntimeProviderAdapter;

#[async_trait]
impl RuntimeProviderAdapter for LlamaCppRuntimeProviderAdapter {
    fn provider(&self) -> RuntimeProviderId {
        RuntimeProviderId::LlamaCpp
    }

    fn capabilities(&self) -> RuntimeProviderCapabilities {
        RuntimeProviderCapabilities::llama_cpp()
    }

    async fn validate_profile(&self, profile: &RuntimeProfileConfig) -> Result<()> {
        if profile.provider != RuntimeProviderId::LlamaCpp {
            return Err(PumasError::InvalidParams {
                message: "llama.cpp adapter received a non-llama.cpp profile".to_string(),
            });
        }
        match profile.provider_mode {
            RuntimeProviderMode::LlamaCppRouter | RuntimeProviderMode::LlamaCppDedicated => {}
            _ => {
                return Err(PumasError::InvalidParams {
                    message: "llama.cpp provider mode does not match provider".to_string(),
                });
            }
        }
        if profile.management_mode == RuntimeManagementMode::External
            && profile.endpoint_url.is_none()
        {
            return Err(PumasError::InvalidParams {
                message: "external llama.cpp profiles require endpoint_url".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeProfileService {
    launcher_root: PathBuf,
    config_path: PathBuf,
    write_lock: Arc<RwLock<()>>,
    event_journal: Arc<RwLock<RuntimeProfileEventJournal>>,
    updates: broadcast::Sender<RuntimeProfileUpdateFeed>,
    operation_locks: Arc<Mutex<HashSet<RuntimeProfileId>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProfileLaunchSpec {
    pub profile_id: RuntimeProfileId,
    pub provider: RuntimeProviderId,
    pub provider_mode: RuntimeProviderMode,
    pub endpoint_url: RuntimeEndpointUrl,
    pub port: RuntimePort,
    pub extra_args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub runtime_dir: PathBuf,
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
    pub health_check_url: RuntimeEndpointUrl,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeProfileLaunchOverrides {
    pub device: Option<RuntimeDeviceSettings>,
    pub context_size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRuntimeProfileEndpoint {
    profile_id: RuntimeProfileId,
    endpoint_url: RuntimeEndpointUrl,
    management_mode: RuntimeManagementMode,
}

#[derive(Debug)]
pub struct RuntimeProfileOperationGuard {
    profile_id: RuntimeProfileId,
    operation_locks: Arc<Mutex<HashSet<RuntimeProfileId>>>,
}

impl Drop for RuntimeProfileOperationGuard {
    fn drop(&mut self) {
        if let Ok(mut operation_locks) = self.operation_locks.lock() {
            operation_locks.remove(&self.profile_id);
        }
    }
}

impl RuntimeProfileService {
    pub fn new(launcher_root: impl AsRef<Path>) -> Self {
        let launcher_root = launcher_root.as_ref().to_path_buf();
        Self {
            config_path: launcher_root
                .join("launcher-data")
                .join("metadata")
                .join("runtime-profiles.json"),
            launcher_root,
            write_lock: Arc::new(RwLock::new(())),
            event_journal: Arc::new(RwLock::new(RuntimeProfileEventJournal::default())),
            updates: broadcast::channel(RUNTIME_PROFILE_UPDATE_CHANNEL_CAPACITY).0,
            operation_locks: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn snapshot(&self) -> Result<RuntimeProfilesSnapshotResponse> {
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        let config = tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            load_or_initialize_config(&config_path)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join runtime profile snapshot task: {err}"
            ))
        })??;

        let snapshot = self.apply_runtime_statuses(config.snapshot())?;

        Ok(RuntimeProfilesSnapshotResponse {
            success: true,
            error: None,
            snapshot,
        })
    }

    pub async fn list_updates_since(
        &self,
        cursor: Option<&str>,
    ) -> Result<RuntimeProfileUpdateFeedResponse> {
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        let requested_cursor = cursor.map(ToOwned::to_owned);
        let config_cursor = tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let config = load_or_initialize_config(&config_path)?;
            Ok::<_, PumasError>(config.cursor)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join runtime profile update-feed task: {err}"
            ))
        })??;
        let feed = self.build_update_feed(requested_cursor.as_deref(), &config_cursor)?;

        Ok(RuntimeProfileUpdateFeedResponse {
            success: true,
            error: None,
            feed,
        })
    }

    pub fn subscribe_updates(&self) -> broadcast::Receiver<RuntimeProfileUpdateFeed> {
        self.updates.subscribe()
    }

    pub async fn record_default_ollama_status(
        &self,
        is_running: bool,
    ) -> Result<Option<RuntimeProfileEvent>> {
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        let default_status =
            tokio::task::spawn_blocking(move || -> Result<Option<RuntimeProfileStatus>> {
                let _guard = write_lock.write().map_err(|_| {
                    PumasError::Other("Failed to acquire runtime profile config lock".to_string())
                })?;
                let config = load_or_initialize_config(&config_path)?;
                let Some(default_profile_id) = config.default_profile_id.clone() else {
                    return Ok(None);
                };
                let Some(profile) = config
                    .profiles
                    .iter()
                    .find(|profile| profile.profile_id == default_profile_id)
                else {
                    return Ok(None);
                };
                if profile.provider != RuntimeProviderId::Ollama {
                    return Ok(None);
                }

                Ok(Some(RuntimeProfileStatus {
                    profile_id: profile.profile_id.clone(),
                    state: if is_running {
                        RuntimeLifecycleState::Running
                    } else {
                        RuntimeLifecycleState::Stopped
                    },
                    endpoint_url: profile.endpoint_url.clone(),
                    pid: None,
                    log_path: None,
                    last_error: None,
                }))
            })
            .await
            .map_err(|err| {
                PumasError::Other(format!(
                    "Failed to join default Ollama status refresh task: {err}"
                ))
            })??;

        let Some(status) = default_status else {
            return Ok(None);
        };
        self.record_profile_status(status)
    }

    pub async fn list_managed_profile_launch_specs(&self) -> Result<Vec<RuntimeProfileLaunchSpec>> {
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        let launcher_root = self.launcher_root.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let config = load_or_initialize_config(&config_path)?;
            derive_managed_profile_launch_specs(&launcher_root, &config)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join runtime profile launch-spec task: {err}"
            ))
        })?
    }

    pub async fn managed_profile_launch_spec(
        &self,
        profile_id: RuntimeProfileId,
    ) -> Result<RuntimeProfileLaunchSpec> {
        let specs = self.list_managed_profile_launch_specs().await?;
        specs
            .into_iter()
            .find(|spec| spec.profile_id == profile_id)
            .ok_or_else(|| PumasError::InvalidParams {
                message: format!("managed runtime profile not found: {}", profile_id.as_str()),
            })
    }

    pub fn begin_profile_operation(
        &self,
        profile_id: RuntimeProfileId,
    ) -> Result<RuntimeProfileOperationGuard> {
        let mut operation_locks = self.operation_locks.lock().map_err(|_| {
            PumasError::Other("Failed to acquire runtime profile operation lock".to_string())
        })?;
        if !operation_locks.insert(profile_id.clone()) {
            return Err(PumasError::InvalidParams {
                message: format!(
                    "runtime profile operation already in progress: {}",
                    profile_id.as_str()
                ),
            });
        }
        Ok(RuntimeProfileOperationGuard {
            profile_id,
            operation_locks: self.operation_locks.clone(),
        })
    }

    pub fn record_profile_lifecycle_status(
        &self,
        status: RuntimeProfileStatus,
    ) -> Result<Option<RuntimeProfileEvent>> {
        self.record_profile_status(status)
    }

    pub async fn upsert_profile(
        &self,
        profile: RuntimeProfileConfig,
    ) -> Result<RuntimeProfileMutationResponse> {
        validate_profile_config(&profile).await?;
        let profile_id = profile.profile_id.clone();
        let launcher_root = self.launcher_root.clone();
        self.mutate_config(move |config| {
            if let Some(existing) = config
                .profiles
                .iter_mut()
                .find(|existing| existing.profile_id == profile.profile_id)
            {
                *existing = profile;
            } else {
                config.profiles.push(profile);
            }
            derive_managed_profile_launch_specs(&launcher_root, config)?;
            Ok(RuntimeProfileMutationResponse::success(Some(profile_id)))
        })
        .await
    }

    pub async fn delete_profile(
        &self,
        profile_id: RuntimeProfileId,
    ) -> Result<RuntimeProfileMutationResponse> {
        self.mutate_config(move |config| {
            config
                .profiles
                .retain(|profile| profile.profile_id != profile_id);
            config.routes.retain(|route| {
                route
                    .profile_id
                    .as_ref()
                    .map(|route_profile_id| route_profile_id != &profile_id)
                    .unwrap_or(true)
            });
            if config.default_profile_id.as_ref() == Some(&profile_id) {
                config.default_profile_id = config
                    .profiles
                    .first()
                    .map(|profile| profile.profile_id.clone());
            }
            Ok(RuntimeProfileMutationResponse::success(Some(profile_id)))
        })
        .await
    }

    pub async fn set_model_route(
        &self,
        route: ModelRuntimeRoute,
    ) -> Result<RuntimeProfileMutationResponse> {
        validate_model_route(&route)?;
        self.mutate_config(move |config| {
            if let Some(profile_id) = &route.profile_id {
                if !config
                    .profiles
                    .iter()
                    .any(|profile| &profile.profile_id == profile_id)
                {
                    return Err(PumasError::InvalidParams {
                        message: format!("runtime profile not found: {}", profile_id.as_str()),
                    });
                }
            }
            if let Some(existing) = config
                .routes
                .iter_mut()
                .find(|existing| existing.model_id == route.model_id)
            {
                *existing = route;
            } else {
                config.routes.push(route);
            }
            Ok(RuntimeProfileMutationResponse::success(None))
        })
        .await
    }

    pub async fn clear_model_route(
        &self,
        model_id: String,
    ) -> Result<RuntimeProfileMutationResponse> {
        let model_id = model_id.trim().to_string();
        if model_id.is_empty() {
            return Err(PumasError::InvalidParams {
                message: "model_id is required".to_string(),
            });
        }
        self.mutate_config(move |config| {
            config.routes.retain(|route| route.model_id != model_id);
            Ok(RuntimeProfileMutationResponse::success(None))
        })
        .await
    }

    pub async fn resolve_profile_endpoint(
        &self,
        provider: RuntimeProviderId,
        profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        Ok(self
            .resolve_profile_endpoint_detail(provider, profile_id)
            .await?
            .endpoint_url)
    }

    pub async fn resolve_profile_endpoint_for_operation(
        &self,
        provider: RuntimeProviderId,
        profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        let resolved = self
            .resolve_profile_endpoint_detail(provider, profile_id)
            .await?;
        self.ensure_profile_available_for_operation(&resolved)?;
        Ok(resolved.endpoint_url)
    }

    async fn resolve_profile_endpoint_detail(
        &self,
        provider: RuntimeProviderId,
        profile_id: Option<RuntimeProfileId>,
    ) -> Result<ResolvedRuntimeProfileEndpoint> {
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        let launcher_root = self.launcher_root.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let config = load_or_initialize_config(&config_path)?;
            let selected_profile_id = profile_id
                .or_else(|| config.default_profile_id.clone())
                .ok_or_else(|| PumasError::InvalidParams {
                    message: "runtime profile id is required".to_string(),
                })?;
            resolve_config_profile_endpoint(&launcher_root, &config, provider, selected_profile_id)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join runtime profile endpoint resolution task: {err}"
            ))
        })?
    }

    pub async fn resolve_model_endpoint(
        &self,
        provider: RuntimeProviderId,
        model_id: &str,
        explicit_profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        Ok(self
            .resolve_model_endpoint_detail(provider, model_id, explicit_profile_id)
            .await?
            .endpoint_url)
    }

    pub async fn resolve_model_endpoint_for_operation(
        &self,
        provider: RuntimeProviderId,
        model_id: &str,
        explicit_profile_id: Option<RuntimeProfileId>,
    ) -> Result<RuntimeEndpointUrl> {
        let resolved = self
            .resolve_model_endpoint_detail(provider, model_id, explicit_profile_id)
            .await?;
        self.ensure_profile_available_for_operation(&resolved)?;
        Ok(resolved.endpoint_url)
    }

    async fn resolve_model_endpoint_detail(
        &self,
        provider: RuntimeProviderId,
        model_id: &str,
        explicit_profile_id: Option<RuntimeProfileId>,
    ) -> Result<ResolvedRuntimeProfileEndpoint> {
        let model_id = model_id.trim().to_string();
        if model_id.is_empty() {
            return Err(PumasError::InvalidParams {
                message: "model_id is required".to_string(),
            });
        }

        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        let launcher_root = self.launcher_root.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let config = load_or_initialize_config(&config_path)?;
            let routed_profile_id = explicit_profile_id
                .or_else(|| {
                    config
                        .routes
                        .iter()
                        .find(|route| route.model_id == model_id)
                        .and_then(|route| route.profile_id.clone())
                })
                .or_else(|| config.default_profile_id.clone())
                .ok_or_else(|| PumasError::InvalidParams {
                    message: "runtime profile id is required".to_string(),
                })?;
            resolve_config_profile_endpoint(&launcher_root, &config, provider, routed_profile_id)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join model runtime profile endpoint resolution task: {err}"
            ))
        })?
    }

    pub async fn model_route_auto_load(&self, model_id: &str) -> Result<Option<bool>> {
        let model_id = model_id.trim().to_string();
        if model_id.is_empty() {
            return Err(PumasError::InvalidParams {
                message: "model_id is required".to_string(),
            });
        }

        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let config = load_or_initialize_config(&config_path)?;
            Ok(config
                .routes
                .iter()
                .find(|route| route.model_id == model_id)
                .map(|route| route.auto_load))
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join model runtime route auto-load task: {err}"
            ))
        })?
    }

    fn ensure_profile_available_for_operation(
        &self,
        resolved: &ResolvedRuntimeProfileEndpoint,
    ) -> Result<()> {
        if resolved.management_mode == RuntimeManagementMode::External {
            return Ok(());
        }

        let journal = self.event_journal.read().map_err(|_| {
            PumasError::Other("Failed to acquire runtime profile event journal lock".to_string())
        })?;
        let state = journal
            .status_for(&resolved.profile_id)
            .map(|status| status.state)
            .unwrap_or(RuntimeLifecycleState::Stopped);
        if state == RuntimeLifecycleState::Running {
            return Ok(());
        }

        Err(PumasError::InvalidParams {
            message: format!(
                "runtime profile {} is not running (state={:?})",
                resolved.profile_id.as_str(),
                state
            ),
        })
    }

    async fn mutate_config<F>(&self, mutate: F) -> Result<RuntimeProfileMutationResponse>
    where
        F: FnOnce(&mut RuntimeProfilesConfigFile) -> Result<RuntimeProfileMutationResponse>
            + Send
            + 'static,
    {
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        let (response, cursor) = tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let mut config = load_or_initialize_config(&config_path)?;
            let response = mutate(&mut config)?;
            bump_cursor(&mut config);
            let cursor = config.cursor.clone();
            atomic_write_json(&config_path, &config, true)?;
            Ok::<_, PumasError>((response, cursor))
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join runtime profile mutation task: {err}"
            ))
        })??;
        self.publish_feed(RuntimeProfileUpdateFeed::snapshot_required(cursor));
        Ok(response)
    }

    fn apply_runtime_statuses(
        &self,
        mut snapshot: RuntimeProfilesSnapshot,
    ) -> Result<RuntimeProfilesSnapshot> {
        let mut journal = self.event_journal.write().map_err(|_| {
            PumasError::Other("Failed to acquire runtime profile event journal lock".to_string())
        })?;
        journal.ensure_cursor_at_least(&snapshot.cursor);
        snapshot.cursor = journal.current_cursor();
        for status in &mut snapshot.statuses {
            if let Some(runtime_status) = journal.status_for(&status.profile_id) {
                *status = runtime_status;
            }
        }
        Ok(snapshot)
    }

    fn build_update_feed(
        &self,
        requested_cursor: Option<&str>,
        config_cursor: &str,
    ) -> Result<RuntimeProfileUpdateFeed> {
        let mut journal = self.event_journal.write().map_err(|_| {
            PumasError::Other("Failed to acquire runtime profile event journal lock".to_string())
        })?;
        journal.ensure_cursor_at_least(config_cursor);
        let current_cursor = journal.current_cursor();
        let config_cursor_number =
            parse_runtime_profile_cursor(config_cursor).ok_or_else(|| {
                PumasError::InvalidParams {
                    message: format!("invalid runtime profile cursor: {config_cursor}"),
                }
            })?;

        let Some(requested_cursor) = requested_cursor else {
            return Ok(RuntimeProfileUpdateFeed::snapshot_required(current_cursor));
        };
        let Some(requested_cursor_number) = parse_runtime_profile_cursor(requested_cursor) else {
            return Ok(RuntimeProfileUpdateFeed::snapshot_required(current_cursor));
        };
        if requested_cursor_number < config_cursor_number {
            return Ok(RuntimeProfileUpdateFeed::snapshot_required(current_cursor));
        }
        if requested_cursor_number == journal.cursor {
            return Ok(RuntimeProfileUpdateFeed::empty(Some(&current_cursor)));
        }

        if let Some(events) = journal.events_after(requested_cursor_number) {
            if !events.is_empty() {
                return Ok(RuntimeProfileUpdateFeed {
                    cursor: current_cursor,
                    events,
                    stale_cursor: false,
                    snapshot_required: false,
                });
            }
        }

        Ok(RuntimeProfileUpdateFeed::snapshot_required(current_cursor))
    }

    fn record_profile_status(
        &self,
        status: RuntimeProfileStatus,
    ) -> Result<Option<RuntimeProfileEvent>> {
        let mut journal = self.event_journal.write().map_err(|_| {
            PumasError::Other("Failed to acquire runtime profile event journal lock".to_string())
        })?;
        let event = journal.record_status(status);
        if let Some(event) = event.clone() {
            self.publish_feed(RuntimeProfileUpdateFeed {
                cursor: event.cursor.clone(),
                events: vec![event],
                stale_cursor: false,
                snapshot_required: false,
            });
        }
        Ok(event)
    }

    fn publish_feed(&self, feed: RuntimeProfileUpdateFeed) {
        let _ = self.updates.send(feed);
    }
}

#[derive(Debug, Default)]
struct RuntimeProfileEventJournal {
    cursor: u64,
    events: VecDeque<RuntimeProfileEvent>,
    statuses: HashMap<RuntimeProfileId, RuntimeProfileStatus>,
}

impl RuntimeProfileEventJournal {
    fn ensure_cursor_at_least(&mut self, cursor: &str) {
        if let Some(cursor) = parse_runtime_profile_cursor(cursor) {
            self.cursor = self.cursor.max(cursor);
        }
    }

    fn current_cursor(&self) -> String {
        format_runtime_profile_cursor(self.cursor)
    }

    fn status_for(&self, profile_id: &RuntimeProfileId) -> Option<RuntimeProfileStatus> {
        self.statuses.get(profile_id).cloned()
    }

    fn events_after(&self, requested_cursor: u64) -> Option<Vec<RuntimeProfileEvent>> {
        if let Some(first_event) = self.events.front() {
            let first_event_cursor = parse_runtime_profile_cursor(&first_event.cursor)?;
            if requested_cursor.saturating_add(1) < first_event_cursor {
                return None;
            }
        } else if requested_cursor < self.cursor {
            return None;
        }

        Some(
            self.events
                .iter()
                .filter(|event| {
                    parse_runtime_profile_cursor(&event.cursor)
                        .map(|cursor| cursor > requested_cursor)
                        .unwrap_or(false)
                })
                .cloned()
                .collect(),
        )
    }

    fn record_status(&mut self, status: RuntimeProfileStatus) -> Option<RuntimeProfileEvent> {
        let status_changed = self
            .statuses
            .get(&status.profile_id)
            .map(|existing| existing != &status)
            .unwrap_or(status.state != RuntimeLifecycleState::Stopped);
        self.statuses
            .insert(status.profile_id.clone(), status.clone());

        if !status_changed {
            return None;
        }

        self.cursor = self.cursor.saturating_add(1);
        let event = RuntimeProfileEvent {
            cursor: self.current_cursor(),
            event_kind: RuntimeProfileEventKind::StatusChanged,
            profile_id: Some(status.profile_id),
            model_id: None,
            producer_revision: Some("runtime-profile-status".to_string()),
        };
        self.events.push_back(event.clone());
        while self.events.len() > RUNTIME_PROFILE_EVENT_RETAIN_LIMIT {
            self.events.pop_front();
        }
        Some(event)
    }
}

fn derive_managed_profile_launch_specs(
    launcher_root: &Path,
    config: &RuntimeProfilesConfigFile,
) -> Result<Vec<RuntimeProfileLaunchSpec>> {
    let mut used_ports: HashMap<u16, RuntimeProfileId> = HashMap::new();
    let mut profiles = config
        .profiles
        .iter()
        .filter(|profile| profile.management_mode == RuntimeManagementMode::Managed)
        .collect::<Vec<_>>();
    profiles.sort_by(|left, right| left.profile_id.as_str().cmp(right.profile_id.as_str()));

    let mut specs = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let port = match profile.port {
            Some(port) => {
                if let Some(existing_profile_id) = used_ports.get(&port.value()) {
                    return Err(PumasError::InvalidParams {
                        message: format!(
                            "runtime profile port collision: {} is already used by {}; choose a unique managed profile process port or leave the port blank for automatic allocation",
                            port.value(),
                            existing_profile_id.as_str()
                        ),
                    });
                }
                used_ports.insert(port.value(), profile.profile_id.clone());
                port
            }
            None => match profile.endpoint_url.as_ref().and_then(endpoint_port) {
                Some(port) => {
                    if let Some(existing_profile_id) = used_ports.get(&port.value()) {
                        return Err(PumasError::InvalidParams {
                            message: format!(
                                "runtime profile endpoint port collision: {} is already used by {}; choose a unique managed profile endpoint or leave the endpoint blank for automatic allocation",
                                port.value(),
                                existing_profile_id.as_str()
                            ),
                        });
                    }
                    used_ports.insert(port.value(), profile.profile_id.clone());
                    port
                }
                None => allocate_implicit_runtime_port(profile, &mut used_ports)?,
            },
        };
        let endpoint_url = match &profile.endpoint_url {
            Some(endpoint_url) => endpoint_url.clone(),
            None => endpoint_url_for_port(port)?,
        };
        let runtime_dir = launcher_root
            .join("launcher-data")
            .join("runtime-profiles")
            .join(provider_path_segment(profile.provider))
            .join(profile.profile_id.as_str());

        specs.push(RuntimeProfileLaunchSpec {
            profile_id: profile.profile_id.clone(),
            provider: profile.provider,
            provider_mode: profile.provider_mode,
            endpoint_url: endpoint_url.clone(),
            port,
            extra_args: profile_runtime_extra_args(launcher_root, profile, &endpoint_url, port)?,
            env_vars: profile_runtime_env_vars(profile, &endpoint_url, port)?,
            pid_file: runtime_dir.join("runtime.pid"),
            log_file: runtime_dir.join("runtime.log"),
            health_check_url: endpoint_url,
            runtime_dir,
        });
    }

    Ok(specs)
}

fn resolve_config_profile_endpoint(
    launcher_root: &Path,
    config: &RuntimeProfilesConfigFile,
    provider: RuntimeProviderId,
    profile_id: RuntimeProfileId,
) -> Result<ResolvedRuntimeProfileEndpoint> {
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
        .ok_or_else(|| PumasError::InvalidParams {
            message: format!("runtime profile not found: {}", profile_id.as_str()),
        })?;
    if profile.provider != provider {
        return Err(PumasError::InvalidParams {
            message: format!(
                "runtime profile {} does not use provider {:?}",
                profile_id.as_str(),
                provider
            ),
        });
    }
    let endpoint_url = match &profile.endpoint_url {
        Some(endpoint_url) => endpoint_url.clone(),
        None if profile.management_mode == RuntimeManagementMode::Managed => {
            derive_managed_profile_launch_specs(launcher_root, config)?
                .into_iter()
                .find(|spec| spec.profile_id == profile_id)
                .map(|spec| spec.endpoint_url)
                .ok_or_else(|| PumasError::InvalidParams {
                    message: format!(
                        "managed runtime profile endpoint could not be derived: {}",
                        profile_id.as_str()
                    ),
                })?
        }
        None => {
            return Err(PumasError::InvalidParams {
                message: format!(
                    "runtime profile {} does not define endpoint_url",
                    profile_id.as_str()
                ),
            });
        }
    };
    Ok(ResolvedRuntimeProfileEndpoint {
        profile_id,
        endpoint_url,
        management_mode: profile.management_mode,
    })
}

fn allocate_implicit_runtime_port(
    profile: &RuntimeProfileConfig,
    used_ports: &mut HashMap<u16, RuntimeProfileId>,
) -> Result<RuntimePort> {
    let base_port = provider_base_port(profile.provider);
    let start_offset = implicit_port_offset(profile.profile_id.as_str());
    for step in 0..IMPLICIT_RUNTIME_PORT_SPAN {
        let offset =
            ((start_offset as u32 + step as u32) % IMPLICIT_RUNTIME_PORT_SPAN as u32) as u16;
        let candidate = base_port as u32 + 1 + offset as u32;
        if candidate > u16::MAX as u32 {
            continue;
        }
        let candidate = candidate as u16;
        if let std::collections::hash_map::Entry::Vacant(entry) = used_ports.entry(candidate) {
            entry.insert(profile.profile_id.clone());
            return RuntimePort::parse(candidate).map_err(|message| PumasError::InvalidParams {
                message: format!("invalid implicit runtime port: {message}"),
            });
        }
    }

    Err(PumasError::InvalidParams {
        message: format!(
            "no available implicit runtime ports for profile {}",
            profile.profile_id.as_str()
        ),
    })
}

fn implicit_port_offset(profile_id: &str) -> u16 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    profile_id.hash(&mut hasher);
    (hasher.finish() % IMPLICIT_RUNTIME_PORT_SPAN as u64) as u16
}

fn endpoint_url_for_port(port: RuntimePort) -> Result<RuntimeEndpointUrl> {
    RuntimeEndpointUrl::parse(format!("http://127.0.0.1:{}", port.value())).map_err(|message| {
        PumasError::InvalidParams {
            message: format!("invalid runtime endpoint URL: {message}"),
        }
    })
}

fn endpoint_port(endpoint_url: &RuntimeEndpointUrl) -> Option<RuntimePort> {
    url::Url::parse(endpoint_url.as_str())
        .ok()
        .and_then(|url| url.port())
        .and_then(|port| RuntimePort::parse(port).ok())
}

fn profile_runtime_env_vars(
    profile: &RuntimeProfileConfig,
    endpoint_url: &RuntimeEndpointUrl,
    port: RuntimePort,
) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();
    env_vars.insert(
        "PUMAS_RUNTIME_PROFILE_ID".to_string(),
        profile.profile_id.as_str().to_string(),
    );
    match profile.provider {
        RuntimeProviderId::Ollama => {
            env_vars.insert(
                "OLLAMA_HOST".to_string(),
                runtime_host_port(endpoint_url, port)?,
            );
        }
        RuntimeProviderId::LlamaCpp => {}
    }
    apply_device_visibility_env(&mut env_vars, profile);
    Ok(env_vars)
}

fn profile_runtime_extra_args(
    launcher_root: &Path,
    profile: &RuntimeProfileConfig,
    endpoint_url: &RuntimeEndpointUrl,
    port: RuntimePort,
) -> Result<Vec<String>> {
    match profile.provider {
        RuntimeProviderId::Ollama => Ok(Vec::new()),
        RuntimeProviderId::LlamaCpp => match profile.provider_mode {
            RuntimeProviderMode::LlamaCppRouter | RuntimeProviderMode::LlamaCppDedicated => {
                let mut args = vec![
                    "--host".to_string(),
                    runtime_host(endpoint_url)?.to_string(),
                    "--port".to_string(),
                    port.value().to_string(),
                ];
                if profile.provider_mode == RuntimeProviderMode::LlamaCppRouter {
                    args.extend([
                        "--models-dir".to_string(),
                        llama_cpp_router_models_dir(launcher_root)
                            .to_string_lossy()
                            .to_string(),
                    ]);
                }
                apply_llama_cpp_device_args(&mut args, profile);
                Ok(args)
            }
            RuntimeProviderMode::OllamaServe => Err(PumasError::InvalidParams {
                message: "llama.cpp runtime profile cannot use ollama_serve mode".to_string(),
            }),
        },
    }
}

fn apply_llama_cpp_device_args(args: &mut Vec<String>, profile: &RuntimeProfileConfig) {
    if let Some(gpu_layers) = llama_cpp_gpu_layers_arg(&profile.device) {
        args.extend(["--n-gpu-layers".to_string(), gpu_layers.to_string()]);
    }

    if let Some(tensor_split) = &profile.device.tensor_split {
        if !tensor_split.is_empty() {
            args.extend([
                "--tensor-split".to_string(),
                tensor_split
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ]);
        }
    }
}

fn llama_cpp_gpu_layers_arg(device: &RuntimeDeviceSettings) -> Option<i32> {
    match device.mode {
        RuntimeDeviceMode::Cpu => Some(0),
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::SpecificDevice => {
            Some(device.gpu_layers.unwrap_or(-1))
        }
        RuntimeDeviceMode::Auto | RuntimeDeviceMode::Hybrid => device.gpu_layers,
    }
}

fn llama_cpp_router_models_dir(launcher_root: &Path) -> PathBuf {
    launcher_root.join("shared-resources").join("models")
}

fn runtime_host(endpoint_url: &RuntimeEndpointUrl) -> Result<String> {
    let parsed =
        url::Url::parse(endpoint_url.as_str()).map_err(|err| PumasError::InvalidParams {
            message: format!("invalid runtime endpoint URL: {err}"),
        })?;
    parsed
        .host_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| PumasError::InvalidParams {
            message: "runtime endpoint URL must include a host".to_string(),
        })
}

fn runtime_host_port(endpoint_url: &RuntimeEndpointUrl, port: RuntimePort) -> Result<String> {
    let host = runtime_host(endpoint_url)?;
    Ok(format!("{host}:{}", port.value()))
}

fn apply_device_visibility_env(
    env_vars: &mut HashMap<String, String>,
    profile: &RuntimeProfileConfig,
) {
    match profile.device.mode {
        RuntimeDeviceMode::Cpu => {
            env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), String::new());
            env_vars.insert("HIP_VISIBLE_DEVICES".to_string(), String::new());
            env_vars.insert("ROCR_VISIBLE_DEVICES".to_string(), String::new());
        }
        RuntimeDeviceMode::Gpu | RuntimeDeviceMode::SpecificDevice => {
            if let Some(device_id) = profile.device.device_id.as_deref() {
                env_vars.insert("CUDA_VISIBLE_DEVICES".to_string(), device_id.to_string());
                env_vars.insert("HIP_VISIBLE_DEVICES".to_string(), device_id.to_string());
                env_vars.insert("ROCR_VISIBLE_DEVICES".to_string(), device_id.to_string());
            }
        }
        RuntimeDeviceMode::Auto | RuntimeDeviceMode::Hybrid => {}
    }
}

fn provider_base_port(provider: RuntimeProviderId) -> u16 {
    match provider {
        RuntimeProviderId::Ollama => OLLAMA_RUNTIME_BASE_PORT,
        RuntimeProviderId::LlamaCpp => LLAMA_CPP_RUNTIME_BASE_PORT,
    }
}

fn provider_path_segment(provider: RuntimeProviderId) -> &'static str {
    match provider {
        RuntimeProviderId::Ollama => "ollama",
        RuntimeProviderId::LlamaCpp => "llama-cpp",
    }
}

fn load_or_initialize_config(path: &Path) -> Result<RuntimeProfilesConfigFile> {
    match atomic_read_json(path)? {
        Some(config) => Ok(config),
        None => {
            let config = RuntimeProfilesConfigFile::default_seed();
            atomic_write_json(path, &config, true)?;
            Ok(config)
        }
    }
}

fn bump_cursor(config: &mut RuntimeProfilesConfigFile) {
    let next = config
        .cursor
        .strip_prefix("runtime-profiles:")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .saturating_add(1);
    config.cursor = format!("runtime-profiles:{next}");
}

fn parse_runtime_profile_cursor(cursor: &str) -> Option<u64> {
    cursor
        .strip_prefix("runtime-profiles:")
        .and_then(|value| value.parse::<u64>().ok())
}

fn format_runtime_profile_cursor(cursor: u64) -> String {
    format!("runtime-profiles:{cursor}")
}

async fn validate_profile_config(profile: &RuntimeProfileConfig) -> Result<()> {
    if profile.name.trim().is_empty() {
        return Err(PumasError::InvalidParams {
            message: "runtime profile name is required".to_string(),
        });
    }

    validate_profile_provider_behavior(profile, &ProviderRegistry::builtin())?;

    match profile.provider {
        RuntimeProviderId::Ollama => OllamaRuntimeProviderAdapter.validate_profile(profile).await,
        RuntimeProviderId::LlamaCpp => {
            LlamaCppRuntimeProviderAdapter
                .validate_profile(profile)
                .await
        }
    }
}

fn validate_profile_provider_behavior(
    profile: &RuntimeProfileConfig,
    registry: &ProviderRegistry,
) -> Result<()> {
    let Some(behavior) = registry.get(profile.provider) else {
        return Err(PumasError::InvalidParams {
            message: "runtime profile provider is not registered".to_string(),
        });
    };

    if !behavior.supports_mode(profile.provider_mode) {
        return Err(PumasError::InvalidParams {
            message: "runtime profile provider does not support provider_mode".to_string(),
        });
    }

    match profile.management_mode {
        RuntimeManagementMode::Managed if !behavior.supports_managed_profiles => {
            Err(PumasError::InvalidParams {
                message: "runtime profile provider does not support managed profiles".to_string(),
            })
        }
        RuntimeManagementMode::External if !behavior.supports_external_profiles => {
            Err(PumasError::InvalidParams {
                message: "runtime profile provider does not support external profiles".to_string(),
            })
        }
        _ => Ok(()),
    }
}

fn validate_model_route(route: &ModelRuntimeRoute) -> Result<()> {
    if route.model_id.trim().is_empty() {
        return Err(PumasError::InvalidParams {
            message: "model_id is required".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RuntimeDeviceSettings, RuntimeSchedulerSettings};
    use std::time::Duration;

    #[test]
    fn provider_capabilities_separate_ollama_and_llama_cpp_modes() {
        let ollama = RuntimeProviderCapabilities::ollama();
        assert_eq!(ollama.provider, RuntimeProviderId::Ollama);
        assert_eq!(
            ollama.provider_modes,
            vec![RuntimeProviderMode::OllamaServe]
        );
        assert!(!ollama.supports_dedicated_model_processes);

        let llama_cpp = RuntimeProviderCapabilities::llama_cpp();
        assert_eq!(llama_cpp.provider, RuntimeProviderId::LlamaCpp);
        assert!(llama_cpp
            .provider_modes
            .contains(&RuntimeProviderMode::LlamaCppRouter));
        assert!(llama_cpp
            .provider_modes
            .contains(&RuntimeProviderMode::LlamaCppDedicated));
        assert!(llama_cpp.supports_dedicated_model_processes);
    }

    #[tokio::test]
    async fn ollama_provider_adapter_rejects_invalid_modes() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;

        let result = OllamaRuntimeProviderAdapter
            .validate_profile(&profile)
            .await;

        assert!(result.is_err());
    }

    fn managed_llama_cpp_profile(profile_id: &str) -> RuntimeProfileConfig {
        RuntimeProfileConfig {
            profile_id: RuntimeProfileId::parse(profile_id).unwrap(),
            provider: RuntimeProviderId::LlamaCpp,
            provider_mode: RuntimeProviderMode::LlamaCppRouter,
            management_mode: RuntimeManagementMode::Managed,
            name: "llama.cpp Router".to_string(),
            enabled: true,
            endpoint_url: RuntimeEndpointUrl::parse("http://127.0.0.1:18080").ok(),
            port: RuntimePort::parse(18080).ok(),
            device: RuntimeDeviceSettings::default(),
            scheduler: RuntimeSchedulerSettings::default(),
        }
    }

    #[tokio::test]
    async fn llama_cpp_provider_adapter_accepts_router_and_dedicated_modes() {
        let mut profile = managed_llama_cpp_profile("llama-router");
        LlamaCppRuntimeProviderAdapter
            .validate_profile(&profile)
            .await
            .unwrap();

        profile.profile_id = RuntimeProfileId::parse("llama-dedicated").unwrap();
        profile.provider_mode = RuntimeProviderMode::LlamaCppDedicated;
        LlamaCppRuntimeProviderAdapter
            .validate_profile(&profile)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn llama_cpp_provider_adapter_rejects_wrong_mode_and_external_missing_endpoint() {
        let mut profile = managed_llama_cpp_profile("llama-invalid");
        profile.provider_mode = RuntimeProviderMode::OllamaServe;
        let wrong_mode = LlamaCppRuntimeProviderAdapter
            .validate_profile(&profile)
            .await;
        assert!(wrong_mode
            .unwrap_err()
            .to_string()
            .contains("provider mode does not match"));

        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;
        profile.management_mode = RuntimeManagementMode::External;
        profile.endpoint_url = None;
        let missing_endpoint = LlamaCppRuntimeProviderAdapter
            .validate_profile(&profile)
            .await;
        assert!(missing_endpoint
            .unwrap_err()
            .to_string()
            .contains("endpoint_url"));
    }

    #[tokio::test]
    async fn validate_profile_config_accepts_builtin_provider_behavior() {
        validate_profile_config(&RuntimeProfileConfig::default_ollama())
            .await
            .unwrap();
        validate_profile_config(&managed_llama_cpp_profile("llama-router"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn validate_profile_config_rejects_mode_before_provider_adapter() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;

        let error = validate_profile_config(&profile).await.unwrap_err();

        assert!(error
            .to_string()
            .contains("provider does not support provider_mode"));
    }

    #[test]
    fn llama_cpp_router_catalog_sorts_and_writes_preset_entries() {
        let catalog = LlamaCppRouterCatalog::from_entries(vec![
            LlamaCppRouterCatalogEntry {
                model_id: "llm/zeta/model".to_string(),
                alias: "llm/zeta/model".to_string(),
                model_type: "llm".to_string(),
                model_path: PathBuf::from("/models/zeta.gguf"),
            },
            LlamaCppRouterCatalogEntry {
                model_id: "llm/alpha/model".to_string(),
                alias: "llm/alpha/model".to_string(),
                model_type: "llm".to_string(),
                model_path: PathBuf::from("/models/alpha.gguf"),
            },
            LlamaCppRouterCatalogEntry {
                model_id: "embedding/qwen/model".to_string(),
                alias: "embedding/qwen/model".to_string(),
                model_type: "embedding".to_string(),
                model_path: PathBuf::from("/models/embedding.gguf"),
            },
            LlamaCppRouterCatalogEntry {
                model_id: "reranker/qwen/model".to_string(),
                alias: "reranker/qwen/model".to_string(),
                model_type: "reranker".to_string(),
                model_path: PathBuf::from("/models/reranker.gguf"),
            },
        ]);

        assert_eq!(catalog.entries[0].model_id, "embedding/qwen/model");
        assert_eq!(catalog.entries[1].model_id, "llm/alpha/model");
        assert_eq!(catalog.entries[3].model_id, "reranker/qwen/model");
        assert!(catalog.preset_ini.contains("[*]\nload-on-startup = false"));
        assert!(
            catalog.preset_ini.find("[llm/alpha/model]").unwrap()
                < catalog.preset_ini.find("[llm/zeta/model]").unwrap()
        );
        assert!(catalog.preset_ini.contains("model = /models/alpha.gguf"));
        assert!(catalog.preset_ini.contains("alias = llm/zeta/model"));
        assert!(catalog.preset_ini.contains(
            "[embedding/qwen/model]\nmodel = /models/embedding.gguf\nalias = embedding/qwen/model\nembedding = true"
        ));
        assert!(catalog.preset_ini.contains(
            "[reranker/qwen/model]\nmodel = /models/reranker.gguf\nalias = reranker/qwen/model\nreranking = true"
        ));
    }

    #[tokio::test]
    async fn runtime_profile_service_seeds_and_persists_default_profile() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let snapshot = service.snapshot().await.unwrap();

        assert!(snapshot.success);
        assert_eq!(snapshot.snapshot.profiles.len(), 1);
        assert_eq!(
            snapshot
                .snapshot
                .default_profile_id
                .as_ref()
                .map(RuntimeProfileId::as_str),
            Some("ollama-default")
        );
        assert!(temp_dir
            .path()
            .join("launcher-data/metadata/runtime-profiles.json")
            .exists());
    }

    #[tokio::test]
    async fn runtime_profile_service_updates_routes_and_requires_known_profiles() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());
        let route = ModelRuntimeRoute {
            model_id: "llm/test/model".to_string(),
            profile_id: Some(RuntimeProfileId::parse("ollama-default").unwrap()),
            auto_load: true,
        };

        service.set_model_route(route).await.unwrap();
        let snapshot = service.snapshot().await.unwrap();

        assert_eq!(snapshot.snapshot.routes.len(), 1);
        assert_eq!(snapshot.snapshot.routes[0].model_id, "llm/test/model");

        let invalid_route = ModelRuntimeRoute {
            model_id: "llm/test/model".to_string(),
            profile_id: Some(RuntimeProfileId::parse("missing-profile").unwrap()),
            auto_load: true,
        };
        assert!(service.set_model_route(invalid_route).await.is_err());
    }

    #[tokio::test]
    async fn runtime_profile_service_resolves_model_route_endpoint() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.profile_id = RuntimeProfileId::parse("ollama-route").unwrap();
        profile.name = "Ollama Route".to_string();
        profile.endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:12557").ok();
        profile.port = RuntimePort::parse(12557).ok();
        service.upsert_profile(profile).await.unwrap();
        service
            .set_model_route(ModelRuntimeRoute {
                model_id: "llm/test/model".to_string(),
                profile_id: Some(RuntimeProfileId::parse("ollama-route").unwrap()),
                auto_load: true,
            })
            .await
            .unwrap();

        let routed_endpoint = service
            .resolve_model_endpoint(RuntimeProviderId::Ollama, "llm/test/model", None)
            .await
            .unwrap();
        assert_eq!(routed_endpoint.as_str(), "http://127.0.0.1:12557/");

        let explicit_endpoint = service
            .resolve_model_endpoint(
                RuntimeProviderId::Ollama,
                "llm/test/model",
                Some(RuntimeProfileId::parse("ollama-default").unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(explicit_endpoint.as_str(), "http://127.0.0.1:11434/");
    }

    #[tokio::test]
    async fn runtime_profile_service_reads_model_route_auto_load_policy() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        assert_eq!(
            service
                .model_route_auto_load("llm/test/model")
                .await
                .unwrap(),
            None
        );
        service
            .set_model_route(ModelRuntimeRoute {
                model_id: "llm/test/model".to_string(),
                profile_id: Some(RuntimeProfileId::parse("ollama-default").unwrap()),
                auto_load: false,
            })
            .await
            .unwrap();

        assert_eq!(
            service
                .model_route_auto_load("llm/test/model")
                .await
                .unwrap(),
            Some(false)
        );
    }

    #[tokio::test]
    async fn runtime_profile_service_rejects_stopped_model_operation_routes() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let stopped = service
            .resolve_model_endpoint_for_operation(
                RuntimeProviderId::Ollama,
                "llm/test/model",
                Some(RuntimeProfileId::parse("ollama-default").unwrap()),
            )
            .await;
        assert!(stopped.unwrap_err().to_string().contains("is not running"));

        service
            .record_profile_lifecycle_status(RuntimeProfileStatus {
                profile_id: RuntimeProfileId::parse("ollama-default").unwrap(),
                state: RuntimeLifecycleState::Running,
                endpoint_url: RuntimeEndpointUrl::parse("http://127.0.0.1:11434").ok(),
                pid: Some(1234),
                log_path: None,
                last_error: None,
            })
            .unwrap();
        let running = service
            .resolve_model_endpoint_for_operation(
                RuntimeProviderId::Ollama,
                "llm/test/model",
                Some(RuntimeProfileId::parse("ollama-default").unwrap()),
            )
            .await
            .unwrap();
        assert_eq!(running.as_str(), "http://127.0.0.1:11434/");
    }

    #[tokio::test]
    async fn runtime_profile_service_allows_external_model_operation_routes() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.profile_id = RuntimeProfileId::parse("ollama-external").unwrap();
        profile.name = "External Ollama".to_string();
        profile.management_mode = RuntimeManagementMode::External;
        profile.endpoint_url = RuntimeEndpointUrl::parse("http://192.0.2.10:11434").ok();
        profile.port = RuntimePort::parse(11434).ok();
        service.upsert_profile(profile).await.unwrap();

        let endpoint = service
            .resolve_model_endpoint_for_operation(
                RuntimeProviderId::Ollama,
                "llm/test/model",
                Some(RuntimeProfileId::parse("ollama-external").unwrap()),
            )
            .await
            .unwrap();

        assert_eq!(endpoint.as_str(), "http://192.0.2.10:11434/");
    }

    #[tokio::test]
    async fn runtime_profile_service_resolves_default_ollama_endpoint() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let endpoint = service
            .resolve_profile_endpoint(RuntimeProviderId::Ollama, None)
            .await
            .unwrap();

        assert_eq!(endpoint.as_str(), "http://127.0.0.1:11434/");
    }

    #[tokio::test]
    async fn runtime_profile_service_derives_managed_launch_specs() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let mut gpu_profile = RuntimeProfileConfig::default_ollama();
        gpu_profile.profile_id = RuntimeProfileId::parse("ollama-gpu").unwrap();
        gpu_profile.name = "Ollama GPU".to_string();
        gpu_profile.endpoint_url = None;
        gpu_profile.port = None;
        gpu_profile.device.mode = RuntimeDeviceMode::Gpu;
        gpu_profile.device.device_id = Some("1".to_string());
        service.upsert_profile(gpu_profile).await.unwrap();

        let specs = service.list_managed_profile_launch_specs().await.unwrap();
        assert_eq!(specs.len(), 2);

        let default_spec = specs
            .iter()
            .find(|spec| spec.profile_id.as_str() == "ollama-default")
            .unwrap();
        assert_eq!(default_spec.port.value(), 11434);
        assert_eq!(
            default_spec.endpoint_url.as_str(),
            "http://127.0.0.1:11434/"
        );
        assert_eq!(
            default_spec.env_vars.get("OLLAMA_HOST").map(String::as_str),
            Some("127.0.0.1:11434")
        );
        assert!(default_spec
            .pid_file
            .ends_with("runtime-profiles/ollama/ollama-default/runtime.pid"));
        assert!(default_spec
            .log_file
            .ends_with("runtime-profiles/ollama/ollama-default/runtime.log"));

        let gpu_spec = specs
            .iter()
            .find(|spec| spec.profile_id.as_str() == "ollama-gpu")
            .unwrap();
        assert_ne!(gpu_spec.port.value(), 11434);
        assert_eq!(
            gpu_spec.endpoint_url.as_str(),
            format!("http://127.0.0.1:{}/", gpu_spec.port.value())
        );
        assert_eq!(
            gpu_spec
                .env_vars
                .get("CUDA_VISIBLE_DEVICES")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(
            gpu_spec
                .env_vars
                .get("PUMAS_RUNTIME_PROFILE_ID")
                .map(String::as_str),
            Some("ollama-gpu")
        );
    }

    #[tokio::test]
    async fn runtime_profile_service_rejects_managed_port_collisions() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let mut duplicate_port_profile = RuntimeProfileConfig::default_ollama();
        duplicate_port_profile.profile_id = RuntimeProfileId::parse("ollama-duplicate").unwrap();
        duplicate_port_profile.name = "Ollama Duplicate".to_string();

        let result = service.upsert_profile(duplicate_port_profile).await;
        let error = result.unwrap_err().to_string();
        assert!(error.contains("runtime profile port collision: 11434"));
        assert!(error.contains("ollama-default"));
        assert!(error.contains("leave the port blank"));
    }

    #[tokio::test]
    async fn runtime_profile_service_derives_cpu_visibility_env() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let mut cpu_profile = RuntimeProfileConfig::default_ollama();
        cpu_profile.profile_id = RuntimeProfileId::parse("ollama-cpu").unwrap();
        cpu_profile.name = "Ollama CPU".to_string();
        cpu_profile.endpoint_url = None;
        cpu_profile.port = None;
        cpu_profile.device.mode = RuntimeDeviceMode::Cpu;
        service.upsert_profile(cpu_profile).await.unwrap();

        let specs = service.list_managed_profile_launch_specs().await.unwrap();
        let cpu_spec = specs
            .iter()
            .find(|spec| spec.profile_id.as_str() == "ollama-cpu")
            .unwrap();

        assert_eq!(
            cpu_spec
                .env_vars
                .get("CUDA_VISIBLE_DEVICES")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            cpu_spec
                .env_vars
                .get("HIP_VISIBLE_DEVICES")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            cpu_spec
                .env_vars
                .get("ROCR_VISIBLE_DEVICES")
                .map(String::as_str),
            Some("")
        );
    }

    #[tokio::test]
    async fn runtime_profile_service_derives_llama_cpp_router_launch_specs() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let mut profile = managed_llama_cpp_profile("llama-router-gpu");
        profile.name = "llama.cpp GPU Router".to_string();
        profile.endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:18080").ok();
        profile.port = RuntimePort::parse(18080).ok();
        profile.device.mode = RuntimeDeviceMode::Gpu;
        profile.device.device_id = Some("1".to_string());
        profile.device.gpu_layers = Some(35);
        profile.device.tensor_split = Some(vec![3.0, 1.0]);
        service.upsert_profile(profile).await.unwrap();

        let specs = service.list_managed_profile_launch_specs().await.unwrap();
        let spec = specs
            .iter()
            .find(|spec| spec.profile_id.as_str() == "llama-router-gpu")
            .unwrap();

        assert_eq!(spec.provider, RuntimeProviderId::LlamaCpp);
        assert_eq!(spec.port.value(), 18080);
        assert!(spec
            .runtime_dir
            .ends_with("runtime-profiles/llama-cpp/llama-router-gpu"));
        assert_eq!(
            spec.extra_args,
            vec![
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                "18080".to_string(),
                "--models-dir".to_string(),
                temp_dir
                    .path()
                    .join("shared-resources")
                    .join("models")
                    .to_string_lossy()
                    .to_string(),
                "--n-gpu-layers".to_string(),
                "35".to_string(),
                "--tensor-split".to_string(),
                "3,1".to_string(),
            ]
        );
        assert_eq!(
            spec.env_vars
                .get("CUDA_VISIBLE_DEVICES")
                .map(String::as_str),
            Some("1")
        );
    }

    #[tokio::test]
    async fn runtime_profile_service_defaults_llama_cpp_gpu_to_full_offload() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let mut profile = managed_llama_cpp_profile("llama-router-gpu-default");
        profile.name = "llama.cpp GPU Default".to_string();
        profile.endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:18081").ok();
        profile.port = RuntimePort::parse(18081).ok();
        profile.device.mode = RuntimeDeviceMode::Gpu;
        profile.device.gpu_layers = None;
        service.upsert_profile(profile).await.unwrap();

        let specs = service.list_managed_profile_launch_specs().await.unwrap();
        let spec = specs
            .iter()
            .find(|spec| spec.profile_id.as_str() == "llama-router-gpu-default")
            .unwrap();

        assert!(spec
            .extra_args
            .windows(2)
            .any(|window| window == ["--n-gpu-layers", "-1"]));
    }

    #[tokio::test]
    async fn runtime_profile_service_resolves_implicit_managed_llama_cpp_endpoint() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());

        let mut profile = managed_llama_cpp_profile("llama-router-auto-port");
        profile.endpoint_url = None;
        profile.port = None;
        service.upsert_profile(profile).await.unwrap();

        let spec = service
            .list_managed_profile_launch_specs()
            .await
            .unwrap()
            .into_iter()
            .find(|spec| spec.profile_id.as_str() == "llama-router-auto-port")
            .unwrap();
        let endpoint = service
            .resolve_model_endpoint(
                RuntimeProviderId::LlamaCpp,
                "embedding/qwen/model",
                Some(RuntimeProfileId::parse("llama-router-auto-port").unwrap()),
            )
            .await
            .unwrap();

        assert_eq!(endpoint, spec.endpoint_url);
    }

    #[test]
    fn runtime_profile_service_serializes_profile_operations() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());
        let profile_id = RuntimeProfileId::parse("ollama-default").unwrap();

        let guard = service
            .begin_profile_operation(profile_id.clone())
            .expect("first profile operation should start");
        let overlapping = service.begin_profile_operation(profile_id.clone());
        assert!(overlapping.is_err());

        drop(guard);
        let next = service.begin_profile_operation(profile_id);
        assert!(next.is_ok());
    }

    #[tokio::test]
    async fn runtime_profile_status_changes_emit_update_events() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());
        let mut updates = service.subscribe_updates();
        let snapshot = service.snapshot().await.unwrap();
        let cursor = snapshot.snapshot.cursor.clone();

        let stopped_event = service.record_default_ollama_status(false).await.unwrap();
        assert!(stopped_event.is_none());

        let running_event = service.record_default_ollama_status(true).await.unwrap();
        assert_eq!(
            running_event.as_ref().map(|event| event.event_kind),
            Some(RuntimeProfileEventKind::StatusChanged)
        );
        let pushed_feed = tokio::time::timeout(Duration::from_secs(1), updates.recv())
            .await
            .expect("runtime profile update should be pushed")
            .expect("runtime profile update channel should remain open");
        assert_eq!(pushed_feed.events.len(), 1);
        assert_eq!(
            pushed_feed.events[0].event_kind,
            RuntimeProfileEventKind::StatusChanged
        );

        let feed = service
            .list_updates_since(Some(cursor.as_str()))
            .await
            .unwrap();
        assert!(feed.success);
        assert_eq!(feed.feed.events.len(), 1);
        assert_eq!(
            feed.feed.events[0]
                .profile_id
                .as_ref()
                .map(RuntimeProfileId::as_str),
            Some("ollama-default")
        );
        assert!(!feed.feed.snapshot_required);

        let snapshot = service.snapshot().await.unwrap();
        assert_eq!(
            snapshot.snapshot.statuses[0].state,
            RuntimeLifecycleState::Running
        );
        assert_eq!(snapshot.snapshot.cursor, feed.feed.cursor);
    }

    #[tokio::test]
    async fn runtime_profile_config_mutations_push_snapshot_required_update() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());
        let mut updates = service.subscribe_updates();
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.profile_id = RuntimeProfileId::parse("ollama-extra").unwrap();
        profile.name = "Ollama Extra".to_string();
        profile.endpoint_url = None;
        profile.port = None;

        service.upsert_profile(profile).await.unwrap();

        let pushed_feed = tokio::time::timeout(Duration::from_secs(1), updates.recv())
            .await
            .expect("runtime profile mutation should push an update")
            .expect("runtime profile update channel should remain open");
        assert!(pushed_feed.snapshot_required);
        assert!(pushed_feed.stale_cursor);
        assert!(pushed_feed.events.is_empty());
    }
}
