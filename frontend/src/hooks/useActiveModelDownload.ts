import { useEffect, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { ModelDownloadStatusResponse } from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('useActiveModelDownload');

const POLL_INTERVAL_MS = 1000;
const ACTIVE_STATUSES = ['queued', 'downloading', 'pausing', 'cancelling'] as const;
type ActiveDownloadStatus = (typeof ACTIVE_STATUSES)[number];

export interface ActiveModelDownload {
  downloadId: string;
  repoId: string | null;
  status: ActiveDownloadStatus;
  progress: number;
  downloadedBytes: number | null;
  totalBytes: number | null;
  speed: number | null;
  etaSeconds: number | null;
}

interface ActiveDownloadStatusResponse extends ModelDownloadStatusResponse {
  status: ActiveDownloadStatus;
}

function isActiveStatus(status: string | undefined): status is ActiveDownloadStatus {
  if (!status) return false;
  return ACTIVE_STATUSES.some((activeStatus) => activeStatus === status);
}

function isActiveDownload(download: ModelDownloadStatusResponse): download is ActiveDownloadStatusResponse {
  return isActiveStatus(download.status);
}

export function useActiveModelDownload() {
  const [activeDownload, setActiveDownload] = useState<ActiveModelDownload | null>(null);
  const [activeDownloadCount, setActiveDownloadCount] = useState(0);

  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      if (!isAPIAvailable()) {
        if (!cancelled) {
          setActiveDownload(null);
          setActiveDownloadCount(0);
        }
        return;
      }

      try {
        const result = await api.list_model_downloads();
        if (!result.success) {
          if (!cancelled) {
            setActiveDownload(null);
            setActiveDownloadCount(0);
          }
          return;
        }

        const activeDownloads = result.downloads.filter(isActiveDownload);

        if (!cancelled) {
          setActiveDownloadCount(activeDownloads.length);
        }

        const active = activeDownloads
          .sort((a, b) => {
            // Prefer real in-flight downloads over queued/transition states.
            const aPriority = a.status === 'downloading' ? 0 : 1;
            const bPriority = b.status === 'downloading' ? 0 : 1;
            if (aPriority !== bPriority) return aPriority - bPriority;

            const aProgress = typeof a.progress === 'number' ? a.progress : 0;
            const bProgress = typeof b.progress === 'number' ? b.progress : 0;
            return bProgress - aProgress;
          })[0];

        if (!active || !active.downloadId) {
          if (!cancelled) setActiveDownload(null);
          return;
        }

        if (!cancelled) {
          setActiveDownload({
            downloadId: active.downloadId,
            repoId: active.repoId ?? null,
            status: active.status,
            progress: typeof active.progress === 'number' ? active.progress : 0,
            downloadedBytes: typeof active.downloadedBytes === 'number' ? active.downloadedBytes : null,
            totalBytes: typeof active.totalBytes === 'number' ? active.totalBytes : null,
            speed: typeof active.speed === 'number' ? active.speed : null,
            etaSeconds: typeof active.etaSeconds === 'number' ? active.etaSeconds : null,
          });
        }
      } catch (error) {
        logger.debug('Failed to poll active model download', { error });
      }
    };

    void poll();
    const intervalId = window.setInterval(() => {
      void poll();
    }, POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, []);

  return { activeDownload, activeDownloadCount };
}
