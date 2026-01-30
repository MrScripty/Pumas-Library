//! NetworkManager - Centralized network connectivity and request management.
//!
//! Provides:
//! - Network connectivity checking before making requests
//! - Circuit breaker management per domain
//! - WebSource registration and management
//! - Automatic offline fallback to cached data

use crate::network::circuit_breaker::{CircuitBreaker, CircuitBreakerStats};
use crate::network::client::HttpClient;
use crate::network::web_source::DynWebSource;
use crate::{PumasError, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Network connectivity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityState {
    /// Network is available.
    Online = 0,
    /// Network is not available.
    Offline = 1,
    /// Connectivity is being checked.
    Checking = 2,
    /// Unknown (initial state before first check).
    Unknown = 3,
}

impl std::fmt::Display for ConnectivityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectivityState::Online => write!(f, "online"),
            ConnectivityState::Offline => write!(f, "offline"),
            ConnectivityState::Checking => write!(f, "checking"),
            ConnectivityState::Unknown => write!(f, "unknown"),
        }
    }
}

/// Configuration for connectivity checking.
#[derive(Debug, Clone)]
pub struct ConnectivityConfig {
    /// URLs to probe for connectivity (in order of preference).
    pub probe_urls: Vec<String>,
    /// Timeout for connectivity probes.
    pub probe_timeout: Duration,
    /// How often to re-check connectivity when offline.
    pub offline_recheck_interval: Duration,
    /// How often to verify connectivity when online.
    pub online_verify_interval: Duration,
}

