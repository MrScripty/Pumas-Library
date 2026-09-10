//! Reusable provider HTTP clients owned by RPC composition state.

use std::time::Duration;

const LLAMA_CPP_ROUTER_READY_TIMEOUT: Duration = Duration::from_secs(2);
const LLAMA_CPP_ROUTER_LOAD_TIMEOUT: Duration = Duration::from_secs(180);
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

    pub async fn endpoint_ready(&self, endpoint: &str) -> bool {
        self.http
            .get(llama_cpp_router_models_url(endpoint))
            .timeout(LLAMA_CPP_ROUTER_READY_TIMEOUT)
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false)
    }

    pub async fn load_model(&self, endpoint: &str, model_alias: &str) -> Result<(), String> {
        let response = self
            .http
            .post(llama_cpp_router_model_load_url(endpoint))
            .timeout(LLAMA_CPP_ROUTER_LOAD_TIMEOUT)
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
