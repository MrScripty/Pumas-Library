//! Backend-owned status/resource telemetry snapshots and update fanout.

use super::state::PrimaryState;
use super::state_runtime::{network_status_response, status_response, system_resources_response};
use crate::error::{PumasError, Result};
use crate::models;
use crate::PumasApi;
use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use tokio::sync::broadcast;
use tracing::debug;

const STATUS_TELEMETRY_CURSOR_PREFIX: &str = "status-telemetry:";
const STATUS_TELEMETRY_CHANNEL_CAPACITY: usize = 32;

pub(crate) struct StatusTelemetryService {
    revision: AtomicU64,
    current: RwLock<Option<models::StatusTelemetrySnapshot>>,
    updates: broadcast::Sender<models::StatusTelemetryUpdateNotification>,
}

impl StatusTelemetryService {
    pub(crate) fn new() -> Self {
        let (updates, _) = broadcast::channel(STATUS_TELEMETRY_CHANNEL_CAPACITY);
        Self {
            revision: AtomicU64::new(0),
            current: RwLock::new(None),
            updates,
        }
    }

    fn next_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub(crate) fn current_snapshot(&self) -> Option<models::StatusTelemetrySnapshot> {
        self.current
            .read()
            .expect("status telemetry cache poisoned")
            .clone()
    }

    pub(crate) fn subscribe(
        &self,
    ) -> broadcast::Receiver<models::StatusTelemetryUpdateNotification> {
        self.updates.subscribe()
    }

    pub(crate) fn publish(&self, snapshot: models::StatusTelemetrySnapshot) {
        {
            let mut current = self
                .current
                .write()
                .expect("status telemetry cache poisoned");
            *current = Some(snapshot.clone());
        }

        let notification = models::StatusTelemetryUpdateNotification {
            cursor: snapshot.cursor.clone(),
            snapshot,
            stale_cursor: false,
            snapshot_required: false,
        };
        if let Err(error) = self.updates.send(notification) {
            debug!("status telemetry update had no receivers: {}", error);
        }
    }

    pub(crate) fn notification_since(
        &self,
        cursor: Option<&str>,
        snapshot: models::StatusTelemetrySnapshot,
    ) -> Option<models::StatusTelemetryUpdateNotification> {
        let requested = cursor.and_then(parse_status_telemetry_cursor);
        let stale_cursor = cursor.is_some() && requested.is_none();
        let snapshot_required = requested
            .map(|revision| revision < snapshot.revision)
            .unwrap_or(true);

        if !snapshot_required && !stale_cursor {
            return None;
        }

        Some(models::StatusTelemetryUpdateNotification {
            cursor: snapshot.cursor.clone(),
            snapshot,
            stale_cursor,
            snapshot_required,
        })
    }
}

impl Default for StatusTelemetryService {
    fn default() -> Self {
        Self::new()
    }
}

fn status_telemetry_cursor(revision: u64) -> String {
    format!("{STATUS_TELEMETRY_CURSOR_PREFIX}{revision}")
}

fn parse_status_telemetry_cursor(cursor: &str) -> Option<u64> {
    cursor
        .strip_prefix(STATUS_TELEMETRY_CURSOR_PREFIX)
        .and_then(|raw| raw.parse::<u64>().ok())
}

async fn build_status_telemetry_snapshot(
    primary: &PrimaryState,
) -> Result<models::StatusTelemetrySnapshot> {
    let status = status_response(primary).await?;
    let resources = system_resources_response(primary).await?.resources;
    let network = network_status_response(primary).await;
    let library = library_status_response(primary).await?;

    let revision = primary.status_telemetry.next_revision();
    Ok(models::StatusTelemetrySnapshot {
        cursor: status_telemetry_cursor(revision),
        revision,
        sampled_at: Utc::now().to_rfc3339(),
        source_state: "ready".to_string(),
        status,
        resources,
        network,
        model_library_loaded: library.success,
        library,
    })
}

async fn library_status_response(primary: &PrimaryState) -> Result<models::LibraryStatusResponse> {
    let library = primary.model_library.clone();
    let model_count = tokio::task::spawn_blocking(move || library.model_count())
        .await
        .map_err(|err| PumasError::Other(format!("Failed to join model count task: {}", err)))??;
    let pending_lookups = primary.model_library.get_pending_lookups().await?.len() as u32;

    Ok(models::LibraryStatusResponse {
        success: true,
        error: None,
        indexing: false,
        deep_scan_in_progress: false,
        model_count: model_count as u32,
        pending_lookups: Some(pending_lookups),
        deep_scan_progress: None,
    })
}

