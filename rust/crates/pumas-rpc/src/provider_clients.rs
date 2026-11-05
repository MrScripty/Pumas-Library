//! Reusable provider HTTP clients owned by RPC composition state.

use std::time::Duration;

const LLAMA_CPP_ROUTER_UNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// Upper bound for a provider model catalog (`/models`, `/v1/models`).
const PROVIDER_CATALOG_BYTES: usize = 4 * 1024 * 1024;
/// Upper bound for a provider status document (`/health`, `/props`).
const PROVIDER_STATUS_BYTES: usize = 1024 * 1024;
/// Upper bound for a provider's load/unload reply, which may carry its reason.
const PROVIDER_REPLY_BYTES: usize = 64 * 1024;
/// Characters of a provider's own failure text kept in a public reason.
const PROVIDER_REASON_CHARS: usize = 512;

/// HTTP clients for provider endpoints.
///
/// A loopback provider is reached directly: `HTTP_PROXY`, `ALL_PROXY` and
/// system proxy settings never see prompts, images or embeddings destined for
/// this machine. A remote provider (an External runtime profile on another
/// host) keeps the system proxy configuration. Neither client follows
/// redirects, so a provider cannot send a request to another origin.
#[derive(Clone)]
pub struct ProviderHttp {
    loopback: reqwest::Client,
    remote: reqwest::Client,
}

impl ProviderHttp {
    /// Build both clients with the same timeouts and headers from `configure`.
    pub fn build(
        configure: impl Fn(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
    ) -> reqwest::Result<Self> {
        let base =
            || configure(reqwest::Client::builder()).redirect(reqwest::redirect::Policy::none());
        Ok(Self {
            loopback: base().no_proxy().build()?,
            remote: base().build()?,
        })
    }

    /// The client for `url`. An unparsable URL gets the direct client; the
    /// request itself then fails with the URL error.
    pub fn for_url(&self, url: &str) -> &reqwest::Client {
        match reqwest::Url::parse(url) {
            Ok(url) if !is_loopback_url(&url) => &self.remote,
            _ => &self.loopback,
        }
    }
}

/// One caller-supplied client for every destination (tests and fixtures).
impl From<reqwest::Client> for ProviderHttp {
    fn from(client: reqwest::Client) -> Self {
        Self {
            loopback: client.clone(),
            remote: client,
        }
    }
}

pub(crate) fn is_loopback_url(url: &reqwest::Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        Some(url::Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost" || name.ends_with(".localhost")
        }
        None => false,
    }
}

/// Why a bounded provider body could not be read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BodyReadError {
    TooLarge { limit: usize },
    Interrupted,
}

impl std::fmt::Display for BodyReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { limit } => write!(f, "the reply exceeds {limit} bytes"),
            Self::Interrupted => f.write_str("the reply ended before it was complete"),
        }
    }
}

