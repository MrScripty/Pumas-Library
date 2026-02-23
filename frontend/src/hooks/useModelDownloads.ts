/**
 * Model Downloads Hook
 *
 * Manages model download state and polling.
 * Supports parallel downloads, pause/resume, and startup recovery.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useModelDownloads');

export interface DownloadStatus {
  downloadId: string;
  status: 'queued' | 'downloading' | 'pausing' | 'paused' | 'cancelling' | 'completed' | 'cancelled' | 'error';
  progress: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: number;
  etaSeconds?: number;
  modelName?: string;
  modelType?: string;
}

const ACTIVE_STATUSES = ['queued', 'downloading', 'cancelling', 'pausing'] as const;

function isActiveStatus(status: string): boolean {
  return (ACTIVE_STATUSES as readonly string[]).includes(status);
}

export function useModelDownloads() {
  const [downloadStatusByRepo, setDownloadStatusByRepo] = useState<Record<string, DownloadStatus>>({});
  const [downloadErrors, setDownloadErrors] = useState<Record<string, string>>({});

  // Refs for stable polling (avoids effect teardown/recreation)
  const downloadStatusRef = useRef(downloadStatusByRepo);
  const hasActiveRef = useRef(false);

  // Keep refs in sync with state
  useEffect(() => {
    downloadStatusRef.current = downloadStatusByRepo;
    hasActiveRef.current = Object.values(downloadStatusByRepo).some((s) => isActiveStatus(s.status));
  }, [downloadStatusByRepo]);

  // Startup recovery: restore any active/paused downloads from backend
  useEffect(() => {
    const restoreDownloads = async () => {
      if (!isAPIAvailable()) return;
      try {
        const result = await api.list_model_downloads();
        if (!result.success || !result.downloads?.length) return;
        const restored: Record<string, DownloadStatus> = {};
        for (const dl of result.downloads) {
          const repoId = dl.repoId;
          if (!repoId) continue;
          // Restore any download that isn't terminal (completed/cancelled)
          if (['paused', 'downloading', 'queued', 'error', 'pausing', 'cancelling'].includes(dl.status ?? '')) {
            restored[repoId] = {
              downloadId: dl.downloadId ?? '',
              status: (dl.status ?? 'error') as DownloadStatus['status'],
              progress: dl.progress ?? 0,
              downloadedBytes: dl.downloadedBytes,
              totalBytes: dl.totalBytes,
              speed: dl.speed,
              etaSeconds: dl.etaSeconds,
            };
          }
        }
        if (Object.keys(restored).length > 0) {
          setDownloadStatusByRepo((prev) => ({ ...restored, ...prev }));
        }
      } catch (error) {
        logger.warn('Failed to restore downloads on startup', { error });
      }
    };
    void restoreDownloads();
  }, []);

  // Stable polling interval -- created once, never torn down by state changes
  useEffect(() => {
    const intervalId = window.setInterval(async () => {
      if (!hasActiveRef.current || !isAPIAvailable()) return;

      const entries = Object.entries(downloadStatusRef.current).filter(([, status]) =>
        isActiveStatus(status.status)
      );
      if (entries.length === 0) return;

      const updates = await Promise.all(
        entries.map(async ([repoId, status]) => {
          if (!isAPIAvailable()) {
            return { repoId, status: status.status, error: 'API not available', transient: true as const };
          }
          try {
            const result = await api.get_model_download_status(status.downloadId);
            if (!result.success) {
              return { repoId, status: 'error' as const, error: result.error || 'Download failed.' };
            }
            return {
              repoId,
              status: (result.status || 'downloading') as DownloadStatus['status'],
              progress: typeof result.progress === 'number' ? result.progress : 0,
              downloadedBytes: typeof result.downloadedBytes === 'number' ? result.downloadedBytes : undefined,
              totalBytes: typeof result.totalBytes === 'number' ? result.totalBytes : undefined,
              speed: typeof result.speed === 'number' ? result.speed : undefined,
              etaSeconds: typeof result.etaSeconds === 'number' ? result.etaSeconds : undefined,
              error: result.error,
            };
          } catch (error) {
            const message = error instanceof Error ? error.message : 'Unable to fetch download status.';
            logger.warn('Transient error fetching download status', { error: message, repoId });
            return {
              repoId,
              status: status.status,
              progress: status.progress,
              downloadedBytes: status.downloadedBytes,
              totalBytes: status.totalBytes,
              error: message,
              transient: true as const,
            };
          }
        })
      );

      setDownloadStatusByRepo((prev) => {
        const next = { ...prev };
        updates.forEach((update) => {
          if (!update) return;
          const previous = prev[update.repoId];
          if (!previous) return;
          next[update.repoId] = {
            ...previous,
            status: update.status,
            progress: update.progress ?? previous.progress,
            downloadedBytes: update.downloadedBytes ?? previous.downloadedBytes,
            totalBytes: update.totalBytes ?? previous.totalBytes,
            speed: update.speed,
            etaSeconds: update.etaSeconds,
          };
        });
        return next;
      });

      // Update per-download errors
      setDownloadErrors((prev) => {
        const next = { ...prev };
        let changed = false;
        updates.forEach((update) => {
          if (!update) return;
          const isTransient = 'transient' in update;
          if (isTransient && update.error) {
            if (next[update.repoId] !== update.error) {
              next[update.repoId] = update.error;
              changed = true;
            }
            return;
          }
          if (update.status === 'error' && update.error) {
            if (next[update.repoId] !== update.error) {
              next[update.repoId] = update.error;
              changed = true;
            }
            return;
          }
          // Clear error when download is progressing or finished
          if (next[update.repoId]) {
            delete next[update.repoId];
            changed = true;
          }
        });
        return changed ? next : prev;
      });
    }, 800);

    return () => window.clearInterval(intervalId);
  }, []); // Empty deps -- interval is stable for component lifetime

  const startDownload = useCallback((
    repoId: string,
    downloadId: string,
    details?: { modelName?: string; modelType?: string }
  ) => {
    setDownloadStatusByRepo((prev) => ({
      ...prev,
      [repoId]: {
        downloadId,
        status: 'queued',
        progress: 0,
        modelName: details?.modelName,
        modelType: details?.modelType,
      },
    }));
    setDownloadErrors((prev) => {
      if (!prev[repoId]) return prev;
      const next = { ...prev };
      delete next[repoId];
      return next;
    });
  }, []);

  const cancelDownload = useCallback(async (repoId: string) => {
    const status = downloadStatusRef.current[repoId];
    if (!status || !isAPIAvailable()) return;

    setDownloadStatusByRepo((prev) => ({
      ...prev,
      [repoId]: {
        ...prev[repoId],
        downloadId: prev[repoId]?.downloadId || status.downloadId,
        status: 'cancelling' as const,
        progress: prev[repoId]?.progress || 0,
      },
    }));

    try {
      await api.cancel_model_download(status.downloadId);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error cancelling download', { error: error.message, endpoint: error.endpoint, repoId });
      } else if (error instanceof Error) {
        logger.error('Unexpected error cancelling download', { error: error.message, repoId });
      } else {
        logger.error('Unknown error cancelling download', { error, repoId });
      }
    }
  }, []);

  const pauseDownload = useCallback(async (repoId: string) => {
    const status = downloadStatusRef.current[repoId];
    if (!status || !isAPIAvailable()) return;

    setDownloadStatusByRepo((prev) => {
      const existing = prev[repoId];
      if (!existing) return prev;
      return { ...prev, [repoId]: { ...existing, status: 'pausing' as const } };
    });

    try {
      await api.pause_model_download(status.downloadId);
    } catch (error) {
      logger.error('Failed to pause download', {
        error: error instanceof Error ? error.message : error,
        repoId,
      });
    }
  }, []);

  const resumeDownload = useCallback(async (repoId: string) => {
    const status = downloadStatusRef.current[repoId];
    if (!status || !isAPIAvailable()) return;

    setDownloadStatusByRepo((prev) => {
      const existing = prev[repoId];
      if (!existing) return prev;
      return { ...prev, [repoId]: { ...existing, status: 'queued' as const, speed: undefined, etaSeconds: undefined } };
    });
    setDownloadErrors((prev) => {
      if (!prev[repoId]) return prev;
      const next = { ...prev };
      delete next[repoId];
      return next;
    });

    try {
      await api.resume_model_download(status.downloadId);
    } catch (error) {
      logger.error('Failed to resume download', {
        error: error instanceof Error ? error.message : error,
        repoId,
      });
    }
  }, []);

  const hasActiveDownloads = Object.values(downloadStatusByRepo).some((s) => isActiveStatus(s.status));

  return {
    downloadStatusByRepo,
    downloadErrors,
    hasActiveDownloads,
    startDownload,
    cancelDownload,
    pauseDownload,
    resumeDownload,
    setDownloadErrors,
  };
}
