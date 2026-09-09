import { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import type { CacheStatus, VersionRelease } from '../types/versions';
import { getLogger } from '../utils/logger';

const logger = getLogger('useAvailableVersionState');

interface UseAvailableVersionStateOptions {
  isEnabled: boolean;
  resolvedAppId: string;
  trackAvailableVersions: boolean;
}

export function useAvailableVersionState({
  isEnabled,
  resolvedAppId,
  trackAvailableVersions,
}: UseAvailableVersionStateOptions) {
  const [availableVersions, setAvailableVersions] = useState<VersionRelease[]>([]);
  const [cacheStatus, setCacheStatus] = useState<CacheStatus>({
    has_cache: false,
    is_valid: false,
    is_fetching: false,
  });
  const [isRateLimited, setIsRateLimited] = useState(false);
  const [rateLimitRetryAfter, setRateLimitRetryAfter] = useState<number | null>(null);

  const followupRefreshRef = useRef<NodeJS.Timeout | null>(null);
  const fetchAvailableVersionsRef = useRef<(forceRefresh?: boolean) => Promise<void>>(
    () => Promise.resolve()
  );

  useEffect(() => {
    setAvailableVersions([]);
    setCacheStatus({
      has_cache: false,
      is_valid: false,
      is_fetching: false,
    });
    setIsRateLimited(false);
    setRateLimitRetryAfter(null);
  }, [resolvedAppId, isEnabled, trackAvailableVersions]);

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
      if (result.success) {
        const mapped = result.versions.map((version) => ({
          ...version,
          body: version.body ?? undefined,
          installing: version.installing ?? false,
        }));
        setAvailableVersions(mapped);
        setIsRateLimited(false);
        setRateLimitRetryAfter(null);
        logger.debug('Set available versions', { count: mapped.length });

        if (forceRefresh) {
          if (followupRefreshRef.current) {
            clearTimeout(followupRefreshRef.current);
          }
          followupRefreshRef.current = setTimeout(() => {
            void fetchAvailableVersionsRef.current(false);
          }, 1500) as unknown as NodeJS.Timeout;
        }
      } else {
        logger.warn('GitHub API rate limited', { retryAfter: result.retry_after_secs });
        setIsRateLimited(true);
        setRateLimitRetryAfter(result.retry_after_secs);
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching available versions', {
          error: error.message,
          endpoint: error.endpoint,
        });
        throw error;
      }
      if (error instanceof Error) {
        logger.error('Unexpected error fetching available versions', { error: error.message });
        throw error;
      }
      logger.error('Unknown error fetching available versions', { error });
      throw new APIError(String(error), 'get_available_versions');
    }
  }, [isEnabled, resolvedAppId]);

  fetchAvailableVersionsRef.current = fetchAvailableVersions;

  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    let waitTimeout: NodeJS.Timeout | null = null;

    if (!isEnabled || !trackAvailableVersions) {
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

      } catch (error) {
        if (error instanceof APIError) {
          logger.error('API error checking background fetch', {
            error: error.message,
            endpoint: error.endpoint,
          });
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
  }, [isEnabled, resolvedAppId, trackAvailableVersions]);

  return {
    availableVersions,
    cacheStatus,
    fetchAvailableVersions,
    isRateLimited,
    rateLimitRetryAfter,
  };
}
