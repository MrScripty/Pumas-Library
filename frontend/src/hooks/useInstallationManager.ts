/**
 * Owns app-scoped version installation actions and progress synchronization.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type {
  InstallationProgress,
  InstallNetworkStatus,
  VersionInfo,
  VersionRelease,
} from '../types/versions';
import {
  createNetworkStatusState,
  type NetworkStatusState,
} from '../utils/networkStatusMonitor';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';
import {
  normalizeInstallationProgress,
  projectInstallationProgress,
  resetInstallationProgressTracking,
} from './installationProgressTracking';
import { useInstallationAccess } from './useInstallationAccess';

const logger = getLogger('useInstallationManager');
const INSTALLATION_POLL_INTERVAL_MS = 800;
const MAX_MISSING_PROGRESS_POLLS = 10;

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
  cancelInstallation: () => Promise<boolean>;
  removeVersion: (tag: string) => Promise<boolean>;
  getVersionInfo: (tag: string) => Promise<VersionInfo | null>;
  openPath: (path: string) => Promise<boolean>;
  openActiveInstall: () => Promise<boolean>;
}

interface InstallationLifecycle {
  appId: string;
  generation: number;
  tag: string;
}

interface ProgressRequest {
  lifecycle: InstallationLifecycle;
  promise: Promise<boolean>;
}

export function useInstallationManager({
  appId,
  enabled = true,
  availableVersions,
  onRefreshVersions,
}: UseInstallationManagerOptions): UseInstallationManagerResult {
  const resolvedAppId = appId ?? 'ollama';
  const isEnabled = enabled;
  const [installingTag, setInstallingTag] = useState<string | null>(null);
  const [installationProgress, setInstallationProgress] = useState<InstallationProgress | null>(null);
  const [installNetworkStatus, setInstallNetworkStatus] = useState<InstallNetworkStatus>('idle');

  const pollTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeProgressRequestRef = useRef<ProgressRequest | null>(null);
  const generationRef = useRef(0);
  const mountedRef = useRef(true);
  const renderIdentityRef = useRef({ appId: resolvedAppId, enabled: isEnabled });
  const lastDownloadTagRef = useRef<string | null>(null);
  const lastStageRef = useRef<InstallationProgress['stage'] | null>(null);
  const pendingInstallTagRef = useRef<string | null>(null);
  const missingProgressPollsRef = useRef(0);
  const networkStateRef = useRef<NetworkStatusState>(createNetworkStatusState());
  const runPollRef = useRef<(lifecycle: InstallationLifecycle) => void>(() => {});

  renderIdentityRef.current = { appId: resolvedAppId, enabled: isEnabled };

  const { getVersionInfo, openActiveInstall, openPath } = useInstallationAccess({
    isEnabled,
    resolvedAppId,
  });

  const resetTracking = useCallback(() => {
    resetInstallationProgressTracking({
      lastDownloadTag: lastDownloadTagRef.current,
      lastStage: lastStageRef.current,
      networkState: networkStateRef.current,
    });
    lastDownloadTagRef.current = null;
    lastStageRef.current = null;
    pendingInstallTagRef.current = null;
    missingProgressPollsRef.current = 0;
  }, []);

  const resetInstallState = useCallback(() => {
    setInstallingTag(null);
    setInstallationProgress(null);
    setInstallNetworkStatus('idle');
    resetTracking();
  }, [resetTracking]);

  const stopInstallPolling = useCallback(() => {
    if (pollTimeoutRef.current) {
      clearTimeout(pollTimeoutRef.current);
      pollTimeoutRef.current = null;
    }
  }, []);

  const isCurrentLifecycle = useCallback((lifecycle: InstallationLifecycle) => {
    const renderIdentity = renderIdentityRef.current;
    return mountedRef.current
      && renderIdentity.enabled
      && renderIdentity.appId === lifecycle.appId
      && generationRef.current === lifecycle.generation;
  }, []);

  const beginInstallationLifecycle = useCallback((tag: string): InstallationLifecycle => {
    generationRef.current += 1;
    stopInstallPolling();
    resetTracking();
    pendingInstallTagRef.current = tag;
    setInstallingTag(tag);
    setInstallationProgress(null);
    setInstallNetworkStatus('idle');
    return {
      appId: resolvedAppId,
      generation: generationRef.current,
      tag,
    };
  }, [resetTracking, resolvedAppId, stopInstallPolling]);

  const finishInstallationLifecycle = useCallback((
    lifecycle: InstallationLifecycle,
    terminalProgress?: InstallationProgress
  ) => {
    if (!isCurrentLifecycle(lifecycle)) {
      return;
    }

    stopInstallPolling();
    generationRef.current += 1;
    resetTracking();
    setInstallingTag(null);

    if (terminalProgress) {
      setInstallationProgress(terminalProgress);
      setInstallNetworkStatus(terminalProgress.success ? 'idle' : 'failed');
      return;
    }

    setInstallationProgress(null);
    setInstallNetworkStatus('idle');
  }, [isCurrentLifecycle, resetTracking, stopInstallPolling]);

  const requestInstallationProgress = useCallback(async (
    lifecycle: InstallationLifecycle
  ): Promise<boolean> => {
    if (!isCurrentLifecycle(lifecycle) || !isAPIAvailable()) {
      return false;
    }

    const activeRequest = activeProgressRequestRef.current;
    if (activeRequest) {
      if (
        activeRequest.lifecycle.appId === lifecycle.appId
        && activeRequest.lifecycle.generation === lifecycle.generation
      ) {
        return activeRequest.promise;
      }

      await activeRequest.promise;
      if (!isCurrentLifecycle(lifecycle)) {
        return false;
      }

      const replacementRequest = activeProgressRequestRef.current;
      if (replacementRequest) {
        return replacementRequest.promise;
      }
    }

    const promise = (async () => {
      try {
        const response = await api.get_installation_progress(lifecycle.appId);
        if (!isCurrentLifecycle(lifecycle)) {
          return false;
        }
        const progress = response ? projectInstallationProgress(response) : null;

        if (progress && !progress.completed_at) {
          pendingInstallTagRef.current = progress.tag || lifecycle.tag;
          missingProgressPollsRef.current = 0;
          setInstallingTag(progress.tag || lifecycle.tag);

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
          return true;
        }

        if (progress?.completed_at) {
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

          if (progress.success) {
            await onRefreshVersions();
            if (!isCurrentLifecycle(lifecycle)) {
              return false;
            }
          }

          finishInstallationLifecycle(lifecycle, adjustedProgress);
          return false;
        }

        if (
          !progress?.completed_at
          && pendingInstallTagRef.current
          && missingProgressPollsRef.current < MAX_MISSING_PROGRESS_POLLS
        ) {
          missingProgressPollsRef.current += 1;
          setInstallingTag(pendingInstallTagRef.current);
          return true;
        }

        if (pendingInstallTagRef.current) {
          await onRefreshVersions();
          if (!isCurrentLifecycle(lifecycle)) {
            return false;
          }
        }
        finishInstallationLifecycle(lifecycle);
        return false;
      } catch (error) {
        if (!isCurrentLifecycle(lifecycle)) {
          return false;
        }
        if (error instanceof APIError) {
          logger.error('API error fetching installation progress', {
            error: error.message,
            endpoint: error.endpoint,
          });
        } else if (error instanceof Error) {
          logger.error('Unexpected error fetching installation progress', { error: error.message });
        } else {
          logger.error('Unknown error fetching installation progress', { error });
        }
        setInstallNetworkStatus('failed');
        return true;
      }
    })();

    const request: ProgressRequest = { lifecycle, promise };
    activeProgressRequestRef.current = request;
    try {
      return await promise;
    } finally {
      if (activeProgressRequestRef.current === request) {
        activeProgressRequestRef.current = null;
      }
    }
  }, [availableVersions, finishInstallationLifecycle, isCurrentLifecycle, onRefreshVersions]);

  const scheduleNextPoll = useCallback((lifecycle: InstallationLifecycle) => {
    if (!isCurrentLifecycle(lifecycle)) {
      return;
    }
    stopInstallPolling();
    pollTimeoutRef.current = setTimeout(() => {
      pollTimeoutRef.current = null;
      runPollRef.current(lifecycle);
    }, INSTALLATION_POLL_INTERVAL_MS);
  }, [isCurrentLifecycle, stopInstallPolling]);

  runPollRef.current = (lifecycle) => {
    void requestInstallationProgress(lifecycle).then((shouldContinue) => {
      if (shouldContinue && isCurrentLifecycle(lifecycle)) {
        scheduleNextPoll(lifecycle);
      }
    });
  };

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
      stopInstallPolling();
    };
  }, [stopInstallPolling]);

  useEffect(() => {
    generationRef.current += 1;
    stopInstallPolling();
    resetInstallState();
  }, [isEnabled, resetInstallState, resolvedAppId, stopInstallPolling]);

  const installingReleaseTag = availableVersions.find((release) => release.installing)?.tagName ?? null;
  useEffect(() => {
    if (!isEnabled || !installingReleaseTag || pendingInstallTagRef.current === installingReleaseTag) {
      return;
    }
    const lifecycle = beginInstallationLifecycle(installingReleaseTag);
    runPollRef.current(lifecycle);
  }, [beginInstallationLifecycle, installingReleaseTag, isEnabled]);

  const switchVersion = useCallback(async (tag: string) => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'switch_version');
    }

    try {
      const result = await api.switch_version(tag, resolvedAppId);
      if (!result.success) {
        throw new APIError('Failed to switch version', 'switch_version');
      }
      await onRefreshVersions();
      return true;
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

  const installVersion = useCallback(async (tag: string) => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'install_version');
    }

    const lifecycle = beginInstallationLifecycle(tag);

    try {
      const result = await api.install_version(tag, resolvedAppId);
      if (!result.success) {
        throw new APIError(result.error || 'Failed to install version', 'install_version');
      }
      runPollRef.current(lifecycle);
      return true;
    } catch (error) {
      finishInstallationLifecycle(lifecycle);
      if (error instanceof APIError) {
        logger.error('API error installing version', { error: error.message, endpoint: error.endpoint, tag });
      } else if (error instanceof Error) {
        logger.error('Unexpected error installing version', { error: error.message, tag });
      } else {
        logger.error('Unknown error installing version', { error, tag });
      }
      throw error;
    }
  }, [beginInstallationLifecycle, finishInstallationLifecycle, isEnabled, resolvedAppId]);

  const cancelInstallation = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'cancel_installation');
    }

    const result = await api.cancel_installation(resolvedAppId);
    if (!result.success) {
      throw new APIError('Failed to cancel installation', 'cancel_installation');
    }
    return true;
  }, [isEnabled, resolvedAppId]);

  const removeVersion = useCallback(async (tag: string) => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'remove_version');
    }

    try {
      const result = await api.remove_version(tag, resolvedAppId);
      if (!result.success) {
        throw new APIError('Failed to remove version', 'remove_version');
      }
      await onRefreshVersions();
      return true;
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

  return {
    installingTag,
    installationProgress,
    installNetworkStatus,
    switchVersion,
    installVersion,
    cancelInstallation,
    removeVersion,
    getVersionInfo,
    openPath,
    openActiveInstall,
  };
}