impl Default for ConnectivityConfig {
    fn default() -> Self {
        Self {
            probe_urls: vec![
                // Use lightweight endpoints that respond quickly
                "https://api.github.com".to_string(),
                "https://huggingface.co/api/models?limit=1".to_string(),
            ],
            probe_timeout: Duration::from_secs(5),
            offline_recheck_interval: Duration::from_secs(30),
            online_verify_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Atomic wrapper for ConnectivityState.
struct AtomicConnectivityState(AtomicU8);

impl AtomicConnectivityState {
    fn new(state: ConnectivityState) -> Self {
        Self(AtomicU8::new(state as u8))
    }

    fn load(&self) -> ConnectivityState {
        match self.0.load(Ordering::SeqCst) {
            0 => ConnectivityState::Online,
            1 => ConnectivityState::Offline,
            2 => ConnectivityState::Checking,
            _ => ConnectivityState::Unknown,
        }
    }

    fn store(&self, state: ConnectivityState) {
        self.0.store(state as u8, Ordering::SeqCst);
    }
}

/// Central network manager for all web operations.
///
/// The NetworkManager provides:
/// - Connectivity state tracking (online/offline detection)
/// - Circuit breaker management per domain
/// - WebSource registration for extensible source management
/// - Request execution with automatic cache fallback when offline
pub struct NetworkManager {
    /// Shared HTTP client for connectivity probes.
    http_client: Arc<HttpClient>,
    /// Current connectivity state.
    connectivity_state: AtomicConnectivityState,
    /// Last successful connectivity check time.
    last_connectivity_check: RwLock<Option<Instant>>,
    /// Last time we detected offline state.
    last_offline_time: RwLock<Option<Instant>>,
    /// Circuit breakers per domain.
    circuit_breakers: RwLock<HashMap<String, CircuitBreaker>>,
    /// Registered web sources.
    sources: RwLock<HashMap<String, DynWebSource>>,
    /// Connectivity configuration.
    config: ConnectivityConfig,
    /// Whether background monitoring is active.
    monitoring_active: AtomicBool,
}

impl NetworkManager {
    /// Create a new NetworkManager with default configuration.
    pub fn new() -> Result<Self> {
        Self::with_config(ConnectivityConfig::default())
    }

    /// Create with custom configuration.
    pub fn with_config(config: ConnectivityConfig) -> Result<Self> {
        let http_client = Arc::new(HttpClient::new()?);

        Ok(Self {
            http_client,
            connectivity_state: AtomicConnectivityState::new(ConnectivityState::Unknown),
            last_connectivity_check: RwLock::new(None),
            last_offline_time: RwLock::new(None),
            circuit_breakers: RwLock::new(HashMap::new()),
            sources: RwLock::new(HashMap::new()),
            config,
            monitoring_active: AtomicBool::new(false),
        })
    }

    // === Connectivity Management ===

    /// Get current connectivity state.
    pub fn connectivity(&self) -> ConnectivityState {
        self.connectivity_state.load()
    }

    /// Check if network is currently online.
    pub fn is_online(&self) -> bool {
        self.connectivity_state.load() == ConnectivityState::Online
    }

    /// Check if network is offline (known to be unavailable).
    pub fn is_offline(&self) -> bool {
        self.connectivity_state.load() == ConnectivityState::Offline
    }

    /// Check connectivity by probing known endpoints.
    ///
    /// This performs a lightweight HEAD request to configured probe URLs.
    /// Returns the detected connectivity state.
    pub async fn check_connectivity(&self) -> ConnectivityState {
        let was_offline = self.is_offline();
        self.connectivity_state.store(ConnectivityState::Checking);

        for url in &self.config.probe_urls {
            match self.probe_url(url).await {
                Ok(true) => {
                    self.connectivity_state.store(ConnectivityState::Online);
                    *self.last_connectivity_check.write().await = Some(Instant::now());

                    if was_offline {
                        info!("Network connectivity restored");
                        self.notify_network_restored().await;
                    }

                    return ConnectivityState::Online;
                }
                Ok(false) | Err(_) => {
                    debug!("Probe failed for {}", url);
                    continue;
                }
            }
        }

        // All probes failed
        self.connectivity_state.store(ConnectivityState::Offline);
        *self.last_offline_time.write().await = Some(Instant::now());

        if !was_offline {
            warn!("Network connectivity lost - all probe URLs failed");
        }

        ConnectivityState::Offline
    }

    /// Quickly check if we should skip network requests.
    ///
    /// Returns true if:
    /// - We recently detected offline state (within recheck interval)
    /// - The circuit breaker for the domain is open
    ///
    /// This is a fast check that doesn't make network requests.
    pub async fn should_skip_network(&self, domain: &str) -> bool {
        // If we're in unknown state, we should try the network
        let state = self.connectivity_state.load();
        if state == ConnectivityState::Unknown {
            return false;
        }

        // If we know we're offline, skip
        if state == ConnectivityState::Offline {
            // But check if we should recheck
            if let Some(offline_time) = *self.last_offline_time.read().await {
                if offline_time.elapsed() >= self.config.offline_recheck_interval {
                    // Time to recheck - don't skip, let the request trigger a check
                    return false;
                }
            }
            return true;
        }

        // Check circuit breaker
        !self.can_request(domain).await
    }

    /// Probe a URL to check connectivity (HEAD request with short timeout).
    async fn probe_url(&self, url: &str) -> Result<bool> {
        let client = reqwest::Client::builder()
            .timeout(self.config.probe_timeout)
            .build()
            .map_err(|e| PumasError::Network {
                message: format!("Failed to create probe client: {}", e),
                cause: None,
            })?;

        match client.head(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                // Accept success, redirects, and even some client errors (like 403 for rate limiting)
                // as signs that network is working
                Ok(status.is_success() || status.is_redirection() || status.as_u16() == 403)
            }
            Err(e) => {
                debug!("Probe request failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Notify all sources that network is restored.
    async fn notify_network_restored(&self) {
        let sources = self.sources.read().await;
        for (id, source) in sources.iter() {
            debug!("Notifying source {} of network restoration", id);
            source.on_network_restored().await;
        }
    }

    // === Source Management ===

    /// Register a web source.
    pub async fn register_source(&self, source: DynWebSource) {
        let id = source.id().to_string();

        // Create circuit breakers for all domains
        {
            let mut breakers = self.circuit_breakers.write().await;
            for domain in source.domains() {
                if !breakers.contains_key(*domain) {
                    debug!("Creating circuit breaker for domain: {}", domain);
                    breakers.insert(domain.to_string(), CircuitBreaker::new(*domain));
                }
            }
        }

        info!("Registered web source: {}", id);
        self.sources.write().await.insert(id, source);
    }

    /// Get a registered source by ID.
    pub async fn get_source(&self, id: &str) -> Option<DynWebSource> {
        self.sources.read().await.get(id).cloned()
    }

    /// List all registered source IDs.
    pub async fn source_ids(&self) -> Vec<String> {
        self.sources.read().await.keys().cloned().collect()
    }

    // === Circuit Breaker Integration ===

    /// Check if a domain's circuit breaker allows requests.
    pub async fn can_request(&self, domain: &str) -> bool {
        let breakers = self.circuit_breakers.read().await;
        breakers
            .get(domain)
            .map(|cb| cb.allow_request())
            .unwrap_or(true) // Allow if no breaker exists
    }

    /// Record success for a domain's circuit breaker.
    pub async fn record_success(&self, domain: &str) {
        let breakers = self.circuit_breakers.read().await;
        if let Some(cb) = breakers.get(domain) {
            cb.record_success();
        }
    }

    /// Record failure for a domain's circuit breaker.
    pub async fn record_failure(&self, domain: &str) {
        let breakers = self.circuit_breakers.read().await;
        if let Some(cb) = breakers.get(domain) {
            cb.record_failure();

            // Notify source if circuit opens
            let state = cb.state();
            if state == crate::network::circuit_breaker::CircuitState::Open {
                drop(breakers); // Release read lock before notifying
                self.notify_circuit_open(domain).await;
            }
        }
    }

    /// Notify sources that a circuit breaker opened.
    async fn notify_circuit_open(&self, domain: &str) {
        let sources = self.sources.read().await;
        for source in sources.values() {
            if source.domains().contains(&domain) {
                source.on_circuit_open(domain);
            }
        }
    }

    /// Get or create a circuit breaker for a domain.
    pub async fn get_or_create_circuit_breaker(&self, domain: &str) -> CircuitBreakerStats {
        {
            let breakers = self.circuit_breakers.read().await;
            if let Some(cb) = breakers.get(domain) {
                return cb.stats();
            }
        }

        // Create new breaker
        let mut breakers = self.circuit_breakers.write().await;
        let cb = CircuitBreaker::new(domain);
        let stats = cb.stats();
        breakers.insert(domain.to_string(), cb);
        stats
    }

    // === Request Execution ===

    /// Execute a network request with connectivity and circuit breaker checks.
    ///
    /// This method:
    /// 1. Checks if we're offline - if so, returns cached data or error
    /// 2. Checks circuit breaker - if open, returns cached data or error
    /// 3. Executes the operation
    /// 4. On failure, tries to return cached data
    /// 5. Records success/failure with circuit breaker
    ///
    /// # Arguments
    ///
    /// * `domain` - The domain being accessed (for circuit breaker)
    /// * `cache_key` - Key for cache lookup
    /// * `operation` - The async operation to execute
    /// * `get_cached` - Function to retrieve cached data
    pub async fn execute<F, T, Fut>(
        &self,
        domain: &str,
        cache_key: &str,
        operation: F,
        get_cached: impl FnOnce() -> Option<T>,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
        T: Clone,
    {
        // Check if we should skip network entirely
        if self.should_skip_network(domain).await {
            if let Some(cached) = get_cached() {
                debug!(
                    "Skipping network for {} (offline/circuit open), using cache",
                    cache_key
                );
                return Ok(cached);
            }

            // No cache and offline
            if self.is_offline() {
                return Err(PumasError::Network {
                    message: "Network unavailable and no cached data".to_string(),
                    cause: None,
                });
            } else {
                return Err(PumasError::CircuitBreakerOpen {
                    domain: domain.to_string(),
                });
            }
        }

        // Execute operation
        match operation().await {
            Ok(result) => {
                self.record_success(domain).await;
                // Update connectivity state on success
                if self.connectivity_state.load() != ConnectivityState::Online {
                    self.connectivity_state.store(ConnectivityState::Online);
                    *self.last_connectivity_check.write().await = Some(Instant::now());
                }
                Ok(result)
            }
            Err(e) => {
                // Record failure if it's a network error
                if e.is_retryable() {
                    self.record_failure(domain).await;

                    // Check if this looks like an offline situation
                    if matches!(&e, PumasError::Network { .. } | PumasError::Timeout(_)) {
                        // Try other probe URLs to confirm offline state
                        let connectivity = self.check_connectivity().await;
                        if connectivity == ConnectivityState::Offline {
                            debug!("Confirmed offline state after request failure");
                        }
                    }
                }

                // Try cache on failure
                if let Some(cached) = get_cached() {
                    warn!(
                        "Request failed for {}, using cached data: {}",
                        cache_key, e
                    );
                    return Ok(cached);
                }

                Err(e)
            }
        }
    }

    // === Background Monitoring ===

    /// Start background connectivity monitoring.
    ///
    /// This spawns a task that periodically checks connectivity.
    pub fn start_monitoring(self: &Arc<Self>) {
        if self.monitoring_active.swap(true, Ordering::SeqCst) {
            debug!("Background monitoring already active");
            return;
        }

        let manager = Arc::clone(self);
        tokio::spawn(async move {
            info!("Starting background connectivity monitoring");

            while manager.monitoring_active.load(Ordering::SeqCst) {
                let interval = if manager.is_online() {
                    manager.config.online_verify_interval
                } else {
                    manager.config.offline_recheck_interval
                };

                tokio::time::sleep(interval).await;

                if !manager.monitoring_active.load(Ordering::SeqCst) {
                    break;
                }

                debug!("Background connectivity check");
                manager.check_connectivity().await;
            }

            info!("Background connectivity monitoring stopped");
        });
    }

    /// Stop background monitoring.
    pub fn stop_monitoring(&self) {
        self.monitoring_active.store(false, Ordering::SeqCst);
    }

    /// Check if background monitoring is active.
    pub fn is_monitoring(&self) -> bool {
        self.monitoring_active.load(Ordering::SeqCst)
    }

    // === Accessors ===

    /// Get the shared HTTP client.
    pub fn http_client(&self) -> Arc<HttpClient> {
        Arc::clone(&self.http_client)
    }

    /// Get circuit breaker stats for all domains.
    pub async fn circuit_breaker_stats(&self) -> Vec<CircuitBreakerStats> {
        self.circuit_breakers
            .read()
            .await
            .values()
            .map(|cb| cb.stats())
            .collect()
    }

    /// Get connectivity status summary.
    pub async fn status(&self) -> NetworkStatus {
        NetworkStatus {
            connectivity: self.connectivity(),
            last_check: self.last_connectivity_check.read().await.map(|t| t.elapsed()),
            last_offline: self.last_offline_time.read().await.map(|t| t.elapsed()),
            circuit_breakers: self.circuit_breaker_stats().await,
            registered_sources: self.source_ids().await,
            monitoring_active: self.is_monitoring(),
        }
    }
}

/// Network status summary.
#[derive(Debug, Clone)]
pub struct NetworkStatus {
    pub connectivity: ConnectivityState,
    pub last_check: Option<Duration>,
    pub last_offline: Option<Duration>,
    pub circuit_breakers: Vec<CircuitBreakerStats>,
    pub registered_sources: Vec<String>,
    pub monitoring_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_manager_creation() {
        let manager = NetworkManager::new().unwrap();
        assert_eq!(manager.connectivity(), ConnectivityState::Unknown);
        assert!(!manager.is_online());
        assert!(!manager.is_offline());
    }

    #[tokio::test]
    async fn test_circuit_breaker_integration() {
        let manager = NetworkManager::new().unwrap();

        // First request should be allowed (no breaker yet)
        assert!(manager.can_request("example.com").await);

        // Create breaker
        manager.get_or_create_circuit_breaker("example.com").await;

        // Still allowed
        assert!(manager.can_request("example.com").await);

        // Record failures until circuit opens (default threshold is 5)
        for _ in 0..5 {
            manager.record_failure("example.com").await;
        }

        // Circuit should be open now
        assert!(!manager.can_request("example.com").await);
    }

    #[tokio::test]
    async fn test_execute_with_cache_fallback() {
        let manager = NetworkManager::new().unwrap();

        // Create a circuit breaker and open it
        manager.get_or_create_circuit_breaker("test.com").await;
        for _ in 0..5 {
            manager.record_failure("test.com").await;
        }

        // Execute should fall back to cache
        let result: Result<String> = manager
            .execute(
                "test.com",
                "test-key",
                || async { Err(PumasError::Network { message: "test".into(), cause: None }) },
                || Some("cached-value".to_string()),
            )
            .await;

        assert_eq!(result.unwrap(), "cached-value");
    }

    #[tokio::test]
    async fn test_status() {
        let manager = NetworkManager::new().unwrap();
        let status = manager.status().await;

        assert_eq!(status.connectivity, ConnectivityState::Unknown);
        assert!(status.registered_sources.is_empty());
        assert!(!status.monitoring_active);
    }
}