/// Read at most `limit` bytes of a provider reply, refusing a larger one
/// (declared or streamed) instead of buffering it.
pub(crate) async fn read_bounded(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, BodyReadError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(BodyReadError::TooLarge { limit });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| BodyReadError::Interrupted)?
    {
        if bytes
            .len()
            .checked_add(chunk.len())
            .is_none_or(|length| length > limit)
        {
            return Err(BodyReadError::TooLarge { limit });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn read_bounded_json(
    response: reqwest::Response,
    limit: usize,
    what: &str,
) -> Result<serde_json::Value, String> {
    let bytes = read_bounded(response, limit)
        .await
        .map_err(|error| format!("{what} could not be read: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|_| format!("{what} returned invalid JSON"))
}

#[derive(Clone)]
pub struct OllamaClientFactory {
    http_clients: pumas_app_manager::OllamaHttpClients,
}

impl OllamaClientFactory {
    pub fn new(http_clients: pumas_app_manager::OllamaHttpClients) -> Self {
        Self { http_clients }
    }

    pub fn client(&self, endpoint: Option<&str>) -> pumas_app_manager::OllamaClient {
        pumas_app_manager::OllamaClient::with_http_clients(endpoint, self.http_clients.clone())
    }
}

#[derive(Clone)]
pub struct LlamaCppRouterClient {
    http: ProviderHttp,
}

impl LlamaCppRouterClient {
    pub fn new(http: impl Into<ProviderHttp>) -> Self {
        Self { http: http.into() }
    }

    pub(crate) async fn router_catalog_ready(
        &self,
        endpoint: &str,
        model_id: &str,
        require_loaded: bool,
        deadline: tokio::time::Instant,
    ) -> Result<bool, String> {
        let url = llama_cpp_router_models_url(endpoint);
        let response = self
            .http
            .for_url(&url)
            .get(&url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) if error.is_connect() || error.is_timeout() => return Ok(false),
            Err(_) => return Err("llama.cpp router readiness request failed".into()),
        };
        if response.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            return Ok(false);
        }
        if response.status() != reqwest::StatusCode::OK {
            return Err(format!(
                "llama.cpp router model endpoint returned unexpected HTTP status {}",
                response.status()
            ));
        }
        let payload = read_bounded_json(
            response,
            PROVIDER_CATALOG_BYTES,
            "llama.cpp router model endpoint",
        )
        .await?;
        router_catalog_model_ready(&payload, model_id, require_loaded)
    }

    pub(crate) async fn router_context_catalog(
        &self,
        endpoint: &str,
        model_id: &str,
        reload: bool,
        deadline: tokio::time::Instant,
    ) -> Result<RouterContextCatalog, String> {
        let mut url = reqwest::Url::parse(&llama_cpp_router_models_url(endpoint))
            .map_err(|_| "llama.cpp router endpoint is invalid")?;
        if reload {
            url.query_pairs_mut().append_pair("reload", "1");
        }
        let response = self
            .http
            .for_url(url.as_str())
            .get(url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await
            .map_err(|error| request_failure("llama.cpp router context catalog", &error))?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(format!(
                "llama.cpp router context catalog returned unexpected HTTP status {}",
                response.status()
            ));
        }
        let payload = read_bounded_json(
            response,
            PROVIDER_CATALOG_BYTES,
            "llama.cpp router context catalog",
        )
        .await?;
        parse_router_context_catalog(&payload, model_id)
    }

    pub(crate) async fn verify_router_runtime_context(
        &self,
        endpoint: &str,
        model_id: &str,
        requested: u32,
        deadline: tokio::time::Instant,
    ) -> Result<(), String> {
        let mut url = reqwest::Url::parse(&format!("{}/props", endpoint.trim_end_matches('/')))
            .map_err(|_| "llama.cpp router endpoint is invalid")?;
        url.query_pairs_mut()
            .append_pair("model", model_id)
            .append_pair("autoload", "false");
        let response = self
            .http
            .for_url(url.as_str())
            .get(url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await
            .map_err(|error| request_failure("llama.cpp router runtime context", &error))?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(format!(
                "llama.cpp router runtime context returned unexpected HTTP status {}",
                response.status()
            ));
        }
        let payload = read_bounded_json(
            response,
            PROVIDER_STATUS_BYTES,
            "llama.cpp router runtime context",
        )
        .await?;
        let actual = payload
            .get("default_generation_settings")
            .and_then(|settings| settings.get("n_ctx"))
            .and_then(serde_json::Value::as_u64)
            .ok_or("llama.cpp router runtime context is missing or invalid")?;
        if actual < u64::from(requested) {
            return Err(format!(
                "llama.cpp router runtime context is {actual} tokens, smaller than the requested {requested}"
            ));
        }
        Ok(())
    }

    /// Prove readiness of one dedicated server against its owned launch model.
    /// `identities` are the IDs that launch can report: the model file path,
    /// or the library model ID when the launch set `--alias` (models with a
    /// projector). The caller owns the startup deadline and process checks.
    pub(crate) async fn dedicated_model_ready(
        &self,
        endpoint: &str,
        identities: &[&str],
        deadline: tokio::time::Instant,
    ) -> Result<bool, String> {
        let health_url = format!("{}/health", endpoint.trim_end_matches('/'));
        let health = self
            .http
            .for_url(&health_url)
            .get(&health_url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await;
        let health = match health {
            Ok(response) => response,
            Err(error) if error.is_connect() || error.is_timeout() => return Ok(false),
            Err(_) => return Err("llama.cpp readiness request failed".into()),
        };
        if health.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            return Ok(false);
        }
        if health.status() != reqwest::StatusCode::OK {
            return Err(format!(
                "llama.cpp health endpoint returned unexpected HTTP status {}",
                health.status()
            ));
        }
        let health =
            read_bounded_json(health, PROVIDER_STATUS_BYTES, "llama.cpp health endpoint").await?;
        if health.get("status").and_then(serde_json::Value::as_str) != Some("ok") {
            return Err("llama.cpp health endpoint did not report ready".into());
        }
        let models_url = llama_cpp_router_models_url(endpoint);
        let models = self
            .http
            .for_url(&models_url)
            .get(&models_url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await
            .map_err(|error| request_failure("llama.cpp model identity", &error))?;
        if models.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            return Ok(false);
        }
        if models.status() != reqwest::StatusCode::OK {
            return Err(format!(
                "llama.cpp model endpoint returned unexpected HTTP status {}",
                models.status()
            ));
        }
        let models =
            read_bounded_json(models, PROVIDER_CATALOG_BYTES, "llama.cpp model endpoint").await?;
        dedicated_model_identity_ready(&models, identities)
    }

    pub async fn load_model(
        &self,
        endpoint: &str,
        model_alias: &str,
        deadline: tokio::time::Instant,
    ) -> Result<(), String> {
        let url = llama_cpp_router_model_load_url(endpoint);
        let response = self
            .http
            .for_url(&url)
            .post(&url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .json(&serde_json::json!({ "model": model_alias }))
            .send()
            .await
            .map_err(|err| format!("failed to load model through llama.cpp router: {err}"))?;
        router_reply("load", "model is already running", response).await
    }

    pub async fn unload_model(&self, endpoint: &str, model_alias: &str) -> Result<(), String> {
        let url = llama_cpp_router_model_unload_url(endpoint);
        let response = self
            .http
            .for_url(&url)
            .post(&url)
            .timeout(LLAMA_CPP_ROUTER_UNLOAD_TIMEOUT)
            .json(&serde_json::json!({ "model": model_alias }))
            .send()
            .await
            .map_err(|err| format!("failed to unload model through llama.cpp router: {err}"))?;
        router_reply("unload", "model is not running", response).await
    }
}

/// Distinguish an unreachable provider from a slow one in a public reason.
fn request_failure(what: &str, error: &reqwest::Error) -> String {
    if error.is_timeout() {
        format!("{what} request timed out")
    } else if error.is_connect() {
        format!("{what} request could not connect to the provider")
    } else {
        format!("{what} request failed")
    }
}

/// A load/unload reply: success, the provider's idempotent "already" answer,
/// or a failure carrying the provider's own (bounded, redacted) reason.
async fn router_reply(
    action: &str,
    already: &str,
    response: reqwest::Response,
) -> Result<(), String> {
    let status = response.status();
    let body = match read_bounded(response, PROVIDER_REPLY_BYTES).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) if status.is_success() => {
            return Err(format!(
                "llama.cpp router {action} reply could not be read: {error}"
            ))
        }
        Err(_) => String::new(),
    };
    if status.is_success() || body.contains(already) {
        return Ok(());
    }
    let reason = crate::public_text::public_reason_within(body.trim(), PROVIDER_REASON_CHARS);
    Err(if reason.is_empty() {
        format!("llama.cpp router failed to {action} model with HTTP status {status}")
    } else {
        format!("llama.cpp router failed to {action} model with HTTP status {status}: {reason}")
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RouterContextCatalog {
    pub context_size: Option<u32>,
    pub selected_loaded: bool,
    pub selected_unloaded: bool,
    pub all_unloaded: bool,
}

fn parse_router_context_catalog(
    payload: &serde_json::Value,
    model_id: &str,
) -> Result<RouterContextCatalog, String> {
    let rows = payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or("llama.cpp router context catalog is missing")?;
    let mut ids = std::collections::HashSet::new();
    let mut selected = None;
    let mut all_unloaded = true;
    for row in rows {
        let id = row
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or("llama.cpp router catalog identity is invalid")?;
        if !ids.insert(id) {
            return Err("llama.cpp router catalog identity is duplicated".into());
        }
        let status = row
            .get("status")
            .ok_or("llama.cpp router catalog status is missing")?;
        let value = router_catalog_status(status, false)?;
        all_unloaded &= value == "unloaded";
        if id == model_id {
            selected = Some((router_context_argument(status.get("args"))?, value));
        }
    }
    let (context_size, status) =
        selected.ok_or("llama.cpp router context catalog omits selected model")?;
    Ok(RouterContextCatalog {
        context_size,
        selected_loaded: status == "loaded",
        selected_unloaded: status == "unloaded",
        all_unloaded,
    })
}

fn router_context_argument(args: Option<&serde_json::Value>) -> Result<Option<u32>, String> {
    let args = args
        .and_then(serde_json::Value::as_array)
        .ok_or("llama.cpp router context arguments are missing or invalid")?;
    let args = args
        .iter()
        .map(|arg| {
            arg.as_str()
                .ok_or("llama.cpp router context argument is invalid")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if args.first().is_none_or(|binary| binary.is_empty()) {
        return Err("llama.cpp router context arguments omit the executable".into());
    }
    let mut context = None;
    let mut index = 0;
    while index < args.len() {
        if matches!(args[index], "-c" | "--ctx-size" | "-ctx") {
            index += 1;
            let value = args
                .get(index)
                .ok_or("llama.cpp router context argument has no value")?;
            let value = value
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or("llama.cpp router context argument value is invalid")?;
            if context.replace(value).is_some() {
                return Err("llama.cpp router context argument is duplicated".into());
            }
        }
        index += 1;
    }
    Ok(context)
}

fn router_catalog_model_ready(
    payload: &serde_json::Value,
    model_id: &str,
    require_loaded: bool,
) -> Result<bool, String> {
    let rows = payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or("llama.cpp router model endpoint omitted its catalog")?;
    let mut selected = rows
        .iter()
        .filter(|row| row.get("id").and_then(serde_json::Value::as_str) == Some(model_id));
    let model = selected
        .next()
        .ok_or("llama.cpp router catalog does not contain the selected model")?;
    if selected.next().is_some() {
        return Err("llama.cpp router catalog contains an ambiguous model identity".into());
    }
    let status = model
        .get("status")
        .ok_or("llama.cpp router catalog omitted model status")?;
    match router_catalog_status(status, require_loaded)? {
        "loaded" => Ok(true),
        _ => Ok(!require_loaded),
    }
}

fn router_catalog_status(status: &serde_json::Value, require_loaded: bool) -> Result<&str, String> {
    match status.get("failed") {
        Some(serde_json::Value::Bool(true))
            if require_loaded
                || status.get("value").and_then(serde_json::Value::as_str) != Some("unloaded") =>
        {
            return Err("llama.cpp router reports that the selected model failed to load".into())
        }
        // An unloaded router entry may carry failure history before this
        // explicit load attempt. Only post-load observation proves readiness.
        Some(serde_json::Value::Bool(_)) | None => {}
        _ => return Err("llama.cpp router catalog returned invalid model failure status".into()),
    }
    match status.get("value").and_then(serde_json::Value::as_str) {
        Some(value @ ("loaded" | "unloaded" | "loading" | "sleeping")) => Ok(value),
        _ => Err("llama.cpp router catalog returned invalid model status".into()),
    }
}

fn dedicated_model_identity_ready(
    models: &serde_json::Value,
    identities: &[&str],
) -> Result<bool, String> {
    let rows = models
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or("llama.cpp model endpoint omitted model identity")?;
    let reported = rows
        .first()
        .and_then(|row| row.get("id"))
        .and_then(serde_json::Value::as_str);
    if rows.len() != 1 || !reported.is_some_and(|id| identities.contains(&id)) {
        return Err("llama.cpp model endpoint does not match the owned launch model".into());
    }
    match rows[0].get("meta") {
        Some(serde_json::Value::Object(_)) => Ok(true),
        Some(serde_json::Value::Null) => Ok(false),
        _ => Err("llama.cpp model endpoint omitted valid model metadata".into()),
    }
}

fn llama_cpp_router_models_url(endpoint: &str) -> String {
    format!("{}/v1/models", endpoint.trim_end_matches('/'))
}

fn llama_cpp_router_model_load_url(endpoint: &str) -> String {
    format!("{}/models/load", endpoint.trim_end_matches('/'))
}

fn llama_cpp_router_model_unload_url(endpoint: &str) -> String {
    format!("{}/models/unload", endpoint.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serve `app` on a loopback port until the returned sender fires.
    async fn serve(app: axum::Router) -> (String, tokio::sync::oneshot::Sender<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        (endpoint, stop)
    }

    #[test]
    fn loopback_destinations_are_classified() {
        for url in [
            "http://127.0.0.1:8080/v1",
            "http://127.9.8.7/",
            "http://[::1]:11434",
            "http://localhost:1234",
            "http://LOCALHOST.:1234",
            "http://runtime.localhost/",
        ] {
            assert!(is_loopback_url(&reqwest::Url::parse(url).unwrap()), "{url}");
        }
        for url in [
            "http://10.0.0.5:8080",
            "https://provider.example/v1",
            "http://[::ffff:7f00:1]/",
            "http://localhost.example/",
        ] {
            assert!(
                !is_loopback_url(&reqwest::Url::parse(url).unwrap()),
                "{url}"
            );
        }
    }

    const PROXY_ENV_CHILD: &str = "PUMAS_RPC_PROXY_ENV_CHILD";

    /// Proxy variables are process-global, so the check runs in a child test
    /// process and the parent's other tests keep their environment.
    #[test]
    fn loopback_provider_requests_bypass_proxy_environment() {
        if std::env::var_os(PROXY_ENV_CHILD).is_some() {
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "provider_clients::tests::proxy_environment_child",
                "--test-threads=1",
            ])
            .env(PROXY_ENV_CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn proxy_environment_child() {
        if std::env::var_os(PROXY_ENV_CHILD).is_none() {
            return;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            // A fake forward proxy that answers every request itself.
            let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
            let proxied = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let counter = proxied.clone();
            tokio::spawn(async move {
                loop {
                    let (mut socket, _) = proxy.accept().await.unwrap();
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let mut buffer = [0u8; 4096];
                    let _ = socket.read(&mut buffer).await;
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nproxy")
                        .await;
                }
            });
            for name in ["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"] {
                std::env::set_var(name, &proxy_url);
            }
            for name in ["NO_PROXY", "no_proxy", "REQUEST_METHOD"] {
                std::env::remove_var(name);
            }
            let app = axum::Router::new().route("/", axum::routing::get(|| async { "direct" }));
            let (target, stop) = serve(app).await;
            let http = ProviderHttp::build(|builder| builder).unwrap();
            let direct = http.for_url(&target).get(&target).send().await.unwrap();
            assert_eq!(direct.text().await.unwrap(), "direct");
            assert_eq!(proxied.load(std::sync::atomic::Ordering::SeqCst), 0);
            // The same environment still proxies a remote provider.
            let remote = "http://198.51.100.7/";
            let reply = http.for_url(remote).get(remote).send().await.unwrap();
            assert_eq!(reply.text().await.unwrap(), "proxy");
            assert_eq!(proxied.load(std::sync::atomic::Ordering::SeqCst), 1);
            let _ = stop.send(());
        });
    }

    #[tokio::test]
    async fn provider_clients_do_not_follow_redirects_and_bound_replies() {
        let app = axum::Router::new()
            .route(
                "/moved",
                axum::routing::get(|| async {
                    axum::response::Redirect::temporary("http://198.51.100.7/elsewhere")
                }),
            )
            .route(
                "/v1/models",
                axum::routing::get(|| async { "x".repeat(PROVIDER_CATALOG_BYTES + 1) }),
            )
            .route(
                "/models/load",
                axum::routing::post(|| async {
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "load failed: token: abc123\nsee logs",
                    )
                }),
            );
        let (endpoint, stop) = serve(app).await;
        let http = ProviderHttp::build(|builder| builder).unwrap();
        let moved = format!("{endpoint}/moved");
        let response = http.for_url(&moved).get(&moved).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::TEMPORARY_REDIRECT);

        let models = format!("{endpoint}/v1/models");
        let response = http.for_url(&models).get(&models).send().await.unwrap();
        assert_eq!(
            read_bounded(response, PROVIDER_CATALOG_BYTES).await,
            Err(BodyReadError::TooLarge {
                limit: PROVIDER_CATALOG_BYTES
            })
        );

        let client = LlamaCppRouterClient::new(http);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let error = client
            .load_model(&endpoint, "model", deadline)
            .await
            .unwrap_err();
        assert!(error.contains("HTTP status 500"), "{error}");
        assert!(error.contains("token: [redacted]"), "{error}");
        assert!(
            !error.contains("abc123") && !error.contains('\n'),
            "{error}"
        );
        let error = client
            .dedicated_model_ready(&endpoint, &["model"], deadline)
            .await
            .unwrap_err();
        assert!(error.contains("unexpected HTTP status 404"), "{error}");
        let _ = stop.send(());
    }

    #[test]
    fn router_context_catalog_requires_complete_unambiguous_status_and_context() {
        let row = serde_json::json!({"id":"selected", "status":{
            "value":"unloaded", "failed":true, "args":["llama-server", "-c", "18000"]
        }});
        let catalog =
            parse_router_context_catalog(&serde_json::json!({"data":[row.clone()]}), "selected")
                .unwrap();
        assert_eq!(catalog.context_size, Some(18000));
        assert!(catalog.all_unloaded);
        for other in [
            serde_json::json!({"id":"other", "status":{"value":"loaded"}}),
            serde_json::json!({"id":"other", "status":{"value":"loading"}}),
            serde_json::json!({"id":"other", "status":{"value":"sleeping"}}),
        ] {
            assert!(
                !parse_router_context_catalog(
                    &serde_json::json!({"data":[row.clone(),other]}),
                    "selected"
                )
                .unwrap()
                .all_unloaded
            );
        }
        for other in [
            serde_json::json!({"id":"other"}),
            serde_json::json!({"id":42, "status":{"value":"unloaded"}}),
            serde_json::json!({"id":"other", "status":{"value":"unknown"}}),
            serde_json::json!({"id":"other", "status":{"value":"unloaded","failed":"false"}}),
            row.clone(),
        ] {
            assert!(parse_router_context_catalog(
                &serde_json::json!({"data":[row.clone(),other]}),
                "selected"
            )
            .is_err());
        }
        for args in [
            serde_json::json!([]),
            serde_json::json!([""]),
            serde_json::json!(["--ctx-size", "18000", "-c", "4096"]),
            serde_json::json!(["--ctx-size"]),
            serde_json::json!(["--ctx-size", "invalid"]),
            serde_json::json!(["--ctx-size", "0"]),
            serde_json::json!(["--ctx-size", 18000]),
            serde_json::Value::Null,
        ] {
            let mut malformed = row.clone();
            malformed["status"]["args"] = args;
            assert!(parse_router_context_catalog(
                &serde_json::json!({"data":[malformed]}),
                "selected"
            )
            .is_err());
        }
    }

    #[test]
    fn router_b9090_fresh_unloaded_catalog_allows_explicit_load() {
        // Captured from b9090-5757c4dcb before any load POST; minimized to the
        // selected identity and status fields consumed at this boundary.
        let model_id = "llm/qwen3/qwen3-4b-instruct-2507-q6_kcopy1";
        let catalog = serde_json::json!({"data": [{
            "id": model_id,
            "status": {"value": "unloaded", "exit_code": 10, "failed": true}
        }]});
        assert_eq!(
            router_catalog_model_ready(&catalog, model_id, false),
            Ok(true)
        );
        assert!(router_catalog_model_ready(&catalog, model_id, true).is_err());
        for value in ["loaded", "loading", "sleeping", "invalid"] {
            let invalid = serde_json::json!({"data": [{"id": model_id,
                "status": {"value": value, "failed": true}}]});
            assert!(router_catalog_model_ready(&invalid, model_id, false).is_err());
            assert!(router_catalog_model_ready(&invalid, model_id, true).is_err());
        }
    }

    #[test]
    fn dedicated_identity_requires_one_matching_loaded_model() {
        let expected = "/models/selected.gguf";
        for payload in [
            serde_json::json!({}),
            serde_json::json!({"data": []}),
            serde_json::json!({"data": [{"id": "other", "meta": {}}]}),
            serde_json::json!({"data": [{"id": expected}]}),
            serde_json::json!({"data": [{"id": expected, "meta": false}]}),
            serde_json::json!({"data": [{"id": expected, "meta": {}}, {"id": expected, "meta": {}}]}),
        ] {
            assert!(dedicated_model_identity_ready(&payload, &[expected]).is_err());
        }
        assert_eq!(
            dedicated_model_identity_ready(
                &serde_json::json!({"data": [{"id": expected, "meta": null}]}),
                &[expected]
            ),
            Ok(false)
        );
        assert_eq!(
            dedicated_model_identity_ready(
                &serde_json::json!({"data": [{"id": expected, "meta": {}}]}),
                &[expected]
            ),
            Ok(true)
        );
        // A launch with a projector sets `--alias <library model ID>`; llama.cpp
        // then reports the alias, which the launch also owns.
        let library_id = "embedding/gemma-embedding2/embeddinggemma-2-gguf";
        assert_eq!(
            dedicated_model_identity_ready(
                &serde_json::json!({"data": [{"id": library_id, "meta": {}}]}),
                &[expected, library_id]
            ),
            Ok(true)
        );
        assert!(dedicated_model_identity_ready(
            &serde_json::json!({"data": [{"id": library_id, "meta": {}}]}),
            &[expected]
        )
        .is_err());
    }

    #[tokio::test]
    async fn dedicated_health_loading_then_ready_requires_model_identity() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let requests = Arc::new(AtomicUsize::new(0));
        let health_requests = requests.clone();
        let app = axum::Router::new()
            .route(
                "/health",
                axum::routing::get(move || {
                    let requests = health_requests.clone();
                    async move {
                        if requests.fetch_add(1, Ordering::SeqCst) == 0 {
                            (
                                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                                axum::Json(serde_json::json!({"error": "loading"})),
                            )
                        } else {
                            (
                                axum::http::StatusCode::OK,
                                axum::Json(serde_json::json!({"status": "ok"})),
                            )
                        }
                    }
                }),
            )
            .route(
                "/v1/models",
                axum::routing::get(|| async {
                    axum::Json(
                        serde_json::json!({"data": [{"id": "/models/test.gguf", "meta": {}}]}),
                    )
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = stopped.await;
        });
        let check = async {
            let client = LlamaCppRouterClient::new(reqwest::Client::new());
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            assert_eq!(
                client
                    .dedicated_model_ready(&endpoint, &["/models/test.gguf"], deadline)
                    .await,
                Ok(false)
            );
            assert_eq!(
                client
                    .dedicated_model_ready(&endpoint, &["/models/test.gguf"], deadline)
                    .await,
                Ok(true)
            );
            assert!(client
                .dedicated_model_ready(&endpoint, &["/models/wrong.gguf"], deadline)
                .await
                .is_err());
            stop.send(()).unwrap();
        };
        let (served, ()) = tokio::join!(server, check);
        served.unwrap();
    }

    #[test]
    fn llama_cpp_router_models_url_normalizes_trailing_slash() {
        assert_eq!(
            llama_cpp_router_models_url("http://127.0.0.1:20617/"),
            "http://127.0.0.1:20617/v1/models"
        );
        assert_eq!(
            llama_cpp_router_models_url("http://127.0.0.1:20617"),
            "http://127.0.0.1:20617/v1/models"
        );
    }

    #[test]
    fn llama_cpp_router_model_load_url_normalizes_trailing_slash() {
        assert_eq!(
            llama_cpp_router_model_load_url("http://127.0.0.1:20617/"),
            "http://127.0.0.1:20617/models/load"
        );
        assert_eq!(
            llama_cpp_router_model_load_url("http://127.0.0.1:20617"),
            "http://127.0.0.1:20617/models/load"
        );
    }

    #[test]
    fn llama_cpp_router_model_unload_url_normalizes_trailing_slash() {
        assert_eq!(
            llama_cpp_router_model_unload_url("http://127.0.0.1:20617/"),
            "http://127.0.0.1:20617/models/unload"
        );
        assert_eq!(
            llama_cpp_router_model_unload_url("http://127.0.0.1:20617"),
            "http://127.0.0.1:20617/models/unload"
        );
    }
}
