//! `pumas-rpc --onnx-host`: the ONNX host process of one ONNX Runtime profile.
//!
//! ONNX Runtime keeps the first native library a process loads, so each
//! profile runs this host with the library of the installed version it pins.
//! The host initializes that library and the profile's execution provider
//! before it listens. If either fails it still listens and reports the reason
//! on `/health`, so the launching backend can refuse the launch with it.
//! The protocol is private to Pumas; see `pumas_library::onnx_runtime::host_protocol`.

use anyhow::{bail, Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use pumas_library::onnx_runtime::host_protocol::{
    self, OnnxHostHealth, OnnxHostLoadRequest, OnnxHostUnloadRequest,
};
use pumas_library::onnx_runtime_distribution::execution_provider_for;
use pumas_library::{
    OnnxEmbeddingBackend, OnnxEmbeddingRequest, OnnxExecutionProvider, OnnxLoadOptions,
    OnnxLoadRequest, OnnxModelId, OnnxRuntimeError, OnnxRuntimeErrorCode, OnnxRuntimeLibrary,
    OnnxSessionManager, RealOnnxEmbeddingBackend,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Concurrent loads and embeddings inside one host.
const HOST_MAX_CONCURRENT_OPERATIONS: usize = 4;
/// How often the host checks that the backend that launched it is alive.
const PARENT_WATCH_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct HostState<B> {
    manager: Option<OnnxSessionManager<B>>,
    health: OnnxHostHealth,
    token: String,
}

/// Run the host until it is stopped or its launching backend exits.
pub(crate) fn run(host: &str, port: u16, device_mode: Option<&str>) -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .init();
    let token = std::env::var(host_protocol::TOKEN_ENV)
        .ok()
        .filter(|token| !token.is_empty())
        .with_context(|| format!("{} is required", host_protocol::TOKEN_ENV))?;
    let version = std::env::var(host_protocol::RUNTIME_VERSION_ENV)
        .with_context(|| format!("{} is required", host_protocol::RUNTIME_VERSION_ENV))?;
    let version_dir = PathBuf::from(
        std::env::var_os(host_protocol::RUNTIME_DIR_ENV)
            .with_context(|| format!("{} is required", host_protocol::RUNTIME_DIR_ENV))?,
    );
    // Nothing the host loads needs these.
    std::env::remove_var(host_protocol::TOKEN_ENV);
    let device_mode = device_mode.unwrap_or("auto");
    let Some(device_mode) = pumas_library::runtime_profiles::parse_onnx_device_mode(device_mode)
    else {
        bail!(
            "unknown {} value {device_mode:?}",
            host_protocol::DEVICE_MODE_FLAG
        );
    };
    let loopback = crate::server::LoopbackHost::parse(host)?;
    watch_parent();

    let (manager, health) = initialize(&version, version_dir, device_mode);
    match &health.error {
        Some(reason) => tracing::error!(%version, %reason, "ONNX host is not ready"),
        None => tracing::info!(
            %version,
            execution_provider = ?health.execution_provider,
            "ONNX host is ready"
        ),
    }
    let state = Arc::new(HostState {
        manager,
        health,
        token,
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(loopback.socket_addr(port)).await?;
        axum::serve(listener, router(state)).await?;
        Ok(())
    })
}

/// Load the installed version's library and initialize the execution
/// provider the profile's device mode selects for that build.
fn initialize(
    version: &str,
    version_dir: PathBuf,
    device_mode: pumas_library::models::RuntimeDeviceMode,
) -> (
    Option<OnnxSessionManager<RealOnnxEmbeddingBackend>>,
    OnnxHostHealth,
) {
    let failed = |error: String| {
        (
            None,
            OnnxHostHealth {
                ready: false,
                runtime_version: version.to_string(),
                execution_provider: None,
                error: Some(error),
            },
        )
    };
    let provider: OnnxExecutionProvider = match execution_provider_for(version, device_mode) {
        Ok(provider) => provider.into(),
        Err(reason) => return failed(reason),
    };
    let library = OnnxRuntimeLibrary::installed_version(version, &version_dir);
    if let Err(error) = pumas_library::onnx_runtime::probe_execution_provider(&library, provider) {
        return failed(error.message);
    }
    let backend = RealOnnxEmbeddingBackend::for_runtime(library, provider);
    match OnnxSessionManager::new(backend, HOST_MAX_CONCURRENT_OPERATIONS) {
        Ok(manager) => (
            Some(manager),
            OnnxHostHealth {
                ready: true,
                runtime_version: version.to_string(),
                execution_provider: Some(provider),
                error: None,
            },
        ),
        Err(error) => failed(error.message),
    }
}

/// Exit when the backend that launched this host is gone, so a host never
/// outlives the Pumas process that owns its profile.
fn watch_parent() {
    let Some(parent) = std::env::var(host_protocol::PARENT_PID_ENV)
        .ok()
        .and_then(|pid| pid.parse::<u32>().ok())
    else {
        return;
    };
    std::thread::spawn(move || loop {
        if !pumas_library::platform::is_process_alive(parent) {
            tracing::info!(parent, "launching backend exited; stopping ONNX host");
            std::process::exit(0);
        }
        std::thread::sleep(PARENT_WATCH_INTERVAL);
    });
}

pub(crate) fn router<B>(state: Arc<HostState<B>>) -> Router
where
    B: OnnxEmbeddingBackend + 'static,
{
    Router::new()
        .route(host_protocol::HEALTH_PATH, get(health::<B>))
        .route(host_protocol::LOAD_PATH, post(load::<B>))
        .route(host_protocol::UNLOAD_PATH, post(unload::<B>))
        .route(host_protocol::SESSIONS_PATH, get(sessions::<B>))
        .route(host_protocol::EMBED_PATH, post(embed::<B>))
        .with_state(state)
}

async fn health<B>(State(state): State<Arc<HostState<B>>>) -> Json<OnnxHostHealth> {
    Json(state.health.clone())
}

fn error_response(error: OnnxRuntimeError) -> Response {
    let status = match error.code {
        OnnxRuntimeErrorCode::Validation => StatusCode::BAD_REQUEST,
        OnnxRuntimeErrorCode::NotLoaded => StatusCode::NOT_FOUND,
        OnnxRuntimeErrorCode::Backend => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(error)).into_response()
}

/// The token check and the ready manager every operation needs.
fn admitted<'a, B>(
    state: &'a HostState<B>,
    headers: &HeaderMap,
) -> Result<&'a OnnxSessionManager<B>, Box<Response>> {
    let presented = headers
        .get(host_protocol::TOKEN_HEADER)
        .and_then(|value| value.to_str().ok());
    if presented != Some(state.token.as_str()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(OnnxRuntimeError::validation(
                "token",
                "ONNX host requests must carry this host's token",
            )),
        )
            .into_response()
            .into());
    }
    state.manager.as_ref().ok_or_else(|| -> Box<Response> {
        error_response(OnnxRuntimeError::backend(
            state
                .health
                .error
                .clone()
                .unwrap_or_else(|| "ONNX host is not ready".to_string()),
        ))
        .into()
    })
}

