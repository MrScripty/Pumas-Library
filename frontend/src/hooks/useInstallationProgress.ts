/**
 * Installation Progress Hook
 *
 * Manages installation progress polling and state.
 * Extracted from InstallDialog.tsx
 */

import { useState, useEffect, useRef } from 'react';
import { api } from '../api/adapter';
import type { InstallationProgress } from './useVersions';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useInstallationProgress');

interface UseInstallationProgressOptions {
  appId?: string;
  installingVersion: string | null;
  externalProgress?: InstallationProgress | null;
  onRefreshProgress?: () => Promise<unknown>;
}

interface UseInstallationProgressResult {
  progress: InstallationProgress | null;
  cancellationNotice: string | null;
  failedInstall: { tag: string; log: string | null } | null;
  setFailedInstall: (value: { tag: string; log: string | null } | null) => void;
  showCancellationNotice: () => void;
}

export function useInstallationProgress({
  appId,
  installingVersion,
  externalProgress,
  onRefreshProgress,
}: UseInstallationProgressOptions): UseInstallationProgressResult {
  const resolvedAppId = appId ?? 'comfyui';
  const [progress, setProgress] = useState<InstallationProgress | null>(externalProgress || null);
  const [cancellationNotice, setCancellationNotice] = useState<string | null>(null);
  const [failedInstall, setFailedInstall] = useState<{ tag: string; log: string | null } | null>(null);
  const cancellationRef = useRef(false);
  const noticeTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const pollIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const completionStopTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Sync external progress
  useEffect(() => {
    if (externalProgress) {
      setProgress(externalProgress);

      const isCancelled = externalProgress.error?.toLowerCase().includes('cancel');
      if (externalProgress.completed_at && !externalProgress.success && externalProgress.tag && !isCancelled) {
        setFailedInstall({
          tag: externalProgress.tag,
          log: externalProgress.log_path || null,
        });
      } else if (externalProgress.completed_at && externalProgress.success && externalProgress.tag && failedInstall?.tag === externalProgress.tag) {
        setFailedInstall(null);
      }
    }
  }, [externalProgress, failedInstall]);

  // Show cancellation notice
  const showCancellationNotice = () => {
    if (noticeTimeoutRef.current) {
      clearTimeout(noticeTimeoutRef.current);
    }

    setCancellationNotice('Installation canceled');
    noticeTimeoutRef.current = setTimeout(() => setCancellationNotice(null), 3000);
  };

  // Poll for progress updates when installing (local polling)
  useEffect(() => {
    // When external polling is provided, rely on that source and skip local polling
    if (onRefreshProgress) {
      return;
    }

    if (!installingVersion) {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      if (completionStopTimeoutRef.current) {
        clearTimeout(completionStopTimeoutRef.current);
        completionStopTimeoutRef.current = null;
      }
      setProgress(null);
      return;
    }

    const fetchProgress = async () => {
      try {
        const result = await api.get_installation_progress(resolvedAppId);
        setProgress(result as InstallationProgress | null);

        // Stop polling if installation is complete
        if (result?.completed_at) {
          if (completionStopTimeoutRef.current) {
            clearTimeout(completionStopTimeoutRef.current);
          }
          completionStopTimeoutRef.current = setTimeout(() => {
            if (pollIntervalRef.current) {
              clearInterval(pollIntervalRef.current);
              pollIntervalRef.current = null;
            }
            completionStopTimeoutRef.current = null;
          }, 1000);
        }

        if (result?.error?.toLowerCase().includes('cancel')) {
          cancellationRef.current = true;
          showCancellationNotice();
        }
      } catch (error) {
        if (error instanceof APIError) {
          logger.error('API error fetching installation progress', { error: error.message, endpoint: error.endpoint });
        } else if (error instanceof Error) {
          logger.error('Unexpected error fetching installation progress', { error: error.message });
        } else {
          logger.error('Unknown error fetching installation progress', { error });
        }
      }
    };

    // Initial fetch
    void fetchProgress();

    // Poll every second
    if (pollIntervalRef.current) {
      clearInterval(pollIntervalRef.current);
    }
    pollIntervalRef.current = setInterval(() => void fetchProgress(), 1000);

    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      if (completionStopTimeoutRef.current) {
        clearTimeout(completionStopTimeoutRef.current);
        completionStopTimeoutRef.current = null;
      }
    };
  }, [installingVersion, onRefreshProgress, resolvedAppId]);

  // Cleanup timers
  useEffect(() => {
    return () => {
      if (noticeTimeoutRef.current) {
        clearTimeout(noticeTimeoutRef.current);
        noticeTimeoutRef.current = null;
      }
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      if (completionStopTimeoutRef.current) {
        clearTimeout(completionStopTimeoutRef.current);
        completionStopTimeoutRef.current = null;
      }
    };
  }, []);

  return {
    progress,
    cancellationNotice,
    failedInstall,
    setFailedInstall,
    showCancellationNotice,
  };
}
