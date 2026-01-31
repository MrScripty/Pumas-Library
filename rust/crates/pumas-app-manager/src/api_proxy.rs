//! Generic plugin API proxy.
//!
//! Enables config-driven API calls to app backends without app-specific code.
//! Reads endpoint definitions from plugin configs and forwards requests.

use pumas_library::plugins::{ApiEndpoint, PluginConfig, PluginLoader};
use pumas_library::{PumasError, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Generic API proxy for calling plugin-defined endpoints.
///
/// Reads endpoint configurations from plugins and makes HTTP requests
/// to the app's local API endpoints.
pub struct PluginApiProxy {
    /// Plugin loader for getting endpoint configs.
    plugin_loader: Arc<PluginLoader>,
    /// HTTP client for making requests.
    http_client: Client,
    /// Request timeout.
    timeout: Duration,
}

impl PluginApiProxy {
    /// Create a new plugin API proxy.
    pub fn new(plugin_loader: Arc<PluginLoader>) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create HTTP client: {}", e),
                cause: None,
            })?;

        Ok(Self {
            plugin_loader,
            http_client,
            timeout: Duration::from_secs(30),
        })
    }

    /// Create with custom timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Get the base URL for an app based on its connection config.
    fn get_base_url(&self, plugin: &PluginConfig) -> Option<String> {
        plugin.connection.as_ref().map(|c| {
            format!("{}://localhost:{}", c.protocol, c.default_port)
        })
    }

    /// Call an endpoint defined in the plugin config.
    ///
    /// # Arguments
    /// * `app_id` - The app to call
    /// * `endpoint_name` - Name of the endpoint (e.g., "loadModel", "stats")
    /// * `params` - Parameters to substitute in the body template
    pub async fn call_endpoint(
        &self,
        app_id: &str,
        endpoint_name: &str,
        params: HashMap<String, String>,
    ) -> Result<Value> {
        let plugin = self.plugin_loader.get(app_id).ok_or_else(|| PumasError::Config {
            message: format!("Plugin not found: {}", app_id),
        })?;

        let endpoint = plugin.api.get(endpoint_name).ok_or_else(|| PumasError::Config {
            message: format!("Endpoint '{}' not found for app '{}'", endpoint_name, app_id),
        })?;

        let base_url = self.get_base_url(&plugin).ok_or_else(|| PumasError::Config {
            message: format!("No connection config for app '{}'", app_id),
        })?;

        let url = format!("{}{}", base_url, endpoint.endpoint);
        debug!("Calling {} {} with params: {:?}", endpoint.method, url, params);

        let response = match endpoint.method.to_uppercase().as_str() {
            "GET" => {
                self.http_client
                    .get(&url)
                    .timeout(self.timeout)
                    .send()
                    .await
            }
            "POST" => {
                let body = self.interpolate_body(&endpoint.body_template, &params);
                self.http_client
                    .post(&url)
                    .json(&body)
                    .timeout(self.timeout)
                    .send()
                    .await
            }
            "PUT" => {
                let body = self.interpolate_body(&endpoint.body_template, &params);
                self.http_client
                    .put(&url)
                    .json(&body)
                    .timeout(self.timeout)
                    .send()
                    .await
            }
            "DELETE" => {
                self.http_client
                    .delete(&url)
                    .timeout(self.timeout)
                    .send()
                    .await
            }
            _ => {
                return Err(PumasError::Config {
                    message: format!("Unsupported HTTP method: {}", endpoint.method),
                });
            }
        };

        let response = response.map_err(|e| PumasError::Network {
            message: format!("API request failed: {}", e),
            cause: Some(e.to_string()),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(PumasError::Network {
                message: format!("API returned {}: {}", status, body),
                cause: None,
            });
        }

        let json: Value = response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse response: {}", e),
            source: None,
        })?;

        // Apply response mapping if configured
        let mapped = self.apply_response_mapping(&json, endpoint);

        Ok(mapped)
    }

    /// Get stats from an app using its stats endpoint config.
    pub async fn get_stats(&self, app_id: &str) -> Result<Value> {
        self.call_endpoint(app_id, "stats", HashMap::new()).await
    }

    /// List models from an app.
    pub async fn list_models(&self, app_id: &str) -> Result<Value> {
        self.call_endpoint(app_id, "listModels", HashMap::new()).await
    }

    /// Load a model in an app.
    pub async fn load_model(&self, app_id: &str, model_name: &str) -> Result<Value> {
        let mut params = HashMap::new();
        params.insert("model_name".to_string(), model_name.to_string());
        self.call_endpoint(app_id, "loadModel", params).await
    }

    /// Unload a model from an app.
    pub async fn unload_model(&self, app_id: &str, model_name: &str) -> Result<Value> {
        let mut params = HashMap::new();
        params.insert("model_name".to_string(), model_name.to_string());
        self.call_endpoint(app_id, "unloadModel", params).await
    }

    /// Check if an app is healthy via its health endpoint.
    pub async fn check_health(&self, app_id: &str) -> Result<bool> {
        let plugin = self.plugin_loader.get(app_id).ok_or_else(|| PumasError::Config {
            message: format!("Plugin not found: {}", app_id),
        })?;

        let conn = plugin.connection.as_ref().ok_or_else(|| PumasError::Config {
            message: format!("No connection config for app '{}'", app_id),
        })?;

        let health_endpoint = conn.health_endpoint.as_ref().ok_or_else(|| PumasError::Config {
            message: format!("No health endpoint for app '{}'", app_id),
        })?;

        let url = format!("{}://localhost:{}{}", conn.protocol, conn.default_port, health_endpoint);

        let result = self.http_client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Interpolate placeholders in the body template.
    ///
    /// Replaces `{{key}}` with the corresponding value from params.
    fn interpolate_body(&self, template: &Option<Value>, params: &HashMap<String, String>) -> Value {
        let template = match template {
            Some(t) => t.clone(),
            None => return Value::Null,
        };

        self.interpolate_value(template, params)
    }

    fn interpolate_value(&self, value: Value, params: &HashMap<String, String>) -> Value {
        match value {
            Value::String(s) => {
                let mut result = s;
                for (key, val) in params {
                    result = result.replace(&format!("{{{{{}}}}}", key), val);
                }
                Value::String(result)
            }
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k, self.interpolate_value(v, params));
                }
                Value::Object(new_map)
            }
            Value::Array(arr) => {
                Value::Array(arr.into_iter().map(|v| self.interpolate_value(v, params)).collect())
            }
            other => other,
        }
    }

    /// Apply response mapping from the endpoint config.
    ///
    /// Uses simple JSONPath-like expressions to extract fields.
    fn apply_response_mapping(&self, response: &Value, endpoint: &ApiEndpoint) -> Value {
        if endpoint.response_mapping.is_empty() {
            return response.clone();
        }

        let mut mapped = serde_json::Map::new();

        for (key, path) in &endpoint.response_mapping {
            if let Some(value) = self.extract_path(response, path) {
                mapped.insert(key.clone(), value);
            }
        }

        if mapped.is_empty() {
            response.clone()
        } else {
            Value::Object(mapped)
        }
    }

    /// Extract a value from JSON using a simple JSONPath-like expression.
    ///
    /// Supports:
    /// - `$.field` - Direct field access
    /// - `$.field.nested` - Nested field access
    /// - `$.array[*]` - All elements of an array
    /// - `$.array[*].field` - Field from each array element
    fn extract_path(&self, value: &Value, path: &str) -> Option<Value> {
        let path = path.strip_prefix("$.").unwrap_or(path);
        let parts: Vec<&str> = path.split('.').collect();

        self.extract_parts(value, &parts)
    }

    fn extract_parts(&self, value: &Value, parts: &[&str]) -> Option<Value> {
        if parts.is_empty() {
            return Some(value.clone());
        }

        let part = parts[0];
        let remaining = &parts[1..];

        // Handle array wildcard
        if part.ends_with("[*]") {
            let field = &part[..part.len() - 3];
            let array = if field.is_empty() {
                value.as_array()?
            } else {
                value.get(field)?.as_array()?
            };

            let results: Vec<Value> = array
                .iter()
                .filter_map(|elem| self.extract_parts(elem, remaining))
                .collect();

            return Some(Value::Array(results));
        }

        // Direct field access
        let next = value.get(part)?;
        self.extract_parts(next, remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_interpolate_body() {
        let plugin_loader = Arc::new(
            PluginLoader::new("/tmp/test-plugins").unwrap()
        );
        let proxy = PluginApiProxy::new(plugin_loader).unwrap();

        let template = Some(json!({
            "model": "{{model_name}}",
            "prompt": "",
            "keep_alive": "5m"
        }));

        let mut params = HashMap::new();
        params.insert("model_name".to_string(), "llama3".to_string());

        let result = proxy.interpolate_body(&template, &params);

        assert_eq!(result["model"], "llama3");
        assert_eq!(result["prompt"], "");
        assert_eq!(result["keep_alive"], "5m");
    }

    #[test]
    fn test_extract_path_simple() {
        let plugin_loader = Arc::new(
            PluginLoader::new("/tmp/test-plugins").unwrap()
        );
        let proxy = PluginApiProxy::new(plugin_loader).unwrap();

        let data = json!({
            "models": [
                {"name": "model1", "size": 1000},
                {"name": "model2", "size": 2000}
            ]
        });

        // Extract array
        let result = proxy.extract_path(&data, "$.models");
        assert!(result.is_some());
        assert!(result.unwrap().is_array());

        // Extract nested field from each element
        let result = proxy.extract_path(&data, "$.models[*].name");
        assert!(result.is_some());
        let names = result.unwrap();
        assert_eq!(names, json!(["model1", "model2"]));
    }
}
