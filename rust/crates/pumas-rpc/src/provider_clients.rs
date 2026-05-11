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
