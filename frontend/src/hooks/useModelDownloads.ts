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
const TRACKED_STATUSES = ['queued', 'downloading', 'pausing', 'paused', 'cancelling', 'error'] as const;
const STATUS_PRIORITY: Record<DownloadStatus['status'], number> = {
  downloading: 0,
  pausing: 1,
  cancelling: 2,
  queued: 3,
  paused: 4,
  error: 5,
  completed: 99,
  cancelled: 99,
};

function isActiveStatus(status: string): boolean {
  return (ACTIVE_STATUSES as readonly string[]).includes(status);
}

function isTrackedStatus(status: string): status is DownloadStatus['status'] {
  return (TRACKED_STATUSES as readonly string[]).includes(status);
}

interface RepoDownloadSelection {
  status: DownloadStatus;
  error?: string;
}

function shouldReplaceSelection(current: DownloadStatus, candidate: DownloadStatus): boolean {
  const currentPriority = STATUS_PRIORITY[current.status] ?? 999;
  const candidatePriority = STATUS_PRIORITY[candidate.status] ?? 999;
  if (candidatePriority !== currentPriority) return candidatePriority < currentPriority;

  const currentBytes = current.downloadedBytes ?? 0;
  const candidateBytes = candidate.downloadedBytes ?? 0;
  if (candidateBytes !== currentBytes) return candidateBytes > currentBytes;

  const currentProgress = current.progress ?? 0;
  const candidateProgress = candidate.progress ?? 0;
  return candidateProgress > currentProgress;
}

function selectDownloadsByRepo(downloads: Array<{
  repoId?: string;
  downloadId?: string;
  status?: string;
  progress?: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: number;
  etaSeconds?: number;
  modelName?: string;
  modelType?: string;
  error?: string;
}>): {
  statuses: Record<string, DownloadStatus>;
  errors: Record<string, string>;
} {
  const selected: Record<string, RepoDownloadSelection> = {};

  for (const dl of downloads) {
    const repoId = dl.repoId;
    const status = dl.status;
    if (!repoId || !status || !isTrackedStatus(status) || !dl.downloadId) continue;

    const candidate: DownloadStatus = {
      downloadId: dl.downloadId,
      status,
      progress: typeof dl.progress === 'number' ? dl.progress : 0,
      downloadedBytes: typeof dl.downloadedBytes === 'number' ? dl.downloadedBytes : undefined,
      totalBytes: typeof dl.totalBytes === 'number' ? dl.totalBytes : undefined,
      speed: typeof dl.speed === 'number' ? dl.speed : undefined,
      etaSeconds: typeof dl.etaSeconds === 'number' ? dl.etaSeconds : undefined,
      modelName: dl.modelName,
      modelType: dl.modelType,
    };

    const current = selected[repoId]?.status;
    if (!current || shouldReplaceSelection(current, candidate)) {
      selected[repoId] = {
        status: candidate,
        error: dl.error,
      };
    }
  }

  const statuses: Record<string, DownloadStatus> = {};
  const errors: Record<string, string> = {};
  for (const [repoId, selectedDownload] of Object.entries(selected)) {
    statuses[repoId] = selectedDownload.status;
    if (selectedDownload.status.status === 'error' && selectedDownload.error) {
      errors[repoId] = selectedDownload.error;
    }
  }
  return { statuses, errors };
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
        if (!result.success) return;
        const { statuses, errors } = selectDownloadsByRepo(result.downloads || []);
        setDownloadStatusByRepo((prev) => ({ ...statuses, ...prev }));
        if (Object.keys(errors).length > 0) {
          setDownloadErrors((prev) => ({ ...prev, ...errors }));
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
      if (!isAPIAvailable()) return;
      if (!hasActiveRef.current && Object.keys(downloadStatusRef.current).length === 0) return;

      try {
        const result = await api.list_model_downloads();
        if (!result.success) return;

        const { statuses, errors } = selectDownloadsByRepo(result.downloads || []);
        setDownloadStatusByRepo(statuses);

        setDownloadErrors((prev) => {
          const next = { ...prev };
          for (const repoId of Object.keys(statuses)) {
            if (errors[repoId]) {
              next[repoId] = errors[repoId];
            } else if (next[repoId]) {
              delete next[repoId];
            }
          }
          return next;
        });
      } catch (error) {
        logger.warn('Transient error fetching download list', { error: error instanceof Error ? error.message : error });
      }
    }, 800);

    return () => window.clearInterval(intervalId);
  }, []); // Empty deps -- interval is stable for component lifetime

  const startDownload = useCallback((
    repoId: string,
    downloadId: string,
    details?: { modelName?: string; modelType?: string }
  ) => {
    setDownloadStatusByRepo((prev) => {
      const existing = prev[repoId];
      if (existing && isActiveStatus(existing.status)) {
        return prev;
      }
      return {
        ...prev,
        [repoId]: {
          downloadId,
          status: 'queued',
          progress: 0,
          modelName: details?.modelName,
          modelType: details?.modelType,
        },
      };
    });
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
      const result = await api.resume_model_download(status.downloadId);
      if (!result.success) {
        throw new APIError(result.error || 'Failed to resume download.', 'resume_model_download');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to resume download.';
      setDownloadStatusByRepo((prev) => {
        const existing = prev[repoId];
        if (!existing) return prev;
        return { ...prev, [repoId]: { ...existing, status: 'error' as const } };
      });
      setDownloadErrors((prev) => ({ ...prev, [repoId]: message }));
      logger.error('Failed to resume download', {
        error: message,
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
