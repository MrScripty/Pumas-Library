/**
 * Installation Manager Hook
 *
 * Manages version installation, removal, switching, and progress tracking.
 * Extracted from hooks/useVersions.ts
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { InstallationProgress, InstallNetworkStatus, VersionRelease, VersionInfo } from '../types/versions';
import {
  createNetworkStatusState,
  type NetworkStatusState,
} from '../utils/networkStatusMonitor';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';
import {
  normalizeInstallationProgress,
  resetInstallationProgressTracking,
} from './installationProgressTracking';
import { useInstallationAccess } from './useInstallationAccess';

const logger = getLogger('useInstallationManager');

interface UseInstallationManagerOptions {
  appId?: string;
  enabled?: boolean;
  availableVersions: VersionRelease[];
  onRefreshVersions: () => Promise<void>;
}

interface UseInstallationManagerResult {
  installingTag: string | null;
  installationProgress: InstallationProgress | null;
  installNetworkStatus: InstallNetworkStatus;
  switchVersion: (tag: string) => Promise<boolean>;
  installVersion: (tag: string) => Promise<boolean>;
  removeVersion: (tag: string) => Promise<boolean>;
  getVersionInfo: (tag: string) => Promise<VersionInfo | null>;
  openPath: (path: string) => Promise<boolean>;
  openActiveInstall: () => Promise<boolean>;
  fetchInstallationProgress: () => Promise<InstallationProgress | null>;
}

export function useInstallationManager({
  appId,
  enabled = true,
  availableVersions,
  onRefreshVersions,
}: UseInstallationManagerOptions): UseInstallationManagerResult {
  const resolvedAppId = appId ?? 'comfyui';
  const isEnabled = enabled;
  const [installingTag, setInstallingTag] = useState<string | null>(null);
  const [installationProgress, setInstallationProgress] = useState<InstallationProgress | null>(null);
  const [installNetworkStatus, setInstallNetworkStatus] = useState<InstallNetworkStatus>('idle');

  const installPollRef = useRef<NodeJS.Timeout | null>(null);
  const lastDownloadTagRef = useRef<string | null>(null);
  const lastStageRef = useRef<InstallationProgress['stage'] | null>(null);
  const networkStateRef = useRef<NetworkStatusState>(createNetworkStatusState());
  const { getVersionInfo, openActiveInstall, openPath } = useInstallationAccess({
    isEnabled,
    resolvedAppId,
  });

  const resetInstallState = useCallback(() => {
    setInstallingTag(null);
    setInstallationProgress(null);
    setInstallNetworkStatus('idle');
    resetInstallationProgressTracking({
      lastDownloadTag: lastDownloadTagRef.current,
      lastStage: lastStageRef.current,
      networkState: networkStateRef.current,
    });
    lastDownloadTagRef.current = null;
    lastStageRef.current = null;
  }, []);

  useEffect(() => {
    if (installPollRef.current) {
      clearInterval(installPollRef.current);
      installPollRef.current = null;
    }
    resetInstallState();
  }, [resolvedAppId, isEnabled, resetInstallState]);

  // Fetch current installation progress
  const fetchInstallationProgress = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      return null;
    }

    try {
      const progress = await api.get_installation_progress(resolvedAppId);

      if (progress && !progress.completed_at) {
        setInstallingTag(progress.tag || null);
        const trackerState = {
          lastDownloadTag: lastDownloadTagRef.current,
          lastStage: lastStageRef.current,
          networkState: networkStateRef.current,
        };
        const { adjustedProgress, networkStatus } = normalizeInstallationProgress(
          progress,
          availableVersions,
          trackerState,
          Date.now()
        );
        lastDownloadTagRef.current = trackerState.lastDownloadTag;
        lastStageRef.current = trackerState.lastStage;

        setInstallationProgress(adjustedProgress);
        setInstallNetworkStatus(networkStatus);

        return adjustedProgress;
      } else if (progress?.completed_at && !progress.success) {
        const trackerState = {
          lastDownloadTag: lastDownloadTagRef.current,
          lastStage: lastStageRef.current,
          networkState: networkStateRef.current,
        };
        const { adjustedProgress } = normalizeInstallationProgress(
          progress,
          availableVersions,
          trackerState,
          Date.now()
        );
        lastDownloadTagRef.current = trackerState.lastDownloadTag;
        lastStageRef.current = trackerState.lastStage;

        if (installPollRef.current) {
          clearInterval(installPollRef.current);
          installPollRef.current = null;
        }
        setInstallingTag(null);
        setInstallationProgress(adjustedProgress);
        setInstallNetworkStatus('failed');

        return adjustedProgress;
      } else {
        // Installation completed (progress.completed_at set) or no progress
        // Clear all state and stop polling
        if (installPollRef.current) {
          clearInterval(installPollRef.current);
          installPollRef.current = null;
        }
        resetInstallState();

        // Refresh version list when installation completes
        if (progress?.completed_at) {
          void onRefreshVersions();
        }
      }

      return null;
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching installation progress', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error fetching installation progress', { error: error.message });
      } else {
        logger.error('Unknown error fetching installation progress', { error });
      }
      setInstallNetworkStatus('failed');
      return null;
    }
  }, [availableVersions, isEnabled, onRefreshVersions, resetInstallState, resolvedAppId]);

  // Switch to a different installed version
  const switchVersion = useCallback(async (tag: string) => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'switch_version');
    }

    try {
      const result = await api.switch_version(tag, resolvedAppId);
      if (result.success) {
        await onRefreshVersions();
        return true;
      } else {
        throw new APIError(result.error || 'Failed to switch version', 'switch_version');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error switching version', { error: error.message, endpoint: error.endpoint, tag });
      } else if (error instanceof Error) {
        logger.error('Unexpected error switching version', { error: error.message, tag });
      } else {
        logger.error('Unknown error switching version', { error, tag });
      }
      throw error;
    }
  }, [isEnabled, onRefreshVersions, resolvedAppId]);

  // Install a new version
  const installVersion = useCallback(async (tag: string) => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'install_version');
    }

    setInstallingTag(tag);

    try {
      const result = await api.install_version(tag, resolvedAppId);
      if (result.success) {
        // Start polling for installation progress
        if (installPollRef.current) {
          clearInterval(installPollRef.current);
        }
        installPollRef.current = setInterval(() => {
          void fetchInstallationProgress();
        }, 800);

        await fetchInstallationProgress();
        await onRefreshVersions();
        return true;
      } else {
        throw new APIError(result.error || 'Failed to install version', 'install_version');
      }
    } catch (error) {
      if (installPollRef.current) {
        clearInterval(installPollRef.current);
        installPollRef.current = null;
      }
      resetInstallState();

      if (error instanceof APIError) {
        logger.error('API error installing version', { error: error.message, endpoint: error.endpoint, tag });
      } else if (error instanceof Error) {
        logger.error('Unexpected error installing version', { error: error.message, tag });
      } else {
        logger.error('Unknown error installing version', { error, tag });
      }
      throw error;
    }
  }, [fetchInstallationProgress, isEnabled, onRefreshVersions, resetInstallState, resolvedAppId]);

  // Remove a version
  const removeVersion = useCallback(async (tag: string) => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'remove_version');
    }

    try {
      const result = await api.remove_version(tag, resolvedAppId);
      if (result.success) {
        await onRefreshVersions();
        return true;
      } else {
        throw new APIError(result.error || 'Failed to remove version', 'remove_version');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error removing version', { error: error.message, endpoint: error.endpoint, tag });
      } else if (error instanceof Error) {
        logger.error('Unexpected error removing version', { error: error.message, tag });
      } else {
        logger.error('Unknown error removing version', { error, tag });
      }
      throw error;
    }
  }, [isEnabled, onRefreshVersions, resolvedAppId]);

  // Cleanup polling on unmount
  useEffect(() => {
    return () => {
      if (installPollRef.current) {
        clearInterval(installPollRef.current);
      }
    };
  }, []);

  return {
    installingTag,
    installationProgress,
    installNetworkStatus,
    switchVersion,
    installVersion,
    removeVersion,
    getVersionInfo,
    openPath,
    openActiveInstall,
    fetchInstallationProgress,
  };
}
