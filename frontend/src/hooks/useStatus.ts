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
  const { pollInterval = 500, initialLoad = true } = options;

  const [statusData, setStatusData] = useState<StatusResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isCheckingDeps, setIsCheckingDeps] = useState(true);
  const [systemResources, setSystemResources] = useState<SystemResources | undefined>(undefined);
  const [networkAvailable, setNetworkAvailable] = useState<boolean | null>(null);
  const [modelLibraryLoaded, setModelLibraryLoaded] = useState<boolean | null>(null);
  const isPolling = useRef(false);
  const lastResourcesFetch = useRef(0);
  const lastNetworkFetch = useRef(0);
  const lastLibraryFetch = useRef(0);

  const fetchStatus = useCallback(async (isInitialLoad = false, force = false) => {
    // Allow force=true to bypass the polling guard for manual refreshes
    if (isPolling.current && !force) {
      return;
    }

    const startTime = Date.now();

    if (isInitialLoad) {
      setIsCheckingDeps(true);
    }

    isPolling.current = true;

    try {
      if (!isAPIAvailable()) {
        setIsLoading(false);
        setIsCheckingDeps(false);
        return;
      }

      const data = await api.get_status();
      setStatusData(data);

      const now = Date.now();
      if (now - lastResourcesFetch.current >= 2000) {
        const resourcesResult = await api.get_system_resources();
        if (resourcesResult?.success) {
          setSystemResources(resourcesResult.resources);
        }
        lastResourcesFetch.current = now;
      }

      if (now - lastNetworkFetch.current >= 5000) {
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
      }

      if (now - lastLibraryFetch.current >= 5000) {
        try {
          const libraryResult = await api.get_library_status();
          if (libraryResult?.success) {
            setModelLibraryLoaded(true);
          } else {
            setModelLibraryLoaded(false);
          }
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          logger.debug('Failed to fetch model library status', { error: message });
          setModelLibraryLoaded(false);
        }
        lastLibraryFetch.current = now;
      }

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
    };
  }, [pollInterval, initialLoad, fetchStatus]);

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
