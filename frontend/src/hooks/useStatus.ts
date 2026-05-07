/**
 * Backend-owned status telemetry hook.
 *
 * Loads the current cached status/resource snapshot and then applies pushed
 * status telemetry updates from Electron.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import type { StatusResponse, StatusTelemetrySnapshot } from '../types/api';
import type { SystemResources } from '../types/apps';
import { useStatusTelemetry } from './statusTelemetryStore';


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
  const initialLoadStartedAt = useRef(Date.now());
  const telemetry = useStatusTelemetry({ loadInitial: initialLoad });

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

    await telemetry.refetch();

    if (isInitialLoad) {
      finishInitialLoad(startedAt);
    } else {
      setIsLoading(false);
    }
  }, [finishInitialLoad, telemetry]);

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
    return () => {
      clearLoadingDelay();
    };
  }, [clearLoadingDelay]);

  useEffect(() => {
    if (!telemetry.snapshot) {
      return;
    }

    applySnapshot(telemetry.snapshot);
    if (initialLoad && isLoading) {
      finishInitialLoad(initialLoadStartedAt.current);
    } else {
      setIsLoading(false);
      setIsCheckingDeps(false);
    }
  }, [applySnapshot, finishInitialLoad, initialLoad, isLoading, telemetry.snapshot]);

  useEffect(() => {
    if (telemetry.error) {
      clearLoadingDelay();
      setIsLoading(false);
      setIsCheckingDeps(false);
    }
  }, [clearLoadingDelay, telemetry.error]);

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
