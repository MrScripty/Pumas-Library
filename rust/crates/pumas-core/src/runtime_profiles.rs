//! Provider-neutral runtime profile service contracts.

#[path = "runtime_profiles/launch_specs.rs"]
mod launch_specs;
#[path = "runtime_profiles/launch_strategy.rs"]
mod launch_strategy;
#[path = "runtime_profiles/route_config.rs"]
mod route_config;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use crate::index::ModelRecord;
use crate::metadata::atomic_write_json;
use crate::model_library::ModelLibrary;
use crate::models::{
    ModelRuntimeRoute, RuntimeDeviceMode, RuntimeDeviceSettings, RuntimeEndpointUrl,
    RuntimeLifecycleState, RuntimeManagementMode, RuntimePort, RuntimeProfileConfig,
    RuntimeProfileEvent, RuntimeProfileEventKind, RuntimeProfileId, RuntimeProfileMutationResponse,
    RuntimeProfileStatus, RuntimeProfileUpdateFeed, RuntimeProfileUpdateFeedResponse,
    RuntimeProfilesConfigFile, RuntimeProfilesSnapshot, RuntimeProfilesSnapshotResponse,
    RuntimeProviderId, RuntimeProviderMode,
};
use crate::providers::{ExecutableArtifactFormat, ProviderBehavior, ProviderRegistry};
use crate::{PumasError, Result};
use tokio::sync::broadcast;

use launch_specs::derive_managed_profile_launch_specs;
pub use launch_strategy::{
    RuntimeProfileBinaryLaunchKind, RuntimeProfileInProcessRuntimeKind,
    RuntimeProfileLaunchStrategy,
};
use route_config::{load_or_initialize_config, validate_model_route};

const RUNTIME_PROFILE_EVENT_RETAIN_LIMIT: usize = 256;
const RUNTIME_PROFILE_UPDATE_CHANNEL_CAPACITY: usize = 64;

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
    pub fn from_behavior(behavior: &ProviderBehavior) -> Self {
        Self {
            provider: behavior.provider,
            provider_modes: behavior.provider_modes.clone(),
            device_modes: behavior.device_modes.clone(),
            supports_managed_profiles: behavior.supports_managed_profiles,
            supports_external_profiles: behavior.supports_external_profiles,
            supports_model_catalog: behavior.supports_model_catalog,
            supports_dedicated_model_processes: behavior.supports_dedicated_model_processes,
        }
    }

    pub fn ollama() -> Self {
        Self::from_behavior(&ProviderBehavior::ollama())
    }

    pub fn llama_cpp() -> Self {
        Self::from_behavior(&ProviderBehavior::llama_cpp())
    }
}

#[async_trait]
pub trait RuntimeProviderAdapter: Send + Sync {
    fn provider(&self) -> RuntimeProviderId;
    fn capabilities(&self) -> RuntimeProviderCapabilities;
    async fn validate_profile(&self, profile: &RuntimeProfileConfig) -> Result<()>;
}

#[derive(Clone)]
pub struct RuntimeProviderAdapters {
    adapters: Arc<HashMap<RuntimeProviderId, Arc<dyn RuntimeProviderAdapter>>>,
}

impl RuntimeProviderAdapters {
    pub fn builtin() -> Self {
        Self::from_adapters(vec![
            Arc::new(OllamaRuntimeProviderAdapter) as Arc<dyn RuntimeProviderAdapter>,
            Arc::new(LlamaCppRuntimeProviderAdapter) as Arc<dyn RuntimeProviderAdapter>,
            Arc::new(OnnxRuntimeProviderAdapter) as Arc<dyn RuntimeProviderAdapter>,
        ])
    }

    pub fn from_adapters(
        adapters: impl IntoIterator<Item = Arc<dyn RuntimeProviderAdapter>>,
    ) -> Self {
        let adapters = adapters
            .into_iter()
            .map(|adapter| (adapter.provider(), adapter))
            .collect();
        Self {
            adapters: Arc::new(adapters),
        }
    }

    async fn validate_profile(&self, profile: &RuntimeProfileConfig) -> Result<()> {
        let Some(adapter) = self.adapters.get(&profile.provider) else {
            return Err(PumasError::InvalidParams {
                message: "runtime profile provider adapter is not registered".to_string(),
            });
        };
        adapter.validate_profile(profile).await
    }
}

