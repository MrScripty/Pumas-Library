//! HTTP server implementation using Axum.

use crate::handlers::{
    handle_health, handle_model_download_update_events, handle_model_library_update_events,
    handle_openai_models, handle_openai_proxy, handle_rpc, handle_runtime_profile_update_events,
    handle_serving_status_update_events, handle_status_telemetry_update_events,
};
use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
use crate::shortcut::ShortcutManager;
use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use pumas_app_manager::{CustomNodesManager, SizeCalculator, VersionManager};
use pumas_library::{PluginLoader, ProviderRegistry, PumasApi};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{error, info, warn};

const MAX_IN_FLIGHT_RPC_REQUESTS: usize = 64;
const MAX_REQUEST_BODY_BYTES: usize = 32 * 1024 * 1024;
const GATEWAY_PROXY_TIMEOUT: Duration = Duration::from_secs(120);
const PROVIDER_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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
    /// Shared HTTP client for OpenAI-compatible gateway proxying.
    pub gateway_http_client: reqwest::Client,
    /// Runtime provider behavior registry for RPC boundary routing.
    pub provider_registry: ProviderRegistry,
    /// Shared llama.cpp router client for provider serving operations.
    pub llama_cpp_router_client: LlamaCppRouterClient,
    /// Shared Ollama client factory for provider serving and app operations.
    pub ollama_client_factory: OllamaClientFactory,
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
    let shortcut_manager = match ShortcutManager::new_async(&launcher_root).await {
        Ok(mgr) => {
            info!("Shortcut manager initialized");
            Some(mgr)
        }
        Err(e) => {
            warn!("Failed to initialize shortcut manager: {}", e);
            None
        }
    };

    let gateway_http_client = build_gateway_http_client()?;
    let provider_http_client = build_provider_http_client()?;
    let ollama_client_factory = build_ollama_client_factory()?;
    let provider_registry = ProviderRegistry::builtin();
    let state = Arc::new(AppState {
        api,
        version_managers: Arc::new(RwLock::new(version_managers)),
        custom_nodes_manager: Arc::new(custom_nodes_manager),
        size_calculator: Arc::new(Mutex::new(size_calculator)),
        shortcut_manager: Arc::new(RwLock::new(shortcut_manager)),
        plugin_loader: Arc::new(plugin_loader),
        gateway_http_client,
        provider_registry,
        llama_cpp_router_client: LlamaCppRouterClient::new(provider_http_client),
        ollama_client_factory,
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
        .route(
            "/events/model-library-updates",
            get(handle_model_library_update_events),
        )
        .route(
            "/events/model-download-updates",
            get(handle_model_download_update_events),
        )
        .route(
            "/events/runtime-profile-updates",
            get(handle_runtime_profile_update_events),
        )
        .route(
            "/events/serving-status-updates",
            get(handle_serving_status_update_events),
        )
        .route(
            "/events/status-telemetry-updates",
            get(handle_status_telemetry_update_events),
        )
        .route("/v1/models", get(handle_openai_models))
        .route("/v1/chat/completions", post(handle_openai_proxy))
        .route("/v1/completions", post(handle_openai_proxy))
        .route("/v1/embeddings", post(handle_openai_proxy))
        .route("/rpc", post(handle_rpc))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(ConcurrencyLimitLayer::new(MAX_IN_FLIGHT_RPC_REQUESTS))
        .layer(cors)
        .with_state(state);

    // Parse the address
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    // Bind to the address
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;

    info!(
        "Server listening on {} with max {} in-flight requests and {} byte request bodies",
        actual_addr, MAX_IN_FLIGHT_RPC_REQUESTS, MAX_REQUEST_BODY_BYTES
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

fn build_gateway_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(GATEWAY_PROXY_TIMEOUT)
        .build()?)
}

fn build_provider_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(PROVIDER_HTTP_CONNECT_TIMEOUT)
        .user_agent("pumas-library")
        .build()?)
}

fn build_ollama_client_factory() -> anyhow::Result<OllamaClientFactory> {
    let http_clients = pumas_app_manager::OllamaHttpClients::new()
        .map_err(|err| anyhow::anyhow!("failed to build Ollama HTTP clients: {err}"))?;
    Ok(OllamaClientFactory::new(http_clients))
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

    #[test]
    fn gateway_http_client_builds_with_configured_policy() {
        build_gateway_http_client().unwrap();
    }
}
