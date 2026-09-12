//! Generation-fenced router projections; observation never settles a load receipt.
use super::{bump_snapshot_cursor, ServingService};
use crate::models::{
    RouterObservationState, RouterProfileSyncStatus, RuntimeProfileId, ServedModelLoadState,
    ServedModelStatus, ServingEndpointMode, ServingEndpointStatus, ServingStatusSnapshot,
    ServingStatusUpdateFeed,
};
use crate::runtime_profiles::{OwnedRuntimeProfileObservation, RuntimeProfileProcessOwner};

impl ServingService {
    pub(crate) fn publish_router_observation(
        &self,
        profile_id: &RuntimeProfileId,
        receipt: &OwnedRuntimeProfileObservation,
        owner: &RuntimeProfileProcessOwner,
        sampled_cursor: &str,
        rows: Vec<ServedModelStatus>,
        sync: RouterProfileSyncStatus,
    ) -> crate::Result<bool> {
        if sync.profile_id != *profile_id || sync.generation != receipt.generation {
            return Ok(false);
        }
        let result = owner.with_running_session(profile_id, receipt, || {
            let mut state = self.state.lock().expect("serving state poisoned");
            if state.snapshot.cursor != sampled_cursor {
                return None;
            }
            let before = state.snapshot.clone();
            let mut merged = state
                .snapshot
                .served_models
                .iter()
                .filter(|row| row.profile_id != *profile_id || state.reserves_target(row))
                .cloned()
                .collect::<Vec<_>>();
            for mut row in rows {
                if row.profile_id != *profile_id || state.reserves_target(&row) {
                    continue;
                }
                if let Some(existing) = before.served_models.iter().find(|existing| {
                    existing.profile_id == row.profile_id
                        && existing.model_id == row.model_id
                        && existing.provider == row.provider
                }) {
                    row.model_alias = existing.model_alias.clone();
                    row.device_mode = existing.device_mode;
                    row.device_id = existing.device_id.clone();
                    row.gpu_layers = existing.gpu_layers;
                    row.tensor_split = existing.tensor_split.clone();
                    row.context_size = existing.context_size;
                    row.keep_loaded = existing.keep_loaded;
                    row.loaded_at = existing.loaded_at.clone();
                }
                state.retire_settled_failure(&row);
                merged.push(row);
            }
            state.snapshot.served_models = merged;
            set_sync(&mut state.snapshot, sync);
            refresh_endpoint(&mut state.snapshot);
            Some(changed_feed(&mut state.snapshot, &before))
        })?;
        match result {
            None => Ok(false),
            Some(feed) => {
                if let Some(feed) = feed {
                    self.publish_feed(feed);
                }
                Ok(true)
            }
        }
    }

    pub(crate) fn publish_router_sync(
        &self,
        receipt: &OwnedRuntimeProfileObservation,
        owner: &RuntimeProfileProcessOwner,
        sync: RouterProfileSyncStatus,
    ) -> crate::Result<()> {
        if sync.generation != receipt.generation {
            return Ok(());
        }
        let profile_id = sync.profile_id.clone();
        let feed = owner
            .with_current_generation(&profile_id, receipt.generation, || {
                let mut state = self.state.lock().expect("serving state poisoned");
                let before = state.snapshot.clone();
                set_sync(&mut state.snapshot, sync);
                refresh_endpoint(&mut state.snapshot);
                changed_feed(&mut state.snapshot, &before)
            })?
            .flatten();
        if let Some(feed) = feed {
            self.publish_feed(feed);
        }
        Ok(())
    }
}

fn set_sync(snapshot: &mut ServingStatusSnapshot, mut sync: RouterProfileSyncStatus) {
    if sync.catalog_state == crate::models::RouterCatalogState::Uncertain {
        sync.observation_state = RouterObservationState::Unavailable;
    }
    if let Some(existing) = snapshot
        .router_profiles
        .iter_mut()
        .find(|row| row.profile_id == sync.profile_id)
    {
        *existing = sync;
    } else {
        snapshot.router_profiles.push(sync);
    }
}

fn changed_feed(
    snapshot: &mut ServingStatusSnapshot,
    before: &ServingStatusSnapshot,
) -> Option<ServingStatusUpdateFeed> {
    if snapshot == before {
        return None;
    }
    bump_snapshot_cursor(snapshot);
    Some(ServingStatusUpdateFeed {
        cursor: snapshot.cursor.clone(),
        events: Vec::new(),
        stale_cursor: false,
        snapshot_required: true,
    })
}

pub(super) fn profile_is_current(
    snapshot: &ServingStatusSnapshot,
    profile_id: &RuntimeProfileId,
) -> bool {
    snapshot
        .router_profiles
        .iter()
        .find(|sync| &sync.profile_id == profile_id)
        .is_none_or(|sync| sync.is_observation_current())
}

pub(super) fn refresh_endpoint(snapshot: &mut ServingStatusSnapshot) {
    let count = snapshot
        .served_models
        .iter()
        .filter(|row| {
            row.load_state == ServedModelLoadState::Loaded
                && profile_is_current(snapshot, &row.profile_id)
        })
        .count() as u32;
    snapshot.endpoint = if count == 0 {
        ServingEndpointStatus::not_configured()
    } else {
        ServingEndpointStatus {
            endpoint_mode: ServingEndpointMode::PumasGateway,
            endpoint_url: None,
            model_count: count,
            message: Some("Use the Pumas /v1 serving gateway for loaded models".to_string()),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RouterCatalogState;

    #[test]
    fn uncertain_catalog_is_never_current_and_normalizes_on_publication() {
        let sync = RouterProfileSyncStatus {
            profile_id: RuntimeProfileId::parse("synthetic-router").unwrap(),
            generation: 1,
            observation_state: RouterObservationState::Current,
            catalog_state: RouterCatalogState::Uncertain,
            pending_model_ids: Vec::new(),
            last_error: None,
        };
        assert!(!sync.is_observation_current());
        let mut snapshot = ServingStatusSnapshot::empty();
        snapshot.router_profiles.push(sync.clone());
        assert!(!profile_is_current(&snapshot, &sync.profile_id));
        set_sync(&mut snapshot, sync);
        assert_eq!(
            snapshot.router_profiles[0].observation_state,
            RouterObservationState::Unavailable
        );
        snapshot.router_profiles[0].catalog_state = RouterCatalogState::Pending;
        snapshot.router_profiles[0].observation_state = RouterObservationState::Current;
        assert!(snapshot.router_profiles[0].is_observation_current());
    }
}
