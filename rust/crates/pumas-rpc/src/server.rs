//! HTTP server implementation using Axum.

use crate::catalog_projection::CatalogProjection;
use crate::handlers::{
    handle_health, handle_model_download_update_events, handle_model_library_update_events,
    handle_rpc, handle_status_telemetry_update_events,
};
#[cfg(feature = "inference-plugins")]
use crate::handlers::{
    handle_openai_models, handle_openai_proxy, handle_runtime_profile_update_events,
    handle_serving_status_update_events,
};
#[cfg(feature = "inference-plugins")]
use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use futures::future::{BoxFuture, Shared};
use futures::FutureExt;
#[cfg(feature = "inference-plugins")]
use pumas_app_manager::{SizeCalculator, VersionManager};
use pumas_library::PumasApi;
#[cfg(feature = "inference-plugins")]
use pumas_library::{
    models::RuntimeEndpointUrl, OnnxEmbeddingBackendKind, OnnxSessionManager, PluginLoader,
    ProviderRegistry,
};
#[cfg(feature = "inference-plugins")]
use std::collections::HashMap;
use std::future::IntoFuture;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
#[cfg(feature = "inference-plugins")]
use std::time::Duration;
use tokio::sync::watch;
#[cfg(feature = "inference-plugins")]
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;

const MAX_IN_FLIGHT_RPC_REQUESTS: usize = 64;
const MAX_REQUEST_BODY_BYTES: usize = 32 * 1024 * 1024;
#[cfg(feature = "inference-plugins")]
const GATEWAY_PROXY_TIMEOUT: Duration = Duration::from_secs(120);
#[cfg(feature = "inference-plugins")]
const PROVIDER_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "inference-plugins")]
const ONNX_MAX_CONCURRENT_OPERATIONS: usize = 4;

/// A validated desktop RPC bind host.
///
/// The private field makes a non-loopback server configuration
/// unrepresentable after CLI admission.
#[derive(Clone, Copy)]
pub(crate) struct LoopbackHost(IpAddr);

impl LoopbackHost {
    pub(crate) fn parse(host: &str) -> anyhow::Result<Self> {
        let address: IpAddr = host
            .parse()
            .map_err(|_| anyhow::anyhow!("RPC host must be a loopback IP address"))?;
        if !address.is_loopback() {
            return Err(anyhow::anyhow!(
                "RPC host must be a loopback IP address; remote access is not supported"
            ));
        }
        Ok(Self(address))
    }

    const fn socket_addr(self, port: u16) -> SocketAddr {
        SocketAddr::new(self.0, port)
    }
}

/// Application state shared across handlers.
pub struct AppState {
    pub(crate) catalog_projection: CatalogProjection,
    /// Core API (model library, system utilities)
    pub api: PumasApi,
    /// Version managers for compiled-in inference plugins.
    #[cfg(feature = "inference-plugins")]
    pub version_managers: Arc<RwLock<HashMap<String, VersionManager>>>,
    /// Size calculator for release size estimates
    #[cfg(feature = "inference-plugins")]
    pub size_calculator: Arc<Mutex<SizeCalculator>>,
    /// Plugin configuration loader
    #[cfg(feature = "inference-plugins")]
    pub plugin_loader: Arc<PluginLoader>,
    /// Shared HTTP client for OpenAI-compatible gateway proxying.
    #[cfg(feature = "inference-plugins")]
    pub gateway_http_client: reqwest::Client,
    /// Public loopback base URL for the OpenAI-compatible serving gateway.
    #[cfg(feature = "inference-plugins")]
    pub gateway_base_url: RuntimeEndpointUrl,
    /// Runtime provider behavior registry for RPC boundary routing.
    #[cfg(feature = "inference-plugins")]
    pub provider_registry: ProviderRegistry,
    /// Shared llama.cpp router client for provider serving operations.
    #[cfg(feature = "inference-plugins")]
    pub llama_cpp_router_client: LlamaCppRouterClient,
    /// Shared Ollama client factory for provider serving and app operations.
    #[cfg(feature = "inference-plugins")]
    pub ollama_client_factory: OllamaClientFactory,
    /// Shared ONNX Runtime session manager for in-process embedding serving.
    #[cfg(feature = "inference-plugins")]
    pub onnx_session_manager: OnnxSessionManager<OnnxEmbeddingBackendKind>,
}

