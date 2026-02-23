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
  const [noticeTimeout, setNoticeTimeout] = useState<NodeJS.Timeout | null>(null);
  const [pollInterval, setPollInterval] = useState<NodeJS.Timeout | null>(null);
  const [failedInstall, setFailedInstall] = useState<{ tag: string; log: string | null } | null>(null);
  const cancellationRef = useRef(false);

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
    if (noticeTimeout) {
      clearTimeout(noticeTimeout);
    }

    setCancellationNotice('Installation canceled');
    const timeoutId = setTimeout(() => setCancellationNotice(null), 3000);
    setNoticeTimeout(timeoutId);
  };

  // Poll for progress updates when installing (local polling)
  useEffect(() => {
    // When external polling is provided, rely on that source and skip local polling
    if (onRefreshProgress) {
      return;
    }

    if (!installingVersion) {
      if (pollInterval) {
        clearInterval(pollInterval);
        setPollInterval(null);
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
          setTimeout(() => {
            if (pollInterval) {
              clearInterval(pollInterval);
              setPollInterval(null);
            }
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
    const interval = setInterval(() => void fetchProgress(), 1000);
    setPollInterval(interval);

    return () => {
      clearInterval(interval);
    };
  }, [installingVersion, onRefreshProgress, resolvedAppId]);

  // Cleanup notice timeout
  useEffect(() => {
    return () => {
      if (noticeTimeout) {
        clearTimeout(noticeTimeout);
      }
    };
  }, [noticeTimeout]);

  return {
    progress,
    cancellationNotice,
    failedInstall,
    setFailedInstall,
    showCancellationNotice,
  };
}
