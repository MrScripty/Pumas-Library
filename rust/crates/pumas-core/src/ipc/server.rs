//! TCP IPC server for the primary pumas-core instance.
//!
//! Listens on `127.0.0.1:0` (OS-assigned port), accepts connections from
//! client instances, and dispatches JSON-RPC method calls to the primary state.
//!
//! # Thread Safety
//!
//! The server runs on the tokio runtime. Each connection is handled in its own
//! spawned task. The `PrimaryState` is shared via `Arc` and uses internal
//! synchronization (RwLock) for mutable access.

use super::protocol::{
    read_frame, write_frame, IpcError, IpcRequest, IpcResponse, LocalIpcCommand, LocalIpcOperation,
};
use crate::config::RegistryConfig;
use crate::model_library::ModelLibraryUpdateSubscriber;
use crate::models::ModelLibraryUpdateNotification;
use crate::{PumasError, Result};
use futures::FutureExt;
#[cfg(test)]
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

type ConnectionTasks = Arc<Mutex<Vec<JoinHandle<()>>>>;

struct ActiveConnectionGuard {
    active_connections: Arc<AtomicUsize>,
}

impl ActiveConnectionGuard {
    fn new(active_connections: Arc<AtomicUsize>) -> Self {
        Self { active_connections }
    }
}

impl Drop for ActiveConnectionGuard {
    fn drop(&mut self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Handle to a running IPC server. Dropping shuts down the server.
pub struct IpcServerHandle {
    #[cfg(test)]
    pub addr: SocketAddr,
    pub port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
    conn_shutdown_tx: watch::Sender<bool>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
    connection_tasks: ConnectionTasks,
    connection_failed: Arc<AtomicBool>,
}

impl IpcServerHandle {
    /// Get the address the server is listening on.
    #[cfg(test)]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Shut down the server gracefully.
    ///
    /// Stops accepting new connections and signals all active connection
    /// handlers to close.
    pub fn shutdown(&mut self) {
        // Signal accept loop to stop
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Signal all connection handlers to close
        let _ = self.conn_shutdown_tx.send(true);
    }

    /// Observe listener and every accepted dispatch before owner release.
    /// The owning shutdown coordinator must retain this future through completion.
    pub(crate) async fn shutdown_and_wait(mut self) -> Result<()> {
        self.shutdown();
        let mut failures = Vec::new();
        if let Some(task) = self.task_handle.take() {
            if let Err(error) = task.await {
                failures.push(error.to_string());
            }
        }
        let tasks: Vec<_> = self
            .connection_tasks
            .lock()
            .map_err(|_| PumasError::Other("IPC task owner poisoned".into()))?
            .drain(..)
            .collect();
        for task in tasks {
            if let Err(error) = task.await {
                failures.push(error.to_string());
            }
        }
        if self.connection_failed.load(Ordering::Acquire) {
            failures.push("an accepted IPC dispatch panicked".into());
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(PumasError::Other(format!(
                "IPC cessation unconfirmed: {}",
                failures.join("; ")
            )))
        }
    }

    #[cfg(test)]
    fn tracked_connection_tasks(&self) -> usize {
        self.connection_tasks
            .lock()
            .expect("IPC connection task lock poisoned")
            .len()
    }
}

impl Drop for IpcServerHandle {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
        abort_connection_tasks(&self.connection_tasks);
    }
}

fn store_connection_task(tasks: &ConnectionTasks, handle: JoinHandle<()>) {
    let mut tasks = tasks.lock().expect("IPC connection task lock poisoned");
    tasks.retain(|handle| !handle.is_finished());
    tasks.push(handle);
}

fn abort_connection_tasks(tasks: &ConnectionTasks) {
    let handles: Vec<JoinHandle<()>> = tasks
        .lock()
        .expect("IPC connection task lock poisoned")
        .drain(..)
        .collect();

    for handle in handles {
        handle.abort();
    }
}

// Response delivery owns no remaining domain effect. Stop blocked transports
// without cancelling the dispatch whose result was already observed.
async fn deliver_or_stop<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    shutdown: &mut watch::Receiver<bool>,
) -> Result<bool> {
    if *shutdown.borrow() {
        return Ok(false);
    }
    tokio::select! {
        result = write_frame(writer, bytes) => { result?; Ok(true) },
        _ = shutdown.changed() => Ok(false),
    }
}

