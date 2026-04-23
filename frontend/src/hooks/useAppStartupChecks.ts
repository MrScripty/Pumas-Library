import { useEffect } from 'react';
import { isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';

const logger = getLogger('useAppStartupChecks');

interface UseAppStartupChecksOptions {
  activeVersion?: string | null | undefined;
  checkLauncherVersion: (force: boolean) => Promise<unknown>;
  fetchDiskSpace: () => Promise<void> | void;
  refetchStatus: (force?: boolean) => Promise<void> | void;
  isApiAvailable?: (() => boolean) | undefined;
}

export function useAppStartupChecks({
  activeVersion,
  checkLauncherVersion,
  fetchDiskSpace,
  refetchStatus,
  isApiAvailable = isAPIAvailable,
}: UseAppStartupChecksOptions) {
  useEffect(() => {
    let waitTimeout: NodeJS.Timeout | null = null;
    let updateTimeout: NodeJS.Timeout | null = null;

    const startPolling = () => {
      updateTimeout = setTimeout(() => {
        checkLauncherVersion(false).catch((err: unknown) => {
          logger.debug('Background update check failed', { error: err });
        });
      }, 3000);
      void fetchDiskSpace();
    };

    const waitForApi = () => {
      if (isApiAvailable()) {
        startPolling();
        return;
      }
      waitTimeout = setTimeout(waitForApi, 100);
    };

    waitForApi();

    return () => {
      if (waitTimeout) clearTimeout(waitTimeout);
      if (updateTimeout) clearTimeout(updateTimeout);
    };
  }, [checkLauncherVersion, fetchDiskSpace, isApiAvailable]);

  useEffect(() => {
    if (activeVersion && isApiAvailable()) {
      void refetchStatus(false);
    }
  }, [activeVersion, isApiAvailable, refetchStatus]);
}
