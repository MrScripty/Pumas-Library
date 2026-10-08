//! HTTP server implementation using Axum.

use crate::catalog_projection::CatalogProjection;
#[cfg(feature = "inference-plugins")]
use crate::handlers::{
    handle_capabilities, handle_model_operations, handle_openai_models, handle_openai_proxy,
    handle_runtime_profile_update_events, handle_serving_status_update_events,
};
use crate::handlers::{
    handle_health, handle_model_download_update_events, handle_model_library_update_events,
    handle_rpc, handle_status_telemetry_update_events,
};
use crate::http_admission::{enforce_local_request, is_allowed_origin};
#[cfg(feature = "inference-plugins")]
use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
use axum::{
    extract::DefaultBodyLimit,
    http::{header, Method},
    middleware,
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
use std::future::Future;
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

/// Admission-only handle. Request handlers never receive the completion future,
/// because it includes completion of their own HTTP response.
#[derive(Clone)]
pub(crate) struct ShutdownRequest {
    signal: watch::Sender<bool>,
}

impl Default for ShutdownRequest {
    fn default() -> Self {
        Self {
            signal: watch::channel(false).0,
        }
    }
}

impl ShutdownRequest {
    pub(crate) fn is_requested(&self) -> bool {
        *self.signal.borrow()
    }

    pub(crate) fn request(&self) {
        self.signal.send_replace(true);
    }

    pub(crate) async fn requested(self) {
        let mut receiver = self.signal.subscribe();
        while !*receiver.borrow_and_update() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

/// Application state shared across handlers.
pub struct AppState {
    #[cfg(feature = "s3")]
    pub(crate) s3_imports: crate::s3_imports::S3Imports,
    pub(crate) shutdown_request: ShutdownRequest,
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
        self.request_shutdown();
        self.wait().await
    }

    /// Request cessation without waiting for the requesting HTTP response.
    pub fn request_shutdown(&self) {
        self.shutdown_signal.send_replace(true);
    }

    /// Observe the process-owned receipt without initiating shutdown.
    pub async fn wait(&self) -> anyhow::Result<()> {
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
    conversions: impl std::future::Future<Output = pumas_library::Result<()>>,
) -> anyhow::Result<()> {
    // No owner may be abandoned merely because another failed first.
    let (downloads_result, catalog_result, setup_result, conversions_result) =
        tokio::join!(downloads, catalog, conversion_setup, conversions);
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
        conversions_result
            .err()
            .map(|error| format!("conversion workers: {error}")),
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
    http_policy: crate::http_transport::HttpShutdownPolicy,
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
    // Prepared registration is invisible; its custody receipt keeps the core
    // generation until this supervisor observes all HTTP-owned effects.
    let mut advertisement = api.prepare_http_service(
        pumas_library::discovery::LoopbackHttpEndpoint::parse(format!("http://{actual_addr}"))?,
        crate::discovery::build_info(),
    )?;
    let route_identity = crate::discovery::HttpRouteIdentity::from(advertisement.description());
    let (catalog_projection, catalog_worker) = CatalogProjection::start(MAX_IN_FLIGHT_RPC_REQUESTS);
    let shutdown_request = ShutdownRequest::default();
    let shutdown_signal = shutdown_request.signal.clone();
    #[cfg(feature = "s3")]
    let (s3_imports, s3_jobs) = crate::s3_imports::S3Imports::channel();
    let state = Arc::new(AppState {
        #[cfg(feature = "s3")]
        s3_imports,
        shutdown_request: shutdown_request.clone(),
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

    #[cfg(feature = "s3")]
    let s3_worker = crate::s3_imports::S3ImportWorker::start(state.clone(), s3_jobs);

    // Configure CORS for local development and packaged renderer diagnostics.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _parts| {
            is_allowed_origin(origin)
        }))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    // Build the router
    let app = Router::new()
        .route("/health", get(handle_health))
        .route(
            pumas_library::discovery::HTTP_DISCOVERY_PATH,
            get(crate::discovery::handle_description),
        )
        .layer(axum::Extension(route_identity))
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
        .route("/v1/capabilities", get(handle_capabilities))
        .route("/v1/model-operations", post(handle_model_operations))
        .route("/v1/models", get(handle_openai_models))
        .route("/v1/chat/completions", post(handle_openai_proxy))
        .route("/v1/completions", post(handle_openai_proxy))
        .route("/v1/embeddings", post(handle_openai_proxy))
        .route("/v1/images/generations", post(handle_openai_proxy));

    let app = app
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(ConcurrencyLimitLayer::new(MAX_IN_FLIGHT_RPC_REQUESTS))
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(
            shutdown_request.clone(),
            reject_during_shutdown,
        ))
        .layer(middleware::from_fn(enforce_local_request))
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
    let (startup_tx, startup_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut serving = Box::pin(crate::http_transport::serve(
            listener,
            app,
            shutdown_request.clone(),
            http_policy,
        ));
        // Poll the actual accept loop before publication. No detached readiness
        // guess or externally supplied port is used.
        let first_poll =
            futures::future::poll_fn(|cx| std::task::Poll::Ready(serving.as_mut().poll(cx))).await;
        let startup = match &first_poll {
            std::task::Poll::Ready(_) => Err("HTTP listener exited before readiness".to_owned()),
            std::task::Poll::Pending if shutdown_request.is_requested() => {
                Err("HTTP startup cancelled".to_owned())
            }
            std::task::Poll::Pending => advertisement.publish().map_err(|error| error.to_string()),
        };
        if startup.is_err() {
            shutdown_request.request();
        }
        let _ = startup_tx.send(startup);
        let early_result = match first_poll {
            std::task::Poll::Ready(result) => Some(result),
            std::task::Poll::Pending => tokio::select! {
                result = &mut serving => Some(result),
                _ = shutdown_request.clone().requested() => None,
            },
        };
        shutdown_request.request();
        let advertisement_revoke = advertisement.revoke();
        #[cfg(feature = "s3")]
        state.s3_imports.close();
        // Retain accepted connections until their responses settle. Core owners
        // drain concurrently, so requests awaiting those owners cannot deadlock
        // behind an HTTP-first shutdown ordering.
        let http_completion = async move {
            match early_result {
                Some(result) => result,
                None => serving.await,
            }
        };
        let installation_cleanup = async {
            #[cfg(feature = "inference-plugins")]
            {
                let managers = state.version_managers.read().await;
                let mut errors = Vec::new();
                let outcomes = futures::future::join_all(
                    managers
                        .values()
                        .map(VersionManager::shutdown_installations),
                )
                .await;
                for outcome in outcomes {
                    if let Err(error) = outcome {
                        errors.push(error.to_string());
                    }
                }
                if !errors.is_empty() {
                    return Err(anyhow::anyhow!(errors.join("; ")));
                }
            }
            Ok::<(), anyhow::Error>(())
        };
        let (http, owners, runtimes, installation_cleanup) = tokio::join!(
            http_completion,
            drain_server_owners(
                Ok(()),
                async {
                    let result = state.api.shutdown_intent().await;
                    #[cfg(test)]
                    downloads_drain_observed.store(true, std::sync::atomic::Ordering::Release);
                    result
                },
                async {
                    #[cfg(feature = "s3")]
                    let result = {
                        let (catalog, source) =
                            tokio::join!(catalog_worker.shutdown(), s3_worker.shutdown());
                        match (catalog, source) {
                            (Ok(()), Ok(())) => Ok(()),
                            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
                            (Err(catalog), Err(source)) => {
                                Err(anyhow::anyhow!("{catalog}; {source}"))
                            }
                        }
                    };
                    #[cfg(not(feature = "s3"))]
                    let result = catalog_worker.shutdown().await;
                    #[cfg(test)]
                    catalog_drain_observed.store(true, std::sync::atomic::Ordering::Release);
                    result
                },
                state.api.shutdown_conversion_setup(),
                state.api.shutdown_conversions(),
            ),
            state.api.stop_all_managed_runtime_profiles(),
            installation_cleanup,
        );
        let runtimes = runtimes.map_err(anyhow::Error::from).and_then(|summary| {
            if summary.errors.is_empty() {
                Ok(())
            } else {
                Err(anyhow::anyhow!(summary.errors.join("; ")))
            }
        });
        // Consumer owners must finish draining before the shared supervisor
        // closes admission. Always observe its settlement, including failures.
        let acquisition_cleanup = state.api.shutdown_acquisition().await;
        let owners = match (owners, http) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(anyhow::anyhow!("HTTP connections: {error}")),
            (Err(owners), Err(error)) => {
                Err(anyhow::anyhow!("{owners}; HTTP connections: {error}"))
            }
        };
        let installation_cleanup = match (installation_cleanup, acquisition_cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(anyhow::anyhow!("Acquisition cleanup: {error}")),
            (Err(installation), Err(acquisition)) => Err(anyhow::anyhow!(
                "{installation}; Acquisition cleanup: {acquisition}"
            )),
        };
        let owners = match (owners, installation_cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(anyhow::anyhow!("Installation cleanup: {error}")),
            (Err(owners), Err(error)) => {
                Err(anyhow::anyhow!("{owners}; Installation cleanup: {error}"))
            }
        };
        let external = match (owners, runtimes, advertisement_revoke) {
            (Ok(()), Ok(()), Ok(_)) => Ok(()),
            (owners, runtimes, advertisement) => {
                let failures = [
                    owners.err().map(|e| e.to_string()),
                    runtimes.err().map(|e| e.to_string()),
                    advertisement.err().map(|e| e.to_string()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
                Err(anyhow::anyhow!(failures.join("; ")))
            }
        };
        let cessation = external
            .as_ref()
            .map(|_| ())
            .map_err(|error| pumas_library::PumasError::Other(error.to_string()));
        let report = advertisement.complete_shutdown(cessation);
        // Revocation failure can leave the settlement sender inside this
        // registration. Close that abandoned receipt before awaiting its core
        // observer; it must fail and retain authority instead of waiting on us.
        drop(advertisement);
        // Only now may the core observe external settlement and release its row.
        let core = state.api.shutdown_instance().await;
        let failures = [
            external.err().map(|e| e.to_string()),
            report.err().map(|e| e.to_string()),
            core.err().map(|e| e.to_string()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(failures.join("; ")))
        }
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
    match startup_rx.await {
        Ok(Ok(())) => Ok(handle),
        startup => {
            let reason = match startup {
                Ok(Err(reason)) => reason,
                _ => "HTTP readiness supervisor unavailable".into(),
            };
            let drain = handle.shutdown().await;
            Err(anyhow::anyhow!("{reason}; shutdown: {drain:?}"))
        }
    }
}

async fn reject_during_shutdown(
    axum::extract::State(shutdown): axum::extract::State<ShutdownRequest>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    // RPC rechecks after decoding so a repeated shutdown can acknowledge without
    // admitting other commands. Other routes stop at the header boundary.
    if shutdown.is_requested() && request.uri().path() != "/rpc" {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    next.run(request).await
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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn shutdown_request_is_idempotent_and_visible_to_late_subscribers() {
        let request = super::ShutdownRequest::default();
        let observed = request.clone().requested();
        tokio::pin!(observed);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut observed)
                .await
                .is_err()
        );
        request.request();
        request.request();
        observed.await;
        tokio::time::timeout(std::time::Duration::from_secs(1), request.requested())
            .await
            .unwrap();
    }

    use super::*;
    use axum::http::HeaderValue;
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
                async {
                    Err(pumas_library::PumasError::Other(
                        "worker drain failed".into(),
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
        for owner in [
            "listener:",
            "downloads:",
            "catalog:",
            "conversion setup:",
            "conversion workers:",
        ] {
            assert!(error.contains(owner), "missing {owner} in {error}");
        }
        assert_eq!(server.shutdown().await.unwrap_err().to_string(), error);
    }

    #[tokio::test]
    async fn conversion_worker_drain_survives_waiter_cancellation_and_other_owner_failure() {
        let (signal, mut shutdown) = watch::channel(false);
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, blocked) = tokio::sync::oneshot::channel();
        let supervisor = tokio::spawn(async move {
            shutdown.changed().await.unwrap();
            drain_server_owners(
                Ok(()),
                async { Ok(()) },
                async { Err(anyhow::anyhow!("catalog sentinel")) },
                async { Ok(()) },
                async move {
                    entered.send(()).unwrap();
                    blocked.await.unwrap();
                    Err(pumas_library::PumasError::ConversionFailed {
                        message: "worker sentinel".into(),
                    })
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
        assert!(error.contains("catalog: catalog sentinel"));
        assert!(error.contains("conversion workers:"));
        assert!(error.contains("worker sentinel"));
        assert_eq!(server.shutdown().await.unwrap_err().to_string(), error);
    }

    #[tokio::test]
    async fn successful_shutdown_is_repeatedly_observable() {
        let (_, catalog_worker) = CatalogProjection::start(1);
        let (signal, mut shutdown) = watch::channel(false);
        let supervisor = tokio::spawn(async move {
            shutdown.changed().await.unwrap();
            drain_server_owners(
                Ok(()),
                async { Ok(()) },
                catalog_worker.shutdown(),
                async { Ok(()) },
                async { Ok(()) },
            )
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
            let result = drain_server_owners(
                Ok(()),
                async { Ok(()) },
                catalog_worker.shutdown(),
                async { Ok(()) },
                async { Ok(()) },
            )
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

    pub(super) async fn start_test_server(
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
            crate::http_transport::HttpShutdownPolicy::default(),
        )
        .await
    }

    #[tokio::test]
    async fn retained_paused_download_status_round_trips_through_http_json_rpc() {
        let temp = TempDir::new().unwrap();
        let launcher_root = temp.path();
        let model_dir = launcher_root.join("shared-resources/models/llm/acme/partial-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::create_dir_all(launcher_root.join("launcher-data")).unwrap();
        std::fs::write(model_dir.join("weights.gguf.part"), b"partial").unwrap();

        let snapshot = serde_json::from_value::<
            pumas_library::model_library::download_store::PersistedDownload,
        >(serde_json::json!({
            "download_id": "tracked-partial-1",
            "repo_id": "acme/model",
            "revision": null,
            "filename": "weights.gguf",
            "filenames": ["weights.gguf"],
            "dest_dir": model_dir,
            "total_bytes": 100,
            "status": "paused",
            "download_request": {
                "repo_id": "acme/model",
                "family": "acme",
                "official_name": "Partial Model",
                "model_type": "llm",
                "filenames": ["weights.gguf"]
            },
            "created_at": "2026-09-03T00:00:00Z",
            "known_sha256": null
        }))
        .unwrap();
        pumas_library::model_library::test_support::admit_paused_download(launcher_root, &snapshot)
            .unwrap();

        let api = crate::handlers::test_support::build_test_api_with_hf(launcher_root).await;
        let server = match start_test_server(api, launcher_root).await {
            Ok(server) => server,
            Err(error) if is_socket_bind_permission_error(&error) => {
                eprintln!(
                    "Skipping retained download status RPC test: local TCP bind not permitted"
                );
                return;
            }
            Err(error) => {
                panic!("failed to start retained download status RPC test server: {error:#}")
            }
        };
        let address = server.addr();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        async fn status_call(
            client: &reqwest::Client,
            address: std::net::SocketAddr,
            request_id: u64,
        ) -> Result<serde_json::Value, &'static str> {
            tokio::time::timeout(std::time::Duration::from_secs(6), async {
                client
                    .post(format!("http://{address}/rpc"))
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "get_model_download_status",
                        "params": {"downloadId": "tracked-partial-1"},
                        "id": request_id
                    }))
                    .send()
                    .await
                    .map_err(|_| "HTTP request failed")?
                    .error_for_status()
                    .map_err(|_| "HTTP status was not successful")?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|_| "HTTP response body was not valid JSON")
            })
            .await
            .map_err(|_| "HTTP status request exceeded its deadline")?
        }

        let first = status_call(&client, address, 61).await;
        let second = status_call(&client, address, 62).await;
        let shutdown =
            tokio::time::timeout(std::time::Duration::from_secs(5), server.shutdown()).await;

        assert!(
            shutdown.is_ok(),
            "RPC server shutdown exceeded its deadline"
        );
        assert!(
            shutdown.unwrap().is_ok(),
            "RPC server shutdown did not observe all owner drains"
        );
        let first = first.expect("first status request must complete with a bounded response");
        let second = second.expect("second status request must complete with a bounded response");

        assert_eq!(first["jsonrpc"], "2.0");
        assert_eq!(first["id"], 61);
        assert!(
            first.get("error").is_none(),
            "unexpected JSON-RPC error response"
        );
        assert_eq!(second["jsonrpc"], "2.0");
        assert_eq!(second["id"], 62);
        assert!(
            second.get("error").is_none(),
            "unexpected JSON-RPC error response"
        );

        let first_result = &first["result"];
        let second_result = &second["result"];
        assert_eq!(first_result["success"], true);
        assert_eq!(first_result["downloadId"], "tracked-partial-1");
        assert_eq!(first_result["libraryModelId"], "llm/acme/partial-model");
        assert_eq!(first_result["repoId"], "acme/model");
        assert_eq!(
            first_result["selectedArtifactId"],
            "acme--model__files_971f5ccbb554"
        );
        assert_eq!(first_result["status"], "paused");
        assert_eq!(first_result["downloadedBytes"], 7);
        assert_eq!(first_result["totalBytes"], 100);
        let progress = first_result["progress"].as_f64().unwrap();
        assert!((0.0..=1.0).contains(&progress));
        assert!(
            (progress - 0.07).abs() < 0.001,
            "unexpected retained progress fraction"
        );
        assert_eq!(
            first_result.get("etaSeconds"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(first_result, second_result);
    }

    #[tokio::test]
    async fn retained_paused_download_cancellation_cleans_only_target_through_http_json_rpc_and_reopens(
    ) {
        const TARGET_ID: &str = "tracked-cancelled-1";
        const KEEPER_ID: &str = "tracked-paused-2";
        const SELECTED_ARTIFACT_ID: &str = "acme--model__files_971f5ccbb554";

        fn paused_snapshot(
            download_id: &str,
            official_name: &str,
            destination: &std::path::Path,
        ) -> pumas_library::model_library::download_store::PersistedDownload {
            serde_json::from_value(serde_json::json!({
                "download_id": download_id,
                "repo_id": "acme/model",
                "revision": null,
                "filename": "weights.gguf",
                "filenames": ["weights.gguf"],
                "dest_dir": destination,
                "total_bytes": 100,
                "status": "paused",
                "download_request": {
                    "repo_id": "acme/model",
                    "family": "acme",
                    "official_name": official_name,
                    "model_type": "llm",
                    "filenames": ["weights.gguf"]
                },
                "created_at": "2026-09-03T00:00:00Z",
                "known_sha256": null
            }))
            .expect("paused download fixture must have the current persisted shape")
        }

        fn seed_paused_download(
            launcher_root: &std::path::Path,
            relative_destination: &str,
            download_id: &str,
            official_name: &str,
            partial_bytes: &[u8],
            sentinel_bytes: &[u8],
        ) -> std::path::PathBuf {
            let destination = launcher_root.join(relative_destination);
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::write(destination.join("weights.gguf.part"), partial_bytes).unwrap();
            std::fs::write(
                destination.join(".pumas_download"),
                serde_json::to_vec(&serde_json::json!({
                    "repo_id": "acme/model",
                    "files": ["weights.gguf"]
                }))
                .unwrap(),
            )
            .unwrap();
            std::fs::write(destination.join("sentinel.txt"), sentinel_bytes).unwrap();
            let snapshot = paused_snapshot(download_id, official_name, &destination);
            pumas_library::model_library::test_support::admit_paused_download(
                launcher_root,
                &snapshot,
            )
            .unwrap();
            destination
        }

        async fn rpc_call(
            client: &reqwest::Client,
            address: std::net::SocketAddr,
            method: &str,
            params: serde_json::Value,
            request_id: u64,
        ) -> Result<serde_json::Value, &'static str> {
            let response = tokio::time::timeout(std::time::Duration::from_secs(6), async {
                client
                    .post(format!("http://{address}/rpc"))
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": params,
                        "id": request_id
                    }))
                    .send()
                    .await
                    .map_err(|_| "HTTP request failed")?
                    .error_for_status()
                    .map_err(|_| "HTTP status was not successful")?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|_| "HTTP response body was not valid JSON")
            })
            .await
            .map_err(|_| "HTTP JSON-RPC request exceeded its deadline")??;
            if response["jsonrpc"] != "2.0" {
                return Err("response did not identify JSON-RPC 2.0");
            }
            if response["id"] != request_id {
                return Err("response request ID did not match the request");
            }
            if response.get("error").is_some() {
                return Err("response contained a JSON-RPC error envelope");
            }
            Ok(response)
        }

        async fn wait_until_cancelled(
            client: &reqwest::Client,
            address: std::net::SocketAddr,
            download_id: &str,
            first_request_id: u64,
        ) -> Result<serde_json::Value, &'static str> {
            tokio::time::timeout(std::time::Duration::from_secs(8), async {
                let mut request_id = first_request_id;
                loop {
                    let response = rpc_call(
                        client,
                        address,
                        "get_model_download_status",
                        serde_json::json!({"downloadId": download_id}),
                        request_id,
                    )
                    .await?;
                    request_id = request_id.wrapping_add(1);
                    if response["result"]["status"] == "cancelled" {
                        return Ok(response);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            })
            .await
            .map_err(|_| "terminal cancellation status exceeded its deadline")?
        }

        fn inventory_has_download(inventory: &serde_json::Value, download_id: &str) -> bool {
            inventory["downloads"].as_array().is_some_and(|downloads| {
                downloads
                    .iter()
                    .any(|download| download["download_id"] == download_id)
            })
        }

        fn object_has_id(inventory: &serde_json::Value, field: &str, download_id: &str) -> bool {
            inventory[field]
                .as_object()
                .is_some_and(|entries| entries.contains_key(download_id))
        }

        let temp = TempDir::new().unwrap();
        let launcher_root = temp.path();
        std::fs::create_dir_all(launcher_root.join("launcher-data")).unwrap();
        let target_directory = seed_paused_download(
            launcher_root,
            "shared-resources/models/llm/acme/cancelled-model",
            TARGET_ID,
            "Cancelled Model",
            b"TARGET-PARTIAL",
            b"target sentinel",
        );
        let keeper_directory = seed_paused_download(
            launcher_root,
            "shared-resources/models/llm/acme/retained-model",
            KEEPER_ID,
            "Retained Model",
            b"KEEP-PARTIAL",
            b"keeper sentinel",
        );
        let target_marker_before = std::fs::read(target_directory.join(".pumas_download")).unwrap();
        let keeper_marker_before = std::fs::read(keeper_directory.join(".pumas_download")).unwrap();
        let inventory_before: serde_json::Value = serde_json::from_slice(
            &std::fs::read(launcher_root.join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();

        let api = crate::handlers::test_support::build_test_api_with_hf(launcher_root).await;
        let server = match start_test_server(api, launcher_root).await {
            Ok(server) => server,
            Err(error) if is_socket_bind_permission_error(&error) => {
                eprintln!(
                    "Skipping retained paused cancellation RPC test: local TCP bind not permitted"
                );
                return;
            }
            Err(_) => panic!("failed to start retained paused cancellation RPC server"),
        };
        let address = server.addr();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        let target_before = rpc_call(
            &client,
            address,
            "get_model_download_status",
            serde_json::json!({"downloadId": TARGET_ID}),
            71,
        )
        .await;
        let keeper_before = rpc_call(
            &client,
            address,
            "get_model_download_status",
            serde_json::json!({"downloadId": KEEPER_ID}),
            72,
        )
        .await;
        let cancel_response = rpc_call(
            &client,
            address,
            "cancel_model_download",
            serde_json::json!({"downloadId": TARGET_ID}),
            73,
        )
        .await;
        let target_cancelled = wait_until_cancelled(&client, address, TARGET_ID, 74).await;
        let keeper_after_cancel = rpc_call(
            &client,
            address,
            "get_model_download_status",
            serde_json::json!({"downloadId": KEEPER_ID}),
            75,
        )
        .await;
        let repeated_cancel = rpc_call(
            &client,
            address,
            "cancel_model_download",
            serde_json::json!({"downloadId": TARGET_ID}),
            76,
        )
        .await;
        let first_shutdown =
            tokio::time::timeout(std::time::Duration::from_secs(6), server.shutdown()).await;
        assert!(
            first_shutdown.is_ok(),
            "first RPC server shutdown exceeded its deadline"
        );
        assert!(
            first_shutdown.unwrap().is_ok(),
            "first RPC server did not observe all owner drains"
        );
        drop(server);
        drop(client);

        let first_inventory: serde_json::Value = serde_json::from_slice(
            &std::fs::read(launcher_root.join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();
        let target_partial_removed = !target_directory.join("weights.gguf.part").exists();
        let target_marker_removed = !target_directory.join(".pumas_download").exists();
        let target_sentinel_after = std::fs::read(target_directory.join("sentinel.txt")).unwrap();
        let keeper_partial_after =
            std::fs::read(keeper_directory.join("weights.gguf.part")).unwrap();
        let keeper_marker_after = std::fs::read(keeper_directory.join(".pumas_download")).unwrap();
        let keeper_sentinel_after = std::fs::read(keeper_directory.join("sentinel.txt")).unwrap();

        let reopened_api =
            crate::handlers::test_support::build_test_api_with_hf(launcher_root).await;
        let reopened_server = start_test_server(reopened_api, launcher_root)
            .await
            .expect("reopened RPC server must bind after the first server drained");
        let reopened_address = reopened_server.addr();
        let reopened_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let reopened_target = rpc_call(
            &reopened_client,
            reopened_address,
            "get_model_download_status",
            serde_json::json!({"downloadId": TARGET_ID}),
            77,
        )
        .await;
        let reopened_keeper = rpc_call(
            &reopened_client,
            reopened_address,
            "get_model_download_status",
            serde_json::json!({"downloadId": KEEPER_ID}),
            78,
        )
        .await;
        let reopened_shutdown = tokio::time::timeout(
            std::time::Duration::from_secs(6),
            reopened_server.shutdown(),
        )
        .await;
        assert!(
            reopened_shutdown.is_ok(),
            "reopened RPC server shutdown exceeded its deadline"
        );
        assert!(
            reopened_shutdown.unwrap().is_ok(),
            "reopened RPC server did not observe all owner drains"
        );

        let target_partial_after_reopen = target_directory.join("weights.gguf.part").exists();
        let target_marker_after_reopen = target_directory.join(".pumas_download").exists();
        let target_sentinel_after_reopen =
            std::fs::read(target_directory.join("sentinel.txt")).unwrap();
        let keeper_partial_after_reopen =
            std::fs::read(keeper_directory.join("weights.gguf.part")).unwrap();
        let keeper_marker_after_reopen =
            std::fs::read(keeper_directory.join(".pumas_download")).unwrap();
        let keeper_sentinel_after_reopen =
            std::fs::read(keeper_directory.join("sentinel.txt")).unwrap();
        let reopened_inventory: serde_json::Value = serde_json::from_slice(
            &std::fs::read(launcher_root.join("launcher-data/downloads.json")).unwrap(),
        )
        .unwrap();

        let inventory_has_keeper = inventory_has_download(&inventory_before, KEEPER_ID);
        assert!(inventory_has_keeper);
        assert!(object_has_id(
            &inventory_before,
            "queue_admissions",
            TARGET_ID
        ));
        assert!(object_has_id(
            &inventory_before,
            "queue_admissions",
            KEEPER_ID
        ));

        let target_before = target_before.expect("target paused status request must complete");
        let keeper_before = keeper_before.expect("keeper paused status request must complete");
        let cancel_response = cancel_response.expect("cancel mutation request must complete");
        let target_cancelled =
            target_cancelled.expect("target must reach terminal cancellation by deadline");
        let keeper_after_cancel =
            keeper_after_cancel.expect("keeper status after target cancellation must complete");
        let repeated_cancel = repeated_cancel.expect("repeat cancellation request must complete");
        let reopened_target =
            reopened_target.expect("reopened target status request must complete");
        let reopened_keeper =
            reopened_keeper.expect("reopened keeper status request must complete");

        for (response, expected_id) in [(&target_before, 71), (&keeper_before, 72)] {
            assert_eq!(response["jsonrpc"], "2.0");
            assert_eq!(response["id"], expected_id);
            assert!(response.get("error").is_none());
            assert_eq!(response["result"]["success"], true);
            assert_eq!(response["result"]["status"], "paused");
            assert_eq!(response["result"]["repoId"], "acme/model");
            assert_eq!(
                response["result"]["selectedArtifactId"],
                SELECTED_ARTIFACT_ID
            );
            assert_eq!(response["result"]["totalBytes"], 100);
        }
        assert_eq!(target_before["result"]["downloadId"], TARGET_ID);
        assert_eq!(
            target_before["result"]["libraryModelId"],
            "llm/acme/cancelled-model"
        );
        assert_eq!(target_before["result"]["downloadedBytes"], 14);
        assert_eq!(keeper_before["result"]["downloadId"], KEEPER_ID);
        assert_eq!(
            keeper_before["result"]["libraryModelId"],
            "llm/acme/retained-model"
        );
        assert_eq!(keeper_before["result"]["downloadedBytes"], 12);
        assert_eq!(keeper_after_cancel["result"], keeper_before["result"]);

        assert_eq!(cancel_response["jsonrpc"], "2.0");
        assert_eq!(cancel_response["id"], 73);
        assert!(cancel_response.get("error").is_none());
        assert_eq!(cancel_response["result"]["success"], true);
        assert!(cancel_response["result"].get("error").is_none());
        assert_eq!(target_cancelled["jsonrpc"], "2.0");
        assert!(target_cancelled.get("error").is_none());
        assert_eq!(target_cancelled["result"]["success"], true);
        assert_eq!(target_cancelled["result"]["downloadId"], TARGET_ID);
        assert_eq!(target_cancelled["result"]["repoId"], "acme/model");
        assert_eq!(
            target_cancelled["result"]["selectedArtifactId"],
            SELECTED_ARTIFACT_ID
        );
        assert_eq!(
            target_cancelled["result"]["libraryModelId"],
            serde_json::Value::Null
        );

        assert_eq!(repeated_cancel["jsonrpc"], "2.0");
        assert_eq!(repeated_cancel["id"], 76);
        assert!(repeated_cancel.get("error").is_none());
        assert_eq!(repeated_cancel["result"]["success"], false);
        assert_eq!(repeated_cancel["result"]["error"], "Download not found");

        assert!(target_partial_removed);
        assert!(target_marker_removed);
        assert_eq!(target_sentinel_after, b"target sentinel");
        assert!(!target_marker_before.is_empty());
        assert_eq!(
            keeper_partial_after, b"KEEP-PARTIAL",
            "target cleanup must preserve the other paused partial"
        );
        assert_eq!(keeper_marker_after, keeper_marker_before);
        assert_eq!(keeper_sentinel_after, b"keeper sentinel");
        assert!(!target_partial_after_reopen);
        assert!(!target_marker_after_reopen);
        assert_eq!(target_sentinel_after_reopen, b"target sentinel");
        assert_eq!(keeper_partial_after_reopen, b"KEEP-PARTIAL");
        assert_eq!(keeper_marker_after_reopen, keeper_marker_before);
        assert_eq!(keeper_sentinel_after_reopen, b"keeper sentinel");

        for inventory in [&first_inventory, &reopened_inventory] {
            assert!(!inventory_has_download(inventory, TARGET_ID));
            assert!(inventory_has_download(inventory, KEEPER_ID));
            assert!(!object_has_id(inventory, "queue_admissions", TARGET_ID));
            assert!(object_has_id(inventory, "queue_admissions", KEEPER_ID));
            assert!(inventory["lifecycle_quarantines"]
                .get(TARGET_ID)
                .is_none_or(|quarantine| quarantine["disposition"] != "pending"));
            assert!(inventory["lifecycle_quarantines"]
                .get(KEEPER_ID)
                .is_none_or(|quarantine| quarantine["disposition"] != "pending"));
        }

        assert_eq!(reopened_target["jsonrpc"], "2.0");
        assert_eq!(reopened_target["id"], 77);
        assert!(reopened_target.get("error").is_none());
        assert_eq!(reopened_target["result"]["success"], false);
        assert_eq!(reopened_target["result"]["error"], "Download not found");
        assert_eq!(reopened_keeper["jsonrpc"], "2.0");
        assert_eq!(reopened_keeper["id"], 78);
        assert!(reopened_keeper.get("error").is_none());
        assert_eq!(reopened_keeper["result"], keeper_before["result"]);
    }

    #[tokio::test]
    async fn retained_weak_selection_resume_refusal_preserves_paused_state_through_http_json_rpc_and_reopens(
    ) {
        const DOWNLOAD_ID: &str = "tracked-weak-resume-3";
        const SELECTED_ARTIFACT_ID: &str = "acme--model__files_971f5ccbb554";
        const PARTIAL_BYTES: &[u8] = b"WEAK-PARTIAL";
        const MARKER_BYTES: &[u8] = br#"{"repo_id":"acme/model","files":["weights.gguf"]}"#;
        const SENTINEL_BYTES: &[u8] = b"weak-selection-sentinel";

        async fn rpc_call(
            client: &reqwest::Client,
            address: std::net::SocketAddr,
            method: &str,
            params: serde_json::Value,
            request_id: u64,
        ) -> Result<serde_json::Value, &'static str> {
            let response = tokio::time::timeout(std::time::Duration::from_secs(6), async {
                client
                    .post(format!("http://{address}/rpc"))
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": params,
                        "id": request_id
                    }))
                    .send()
                    .await
                    .map_err(|_| "HTTP request failed")?
                    .error_for_status()
                    .map_err(|_| "HTTP status was not successful")?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|_| "HTTP response body was not valid JSON")
            })
            .await
            .map_err(|_| "HTTP JSON-RPC request exceeded its deadline")??;
            if response["jsonrpc"] != "2.0" {
                return Err("response did not identify JSON-RPC 2.0");
            }
            if response["id"] != request_id {
                return Err("response request ID did not match the request");
            }
            Ok(response)
        }

        fn read_inventory(
            launcher_root: &std::path::Path,
        ) -> Result<serde_json::Value, &'static str> {
            let bytes = std::fs::read(launcher_root.join("launcher-data/downloads.json"))
                .map_err(|_| "download inventory could not be read")?;
            serde_json::from_slice(&bytes).map_err(|_| "download inventory was not valid JSON")
        }

        fn assert_paused_status(
            response: &serde_json::Value,
            expected_id: u64,
            expected_bytes: usize,
        ) {
            assert_eq!(response["jsonrpc"], "2.0");
            assert_eq!(response["id"], expected_id);
            assert!(response.get("error").is_none());
            assert_eq!(response["result"]["success"], true);
            assert_eq!(response["result"]["downloadId"], DOWNLOAD_ID);
            assert_eq!(
                response["result"]["libraryModelId"],
                "llm/acme/weak-resume-model"
            );
            assert_eq!(response["result"]["repoId"], "acme/model");
            assert_eq!(
                response["result"]["selectedArtifactId"],
                SELECTED_ARTIFACT_ID
            );
            assert_eq!(response["result"]["status"], "paused");
            assert_eq!(response["result"]["downloadedBytes"], expected_bytes);
            assert_eq!(response["result"]["totalBytes"], 100);
        }

        fn assert_resume_refusal(response: &serde_json::Value, expected_id: u64) {
            assert_eq!(
                response,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": expected_id,
                    "error": {
                        "code": -32005,
                        "message": "Request parameters are invalid.",
                        "data": {"class": "invalid_request"}
                    }
                })
            );
            assert!(response.get("result").is_none());
        }

        let temp = TempDir::new().unwrap();
        let launcher_root = temp.path();
        std::fs::create_dir_all(launcher_root.join("launcher-data")).unwrap();
        let destination = launcher_root.join("shared-resources/models/llm/acme/weak-resume-model");
        std::fs::create_dir_all(&destination).unwrap();
        let partial_path = destination.join("weights.gguf.part");
        let marker_path = destination.join(".pumas_download");
        let sentinel_path = destination.join("sentinel.txt");
        std::fs::write(&partial_path, PARTIAL_BYTES).unwrap();
        std::fs::write(&marker_path, MARKER_BYTES).unwrap();
        std::fs::write(&sentinel_path, SENTINEL_BYTES).unwrap();
        let snapshot = serde_json::from_value::<
            pumas_library::model_library::download_store::PersistedDownload,
        >(serde_json::json!({
            "download_id": DOWNLOAD_ID,
            "repo_id": "acme/model",
            "revision": null,
            "filename": "weights.gguf",
            "filenames": ["weights.gguf"],
            "dest_dir": destination,
            "total_bytes": 100,
            "status": "paused",
            "download_request": {
                "repo_id": "acme/model",
                "family": "acme",
                "official_name": "Weak Resume Model",
                "model_type": "llm",
                "filenames": ["weights.gguf"]
            },
            "created_at": "2026-09-03T00:00:00Z",
            "known_sha256": null
        }))
        .unwrap();
        pumas_library::model_library::test_support::admit_paused_download(launcher_root, &snapshot)
            .unwrap();

        let api = crate::handlers::test_support::build_test_api_with_hf(launcher_root).await;
        let server = match start_test_server(api, launcher_root).await {
            Ok(server) => server,
            Err(error) if is_socket_bind_permission_error(&error) => {
                eprintln!("Skipping weak resume RPC test: local TCP bind not permitted");
                return;
            }
            Err(_) => panic!("failed to start weak resume RPC test server"),
        };
        let address = server.addr();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        let status_before = rpc_call(
            &client,
            address,
            "get_model_download_status",
            serde_json::json!({"downloadId": DOWNLOAD_ID}),
            81,
        )
        .await;
        let inventory_before_resume = read_inventory(launcher_root);
        let resume_response = rpc_call(
            &client,
            address,
            "resume_model_download",
            serde_json::json!({"downloadId": DOWNLOAD_ID}),
            82,
        )
        .await;
        let status_after = rpc_call(
            &client,
            address,
            "get_model_download_status",
            serde_json::json!({"downloadId": DOWNLOAD_ID}),
            83,
        )
        .await;
        let inventory_after_resume_refusal = read_inventory(launcher_root);
        let first_shutdown =
            tokio::time::timeout(std::time::Duration::from_secs(6), server.shutdown()).await;
        assert!(
            first_shutdown.is_ok(),
            "first weak resume RPC server shutdown exceeded its deadline"
        );
        assert!(
            first_shutdown.unwrap().is_ok(),
            "first weak resume RPC server did not observe all owner drains"
        );
        drop(server);
        drop(client);

        let first_inventory_after_shutdown = read_inventory(launcher_root);
        let partial_after_shutdown = std::fs::read(&partial_path).unwrap();
        let marker_after_shutdown = std::fs::read(&marker_path).unwrap();
        let sentinel_after_shutdown = std::fs::read(&sentinel_path).unwrap();

        let reopened_api =
            crate::handlers::test_support::build_test_api_with_hf(launcher_root).await;
        let reopened_server = start_test_server(reopened_api, launcher_root)
            .await
            .expect("reopened weak resume RPC server must bind after the first server drained");
        let reopened_address = reopened_server.addr();
        let reopened_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let reopened_status = rpc_call(
            &reopened_client,
            reopened_address,
            "get_model_download_status",
            serde_json::json!({"downloadId": DOWNLOAD_ID}),
            84,
        )
        .await;
        let reopened_resume_response = rpc_call(
            &reopened_client,
            reopened_address,
            "resume_model_download",
            serde_json::json!({"downloadId": DOWNLOAD_ID}),
            85,
        )
        .await;
        let reopened_status_after = rpc_call(
            &reopened_client,
            reopened_address,
            "get_model_download_status",
            serde_json::json!({"downloadId": DOWNLOAD_ID}),
            86,
        )
        .await;
        let reopened_shutdown = tokio::time::timeout(
            std::time::Duration::from_secs(6),
            reopened_server.shutdown(),
        )
        .await;
        assert!(
            reopened_shutdown.is_ok(),
            "reopened weak resume RPC server shutdown exceeded its deadline"
        );
        assert!(
            reopened_shutdown.unwrap().is_ok(),
            "reopened weak resume RPC server did not observe all owner drains"
        );
        drop(reopened_server);
        drop(reopened_client);

        let inventory_after_reopen = read_inventory(launcher_root);
        let partial_after_reopen = std::fs::read(&partial_path).unwrap();
        let marker_after_reopen = std::fs::read(&marker_path).unwrap();
        let sentinel_after_reopen = std::fs::read(&sentinel_path).unwrap();

        let status_before = status_before.expect("initial paused status request must complete");
        let resume_response = resume_response.expect("resume request must have a bounded response");
        let status_after = status_after.expect("post-resume paused status request must complete");
        let reopened_status =
            reopened_status.expect("reopened paused status request must complete");
        let reopened_resume_response =
            reopened_resume_response.expect("reopened resume request must have a bounded response");
        let reopened_status_after =
            reopened_status_after.expect("reopened post-resume status request must complete");
        let inventory_before_resume = inventory_before_resume
            .expect("pre-resume download inventory must be readable and valid");
        let inventory_after_resume_refusal = inventory_after_resume_refusal
            .expect("post-refusal download inventory must be readable and valid");
        let first_inventory_after_shutdown = first_inventory_after_shutdown
            .expect("post-shutdown download inventory must be readable and valid");
        let inventory_after_reopen =
            inventory_after_reopen.expect("reopened download inventory must be readable and valid");

        assert_paused_status(&status_before, 81, PARTIAL_BYTES.len());
        assert_paused_status(&status_after, 83, PARTIAL_BYTES.len());
        assert_eq!(status_after["result"], status_before["result"]);
        assert_resume_refusal(&resume_response, 82);
        assert_paused_status(&reopened_status, 84, PARTIAL_BYTES.len());
        assert_paused_status(&reopened_status_after, 86, PARTIAL_BYTES.len());
        assert_eq!(reopened_status["result"], status_before["result"]);
        assert_eq!(reopened_status_after["result"], status_before["result"]);
        assert_resume_refusal(&reopened_resume_response, 85);

        for inventory in [
            &inventory_after_resume_refusal,
            &first_inventory_after_shutdown,
            &inventory_after_reopen,
        ] {
            assert_eq!(inventory, &inventory_before_resume);
            assert!(inventory["downloads"].as_array().is_some_and(|downloads| {
                downloads
                    .iter()
                    .any(|download| download["download_id"] == DOWNLOAD_ID)
            }));
            assert!(inventory["queue_admissions"]
                .as_object()
                .is_some_and(|admissions| admissions.contains_key(DOWNLOAD_ID)));
        }
        let persisted_download = inventory_before_resume["downloads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|download| download["download_id"] == DOWNLOAD_ID)
            .unwrap();
        assert_eq!(persisted_download["revision"], serde_json::Value::Null);
        assert_eq!(persisted_download["known_sha256"], serde_json::Value::Null);
        assert_eq!(partial_after_shutdown, PARTIAL_BYTES);
        assert_eq!(marker_after_shutdown, MARKER_BYTES);
        assert_eq!(sentinel_after_shutdown, SENTINEL_BYTES);
        assert_eq!(partial_after_reopen, PARTIAL_BYTES);
        assert_eq!(marker_after_reopen, MARKER_BYTES);
        assert_eq!(sentinel_after_reopen, SENTINEL_BYTES);
    }

    #[tokio::test]
    async fn retained_digest_resume_status_waits_for_import_settlement_through_http_json_rpc() {
        use std::path::Path;
        use std::sync::{mpsc, Arc, Mutex};
        use std::time::Duration;

        const DOWNLOAD_ID: &str = "tracked-digest-resume-4";
        const SELECTED_ARTIFACT_ID: &str = "acme--model__files_971f5ccbb554";
        const MODEL_ID: &str = "llm/acme/digest-resume-model";
        const PARTIAL_BYTES: &[u8] = b"GGUF\x02\x00\x00\x00";
        const SHA256: &str = "69cb86ffffe1039003092edf0dc9415a36b5016eae5f93b7300f97e7fe67dcd9";
        const MARKER_BYTES: &[u8] = br#"{"repo_id":"acme/model","files":["weights.gguf"]}"#;
        const SENTINEL_BYTES: &[u8] = b"digest-resume-sentinel";
        const TEST_TIMEOUT: Duration = Duration::from_secs(10);

        struct ReleaseOnDrop(Option<mpsc::Sender<()>>);
        impl ReleaseOnDrop {
            fn release(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }
        impl Drop for ReleaseOnDrop {
            fn drop(&mut self) {
                self.release();
            }
        }

        fn fixture_gguf() -> Vec<u8> {
            let mut bytes = b"GGUF".to_vec();
            bytes.extend_from_slice(&2_u32.to_le_bytes());
            bytes.extend_from_slice(&0_u64.to_le_bytes());
            bytes.extend_from_slice(&0_u64.to_le_bytes());
            bytes
        }

        fn read_store_document(root: &Path) -> Result<serde_json::Value, &'static str> {
            let bytes = std::fs::read(root.join("launcher-data/downloads.json"))
                .map_err(|_| "download store could not be read")?;
            serde_json::from_slice(&bytes).map_err(|_| "download store was not valid JSON")
        }

        async fn rpc_call(
            client: &reqwest::Client,
            address: std::net::SocketAddr,
            method: &str,
            download_id: &str,
            request_id: u64,
        ) -> Result<serde_json::Value, &'static str> {
            let response = tokio::time::timeout(Duration::from_secs(5), async {
                client
                    .post(format!("http://{address}/rpc"))
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": {"downloadId": download_id},
                        "id": request_id
                    }))
                    .send()
                    .await
                    .map_err(|_| "HTTP request failed")?
                    .error_for_status()
                    .map_err(|_| "HTTP status was not successful")?
                    .json::<serde_json::Value>()
                    .await
                    .map_err(|_| "HTTP response body was not valid JSON")
            })
            .await
            .map_err(|_| "HTTP JSON-RPC request exceeded its deadline")??;
            if response["jsonrpc"] != "2.0" {
                return Err("response did not identify JSON-RPC 2.0");
            }
            if response["id"] != request_id {
                return Err("response request ID did not match the request");
            }
            if response.get("error").is_some() {
                return Err("response contained a JSON-RPC error envelope");
            }
            Ok(response)
        }

        async fn wait_for_completed(
            client: &reqwest::Client,
            address: std::net::SocketAddr,
            download_id: &str,
            first_request_id: u64,
        ) -> Result<serde_json::Value, &'static str> {
            tokio::time::timeout(TEST_TIMEOUT, async {
                let mut request_id = first_request_id;
                loop {
                    let response = rpc_call(
                        client,
                        address,
                        "get_model_download_status",
                        download_id,
                        request_id,
                    )
                    .await?;
                    let status = response["result"]["status"]
                        .as_str()
                        .ok_or("status response omitted its status")?;
                    if status == "completed" {
                        return Ok(response);
                    }
                    if status == "cancelled" || status == "error" {
                        return Err("resumed download reached a non-completed terminal state");
                    }
                    request_id = request_id.wrapping_add(1);
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            })
            .await
            .map_err(|_| "completed status exceeded its deadline")?
        }

        let complete_bytes = fixture_gguf();
        assert_eq!(complete_bytes.len(), 24);
        let temp = TempDir::new().unwrap();
        let launcher_root = temp.path();
        std::fs::create_dir_all(launcher_root.join("launcher-data")).unwrap();
        let destination =
            launcher_root.join("shared-resources/models/llm/acme/digest-resume-model");
        std::fs::create_dir_all(&destination).unwrap();
        let partial_path = destination.join("weights.gguf.part");
        let payload_path = destination.join("weights.gguf");
        let marker_path = destination.join(".pumas_download");
        let sentinel_path = destination.join("sentinel.txt");
        std::fs::write(&partial_path, PARTIAL_BYTES).unwrap();
        std::fs::write(&marker_path, MARKER_BYTES).unwrap();
        std::fs::write(&sentinel_path, SENTINEL_BYTES).unwrap();
        let snapshot = serde_json::from_value::<
            pumas_library::model_library::download_store::PersistedDownload,
        >(serde_json::json!({
            "download_id": DOWNLOAD_ID,
            "repo_id": "acme/model",
            "revision": null,
            "filename": "weights.gguf",
            "filenames": ["weights.gguf"],
            "dest_dir": destination,
            "total_bytes": 24,
            "status": "paused",
            "download_request": {
                "repo_id": "acme/model",
                "family": "acme",
                "official_name": "Digest Resume Model",
                "model_type": "llm",
                "filenames": ["weights.gguf"]
            },
            "created_at": "2026-09-03T00:00:00Z",
            "known_sha256": SHA256
        }))
        .unwrap();
        pumas_library::model_library::test_support::admit_paused_download(launcher_root, &snapshot)
            .unwrap();

        let api = crate::handlers::test_support::build_test_api_with_hf(launcher_root).await;
        let library = api.model_library().clone();
        let acquisition_store = api.acquisition().store().clone();
        let library_model_id = library
            .get_model_id(&destination)
            .expect("fixture destination must be inside the model library");
        let server = match start_test_server(api, launcher_root).await {
            Ok(server) => server,
            Err(error) if is_socket_bind_permission_error(&error) => {
                eprintln!("Skipping digest resume RPC test: local TCP bind not permitted");
                return;
            }
            Err(_) => panic!("failed to start digest resume RPC test server"),
        };
        let address = server.addr();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        let status_before = rpc_call(
            &client,
            address,
            "get_model_download_status",
            DOWNLOAD_ID,
            91,
        )
        .await;
        let partial_replacement = std::fs::write(&partial_path, &complete_bytes);

        let (import_started_tx, import_started_rx) = tokio::sync::oneshot::channel();
        let import_started_tx = Mutex::new(Some(import_started_tx));
        let (release_import_tx, release_import_rx) = mpsc::channel();
        let release_import_rx = Mutex::new(Some(release_import_rx));
        let mut release_guard = ReleaseOnDrop(Some(release_import_tx));
        let import_destination = destination.clone();
        library.set_metadata_write_notifier(Some(Arc::new(move |_| {
            if import_destination.join(".pumas_download").exists()
                || !import_destination.join("weights.gguf").is_file()
            {
                return;
            }
            if let Some(sender) = import_started_tx.lock().unwrap().take() {
                let _ = sender.send(());
                let receiver = release_import_rx.lock().unwrap().take();
                if let Some(receiver) = receiver {
                    let _ = receiver.recv();
                }
            }
        })));

        let resume_response = if partial_replacement.is_ok() {
            Some(rpc_call(&client, address, "resume_model_download", DOWNLOAD_ID, 92).await)
        } else {
            None
        };
        let resume_succeeded = resume_response
            .as_ref()
            .and_then(|response| response.as_ref().ok())
            .is_some_and(|response| response["result"]["success"] == true);
        let import_gate_observed = if resume_succeeded {
            tokio::time::timeout(TEST_TIMEOUT, import_started_rx)
                .await
                .is_ok_and(|result| result.is_ok())
        } else {
            false
        };

        let status_at_import = if import_gate_observed {
            Some(
                rpc_call(
                    &client,
                    address,
                    "get_model_download_status",
                    DOWNLOAD_ID,
                    93,
                )
                .await,
            )
        } else {
            None
        };
        let acquisitions_at_import = if import_gate_observed {
            let store = acquisition_store.clone();
            Some(
                tokio::task::spawn_blocking(move || store.acquisitions())
                    .await
                    .map_err(|_| "acquisition snapshot task failed")
                    .and_then(|result| {
                        result.map_err(|_| "acquisition snapshot could not be read")
                    }),
            )
        } else {
            None
        };
        let document_at_import = if import_gate_observed {
            Some(read_store_document(launcher_root))
        } else {
            None
        };
        let payload_at_import = if import_gate_observed {
            Some(std::fs::read(&payload_path))
        } else {
            None
        };
        let partial_exists_at_import = partial_path.exists();

        release_guard.release();
        let completed_response = if resume_succeeded {
            Some(wait_for_completed(&client, address, DOWNLOAD_ID, 100).await)
        } else {
            None
        };
        let repeated_completed_response = if completed_response
            .as_ref()
            .is_some_and(|result| result.is_ok())
        {
            Some(
                rpc_call(
                    &client,
                    address,
                    "get_model_download_status",
                    DOWNLOAD_ID,
                    150,
                )
                .await,
            )
        } else {
            None
        };

        library.set_metadata_write_notifier(None);
        let shutdown = tokio::time::timeout(Duration::from_secs(8), server.shutdown()).await;
        drop(client);
        drop(server);

        let final_document = read_store_document(launcher_root);
        let final_acquisitions = {
            let store = acquisition_store.clone();
            tokio::task::spawn_blocking(move || store.acquisitions())
                .await
                .map_err(|_| "final acquisition snapshot task failed")
                .and_then(|result| {
                    result.map_err(|_| "final acquisition snapshot could not be read")
                })
        };
        let final_payload = std::fs::read(&payload_path);
        let final_partial_exists = partial_path.exists();
        let final_marker_exists = marker_path.exists();
        let final_sentinel = std::fs::read(&sentinel_path);
        let final_metadata = library.load_metadata(&destination);
        let final_index = library.index().get(&library_model_id);

        assert!(
            shutdown.is_ok(),
            "RPC server shutdown exceeded its deadline"
        );
        assert!(
            shutdown.unwrap().is_ok(),
            "RPC server did not observe all owner drains"
        );
        assert!(
            partial_replacement.is_ok(),
            "complete retained fixture bytes must be written before resume"
        );
        let status_before = status_before.expect("initial paused status request must complete");
        let resume_response = resume_response
            .expect("successful fixture replacement must issue the resume request")
            .expect("resume request must have a bounded response");
        assert_eq!(status_before["result"]["success"], true);
        assert_eq!(status_before["result"]["downloadId"], DOWNLOAD_ID);
        assert_eq!(status_before["result"]["libraryModelId"], MODEL_ID);
        assert_eq!(status_before["result"]["repoId"], "acme/model");
        assert_eq!(
            status_before["result"]["selectedArtifactId"],
            SELECTED_ARTIFACT_ID
        );
        assert_eq!(status_before["result"]["status"], "paused");
        assert_eq!(
            status_before["result"]["downloadedBytes"],
            PARTIAL_BYTES.len()
        );
        assert_eq!(status_before["result"]["totalBytes"], complete_bytes.len());
        assert_eq!(resume_response["result"]["success"], true);
        assert!(import_gate_observed);

        let status_at_import = status_at_import
            .expect("import barrier must be reached before observing RPC status")
            .expect("status request at import barrier must complete");
        assert_eq!(status_at_import["result"]["success"], true);
        assert_eq!(status_at_import["result"]["downloadId"], DOWNLOAD_ID);
        assert_eq!(status_at_import["result"]["libraryModelId"], MODEL_ID);
        assert_eq!(status_at_import["result"]["repoId"], "acme/model");
        assert_eq!(
            status_at_import["result"]["selectedArtifactId"],
            SELECTED_ARTIFACT_ID
        );
        assert_eq!(status_at_import["result"]["status"], "downloading");
        assert_eq!(
            status_at_import["result"]["downloadedBytes"],
            complete_bytes.len()
        );
        assert_eq!(
            status_at_import["result"]["totalBytes"],
            complete_bytes.len()
        );
        let import_progress = status_at_import["result"]["progress"]
            .as_f64()
            .expect("complete-byte import status must include numeric progress");
        assert!(import_progress.is_finite() && (0.0..=1.0).contains(&import_progress));
        assert!(!partial_exists_at_import);
        assert_eq!(
            payload_at_import
                .expect("payload must be captured at the import barrier")
                .expect("completed source bytes must be readable at the import barrier"),
            complete_bytes
        );
        let acquisitions_at_import = acquisitions_at_import
            .expect("acquisition snapshot must be captured at the import barrier")
            .expect("acquisition snapshot must complete successfully");
        assert_eq!(acquisitions_at_import.len(), 1);
        let using = acquisitions_at_import
            .values()
            .next()
            .expect("one resumed HF acquisition must be retained");
        assert_eq!(using.demand.consumer, "hf.model");
        assert_eq!(using.manifest.source().source_id(), "acme/model");
        assert!(matches!(
            &using.phase,
            pumas_library::acquisition::AcquisitionPhase::Using { .. }
        ));
        assert_eq!(using.files.len(), 1);
        assert_eq!(using.files[0].path, "weights.gguf");
        assert_eq!(using.files[0].bytes, complete_bytes.len() as u64);
        assert_eq!(using.files[0].sha256, SHA256);
        let using_id = using.id.to_string();
        let document_at_import = document_at_import
            .expect("durable document must be captured at the import barrier")
            .expect("durable document must be readable at the import barrier");
        assert!(document_at_import["queue_admissions"]
            .as_object()
            .is_some_and(|admissions| admissions.contains_key(DOWNLOAD_ID)));
        assert!(document_at_import["consumer_receipts"]
            .as_object()
            .is_some_and(|receipts| !receipts.contains_key(&using_id)));

        let completed_response = completed_response
            .expect("successful resume must be polled to settlement")
            .expect("resumed import must reach Completed within its deadline");
        let repeated_completed_response = repeated_completed_response
            .expect("settled status must be polled a second time")
            .expect("repeated terminal status request must complete");
        assert_eq!(completed_response["result"]["status"], "completed");
        assert_eq!(completed_response["result"]["downloadId"], DOWNLOAD_ID);
        assert_eq!(completed_response["result"]["repoId"], "acme/model");
        assert_eq!(
            completed_response["result"]["selectedArtifactId"],
            SELECTED_ARTIFACT_ID
        );
        assert_eq!(
            completed_response["result"]["downloadedBytes"],
            complete_bytes.len()
        );
        assert_eq!(
            completed_response["result"],
            repeated_completed_response["result"]
        );
        assert!(
            completed_response["result"]["libraryModelId"].is_null()
                || completed_response["result"]["libraryModelId"] == MODEL_ID
        );
        assert_eq!(
            final_payload.expect("completed model bytes must be readable"),
            complete_bytes
        );
        assert!(!final_partial_exists);
        assert!(!final_marker_exists);
        assert_eq!(
            final_sentinel.expect("sentinel must remain readable"),
            SENTINEL_BYTES
        );
        let final_metadata = final_metadata
            .expect("completed model metadata lookup must succeed")
            .expect("completed resume must publish model metadata");
        assert_eq!(final_metadata.repo_id.as_deref(), Some("acme/model"));
        assert!(final_index
            .expect("completed model index lookup must succeed")
            .is_some());

        let final_document =
            final_document.expect("final durable store document must be readable and valid JSON");
        let final_acquisitions =
            final_acquisitions.expect("final acquisition snapshot must complete successfully");
        assert_eq!(final_acquisitions.len(), 1);
        let adopted = final_acquisitions
            .values()
            .next()
            .expect("settled HF acquisition must remain retained");
        assert_eq!(adopted.id.to_string(), using_id);
        assert!(matches!(
            &adopted.phase,
            pumas_library::acquisition::AcquisitionPhase::Adopted { .. }
        ));
        assert!(!final_document["queue_admissions"]
            .as_object()
            .is_some_and(|admissions| admissions.contains_key(DOWNLOAD_ID)));
        assert!(final_document["released_queue_admissions"]
            .as_object()
            .is_some_and(|admissions| admissions.contains_key(DOWNLOAD_ID)));
        let receipt = final_document["consumer_receipts"]
            .get(&using_id)
            .expect("settled acquisition must have a completion receipt");
        let receipt_payload = if receipt["receipt_kind"] == "pumas.consumer-completion" {
            &receipt["payload"]
        } else {
            receipt
        };
        assert_eq!(receipt_payload["acquisition_id"], using_id);
        assert_eq!(receipt_payload["download_id"], DOWNLOAD_ID);
        assert_eq!(receipt_payload["model_id"], MODEL_ID);
        assert_eq!(receipt_payload["verified_files"][0]["path"], "weights.gguf");
        assert_eq!(
            receipt_payload["verified_files"][0]["bytes"],
            complete_bytes.len()
        );
        assert_eq!(receipt_payload["verified_files"][0]["sha256"], SHA256);
        if let pumas_library::acquisition::AcquisitionPhase::Adopted { lease } = &adopted.phase {
            assert_eq!(receipt_payload["use_lease"], lease.to_string());
        }
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

    #[cfg(feature = "inference-plugins")]
    #[tokio::test]
    async fn real_server_shutdown_closes_native_installation_admission() {
        let root = TempDir::new().unwrap();
        let api = crate::handlers::test_support::build_test_api_with_hf(root.path()).await;
        let manager = VersionManager::new_with_acquisition(
            root.path(),
            AppId::LlamaCpp,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        let managers = HashMap::from([("llama-cpp".into(), manager.clone())]);
        let sizes = SizeCalculator::new_with_cache(root.path().join("launcher-data/cache")).await;
        let plugins = PluginLoader::new_async(root.path().join("launcher-data/plugins"))
            .await
            .unwrap();
        let server = match start_server(
            api,
            managers,
            sizes,
            plugins,
            LoopbackHost::parse("127.0.0.1").unwrap(),
            0,
            crate::http_transport::HttpShutdownPolicy::default(),
        )
        .await
        {
            Ok(server) => server,
            Err(error) if is_socket_bind_permission_error(&error) => {
                eprintln!(
                    "Native shutdown integration unavailable: socket bind not permitted ({error})"
                );
                return;
            }
            Err(error) => panic!("Native shutdown server failed: {error:#}"),
        };
        server.shutdown().await.unwrap();
        server.shutdown().await.unwrap();
        let error = manager.install_version("b1234").await.unwrap_err();
        assert!(error.to_string().contains("shutting down"));
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
            assert!(is_allowed_origin(&header), "{origin}");
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
            assert!(!is_allowed_origin(&header), "{origin}");
        }
    }

    #[cfg(feature = "inference-plugins")]
    #[test]
    fn gateway_http_client_builds_with_configured_policy() {
        build_gateway_http_client().unwrap();
    }
    #[tokio::test]
    async fn shutdown_rejects_health_events_and_gateway_before_handlers() {
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use tower::ServiceExt;
        let shutdown = ShutdownRequest::default();
        let app = Router::new()
            .fallback(|| async { axum::http::StatusCode::IM_A_TEAPOT })
            .layer(axum::middleware::from_fn_with_state(
                shutdown.clone(),
                reject_during_shutdown,
            ));
        shutdown.request();
        for path in [
            "/health",
            "/events/model-library-updates",
            "/v1/completions",
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        }
    }
}

