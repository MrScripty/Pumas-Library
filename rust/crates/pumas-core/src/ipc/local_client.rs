//! Explicit local client API for attaching to a running Pumas instance.

use super::protocol::{read_frame, write_frame, IpcRequest, IpcResponse};
use super::IpcClient;
use crate::models::{
    ModelExecutionDescriptorBatchItem, ModelInferenceSettingsBatchItem,
    ModelLibrarySelectorSnapshot, ModelLibrarySelectorSnapshotRequest,
    ModelLibraryUpdateNotification, ModelLibraryUpdateSubscription,
    ModelPackageFactsSummaryBatchItem,
};
use crate::registry::{InstanceEntry, InstanceStatus, LibraryRegistry, LocalInstanceTransportKind};
use crate::{PumasError, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::TcpStream;

static LOCAL_STREAM_REQUEST_ID: AtomicU64 = AtomicU64::new(1_000_000);

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
        Self::ready_instances_in_registry(&registry)
    }

    pub fn ready_instances_in_registry(registry: &LibraryRegistry) -> Result<Vec<InstanceEntry>> {
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
        if instance.connection_token.is_none() {
            return Err(PumasError::InvalidParams {
                message: "local Pumas instance is missing a connection token".to_string(),
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
        self.call_owner_method(
            "model_library_selector_snapshot",
            serde_json::json!({ "request": request }),
        )
        .await
    }

    /// Resolve package-facts summaries for selected models in one transport request.
    pub async fn resolve_model_package_facts_summaries(
        &self,
        model_ids: Vec<String>,
    ) -> Result<Vec<ModelPackageFactsSummaryBatchItem>> {
        self.call_owner_method(
            "resolve_model_package_facts_summaries",
            serde_json::json!({ "model_ids": model_ids }),
        )
        .await
    }

    /// Resolve cheap execution descriptors for selected models in one transport request.
    pub async fn resolve_model_execution_descriptors_batch(
        &self,
        model_ids: Vec<String>,
    ) -> Result<Vec<ModelExecutionDescriptorBatchItem>> {
        self.call_owner_method(
            "resolve_model_execution_descriptors_batch",
            serde_json::json!({ "model_ids": model_ids }),
        )
        .await
    }

    /// Resolve inference settings for selected models in one transport request.
    pub async fn get_inference_settings_batch(
        &self,
        model_ids: Vec<String>,
    ) -> Result<Vec<ModelInferenceSettingsBatchItem>> {
        self.call_owner_method(
            "get_inference_settings_batch",
            serde_json::json!({ "model_ids": model_ids }),
        )
        .await
    }

    /// Open one IPC stream for model-library update notifications.
    pub async fn subscribe_model_library_update_stream_since(
        &self,
        cursor: &str,
    ) -> Result<PumasLocalModelLibraryUpdateStream> {
        let addr = loopback_tcp_addr(&self.instance)?;
        let mut stream =
            TcpStream::connect(addr)
                .await
                .map_err(|_| PumasError::SharedInstanceLost {
                    pid: self.instance.pid,
                    port: self.instance.port,
                })?;
        let request_id = LOCAL_STREAM_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let request = IpcRequest::new(
            "subscribe_model_library_update_stream_since",
            serde_json::json!({
                "cursor": cursor,
                "connection_token": self.connection_token()?,
            }),
            request_id,
        );
        let request_bytes = serde_json::to_vec(&request)?;
        write_frame(&mut stream, &request_bytes)
            .await
            .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.instance.pid,
                port: self.instance.port,
            })?;

        let handshake: ModelLibraryUpdateSubscription =
            read_stream_response(&mut stream, self.instance.pid, self.instance.port).await?;
        Ok(PumasLocalModelLibraryUpdateStream {
            handshake,
            stream,
            primary_pid: self.instance.pid,
            primary_port: self.instance.port,
        })
    }

    fn connection_token(&self) -> Result<&str> {
        self.instance
            .connection_token
            .as_deref()
            .ok_or_else(|| PumasError::InvalidParams {
                message: "local Pumas instance is missing a connection token".to_string(),
            })
    }

    async fn call_owner_method<T>(&self, method: &str, mut params: serde_json::Value) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        params["connection_token"] = serde_json::json!(self.connection_token()?);
        let value = self.client.call(method, params).await?;
        serde_json::from_value(value).map_err(|err| PumasError::Json {
            message: format!("Failed to decode local client response for {method}: {err}"),
            source: Some(err),
        })
    }
}