/// Owned handle for the running HTTP server task.
pub struct ServerHandle {
    addr: SocketAddr,
    completion: Shared<BoxFuture<'static, Result<(), Arc<anyhow::Error>>>>,
    shutdown_signal: watch::Sender<bool>,
    #[cfg(test)]
    catalog_projection: Option<CatalogProjection>,
    #[cfg(test)]
    catalog_drained: Option<Arc<std::sync::atomic::AtomicBool>>,
    #[cfg(test)]
    downloads_drained: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl ServerHandle {
    fn new(
        addr: SocketAddr,
        task: JoinHandle<anyhow::Result<()>>,
        shutdown_signal: watch::Sender<bool>,
    ) -> Self {
        let completion = async move {
            match task.await {
                Ok(result) => result.map_err(Arc::new),
                Err(error) => Err(Arc::new(anyhow::anyhow!("RPC supervisor failed: {error}"))),
            }
        }
        .boxed()
        .shared();
        Self {
            addr,
            completion,
            shutdown_signal,
            #[cfg(test)]
            catalog_projection: None,
            #[cfg(test)]
            catalog_drained: None,
            #[cfg(test)]
            downloads_drained: None,
        }
    }

    /// Address the server actually bound to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stop serving and observe both owned drains. Cancelling a waiter does not
    /// cancel the supervisor or consume its result; repeated waiters share it.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.shutdown_signal.send_replace(true);
        self.completion
            .clone()
            .await
            .map_err(|error| anyhow::anyhow!("{error:#}"))
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        // Drop requests shutdown but cannot prove its asynchronous completion.
        // Call shutdown() for an observed drain; the supervisor owns its worker.
        self.shutdown_signal.send_replace(true);
    }
}

async fn drain_server_owners(
    server_result: anyhow::Result<()>,
    downloads: impl std::future::Future<Output = pumas_library::Result<()>>,
    catalog: impl std::future::Future<Output = anyhow::Result<()>>,
    conversion_setup: impl std::future::Future<Output = pumas_library::Result<()>>,
) -> anyhow::Result<()> {
    // No owner may be abandoned merely because another failed first.
    let (downloads_result, catalog_result, setup_result) =
        tokio::join!(downloads, catalog, conversion_setup);
    let failures = [
        server_result
            .err()
            .map(|error| format!("listener: {error:#}")),
        downloads_result
            .err()
            .map(|error| format!("downloads: {error}")),
        catalog_result
            .err()
            .map(|error| format!("catalog: {error:#}")),
        setup_result
            .err()
            .map(|error| format!("conversion setup: {error}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "RPC shutdown failed: {}",
            failures.join("; ")
        ))
    }
}

