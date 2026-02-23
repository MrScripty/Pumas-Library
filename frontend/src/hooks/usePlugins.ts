/**
 * Hook for accessing plugin configurations.
 *
 * Fetches plugin configs from the backend and provides
 * utilities for working with plugins.
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { PluginConfig } from '../types/plugins';
import { getLogger } from '../utils/logger';

const logger = getLogger('usePlugins');

interface UsePluginsResult {
  /** All loaded plugins */
  plugins: PluginConfig[];
  /** Enabled plugins sorted by sidebar priority */
  enabledPlugins: PluginConfig[];
  /** Get a specific plugin by ID */
  getPlugin: (id: string) => PluginConfig | undefined;
  /** Check if a plugin exists */
  hasPlugin: (id: string) => boolean;
  /** Loading state */
  isLoading: boolean;
  /** Error state */
  error: string | null;
  /** Reload plugins from backend */
  reload: () => Promise<void>;
}

/**
 * Hook for managing plugin configurations.
 *
 * Loads plugins from the backend on mount and provides
 * access to plugin data and utilities.
 */
export function usePlugins(): UsePluginsResult {
  const [plugins, setPlugins] = useState<PluginConfig[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchPlugins = useCallback(async () => {
    if (!isAPIAvailable()) {
      setIsLoading(false);
      return;
    }

    try {
      setIsLoading(true);
      setError(null);

      // Call the backend to get plugins
      // This assumes the API has a get_plugins method
      const result = await api.get_plugins();

      if (result.success && result.plugins) {
        setPlugins(result.plugins as PluginConfig[]);
        logger.debug('Loaded plugins', { count: result.plugins.length });
      } else {
        setError(result.error || 'Failed to load plugins');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      logger.error('Failed to fetch plugins', { error: message });
      setError(message);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchPlugins();
  }, [fetchPlugins]);

  const enabledPlugins = useMemo(() => {
    return plugins
      .filter((p) => p.enabledByDefault)
      .sort((a, b) => a.sidebarPriority - b.sidebarPriority);
  }, [plugins]);

  const getPlugin = useCallback(
    (id: string) => {
      return plugins.find((p) => p.id === id);
    },
    [plugins]
  );

  const hasPlugin = useCallback(
    (id: string) => {
      return plugins.some((p) => p.id === id);
    },
    [plugins]
  );

  return {
    plugins,
    enabledPlugins,
    getPlugin,
    hasPlugin,
    isLoading,
    error,
    reload: fetchPlugins,
  };
}

/**
 * Hook for accessing a single plugin's API.
 *
 * Provides methods for calling plugin-defined endpoints.
 */
export function usePluginApi(appId: string) {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const callEndpoint = useCallback(
    async (
      endpointName: string,
      params: Record<string, string> = {}
    ): Promise<unknown> => {
      if (!isAPIAvailable()) {
        throw new Error('API not available');
      }

      setIsLoading(true);
      setError(null);

      try {
        const result = await api.call_plugin_endpoint(appId, endpointName, params);
        if (!result.success) {
          throw new Error(result.error || 'Endpoint call failed');
        }
        return result.data;
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        setError(message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [appId]
  );

  const getStats = useCallback(async () => {
    return callEndpoint('stats');
  }, [callEndpoint]);

  const listModels = useCallback(async () => {
    return callEndpoint('listModels');
  }, [callEndpoint]);

  const loadModel = useCallback(
    async (modelName: string) => {
      return callEndpoint('loadModel', { model_name: modelName });
    },
    [callEndpoint]
  );

  const unloadModel = useCallback(
    async (modelName: string) => {
      return callEndpoint('unloadModel', { model_name: modelName });
    },
    [callEndpoint]
  );

  const checkHealth = useCallback(async (): Promise<boolean> => {
    if (!isAPIAvailable()) return false;

    try {
      const result = await api.check_plugin_health(appId);
      return result.success && result.healthy;
    } catch {
      return false;
    }
  }, [appId]);

  return {
    callEndpoint,
    getStats,
    listModels,
    loadModel,
    unloadModel,
    checkHealth,
    isLoading,
    error,
  };
}

/**
 * Hook for managing plugin process lifecycle.
 */
export function usePluginProcess(appId: string) {
  const [isRunning, setIsRunning] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isStopping, setIsStopping] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const checkStatus = useCallback(async () => {
    if (!isAPIAvailable()) return;

    try {
      const result = await api.get_app_status(appId);
      if (result.success) {
        setIsRunning(result.running ?? false);
      }
    } catch (err) {
      logger.debug('Failed to check app status', { appId, error: err });
    }
  }, [appId]);

  useEffect(() => {
    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, [checkStatus]);

  const launch = useCallback(
    async (versionTag: string) => {
      if (!isAPIAvailable()) return;

      setIsStarting(true);
      setError(null);

      try {
        const result = await api.launch_app(appId, versionTag);
        if (result.success) {
          setIsRunning(true);
        } else {
          setError(result.error || 'Failed to launch');
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Unknown error';
        setError(message);
      } finally {
        setIsStarting(false);
      }
    },
    [appId]
  );

  const stop = useCallback(async () => {
    if (!isAPIAvailable()) return;

    setIsStopping(true);
    setError(null);

    try {
      const result = await api.stop_app(appId);
      if (result.success) {
        setIsRunning(false);
      } else {
        setError(result.error || 'Failed to stop');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      setError(message);
    } finally {
      setIsStopping(false);
    }
  }, [appId]);

  return {
    isRunning,
    isStarting,
    isStopping,
    error,
    launch,
    stop,
    checkStatus,
  };
}
