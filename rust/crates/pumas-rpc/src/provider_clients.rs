//! Reusable provider HTTP clients owned by RPC composition state.

use std::time::Duration;

const LLAMA_CPP_ROUTER_UNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

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
    http: reqwest::Client,
}

impl LlamaCppRouterClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub(crate) async fn router_catalog_ready(
        &self,
        endpoint: &str,
        model_id: &str,
        require_loaded: bool,
        deadline: tokio::time::Instant,
    ) -> Result<bool, String> {
        let response = self
            .http
            .get(llama_cpp_router_models_url(endpoint))
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
            return Err("llama.cpp router model endpoint returned an unexpected status".into());
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "llama.cpp router model endpoint returned invalid JSON")?;
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
            .get(url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await
            .map_err(|_| "llama.cpp router context catalog request failed")?;
        if response.status() != reqwest::StatusCode::OK {
            return Err("llama.cpp router context catalog returned an unexpected status".into());
        }
        let payload = response
            .json()
            .await
            .map_err(|_| "llama.cpp router context catalog returned invalid JSON")?;
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
            .get(url)
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await
            .map_err(|_| "llama.cpp router runtime context request failed")?;
        if response.status() != reqwest::StatusCode::OK {
            return Err("llama.cpp router runtime context returned an unexpected status".into());
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "llama.cpp router runtime context returned invalid JSON")?;
        let actual = payload
            .get("default_generation_settings")
            .and_then(|settings| settings.get("n_ctx"))
            .and_then(serde_json::Value::as_u64)
            .ok_or("llama.cpp router runtime context is missing or invalid")?;
        if actual < u64::from(requested) {
            return Err("llama.cpp router runtime context is smaller than requested".into());
        }
        Ok(())
    }

    /// Prove readiness of one dedicated server against its owned launch model.
    /// The caller owns the overall startup deadline and process identity checks.
    pub(crate) async fn dedicated_model_ready(
        &self,
        endpoint: &str,
        model_path: &str,
        deadline: tokio::time::Instant,
    ) -> Result<bool, String> {
        let health = self
            .http
            .get(format!("{}/health", endpoint.trim_end_matches('/')))
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
            return Err("llama.cpp health endpoint returned an unexpected status".into());
        }
        let health: serde_json::Value = health
            .json()
            .await
            .map_err(|_| "llama.cpp health endpoint returned invalid JSON")?;
        if health.get("status").and_then(serde_json::Value::as_str) != Some("ok") {
            return Err("llama.cpp health endpoint did not report ready".into());
        }
        let models = self
            .http
            .get(llama_cpp_router_models_url(endpoint))
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .send()
            .await
            .map_err(|_| "llama.cpp model identity request failed")?;
        if models.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            return Ok(false);
        }
        if models.status() != reqwest::StatusCode::OK {
            return Err("llama.cpp model endpoint returned an unexpected status".into());
        }
        let models: serde_json::Value = models
            .json()
            .await
            .map_err(|_| "llama.cpp model endpoint returned invalid JSON")?;
        dedicated_model_identity_ready(&models, model_path)
    }

    pub async fn load_model(
        &self,
        endpoint: &str,
        model_alias: &str,
        deadline: tokio::time::Instant,
    ) -> Result<(), String> {
        let response = self
            .http
            .post(llama_cpp_router_model_load_url(endpoint))
            .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
            .json(&serde_json::json!({ "model": model_alias }))
            .send()
            .await
            .map_err(|err| format!("failed to load model through llama.cpp router: {err}"))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.is_success() || body.contains("model is already running") {
            return Ok(());
        }
        Err(if body.trim().is_empty() {
            format!("llama.cpp router failed to load model with HTTP status {status}")
        } else {
            format!("llama.cpp router failed to load model with HTTP status {status}: {body}")
        })
    }

    pub async fn unload_model(&self, endpoint: &str, model_alias: &str) -> Result<(), String> {
        let response = self
            .http
            .post(llama_cpp_router_model_unload_url(endpoint))
            .timeout(LLAMA_CPP_ROUTER_UNLOAD_TIMEOUT)
            .json(&serde_json::json!({ "model": model_alias }))
            .send()
            .await
            .map_err(|err| format!("failed to unload model through llama.cpp router: {err}"))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.is_success() || body.contains("model is not running") {
            return Ok(());
        }
        Err(if body.trim().is_empty() {
            format!("llama.cpp router failed to unload model with HTTP status {status}")
        } else {
            format!("llama.cpp router failed to unload model with HTTP status {status}: {body}")
        })
    }
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
    model_path: &str,
) -> Result<bool, String> {
    let rows = models
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or("llama.cpp model endpoint omitted model identity")?;
    if rows.len() != 1 || rows[0].get("id").and_then(serde_json::Value::as_str) != Some(model_path)
    {
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
            assert!(dedicated_model_identity_ready(&payload, expected).is_err());
        }
        assert_eq!(
            dedicated_model_identity_ready(
                &serde_json::json!({"data": [{"id": expected, "meta": null}]}),
                expected
            ),
            Ok(false)
        );
        assert_eq!(
            dedicated_model_identity_ready(
                &serde_json::json!({"data": [{"id": expected, "meta": {}}]}),
                expected
            ),
            Ok(true)
        );
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
                    .dedicated_model_ready(&endpoint, "/models/test.gguf", deadline)
                    .await,
                Ok(false)
            );
            assert_eq!(
                client
                    .dedicated_model_ready(&endpoint, "/models/test.gguf", deadline)
                    .await,
                Ok(true)
            );
            assert!(client
                .dedicated_model_ready(&endpoint, "/models/wrong.gguf", deadline)
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
