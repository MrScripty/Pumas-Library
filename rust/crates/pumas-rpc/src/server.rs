//! HTTP server implementation using Axum.

use crate::handlers::{handle_health, handle_rpc};
use crate::shortcut::ShortcutManager;
use axum::{
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use pumas_app_manager::{CustomNodesManager, SizeCalculator, VersionManager};
use pumas_library::{PluginLoader, PumasApi};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{error, info, warn};

const MAX_IN_FLIGHT_RPC_REQUESTS: usize = 64;

/// Application state shared across handlers.
pub struct AppState {
    /// Core API (model library, system utilities)
    pub api: PumasApi,
    /// Version managers for each supported app (keyed by app_id: "comfyui", "ollama", etc.)
    pub version_managers: Arc<RwLock<HashMap<String, VersionManager>>>,
    /// Custom nodes manager (from pumas-app-manager)
    pub custom_nodes_manager: Arc<CustomNodesManager>,
    /// Size calculator for release size estimates
    pub size_calculator: Arc<Mutex<SizeCalculator>>,
    /// Shortcut manager for desktop/menu shortcuts
    pub shortcut_manager: Arc<RwLock<Option<ShortcutManager>>>,
    /// Plugin configuration loader
    pub plugin_loader: Arc<PluginLoader>,
}

/// Owned handle for the running HTTP server task.
pub struct ServerHandle {
    addr: SocketAddr,
    task: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// Address the server actually bound to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stop the server task and wait until it is no longer running.
    pub async fn shutdown(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            match task.await {
                Ok(()) => {}
                Err(error) if error.is_cancelled() => {}
                Err(error) => warn!("RPC server task failed during shutdown: {}", error),
            }
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Start the JSON-RPC HTTP server.
///
/// Returns an owned handle that exposes the actual bound address and server task.
#[allow(clippy::too_many_arguments)]
pub async fn start_server(
    api: PumasApi,
    version_managers: HashMap<String, VersionManager>,
    custom_nodes_manager: CustomNodesManager,
    size_calculator: SizeCalculator,
    plugin_loader: PluginLoader,
    launcher_root: PathBuf,
    host: &str,
    port: u16,
) -> anyhow::Result<ServerHandle> {
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
        size_calculator: Arc::new(Mutex::new(size_calculator)),
        shortcut_manager: Arc::new(RwLock::new(shortcut_manager)),
        plugin_loader: Arc::new(plugin_loader),
    });

    // Configure CORS for local development and packaged renderer diagnostics.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _parts| {
            is_allowed_cors_origin(origin)
        }))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    // Build the router
    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/rpc", post(handle_rpc))
        .layer(ConcurrencyLimitLayer::new(MAX_IN_FLIGHT_RPC_REQUESTS))
        .layer(cors)
        .with_state(state);

    // Parse the address
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    // Bind to the address
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    info!(
        "Server listening on {} with max {} in-flight requests",
        actual_addr, MAX_IN_FLIGHT_RPC_REQUESTS
    );

    // Spawn the server in the background and retain ownership of the task.
    let task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            error!("RPC server error: {}", error);
        }
    });

    Ok(ServerHandle {
        addr: actual_addr,
        task: Some(task),
    })
}

fn is_allowed_cors_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::AppId;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    fn is_socket_bind_permission_error(err: &anyhow::Error) -> bool {
        err.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .map(|io_err| {
                    io_err.kind() == ErrorKind::PermissionDenied || io_err.raw_os_error() == Some(1)
                })
                .unwrap_or(false)
        })
    }

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
        let size_calculator = SizeCalculator::new_with_cache(cache_dir).await;

        // Initialize plugin loader
        let plugins_dir = launcher_root.join("launcher-data").join("plugins");
        let plugin_loader = PluginLoader::new_async(plugins_dir).await.unwrap();

        let result = start_server(
            api,
            version_managers,
            custom_nodes_manager,
            size_calculator,
            plugin_loader,
            launcher_root,
            "127.0.0.1",
            0,
        )
        .await;
        let server = match result {
            Ok(server) => server,
            Err(err) if is_socket_bind_permission_error(&err) => {
                eprintln!("Skipping test_server_starts: socket bind not permitted ({err})");
                return;
            }
            Err(err) => panic!("test_server_starts failed: {err:#}"),
        };
        let addr = server.addr();
        assert!(addr.port() > 0);
        server.shutdown().await;
    }

    #[test]
    fn cors_allows_loopback_origins() {
        for origin in [
            "http://localhost:5173",
            "http://127.0.0.1:5173",
            "http://[::1]:5173",
        ] {
            let header = HeaderValue::from_str(origin).unwrap();
            assert!(is_allowed_cors_origin(&header), "{origin}");
        }
    }

    #[test]
    fn cors_rejects_non_loopback_origins() {
        for origin in [
            "https://example.com",
            "http://192.168.1.10:5173",
            "file:///tmp/index.html",
        ] {
            let header = HeaderValue::from_str(origin).unwrap();
            assert!(!is_allowed_cors_origin(&header), "{origin}");
        }
    }
}
