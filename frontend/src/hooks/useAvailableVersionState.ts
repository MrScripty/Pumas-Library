import { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import type { VersionReleaseInfo } from '../types/api';
import type { CacheStatus, VersionRelease } from '../types/versions';
import { getLogger } from '../utils/logger';

const logger = getLogger('useAvailableVersionState');

type RawVersionRelease = Partial<VersionReleaseInfo> & Partial<VersionRelease> & Record<string, unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function readString(record: RawVersionRelease, ...keys: string[]): string | undefined {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === 'string') {
      return value;
    }
  }
  return undefined;
}

function readBoolean(record: RawVersionRelease, defaultValue: boolean, ...keys: string[]): boolean {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === 'boolean') {
      return value;
    }
  }
  return defaultValue;
}

function readNumber(record: RawVersionRelease, ...keys: string[]): number | null | undefined {
  for (const key of keys) {
    const value = record[key];
    if (value === null) {
      return null;
    }
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
  }
  return undefined;
}

export function normalizeVersionReleaseInfo(version: unknown): VersionRelease | null {
  if (!isRecord(version)) {
    return null;
  }

  const record = version as RawVersionRelease;
  const tagName = readString(record, 'tag_name', 'tagName')?.trim();
  if (!tagName) {
    return null;
  }

  return {
    tagName,
    name: readString(record, 'name') || tagName,
    publishedAt: readString(record, 'published_at', 'publishedAt') || '',
    prerelease: readBoolean(record, false, 'prerelease'),
    body: readString(record, 'body'),
    htmlUrl: readString(record, 'html_url', 'htmlUrl'),
    totalSize: readNumber(record, 'total_size', 'totalSize'),
    archiveSize: readNumber(record, 'archive_size', 'archiveSize'),
    dependenciesSize: readNumber(record, 'dependencies_size', 'dependenciesSize'),
    installing: readBoolean(record, false, 'installing'),
  };
}

interface UseAvailableVersionStateOptions {
  isEnabled: boolean;
  onInstallingTagUpdate?: (tag: string | null) => void;
  resolvedAppId: string;
  trackAvailableVersions: boolean;
}

export function useAvailableVersionState({
  isEnabled,
  onInstallingTagUpdate,
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
      logger.debug('Available versions result received', {
        versionsCount: result.versions.length,
      });

      if (result.success) {
        const mapped = result.versions
          .map(normalizeVersionReleaseInfo)
          .filter((release): release is VersionRelease => release !== null);
        const skippedVersions = result.versions.length - mapped.length;
        if (skippedVersions > 0) {
          logger.warn('Dropped version releases with missing tag names', {
            appId: resolvedAppId,
            skippedVersions,
          });
        }
        setAvailableVersions(mapped);
        setIsRateLimited(false);
        setRateLimitRetryAfter(null);
        logger.debug('Set available versions', { count: mapped.length });

        const installingRelease = mapped.find((release) => release.installing);
        if (installingRelease?.tagName && onInstallingTagUpdate) {
          onInstallingTagUpdate(installingRelease.tagName);
        }

        if (forceRefresh) {
          if (followupRefreshRef.current) {
            clearTimeout(followupRefreshRef.current);
          }
          followupRefreshRef.current = setTimeout(() => {
            void fetchAvailableVersionsRef.current(false);
          }, 1500) as unknown as NodeJS.Timeout;
        }
      } else if (result.rate_limited) {
        logger.warn('GitHub API rate limited', { retryAfter: result.retry_after_secs });
        setIsRateLimited(true);
        setRateLimitRetryAfter(result.retry_after_secs ?? null);
      } else {
        logger.error('Failed to fetch available versions', { error: result.error });
        throw new APIError(
          result.error || 'Failed to fetch available versions',
          'get_available_versions'
        );
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
  }, [isEnabled, onInstallingTagUpdate, resolvedAppId]);

  fetchAvailableVersionsRef.current = fetchAvailableVersions;

  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    let waitTimeout: NodeJS.Timeout | null = null;
    const supportsBackgroundFetch = resolvedAppId === 'comfyui';

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
