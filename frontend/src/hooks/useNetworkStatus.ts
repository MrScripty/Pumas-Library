/**
 * Network Status Hook
 *
 * Monitors network status and circuit breaker state for UI indicators.
 * Polls every 5 seconds and provides offline/rate limit warnings.
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { importAPI } from '../api/import';
import type { NetworkStatusResponse } from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('useNetworkStatus');

/** Polling interval for network status (ms) */
const POLL_INTERVAL_MS = 5000;

/** Status data exposed by the hook */
export interface NetworkStatus {
  /** Whether any circuit breaker is open (offline mode) */
  isOffline: boolean;
  /** Whether rate limiting is approaching (< 10% remaining) */
  isRateLimited: boolean;
  /** Current success rate as percentage */
  successRate: number;
  /** Number of circuit breaker rejections */
  circuitBreakerRejections: number;
  /** Map of domain to circuit state */
  circuitStates: Record<string, string>;
  /** Total number of requests made */
  totalRequests: number;
  /** Number of failed requests */
  failedRequests: number;
}

const DEFAULT_STATUS: NetworkStatus = {
  isOffline: false,
  isRateLimited: false,
  successRate: 100,
  circuitBreakerRejections: 0,
  circuitStates: {},
  totalRequests: 0,
  failedRequests: 0,
};

export function useNetworkStatus() {
  const [status, setStatus] = useState<NetworkStatus>(DEFAULT_STATUS);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const pollingRef = useRef(false);

  const fetchStatus = useCallback(async () => {
    if (pollingRef.current) {
      return;
    }

    pollingRef.current = true;
    try {
      const response: NetworkStatusResponse = await importAPI.getNetworkStatus();

      if (response.success) {
        const totalRequests = response.total_requests ?? 0;
        const successRate = totalRequests === 0
          ? 100
          : typeof response.success_rate === 'number'
            ? response.success_rate
            : 0;
        setStatus({
          isOffline: response.is_offline ?? false,
          isRateLimited: totalRequests > 0 && successRate < 50,
          successRate,
          circuitBreakerRejections: response.circuit_breaker_rejections ?? 0,
          circuitStates: response.circuit_states ?? {},
          totalRequests,
          failedRequests: response.failed_requests ?? 0,
        });
        setError(null);
      } else {
        logger.warn('Failed to fetch network status', { error: response.error });
        setError(response.error || 'Failed to fetch network status');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      logger.error('Error fetching network status', { error: message });
      setError(message);
    } finally {
      setIsLoading(false);
      pollingRef.current = false;
    }
  }, []);

  // Initial fetch and polling
  useEffect(() => {
    // Initial fetch
    void fetchStatus();

    // Set up polling
    const interval = setInterval(() => {
      void fetchStatus();
    }, POLL_INTERVAL_MS);

    return () => clearInterval(interval);
  }, [fetchStatus]);

  // Manual refresh
  const refresh = useCallback(() => {
    void fetchStatus();
  }, [fetchStatus]);

  return {
    ...status,
    isLoading,
    error,
    refresh,
  };
}
