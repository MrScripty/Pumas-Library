//! Provider-neutral runtime profile service contracts.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::models::{
    ModelRuntimeRoute, RuntimeDeviceMode, RuntimeEndpointUrl, RuntimeLifecycleState,
    RuntimeManagementMode, RuntimeProfileConfig, RuntimeProfileEvent, RuntimeProfileEventKind,
    RuntimeProfileId, RuntimeProfileMutationResponse, RuntimeProfileStatus,
    RuntimeProfileUpdateFeed, RuntimeProfileUpdateFeedResponse, RuntimeProfilesConfigFile,
    RuntimeProfilesSnapshot, RuntimeProfilesSnapshotResponse, RuntimeProviderId,
    RuntimeProviderMode,
};
use crate::{PumasError, Result};

const RUNTIME_PROFILE_EVENT_RETAIN_LIMIT: usize = 256;

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

#[derive(Debug, Clone)]
pub struct RuntimeProfileService {
    config_path: PathBuf,
    write_lock: Arc<RwLock<()>>,
    event_journal: Arc<RwLock<RuntimeProfileEventJournal>>,
}

impl RuntimeProfileService {
    pub fn new(launcher_root: impl AsRef<Path>) -> Self {
        Self {
            config_path: launcher_root
                .as_ref()
                .join("launcher-data")
                .join("metadata")
                .join("runtime-profiles.json"),
            write_lock: Arc::new(RwLock::new(())),
            event_journal: Arc::new(RwLock::new(RuntimeProfileEventJournal::default())),
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

    pub async fn upsert_profile(
        &self,
        profile: RuntimeProfileConfig,
    ) -> Result<RuntimeProfileMutationResponse> {
        validate_profile_config(&profile).await?;
        let profile_id = profile.profile_id.clone();
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
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
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
            let profile = config
                .profiles
                .iter()
                .find(|profile| profile.profile_id == selected_profile_id)
                .ok_or_else(|| PumasError::InvalidParams {
                    message: format!(
                        "runtime profile not found: {}",
                        selected_profile_id.as_str()
                    ),
                })?;
            if profile.provider != provider {
                return Err(PumasError::InvalidParams {
                    message: format!(
                        "runtime profile {} does not use provider {:?}",
                        selected_profile_id.as_str(),
                        provider
                    ),
                });
            }
            profile
                .endpoint_url
                .clone()
                .ok_or_else(|| PumasError::InvalidParams {
                    message: format!(
                        "runtime profile {} does not define endpoint_url",
                        selected_profile_id.as_str()
                    ),
                })
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join runtime profile endpoint resolution task: {err}"
            ))
        })?
    }

    async fn mutate_config<F>(&self, mutate: F) -> Result<RuntimeProfileMutationResponse>
    where
        F: FnOnce(&mut RuntimeProfilesConfigFile) -> Result<RuntimeProfileMutationResponse>
            + Send
            + 'static,
    {
        let config_path = self.config_path.clone();
        let write_lock = self.write_lock.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = write_lock.write().map_err(|_| {
                PumasError::Other("Failed to acquire runtime profile config lock".to_string())
            })?;
            let mut config = load_or_initialize_config(&config_path)?;
            let response = mutate(&mut config)?;
            bump_cursor(&mut config);
            atomic_write_json(&config_path, &config, true)?;
            Ok(response)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join runtime profile mutation task: {err}"
            ))
        })?
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
        Ok(journal.record_status(status))
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

    match profile.provider {
        RuntimeProviderId::Ollama => OllamaRuntimeProviderAdapter.validate_profile(profile).await,
        RuntimeProviderId::LlamaCpp => {
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
    async fn runtime_profile_status_changes_emit_update_events() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = RuntimeProfileService::new(temp_dir.path());
        let snapshot = service.snapshot().await.unwrap();
        let cursor = snapshot.snapshot.cursor.clone();

        let stopped_event = service.record_default_ollama_status(false).await.unwrap();
        assert!(stopped_event.is_none());

        let running_event = service.record_default_ollama_status(true).await.unwrap();
        assert_eq!(
            running_event.as_ref().map(|event| event.event_kind),
            Some(RuntimeProfileEventKind::StatusChanged)
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
}
