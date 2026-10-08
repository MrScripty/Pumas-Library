//! TCP IPC client for connecting to a primary pumas-core instance.
//!
//! Establishes a TCP connection to the primary's IPC server and provides
//! a `call()` method for transparent JSON-RPC method invocation.
//!
//! # Thread Safety
//!
//! A client-owned worker serializes exchanges and retains the TCP stream when
//! a caller stops waiting. Queued calls whose callers disappear are skipped.

use super::protocol::{read_frame, write_frame, IpcRequest, IpcResponse, LocalIpcOperation};
#[cfg(test)]
use super::protocol::{read_frame_blocking, write_frame_blocking};
use crate::config::RegistryConfig;
use crate::{PumasError, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::debug;

/// IPC client that connects to a primary instance's server.
#[derive(Debug)]
pub(crate) struct IpcClient {
    exchanges: mpsc::Sender<Exchange>,
    worker: JoinHandle<()>,
    #[cfg(test)]
    addr: SocketAddr,
    next_id: AtomicU64,
    /// PID of the primary instance (for error reporting).
    pub primary_pid: u32,
    /// Port of the primary instance (for error reporting).
    pub primary_port: u16,
}

#[derive(Debug)]
struct Exchange {
    id: u64,
    bytes: Vec<u8>,
    reply: oneshot::Sender<Result<serde_json::Value>>,
}

impl Drop for IpcClient {
    fn drop(&mut self) {
        // Final-owner disposal requests socket closure, not server cancellation.
        // Drop cannot await the worker; its stream closes when abort is polled.
        self.worker.abort();
    }
}

impl IpcClient {
    /// Connect to a primary instance's IPC server.
    ///
    /// Uses the configured connection timeout from `RegistryConfig`.
    pub(crate) async fn connect(addr: SocketAddr, pid: u32) -> Result<Self> {
        let stream = tokio::time::timeout(
            RegistryConfig::IPC_CONNECT_TIMEOUT,
            TcpStream::connect(addr),
        )
        .await
        .map_err(|_| PumasError::SharedInstanceLost {
            pid,
            port: addr.port(),
        })?
        .map_err(|_| PumasError::SharedInstanceLost {
            pid,
            port: addr.port(),
        })?;

        debug!("IPC client connected to {} (PID {})", addr, pid);

        Ok(Self::from_stream(stream, addr, pid))
    }

    fn from_stream<S>(stream: S, addr: SocketAddr, pid: u32) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        // One queued exchange; further callers wait in cancellation-safe send.
        let (exchanges, receiver) = mpsc::channel(1);
        let worker = tokio::spawn(run_exchanges(stream, receiver, pid, addr.port()));
        Self {
            exchanges,
            worker,
            #[cfg(test)]
            addr,
            next_id: AtomicU64::new(1),
            primary_pid: pid,
            primary_port: addr.port(),
        }
    }

    /// Call a JSON-RPC method on the primary instance.
    ///
    /// Returns the result value on success, or a `PumasError` on failure.
    /// If the connection is broken, returns `SharedInstanceLost`.
    ///
    /// Dropping this future skips a queued call. Once admitted, the client owns
    /// the full exchange until the original reply is drained and validated.
    /// A silent peer can retain that custody indefinitely; there is no read
    /// timeout or automatic replay of an uncertain operation.
    pub(crate) async fn call(
        &self,
        operation: LocalIpcOperation,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = IpcRequest::new(operation, params, id);
        let request_bytes = serde_json::to_vec(&request)?;

        let (reply, result) = oneshot::channel();
        self.exchanges
            .send(Exchange {
                id,
                bytes: request_bytes,
                reply,
            })
            .await
            .map_err(|_| self.connection_lost())?;
        result.await.map_err(|_| self.connection_lost())?
    }

    fn connection_lost(&self) -> PumasError {
        PumasError::SharedInstanceLost {
            pid: self.primary_pid,
            port: self.primary_port,
        }
    }

    /// Call a JSON-RPC method on the primary instance using a fresh blocking socket.
    #[cfg(test)]
    pub fn call_blocking(
        &self,
        operation: LocalIpcOperation,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = IpcRequest::new(operation, params, id);
        let request_bytes = serde_json::to_vec(&request)?;

        let mut stream =
            std::net::TcpStream::connect_timeout(&self.addr, RegistryConfig::IPC_CONNECT_TIMEOUT)
                .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?;
        stream
            .set_nodelay(true)
            .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?;
        stream
            .set_read_timeout(Some(RegistryConfig::PRIMARY_READY_TIMEOUT))
            .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?;
        stream
            .set_write_timeout(Some(RegistryConfig::PRIMARY_READY_TIMEOUT))
            .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?;

        write_frame_blocking(&mut stream, &request_bytes).map_err(|_| {
            PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            }
        })?;

        let response_bytes = read_frame_blocking(&mut stream)
            .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?
            .ok_or(PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?;

        let response: IpcResponse =
            serde_json::from_slice(&response_bytes).map_err(invalid_response)?;
        response.into_result(id)
    }
}

