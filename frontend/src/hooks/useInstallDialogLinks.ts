import { useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import { getLogger } from '../utils/logger';
import { openExternalUrl } from '../components/ui/openExternalUrl';

const logger = getLogger('useInstallDialogLinks');

export function useInstallDialogLinks() {
  const openLogPath = useCallback(async (path?: string | null) => {
    if (!path || !isAPIAvailable()) return;

    try {
      await api.open_path(path);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening log path', {
          error: error.message,
          endpoint: error.endpoint,
          path,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to open log path', {
          error: error.message,
          path,
        });
      } else {
        logger.error('Unknown error opening log path', {
          error,
          path,
        });
      }
    }
  }, []);

  const openReleaseLink = useCallback(async (url: string) => {
    // The system browser is the only exit; there is no in-app window fallback.
    await openExternalUrl(url);
  }, []);

  return {
    openLogPath,
    openReleaseLink,
  };
}