/// Active local model-library update stream.
#[derive(Debug)]
pub struct PumasLocalModelLibraryUpdateStream {
    handshake: ModelLibraryUpdateSubscription,
    stream: TcpStream,
    primary_pid: u32,
    primary_port: u16,
}

impl PumasLocalModelLibraryUpdateStream {
    pub fn handshake(&self) -> &ModelLibraryUpdateSubscription {
        &self.handshake
    }

    pub async fn next_notification(&mut self) -> Result<ModelLibraryUpdateNotification> {
        read_stream_response(&mut self.stream, self.primary_pid, self.primary_port).await
    }
}

async fn read_stream_response<T: serde::de::DeserializeOwned>(
    stream: &mut TcpStream,
    primary_pid: u32,
    primary_port: u16,
) -> Result<T> {
    let response_bytes = read_frame(stream)
        .await
        .map_err(|_| PumasError::SharedInstanceLost {
            pid: primary_pid,
            port: primary_port,
        })?
        .ok_or(PumasError::SharedInstanceLost {
            pid: primary_pid,
            port: primary_port,
        })?;
    let response: IpcResponse =
        serde_json::from_slice(&response_bytes).map_err(|err| PumasError::Json {
            message: format!("Failed to decode local stream response: {err}"),
            source: Some(err),
        })?;

    if let Some(error) = response.error {
        return Err(PumasError::Other(error.message));
    }

    let result = response
        .result
        .ok_or_else(|| PumasError::Other("local stream response missing result".to_string()))?;
    serde_json::from_value(result).map_err(|err| PumasError::Json {
        message: format!("Failed to decode local stream payload: {err}"),
        source: Some(err),
    })
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
    use crate::model_library::ModelLibrary;
    use crate::models::{ModelLibraryChangeKind, ModelLibrarySelectorSnapshot};
    use async_trait::async_trait;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

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

    struct UpdateStreamDispatch {
        library: ModelLibrary,
    }

    struct BatchHydrationDispatch;

    #[async_trait]
    impl IpcDispatch for BatchHydrationDispatch {
        async fn dispatch(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> std::result::Result<serde_json::Value, PumasError> {
            assert_eq!(params["connection_token"].as_str(), Some("token"));
            let model_ids: Vec<String> = serde_json::from_value(params["model_ids"].clone())
                .expect("model_ids should be passed as strings");
            assert_eq!(model_ids, vec!["llm/batch/model".to_string()]);
            match method {
                "resolve_model_package_facts_summaries" => Ok(serde_json::to_value(vec![
                    ModelPackageFactsSummaryBatchItem {
                        model_id: model_ids[0].clone(),
                        result: None,
                        error: Some("not hydrated in test".to_string()),
                    },
                ])?),
                "resolve_model_execution_descriptors_batch" => Ok(serde_json::to_value(vec![
                    ModelExecutionDescriptorBatchItem {
                        model_id: model_ids[0].clone(),
                        descriptor: None,
                        error: Some("not hydrated in test".to_string()),
                    },
                ])?),
                "get_inference_settings_batch" => Ok(serde_json::to_value(vec![
                    ModelInferenceSettingsBatchItem {
                        model_id: model_ids[0].clone(),
                        settings: Vec::new(),
                        error: Some("not hydrated in test".to_string()),
                    },
                ])?),
                _ => Err(PumasError::Other(format!(
                    "unexpected IPC method: {method}"
                ))),
            }
        }
    }

    #[async_trait]
    impl IpcDispatch for UpdateStreamDispatch {
        async fn dispatch(
            &self,
            method: &str,
            _params: serde_json::Value,
        ) -> std::result::Result<serde_json::Value, PumasError> {
            Err(PumasError::Other(format!(
                "unexpected IPC method: {method}"
            )))
        }

        async fn subscribe_model_library_update_stream_since(
            &self,
            cursor: &str,
            _connection_token: Option<&str>,
        ) -> std::result::Result<
            Option<crate::model_library::ModelLibraryUpdateSubscriber>,
            PumasError,
        > {
            Ok(Some(
                self.library
                    .subscribe_model_library_update_stream_since(cursor)
                    .await?,
            ))
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
    async fn local_client_fetches_batch_hydration_in_one_request_each() {
        let Some(server) = IpcServer::start(Arc::new(BatchHydrationDispatch))
            .await
            .ok()
        else {
            eprintln!("Skipping local_client_fetches_batch_hydration_in_one_request_each");
            return;
        };

        let client = PumasLocalClient::connect(ready_instance(server.port))
            .await
            .unwrap();
        let model_ids = vec!["llm/batch/model".to_string()];

        let summaries = client
            .resolve_model_package_facts_summaries(model_ids.clone())
            .await
            .unwrap();
        let descriptors = client
            .resolve_model_execution_descriptors_batch(model_ids.clone())
            .await
            .unwrap();
        let settings = client
            .get_inference_settings_batch(model_ids)
            .await
            .unwrap();

        assert_eq!(summaries[0].model_id, "llm/batch/model");
        assert!(summaries[0].error.is_some());
        assert_eq!(descriptors[0].model_id, "llm/batch/model");
        assert!(descriptors[0].error.is_some());
        assert_eq!(settings[0].model_id, "llm/batch/model");
        assert!(settings[0].error.is_some());
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

    #[tokio::test]
    async fn local_client_rejects_missing_connection_token() {
        let mut instance = ready_instance(12345);
        instance.connection_token = None;

        let err = PumasLocalClient::connect(instance).await.unwrap_err();
        assert!(matches!(err, PumasError::InvalidParams { .. }));
    }

    #[tokio::test]
    async fn local_client_subscribes_to_one_update_stream() {
        let temp_dir = TempDir::new().unwrap();
        let library_root = temp_dir.path().join("models");
        std::fs::create_dir_all(&library_root).unwrap();
        let library = ModelLibrary::new(&library_root).await.unwrap();
        let cursor = library
            .list_model_library_updates_since(None, 100)
            .await
            .unwrap()
            .cursor;
        let Some(server) = IpcServer::start(Arc::new(UpdateStreamDispatch {
            library: library.clone(),
        }))
        .await
        .ok() else {
            eprintln!("Skipping local_client_subscribes_to_one_update_stream");
            return;
        };

        let client = PumasLocalClient::connect(ready_instance(server.port))
            .await
            .unwrap();
        let mut stream = client
            .subscribe_model_library_update_stream_since(&cursor)
            .await
            .unwrap();
        assert!(stream.handshake().live_stream_ready);

        library
            .notify_model_library_refresh("local-client-stream-test")
            .unwrap();
        let notification = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            stream.next_notification(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(!notification.snapshot_required);
        assert_eq!(notification.events.len(), 1);
        assert_eq!(
            notification.events[0].change_kind,
            ModelLibraryChangeKind::MetadataModified
        );
    }

    #[test]
    fn local_client_discovers_ready_instances_only() {
        let temp_dir = TempDir::new().unwrap();
        let ready_root = temp_dir.path().join("ready-library");
        let claiming_root = temp_dir.path().join("claiming-library");
        std::fs::create_dir_all(&ready_root).unwrap();
        std::fs::create_dir_all(&claiming_root).unwrap();

        let registry = LibraryRegistry::open_at(&temp_dir.path().join("registry.db")).unwrap();
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

        let instances = PumasLocalClient::ready_instances_in_registry(&registry).unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].port, 34567);
        assert_eq!(instances[0].status, InstanceStatus::Ready);
    }
}