impl PumasApi {
    pub async fn get_status_telemetry_snapshot(&self) -> Result<models::StatusTelemetrySnapshot> {
        let primary = self.primary();
        if let Some(snapshot) = primary.status_telemetry.current_snapshot() {
            return Ok(snapshot);
        }

        let snapshot = build_status_telemetry_snapshot(primary).await?;
        primary.status_telemetry.publish(snapshot.clone());
        Ok(snapshot)
    }

    pub async fn refresh_status_telemetry_snapshot(
        &self,
    ) -> Result<models::StatusTelemetrySnapshot> {
        let primary = self.primary();
        let snapshot = build_status_telemetry_snapshot(primary).await?;
        primary.status_telemetry.publish(snapshot.clone());
        Ok(snapshot)
    }

    pub fn subscribe_status_telemetry_updates(
        &self,
    ) -> broadcast::Receiver<models::StatusTelemetryUpdateNotification> {
        self.primary().status_telemetry.subscribe()
    }

    pub fn status_telemetry_notification_since(
        &self,
        cursor: Option<&str>,
        snapshot: models::StatusTelemetrySnapshot,
    ) -> Option<models::StatusTelemetryUpdateNotification> {
        self.primary()
            .status_telemetry
            .notification_since(cursor, snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_status_telemetry_cursor, status_telemetry_cursor, StatusTelemetryService};
    use crate::models;

    fn snapshot(revision: u64) -> models::StatusTelemetrySnapshot {
        models::StatusTelemetrySnapshot {
            cursor: status_telemetry_cursor(revision),
            revision,
            sampled_at: "2026-05-06T00:00:00Z".to_string(),
            source_state: "ready".to_string(),
            status: models::StatusResponse {
                success: true,
                error: None,
                version: "test".to_string(),
                deps_ready: true,
                patched: false,
                menu_shortcut: false,
                desktop_shortcut: false,
                shortcut_version: None,
                message: "Ready".to_string(),
                comfyui_running: false,
                ollama_running: false,
                torch_running: false,
                last_launch_error: None,
                last_launch_log: None,
                app_resources: None,
            },
            resources: models::SystemResources {
                cpu: models::CpuResources {
                    usage: 0.0,
                    temp: None,
                },
                gpu: models::GpuResources {
                    usage: 0.0,
                    memory: 0,
                    memory_total: 0,
                    temp: None,
                },
                ram: models::RamResources {
                    usage: 0.0,
                    total: 0,
                },
                disk: models::DiskResources {
                    usage: 0.0,
                    total: 0,
                    free: 0,
                },
            },
            network: models::NetworkStatusResponse {
                success: true,
                error: None,
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                circuit_breaker_rejections: 0,
                retries: 0,
                success_rate: 1.0,
                circuit_states: Default::default(),
                is_offline: false,
            },
            library: models::LibraryStatusResponse {
                success: true,
                error: None,
                indexing: false,
                deep_scan_in_progress: false,
                model_count: 0,
                pending_lookups: Some(0),
                deep_scan_progress: None,
            },
            model_library_loaded: true,
        }
    }

    #[test]
    fn telemetry_cursor_round_trips_revision() {
        let cursor = status_telemetry_cursor(42);
        assert_eq!(parse_status_telemetry_cursor(&cursor), Some(42));
    }

    #[test]
    fn notification_since_current_cursor_returns_none() {
        let service = StatusTelemetryService::default();
        let snapshot = snapshot(7);

        let notification = service.notification_since(Some("status-telemetry:7"), snapshot);

        assert!(notification.is_none());
    }

    #[test]
    fn notification_since_stale_cursor_returns_snapshot() {
        let service = StatusTelemetryService::default();
        let snapshot = snapshot(7);

        let notification = service
            .notification_since(Some("status-telemetry:3"), snapshot)
            .expect("stale cursor should receive snapshot");

        assert!(notification.snapshot_required);
        assert!(!notification.stale_cursor);
        assert_eq!(notification.cursor, "status-telemetry:7");
    }

    #[test]
    fn notification_since_invalid_cursor_marks_stale() {
        let service = StatusTelemetryService::default();
        let snapshot = snapshot(7);

        let notification = service
            .notification_since(Some("invalid"), snapshot)
            .expect("invalid cursor should receive snapshot");

        assert!(notification.snapshot_required);
        assert!(notification.stale_cursor);
    }
}
