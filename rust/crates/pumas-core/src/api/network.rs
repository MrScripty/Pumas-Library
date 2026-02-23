//! Network connectivity methods on PumasApi.

use std::sync::Arc;

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

    /// Get the network manager for advanced operations.
    pub fn network_manager(&self) -> &Arc<network::NetworkManager> {
        &self.primary().network_manager
    }

    /// Get the model library for direct access.
    pub fn model_library(&self) -> &Arc<model_library::ModelLibrary> {
        &self.primary().model_library
    }
}
