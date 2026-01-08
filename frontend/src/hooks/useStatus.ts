/**
 * Status polling hook
 *
 * Manages system status polling and state.
 * Extracted from App.tsx to separate concerns.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { pywebview } from '../api/pywebview';
import type { StatusResponse } from '../types/pywebview';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useStatus');

interface UseStatusOptions {
  pollInterval?: number;
  initialLoad?: boolean;
}

export function useStatus(options: UseStatusOptions = {}) {
  const { pollInterval = 500, initialLoad = true } = options;

  const [statusData, setStatusData] = useState<StatusResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isCheckingDeps, setIsCheckingDeps] = useState(true);
  const isPolling = useRef(false);

  const fetchStatus = useCallback(async (isInitialLoad = false) => {
    if (isPolling.current) {
      return;
    }

    const startTime = Date.now();

    if (isInitialLoad) {
      setIsCheckingDeps(true);
    }

    isPolling.current = true;

    try {
      if (!pywebview.isAvailable()) {
        setIsLoading(false);
        setIsCheckingDeps(false);
        return;
      }

      const data = await pywebview.getStatus();
      setStatusData(data);

      if (isInitialLoad) {
        const elapsedTime = Date.now() - startTime;
        const remainingTime = Math.max(0, 800 - elapsedTime);
        setTimeout(() => {
          setIsLoading(false);
          setIsCheckingDeps(false);
        }, remainingTime);
      } else {
        setIsLoading(false);
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching status', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching status', { error: error.message });
      } else {
        logger.error('Unknown error fetching status', { error });
      }
      setIsLoading(false);
      setIsCheckingDeps(false);
    } finally {
      isPolling.current = false;
    }
  }, []);

  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    let waitTimeout: NodeJS.Timeout | null = null;

    const startPolling = () => {
      if (initialLoad) {
        fetchStatus(true).catch(error => {
          if (error instanceof APIError) {
            logger.error('Initial status fetch failed', { error: error.message, endpoint: error.endpoint });
          } else if (error instanceof Error) {
            logger.error('Unexpected error during initial fetch', { error: error.message });
          } else {
            logger.error('Unknown error during initial fetch', { error });
          }
          setIsLoading(false);
          setIsCheckingDeps(false);
        });
      }

      interval = setInterval(() => {
        void fetchStatus(false);
      }, pollInterval);
    };

    const waitForApi = () => {
      if (window.pywebview?.api && typeof window.pywebview.api.get_status === 'function') {
        startPolling();
        return;
      }
      waitTimeout = setTimeout(waitForApi, 100);
    };

    waitForApi();

    return () => {
      if (interval) clearInterval(interval);
      if (waitTimeout) clearTimeout(waitTimeout);
    };
  }, [pollInterval, initialLoad, fetchStatus]);

  return {
    status: statusData,
    isLoading,
    isCheckingDeps,
    refetch: fetchStatus,
  };
}
