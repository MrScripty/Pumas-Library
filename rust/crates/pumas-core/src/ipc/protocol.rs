//! Shared IPC protocol types and framing.
//!
//! Defines the wire format for local IPC: 4-byte big-endian length prefix
//! followed by a UTF-8 JSON-RPC 2.0 payload.
//!
//! ```text
//! [u32 BE: len][UTF-8 JSON bytes of len]
//! ```

use crate::config::RegistryConfig;
use crate::{PumasError, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// JSON-RPC 2.0 request for IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

impl IpcRequest {
    /// Create a new JSON-RPC 2.0 request.
    pub fn new(method: impl Into<String>, params: serde_json::Value, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params: Some(params),
            id: Some(serde_json::Value::Number(id.into())),
        }
    }
}

/// JSON-RPC 2.0 response for IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<IpcError>,
    pub id: Option<serde_json::Value>,
}

impl IpcResponse {
    /// Create a success response.
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(IpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Read a length-prefixed frame from an async reader.
///
/// Frame format: `[4-byte BE u32 length][payload bytes]`
///
/// Returns `None` on clean EOF (peer closed connection).
pub async fn read_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }

    let len = u32::from_be_bytes(len_buf) as usize;

    if len > RegistryConfig::MAX_IPC_MESSAGE_SIZE {
        return Err(PumasError::Validation {
            field: "ipc_frame".to_string(),
            message: format!(
                "IPC message size {} exceeds maximum {}",
                len,
                RegistryConfig::MAX_IPC_MESSAGE_SIZE
            ),
        });
    }

    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;

    Ok(Some(payload))
}

/// Write a length-prefixed frame to an async writer.
///
/// Frame format: `[4-byte BE u32 length][payload bytes]`
pub async fn write_frame<W: AsyncWriteExt + Unpin>(writer: &mut W, payload: &[u8]) -> Result<()> {
    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_request_serialization_roundtrip() {
        let req = IpcRequest::new("list_models", serde_json::json!({}), 1);
        let json = serde_json::to_string(&req).unwrap();
        let parsed: IpcRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.method, "list_models");
        assert_eq!(parsed.id, Some(serde_json::Value::Number(1.into())));
    }

    #[test]
    fn test_ipc_response_success_serialization() {
        let resp = IpcResponse::success(
            Some(serde_json::Value::Number(1.into())),
            serde_json::json!({"models": []}),
        );
        let json = serde_json::to_string(&resp).unwrap();

        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_ipc_response_error_serialization() {
        let resp = IpcResponse::error(
            Some(serde_json::Value::Number(1.into())),
            -32603,
            "Internal error".to_string(),
        );
        let json = serde_json::to_string(&resp).unwrap();

        assert!(!json.contains("\"result\""));
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32603"));
    }

    #[tokio::test]
    async fn test_frame_read_write_roundtrip() {
        let payload = b"hello world";
        let mut buf = Vec::new();

        write_frame(&mut buf, payload).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let read_back = read_frame(&mut cursor).await.unwrap();

        assert_eq!(read_back, Some(payload.to_vec()));
    }

    #[tokio::test]
    async fn test_frame_read_empty_stream_returns_none() {
        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        let result = read_frame(&mut cursor).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_frame_read_oversized_returns_error() {
        // Craft a frame header claiming a huge payload
        let huge_len: u32 = (RegistryConfig::MAX_IPC_MESSAGE_SIZE + 1) as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&huge_len.to_be_bytes());
        buf.extend_from_slice(&[0u8; 8]); // some bytes but not enough

        let mut cursor = std::io::Cursor::new(buf);
        let result = read_frame(&mut cursor).await;
        assert!(result.is_err());
    }
}
