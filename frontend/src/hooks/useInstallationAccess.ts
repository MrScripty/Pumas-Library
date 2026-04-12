import { useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import type { VersionInfo } from '../types/versions';
import { getLogger } from '../utils/logger';

const logger = getLogger('useInstallationAccess');

interface UseInstallationAccessOptions {
  isEnabled: boolean;
  resolvedAppId: string;
}

export function useInstallationAccess({
  isEnabled,
  resolvedAppId,
}: UseInstallationAccessOptions) {
  const openPath = useCallback(async (path: string) => {
    if (!isAPIAvailable()) {
      throw new APIError('API not available', 'open_path');
    }

    try {
      const result = await api.open_path(path);
      if (!result.success) {
        throw new APIError(result.error || 'Failed to open path', 'open_path');
      }
      return true;
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening path', {
          error: error.message,
          endpoint: error.endpoint,
          path,
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening path', { error: error.message, path });
      } else {
        logger.error('Unknown error opening path', { error, path });
      }
      throw error;
    }
  }, []);

  const openActiveInstall = useCallback(async () => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'open_active_install');
    }

    try {
      const result = await api.open_active_install(resolvedAppId);
      if (!result.success) {
        throw new APIError(
          result.error || 'Failed to open active installation',
          'open_active_install'
        );
      }
      return true;
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening active installation', {
          error: error.message,
          endpoint: error.endpoint,
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening active installation', { error: error.message });
      } else {
        logger.error('Unknown error opening active installation', { error });
      }
      throw error;
    }
  }, [isEnabled, resolvedAppId]);

  const getVersionInfo = useCallback(async (tag: string): Promise<VersionInfo | null> => {
    if (!isAPIAvailable() || !isEnabled) {
      throw new APIError('API not available', 'get_version_info');
    }

    try {
      const result = await api.get_version_info(tag, resolvedAppId);
      if (result.success) {
        return result.info || null;
      }
      throw new APIError(result.error || 'Failed to get version info', 'get_version_info');
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error getting version info', {
          error: error.message,
          endpoint: error.endpoint,
          tag,
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error getting version info', { error: error.message, tag });
      } else {
        logger.error('Unknown error getting version info', { error, tag });
      }
      throw error;
    }
  }, [isEnabled, resolvedAppId]);

  return {
    getVersionInfo,
    openActiveInstall,
    openPath,
  };
}
