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
  resetNetworkStatusState,
  computeNetworkStatus,
  computeAverageSpeed,
  updateDownloadSamples,
  type NetworkStatusState,
} from '../utils/networkStatusMonitor';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

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

  const resetInstallState = useCallback(() => {
    setInstallingTag(null);
    setInstallationProgress(null);
    setInstallNetworkStatus('idle');
    resetNetworkStatusState(networkStateRef.current);
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
        const now = Date.now();
        const downloadedBytes = progress.downloaded_bytes || 0;
        const speed = progress.download_speed || 0;

        // Reset download tracker when tag changes
        if (progress.tag !== lastDownloadTagRef.current) {
          lastDownloadTagRef.current = progress.tag || null;
          lastStageRef.current = progress.stage || null;
          resetNetworkStatusState(networkStateRef.current);
          networkStateRef.current.lastDownload = { bytes: downloadedBytes, speed, ts: now };
          networkStateRef.current.topSpeed = speed || 0;
        } else if (progress.stage !== lastStageRef.current) {
          networkStateRef.current.downloadSamples = [];
          lastStageRef.current = progress.stage || null;
        }

        // Update download samples
        networkStateRef.current.downloadSamples = updateDownloadSamples(
          networkStateRef.current.downloadSamples,
          now,
          downloadedBytes
        );

        const averageSpeed = computeAverageSpeed(networkStateRef.current.downloadSamples);

        // Estimate total size based on stage
        const release = availableVersions.find((r) => r.tag_name === progress.tag);
        const archiveEstimate = release?.archive_size ?? null;
        const dependencyEstimate =
          release?.total_size && release?.archive_size
            ? Math.max(release.total_size - release.archive_size, 0)
            : null;

        let expectedTotal: number | null = null;
        if (progress.stage === 'download') {
          expectedTotal = progress.total_size ?? archiveEstimate ?? release?.total_size ?? null;
        } else if (progress.stage === 'dependencies') {
          expectedTotal = dependencyEstimate ?? release?.total_size ?? null;
        }

        // Calculate ETA
        let etaSeconds: number | null = null;
        const etaSpeed = averageSpeed > 0 ? averageSpeed : speed;
        if ((progress.stage === 'download' || progress.stage === 'dependencies') && expectedTotal && etaSpeed > 0) {
          const remaining = Math.max(expectedTotal - downloadedBytes, 0);
          etaSeconds = Math.ceil(remaining / etaSpeed);
        }

        const adjustedProgress: InstallationProgress = {
          tag: progress.tag || '',
          started_at: progress.started_at || '',
          stage: progress.stage || 'download',
          stage_progress: progress.stage_progress || 0,
          overall_progress: progress.overall_progress || 0,
          current_item: progress.current_item || null,
          download_speed: progress.download_speed ?? (averageSpeed > 0 ? averageSpeed : null),
          eta_seconds: etaSeconds,
          total_size: expectedTotal ?? progress.total_size ?? null,
          downloaded_bytes: progress.downloaded_bytes || 0,
          dependency_count: progress.dependency_count ?? null,
          completed_dependencies: progress.completed_dependencies ?? 0,
          completed_items: progress.completed_items ?? [],
          error: progress.error ?? null,
        };

        setInstallationProgress(adjustedProgress);

        // Compute network status
        const status = computeNetworkStatus(adjustedProgress, networkStateRef.current, now);
        setInstallNetworkStatus(status);

        return adjustedProgress;
      } else {
        if (!progress && installingTag) {
        return null;
      }
      setInstallingTag(null);
      setInstallationProgress(null);
      setInstallNetworkStatus('idle');
      resetNetworkStatusState(networkStateRef.current);
      lastDownloadTagRef.current = null;
      lastStageRef.current = null;
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
  }, [availableVersions, installingTag, isEnabled, resolvedAppId]);

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
      if (error instanceof APIError) {
        logger.error('API error installing version', { error: error.message, endpoint: error.endpoint, tag });
      } else if (error instanceof Error) {
        logger.error('Unexpected error installing version', { error: error.message, tag });
      } else {
        logger.error('Unknown error installing version', { error, tag });
      }
      throw error;
    }
  }, [fetchInstallationProgress, isEnabled, onRefreshVersions, resolvedAppId]);

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

  // Open arbitrary path in the system file manager
  const openPath = useCallback(async (path: string) => {
    if (!isAPIAvailable()) {
      throw new APIError('API not available', 'open_path');
    }

    try {
      const result = await api.open_path(path);
      if (!result.success) {
        const message = result.error || 'Failed to open path';
        throw new APIError(message, 'open_path');
      }
      return true;
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening path', { error: error.message, endpoint: error.endpoint, path });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening path', { error: error.message, path });
      } else {
        logger.error('Unknown error opening path', { error, path });
      }
      throw error;
    }
  }, []);

  // Open the active installation directory
  const openActiveInstall = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'open_active_install');
    }

    try {
      const result = await api.open_active_install(resolvedAppId);
      if (!result.success) {
        const message = result.error || 'Failed to open active installation';
        throw new APIError(message, 'open_active_install');
      }
      return true;
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening active installation', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening active installation', { error: error.message });
      } else {
        logger.error('Unknown error opening active installation', { error });
      }
      throw error;
    }
  }, [isEnabled, resolvedAppId]);

  // Get version info
  const getVersionInfo = useCallback(async (tag: string): Promise<VersionInfo | null> => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'get_version_info');
    }

    try {
      const result = await api.get_version_info(tag, resolvedAppId);
      if (result.success) {
        return result.info || null;
      } else {
        throw new APIError(result.error || 'Failed to get version info', 'get_version_info');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error getting version info', { error: error.message, endpoint: error.endpoint, tag });
      } else if (error instanceof Error) {
        logger.error('Unexpected error getting version info', { error: error.message, tag });
      } else {
        logger.error('Unknown error getting version info', { error, tag });
      }
      throw error;
    }
  }, [isEnabled, resolvedAppId]);

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
