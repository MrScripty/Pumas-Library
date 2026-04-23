/**
 * Status polling hook
 *
 * Manages system status polling and state.
 * Extracted from App.tsx to separate concerns.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { StatusResponse } from '../types/api';
import type { SystemResources } from '../types/apps';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useStatus');

interface UseStatusOptions {
  pollInterval?: number;
  initialLoad?: boolean;
}

export function useStatus(options: UseStatusOptions = {}) {
  const { pollInterval = 1000, initialLoad = true } = options;

  const [statusData, setStatusData] = useState<StatusResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isCheckingDeps, setIsCheckingDeps] = useState(true);
  const [systemResources, setSystemResources] = useState<SystemResources | undefined>(undefined);
  const [networkAvailable, setNetworkAvailable] = useState<boolean | null>(null);
  const [modelLibraryLoaded, setModelLibraryLoaded] = useState<boolean | null>(null);
  const inFlightRequest = useRef<Promise<void> | null>(null);
  const pendingRefresh = useRef<{ isInitialLoad: boolean; force: boolean } | null>(null);
  const lastResourcesFetch = useRef(0);
  const lastNetworkFetch = useRef(0);
  const lastLibraryFetch = useRef(0);
  const loadingDelayTimeout = useRef<NodeJS.Timeout | null>(null);

  const clearLoadingDelay = useCallback(() => {
    if (loadingDelayTimeout.current) {
      clearTimeout(loadingDelayTimeout.current);
      loadingDelayTimeout.current = null;
    }
  }, []);

  const refreshSystemResources = useCallback(async (now: number) => {
    if (now - lastResourcesFetch.current < 2000) return;

    const resourcesResult = await api.get_system_resources();
    if (resourcesResult?.success) {
      setSystemResources(resourcesResult.resources);
    }
    lastResourcesFetch.current = now;
  }, []);

  const refreshNetworkStatus = useCallback(async (now: number) => {
    if (now - lastNetworkFetch.current < 5000) return;

    try {
      const networkResult = await api.get_network_status();
      if (networkResult?.success) {
        setNetworkAvailable(!networkResult.is_offline);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      logger.debug('Failed to fetch network status', { error: message });
    }
    lastNetworkFetch.current = now;
  }, []);

  const refreshLibraryStatus = useCallback(async (now: number) => {
    if (now - lastLibraryFetch.current < 5000) return;

    try {
      const libraryResult = await api.get_library_status();
      setModelLibraryLoaded(Boolean(libraryResult?.success));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      logger.debug('Failed to fetch model library status', { error: message });
      setModelLibraryLoaded(false);
    }
    lastLibraryFetch.current = now;
  }, []);

  const runStatusFetch = useCallback(async (isInitialLoad = false) => {
    const startTime = Date.now();

    if (isInitialLoad) {
      setIsCheckingDeps(true);
    }

    try {
      if (!isAPIAvailable()) {
        clearLoadingDelay();
        setIsLoading(false);
        setIsCheckingDeps(false);
        return;
      }

      const data = await api.get_status();
      setStatusData(data);

      const now = Date.now();
      await refreshSystemResources(now);
      await refreshNetworkStatus(now);
      await refreshLibraryStatus(now);

      if (isInitialLoad) {
        const elapsedTime = Date.now() - startTime;
        const remainingTime = Math.max(0, 800 - elapsedTime);
        clearLoadingDelay();
        loadingDelayTimeout.current = setTimeout(() => {
          loadingDelayTimeout.current = null;
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
    }
  }, [clearLoadingDelay, refreshLibraryStatus, refreshNetworkStatus, refreshSystemResources]);

  const fetchStatus = useCallback(async (isInitialLoad = false, force = false) => {
    if (inFlightRequest.current) {
      const existingPending = pendingRefresh.current;
      pendingRefresh.current = {
        isInitialLoad: existingPending?.isInitialLoad || isInitialLoad,
        force: existingPending?.force || force,
      };
      await inFlightRequest.current;
      return;
    }

    const request = runStatusFetch(isInitialLoad).finally(() => {
      inFlightRequest.current = null;
      const nextRefresh = pendingRefresh.current;
      pendingRefresh.current = null;
      if (nextRefresh) {
        void fetchStatus(nextRefresh.isInitialLoad, nextRefresh.force);
      }
    });
    inFlightRequest.current = request;
    await request;
  }, [runStatusFetch]);

  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    let waitTimeout: NodeJS.Timeout | null = null;

    const startPolling = () => {
      if (initialLoad) {
        fetchStatus(true).catch((error: unknown) => {
          if (error instanceof APIError) {
            logger.error('Initial status fetch failed', { error: error.message, endpoint: error.endpoint });
          } else if (error instanceof Error) {
            logger.error('Unexpected error during initial fetch', { error: error.message });
          } else {
            logger.error('Unknown error during initial fetch', { error: String(error) });
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
      if (isAPIAvailable()) {
        startPolling();
        return;
      }
      waitTimeout = setTimeout(waitForApi, 100);
    };

    waitForApi();

    return () => {
      if (interval) clearInterval(interval);
      if (waitTimeout) clearTimeout(waitTimeout);
      clearLoadingDelay();
    };
  }, [pollInterval, initialLoad, fetchStatus, clearLoadingDelay]);

  return {
    status: statusData,
    systemResources,
    networkAvailable,
    modelLibraryLoaded,
    isLoading,
    isCheckingDeps,
    refetch: fetchStatus,
  };
}
