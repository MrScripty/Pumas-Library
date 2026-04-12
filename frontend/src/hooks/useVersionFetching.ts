/**
 * Version Fetching Hook
 *
 * Manages fetching of installed, active, and available versions with caching.
 * Extracted from hooks/useVersions.ts
 */

import { useState, useCallback, useEffect } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { VersionRelease, VersionStatus, CacheStatus } from '../types/versions';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';
import { useAvailableVersionState } from './useAvailableVersionState';

const logger = getLogger('useVersionFetching');

interface UseVersionFetchingOptions {
  appId?: string;
  enabled?: boolean;
  trackAvailableVersions?: boolean;
  onInstallingTagUpdate?: (tag: string | null) => void;
}

interface UseVersionFetchingResult {
  installedVersions: string[];
  activeVersion: string | null;
  availableVersions: VersionRelease[];
  versionStatus: VersionStatus | null;
  defaultVersion: string | null;
  cacheStatus: CacheStatus;
  isLoading: boolean;
  error: string | null;
  /** True when GitHub API rate limit was hit */
  isRateLimited: boolean;
  /** Seconds until rate limit resets (if known) */
  rateLimitRetryAfter: number | null;
  fetchInstalledVersions: () => Promise<void>;
  fetchActiveVersion: () => Promise<void>;
  fetchAvailableVersions: (forceRefresh?: boolean) => Promise<void>;
  fetchVersionStatus: () => Promise<void>;
  fetchDefaultVersion: () => Promise<void>;
  refreshAll: (forceRefresh?: boolean) => Promise<void>;
  setDefaultVersion: (tag: string | null) => Promise<void>;
}

export function useVersionFetching({
  appId,
  enabled = true,
  trackAvailableVersions = true,
  onInstallingTagUpdate,
}: UseVersionFetchingOptions = {}): UseVersionFetchingResult {
  const resolvedAppId = appId ?? 'comfyui';
  const isEnabled = enabled;
  const [installedVersions, setInstalledVersions] = useState<string[]>([]);
  const [activeVersion, setActiveVersion] = useState<string | null>(null);
  const [versionStatus, setVersionStatus] = useState<VersionStatus | null>(null);
  const [defaultVersion, setDefaultVersionState] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const {
    availableVersions,
    cacheStatus,
    fetchAvailableVersions,
    isRateLimited,
    rateLimitRetryAfter,
  } = useAvailableVersionState({
    isEnabled,
    onInstallingTagUpdate,
    resolvedAppId,
    trackAvailableVersions,
  });

  useEffect(() => {
    setInstalledVersions([]);
    setActiveVersion(null);
    setVersionStatus(null);
    setDefaultVersionState(null);
    setError(null);
    setIsLoading(isEnabled);
    if (!isEnabled) {
      setIsLoading(false);
    }
  }, [resolvedAppId, isEnabled, trackAvailableVersions]);

  // Fetch installed versions
  const fetchInstalledVersions = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      return;
    }

    try {
      const result = await api.get_installed_versions(resolvedAppId);
      if (result.success) {
        setInstalledVersions(result.versions || []);
      } else {
        setError(result.error || 'Failed to fetch installed versions');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching installed versions', { error: error.message, endpoint: error.endpoint });
        setError(error.message);
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching installed versions', { error: error.message });
        setError(error.message);
      } else {
        logger.error('Unknown error fetching installed versions', { error });
        setError(String(error));
      }
    }
  }, [isEnabled, resolvedAppId]);

  // Fetch active version
  const fetchActiveVersion = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      return;
    }

    try {
      const result = await api.get_active_version(resolvedAppId);
      if (result.success) {
        setActiveVersion(result.version || null);
      } else {
        setError(result.error || 'Failed to fetch active version');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching active version', { error: error.message, endpoint: error.endpoint });
        setError(error.message);
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching active version', { error: error.message });
        setError(error.message);
      } else {
        logger.error('Unknown error fetching active version', { error });
        setError(String(error));
      }
    }
  }, [isEnabled, resolvedAppId]);

  // Fetch default version
  const fetchDefaultVersion = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      return;
    }

    try {
      const result = await api.get_default_version(resolvedAppId);
      if (result.success) {
        setDefaultVersionState(result.version || null);
      }
    } catch (error) {
      // non-fatal - log but don't set error state
      if (error instanceof APIError) {
        logger.warn('API error fetching default version', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.warn('Unexpected error fetching default version', { error: error.message });
      } else {
        logger.warn('Unknown error fetching default version', { error });
      }
    }
  }, [isEnabled, resolvedAppId]);

  // Fetch comprehensive version status
  const fetchVersionStatus = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      return;
    }

    try {
      const result = await api.get_version_status(resolvedAppId);
      if (result.success) {
        setVersionStatus(result.status || null);
        if (result.status?.defaultVersion !== undefined) {
          setDefaultVersionState(result.status.defaultVersion || null);
        }
      } else {
        setError(result.error || 'Failed to fetch version status');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching version status', { error: error.message, endpoint: error.endpoint });
        setError(error.message);
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching version status', { error: error.message });
        setError(error.message);
      } else {
        logger.error('Unknown error fetching version status', { error });
        setError(String(error));
      }
    }
  }, [isEnabled, resolvedAppId]);

  // Set default version
  const setDefaultVersion = useCallback(async (tag: string | null) => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'set_default_version');
    }

    try {
      const result = await api.set_default_version(tag, resolvedAppId);
      if (result.success) {
        setDefaultVersionState(tag);
        await fetchVersionStatus();
      } else {
        throw new APIError(result.error || 'Failed to set default version', 'set_default_version');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error setting default version', { error: error.message, endpoint: error.endpoint, tag });
        setError(error.message);
      } else if (error instanceof Error) {
        logger.error('Unexpected error setting default version', { error: error.message, tag });
        setError(error.message);
      } else {
        logger.error('Unknown error setting default version', { error, tag });
        setError(String(error));
      }
      throw error;
    }
  }, [fetchVersionStatus, isEnabled, resolvedAppId]);

  // Refresh all version data
  const refreshAll = useCallback(async (forceRefresh: boolean = false) => {
    if (!isEnabled) {
      setIsLoading(false);
      return;
    }
    setIsLoading(true);
    try {
      const requests = [
        fetchInstalledVersions(),
        fetchActiveVersion(),
        fetchDefaultVersion(),
        fetchVersionStatus(),
      ];

      if (trackAvailableVersions) {
        requests.push(
          fetchAvailableVersions(forceRefresh).catch((fetchError: unknown) => {
            if (fetchError instanceof Error) {
              setError(fetchError.message);
            } else {
              setError(String(fetchError));
            }
          })
        );
      }

      await Promise.all(requests);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error refreshing version data', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error refreshing version data', { error: error.message });
      } else {
        logger.error('Unknown error refreshing version data', { error });
      }
    } finally {
      setIsLoading(false);
    }
  }, [
    fetchInstalledVersions,
    fetchActiveVersion,
    fetchDefaultVersion,
    fetchAvailableVersions,
    fetchVersionStatus,
    isEnabled,
    trackAvailableVersions,
  ]);

  return {
    installedVersions,
    activeVersion,
    availableVersions,
    versionStatus,
    defaultVersion,
    cacheStatus,
    isLoading,
    error,
    isRateLimited,
    rateLimitRetryAfter,
    fetchInstalledVersions,
    fetchActiveVersion,
    fetchAvailableVersions,
    fetchVersionStatus,
    fetchDefaultVersion,
    refreshAll,
    setDefaultVersion,
  };
}
