/**
 * Backend-owned status telemetry hook.
 *
 * Loads the current cached status/resource snapshot and then applies pushed
 * status telemetry updates from Electron.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { api, getElectronAPI, isAPIAvailable } from '../api/adapter';
import type { StatusResponse, StatusTelemetrySnapshot } from '../types/api';
import type { SystemResources } from '../types/apps';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useStatus');

interface UseStatusOptions {
  initialLoad?: boolean;
}

export function useStatus(options: UseStatusOptions = {}) {
  const { initialLoad = true } = options;

  const [statusData, setStatusData] = useState<StatusResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isCheckingDeps, setIsCheckingDeps] = useState(true);
  const [systemResources, setSystemResources] = useState<SystemResources | undefined>(undefined);
  const [networkAvailable, setNetworkAvailable] = useState<boolean | null>(null);
  const [modelLibraryLoaded, setModelLibraryLoaded] = useState<boolean | null>(null);
  const inFlightRequest = useRef<Promise<void> | null>(null);
  const pendingRefresh = useRef<{ isInitialLoad: boolean; force: boolean } | null>(null);
  const loadingDelayTimeout = useRef<NodeJS.Timeout | null>(null);

  const clearLoadingDelay = useCallback(() => {
    if (loadingDelayTimeout.current) {
      clearTimeout(loadingDelayTimeout.current);
      loadingDelayTimeout.current = null;
    }
  }, []);

  const applySnapshot = useCallback((snapshot: StatusTelemetrySnapshot) => {
    setStatusData(snapshot.status);
    setSystemResources(snapshot.resources);
    setNetworkAvailable(!snapshot.network.is_offline);
    setModelLibraryLoaded(snapshot.model_library_loaded && snapshot.library.success);
  }, []);

  const finishInitialLoad = useCallback((startedAt: number) => {
    const elapsedTime = Date.now() - startedAt;
    const remainingTime = Math.max(0, 800 - elapsedTime);
    clearLoadingDelay();
    loadingDelayTimeout.current = setTimeout(() => {
      loadingDelayTimeout.current = null;
      setIsLoading(false);
      setIsCheckingDeps(false);
    }, remainingTime);
  }, [clearLoadingDelay]);

  const runStatusFetch = useCallback(async (isInitialLoad = false) => {
    const startedAt = Date.now();

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

      const snapshot = await api.get_status_telemetry_snapshot();
      applySnapshot(snapshot);

      if (isInitialLoad) {
        finishInitialLoad(startedAt);
      } else {
        setIsLoading(false);
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching status telemetry', {
          error: error.message,
          endpoint: error.endpoint,
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching status telemetry', { error: error.message });
      } else {
        logger.error('Unknown error fetching status telemetry', { error });
      }
      setIsLoading(false);
      setIsCheckingDeps(false);
    }
  }, [applySnapshot, clearLoadingDelay, finishInitialLoad]);

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
    let waitTimeout: NodeJS.Timeout | null = null;
    let unsubscribeTelemetry: (() => void) | null = null;

    const startTelemetry = () => {
      if (initialLoad) {
        fetchStatus(true).catch((error: unknown) => {
          if (error instanceof APIError) {
            logger.error('Initial status telemetry fetch failed', {
              error: error.message,
              endpoint: error.endpoint,
            });
          } else if (error instanceof Error) {
            logger.error('Unexpected error during initial telemetry fetch', {
              error: error.message,
            });
          } else {
            logger.error('Unknown error during initial telemetry fetch', { error: String(error) });
          }
          setIsLoading(false);
          setIsCheckingDeps(false);
        });
      }

      const electronAPI = getElectronAPI();
      if (electronAPI?.onStatusTelemetryUpdate) {
        unsubscribeTelemetry = electronAPI.onStatusTelemetryUpdate((notification) => {
          applySnapshot(notification.snapshot);
          setIsLoading(false);
          setIsCheckingDeps(false);
        });
      }
    };

    const waitForApi = () => {
      if (isAPIAvailable()) {
        startTelemetry();
        return;
      }
      waitTimeout = setTimeout(waitForApi, 100);
    };

    waitForApi();

    return () => {
      if (waitTimeout) clearTimeout(waitTimeout);
      if (unsubscribeTelemetry) unsubscribeTelemetry();
      clearLoadingDelay();
    };
  }, [initialLoad, fetchStatus, applySnapshot, clearLoadingDelay]);

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