async fn load<B: OnnxEmbeddingBackend>(
    State(state): State<Arc<HostState<B>>>,
    headers: HeaderMap,
    Json(body): Json<OnnxHostLoadRequest>,
) -> Response {
    let manager = match admitted(&state, &headers) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    // Validate the path against its root here too: the host reads the file.
    let options = OnnxLoadOptions {
        embedding_dimensions: body.embedding_dimensions,
        ..OnnxLoadOptions::default()
    };
    let request =
        match OnnxLoadRequest::parse(&body.model_root, &body.model_path, &body.model_id, options) {
            Ok(request) => request,
            Err(error) => return error_response(error),
        };
    match manager.load(request).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

async fn unload<B: OnnxEmbeddingBackend>(
    State(state): State<Arc<HostState<B>>>,
    headers: HeaderMap,
    Json(body): Json<OnnxHostUnloadRequest>,
) -> Response {
    let manager = match admitted(&state, &headers) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    let model_id = match OnnxModelId::parse(&body.model_id) {
        Ok(model_id) => model_id,
        Err(error) => return error_response(error),
    };
    match manager.unload(&model_id).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

async fn sessions<B: OnnxEmbeddingBackend>(
    State(state): State<Arc<HostState<B>>>,
    headers: HeaderMap,
) -> Response {
    let manager = match admitted(&state, &headers) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    match manager.list().await {
        Ok(sessions) => Json(sessions).into_response(),
        Err(error) => error_response(error),
    }
}

async fn embed<B: OnnxEmbeddingBackend>(
    State(state): State<Arc<HostState<B>>>,
    headers: HeaderMap,
    Json(body): Json<OnnxEmbeddingRequest>,
) -> Response {
    let manager = match admitted(&state, &headers) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    // Re-apply the request limits; the body was decoded without them.
    let request =
        match OnnxEmbeddingRequest::parse(body.model_id.as_str(), body.input, body.dimensions) {
            Ok(request) => request,
            Err(error) => return error_response(error),
        };
    match manager.embed(request).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => error_response(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::{
        FakeOnnxEmbeddingBackend, OnnxHostEndpoint, OnnxSessionManager, OnnxSessionStatus,
        RemoteOnnxEmbeddingBackend,
    };

    async fn serve_fake_host(ready: bool) -> (OnnxHostEndpoint, tokio::task::JoinHandle<()>) {
        let state = Arc::new(HostState {
            manager: ready
                .then(|| OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 2).unwrap()),
            health: OnnxHostHealth {
                ready,
                runtime_version: "v1.31.1+cuda12".into(),
                execution_provider: ready.then_some(OnnxExecutionProvider::Cuda),
                error: (!ready).then(|| "CUDA execution provider could not initialize".into()),
            },
            token: host_protocol::host_api_token().to_string(),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint =
            OnnxHostEndpoint::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router(state)).await.unwrap();
        });
        (endpoint, task)
    }

    fn model_fixture() -> (tempfile::TempDir, OnnxLoadRequest) {
        let fixture = tempfile::tempdir().unwrap();
        std::fs::write(fixture.path().join("model.onnx"), b"fixture").unwrap();
        let request = OnnxLoadRequest::parse(
            fixture.path(),
            "model.onnx",
            "fixture/embedder",
            OnnxLoadOptions::default(),
        )
        .unwrap();
        (fixture, request)
    }

    #[tokio::test]
    async fn pumas_loads_embeds_and_unloads_through_a_host_over_its_protocol() {
        let (endpoint, server) = serve_fake_host(true).await;
        let manager =
            OnnxSessionManager::new(RemoteOnnxEmbeddingBackend::new().unwrap(), 2).unwrap();
        let (_fixture, request) = model_fixture();

        let status = manager
            .load_on_host(request, endpoint.clone())
            .await
            .unwrap();
        assert_eq!(status.model_id.as_str(), "fixture/embedder");
        assert_eq!(manager.list().await.unwrap().len(), 1);

        let response = manager
            .embed(
                OnnxEmbeddingRequest::parse("fixture/embedder", vec!["hello".into()], None)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.data.len(), 1);

        let unloaded = manager
            .unload(&OnnxModelId::parse("fixture/embedder").unwrap())
            .await
            .unwrap();
        assert!(unloaded.is_some());
        assert!(manager.list().await.unwrap().is_empty());
        let not_loaded = manager
            .embed(
                OnnxEmbeddingRequest::parse("fixture/embedder", vec!["hello".into()], None)
                    .unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(not_loaded.code, OnnxRuntimeErrorCode::NotLoaded);
        server.abort();
    }

    #[tokio::test]
    async fn a_model_moves_to_another_host_only_once_its_first_host_dropped_it() {
        let (first, first_server) = serve_fake_host(true).await;
        let (second, second_server) = serve_fake_host(true).await;
        let manager =
            OnnxSessionManager::new(RemoteOnnxEmbeddingBackend::new().unwrap(), 2).unwrap();
        let (_fixture, request) = model_fixture();
        manager
            .load_on_host(request.clone(), first.clone())
            .await
            .unwrap();

        // The first host still holds the model: loading it elsewhere is refused.
        let refused = manager
            .load_on_host(request.clone(), second.clone())
            .await
            .unwrap_err();
        assert!(
            refused.message.contains(first.as_str()),
            "{}",
            refused.message
        );

        // The first host lost the session (as when its profile stops): the
        // stale record no longer blocks the second host.
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let dropped = client
            .post(format!("{}{}", first.as_str(), host_protocol::UNLOAD_PATH))
            .header(host_protocol::TOKEN_HEADER, host_protocol::host_api_token())
            .json(&OnnxHostUnloadRequest {
                model_id: "fixture/embedder".into(),
            })
            .send()
            .await
            .unwrap();
        assert!(dropped.status().is_success());
        let moved = manager.load_on_host(request, second.clone()).await.unwrap();
        assert_eq!(moved.model_id.as_str(), "fixture/embedder");
        let held: Vec<OnnxSessionStatus> = client
            .get(format!(
                "{}{}",
                second.as_str(),
                host_protocol::SESSIONS_PATH
            ))
            .header(host_protocol::TOKEN_HEADER, host_protocol::host_api_token())
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(held.len(), 1);
        first_server.abort();
        second_server.abort();
    }

    #[tokio::test]
    async fn concurrent_loads_of_one_model_on_two_hosts_leave_one_holder() {
        let (first, first_server) = serve_fake_host(true).await;
        let (second, second_server) = serve_fake_host(true).await;
        let manager =
            OnnxSessionManager::new(RemoteOnnxEmbeddingBackend::new().unwrap(), 2).unwrap();
        let (_fixture, request) = model_fixture();
        let (left, right) = tokio::join!(
            manager.load_on_host(request.clone(), first.clone()),
            manager.load_on_host(request, second.clone()),
        );
        assert_eq!(
            usize::from(left.is_ok()) + usize::from(right.is_ok()),
            1,
            "{left:?} {right:?}"
        );
        let refused = left.err().or(right.err()).unwrap();
        assert!(refused.message.contains("already loaded by the ONNX host"));

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut holders = 0;
        for host in [&first, &second] {
            let held: Vec<OnnxSessionStatus> = client
                .get(format!("{}{}", host.as_str(), host_protocol::SESSIONS_PATH))
                .header(host_protocol::TOKEN_HEADER, host_protocol::host_api_token())
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            holders += held.len();
        }
        assert_eq!(holders, 1);
        first_server.abort();
        second_server.abort();
    }

    #[tokio::test]
    async fn a_host_that_failed_to_initialize_refuses_loads_with_its_reason() {
        let (endpoint, server) = serve_fake_host(false).await;
        let health: OnnxHostHealth = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!(
                "{}{}",
                endpoint.as_str(),
                host_protocol::HEALTH_PATH
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(!health.ready);

        let manager =
            OnnxSessionManager::new(RemoteOnnxEmbeddingBackend::new().unwrap(), 2).unwrap();
        let (_fixture, request) = model_fixture();
        let error = manager.load_on_host(request, endpoint).await.unwrap_err();
        assert!(
            error
                .message
                .contains("CUDA execution provider could not initialize"),
            "{}",
            error.message
        );
        server.abort();
    }

    #[tokio::test]
    async fn host_requests_without_the_token_are_refused() {
        let (endpoint, server) = serve_fake_host(true).await;
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(format!(
                "{}{}",
                endpoint.as_str(),
                host_protocol::UNLOAD_PATH
            ))
            .json(&OnnxHostUnloadRequest {
                model_id: "fixture/embedder".into(),
            })
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        server.abort();
    }
}
