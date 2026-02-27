//! Network connectivity methods on PumasApi.

use std::sync::Arc;

use crate::models;
use crate::model_library;
use crate::network;
use crate::PumasApi;

impl PumasApi {
    // ========================================
    // Network Connectivity
    // ========================================

    /// Check if network is currently online.
    pub fn is_online(&self) -> bool {
        self.primary().network_manager.is_online()
    }

    /// Get current network connectivity state.
    pub fn connectivity_state(&self) -> network::ConnectivityState {
        self.primary().network_manager.connectivity()
    }

    /// Check network connectivity (performs actual probe).
    pub async fn check_connectivity(&self) -> network::ConnectivityState {
        self.primary().network_manager.check_connectivity().await
    }

    /// Get detailed network status including circuit breaker states.
    pub async fn network_status(&self) -> network::NetworkStatus {
        self.primary().network_manager.status().await
    }

    /// Get frontend-facing network status counters and circuit states.
    pub async fn get_network_status_response(&self) -> models::NetworkStatusResponse {
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
            is_offline: status.connectivity == network::ConnectivityState::Offline || any_open_circuit,
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
