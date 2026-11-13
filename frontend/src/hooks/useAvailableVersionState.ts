import { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import type { CacheStatus, VersionRelease } from '../types/versions';
import { getLogger } from '../utils/logger';
import { useCurrentAppGuard } from './useCurrentAppGuard';

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
  const isCurrentApp = useCurrentAppGuard(resolvedAppId);

  const followupRefreshRef = useRef<NodeJS.Timeout | null>(null);
  // Set when the listing was served from an expired cache, which the backend
  // refreshes in the background; cleared once the refreshed listing loads.
  const listingMayBeStaleRef = useRef(false);
  const fetchAvailableVersionsRef = useRef<(forceRefresh?: boolean) => Promise<void>>(
    () => Promise.resolve()
  );
  // Listing requests in flight. A listing that reports builds still being
  // checked asks again only when no other request is already on its way.
  const listingsInFlightRef = useRef(0);

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

    listingsInFlightRef.current += 1;
    try {
      logger.debug('Fetching available versions', { forceRefresh });
      // Read before listing: an expired cache here means the listing may be
      // the stale one the backend serves while it refreshes.
      const before = forceRefresh
        ? null
        : await api.get_github_cache_status(resolvedAppId).catch(() => null);
      const result = await api.get_available_versions(forceRefresh, resolvedAppId);
      // A listing for a previously selected app never replaces this one.
      if (!isCurrentApp(resolvedAppId)) return;
      if (result.success) {
        listingMayBeStaleRef.current = before !== null && !before.is_valid;
        const mapped = result.versions.map((version) => ({
          ...version,
          body: version.body ?? undefined,
          installing: version.installing ?? false,
        }));
        setAvailableVersions(mapped);
        setIsRateLimited(false);
        setRateLimitRetryAfter(null);
        logger.debug('Set available versions', { count: mapped.length });

        // The backend says some builds are still being checked: ask again.
        // The backend paces this, since a listing waits for running checks.
        if (result.availability_pending && listingsInFlightRef.current === 1) {
          void fetchAvailableVersionsRef.current(false).catch(() => {
            // fetchAvailableVersions logs its own failures.
          });
        }

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
    } finally {
      listingsInFlightRef.current -= 1;
    }
  }, [isCurrentApp, isEnabled, resolvedAppId]);

  fetchAvailableVersionsRef.current = fetchAvailableVersions;

  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    let waitTimeout: NodeJS.Timeout | null = null;
    let stopped = false;

    if (!isEnabled || !trackAvailableVersions) {
      return () => {};
    }

    const checkBackgroundFetch = async () => {
      try {
        if (!isAPIAvailable()) return;
        const status = await api.get_github_cache_status(resolvedAppId);
        if (stopped) return;
        setCacheStatus(status);
        // Reload a stale listing once its background refresh has completed,
        // however briefly the refresh was visible to this poll.
        if (listingMayBeStaleRef.current && status.is_valid && !status.is_fetching) {
          listingMayBeStaleRef.current = false;
          void fetchAvailableVersionsRef.current(false).catch(() => {
            // fetchAvailableVersions logs its own failures.
          });
        }

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
      stopped = true;
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
