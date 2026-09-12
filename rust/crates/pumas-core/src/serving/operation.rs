//! Invocation-owned load admission and cancellation projection.
//!
//! The mutex covers canonical loaded state, projected work and admission together.
//! A cancelled request retains uncertainty, never a detached cleanup task.
use std::sync::{Arc, Mutex};

use super::{bump_snapshot_cursor, ServingService};
use crate::models::{
    ModelServeError, ModelServeErrorCode, RuntimeProfileId, ServeModelRequest,
    ServedModelLoadState, ServedModelStatus, ServingStatusSnapshot, ServingStatusUpdateFeed,
};
use tokio::sync::broadcast;

#[derive(Debug)]
pub(super) struct ServingState {
    pub snapshot: ServingStatusSnapshot,
    pending: Vec<PendingLoad>,
    next_token: u64,
}

#[derive(Debug)]
struct PendingLoad {
    token: u64,
    status: ServedModelStatus,
    valid: bool,
    active: bool,
}

impl ServingState {
    pub fn new() -> Self {
        Self {
            snapshot: ServingStatusSnapshot::empty(),
            pending: Vec::new(),
            next_token: 0,
        }
    }

    pub fn project(&self) -> ServingStatusSnapshot {
        let mut snapshot = self.snapshot.clone();
        snapshot.served_models.extend(
            self.pending
                .iter()
                .filter(|load| load.valid)
                .map(|load| load.status.clone()),
        );
        snapshot
    }

    pub fn invalidate_profile(&mut self, profile_id: &RuntimeProfileId) -> bool {
        let mut changed = false;
        self.pending.retain_mut(|load| {
            if &load.status.profile_id != profile_id {
                return true;
            }
            changed |= load.valid;
            load.valid = false;
            load.active
        });
        changed
    }
}

/// Exact invocation receipt. Dropping unfinished work publishes explicit uncertainty.
/// Receipts are deliberately neither cloneable nor constructible by adapters.
#[derive(Debug)]
pub struct ServingLoadOperation {
    state: Arc<Mutex<ServingState>>,
    updates: broadcast::Sender<ServingStatusUpdateFeed>,
    token: u64,
}

impl ServingService {
    /// Atomically reserve the target and gateway alias before any provider mutation.
    pub fn begin_load(
        &self,
        request: &ServeModelRequest,
    ) -> Result<ServingLoadOperation, ModelServeError> {
        let error = |code, message| request_error(request, code, message);
        let cursor = {
            let mut state = self.state.lock().map_err(|_| {
                error(
                    ModelServeErrorCode::Unknown,
                    "serving authority is unavailable",
                )
            })?;
            let status = pending_status(request);
            let targets = state.snapshot.served_models.iter().chain(
                state
                    .pending
                    .iter()
                    .filter(|load| {
                        load.active
                            || load
                                .status
                                .last_error
                                .as_ref()
                                .is_some_and(|error| error.code == ModelServeErrorCode::Unknown)
                    })
                    .map(|load| &load.status),
            );
            if targets.clone().any(|loaded| same_target(loaded, &status)) {
                return Err(error(
                    ModelServeErrorCode::InvalidRequest,
                    "model already has a loaded or unresolved serving operation on this profile",
                ));
            }
            let alias = super::gateway_alias::gateway_alias_key(
                super::gateway_alias::served_status_effective_gateway_alias(&status),
            );
            if targets.into_iter().any(|loaded| {
                super::gateway_alias::gateway_alias_key(
                    super::gateway_alias::served_status_effective_gateway_alias(loaded),
                ) == alias
            }) {
                return Err(error(
                    ModelServeErrorCode::DuplicateModelAlias,
                    "gateway model alias is reserved by another serving operation",
                ));
            }
            state.next_token = state.next_token.checked_add(1).ok_or_else(|| {
                error(
                    ModelServeErrorCode::Unknown,
                    "serving operation identity exhausted",
                )
            })?;
            let token = state.next_token;
            state
                .pending
                .retain(|load| !same_target(&load.status, &status));
            state.pending.push(PendingLoad {
                token,
                status,
                valid: true,
                active: true,
            });
            bump_snapshot_cursor(&mut state.snapshot);
            (token, state.snapshot.cursor.clone())
        };
        self.publish_feed(ServingStatusUpdateFeed::snapshot_required(cursor.1));
        Ok(ServingLoadOperation {
            state: self.state.clone(),
            updates: self.updates.clone(),
            token: cursor.0,
        })
    }

