/**
 * Version Fetching Hook
 *
 * Manages fetching of installed, active, and available versions with caching.
 * Extracted from hooks/useVersions.ts
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { VersionRelease, VersionStatus, CacheStatus } from '../types/versions';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useVersionFetching');

interface UseVersionFetchingOptions {
  appId?: string;
  enabled?: boolean;
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
  onInstallingTagUpdate,
}: UseVersionFetchingOptions = {}): UseVersionFetchingResult {
  const resolvedAppId = appId ?? 'comfyui';
  const isEnabled = enabled;
  const [installedVersions, setInstalledVersions] = useState<string[]>([]);
  const [activeVersion, setActiveVersion] = useState<string | null>(null);
  const [availableVersions, setAvailableVersions] = useState<VersionRelease[]>([]);
  const [versionStatus, setVersionStatus] = useState<VersionStatus | null>(null);
  const [defaultVersion, setDefaultVersionState] = useState<string | null>(null);
  const [cacheStatus, setCacheStatus] = useState<CacheStatus>({
    has_cache: false,
    is_valid: false,
    is_fetching: false,
  });
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isRateLimited, setIsRateLimited] = useState(false);
  const [rateLimitRetryAfter, setRateLimitRetryAfter] = useState<number | null>(null);

  const followupRefreshRef = useRef<NodeJS.Timeout | null>(null);
  const fetchAvailableVersionsRef = useRef<(forceRefresh?: boolean) => Promise<void>>(() => Promise.resolve());

  useEffect(() => {
    setInstalledVersions([]);
    setActiveVersion(null);
    setAvailableVersions([]);
    setVersionStatus(null);
    setDefaultVersionState(null);
    setCacheStatus({
      has_cache: false,
      is_valid: false,
      is_fetching: false,
    });
    setError(null);
    setIsRateLimited(false);
    setRateLimitRetryAfter(null);
    setIsLoading(isEnabled);
    if (!isEnabled) {
      setIsLoading(false);
    }
  }, [resolvedAppId, isEnabled]);

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

  // Fetch available versions from GitHub
  const fetchAvailableVersions = useCallback(async (forceRefresh: boolean = false) => {
    if (!isAPIAvailable() || !isEnabled) {
      if (!isAPIAvailable()) {
        logger.error('get_available_versions not available');
      }
      return;
    }

    try {
      logger.debug('Fetching available versions', { forceRefresh });
      const result = await api.get_available_versions(forceRefresh, resolvedAppId);
      logger.debug('Available versions result received', { versionsCount: result.versions?.length });
      if (result.success) {
        setAvailableVersions(result.versions || []);
        setIsRateLimited(false);
        setRateLimitRetryAfter(null);
        logger.debug('Set available versions', { count: result.versions?.length });

        // If backend flags an installing release, update local state
        const installingRelease = (result.versions || []).find((r: VersionRelease) => r.installing);
        if (installingRelease?.tagName && onInstallingTagUpdate) {
          onInstallingTagUpdate(installingRelease.tagName);
        }

        // Schedule a follow-up fetch to pick up size data after background calc
        if (forceRefresh) {
          if (followupRefreshRef.current) {
            clearTimeout(followupRefreshRef.current);
          }
          followupRefreshRef.current = setTimeout(() => {
            void fetchAvailableVersionsRef.current(false);
          }, 1500) as any;
        }
      } else if (result.rate_limited) {
        // Handle rate limit gracefully - show cached data if available
        logger.warn('GitHub API rate limited', { retryAfter: result.retry_after_secs });
        setIsRateLimited(true);
        setRateLimitRetryAfter(result.retry_after_secs ?? null);
        // Don't set error - we want to show the rate limit banner instead
      } else {
        logger.error('Failed to fetch available versions', { error: result.error });
        setError(result.error || 'Failed to fetch available versions');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching available versions', { error: error.message, endpoint: error.endpoint });
        setError(error.message);
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching available versions', { error: error.message });
        setError(error.message);
      } else {
        logger.error('Unknown error fetching available versions', { error });
        setError(String(error));
      }
    }
  }, [isEnabled, onInstallingTagUpdate, resolvedAppId]);

  // Update ref to latest function
  fetchAvailableVersionsRef.current = fetchAvailableVersions;

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
      await Promise.all([
        fetchInstalledVersions(),
        fetchActiveVersion(),
        fetchDefaultVersion(),
        fetchAvailableVersions(forceRefresh),
        fetchVersionStatus(),
      ]);
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
  ]);

  // Poll cache status for background GitHub fetches
  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    let waitTimeout: NodeJS.Timeout | null = null;
    const supportsBackgroundFetch = resolvedAppId === 'comfyui';

    if (!isEnabled) {
      return () => {};
    }

    const checkBackgroundFetch = async () => {
      try {
        if (!isAPIAvailable()) return;
        const status = await api.get_github_cache_status(resolvedAppId);
        setCacheStatus(status);

        if (status.is_fetching && !status.has_cache) {
          logger.info('Background GitHub fetch in progress (first-time fetch)');
        } else if (status.is_fetching && status.has_cache) {
          logger.info('Background GitHub fetch in progress (refreshing cache)');
        }

        if (!isAPIAvailable()) return;
        if (
          supportsBackgroundFetch
          && !status.is_fetching
          && status.has_cache
          && await api.should_update_ui_from_background_fetch()
        ) {
          logger.info('Background fetch completed - refreshing UI with new data');
          await api.reset_background_fetch_flag();
          await fetchAvailableVersionsRef.current(false);
        }
      } catch (error) {
        if (error instanceof APIError) {
          logger.error('API error checking background fetch', { error: error.message, endpoint: error.endpoint });
        } else if (error instanceof Error) {
          logger.error('Unexpected error checking background fetch', { error: error.message });
        } else {
          logger.error('Unknown error checking background fetch', { error });
        }
      }
    };

    const waitAndStartPolling = () => {
      if (isAPIAvailable()) {
        logger.debug('Starting cache status polling');
        void checkBackgroundFetch();
        interval = setInterval(checkBackgroundFetch, 2000);
      } else {
        logger.debug('Waiting for API to start cache status polling');
        waitTimeout = setTimeout(waitAndStartPolling, 100);
      }
    };

    waitAndStartPolling();

    return () => {
      if (interval) clearInterval(interval);
      if (waitTimeout) clearTimeout(waitTimeout);
      if (followupRefreshRef.current) clearTimeout(followupRefreshRef.current);
    };
  }, [fetchAvailableVersionsRef, isEnabled, resolvedAppId]);

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
