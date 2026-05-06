//! Explicit local client API for attaching to a running Pumas instance.

use super::IpcClient;
use crate::models::{ModelLibrarySelectorSnapshot, ModelLibrarySelectorSnapshotRequest};
use crate::registry::{InstanceEntry, InstanceStatus, LibraryRegistry, LocalInstanceTransportKind};
use crate::{PumasError, Result};
use std::net::SocketAddr;

/// Explicit same-device client for a running Pumas Library instance.
#[derive(Debug)]
pub struct PumasLocalClient {
    client: IpcClient,
    instance: InstanceEntry,
}

impl PumasLocalClient {
    /// Discover ready local instances from the platform registry.
    pub fn discover_ready_instances() -> Result<Vec<InstanceEntry>> {
        let registry = LibraryRegistry::open()?;
        let _ = registry.cleanup_stale()?;
        Ok(registry
            .list_instances()?
            .into_iter()
            .filter(|instance| instance.status == InstanceStatus::Ready)
            .collect())
    }

    /// Connect to a ready local instance advertised by the registry.
    pub async fn connect(instance: InstanceEntry) -> Result<Self> {
        if instance.status != InstanceStatus::Ready {
            return Err(PumasError::InvalidParams {
                message: "local Pumas instance must be ready before clients can connect"
                    .to_string(),
            });
        }

        let addr = loopback_tcp_addr(&instance)?;
        let client = IpcClient::connect(addr, instance.pid).await?;
        Ok(Self { client, instance })
    }

    pub fn instance(&self) -> &InstanceEntry {
        &self.instance
    }

    /// Fetch the selector snapshot in one transport request.
    pub async fn model_library_selector_snapshot(
        &self,
        request: ModelLibrarySelectorSnapshotRequest,
    ) -> Result<ModelLibrarySelectorSnapshot> {
        let value = self
            .client
            .call(
                "model_library_selector_snapshot",
                serde_json::json!({ "request": request }),
            )
            .await?;
        serde_json::from_value(value).map_err(|err| PumasError::Json {
            message: format!("Failed to decode local selector snapshot: {err}"),
            source: Some(err),
        })
    }
}

