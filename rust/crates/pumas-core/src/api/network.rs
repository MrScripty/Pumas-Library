//! Network connectivity methods on PumasApi.

use std::sync::Arc;

use crate::PumasApi;
use crate::model_library;
use crate::models;
use crate::network;

impl PumasApi {
    // ========================================
    // Network Connectivity
    // ========================================

    /// Check if network is currently online.
    pub fn is_online(&self) -> bool {
        if self.try_client().is_some() {
            return self.call_client_method_blocking_or_default("is_online", serde_json::json!({}));
        }

        self.primary().network_manager.is_online()
    }

    /// Get current network connectivity state.
    pub fn connectivity_state(&self) -> network::ConnectivityState {
        if self.try_client().is_some() {
            return self
                .call_client_method_blocking("connectivity_state", serde_json::json!({}))
                .unwrap_or_else(|err| {
                    tracing::warn!("Failed to proxy connectivity_state over IPC: {}", err);
                    network::ConnectivityState::Unknown
                });
        }

        self.primary().network_manager.connectivity()
    }

    /// Check network connectivity (performs actual probe).
    pub async fn check_connectivity(&self) -> network::ConnectivityState {
        if self.try_client().is_some() {
            return self
                .call_client_method("check_connectivity", serde_json::json!({}))
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!("Failed to proxy check_connectivity over IPC: {}", err);
                    network::ConnectivityState::Unknown
                });
        }

        self.primary().network_manager.check_connectivity().await
    }

    /// Get detailed network status including circuit breaker states.
    pub async fn network_status(&self) -> network::NetworkStatus {
        if self.try_client().is_some() {
            return self
                .call_client_method("network_status", serde_json::json!({}))
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!("Failed to proxy network_status over IPC: {}", err);
                    network::NetworkStatus {
                        connectivity: network::ConnectivityState::Unknown,
                        last_check: None,
                        last_offline: None,
                        circuit_breakers: vec![],
                        registered_sources: vec![],
                        monitoring_active: false,
                    }
                });
        }

        self.primary().network_manager.status().await
    }

    /// Get frontend-facing network status counters and circuit states.
    pub async fn get_network_status_response(&self) -> models::NetworkStatusResponse {
        if self.try_client().is_some() {
            return self
                .call_client_method("get_network_status_response", serde_json::json!({}))
                .await
                .unwrap_or_else(|err| models::NetworkStatusResponse {
                    success: false,
                    error: Some(err.to_string()),
                    total_requests: 0,
                    successful_requests: 0,
                    failed_requests: 0,
                    circuit_breaker_rejections: 0,
                    retries: 0,
                    success_rate: 0.0,
                    circuit_states: std::collections::HashMap::new(),
                    is_offline: false,
                });
        }

        let status = self.network_status().await;

        let mut total_successful_requests: u64 = 0;
        let mut total_failed_requests: u64 = 0;
        let mut circuit_states = std::collections::HashMap::new();
        let mut any_open_circuit = false;

        for breaker in &status.circuit_breakers {
            total_successful_requests += breaker.total_successes;
            total_failed_requests += breaker.total_failures;
            let state = breaker.state.to_string();
            if state == "OPEN" {
                any_open_circuit = true;
            }
            circuit_states.insert(breaker.domain.clone(), state);
        }

        let total_requests = total_successful_requests + total_failed_requests;
        let success_rate = if total_requests > 0 {
            total_successful_requests as f64 / total_requests as f64
        } else {
            1.0
        };

        models::NetworkStatusResponse {
            success: true,
            error: None,
            total_requests,
            successful_requests: total_successful_requests,
            failed_requests: total_failed_requests,
            // Rejection count is not tracked separately yet.
            circuit_breaker_rejections: 0,
            // Retry count is tracked per-request path, not globally aggregated yet.
            retries: 0,
            success_rate,
            circuit_states,
            is_offline: status.connectivity == network::ConnectivityState::Offline
                || any_open_circuit,
        }
    }

    /// Get the network manager for advanced operations.
    pub fn network_manager(&self) -> &Arc<network::NetworkManager> {
        &self.primary().network_manager
    }

    /// Get the model library for direct access.
    pub fn model_library(&self) -> &Arc<model_library::ModelLibrary> {
        &self.primary().model_library
    }
}