/// Start the JSON-RPC HTTP server.
///
/// Returns an owned handle that exposes the actual bound address and server task.
#[allow(clippy::too_many_arguments)]
pub async fn start_server(
    api: PumasApi,
    #[cfg(feature = "inference-plugins")] version_managers: HashMap<String, VersionManager>,
    #[cfg(feature = "inference-plugins")] size_calculator: SizeCalculator,
    #[cfg(feature = "inference-plugins")] plugin_loader: PluginLoader,
    host: LoopbackHost,
    port: u16,
) -> anyhow::Result<ServerHandle> {
    #[cfg(feature = "inference-plugins")]
    let gateway_http_client = build_gateway_http_client()?;
    #[cfg(feature = "inference-plugins")]
    let provider_http_client = build_provider_http_client()?;
    let addr = host.socket_addr(port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    #[cfg(feature = "inference-plugins")]
    let gateway_base_url = RuntimeEndpointUrl::parse(format!("http://{actual_addr}/v1"))
        .map_err(|message| anyhow::anyhow!("invalid gateway base URL: {message}"))?;
    #[cfg(feature = "inference-plugins")]
    let ollama_client_factory = build_ollama_client_factory()?;
    #[cfg(feature = "inference-plugins")]
    let onnx_session_manager = OnnxSessionManager::new(
        OnnxEmbeddingBackendKind::real(),
        ONNX_MAX_CONCURRENT_OPERATIONS,
    )
    .map_err(|err| anyhow::anyhow!("failed to build ONNX session manager: {err}"))?;
    #[cfg(feature = "inference-plugins")]
    let provider_registry = ProviderRegistry::builtin();
    let (catalog_projection, catalog_worker) = CatalogProjection::start(MAX_IN_FLIGHT_RPC_REQUESTS);
    let state = Arc::new(AppState {
        catalog_projection,
        api,
        #[cfg(feature = "inference-plugins")]
        version_managers: Arc::new(RwLock::new(version_managers)),
        #[cfg(feature = "inference-plugins")]
        size_calculator: Arc::new(Mutex::new(size_calculator)),
        #[cfg(feature = "inference-plugins")]
        plugin_loader: Arc::new(plugin_loader),
        #[cfg(feature = "inference-plugins")]
        gateway_http_client,
        #[cfg(feature = "inference-plugins")]
        gateway_base_url,
        #[cfg(feature = "inference-plugins")]
        provider_registry,
        #[cfg(feature = "inference-plugins")]
        llama_cpp_router_client: LlamaCppRouterClient::new(provider_http_client),
        #[cfg(feature = "inference-plugins")]
        ollama_client_factory,
        #[cfg(feature = "inference-plugins")]
        onnx_session_manager,
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
            "/events/status-telemetry-updates",
            get(handle_status_telemetry_update_events),
        )
        .route("/rpc", post(handle_rpc));

    #[cfg(feature = "inference-plugins")]
    let app = app
        .route(
            "/events/runtime-profile-updates",
            get(handle_runtime_profile_update_events),
        )
        .route(
            "/events/serving-status-updates",
            get(handle_serving_status_update_events),
        )
        .route("/v1/models", get(handle_openai_models))
        .route("/v1/chat/completions", post(handle_openai_proxy))
        .route("/v1/completions", post(handle_openai_proxy))
        .route("/v1/embeddings", post(handle_openai_proxy));

    let app = app
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(ConcurrencyLimitLayer::new(MAX_IN_FLIGHT_RPC_REQUESTS))
        .layer(cors)
        .with_state(state.clone());

    info!(
        "Server listening on {} with max {} in-flight requests and {} byte request bodies",
        actual_addr, MAX_IN_FLIGHT_RPC_REQUESTS, MAX_REQUEST_BODY_BYTES
    );

    // Spawn the server in the background and retain ownership of the task.
    #[cfg(test)]
    let catalog_for_test = state.catalog_projection.clone();
    #[cfg(test)]
    let catalog_drained = Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(test)]
    let catalog_drain_observed = catalog_drained.clone();
    #[cfg(test)]
    let downloads_drained = Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(test)]
    let downloads_drain_observed = downloads_drained.clone();
    let (shutdown_signal, mut shutdown) = watch::channel(false);
    let task = tokio::spawn(async move {
        let serving = axum::serve(listener, app).into_future();
        let server_result = tokio::select! {
            result = serving => result.map_err(anyhow::Error::from),
            _ = shutdown.changed() => Ok(()),
        };
        drain_server_owners(
            server_result,
            async {
                let result = state.api.shutdown_downloads().await;
                #[cfg(test)]
                downloads_drain_observed.store(true, std::sync::atomic::Ordering::Release);
                result
            },
            async {
                let result = catalog_worker.shutdown().await;
                #[cfg(test)]
                catalog_drain_observed.store(true, std::sync::atomic::Ordering::Release);
                result
            },
            state.api.shutdown_conversion_setup(),
        )
        .await
    });

    let handle = ServerHandle::new(actual_addr, task, shutdown_signal);
    #[cfg(test)]
    let handle = {
        let mut handle = handle;
        handle.catalog_projection = Some(catalog_for_test);
        handle.catalog_drained = Some(catalog_drained);
        handle.downloads_drained = Some(downloads_drained);
        handle
    };
    Ok(handle)
}

#[cfg(feature = "inference-plugins")]
fn build_gateway_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(GATEWAY_PROXY_TIMEOUT)
        .build()?)
}