    /// Publish only for this exact admission; managed process custody is checked in
    /// the same critical section as the operation receipt and loaded transition.
    pub(crate) fn record_loaded_model_for_operation(
        &self,
        operation: &ServingLoadOperation,
        status: ServedModelStatus,
        owned: Option<(
            &crate::runtime_profiles::RuntimeProfileProcessOwner,
            &crate::runtime_profiles::OwnedRuntimeProfileObservation,
        )>,
    ) -> crate::Result<ServingStatusSnapshot> {
        if !Arc::ptr_eq(&self.state, &operation.state) {
            return Err(stale_operation());
        }
        let profile_id = status.profile_id.clone();
        // Process observation (including listener attribution) precedes this
        // mutex. Every managed path orders process owner before serving state.
        let publish = || {
            let mut state = self.state.lock().map_err(|_| stale_operation())?;
            let index = state
                .pending
                .iter()
                .position(|load| {
                    load.token == operation.token
                        && load.valid
                        && load.active
                        && same_target(&load.status, &status)
                        && same_gateway_alias(&load.status, &status)
                })
                .ok_or_else(stale_operation)?;
            if status.load_state != ServedModelLoadState::Loaded {
                return Err(stale_operation());
            }
            state.pending.remove(index);
            let event = Self::record_loaded_model_locked(&mut state.snapshot, status);
            Ok((state.project(), event))
        };
        let (snapshot, event) = match owned {
            Some((owner, expected)) => {
                owner.with_running_session(&profile_id, expected, publish)??
            }
            None => publish()?,
        };
        self.publish_event(event);
        Ok(snapshot)
    }
}

impl ServingLoadOperation {
    /// Finish an observed domain failure. Unknown means provider effects remain
    /// uncertain and must continue reserving the target until profile invalidation.
    pub fn finish_failure(&self, error: ModelServeError) {
        self.finish(Some(error));
    }

    fn finish(&self, error: Option<ModelServeError>) {
        let cursor = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let Some(index) = state
                .pending
                .iter()
                .position(|load| load.token == self.token)
            else {
                return;
            };
            if !state.pending[index].valid {
                if error.is_none() {
                    state.pending.remove(index);
                }
                return;
            }
            if !state.pending[index].active {
                return;
            }
            let status = &state.pending[index].status;
            let error = error.unwrap_or_else(|| {
                ModelServeError::non_critical(
                    ModelServeErrorCode::Unknown,
                    "serving operation ended before its provider outcome was confirmed",
                )
                .for_model(status.model_id.clone())
                .for_profile(status.profile_id.clone())
                .for_provider(status.provider)
            });
            let load = &mut state.pending[index];
            load.active = false;
            load.status.load_state = ServedModelLoadState::Failed;
            load.status.last_error = Some(error.clone());
            state.snapshot.last_errors.push(error);
            bump_snapshot_cursor(&mut state.snapshot);
            state.snapshot.cursor.clone()
        };
        let _ = self
            .updates
            .send(ServingStatusUpdateFeed::snapshot_required(cursor));
    }
}

impl Drop for ServingLoadOperation {
    fn drop(&mut self) {
        self.finish(None);
    }
}

fn stale_operation() -> crate::PumasError {
    crate::PumasError::Config {
        message: "serving load admission is no longer current".into(),
    }
}

fn same_target(left: &ServedModelStatus, right: &ServedModelStatus) -> bool {
    left.model_id == right.model_id
        && left.profile_id == right.profile_id
        && left.provider == right.provider
}

fn same_gateway_alias(left: &ServedModelStatus, right: &ServedModelStatus) -> bool {
    use super::gateway_alias::{gateway_alias_key, served_status_effective_gateway_alias};
    gateway_alias_key(served_status_effective_gateway_alias(left))
        == gateway_alias_key(served_status_effective_gateway_alias(right))
}

fn request_error(
    request: &ServeModelRequest,
    code: ModelServeErrorCode,
    message: &str,
) -> ModelServeError {
    ModelServeError::non_critical(code, message)
        .for_model(request.model_id.trim())
        .for_profile(request.config.profile_id.clone())
        .for_provider(request.config.provider)
}

