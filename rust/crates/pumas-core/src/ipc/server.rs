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

use super::protocol::{read_frame, write_frame, IpcRequest, IpcResponse};
use crate::config::RegistryConfig;
use crate::{PumasError, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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
    pub addr: SocketAddr,
    pub port: u16,
    shutdown_tx: Option<oneshot::Sender<()>>,
    conn_shutdown_tx: watch::Sender<bool>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
    connection_tasks: ConnectionTasks,
}

impl IpcServerHandle {
    /// Get the address the server is listening on.
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

/// Trait for dispatching IPC method calls to the primary state.
///
/// Implemented by `PrimaryState` to handle incoming requests.
#[async_trait::async_trait]
pub trait IpcDispatch: Send + Sync + 'static {
    /// Dispatch a JSON-RPC method call and return the result.
    async fn dispatch(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> std::result::Result<serde_json::Value, PumasError>;
}

/// IPC server that listens for client connections.
pub struct IpcServer;

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

        let task_handle = tokio::spawn(Self::accept_loop(
            listener,
            dispatch,
            shutdown_rx,
            conn_shutdown_rx,
            active_connections,
            connection_tasks.clone(),
        ));

        Ok(IpcServerHandle {
            addr,
            port,
            shutdown_tx: Some(shutdown_tx),
            conn_shutdown_tx,
            task_handle: Some(task_handle),
            connection_tasks,
        })
    }

    async fn accept_loop<D: IpcDispatch>(
        listener: TcpListener,
        dispatch: Arc<D>,
        mut shutdown_rx: oneshot::Receiver<()>,
        conn_shutdown_rx: watch::Receiver<bool>,
        active_connections: Arc<AtomicUsize>,
        connection_tasks: ConnectionTasks,
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

                            let handle = tokio::spawn(async move {
                                debug!("IPC connection from {}", peer_addr);
                                if let Err(e) = Self::handle_connection(stream, &*dispatch, &mut conn_shutdown).await {
                                    debug!("IPC connection {} ended: {}", peer_addr, e);
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

            let request_str = String::from_utf8(frame).map_err(|_| PumasError::Validation {
                field: "ipc_payload".to_string(),
                message: "Invalid UTF-8 in IPC frame".to_string(),
            })?;

            let response = Self::process_request(&request_str, dispatch).await;

            let response_bytes = serde_json::to_vec(&response)?;
            write_frame(&mut writer, &response_bytes).await?;
        }
    }

    async fn process_request<D: IpcDispatch>(request_str: &str, dispatch: &D) -> IpcResponse {
        let request: IpcRequest = match serde_json::from_str(request_str) {
            Ok(req) => req,
            Err(e) => {
                return IpcResponse::error(None, -32700, format!("Parse error: {}", e));
            }
        };

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return IpcResponse::error(
                request.id,
                -32600,
                "Invalid Request: expected jsonrpc 2.0".to_string(),
            );
        }

        let params = request
            .params
            .unwrap_or(serde_json::Value::Object(Default::default()));

        match dispatch.dispatch(&request.method, params).await {
            Ok(result) => IpcResponse::success(request.id, result),
            Err(e) => {
                let code = e.to_rpc_error_code();
                IpcResponse::error(request.id, code, e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use tokio::time::{timeout, Duration};

    struct EchoDispatch;

    #[async_trait::async_trait]
    impl IpcDispatch for EchoDispatch {
        async fn dispatch(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> std::result::Result<serde_json::Value, PumasError> {
            match method {
                "echo" => Ok(params),
                "fail" => Err(PumasError::Other("test failure".to_string())),
                _ => Err(PumasError::InvalidParams {
                    message: format!("Unknown method: {}", method),
                }),
            }
        }
    }

    async fn start_test_server() -> Option<IpcServerHandle> {
        match IpcServer::start(Arc::new(EchoDispatch)).await {
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
    async fn test_server_echo_roundtrip() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        // Connect as a client
        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();

        // Send a request
        let request = IpcRequest::new("echo", serde_json::json!({"hello": "world"}), 1);
        let request_bytes = serde_json::to_vec(&request).unwrap();
        write_frame(&mut writer, &request_bytes).await.unwrap();

        // Read response
        let response_bytes = read_frame(&mut reader).await.unwrap().unwrap();
        let response: IpcResponse = serde_json::from_slice(&response_bytes).unwrap();

        assert!(response.error.is_none());
        assert_eq!(response.result, Some(serde_json::json!({"hello": "world"})));

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_server_error_response() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let mut stream = TcpStream::connect(handle.addr()).await.unwrap();
        let (mut reader, mut writer) = stream.split();

        let request = IpcRequest::new("fail", serde_json::json!({}), 2);
        let request_bytes = serde_json::to_vec(&request).unwrap();
        write_frame(&mut writer, &request_bytes).await.unwrap();

        let response_bytes = read_frame(&mut reader).await.unwrap().unwrap();
        let response: IpcResponse = serde_json::from_slice(&response_bytes).unwrap();

        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, -32603); // Internal error
        assert!(err.message.contains("test failure"));

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
