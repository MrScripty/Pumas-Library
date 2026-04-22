import { useCallback, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import type { CheckLauncherUpdatesResponse } from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('useLauncherUpdates');

export type LauncherUpdateState = {
  latestVersion: string | null;
  releaseUrl: string | null;
  downloadUrl: string | null;
};

const NO_LAUNCHER_UPDATE: CheckLauncherUpdatesResponse = {
  success: false,
  hasUpdate: false,
  currentCommit: '',
  latestCommit: '',
  commitsBehind: 0,
  commits: [],
};

export function useLauncherUpdates() {
  const [launcherUpdateAvailable, setLauncherUpdateAvailable] = useState(false);
  const [launcherUpdateState, setLauncherUpdateState] = useState<LauncherUpdateState | null>(null);
  const [isCheckingLauncherUpdates, setIsCheckingLauncherUpdates] = useState(false);

  const setLauncherUpdateResult = useCallback((updateResult: CheckLauncherUpdatesResponse) => {
    setLauncherUpdateAvailable(updateResult.hasUpdate);

    if (updateResult.hasUpdate) {
      setLauncherUpdateState({
        latestVersion: updateResult.latestVersion ?? null,
        releaseUrl: updateResult.releaseUrl ?? null,
        downloadUrl: updateResult.downloadUrl ?? null,
      });
      return;
    }

    setLauncherUpdateState(null);
  }, []);

  const checkLauncherVersion = useCallback(async (forceRefresh = false) => {
    try {
      if (!isAPIAvailable()) {
        return NO_LAUNCHER_UPDATE;
      }

      await api.get_launcher_version();

      const updateResult = await api.check_launcher_updates(forceRefresh);
      if (updateResult.success) {
        setLauncherUpdateResult(updateResult);
      }
      return updateResult;
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error checking launcher version', {
          error: error.message,
          endpoint: error.endpoint,
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error checking launcher version', { error: error.message });
      } else {
        logger.error('Unknown error checking launcher version', { error });
      }
      return NO_LAUNCHER_UPDATE;
    }
  }, [setLauncherUpdateResult]);

  const checkLauncherUpdates = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setIsCheckingLauncherUpdates(true);
    try {
      await checkLauncherVersion(true);
    } finally {
      setIsCheckingLauncherUpdates(false);
    }
  }, [checkLauncherVersion]);

  const openLauncherUpdate = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    const targetUrl = launcherUpdateState?.downloadUrl ?? launcherUpdateState?.releaseUrl;
    if (!targetUrl) {
      return;
    }

    try {
      await api.open_url(targetUrl);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening launcher update URL', {
          error: error.message,
          endpoint: error.endpoint,
          targetUrl,
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening launcher update URL', {
          error: error.message,
          targetUrl,
        });
      } else {
        logger.error('Unknown error opening launcher update URL', { error, targetUrl });
      }
    }
  }, [launcherUpdateState]);

  return {
    checkLauncherUpdates,
    checkLauncherVersion,
    isCheckingLauncherUpdates,
    launcherUpdateAvailable,
    launcherUpdateState,
    openLauncherUpdate,
  };
}