fn pending_status(request: &ServeModelRequest) -> ServedModelStatus {
    ServedModelStatus {
        model_id: request.model_id.trim().into(),
        model_alias: Some(super::effective_gateway_model_alias(request)),
        provider: request.config.provider,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loading,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: request.config.gpu_layers,
        tensor_split: request.config.tensor_split.clone(),
        context_size: request.config.context_size,
        keep_loaded: request.config.keep_loaded,
        endpoint_url: None,
        memory_bytes: None,
        loaded_at: None,
        last_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModelServingConfig, RuntimeDeviceMode, RuntimeProviderId};
    use crate::providers::ProviderRegistry;

    fn request(model: &str, alias: &str) -> ServeModelRequest {
        ServeModelRequest {
            model_id: model.into(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::LlamaCpp,
                profile_id: RuntimeProfileId::parse("router").unwrap(),
                device_mode: RuntimeDeviceMode::Cpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: Some(18000),
                keep_loaded: true,
                model_alias: Some(alias.into()),
            },
        }
    }
    fn service() -> ServingService {
        ServingService::with_provider_registry(ProviderRegistry::builtin())
    }
    fn loaded(request: &ServeModelRequest) -> ServedModelStatus {
        let mut status = pending_status(request);
        status.load_state = ServedModelLoadState::Loaded;
        status
    }

    #[tokio::test]
    async fn admission_reserves_identity_and_normalized_alias_and_only_counts_loaded() {
        let service = service();
        let request = request("one", "shared.alias");
        let mut updates = service.subscribe_updates();
        let operation = service.begin_load(&request).unwrap();
        assert!(updates.try_recv().unwrap().snapshot_required);
        let mut duplicate = request.clone();
        duplicate.config.model_alias = Some("other".into());
        assert!(service.begin_load(&duplicate).is_err());
        duplicate.model_id = "two".into();
        duplicate.config.model_alias = Some("shared_alias".into());
        assert_eq!(
            service.begin_load(&duplicate).unwrap_err().code,
            ModelServeErrorCode::DuplicateModelAlias
        );
        assert!(service.find_served_model("one", None, None).await.is_none());
        assert_eq!(service.status().await.snapshot.endpoint.model_count, 0);
        service
            .record_loaded_model_for_operation(&operation, loaded(&request), None)
            .unwrap();
        drop(operation);
        let snapshot = service.status().await.snapshot;
        assert_eq!(snapshot.served_models.len(), 1);
        assert_eq!(
            snapshot.served_models[0].load_state,
            ServedModelLoadState::Loaded
        );
        assert_eq!(snapshot.endpoint.model_count, 1);
        assert!(service.begin_load(&request).is_err());
    }

    #[tokio::test]
    async fn receipt_cannot_publish_an_alias_reserved_by_another_operation() {
        let service = service();
        let first = request("one", "first.alias");
        let second = request("two", "second.alias");
        let operation = service.begin_load(&first).unwrap();
        let other = service.begin_load(&second).unwrap();
        let before = service.status().await.snapshot;
        let mut forged = loaded(&first);
        forged.model_alias = second.config.model_alias.clone();
        assert!(service
            .record_loaded_model_for_operation(&operation, forged, None)
            .is_err());
        assert_eq!(service.status().await.snapshot, before);
        assert!(service.begin_load(&second).is_err());
        let mut normalized = loaded(&first);
        normalized.model_alias = Some("first_alias".into());
        service
            .record_loaded_model_for_operation(&operation, normalized, None)
            .unwrap();
        service
            .record_loaded_model_for_operation(&other, loaded(&second), None)
            .unwrap();
        assert_eq!(service.status().await.snapshot.endpoint.model_count, 2);
    }

    #[tokio::test]
    async fn cancellation_retains_uncertainty_until_profile_is_observed_unavailable() {
        let service = service();
        let request = request("one", "one");
        drop(service.begin_load(&request).unwrap());
        let snapshot = service.status().await.snapshot;
        assert_eq!(
            snapshot.served_models[0].load_state,
            ServedModelLoadState::Failed
        );
        assert_eq!(
            snapshot.served_models[0].last_error.as_ref().unwrap().code,
            ModelServeErrorCode::Unknown
        );
        assert!(service.begin_load(&request).is_err());
        assert_eq!(snapshot.endpoint.model_count, 0);
        service
            .record_profile_unavailable(&request.config.profile_id)
            .await
            .unwrap();
        assert!(service.status().await.snapshot.served_models.is_empty());
        assert!(service.begin_load(&request).is_ok());
    }

    #[tokio::test]
    async fn explicit_unknown_failure_retains_target_and_alias_reservations_after_drop() {
        let service = service();
        let first = request("one", "shared.alias");
        let operation = service.begin_load(&first).unwrap();
        operation.finish_failure(request_error(
            &first,
            ModelServeErrorCode::Unknown,
            "provider outcome is unknown",
        ));
        drop(operation);
        let snapshot = service.status().await.snapshot;
        assert_eq!(
            snapshot.served_models[0].load_state,
            ServedModelLoadState::Failed
        );
        assert_eq!(
            snapshot.served_models[0].last_error.as_ref().unwrap().code,
            ModelServeErrorCode::Unknown
        );
        assert!(service.begin_load(&first).is_err());
        let other = request("two", "shared_alias");
        assert_eq!(
            service.begin_load(&other).unwrap_err().code,
            ModelServeErrorCode::DuplicateModelAlias
        );
        service
            .record_profile_unavailable(&first.config.profile_id)
            .await
            .unwrap();
        assert!(service.status().await.snapshot.served_models.is_empty());
        assert!(service.begin_load(&other).is_ok());
    }

    #[tokio::test]
    async fn invalidated_active_receipt_excludes_overlap_and_cannot_publish_or_clear_successor() {
        let service = service();
        let request = request("one", "one");
        let old = service.begin_load(&request).unwrap();
        service
            .record_profile_unavailable(&request.config.profile_id)
            .await
            .unwrap();
        assert!(service.status().await.snapshot.served_models.is_empty());
        assert!(
            service.begin_load(&request).is_err(),
            "invisible active reservation must exclude overlap"
        );
        assert!(service
            .record_loaded_model_for_operation(&old, loaded(&request), None)
            .is_err());
        old.finish_failure(request_error(
            &request,
            ModelServeErrorCode::ProviderLoadFailed,
            "old failure",
        ));
        assert!(service.begin_load(&request).is_err());
        drop(old);
        let successor = service.begin_load(&request).unwrap();
        assert_eq!(
            service.status().await.snapshot.served_models[0].load_state,
            ServedModelLoadState::Loading
        );
        service
            .record_loaded_model_for_operation(&successor, loaded(&request), None)
            .unwrap();
        drop(successor);
        assert_eq!(
            service.status().await.snapshot.served_models[0].load_state,
            ServedModelLoadState::Loaded
        );
    }

    #[tokio::test]
    async fn known_failure_settles_only_its_target_and_allows_retry_without_alias_reservation() {
        let service = service();
        let first = request("one", "one");
        let second = request("two", "two");
        let old = service.begin_load(&first).unwrap();
        let other = service.begin_load(&second).unwrap();
        old.finish_failure(request_error(
            &first,
            ModelServeErrorCode::ProviderLoadFailed,
            "known failure",
        ));
        let snapshot = service.status().await.snapshot;
        assert_eq!(
            snapshot.served_models[0].load_state,
            ServedModelLoadState::Failed
        );
        assert_eq!(
            snapshot.served_models[1].load_state,
            ServedModelLoadState::Loading
        );
        let successor = service.begin_load(&first).unwrap();
        drop(old);
        assert_eq!(service.status().await.snapshot.served_models.len(), 2);
        service
            .record_loaded_model_for_operation(&successor, loaded(&first), None)
            .unwrap();
        drop(other);
        assert_eq!(service.status().await.snapshot.endpoint.model_count, 1);
    }

    #[tokio::test]
    async fn operation_receipt_never_bypasses_managed_process_generation() {
        let service = service();
        let request = request("one", "one");
        let operation = service.begin_load(&request).unwrap();
        let owner = crate::runtime_profiles::RuntimeProfileProcessOwner::default();
        let expected = crate::runtime_profiles::OwnedRuntimeProfileObservation {
            generation: 99,
            pid: Some(42),
            state: crate::models::RuntimeLifecycleState::Running,
            endpoint_url: crate::models::RuntimeEndpointUrl::parse("http://127.0.0.1:1").unwrap(),
            model_path: None,
            context_size: None,
        };
        assert!(service
            .record_loaded_model_for_operation(
                &operation,
                loaded(&request),
                Some((&owner, &expected))
            )
            .is_err());
        assert_eq!(
            service.status().await.snapshot.served_models[0].load_state,
            ServedModelLoadState::Loading
        );
        assert_eq!(service.status().await.snapshot.endpoint.model_count, 0);
    }
}