/// Trait for dispatching IPC method calls to the primary state.
///
/// Implemented by `PrimaryState` to handle incoming requests.
#[async_trait::async_trait]
pub(crate) trait IpcDispatch: Send + Sync + 'static {
    /// Dispatch a JSON-RPC method call and return the result.
    async fn dispatch(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, PumasError>;

    /// Optionally open a typed model-library update stream.
    async fn subscribe_model_library_update_stream_since(
        &self,
        _cursor: &str,
        _connection_token: Option<&str>,
    ) -> std::result::Result<Option<ModelLibraryUpdateSubscriber>, PumasError> {
        Ok(None)
    }
}

/// IPC server that listens for client connections.
pub(crate) struct IpcServer;

impl IpcServer {
    /// Start the IPC server on a random local port.
    ///
    /// Returns a handle that can be used to get the port and shut down the server.
    /// The server runs in background tokio tasks.
    pub async fn start<D: IpcDispatch>(dispatch: Arc<D>) -> Result<IpcServerHandle> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let port = addr.port();

        info!("IPC server listening on {}", addr);

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let (conn_shutdown_tx, conn_shutdown_rx) = watch::channel(false);
        let active_connections = Arc::new(AtomicUsize::new(0));
        let connection_tasks = Arc::new(Mutex::new(Vec::new()));
        let connection_failed = Arc::new(AtomicBool::new(false));

        let task_handle = tokio::spawn(Self::accept_loop(
            listener,
            dispatch,
            shutdown_rx,
            conn_shutdown_rx,
            active_connections,
            connection_tasks.clone(),
            connection_failed.clone(),
        ));

