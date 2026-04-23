import { useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import { getLogger } from '../utils/logger';

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
    try {
      if (isAPIAvailable()) {
        const result = await api.open_url(url);
        if (!result.success) {
          logger.warn('API failed to open URL, falling back to window.open', { url });
          window.open(url, '_blank');
        }
      } else {
        window.open(url, '_blank');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening release link', {
          error: error.message,
          endpoint: error.endpoint,
          url,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to open release link', {
          error: error.message,
          url,
        });
      } else {
        logger.error('Unknown error opening release link', {
          error,
          url,
        });
      }
      window.open(url, '_blank');
    }
  }, []);

  return {
    openLogPath,
    openReleaseLink,
  };
}