async fn run_exchanges<S>(
    mut stream: S,
    mut exchanges: mpsc::Receiver<Exchange>,
    pid: u32,
    port: u16,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(exchange) = exchanges.recv().await {
        // Admission boundary: cancellation observed here has no wire effect.
        // After this check, receiver closure must not interrupt framing or read.
        if exchange.reply.is_closed() {
            continue;
        }
        let response = async {
            let lost = || PumasError::SharedInstanceLost { pid, port };
            write_frame(&mut stream, &exchange.bytes)
                .await
                .map_err(|_| lost())?;
            let bytes = read_frame(&mut stream)
                .await
                .map_err(|_| lost())?
                .ok_or_else(lost)?;
            let response: IpcResponse = serde_json::from_slice(&bytes).map_err(invalid_response)?;
            response.validate_envelope(exchange.id)?;
            Ok(response)
        }
        .await;

        match response {
            Ok(response) => {
                // A valid RPC error is an ordinary completed exchange.
                let _ = exchange.reply.send(response.into_result(exchange.id));
            }
            Err(error) => {
                let _ = exchange.reply.send(Err(error));
                // No resynchronization/retry after partial IO or incoherent data.
                // Drop the stream and queue; subsequent calls report owner loss.
                break;
            }
        }
    }
}

fn invalid_response(error: serde_json::Error) -> PumasError {
    PumasError::Json {
        message: "Invalid local IPC response envelope".to_string(),
        source: Some(error),
    }
}

#[cfg(test)]
#[path = "client/cancellation_tests.rs"]
mod cancellation_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::server::{IpcDispatch, IpcServer, IpcServerHandle};
    use std::io::ErrorKind;
    use std::sync::Arc;

    struct TestDispatch;

    #[async_trait::async_trait]
    impl IpcDispatch for TestDispatch {
        async fn dispatch(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> std::result::Result<serde_json::Value, PumasError> {
            assert_eq!(method, "model_library_selector_snapshot");
            assert_eq!(params["connection_token"], "test-token");
            Ok(serde_json::to_value(
                crate::models::ModelLibrarySelectorSnapshot::empty("model-library-updates:0"),
            )?)
        }
    }

    async fn start_test_server() -> Option<IpcServerHandle> {
        match IpcServer::start(Arc::new(TestDispatch)).await {
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
    async fn test_client_call_success() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        let result = client
            .call(
                LocalIpcOperation::ModelLibrarySelectorSnapshot,
                serde_json::json!({
                    "request": { "limit": 25 },
                    "connection_token": "test-token",
                }),
            )
            .await
            .unwrap();
        let snapshot: crate::models::ModelLibrarySelectorSnapshot =
            serde_json::from_value(result).unwrap();
        assert_eq!(snapshot.cursor, "model-library-updates:0");

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_client_call_rejects_invalid_params() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        let result = client
            .call(
                LocalIpcOperation::ModelLibrarySelectorSnapshot,
                serde_json::json!({
                    "request": { "limit": -1 },
                    "connection_token": "test-token",
                }),
            )
            .await;
        assert!(matches!(
            result,
            Err(PumasError::InvalidParams { message })
                if message == "Invalid local IPC parameters"
        ));

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_client_call_blocking_success() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        let result = tokio::task::spawn_blocking(move || {
            client.call_blocking(
                LocalIpcOperation::ModelLibrarySelectorSnapshot,
                serde_json::json!({
                    "request": { "limit": 25 },
                    "connection_token": "test-token",
                }),
            )
        })
        .await
        .unwrap()
        .unwrap();
        let snapshot: crate::models::ModelLibrarySelectorSnapshot =
            serde_json::from_value(result).unwrap();
        assert_eq!(snapshot.cursor, "model-library-updates:0");

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_client_call_error_returns_err() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        let result = client
            .call(
                LocalIpcOperation::ModelLibrarySelectorSnapshot,
                serde_json::json!({}),
            )
            .await;
        assert!(result.is_err());

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_client_connect_to_dead_server_returns_shared_instance_lost() {
        // Use a port that nothing is listening on
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let result = IpcClient::connect(addr, 999_999).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            PumasError::SharedInstanceLost { pid, port } => {
                assert_eq!(pid, 999_999);
                assert_eq!(port, 1);
            }
            other => panic!("Expected SharedInstanceLost, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_client_detects_server_shutdown() {
        let Some(mut handle) = start_test_server().await else {
            return;
        };

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        // Verify it works first
        let params = serde_json::json!({
            "request": { "limit": 25 },
            "connection_token": "test-token",
        });
        let result = client
            .call(
                LocalIpcOperation::ModelLibrarySelectorSnapshot,
                params.clone(),
            )
            .await;
        assert!(result.is_ok());

        // Shut down the server
        handle.shutdown();

        // Retry until the server is fully closed (up to 1s)
        let mut detected_shutdown = false;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let result = client
                .call(
                    LocalIpcOperation::ModelLibrarySelectorSnapshot,
                    params.clone(),
                )
                .await;
            if result.is_err() {
                detected_shutdown = true;
                break;
            }
        }
        assert!(detected_shutdown, "Client should detect server shutdown");
    }
}