        Ok(IpcServerHandle {
            #[cfg(test)]
            addr,
            port,
            shutdown_tx: Some(shutdown_tx),
            conn_shutdown_tx,
            task_handle: Some(task_handle),
            connection_tasks,
            connection_failed,
        })
    }

    async fn accept_loop<D: IpcDispatch>(
        listener: TcpListener,
        dispatch: Arc<D>,
        mut shutdown_rx: oneshot::Receiver<()>,
        conn_shutdown_rx: watch::Receiver<bool>,
        active_connections: Arc<AtomicUsize>,
        connection_tasks: ConnectionTasks,
        connection_failed: Arc<AtomicBool>,
    ) {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    info!("IPC server shutting down");
                    break;
                }
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            let current = active_connections.load(Ordering::Relaxed);
                            if current >= RegistryConfig::MAX_IPC_CONNECTIONS {
                                warn!(
                                    "Rejecting IPC connection from {}: at max capacity ({})",
                                    peer_addr,
                                    RegistryConfig::MAX_IPC_CONNECTIONS
                                );
                                continue;
                            }

                            active_connections.fetch_add(1, Ordering::Relaxed);
                            let dispatch = dispatch.clone();
                            let connection_guard = ActiveConnectionGuard::new(active_connections.clone());
                            let mut conn_shutdown = conn_shutdown_rx.clone();
                            let failed = connection_failed.clone();

                            let handle = tokio::spawn(async move {
                                debug!("IPC connection from {}", peer_addr);
                                match AssertUnwindSafe(Self::handle_connection(stream, &*dispatch, &mut conn_shutdown)).catch_unwind().await {
                                    Ok(Ok(())) => {},
                                    Ok(Err(error)) => debug!("IPC connection {} ended: {}", peer_addr, error),
                                    Err(_) => { failed.store(true, Ordering::Release); },
                                }
                                drop(connection_guard);
                            });
                            store_connection_task(&connection_tasks, handle);
                        }
                        Err(e) => {
                            error!("IPC accept error: {}", e);
                        }
                    }
                }
            }
        }
    }

    async fn handle_connection<D: IpcDispatch>(
        mut stream: TcpStream,
        dispatch: &D,
        shutdown_rx: &mut watch::Receiver<bool>,
    ) -> Result<()> {
        let (mut reader, mut writer) = stream.split();

        loop {
            if *shutdown_rx.borrow() {
                return Ok(());
            }
            // Wait for either a frame or a shutdown signal
            let frame = tokio::select! {
                result = read_frame(&mut reader) => {
                    match result? {
                        Some(f) => f,
                        None => return Ok(()), // Clean disconnect
                    }
                }
                _ = shutdown_rx.changed() => {
                    return Ok(()); // Server shutting down
                }
            };

            let request = match Self::parse_request_frame(frame) {
                Ok(request) => request,
                Err(response) => {
                    let response_bytes = serde_json::to_vec(&response)?;
                    if !deliver_or_stop(&mut writer, &response_bytes, shutdown_rx).await? {
                        return Ok(());
                    }
                    continue;
                }
            };

            if LocalIpcOperation::from_wire_name(&request.method)
                == Some(LocalIpcOperation::SubscribeModelLibraryUpdateStreamSince)
            {
                match LocalIpcCommand::decode(
                    LocalIpcOperation::SubscribeModelLibraryUpdateStreamSince,
                    request.params,
                ) {
                    Ok(LocalIpcCommand::SubscribeModelLibraryUpdateStreamSince {
                        cursor,
                        connection_token,
                    }) => {
                        Self::handle_model_library_update_stream_request(
                            request.id,
                            &cursor,
                            &connection_token,
                            dispatch,
                            &mut writer,
                            shutdown_rx,
                        )
                        .await?;
                        return Ok(());
                    }
                    Ok(_) => unreachable!("stream operation decoded to a non-stream command"),
                    Err(error) => {
                        let response = IpcResponse::error(request.id, error);
                        let response_bytes = serde_json::to_vec(&response)?;
                        if !deliver_or_stop(&mut writer, &response_bytes, shutdown_rx).await? {
                            return Ok(());
                        }
                        continue;
                    }
                }
            }

            let response = Self::process_request(request, dispatch).await;

            if *shutdown_rx.borrow() {
                return Ok(());
            }
            let response_bytes = serde_json::to_vec(&response)?;
            if !deliver_or_stop(&mut writer, &response_bytes, shutdown_rx).await? {
                return Ok(());
            }
        }
    }

    fn parse_request_frame(frame: Vec<u8>) -> std::result::Result<IpcRequest, Box<IpcResponse>> {
        let request_value: serde_json::Value = serde_json::from_slice(&frame)
            .map_err(|_| Box::new(IpcResponse::error(None, IpcError::parse_error())))?;
        let request: IpcRequest = serde_json::from_value(request_value)
            .map_err(|_| Box::new(IpcResponse::error(None, IpcError::invalid_request())))?;

        if request.jsonrpc != "2.0"
            || request
                .id
                .as_ref()
                .and_then(serde_json::Value::as_u64)
                .is_none()
        {
            return Err(Box::new(IpcResponse::error(
                request.id,
                IpcError::invalid_request(),
            )));
        }

        Ok(request)
    }

    async fn process_request<D: IpcDispatch>(request: IpcRequest, dispatch: &D) -> IpcResponse {
        let Some(operation) = LocalIpcOperation::from_wire_name(&request.method) else {
            return IpcResponse::error(request.id, IpcError::method_not_found());
        };
        let command = match LocalIpcCommand::decode(operation, request.params) {
            Ok(command) => command,
            Err(error) => return IpcResponse::error(request.id, error),
        };
        let method = command.operation().wire_name();
        let params = command.into_dispatch_params();

        match dispatch.dispatch(method, params).await {
            Ok(result) => match operation.validate_outcome(result) {
                Ok(outcome) => IpcResponse::success(request.id, outcome),
                Err(error) => IpcResponse::error(request.id, error),
            },
            Err(error) => IpcResponse::error(request.id, IpcError::from_pumas(&error)),
        }
    }

    async fn handle_model_library_update_stream_request<D, W>(
        id: Option<serde_json::Value>,
        cursor: &str,
        connection_token: &str,
        dispatch: &D,
        writer: &mut W,
        shutdown_rx: &mut watch::Receiver<bool>,
    ) -> Result<()>
    where
        D: IpcDispatch,
        W: AsyncWriteExt + Unpin,
    {
        let mut subscriber = match dispatch
            .subscribe_model_library_update_stream_since(cursor, Some(connection_token))
            .await
        {
            Ok(Some(subscriber)) => subscriber,
            Ok(None) => {
                let response = IpcResponse::error(id, IpcError::method_not_found());
                let response_bytes = serde_json::to_vec(&response)?;
                if !deliver_or_stop(writer, &response_bytes, shutdown_rx).await? {
                    return Ok(());
                }
                return Ok(());
            }
            Err(error) => {
                let response = IpcResponse::error(id, IpcError::from_pumas(&error));
                let response_bytes = serde_json::to_vec(&response)?;
                if !deliver_or_stop(writer, &response_bytes, shutdown_rx).await? {
                    return Ok(());
                }
                return Ok(());
            }
        };

        let handshake = subscriber.handshake().clone();
        let response = IpcResponse::success(id.clone(), serde_json::to_value(&handshake)?);
        let response_bytes = serde_json::to_vec(&response)?;
        if !deliver_or_stop(writer, &response_bytes, shutdown_rx).await? {
            return Ok(());
        }

        if !handshake.live_stream_ready {
            return Ok(());
        }

        loop {
            if *shutdown_rx.borrow() {
                return Ok(());
            }
            let update_result = tokio::select! {
                result = subscriber.next_event() => result,
                _ = shutdown_rx.changed() => return Ok(()),
            };
            let update = match update_result {
                Ok(update) => update,
                Err(error) => {
                    let response = IpcResponse::error(id, IpcError::from_pumas(&error));
                    let response_bytes = serde_json::to_vec(&response)?;
                    if !deliver_or_stop(writer, &response_bytes, shutdown_rx).await? {
                        return Ok(());
                    }
                    return Ok(());
                }
            };
            let notification = ModelLibraryUpdateNotification {
                cursor: update.cursor.clone(),
                events: vec![update],
                stale_cursor: false,
                snapshot_required: false,
            };
            let response = IpcResponse::success(id.clone(), serde_json::to_value(notification)?);
            let response_bytes = serde_json::to_vec(&response)?;
            if !deliver_or_stop(writer, &response_bytes, shutdown_rx).await? {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use tokio::time::{timeout, Duration};

    struct ContractDispatch;

    #[async_trait::async_trait]
    impl IpcDispatch for ContractDispatch {
        async fn dispatch(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> std::result::Result<serde_json::Value, PumasError> {
            assert_eq!(method, "model_library_selector_snapshot");
            assert_eq!(params["connection_token"], "test-connection-token");
            match params["request"]["search"].as_str() {
                Some("trigger-failure") => Err(PumasError::Other(
                    "private dispatch failure at /private/model".to_string(),
                )),
                Some("trigger-wrong-outcome") => Ok(serde_json::json!({ "wrong": true })),
                _ => Ok(serde_json::to_value(
                    crate::models::ModelLibrarySelectorSnapshot::empty("model-library-updates:0"),
                )?),
            }
        }
    }

    async fn start_test_server() -> Option<IpcServerHandle> {
        match IpcServer::start(Arc::new(ContractDispatch)).await {
            Ok(handle) => Some(handle),
            Err(PumasError::Io {
                source: Some(err), ..
            }) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping IPC socket test: {}", err);
                None
            }
            Err(err) => panic!("failed to start IPC server: {}", err),
        }
    }

    #[tokio::test]
    async fn test_server_start_and_shutdown() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        assert!(handle.port > 0);
        assert_eq!(handle.addr.ip(), std::net::Ipv4Addr::LOCALHOST);

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_closed_contract_roundtrip() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        // Connect as a client
        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();

        let request = IpcRequest::new(
            LocalIpcOperation::ModelLibrarySelectorSnapshot,
            serde_json::json!({
                "request": { "limit": 25 },
                "connection_token": "test-connection-token",
            }),
            1,
        );
        let request_bytes = serde_json::to_vec(&request).unwrap();
        write_frame(&mut writer, &request_bytes).await.unwrap();

        // Read response
        let response_bytes = read_frame(&mut reader).await.unwrap().unwrap();
        let response: IpcResponse = serde_json::from_slice(&response_bytes).unwrap();

        assert!(response.error.is_none());
        let snapshot: crate::models::ModelLibrarySelectorSnapshot =
            serde_json::from_value(response.result.unwrap()).unwrap();
        assert_eq!(snapshot.cursor, "model-library-updates:0");

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_error_response() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();

        let request = IpcRequest::new(
            LocalIpcOperation::ModelLibrarySelectorSnapshot,
            serde_json::json!({
                "request": { "search": "trigger-failure" },
                "connection_token": "test-connection-token",
            }),
            2,
        );
        let request_bytes = serde_json::to_vec(&request).unwrap();
        write_frame(&mut writer, &request_bytes).await.unwrap();

        let response_bytes = read_frame(&mut reader).await.unwrap().unwrap();
        let response: IpcResponse = serde_json::from_slice(&response_bytes).unwrap();

        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, -32603); // Internal error
        assert_eq!(err.message, "Local IPC operation failed");
        let encoded = serde_json::to_string(&err).unwrap();
        assert!(!encoded.contains("private dispatch failure"));
        assert!(!encoded.contains("/private/model"));

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_rejects_unknown_operation_before_dispatch() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };
        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();
        let request = IpcRequest::new_unchecked("list_models", serde_json::json!({}), 3);
        write_frame(&mut writer, &serde_json::to_vec(&request).unwrap())
            .await
            .unwrap();
        let response: IpcResponse =
            serde_json::from_slice(&read_frame(&mut reader).await.unwrap().unwrap()).unwrap();

        assert_eq!(response.error.unwrap().code, -32601);
        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_rejects_extra_fields_and_oversized_values() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };
        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();

        for (id, params) in [
            (
                4,
                serde_json::json!({
                    "request": { "limit": 25 },
                    "connection_token": "test-connection-token",
                    "unexpected": true,
                }),
            ),
            (
                5,
                serde_json::json!({
                    "request": { "limit": 1001 },
                    "connection_token": "test-connection-token",
                }),
            ),
            (
                6,
                serde_json::json!({
                    "request": { "offset": -1 },
                    "connection_token": "test-connection-token",
                }),
            ),
        ] {
            let request =
                IpcRequest::new(LocalIpcOperation::ModelLibrarySelectorSnapshot, params, id);
            write_frame(&mut writer, &serde_json::to_vec(&request).unwrap())
                .await
                .unwrap();
            let response: IpcResponse =
                serde_json::from_slice(&read_frame(&mut reader).await.unwrap().unwrap()).unwrap();
            assert_eq!(response.error.unwrap().code, -32602);
        }

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_rejects_wrong_dispatch_outcome_type() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };
        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();
        let request = IpcRequest::new(
            LocalIpcOperation::ModelLibrarySelectorSnapshot,
            serde_json::json!({
                "request": { "search": "trigger-wrong-outcome" },
                "connection_token": "test-connection-token",
            }),
            7,
        );
        write_frame(&mut writer, &serde_json::to_vec(&request).unwrap())
            .await
            .unwrap();
        let response: IpcResponse =
            serde_json::from_slice(&read_frame(&mut reader).await.unwrap().unwrap()).unwrap();

        assert_eq!(response.error.unwrap().code, -32603);
        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_invalid_json_returns_parse_error() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();

        // Send invalid JSON
        write_frame(&mut writer, b"not valid json").await.unwrap();

        let response_bytes = read_frame(&mut reader).await.unwrap().unwrap();
        let response: IpcResponse = serde_json::from_slice(&response_bytes).unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32700);

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_distinguishes_invalid_envelope_from_invalid_params() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };
        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();
        let cases = [
            (
                serde_json::json!({
                    "jsonrpc": "1.0",
                    "method": "model_library_selector_snapshot",
                    "params": {
                        "request": {},
                        "connection_token": "test-connection-token",
                    },
                    "id": 8,
                }),
                -32600,
            ),
            (
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "model_library_selector_snapshot",
                    "params": {
                        "request": {},
                        "connection_token": "test-connection-token",
                    },
                    "id": 9,
                    "unexpected": true,
                }),
                -32600,
            ),
            (
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "model_library_selector_snapshot",
                    "params": null,
                    "id": 10,
                }),
                -32602,
            ),
        ];

        for (request, expected_code) in cases {
            write_frame(&mut writer, &serde_json::to_vec(&request).unwrap())
                .await
                .unwrap();
            let response: IpcResponse =
                serde_json::from_slice(&read_frame(&mut reader).await.unwrap().unwrap()).unwrap();
            assert_eq!(response.error.unwrap().code, expected_code);
        }

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_drop_aborts_tracked_connection_tasks() {
        let Some(handle) = start_test_server().await else {
            return;
        };

        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        timeout(Duration::from_secs(1), async {
            loop {
                if handle.tracked_connection_tasks() > 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("connection task should be tracked");

        drop(handle);

        let read_result = timeout(Duration::from_secs(1), read_frame(&mut stream)).await;
        assert!(read_result.is_ok(), "connection should close after drop");
    }
}

#[cfg(test)]
mod shutdown_receipt_tests {
    use super::*;
    use crate::ipc::protocol::LocalIpcOperation;
    use crate::ipc::IpcClient;
    use std::sync::atomic::AtomicBool;

    struct GatedDispatch {
        started: Mutex<Option<oneshot::Sender<()>>>,
        release: tokio::sync::Notify,
        settled: AtomicBool,
    }
    #[async_trait::async_trait]
    impl IpcDispatch for GatedDispatch {
        async fn dispatch(&self, _: &str, _: serde_json::Value) -> Result<serde_json::Value> {
            self.started
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(())
                .unwrap();
            self.release.notified().await;
            self.settled.store(true, Ordering::Release);
            Ok(serde_json::to_value(
                crate::models::ModelLibrarySelectorSnapshot::empty("fixture"),
            )?)
        }
    }

    #[tokio::test]
    async fn ipc_cessation_waits_for_admitted_dispatch_and_closes_its_connection() {
        let (started_tx, started_rx) = oneshot::channel();
        let dispatch = Arc::new(GatedDispatch {
            started: Mutex::new(Some(started_tx)),
            release: tokio::sync::Notify::new(),
            settled: AtomicBool::new(false),
        });
        let server = IpcServer::start(dispatch.clone()).await.unwrap();
        let address = server.addr();
        let client = IpcClient::connect(address, std::process::id())
            .await
            .unwrap();
        let request = tokio::spawn(async move {
            client
                .call(
                    LocalIpcOperation::ModelLibrarySelectorSnapshot,
                    serde_json::json!({"request": {}, "connection_token": "fixture"}),
                )
                .await
        });
        started_rx.await.unwrap();
        let mut shutdown = tokio::spawn(server.shutdown_and_wait());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), &mut shutdown)
                .await
                .is_err()
        );
        assert!(!dispatch.settled.load(Ordering::Acquire));
        dispatch.release.notify_one();
        shutdown.await.unwrap().unwrap();
        assert!(dispatch.settled.load(Ordering::Acquire));
        assert!(request.await.unwrap().is_err());
        assert!(TcpStream::connect(address).await.is_err());
    }
}
