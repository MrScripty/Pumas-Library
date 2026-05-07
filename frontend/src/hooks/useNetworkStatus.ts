/**
 * Network status hook backed by status telemetry.
 */

import { useState, useEffect, useCallback } from 'react';
import type { NetworkStatusResponse, StatusTelemetrySnapshot } from '../types/api';
import { getLogger } from '../utils/logger';
import { useStatusTelemetry } from './statusTelemetryStore';

const logger = getLogger('useNetworkStatus');

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

function mapNetworkStatus(response: NetworkStatusResponse): NetworkStatus {
  const totalRequests = response.total_requests;
  const successRate = totalRequests === 0
    ? 100
    : response.success_rate <= 1
      ? response.success_rate * 100
      : response.success_rate;
  return {
    isOffline: response.is_offline,
    isRateLimited: totalRequests > 0 && successRate < 50,
    successRate,
    circuitBreakerRejections: response.circuit_breaker_rejections,
    circuitStates: response.circuit_states,
    totalRequests,
    failedRequests: response.failed_requests,
  };
}

export function useNetworkStatus() {
  const [status, setStatus] = useState<NetworkStatus>(DEFAULT_STATUS);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const telemetry = useStatusTelemetry({ loadInitial: true });

  const applySnapshot = useCallback((snapshot: StatusTelemetrySnapshot) => {
    setStatus(mapNetworkStatus(snapshot.network));
    setError(null);
    setIsLoading(false);
  }, []);

  const fetchStatus = useCallback(async () => {
    await telemetry.refetch();
  }, [telemetry]);

  useEffect(() => {
    if (!telemetry.snapshot) {
      return;
    }

    applySnapshot(telemetry.snapshot);
  }, [applySnapshot, telemetry.snapshot]);

  useEffect(() => {
    if (telemetry.error) {
      logger.error('Error fetching status telemetry network state', { error: telemetry.error });
      setError(telemetry.error);
      setIsLoading(false);
    }
  }, [telemetry.error]);

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