#[cfg(test)]
mod acquisition_integration;

#[cfg(test)]
mod rpc_http_resume;

#[cfg(test)]
mod http_discovery_tests {
    use super::*;
    use pumas_library::{
        discovery::{
            CompatibilityRequirements, HttpServiceDescription, LocalDiscovery, HTTP_DISCOVERY_PATH,
        },
        registry::LibraryRegistry,
    };

    async fn api_fixture() -> (
        tempfile::TempDir,
        LibraryRegistry,
        std::path::PathBuf,
        PumasApi,
    ) {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        let root = temp.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let api = PumasApi::builder(&root)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await
            .unwrap();
        api.start_ipc_server().await.unwrap();
        (temp, registry, root, api)
    }
    async fn start(
        api: PumasApi,
        root: &std::path::Path,
        port: u16,
    ) -> anyhow::Result<ServerHandle> {
        #[cfg(not(feature = "inference-plugins"))]
        let _ = root;
        start_server(
            api,
            #[cfg(feature = "inference-plugins")]
            HashMap::new(),
            #[cfg(feature = "inference-plugins")]
            SizeCalculator::new_with_cache(root.join("launcher-data/cache")).await,
            #[cfg(feature = "inference-plugins")]
            PluginLoader::new_async(root.join("launcher-data/plugins")).await?,
            LoopbackHost::parse("127.0.0.1")?,
            port,
            crate::http_transport::HttpShutdownPolicy::default(),
        )
        .await
    }
    #[tokio::test]
    async fn real_http_advertisement_borrowing_and_ordered_shutdown() {
        let (temp, registry, root, api) = api_fixture().await;
        let server = start(api, &root, 0).await.unwrap();
        let observer = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
        let description = registry.list_http_services().unwrap().remove(0);
        assert_eq!(
            description.endpoint.as_str(),
            format!("http://{}", server.addr())
        );
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let response = client
            .get(format!(
                "{}{HTTP_DISCOVERY_PATH}",
                description.endpoint.as_str()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response.json::<HttpServiceDescription>().await.unwrap(),
            description
        );
        let borrowed = observer
            .borrow_http_service(&root, &CompatibilityRequirements::default())
            .await
            .unwrap();
        assert_eq!(borrowed.description(), &description);
        drop(borrowed);
        assert!(client
            .get(format!("{}/health", description.endpoint.as_str()))
            .send()
            .await
            .unwrap()
            .status()
            .is_success());
        assert_eq!(registry.list_http_services().unwrap()[0], description);
        server.shutdown().await.unwrap();
        assert!(registry.list_http_services().unwrap().is_empty());
        assert!(registry.get_instance(&root).unwrap().is_none());
        assert!(tokio::net::TcpStream::connect(server.addr()).await.is_err());
    }
    #[tokio::test]
    async fn corrupt_foreign_http_row_preserves_real_descriptor_and_borrowing() {
        let (temp, registry, root, api) = api_fixture().await;
        let other_root = temp.path().join("other");
        std::fs::create_dir(&other_root).unwrap();
        let other = PumasApi::builder(&other_root)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await
            .unwrap();
        other.start_ipc_server().await.unwrap();
        let healthy_server = start(api, &root, 0).await.unwrap();
        let corrupt_server = start(other, &other_root, 0).await.unwrap();
        let descriptions = registry.list_http_services().unwrap();
        let healthy = descriptions
            .iter()
            .find(|d| d.instance.library_root == root)
            .unwrap();
        let corrupt = descriptions
            .iter()
            .find(|d| d.instance.library_root == other_root)
            .unwrap();
        let before = registry.get_instance(&other_root).unwrap().unwrap();
        let connection = rusqlite::Connection::open(temp.path().join("registry.db")).unwrap();
        let observer = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for bad_json in ["{".to_owned(), {
            let mut value = serde_json::to_value(corrupt).unwrap();
            value["instance"]["registry_library_id"] = serde_json::json!("wrong-library");
            value.to_string()
        }] {
            connection
                .execute(
                    "UPDATE http_services SET description_json=?1 WHERE library_path=?2",
                    rusqlite::params![bad_json, other_root.to_string_lossy()],
                )
                .unwrap();
            let response = client
                .get(format!(
                    "{}{HTTP_DISCOVERY_PATH}",
                    healthy.endpoint.as_str()
                ))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            assert_eq!(
                response.json::<HttpServiceDescription>().await.unwrap(),
                *healthy
            );
            let borrowed = observer
                .borrow_http_service(&root, &CompatibilityRequirements::default())
                .await
                .unwrap();
            assert_eq!(borrowed.description(), healthy);
            drop(borrowed);
            let refused = client
                .get(format!(
                    "{}{HTTP_DISCOVERY_PATH}",
                    corrupt.endpoint.as_str()
                ))
                .send()
                .await
                .unwrap();
            assert_eq!(refused.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
            assert!(observer
                .borrow_http_service(&other_root, &CompatibilityRequirements::default())
                .await
                .is_err());
            assert!(observer.snapshot().is_err());
            let retained = registry.get_instance(&other_root).unwrap().unwrap();
            assert_eq!(retained.connection_token, before.connection_token);
            assert_eq!(retained.started_at, before.started_at);
            let retained_json: String = connection
                .query_row(
                    "SELECT description_json FROM http_services WHERE library_path=?1",
                    [other_root.to_string_lossy()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(retained_json, bad_json);
        }
        healthy_server.shutdown().await.unwrap();
        corrupt_server.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn ordinary_server_drop_revokes_admission_but_retains_core_until_catalog_settles() {
        let (_temp, registry, root, api) = api_fixture().await;
        let server = start(api, &root, 0).await.unwrap();
        let (entered, release) = server
            .catalog_projection
            .as_ref()
            .unwrap()
            .hold_for_test()
            .unwrap();
        entered.await.unwrap();
        let receipt = server.completion.clone();
        let addr = server.addr();
        drop(server);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), receipt.clone())
                .await
                .is_err()
        );
        assert!(registry.list_http_services().unwrap().is_empty());
        assert!(registry.get_instance(&root).unwrap().is_some());
        assert!(matches!(
            registry
                .try_claim_instance(&root, std::process::id())
                .unwrap(),
            pumas_library::registry::InstanceClaimResult::Occupied(_)
        ));
        release.send(()).unwrap();
        receipt.await.unwrap();
        assert!(registry.get_instance(&root).unwrap().is_none());
        assert!(tokio::net::TcpStream::connect(addr).await.is_err());
    }
    #[tokio::test]
    async fn failed_http_owned_drain_revokes_advertisement_but_retains_core_authority() {
        let (_temp, registry, root, api) = api_fixture().await;
        let server = start(api, &root, 0).await.unwrap();
        let (entered, release) = server
            .catalog_projection
            .as_ref()
            .unwrap()
            .hold_for_test()
            .unwrap();
        entered.await.unwrap();
        // Deterministically panic the actual blocking catalog owner.
        drop(release);
        assert!(server.shutdown().await.is_err());
        assert!(registry.list_http_services().unwrap().is_empty());
        assert!(registry.get_instance(&root).unwrap().is_some());
        assert!(tokio::net::TcpStream::connect(server.addr()).await.is_err());
    }

    #[tokio::test]
    async fn failed_advertisement_revoke_settles_shutdown_and_retains_core_authority() {
        let (temp, registry, root, api) = api_fixture().await;
        let server = start(api, &root, 0).await.unwrap();
        let owner = registry.get_instance(&root).unwrap().unwrap();
        let description = registry.list_http_services().unwrap().remove(0);
        // Fail the real SQLite deletion, leaving publication and its settlement
        // sender retained. This exercises complete_shutdown's early error path.
        let connection = rusqlite::Connection::open(temp.path().join("registry.db")).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_http_revoke BEFORE DELETE ON http_services
                 BEGIN SELECT RAISE(ABORT, 'controlled HTTP revoke failure'); END;",
            )
            .unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), server.shutdown())
            .await
            .expect("failed HTTP revocation must settle instead of retaining its sender forever");
        let error = result.unwrap_err().to_string();
        assert!(error.contains("controlled HTTP revoke failure"), "{error}");
        let retained = registry.get_instance(&root).unwrap().unwrap();
        assert_eq!(retained.started_at, owner.started_at);
        assert_eq!(retained.connection_token, owner.connection_token);
        assert_eq!(registry.list_http_services().unwrap()[0], description);
        assert!(tokio::net::TcpStream::connect(server.addr()).await.is_err());
        assert!(matches!(
            registry
                .try_claim_instance(&root, std::process::id())
                .unwrap(),
            pumas_library::registry::InstanceClaimResult::Occupied(_)
        ));
        // Clearing the injected storage fault cannot turn an abandoned receipt
        // into cessation evidence, even through a repeated shared waiter.
        connection
            .execute_batch("DROP TRIGGER reject_http_revoke")
            .unwrap();
        assert!(server.shutdown().await.is_err());
        assert!(registry.get_instance(&root).unwrap().is_some());
    }

    #[tokio::test]
    async fn bind_failure_publishes_nothing_and_old_router_cannot_describe_successor() {
        let (temp, registry, root, api) = api_fixture().await;
        let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        assert!(start(api, &root, occupied.local_addr().unwrap().port())
            .await
            .is_err());
        assert!(registry.list_http_services().unwrap().is_empty());
        // Wait for ordinary API drop's ordered coordinator, without PID inference.
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while registry.get_instance(&root).unwrap().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let api = PumasApi::builder(&root)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await
            .unwrap();
        api.start_ipc_server().await.unwrap();
        let server = start(api, &root, 0).await.unwrap();
        let old_description = registry.list_http_services().unwrap().remove(0);
        // Inject a hostile rendezvous generation and advertisement. This tests
        // router fencing without admitting a second live physical-store owner.
        registry
            .register_instance(&root, std::process::id(), server.addr().port())
            .unwrap();
        let successor = registry.get_instance(&root).unwrap().unwrap();
        let mut description = old_description.clone();
        description.instance.generation = successor.started_at.clone();
        description.service_generation = "hostile-fixture-successor".into();
        let connection = rusqlite::Connection::open(temp.path().join("registry.db")).unwrap();
        connection
            .execute(
                "UPDATE http_services SET owner_started_at=?1, owner_token=?2,
             service_generation=?3, description_json=?4 WHERE library_path=?5",
                rusqlite::params![
                    successor.started_at,
                    successor.connection_token,
                    description.service_generation,
                    serde_json::to_string(&description).unwrap(),
                    root.to_string_lossy()
                ],
            )
            .unwrap();
        assert!(PumasApi::builder(&root)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .is_err());
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!(
                "{}{HTTP_DISCOVERY_PATH}",
                old_description.endpoint.as_str()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
        server.shutdown().await.unwrap();
        assert_eq!(registry.list_http_services().unwrap()[0], description);
        assert_eq!(
            registry
                .get_instance(&root)
                .unwrap()
                .unwrap()
                .connection_token,
            successor.connection_token
        );
    }
}