#[cfg(feature = "inference-plugins")]
fn build_provider_http_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(PROVIDER_HTTP_CONNECT_TIMEOUT)
        .user_agent("pumas-library")
        .build()?)
}

#[cfg(feature = "inference-plugins")]
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
    #[cfg(feature = "inference-plugins")]
    use pumas_library::AppId;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    #[tokio::test]
    async fn shutdown_receipt_survives_waiter_cancellation_and_retains_all_failures() {
        for fail_downloads in [false, true] {
            for fail_catalog in [false, true] {
                let (catalog, catalog_worker) = CatalogProjection::start(1);
                let (catalog_entered, catalog_release) = catalog.hold_for_test().unwrap();
                catalog_entered.await.unwrap();
                let (downloads_entered, downloads_ready) = tokio::sync::oneshot::channel();
                let (downloads_release, downloads_blocked) = tokio::sync::oneshot::channel();
                let (signal, mut shutdown) = watch::channel(false);
                let supervisor = tokio::spawn(async move {
                    shutdown.changed().await.unwrap();
                    drain_server_owners(
                        Err(anyhow::anyhow!("listener sentinel")),
                        async move {
                            downloads_entered.send(()).unwrap();
                            downloads_blocked.await.unwrap();
                            if fail_downloads {
                                Err(pumas_library::PumasError::DownloadShutdownFailed {
                                    failures: 1,
                                })
                            } else {
                                Ok(())
                            }
                        },
                        catalog_worker.shutdown(),
                        async { Ok(()) },
                    )
                    .await
                });
                let server = Arc::new(ServerHandle::new(
                    "127.0.0.1:1".parse().unwrap(),
                    supervisor,
                    signal,
                ));
                let waiter = tokio::spawn({
                    let server = server.clone();
                    async move { server.shutdown().await }
                });
                downloads_ready.await.unwrap();
                waiter.abort();
                assert!(waiter.await.unwrap_err().is_cancelled());
                // This is a real catalog worker; closure must reach it even
                // while the other drain is pending and the listener failed.
                assert!(catalog
                    .models(Vec::new(), std::path::PathBuf::new())
                    .await
                    .is_err());
                let repeated = server.shutdown();
                tokio::pin!(repeated);
                assert!(futures::poll!(&mut repeated).is_pending());
                downloads_release.send(()).unwrap();
                if fail_catalog {
                    // The real blocked worker reports its failed job/join.
                    drop(catalog_release);
                } else {
                    catalog_release.send(()).unwrap();
                }
                let error = tokio::time::timeout(std::time::Duration::from_secs(3), &mut repeated)
                    .await
                    .unwrap()
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("listener: listener sentinel"));
                assert_eq!(error.contains("downloads:"), fail_downloads);
                assert_eq!(error.contains("catalog:"), fail_catalog);
                assert_eq!(server.shutdown().await.unwrap_err().to_string(), error);
            }
        }
    }

    #[tokio::test]
    async fn setup_drain_survives_a_cancelled_shutdown_waiter_and_retains_failure() {
        let (signal, mut shutdown) = watch::channel(false);
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, blocked) = tokio::sync::oneshot::channel();
        let supervisor = tokio::spawn(async move {
            shutdown.changed().await.unwrap();
            drain_server_owners(
                Err(anyhow::anyhow!("listener failed")),
                async {
                    Err(pumas_library::PumasError::Other(
                        "download drain failed".into(),
                    ))
                },
                async { Err(anyhow::anyhow!("catalog drain failed")) },
                async move {
                    entered.send(()).unwrap();
                    blocked.await.unwrap();
                    Err(pumas_library::PumasError::Other(
                        "setup drain failed".into(),
                    ))
                },
            )
            .await
        });
        let server = Arc::new(ServerHandle::new(
            "127.0.0.1:1".parse().unwrap(),
            supervisor,
            signal,
        ));
        let waiter = tokio::spawn({
            let server = server.clone();
            async move { server.shutdown().await }
        });
        started.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        let repeated = server.shutdown();
        tokio::pin!(repeated);
        assert!(futures::poll!(&mut repeated).is_pending());
        release.send(()).unwrap();
        let error = repeated.await.unwrap_err().to_string();
        for owner in ["listener:", "downloads:", "catalog:", "conversion setup:"] {
            assert!(error.contains(owner), "missing {owner} in {error}");
        }
        assert_eq!(server.shutdown().await.unwrap_err().to_string(), error);
    }

    #[tokio::test]
    async fn successful_shutdown_is_repeatedly_observable() {
        let (_, catalog_worker) = CatalogProjection::start(1);
        let (signal, mut shutdown) = watch::channel(false);
        let supervisor = tokio::spawn(async move {
            shutdown.changed().await.unwrap();
            drain_server_owners(Ok(()), async { Ok(()) }, catalog_worker.shutdown(), async {
                Ok(())
            })
            .await
        });
        let server = ServerHandle::new("127.0.0.1:1".parse().unwrap(), supervisor, signal);
        server.shutdown().await.unwrap();
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn dropping_handle_requests_shutdown_without_aborting_owned_catalog_drain() {
        let (catalog, catalog_worker) = CatalogProjection::start(1);
        let (entered, release) = catalog.hold_for_test().unwrap();
        entered.await.unwrap();
        let (signal, mut shutdown) = watch::channel(false);
        let (drain_started, started) = tokio::sync::oneshot::channel();
        let (drained, mut completion) = tokio::sync::oneshot::channel();
        let supervisor = tokio::spawn(async move {
            shutdown.changed().await.unwrap();
            drain_started.send(()).unwrap();
            let result =
                drain_server_owners(Ok(()), async { Ok(()) }, catalog_worker.shutdown(), async {
                    Ok(())
                })
                .await;
            drained.send(result.is_ok()).unwrap();
            result
        });
        let server = ServerHandle::new("127.0.0.1:1".parse().unwrap(), supervisor, signal);
        drop(server);
        started.await.unwrap();
        let premature = completion.try_recv();
        release.send(()).unwrap();
        assert!(matches!(
            premature,
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(3), completion)
                .await
                .unwrap()
                .unwrap()
        );
    }

    #[test]
    fn loopback_host_rejects_every_remote_or_ambiguous_form() {
        assert!(LoopbackHost::parse("127.0.0.1").is_ok());
        assert!(LoopbackHost::parse("::1").is_ok());

        for host in ["0.0.0.0", "::", "192.168.1.10", "8.8.8.8", "localhost"] {
            let error = LoopbackHost::parse(host).err().expect("host must fail");
            assert!(error.to_string().contains("loopback"), "{host}: {error}");
        }
    }

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

    async fn start_test_server(
        api: PumasApi,
        launcher_root: &std::path::Path,
    ) -> anyhow::Result<ServerHandle> {
        #[cfg(not(feature = "inference-plugins"))]
        let _ = launcher_root;
        #[cfg(feature = "inference-plugins")]
        let mut version_managers = HashMap::new();
        #[cfg(feature = "inference-plugins")]
        if let Ok(vm) = VersionManager::new(&launcher_root, AppId::Ollama).await {
            version_managers.insert("ollama".to_string(), vm);
        }

        #[cfg(feature = "inference-plugins")]
        let cache_dir = launcher_root.join("launcher-data").join("cache");
        #[cfg(feature = "inference-plugins")]
        let size_calculator = SizeCalculator::new_with_cache(cache_dir).await;

        #[cfg(feature = "inference-plugins")]
        let plugins_dir = launcher_root.join("launcher-data").join("plugins");
        #[cfg(feature = "inference-plugins")]
        let plugin_loader = PluginLoader::new_async(plugins_dir).await.unwrap();

        start_server(
            api,
            #[cfg(feature = "inference-plugins")]
            version_managers,
            #[cfg(feature = "inference-plugins")]
            size_calculator,
            #[cfg(feature = "inference-plugins")]
            plugin_loader,
            LoopbackHost::parse("127.0.0.1").unwrap(),
            0,
        )
        .await
    }

    #[tokio::test]
    async fn real_server_shutdown_drains_hf_and_catalog_after_callers_leave() {
        for outcome in ["success", "error", "panic"] {
            let temp = TempDir::new().unwrap();
            let api = crate::handlers::test_support::build_test_api_with_hf(temp.path()).await;
            let path = temp.path().join("owned-shutdown-write");
            let written_path = path.clone();
            let (entered, ready) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            let fixture = pumas_library::model_library::test_support::run_download_blocking_fixture(
                &api,
                move || -> pumas_library::Result<()> {
                    entered.send(()).unwrap();
                    blocked.recv().unwrap();
                    std::fs::write(written_path, b"owned effect completed")?;
                    match outcome {
                        "success" => Ok(()),
                        "error" => Err(pumas_library::PumasError::Other(
                            "held fixture failure".into(),
                        )),
                        _ => panic!("held fixture panic"),
                    }
                },
            );
            let caller = tokio::spawn(fixture);
            ready.await.unwrap();
            let server = Arc::new(start_test_server(api, temp.path()).await.unwrap());
            let (catalog_ready, catalog_release) = server
                .catalog_projection
                .as_ref()
                .unwrap()
                .hold_for_test()
                .unwrap();
            catalog_ready.await.unwrap();
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
            let waiter = tokio::spawn({
                let server = server.clone();
                async move { server.shutdown().await }
            });
            // Polling this independent borrowed receipt requests shutdown too;
            // neither waiter owns the lifetime of either actual effect.
            let repeated = server.shutdown();
            tokio::pin!(repeated);
            assert!(futures::poll!(&mut repeated).is_pending());
            waiter.abort();
            assert!(waiter.await.unwrap_err().is_cancelled());
            assert!(!path.exists());
            let mut catalog_release = Some(catalog_release);
            let catalog_observation = if outcome == "success" {
                catalog_release.take().unwrap().send(()).unwrap();
                Some(
                    tokio::time::timeout(std::time::Duration::from_secs(3), async {
                        while !server
                            .catalog_drained
                            .as_ref()
                            .unwrap()
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await,
                )
            } else {
                None
            };
            let pending_with_hf_held = futures::poll!(&mut repeated).is_pending();
            release.send(()).unwrap();
            if let Some(observation) = catalog_observation {
                observation.expect("the real catalog drain must finish independently of HF");
                assert!(
                    pending_with_hf_held,
                    "held HF work must prevent shutdown completion after catalog drains"
                );
            }
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while !path.exists() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("the real HF effect must finish after its caller leaves");
            if let Some(catalog_release) = catalog_release {
                let observation = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    while !server
                        .downloads_drained
                        .as_ref()
                        .unwrap()
                        .load(std::sync::atomic::Ordering::Acquire)
                    {
                        tokio::task::yield_now().await;
                    }
                })
                .await;
                // Sample while catalog is held, but release before assertions
                // so a failing oracle cannot strand a blocking fixture.
                let pending_with_catalog_held = futures::poll!(&mut repeated).is_pending();
                drop(catalog_release);
                observation.expect("HF failure must be observed even while catalog is held");
                assert!(pending_with_catalog_held);
            }
            let result = tokio::time::timeout(std::time::Duration::from_secs(3), &mut repeated)
                .await
                .unwrap();
            assert_eq!(std::fs::read(path).unwrap(), b"owned effect completed");
            if outcome == "success" {
                result.unwrap();
                server.shutdown().await.unwrap();
            } else {
                let message = result.unwrap_err().to_string();
                assert!(message.contains("downloads:"), "{message}");
                assert!(message.contains("catalog:"), "{message}");
                assert_eq!(server.shutdown().await.unwrap_err().to_string(), message);
            }
        }
    }

    #[tokio::test]
    async fn test_server_starts() {
        let temp_dir = TempDir::new().unwrap();
        let api = crate::handlers::test_support::build_test_api_with_hf(temp_dir.path()).await;
        let result = start_test_server(api, temp_dir.path()).await;
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
        server.shutdown().await.unwrap();
        server.shutdown().await.unwrap();
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

    #[cfg(feature = "inference-plugins")]
    #[test]
    fn gateway_http_client_builds_with_configured_policy() {
        build_gateway_http_client().unwrap();
    }
}
