//! TCP IPC client for connecting to a primary pumas-core instance.
//!
//! Establishes a TCP connection to the primary's IPC server and provides
//! a `call()` method for transparent JSON-RPC method invocation.
//!
//! # Thread Safety
//!
//! The client uses a tokio `Mutex` to serialize access to the TCP stream,
//! allowing safe concurrent use from multiple async tasks.

use super::protocol::{read_frame, write_frame, IpcRequest, IpcResponse};
use crate::config::RegistryConfig;
use crate::{PumasError, Result};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::debug;

/// IPC client that connects to a primary instance's server.
#[derive(Debug)]
pub struct IpcClient {
    stream: Mutex<TcpStream>,
    addr: SocketAddr,
    next_id: AtomicU64,
    /// PID of the primary instance (for error reporting).
    pub primary_pid: u32,
    /// Port of the primary instance (for error reporting).
    pub primary_port: u16,
}

impl IpcClient {
    /// Connect to a primary instance's IPC server.
    ///
    /// Uses the configured connection timeout from `RegistryConfig`.
    pub async fn connect(addr: SocketAddr, pid: u32) -> Result<Self> {
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

        Ok(Self {
            stream: Mutex::new(stream),
            addr,
            next_id: AtomicU64::new(1),
            primary_pid: pid,
            primary_port: addr.port(),
        })
    }

    /// Call a JSON-RPC method on the primary instance.
    ///
    /// Returns the result value on success, or a `PumasError` on failure.
    /// If the connection is broken, returns `SharedInstanceLost`.
    pub async fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = IpcRequest::new(method, params, id);
        let request_bytes = serde_json::to_vec(&request)?;

        let mut stream = self.stream.lock().await;
        let (mut reader, mut writer) = stream.split();

        // Send request
        write_frame(&mut writer, &request_bytes)
            .await
            .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?;

        // Read response
        let response_bytes = read_frame(&mut reader)
            .await
            .map_err(|_| PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?
            .ok_or(PumasError::SharedInstanceLost {
                pid: self.primary_pid,
                port: self.primary_port,
            })?;

        let response: IpcResponse =
            serde_json::from_slice(&response_bytes).map_err(|e| PumasError::Json {
                message: format!("Failed to parse IPC response: {}", e),
                source: Some(e),
            })?;

        if let Some(err) = response.error {
            return Err(PumasError::Other(err.message));
        }

        response
            .result
            .ok_or_else(|| PumasError::Other("IPC response missing result".to_string()))
    }

    /// Get the address of the connected primary instance.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::server::{IpcDispatch, IpcServer};
    use std::sync::Arc;

    struct TestDispatch;

    #[async_trait::async_trait]
    impl IpcDispatch for TestDispatch {
        async fn dispatch(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> std::result::Result<serde_json::Value, PumasError> {
            match method {
                "ping" => Ok(serde_json::json!("pong")),
                "add" => {
                    let a = params["a"].as_i64().unwrap_or(0);
                    let b = params["b"].as_i64().unwrap_or(0);
                    Ok(serde_json::json!(a + b))
                }
                _ => Err(PumasError::InvalidParams {
                    message: format!("Unknown method: {}", method),
                }),
            }
        }
    }

    #[tokio::test]
    async fn test_client_call_success() {
        let dispatch = Arc::new(TestDispatch);
        let mut handle = IpcServer::start(dispatch).await.unwrap();

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        let result = client.call("ping", serde_json::json!({})).await.unwrap();
        assert_eq!(result, serde_json::json!("pong"));

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_client_call_with_params() {
        let dispatch = Arc::new(TestDispatch);
        let mut handle = IpcServer::start(dispatch).await.unwrap();

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        let result = client
            .call("add", serde_json::json!({"a": 3, "b": 4}))
            .await
            .unwrap();
        assert_eq!(result, serde_json::json!(7));

        handle.shutdown();
    }

    #[tokio::test]
    async fn test_client_call_error_returns_err() {
        let dispatch = Arc::new(TestDispatch);
        let mut handle = IpcServer::start(dispatch).await.unwrap();

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        let result = client.call("nonexistent", serde_json::json!({})).await;
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
        let dispatch = Arc::new(TestDispatch);
        let mut handle = IpcServer::start(dispatch).await.unwrap();

        let client = IpcClient::connect(handle.addr(), std::process::id())
            .await
            .unwrap();

        // Verify it works first
        let result = client.call("ping", serde_json::json!({})).await;
        assert!(result.is_ok());

        // Shut down the server
        handle.shutdown();

        // Retry until the server is fully closed (up to 1s)
        let mut detected_shutdown = false;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let result = client.call("ping", serde_json::json!({})).await;
            if result.is_err() {
                detected_shutdown = true;
                break;
            }
        }
        assert!(detected_shutdown, "Client should detect server shutdown");
    }
}
