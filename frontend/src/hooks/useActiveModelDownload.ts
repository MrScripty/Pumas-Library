import { useCallback, useEffect, useState } from 'react';
import { api, getElectronAPI, isAPIAvailable } from '../api/adapter';
import type { ModelDownloadSnapshotEntry } from '../types/api';
import { getLogger } from '../utils/logger';
import { projectDownloadProgress } from '../utils/downloadProgressProjection';

const logger = getLogger('useActiveModelDownload');

const ACTIVE_STATUSES = ['queued', 'downloading', 'pausing', 'cancelling'] as const;
type ActiveDownloadStatus = (typeof ACTIVE_STATUSES)[number];

export interface ActiveModelDownload {
  downloadId: string;
  repoId: string | null;
  status: ActiveDownloadStatus;
  progress: number; // Backend download fraction in [0,1]; zero when unavailable.
  downloadedBytes: number | null;
  totalBytes: number | null;
  speed: number | null;
  etaSeconds: number | null;
}

interface ActiveDownloadStatusResponse extends ModelDownloadSnapshotEntry {
  status: ActiveDownloadStatus;
}

function isActiveStatus(status: string | undefined): status is ActiveDownloadStatus {
  if (!status) return false;
  return ACTIVE_STATUSES.some((activeStatus) => activeStatus === status);
}

function isActiveDownload(download: ModelDownloadSnapshotEntry): download is ActiveDownloadStatusResponse {
  return isActiveStatus(download.status);
}

export function useActiveModelDownload() {
  const [activeDownload, setActiveDownload] = useState<ActiveModelDownload | null>(null);
  const [activeDownloadCount, setActiveDownloadCount] = useState(0);

  const applyDownloads = useCallback((downloads: ModelDownloadSnapshotEntry[]) => {
    const activeDownloads = downloads.filter(isActiveDownload);
    setActiveDownloadCount(activeDownloads.length);

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
    const aggregateSpeed = activeDownloads.reduce((sum, download) => {
      const speed = typeof download.speed === 'number' ? download.speed : 0;
      return sum + Math.max(speed, 0);
    }, 0);

    if (!active || !active.downloadId) {
      setActiveDownload(null);
      return;
    }

    setActiveDownload({
      downloadId: active.downloadId,
      repoId: active.repoId ?? null,
      status: active.status,
      progress: typeof active.progress === 'number' ? active.progress : 0,
      downloadedBytes: typeof active.downloadedBytes === 'number' ? active.downloadedBytes : null,
      totalBytes: typeof active.totalBytes === 'number' ? active.totalBytes : null,
      speed: aggregateSpeed > 0 ? aggregateSpeed : null,
      etaSeconds: typeof active.etaSeconds === 'number' ? active.etaSeconds : null,
    });
  }, []);

  useEffect(() => {
    let cancelled = false;
    // The startup list has no revision. Once the subscription supplies a full
    // snapshot, that source owns presentation for this mounted effect.
    let startupSuperseded = false;

    const loadSnapshot = async () => {
      if (!isAPIAvailable()) {
        if (!cancelled) {
          setActiveDownload(null);
          setActiveDownloadCount(0);
        }
        return;
      }

      try {
        const result = await api.list_model_downloads();
        if (cancelled) {
          return;
        }
        if (startupSuperseded) {
          logger.debug('Startup download snapshot superseded by subscribed snapshot');
          return;
        }

        applyDownloads(result.downloads.map(projectDownloadProgress));
      } catch (error) {
        logger.debug('Failed to load active model download snapshot', { error });
      }
    };

    void loadSnapshot();

    const unsubscribe = getElectronAPI()?.onModelDownloadUpdate((notification) => {
      if (!cancelled) {
        startupSuperseded = true;
        applyDownloads(notification.snapshot.downloads.map(projectDownloadProgress));
      }
    });

    return () => {
      cancelled = true;
      unsubscribe?.();
    };
  }, [applyDownloads]);

  return { activeDownload, activeDownloadCount };
}
