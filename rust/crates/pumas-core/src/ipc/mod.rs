//! Local IPC for instance convergence.
//!
//! Provides a lightweight TCP-based IPC mechanism for transparent communication
//! between primary and client pumas-core instances. Uses length-prefixed JSON-RPC 2.0
//! over `127.0.0.1` TCP connections.
//!
//! # Architecture
//!
//! - **Server**: Runs on the primary instance, accepts connections, dispatches method calls
//! - **Client**: Connects to a primary instance, proxies API calls transparently
//! - **Protocol**: Shared framing and JSON-RPC types used by both

pub mod client;
pub mod protocol;
pub mod server;

pub use client::IpcClient;
pub use protocol::{IpcRequest, IpcResponse};
pub use server::{IpcServer, IpcServerHandle};
