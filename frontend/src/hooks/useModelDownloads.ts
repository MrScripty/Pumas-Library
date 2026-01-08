/**
 * Model Downloads Hook
 *
 * Manages model download state and polling.
 * Extracted from ModelManager.tsx
 */

import { useState, useEffect, useRef } from 'react';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useModelDownloads');

export interface DownloadStatus {
  downloadId: string;
  status: 'queued' | 'downloading' | 'cancelling' | 'completed' | 'cancelled' | 'error';
  progress: number;
  downloadedBytes?: number;
  totalBytes?: number;
}

export function useModelDownloads() {
  const [downloadStatusByRepo, setDownloadStatusByRepo] = useState<Record<string, DownloadStatus>>({});
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [downloadRepoId, setDownloadRepoId] = useState<string | null>(null);

  const downloadStatusRef = useRef(downloadStatusByRepo);
  const downloadPollingRef = useRef<number | null>(null);

  // Keep ref in sync
  useEffect(() => {
    downloadStatusRef.current = downloadStatusByRepo;
  }, [downloadStatusByRepo]);

  // Poll for download status updates
  useEffect(() => {
    const hasActiveDownloads = Object.values(downloadStatusByRepo).some((status) =>
      ['queued', 'downloading', 'cancelling'].includes(status.status)
    );

    if (!hasActiveDownloads) {
      if (downloadPollingRef.current) {
        window.clearInterval(downloadPollingRef.current);
        downloadPollingRef.current = null;
      }
      return;
    }

    if (downloadPollingRef.current) {
      return;
    }

    downloadPollingRef.current = window.setInterval(async () => {
      const entries = Object.entries(downloadStatusRef.current).filter(([, status]) =>
        ['queued', 'downloading', 'cancelling'].includes(status.status)
      );

      if (!window.pywebview?.api?.get_model_download_status || entries.length === 0) {
        return;
      }

      const updates = await Promise.all(
        entries.map(async ([repoId, status]) => {
          if (!window.pywebview?.api) return { repoId, status: 'error' as const, error: 'API not available' };
          const result = await window.pywebview.api.get_model_download_status(status.downloadId);
          if (!result.success) {
            return { repoId, status: 'error' as const, error: result.error || 'Download failed.' };
          }
          return {
            repoId,
            status: (result.status || 'downloading') as DownloadStatus['status'],
            progress: typeof result.progress === 'number' ? result.progress : 0,
            downloadedBytes: typeof result.downloaded_bytes === 'number' ? result.downloaded_bytes : undefined,
            totalBytes: typeof result.total_bytes === 'number' ? result.total_bytes : undefined,
            error: result.error,
          };
        })
      );

      setDownloadStatusByRepo((prev) => {
        const next = { ...prev };
        updates.forEach((update) => {
          if (!update) {
            return;
          }
          const previous = prev[update.repoId];
          if (!previous) {
            return;
          }
          next[update.repoId] = {
            ...previous,
            status: update.status,
            progress: update.progress ?? previous.progress,
            downloadedBytes: update.downloadedBytes ?? previous.downloadedBytes,
            totalBytes: update.totalBytes ?? previous.totalBytes,
          };
          if (update.status === 'error') {
            setDownloadRepoId(update.repoId);
            setDownloadError(update.error || 'Download failed.');
          } else if (update.status === 'completed' || update.status === 'cancelled') {
            if (downloadRepoId === update.repoId) {
              setDownloadError(null);
            }
          }
        });
        return next;
      });
    }, 800);

    return () => {
      if (downloadPollingRef.current) {
        window.clearInterval(downloadPollingRef.current);
        downloadPollingRef.current = null;
      }
    };
  }, [downloadStatusByRepo, downloadRepoId]);

  const startDownload = (repoId: string, downloadId: string) => {
    setDownloadStatusByRepo(prev => ({
      ...prev,
      [repoId]: {
        downloadId,
        status: 'queued',
        progress: 0,
      },
    }));
    setDownloadRepoId(repoId);
    setDownloadError(null);
  };

  const cancelDownload = async (repoId: string) => {
    const status = downloadStatusByRepo[repoId];
    if (!status || !window.pywebview?.api?.cancel_model_download) {
      return;
    }

    setDownloadStatusByRepo(prev => ({
      ...prev,
      [repoId]: {
        downloadId: prev[repoId]?.downloadId || status.downloadId,
        status: 'cancelling' as const,
        progress: prev[repoId]?.progress || 0,
        downloadedBytes: prev[repoId]?.downloadedBytes,
        totalBytes: prev[repoId]?.totalBytes,
      },
    }));

    try {
      await window.pywebview.api.cancel_model_download(status.downloadId);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error cancelling download', { error: error.message, endpoint: error.endpoint, repoId });
      } else if (error instanceof Error) {
        logger.error('Unexpected error cancelling download', { error: error.message, repoId });
      } else {
        logger.error('Unknown error cancelling download', { error, repoId });
      }
    }
  };

  return {
    downloadStatusByRepo,
    downloadError,
    downloadRepoId,
    startDownload,
    cancelDownload,
    setDownloadError,
    setDownloadRepoId,
  };
}