fn loopback_tcp_addr(instance: &InstanceEntry) -> Result<SocketAddr> {
    if instance.transport_kind != LocalInstanceTransportKind::LoopbackTcp {
        return Err(PumasError::InvalidParams {
            message: format!(
                "unsupported local Pumas transport kind: {:?}",
                instance.transport_kind
            ),
        });
    }

    let addr: SocketAddr = instance
        .endpoint
        .parse()
        .map_err(|err| PumasError::InvalidParams {
            message: format!(
                "invalid local Pumas endpoint '{}': {err}",
                instance.endpoint
            ),
        })?;

    if !addr.ip().is_loopback() {
        return Err(PumasError::InvalidParams {
            message: format!(
                "local Pumas loopback TCP endpoint must be loopback-only: {}",
                instance.endpoint
            ),
        });
    }

    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{IpcDispatch, IpcServer};
    use crate::models::ModelLibrarySelectorSnapshot;
    use async_trait::async_trait;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};
    use tempfile::TempDir;

    static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct RegistryOverrideGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl RegistryOverrideGuard {
        fn new(root: &std::path::Path) -> Self {
            let lock = REGISTRY_TEST_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("registry test lock poisoned");
            crate::platform::paths::set_test_registry_db_path(Some(
                root.join("registry-test")
                    .join(crate::config::RegistryConfig::DB_FILENAME),
            ));
            Self { _lock: lock }
        }
    }

    impl Drop for RegistryOverrideGuard {
        fn drop(&mut self) {
            crate::platform::paths::set_test_registry_db_path(None);
        }
    }

    struct SelectorSnapshotDispatch;

    #[async_trait]
    impl IpcDispatch for SelectorSnapshotDispatch {
        async fn dispatch(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> std::result::Result<serde_json::Value, PumasError> {
            assert_eq!(method, "model_library_selector_snapshot");
            let request: ModelLibrarySelectorSnapshotRequest =
                serde_json::from_value(params["request"].clone()).unwrap();
            assert_eq!(request.limit, Some(25));
            Ok(serde_json::to_value(ModelLibrarySelectorSnapshot::empty(
                "model-library-updates:7",
            ))?)
        }
    }

    fn ready_instance(port: u16) -> InstanceEntry {
        InstanceEntry {
            library_path: PathBuf::from("/tmp/pumas-test-library"),
            pid: std::process::id(),
            port,
            transport_kind: LocalInstanceTransportKind::LoopbackTcp,
            endpoint: format!("127.0.0.1:{port}"),
            connection_token: Some("token".to_string()),
            started_at: "2026-05-06T00:00:00Z".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            status: InstanceStatus::Ready,
        }
    }

    #[tokio::test]
    async fn local_client_connects_and_fetches_selector_snapshot_once() {
        let Some(server) = IpcServer::start(Arc::new(SelectorSnapshotDispatch))
            .await
            .ok()
        else {
            eprintln!("Skipping local_client_connects_and_fetches_selector_snapshot_once");
            return;
        };

        let client = PumasLocalClient::connect(ready_instance(server.port))
            .await
            .unwrap();
        let snapshot = client
            .model_library_selector_snapshot(ModelLibrarySelectorSnapshotRequest {
                limit: Some(25),
                ..ModelLibrarySelectorSnapshotRequest::default()
            })
            .await
            .unwrap();

        assert_eq!(snapshot.cursor, "model-library-updates:7");
        assert!(snapshot.rows.is_empty());
    }

    #[tokio::test]
    async fn local_client_selector_snapshot_reports_transport_timing_target() {
        let Some(server) = IpcServer::start(Arc::new(SelectorSnapshotDispatch))
            .await
            .ok()
        else {
            eprintln!("Skipping local_client_selector_snapshot_reports_transport_timing_target");
            return;
        };

        let client = PumasLocalClient::connect(ready_instance(server.port))
            .await
            .unwrap();
        let started = std::time::Instant::now();
        let snapshot = client
            .model_library_selector_snapshot(ModelLibrarySelectorSnapshotRequest {
                limit: Some(25),
                ..ModelLibrarySelectorSnapshotRequest::default()
            })
            .await
            .unwrap();
        let elapsed = started.elapsed();
        eprintln!(
            "local_client_selector_snapshot_transport_ms={:.3}",
            elapsed.as_secs_f64() * 1000.0
        );

        assert_eq!(snapshot.cursor, "model-library-updates:7");
        assert!(
            elapsed <= std::time::Duration::from_millis(25),
            "local-client selector snapshot exceeded 25ms target: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn local_client_rejects_non_loopback_tcp_endpoint() {
        let mut instance = ready_instance(12345);
        instance.endpoint = "0.0.0.0:12345".to_string();

        let err = PumasLocalClient::connect(instance).await.unwrap_err();
        assert!(matches!(err, PumasError::InvalidParams { .. }));
    }

    #[tokio::test]
    async fn local_client_rejects_non_ready_instance() {
        let mut instance = ready_instance(12345);
        instance.status = InstanceStatus::Claiming;

        let err = PumasLocalClient::connect(instance).await.unwrap_err();
        assert!(matches!(err, PumasError::InvalidParams { .. }));
    }

    #[test]
    fn local_client_discovers_ready_instances_only() {
        let temp_dir = TempDir::new().unwrap();
        let _registry_override = RegistryOverrideGuard::new(temp_dir.path());
        let ready_root = temp_dir.path().join("ready-library");
        let claiming_root = temp_dir.path().join("claiming-library");
        std::fs::create_dir_all(&ready_root).unwrap();
        std::fs::create_dir_all(&claiming_root).unwrap();

        let registry = LibraryRegistry::open().unwrap();
        registry.register(&ready_root, "Ready Library").unwrap();
        registry
            .register(&claiming_root, "Claiming Library")
            .unwrap();
        registry
            .register_instance(&ready_root, std::process::id(), 34567)
            .unwrap();
        let _ = registry
            .try_claim_instance(&claiming_root, std::process::id())
            .unwrap();

        let instances = PumasLocalClient::discover_ready_instances().unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].port, 34567);
        assert_eq!(instances[0].status, InstanceStatus::Ready);
    }
}
