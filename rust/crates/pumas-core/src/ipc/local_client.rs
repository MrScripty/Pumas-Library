//! Explicit local client API for attaching to a running Pumas instance.

use super::IpcClient;
use crate::models::{ModelLibrarySelectorSnapshot, ModelLibrarySelectorSnapshotRequest};
use crate::registry::{InstanceEntry, InstanceStatus, LocalInstanceTransportKind};
use crate::{PumasError, Result};
use std::net::SocketAddr;

/// Explicit same-device client for a running Pumas Library instance.
#[derive(Debug)]
pub struct PumasLocalClient {
    client: IpcClient,
    instance: InstanceEntry,
}

impl PumasLocalClient {
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
    use std::sync::Arc;

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
}
