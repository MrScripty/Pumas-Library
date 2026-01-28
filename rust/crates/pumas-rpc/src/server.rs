//! HTTP server implementation using Axum.

use crate::handler::{handle_health, handle_rpc};
use axum::{
    routing::{get, post},
    Router,
};
use pumas_app_manager::{CustomNodesManager, VersionManager, SizeCalculator};
use pumas_core::PumasApi;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// Application state shared across handlers.
pub struct AppState {
    /// Core API (model library, system utilities)
    pub api: PumasApi,
    /// Version manager for ComfyUI (from pumas-app-manager)
    pub version_manager: Arc<RwLock<Option<VersionManager>>>,
    /// Custom nodes manager (from pumas-app-manager)
    pub custom_nodes_manager: Arc<CustomNodesManager>,
    /// Size calculator for release size estimates
    pub size_calculator: Arc<RwLock<SizeCalculator>>,
    /// Launcher root directory
    pub launcher_root: PathBuf,
}

/// Start the JSON-RPC HTTP server.
///
/// Returns the actual address the server is bound to (useful when port=0).
pub async fn start_server(
    api: PumasApi,
    version_manager: Option<VersionManager>,
    custom_nodes_manager: CustomNodesManager,
    size_calculator: SizeCalculator,
    launcher_root: PathBuf,
    host: &str,
    port: u16,
) -> anyhow::Result<SocketAddr> {
    let state = Arc::new(AppState {
        api,
        version_manager: Arc::new(RwLock::new(version_manager)),
        custom_nodes_manager: Arc::new(custom_nodes_manager),
        size_calculator: Arc::new(RwLock::new(size_calculator)),
        launcher_root,
    });

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
    use pumas_core::AppId;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_starts() {
        let temp_dir = TempDir::new().unwrap();
        let launcher_root = temp_dir.path().to_path_buf();
        let api = PumasApi::new(&launcher_root).await.unwrap();

        // Initialize version manager (may fail if directory doesn't exist, which is fine for test)
        let version_manager = VersionManager::new(&launcher_root, AppId::ComfyUI).await.ok();

        // Initialize custom nodes manager
        let versions_dir = launcher_root.join(AppId::ComfyUI.versions_dir_name());
        let custom_nodes_manager = CustomNodesManager::new(versions_dir);

        // Initialize size calculator
        let cache_dir = launcher_root.join("launcher-data").join("cache");
        let size_calculator = SizeCalculator::new(cache_dir);

        let addr = start_server(
            api,
            version_manager,
            custom_nodes_manager,
            size_calculator,
            launcher_root,
            "127.0.0.1",
            0,
        )
        .await
        .unwrap();
        assert!(addr.port() > 0);
    }
}
