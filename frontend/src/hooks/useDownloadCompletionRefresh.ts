import { useEffect, useRef } from 'react';
import type { DownloadStatus } from './modelDownloadState';
import { getLogger } from '../utils/logger';

const logger = getLogger('useDownloadCompletionRefresh');
const REFRESH_ON_DISAPPEAR_STATUSES = new Set(['queued', 'downloading', 'pausing']);

type UseDownloadCompletionRefreshOptions = {
  delayMs?: number;
  downloadStatusByRepo: Record<string, DownloadStatus>;
  onModelsImported?: () => void;
};

export function useDownloadCompletionRefresh({
  delayMs = 1000,
  downloadStatusByRepo,
  onModelsImported,
}: UseDownloadCompletionRefreshOptions) {
  const previousDownloadStatusRef = useRef<Record<string, string>>({});
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const previousStatuses = previousDownloadStatusRef.current;
    let shouldRefresh = false;

    for (const [downloadKey, status] of Object.entries(downloadStatusByRepo)) {
      if (
        status.status === 'completed'
        && previousStatuses[downloadKey]
        && previousStatuses[downloadKey] !== 'completed'
      ) {
        shouldRefresh = true;
        logger.info('Download completed, will refresh model list', { downloadKey, repoId: status.repoId });
      }
    }

    for (const [downloadKey, previousStatus] of Object.entries(previousStatuses)) {
      if (
        !downloadStatusByRepo[downloadKey]
        && REFRESH_ON_DISAPPEAR_STATUSES.has(previousStatus)
      ) {
        shouldRefresh = true;
        logger.info('Download left tracked state, will refresh model list', {
          downloadKey,
          previousStatus,
        });
      }
    }

    previousDownloadStatusRef.current = Object.fromEntries(
      Object.entries(downloadStatusByRepo).map(([downloadKey, status]) => [downloadKey, status.status])
    );

    if (shouldRefresh) {
      if (refreshTimerRef.current) {
        clearTimeout(refreshTimerRef.current);
      }
      refreshTimerRef.current = setTimeout(() => {
        refreshTimerRef.current = null;
        onModelsImported?.();
      }, delayMs);
    }
  }, [delayMs, downloadStatusByRepo, onModelsImported]);

  useEffect(() => {
    return () => {
      if (refreshTimerRef.current) {
        clearTimeout(refreshTimerRef.current);
      }
    };
  }, []);
}