impl fmt::Debug for RuntimeProviderAdapters {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProviderAdapters")
            .field("providers", &self.adapters.keys().collect::<Vec<_>>())
            .finish()
    }
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
    let (format, model_path) = executable_artifact_for_record(library, record)?;
    if format != ExecutableArtifactFormat::Gguf {
        return None;
    }

    Some(LlamaCppRouterCatalogEntry {
        model_id: record.id.clone(),
        alias: record.id.clone(),
        model_type: record.model_type.clone(),
        model_path,
    })
}

fn executable_artifact_for_record(
    library: &ModelLibrary,
    record: &ModelRecord,
) -> Option<(ExecutableArtifactFormat, PathBuf)> {
    let model_path = library.get_primary_model_file(&record.id)?;
    let format = ExecutableArtifactFormat::from_path(&model_path)?;
    Some((format, model_path))
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

pub struct OnnxRuntimeProviderAdapter;

#[async_trait]
impl RuntimeProviderAdapter for OnnxRuntimeProviderAdapter {
    fn provider(&self) -> RuntimeProviderId {
        RuntimeProviderId::OnnxRuntime
    }

    fn capabilities(&self) -> RuntimeProviderCapabilities {
        RuntimeProviderCapabilities::from_behavior(&ProviderBehavior::onnx_runtime())
    }

    async fn validate_profile(&self, profile: &RuntimeProfileConfig) -> Result<()> {
        if profile.provider != RuntimeProviderId::OnnxRuntime {
            return Err(PumasError::InvalidParams {
                message: "ONNX Runtime adapter received a non-ONNX Runtime profile".to_string(),
            });
        }
        if profile.provider_mode != RuntimeProviderMode::OnnxServe {
            return Err(PumasError::InvalidParams {
                message: "ONNX Runtime profiles must use provider_mode=onnx_serve".to_string(),
            });
        }
        if profile.management_mode != RuntimeManagementMode::Managed {
            return Err(PumasError::InvalidParams {
                message: "ONNX Runtime profiles must use managed in-process lifecycle".to_string(),
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
    provider_registry: ProviderRegistry,
    provider_adapters: RuntimeProviderAdapters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProfileLaunchSpec {
    pub profile_id: RuntimeProfileId,
    pub provider: RuntimeProviderId,
    pub provider_mode: RuntimeProviderMode,
    pub launch_strategy: RuntimeProfileLaunchStrategy,
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
    pub fn with_provider_registry_and_adapters(
        launcher_root: impl AsRef<Path>,
        provider_registry: ProviderRegistry,
        provider_adapters: RuntimeProviderAdapters,
    ) -> Self {
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
            provider_registry,
            provider_adapters,
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
        let provider_registry = self.provider_registry.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let config = load_or_initialize_config(&config_path)?;
            derive_managed_profile_launch_specs(&launcher_root, &config, &provider_registry)
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
        validate_profile_config(&profile, &self.provider_registry, &self.provider_adapters).await?;
        let profile_id = profile.profile_id.clone();
        let launcher_root = self.launcher_root.clone();
        let provider_registry = self.provider_registry.clone();
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
            derive_managed_profile_launch_specs(&launcher_root, config, &provider_registry)?;
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
            if let Some(existing) = config.routes.iter_mut().find(|existing| {
                existing.provider == route.provider && existing.model_id == route.model_id
            }) {
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
        provider: RuntimeProviderId,
        model_id: String,
    ) -> Result<RuntimeProfileMutationResponse> {
        let model_id = model_id.trim().to_string();
        if model_id.is_empty() {
            return Err(PumasError::InvalidParams {
                message: "model_id is required".to_string(),
            });
        }
        self.mutate_config(move |config| {
            config
                .routes
                .retain(|route| !(route.provider == provider && route.model_id == model_id));
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
        let provider_registry = self.provider_registry.clone();
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
            resolve_config_profile_endpoint(
                &launcher_root,
                &config,
                &provider_registry,
                provider,
                selected_profile_id,
            )
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
        let provider_registry = self.provider_registry.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let config = load_or_initialize_config(&config_path)?;
            let supports_default_profile_fallback = provider_registry
                .get(provider)
                .map(|behavior| behavior.supports_default_profile_fallback)
                .unwrap_or(false);
            let routed_profile_id = explicit_profile_id
                .or_else(|| {
                    config
                        .routes
                        .iter()
                        .find(|route| route.provider == provider && route.model_id == model_id)
                        .and_then(|route| route.profile_id.clone())
                })
                .or_else(|| {
                    supports_default_profile_fallback
                        .then(|| config.default_profile_id.clone())
                        .flatten()
                })
                .ok_or_else(|| PumasError::InvalidParams {
                    message: "runtime profile id is required".to_string(),
                })?;
            resolve_config_profile_endpoint(
                &launcher_root,
                &config,
                &provider_registry,
                provider,
                routed_profile_id,
            )
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join model runtime profile endpoint resolution task: {err}"
            ))
        })?
    }

    pub async fn model_route_auto_load(
        &self,
        provider: RuntimeProviderId,
        model_id: &str,
    ) -> Result<Option<bool>> {
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
                .find(|route| route.provider == provider && route.model_id == model_id)
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
            provider: None,
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

fn resolve_config_profile_endpoint(
    launcher_root: &Path,
    config: &RuntimeProfilesConfigFile,
    provider_registry: &ProviderRegistry,
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
            derive_managed_profile_launch_specs(launcher_root, config, provider_registry)?
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

async fn validate_profile_config(
    profile: &RuntimeProfileConfig,
    provider_registry: &ProviderRegistry,
    provider_adapters: &RuntimeProviderAdapters,
) -> Result<()> {
    if profile.name.trim().is_empty() {
        return Err(PumasError::InvalidParams {
            message: "runtime profile name is required".to_string(),
        });
    }

    validate_profile_provider_behavior(profile, provider_registry)?;
    provider_adapters.validate_profile(profile).await
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

    if !behavior.supports_management_mode(profile.management_mode) {
        return Err(PumasError::InvalidParams {
            message: "runtime profile provider does not support management mode".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        RuntimeDeviceSettings, RuntimeSchedulerSettings, RUNTIME_PROFILES_SCHEMA_VERSION,
    };
    use serde_json::Value;
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

    #[test]
    fn provider_capabilities_project_from_provider_behavior() {
        let registry = ProviderRegistry::builtin();
        let behavior = registry.get(RuntimeProviderId::LlamaCpp).unwrap();
        let capabilities = RuntimeProviderCapabilities::from_behavior(behavior);

        assert_eq!(capabilities.provider, behavior.provider);
        assert_eq!(capabilities.provider_modes, behavior.provider_modes);
        assert_eq!(capabilities.device_modes, behavior.device_modes);
        assert_eq!(
            capabilities.supports_dedicated_model_processes,
            behavior.supports_dedicated_model_processes
        );
    }

    #[test]
    fn provider_capabilities_contract_serializes_from_behavior() {
        let capabilities =
            RuntimeProviderCapabilities::from_behavior(&ProviderBehavior::llama_cpp());
        let encoded = serde_json::to_value(&capabilities).unwrap();

        assert_eq!(encoded["provider"], "llama_cpp");
        assert_eq!(encoded["provider_modes"][0], "llama_cpp_router");
        assert_eq!(encoded["device_modes"][0], "auto");
        assert_eq!(encoded["supports_managed_profiles"], true);
        assert_eq!(encoded["supports_external_profiles"], true);
        assert_eq!(encoded["supports_model_catalog"], true);
        assert_eq!(encoded["supports_dedicated_model_processes"], true);

        let decoded: RuntimeProviderCapabilities = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, capabilities);
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

    fn managed_onnx_runtime_profile(profile_id: &str) -> RuntimeProfileConfig {
        RuntimeProfileConfig {
            profile_id: RuntimeProfileId::parse(profile_id).unwrap(),
            provider: RuntimeProviderId::OnnxRuntime,
            provider_mode: RuntimeProviderMode::OnnxServe,
            management_mode: RuntimeManagementMode::Managed,
            name: "ONNX Runtime".to_string(),
            enabled: true,
            endpoint_url: None,
            port: None,
            device: RuntimeDeviceSettings {
                mode: RuntimeDeviceMode::Cpu,
                ..RuntimeDeviceSettings::default()
            },
            scheduler: RuntimeSchedulerSettings::default(),
        }
    }

    async fn validate_builtin_profile_config(profile: &RuntimeProfileConfig) -> Result<()> {
        validate_profile_config(
            profile,
            &ProviderRegistry::builtin(),
            &RuntimeProviderAdapters::builtin(),
        )
        .await
    }

    fn runtime_profile_service(launcher_root: impl AsRef<Path>) -> RuntimeProfileService {
        RuntimeProfileService::with_provider_registry_and_adapters(
            launcher_root,
            ProviderRegistry::builtin(),
            RuntimeProviderAdapters::builtin(),
        )
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
    async fn onnx_runtime_provider_adapter_accepts_managed_in_process_profile() {
        OnnxRuntimeProviderAdapter
            .validate_profile(&managed_onnx_runtime_profile("onnx-managed"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn onnx_runtime_provider_adapter_rejects_external_or_wrong_modes() {
        let mut profile = managed_onnx_runtime_profile("onnx-invalid");
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;
        let wrong_mode = OnnxRuntimeProviderAdapter.validate_profile(&profile).await;
        assert!(wrong_mode
            .unwrap_err()
            .to_string()
            .contains("provider_mode=onnx_serve"));

        profile.provider_mode = RuntimeProviderMode::OnnxServe;
        profile.management_mode = RuntimeManagementMode::External;
        let external = OnnxRuntimeProviderAdapter.validate_profile(&profile).await;
        assert!(external
            .unwrap_err()
            .to_string()
            .contains("managed in-process"));
    }

    #[tokio::test]
    async fn validate_profile_config_accepts_builtin_provider_behavior() {
        validate_builtin_profile_config(&RuntimeProfileConfig::default_ollama())
            .await
            .unwrap();
        validate_builtin_profile_config(&managed_llama_cpp_profile("llama-router"))
            .await
            .unwrap();
        validate_builtin_profile_config(&managed_onnx_runtime_profile("onnx-managed"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn validate_profile_config_rejects_mode_before_provider_adapter() {
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;

        let error = validate_builtin_profile_config(&profile).await.unwrap_err();

        assert!(error
            .to_string()
            .contains("provider does not support provider_mode"));
    }

    #[tokio::test]
    async fn runtime_profile_service_validation_uses_composed_provider_registry() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = RuntimeProfileService::with_provider_registry_and_adapters(
            temp_dir.path(),
            ProviderRegistry::from_behaviors([ProviderBehavior::ollama()]),
            RuntimeProviderAdapters::builtin(),
        );

        let error = service
            .upsert_profile(managed_llama_cpp_profile("llama-router"))
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("runtime profile provider is not registered"));
    }

    #[tokio::test]
    async fn runtime_profile_launch_specs_require_composed_provider_behavior() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir
            .path()
            .join("launcher-data/metadata/runtime-profiles.json");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let mut llama_profile = managed_llama_cpp_profile("llama-router");
        llama_profile.endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:18181").ok();
        llama_profile.port = RuntimePort::parse(18181).ok();
        let config = serde_json::json!({
            "schema_version": RUNTIME_PROFILES_SCHEMA_VERSION,
            "cursor": "runtime-profiles:1",
            "profiles": [llama_profile],
            "routes": [],
            "default_profile_id": null
        });
        std::fs::write(&config_path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        let service = RuntimeProfileService::with_provider_registry_and_adapters(
            temp_dir.path(),
            ProviderRegistry::from_behaviors([ProviderBehavior::ollama()]),
            RuntimeProviderAdapters::builtin(),
        );

        let error = service
            .list_managed_profile_launch_specs()
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("runtime profile provider is not registered"));
    }

    #[tokio::test]
    async fn runtime_profile_service_validation_uses_composed_provider_adapters() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = RuntimeProfileService::with_provider_registry_and_adapters(
            temp_dir.path(),
            ProviderRegistry::builtin(),
            RuntimeProviderAdapters::from_adapters(vec![
                Arc::new(OllamaRuntimeProviderAdapter) as Arc<dyn RuntimeProviderAdapter>
            ]),
        );

        let error = service
            .upsert_profile(managed_llama_cpp_profile("llama-router"))
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("runtime profile provider adapter is not registered"));
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
        let service = runtime_profile_service(temp_dir.path());

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
        let service = runtime_profile_service(temp_dir.path());
        let route = ModelRuntimeRoute {
            provider: RuntimeProviderId::Ollama,
            model_id: "llm/test/model".to_string(),
            profile_id: Some(RuntimeProfileId::parse("ollama-default").unwrap()),
            auto_load: true,
        };

        service.set_model_route(route).await.unwrap();
        let snapshot = service.snapshot().await.unwrap();

        assert_eq!(snapshot.snapshot.routes.len(), 1);
        assert_eq!(
            snapshot.snapshot.routes[0].provider,
            RuntimeProviderId::Ollama
        );
        assert_eq!(snapshot.snapshot.routes[0].model_id, "llm/test/model");

        let invalid_route = ModelRuntimeRoute {
            provider: RuntimeProviderId::Ollama,
            model_id: "llm/test/model".to_string(),
            profile_id: Some(RuntimeProfileId::parse("missing-profile").unwrap()),
            auto_load: true,
        };
        assert!(service.set_model_route(invalid_route).await.is_err());
    }

    #[tokio::test]
    async fn runtime_profile_service_resolves_model_route_endpoint() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = runtime_profile_service(temp_dir.path());
        let mut profile = RuntimeProfileConfig::default_ollama();
        profile.profile_id = RuntimeProfileId::parse("ollama-route").unwrap();
        profile.name = "Ollama Route".to_string();
        profile.endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:12557").ok();
        profile.port = RuntimePort::parse(12557).ok();
        service.upsert_profile(profile).await.unwrap();
        service
            .set_model_route(ModelRuntimeRoute {
                provider: RuntimeProviderId::Ollama,
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
    async fn runtime_profile_service_does_not_default_onnx_model_endpoint_to_global_profile() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = runtime_profile_service(temp_dir.path());

        let ollama_endpoint = service
            .resolve_model_endpoint(RuntimeProviderId::Ollama, "unrouted/model", None)
            .await
            .unwrap();
        assert_eq!(ollama_endpoint.as_str(), "http://127.0.0.1:11434/");

        let onnx_error = service
            .resolve_model_endpoint(RuntimeProviderId::OnnxRuntime, "unrouted/model", None)
            .await
            .unwrap_err()
            .to_string();

        assert!(onnx_error.contains("runtime profile id is required"));
    }

    #[tokio::test]
    async fn runtime_profile_service_routes_same_model_id_by_provider() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = runtime_profile_service(temp_dir.path());

        let mut ollama = RuntimeProfileConfig::default_ollama();
        ollama.profile_id = RuntimeProfileId::parse("ollama-route").unwrap();
        ollama.name = "Ollama Route".to_string();
        ollama.endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:12557").ok();
        ollama.port = RuntimePort::parse(12557).ok();
        service.upsert_profile(ollama).await.unwrap();

        let mut llama = managed_llama_cpp_profile("llama-route");
        llama.endpoint_url = RuntimeEndpointUrl::parse("http://127.0.0.1:18088").ok();
        llama.port = RuntimePort::parse(18088).ok();
        service.upsert_profile(llama).await.unwrap();

        service
            .set_model_route(ModelRuntimeRoute {
                provider: RuntimeProviderId::Ollama,
                model_id: "shared/model".to_string(),
                profile_id: Some(RuntimeProfileId::parse("ollama-route").unwrap()),
                auto_load: true,
            })
            .await
            .unwrap();
        service
            .set_model_route(ModelRuntimeRoute {
                provider: RuntimeProviderId::LlamaCpp,
                model_id: "shared/model".to_string(),
                profile_id: Some(RuntimeProfileId::parse("llama-route").unwrap()),
                auto_load: false,
            })
            .await
            .unwrap();

        let ollama_endpoint = service
            .resolve_model_endpoint(RuntimeProviderId::Ollama, "shared/model", None)
            .await
            .unwrap();
        let llama_endpoint = service
            .resolve_model_endpoint(RuntimeProviderId::LlamaCpp, "shared/model", None)
            .await
            .unwrap();

        assert_eq!(ollama_endpoint.as_str(), "http://127.0.0.1:12557/");
        assert_eq!(llama_endpoint.as_str(), "http://127.0.0.1:18088/");
        assert_eq!(
            service
                .model_route_auto_load(RuntimeProviderId::Ollama, "shared/model")
                .await
                .unwrap(),
            Some(true)
        );
        assert_eq!(
            service
                .model_route_auto_load(RuntimeProviderId::LlamaCpp, "shared/model")
                .await
                .unwrap(),
            Some(false)
        );
    }

    #[tokio::test]
    async fn runtime_profile_service_reads_model_route_auto_load_policy() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = runtime_profile_service(temp_dir.path());

        assert_eq!(
            service
                .model_route_auto_load(RuntimeProviderId::Ollama, "llm/test/model")
                .await
                .unwrap(),
            None
        );
        service
            .set_model_route(ModelRuntimeRoute {
                provider: RuntimeProviderId::Ollama,
                model_id: "llm/test/model".to_string(),
                profile_id: Some(RuntimeProfileId::parse("ollama-default").unwrap()),
                auto_load: false,
            })
            .await
            .unwrap();

        assert_eq!(
            service
                .model_route_auto_load(RuntimeProviderId::Ollama, "llm/test/model")
                .await
                .unwrap(),
            Some(false)
        );
    }

    #[tokio::test]
    async fn runtime_profile_service_migrates_legacy_routes_to_provider_scope() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config_path = temp_dir
            .path()
            .join("launcher-data/metadata/runtime-profiles.json");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let legacy_config = serde_json::json!({
            "schema_version": 1,
            "cursor": "runtime-profiles:7",
            "profiles": [RuntimeProfileConfig::default_ollama()],
            "routes": [
                {
                    "model_id": "llm/test/model",
                    "profile_id": "ollama-default",
                    "auto_load": false
                },
                {
                    "model_id": "ambiguous/model",
                    "auto_load": true
                }
            ],
            "default_profile_id": "ollama-default"
        });
        std::fs::write(
            &config_path,
            serde_json::to_vec_pretty(&legacy_config).unwrap(),
        )
        .unwrap();

        let service = runtime_profile_service(temp_dir.path());
        let snapshot = service.snapshot().await.unwrap();

        assert_eq!(
            snapshot.snapshot.schema_version,
            RUNTIME_PROFILES_SCHEMA_VERSION
        );
        assert_eq!(snapshot.snapshot.routes.len(), 1);
        assert_eq!(
            snapshot.snapshot.routes[0].provider,
            RuntimeProviderId::Ollama
        );
        assert_eq!(snapshot.snapshot.routes[0].model_id, "llm/test/model");
        assert_eq!(snapshot.snapshot.routes[0].auto_load, false);

        let persisted: Value =
            serde_json::from_slice(&std::fs::read(config_path).unwrap()).unwrap();
        assert_eq!(persisted["schema_version"], RUNTIME_PROFILES_SCHEMA_VERSION);
        assert_eq!(persisted["routes"][0]["provider"], "ollama");
        assert_eq!(persisted["routes"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn runtime_profile_service_rejects_stopped_model_operation_routes() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = runtime_profile_service(temp_dir.path());

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
        let service = runtime_profile_service(temp_dir.path());
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
        let service = runtime_profile_service(temp_dir.path());

        let endpoint = service
            .resolve_profile_endpoint(RuntimeProviderId::Ollama, None)
            .await
            .unwrap();

        assert_eq!(endpoint.as_str(), "http://127.0.0.1:11434/");
    }

    #[tokio::test]
    async fn runtime_profile_service_derives_managed_launch_specs() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = runtime_profile_service(temp_dir.path());

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
            default_spec.launch_strategy,
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::OllamaServe
            )
        );
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
        let service = runtime_profile_service(temp_dir.path());

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
        let service = runtime_profile_service(temp_dir.path());

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
        let service = runtime_profile_service(temp_dir.path());

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
        assert_eq!(
            spec.launch_strategy,
            RuntimeProfileLaunchStrategy::BinaryProcess(
                RuntimeProfileBinaryLaunchKind::LlamaCppRouter
            )
        );
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
        let service = runtime_profile_service(temp_dir.path());

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
        let service = runtime_profile_service(temp_dir.path());

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
        let service = runtime_profile_service(temp_dir.path());
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
        let service = runtime_profile_service(temp_dir.path());
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
        let service = runtime_profile_service(temp_dir.path());
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
