//! HTTP server implementation using Axum.

use crate::handler::{handle_health, handle_rpc};
use axum::{
    routing::{get, post},
    Router,
};
use pumas_core::PumasApi;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// Application state shared across handlers.
pub struct AppState {
    pub api: PumasApi,
}

/// Start the JSON-RPC HTTP server.
///
/// Returns the actual address the server is bound to (useful when port=0).
pub async fn start_server(
    api: PumasApi,
    host: &str,
    port: u16,
) -> anyhow::Result<SocketAddr> {
    let state = Arc::new(AppState { api });

    // Configure CORS for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/rpc", post(handle_rpc))
        .layer(cors)
        .with_state(state);

    // Parse the address
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    // Bind to the address
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    info!("Server listening on {}", actual_addr);

    // Spawn the server in the background
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("Server error");
    });

    Ok(actual_addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_starts() {
        let temp_dir = TempDir::new().unwrap();
        let api = PumasApi::new(temp_dir.path()).await.unwrap();

        let addr = start_server(api, "127.0.0.1", 0).await.unwrap();
        assert!(addr.port() > 0);
    }
}
