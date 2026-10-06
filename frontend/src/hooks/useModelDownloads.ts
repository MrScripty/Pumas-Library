/**
 * Model Downloads Hook
 *
 * Manages model download state from backend snapshots and pushed updates.
 * Supports parallel downloads, pause/resume, and startup recovery.
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import { api, getElectronAPI, isAPIAvailable } from '../api/adapter';
import type { ModelDownloadSnapshotEntry } from '../types/api';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';
import { projectDownloadProgress } from '../utils/downloadProgressProjection';
import {
  getDownloadArtifactKey,
  selectDownloadsByRepo,
  getUnusedDownloadKey,
  type DownloadStatus,
  type DownloadArtifactIdentity,
} from './modelDownloadState';

const logger = getLogger('useModelDownloads');

const ACTIVE_STATUSES = ['queued', 'downloading', 'cancelling', 'pausing'] as const;

type DownloadCommand = {
  previousStatus: DownloadStatus;
  outcome: 'pending' | 'succeeded' | 'failed';
};

function isActiveStatus(status: string): boolean {
  return (ACTIVE_STATUSES as readonly string[]).includes(status);
}

export function useModelDownloads() {
  const [downloadStatusByRepo, setDownloadStatusByRepo] = useState<Record<string, DownloadStatus>>({});
  const [downloadErrors, setDownloadErrors] = useState<Record<string, string>>({});

  // Ref keeps command handlers independent from render timing.
  const downloadStatusRef = useRef(downloadStatusByRepo);
  const downloadKeysRef = useRef(new Map<string, string>());
  const downloadCommandsRef = useRef(new WeakMap<DownloadStatus, DownloadCommand>());

  useEffect(() => {
    downloadStatusRef.current = downloadStatusByRepo;
    downloadKeysRef.current = new Map(Object.entries(downloadStatusByRepo)
      .map(([key, status]) => [status.downloadId, key]));
  }, [downloadStatusByRepo]);

  const applyDownloadSnapshot = useCallback((
    downloads: ModelDownloadSnapshotEntry[],
    options: { preserveExisting?: boolean } = {}
  ) => {
    const { statuses, errors } = selectDownloadsByRepo(downloads);
    // Pushes can arrive in the same React batch as command settlement.
    // Record presentation ownership before either state update is rendered.
    const snapshotKeys = new Map(Object.entries(statuses).map(([key, status]) => [status.downloadId, key]));
    if (options.preserveExisting) {
      const occupiedKeys = new Set(Object.keys(statuses));
      for (const [downloadId, key] of downloadKeysRef.current) {
        if (snapshotKeys.has(downloadId)) continue;
        const targetKey = occupiedKeys.has(key) ? getUnusedDownloadKey(downloadId, occupiedKeys) : key;
        snapshotKeys.set(downloadId, targetKey);
        occupiedKeys.add(targetKey);
      }
    }
    downloadKeysRef.current = snapshotKeys;
    setDownloadStatusByRepo((prev) => {
      if (!options.preserveExisting) return statuses;
      const merged = { ...statuses };
      const snapshotKeys = new Map(Object.entries(statuses).map(([key, status]) => [status.downloadId, key]));
      for (const [key, status] of Object.entries(prev)) {
        const snapshotKey = snapshotKeys.get(status.downloadId);
        const targetKey = snapshotKey ?? (merged[key] && merged[key].downloadId !== status.downloadId
          ? getUnusedDownloadKey(status.downloadId, Object.keys(merged)) : key);
        const mergedStatus: DownloadStatus = {
          ...statuses[targetKey], ...status,
          libraryModelId: snapshotKey ? statuses[snapshotKey]?.libraryModelId : status.libraryModelId,
        };
        // Startup recovery retains local command state, including its ownership.
        const operation = downloadCommandsRef.current.get(status);
        if (operation) downloadCommandsRef.current.set(mergedStatus, operation);
        merged[targetKey] = mergedStatus;
      }
      return merged;
    });
    setDownloadErrors(errors);
  }, []);

  // Startup recovery plus backend-owned pushed updates.
  useEffect(() => {
    let cancelled = false;
    // The startup list has no revision. Once the subscription supplies a full
    // snapshot, that source owns presentation for this mounted effect.
    let startupSuperseded = false;

    const restoreDownloads = async () => {
      if (!isAPIAvailable()) return;
      try {
        const result = await api.list_model_downloads();
        if (cancelled) return;
        if (startupSuperseded) {
          logger.debug('Startup download snapshot superseded by subscribed snapshot');
          return;
        }
        applyDownloadSnapshot(result.downloads.map(projectDownloadProgress), { preserveExisting: true });
      } catch (error) {
        logger.warn('Failed to restore downloads on startup', { error });
      }
    };

    void restoreDownloads();

    const unsubscribe = getElectronAPI()?.onModelDownloadUpdate((notification) => {
      if (cancelled) return;
      startupSuperseded = true;
      applyDownloadSnapshot(notification.snapshot.downloads.map(projectDownloadProgress));
    });

    return () => {
      cancelled = true;
      unsubscribe?.();
    };
  }, [applyDownloadSnapshot]);

  const startDownload = useCallback((
    downloadKey: string,
    downloadId: string,
    details?: { libraryModelId?: string | null; modelName?: string; modelType?: string } & DownloadArtifactIdentity
  ) => {
    const artifactKey = getDownloadArtifactKey({
      selectedArtifactId: details?.selectedArtifactId,
      artifactId: details?.artifactId,
      repoId: details?.repoId ?? downloadKey,
    }) ?? downloadKey;

    setDownloadStatusByRepo((prev) => {
      const existingEntry = Object.entries(prev).find(([, status]) => status.downloadId === downloadId);
      const targetKey = existingEntry?.[0] ?? (prev[artifactKey]
        ? getUnusedDownloadKey(downloadId, Object.keys(prev)) : artifactKey);
      const existing = prev[targetKey];
      if (existing && isActiveStatus(existing.status)) {
        if (existing.downloadId === downloadId && details?.libraryModelId != null
          && existing.libraryModelId !== details.libraryModelId) {
          const associatedStatus = { ...existing, libraryModelId: details.libraryModelId };
          const operation = downloadCommandsRef.current.get(existing);
          if (operation) downloadCommandsRef.current.set(associatedStatus, operation);
          return { ...prev, [targetKey]: associatedStatus };
        }
        return prev;
      }
      return {
        ...prev,
        [targetKey]: {
          downloadId,
          libraryModelId: details?.libraryModelId
            ?? (existing?.downloadId === downloadId ? existing.libraryModelId : undefined),
          status: 'queued',
          progress: 0,
          repoId: details?.repoId ?? downloadKey,
          selectedArtifactId: details?.selectedArtifactId,
          artifactId: details?.artifactId,
          modelName: details?.modelName,
          modelType: details?.modelType,
        },
      };
    });
    setDownloadErrors((prev) => {
      const keys = Object.keys(prev).filter((key) => downloadStatusRef.current[key]?.downloadId === downloadId
        || (key === artifactKey && !downloadStatusRef.current[key]));
      if (keys.length === 0) return prev;
      const next = { ...prev };
      for (const key of keys) delete next[key];
      return next;
    });
  }, []);

  const reportCommandFailure = useCallback((
    previousStatus: DownloadStatus,
    optimisticStatus: DownloadStatus,
    message: string
  ) => {
    const operation = downloadCommandsRef.current.get(optimisticStatus);
    if (operation) operation.outcome = 'failed';
    setDownloadStatusByRepo((prev) => {
      const entry = Object.entries(prev).find(([, current]) => current.downloadId === previousStatus.downloadId);
      // Only the command's object or its startup-recovery alias can roll back.
      // Fresh pushed snapshots and later commands have independent ownership.
      if (!entry || (entry[1] !== optimisticStatus
        && (!operation || downloadCommandsRef.current.get(entry[1]) !== operation))) return prev;
      let restoredStatus = previousStatus;
      let ancestor = downloadCommandsRef.current.get(restoredStatus);
      while (ancestor?.outcome === 'failed') {
        restoredStatus = ancestor.previousStatus;
        ancestor = downloadCommandsRef.current.get(restoredStatus);
      }
      if (entry[1] !== optimisticStatus) {
        const restoredOperation = downloadCommandsRef.current.get(restoredStatus);
        restoredStatus = { ...restoredStatus, libraryModelId: entry[1].libraryModelId };
        if (restoredOperation) downloadCommandsRef.current.set(restoredStatus, restoredOperation);
      }
      return { ...prev, [entry[0]]: restoredStatus };
    });
    setDownloadErrors((prev) => {
      const currentKey = downloadKeysRef.current.get(previousStatus.downloadId);
      if (currentKey === undefined) return prev;
      return { ...prev, [currentKey]: message };
    });
  }, []);

  const cancelDownload = useCallback(async (downloadKey: string) => {
    const status = downloadStatusRef.current[downloadKey];
    if (!status || !isAPIAvailable()) return;

    const optimisticStatus: DownloadStatus = { ...status, status: 'cancelling' };
    const operation: DownloadCommand = { previousStatus: status, outcome: 'pending' };
    downloadCommandsRef.current.set(optimisticStatus, operation);
    setDownloadStatusByRepo((prev) => prev[downloadKey] === status
      ? { ...prev, [downloadKey]: optimisticStatus } : prev);

    try {
      const result = await api.cancel_model_download(status.downloadId);
      if (!result.success) {
        throw new APIError(result.error || 'Failed to cancel download.', 'cancel_model_download');
      }
      operation.outcome = 'succeeded';
    } catch (error) {
      const message = error instanceof Error ? error.message
        : typeof error === 'string' && error ? error : 'Failed to cancel download.';
      reportCommandFailure(status, optimisticStatus, message || 'Failed to cancel download.');
      if (error instanceof APIError) {
        logger.error('API error cancelling download', { error: message, endpoint: error.endpoint, downloadKey, repoId: status.repoId });
      } else {
        logger.error('Failed to cancel download', { error: message, downloadKey, repoId: status.repoId });
      }
    }
  }, [reportCommandFailure]);

  const pauseDownload = useCallback(async (downloadKey: string) => {
    const status = downloadStatusRef.current[downloadKey];
    if (!status || !isAPIAvailable()) return;

    const optimisticStatus: DownloadStatus = { ...status, status: 'pausing' };
    const operation: DownloadCommand = { previousStatus: status, outcome: 'pending' };
    downloadCommandsRef.current.set(optimisticStatus, operation);
    setDownloadStatusByRepo((prev) => prev[downloadKey] === status
      ? { ...prev, [downloadKey]: optimisticStatus } : prev);

    try {
      const result = await api.pause_model_download(status.downloadId);
      if (!result.success) {
        throw new APIError(result.error || 'Failed to pause download.', 'pause_model_download');
      }
      operation.outcome = 'succeeded';
    } catch (error) {
      const message = error instanceof Error ? error.message
        : typeof error === 'string' && error ? error : 'Failed to pause download.';
      reportCommandFailure(status, optimisticStatus, message || 'Failed to pause download.');
      logger.error('Failed to pause download', {
        error: message,
        downloadKey,
        repoId: status.repoId,
      });
    }
  }, [reportCommandFailure]);

  const resumeDownload = useCallback(async (downloadKey: string) => {
    const status = downloadStatusRef.current[downloadKey];
    if (!status || !isAPIAvailable()) return;

    const optimisticStatus: DownloadStatus = { ...status, status: 'queued', speed: undefined, etaSeconds: undefined };
    const operation: DownloadCommand = { previousStatus: status, outcome: 'pending' };
    downloadCommandsRef.current.set(optimisticStatus, operation);
    setDownloadStatusByRepo((prev) => prev[downloadKey] === status
      ? { ...prev, [downloadKey]: optimisticStatus } : prev);
    setDownloadErrors((prev) => {
      if (!prev[downloadKey]) return prev;
      const next = { ...prev };
      delete next[downloadKey];
      return next;
    });

    try {
      const result = await api.resume_model_download(status.downloadId);
      if (!result.success) {
        throw new APIError(result.error || 'Failed to resume download.', 'resume_model_download');
      }
      operation.outcome = 'succeeded';
    } catch (error) {
      const message = error instanceof Error ? error.message
        : typeof error === 'string' && error ? error : 'Failed to resume download.';
      reportCommandFailure(status, optimisticStatus, message || 'Failed to resume download.');
      logger.error('Failed to resume download', {
        error: message,
        downloadKey,
        repoId: status.repoId,
      });
    }
  }, [reportCommandFailure]);

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
