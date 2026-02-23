//! HTTP server implementation using Axum.

use crate::handlers::{handle_health, handle_rpc};
use crate::shortcut::ShortcutManager;
use axum::{
    routing::{get, post},
    Router,
};
use pumas_app_manager::{CustomNodesManager, VersionManager, SizeCalculator};
use pumas_library::{PumasApi, PluginLoader};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};

/// Application state shared across handlers.
pub struct AppState {
    /// Core API (model library, system utilities)
    pub api: PumasApi,
    /// Version managers for each supported app (keyed by app_id: "comfyui", "ollama", etc.)
    pub version_managers: Arc<RwLock<HashMap<String, VersionManager>>>,
    /// Custom nodes manager (from pumas-app-manager)
    pub custom_nodes_manager: Arc<CustomNodesManager>,
    /// Size calculator for release size estimates
    pub size_calculator: Arc<RwLock<SizeCalculator>>,
    /// Shortcut manager for desktop/menu shortcuts
    pub shortcut_manager: Arc<RwLock<Option<ShortcutManager>>>,
    /// Plugin configuration loader
    pub plugin_loader: Arc<PluginLoader>,
}

/// Start the JSON-RPC HTTP server.
///
/// Returns the actual address the server is bound to (useful when port=0).
pub async fn start_server(
    api: PumasApi,
    version_managers: HashMap<String, VersionManager>,
    custom_nodes_manager: CustomNodesManager,
    size_calculator: SizeCalculator,
    plugin_loader: PluginLoader,
    launcher_root: PathBuf,
    host: &str,
    port: u16,
) -> anyhow::Result<SocketAddr> {
    // Initialize shortcut manager
    let shortcut_manager = match ShortcutManager::new(&launcher_root) {
        Ok(mgr) => {
            info!("Shortcut manager initialized");
            Some(mgr)
        }
        Err(e) => {
            warn!("Failed to initialize shortcut manager: {}", e);
            None
        }
    };

    let state = Arc::new(AppState {
        api,
        version_managers: Arc::new(RwLock::new(version_managers)),
        custom_nodes_manager: Arc::new(custom_nodes_manager),
        size_calculator: Arc::new(RwLock::new(size_calculator)),
        shortcut_manager: Arc::new(RwLock::new(shortcut_manager)),
        plugin_loader: Arc::new(plugin_loader),
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
    use pumas_library::AppId;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_starts() {
        let temp_dir = TempDir::new().unwrap();
        let launcher_root = temp_dir.path().to_path_buf();
        let api = PumasApi::new(&launcher_root).await.unwrap();

        // Initialize version managers (may fail if directories don't exist, which is fine for test)
        let mut version_managers = HashMap::new();
        if let Ok(vm) = VersionManager::new(&launcher_root, AppId::ComfyUI).await {
            version_managers.insert("comfyui".to_string(), vm);
        }

        // Initialize custom nodes manager
        let versions_dir = launcher_root.join(AppId::ComfyUI.versions_dir_name());
        let custom_nodes_manager = CustomNodesManager::new(versions_dir);

        // Initialize size calculator
        let cache_dir = launcher_root.join("launcher-data").join("cache");
        let size_calculator = SizeCalculator::new(cache_dir);

        // Initialize plugin loader
        let plugins_dir = launcher_root.join("launcher-data").join("plugins");
        let plugin_loader = PluginLoader::new(&plugins_dir).unwrap();

        let addr = start_server(
            api,
            version_managers,
            custom_nodes_manager,
            size_calculator,
            plugin_loader,
            launcher_root,
            "127.0.0.1",
            0,
        )
        .await
        .unwrap();
        assert!(addr.port() > 0);
    }
}
