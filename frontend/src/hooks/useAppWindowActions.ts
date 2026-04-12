import { api, isAPIAvailable, windowAPI } from '../api/adapter';
import { APIError } from '../errors';
import { getLogger } from '../utils/logger';

const logger = getLogger('useAppWindowActions');

export function useAppWindowActions() {
  const openModelsRoot = async () => {
    if (!isAPIAvailable()) return;
    try {
      const result = await api.open_path('shared-resources/models');
      if (!result.success) {
        throw new APIError(result.error || 'Failed to open models folder', 'open_path');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening models folder', {
          error: error.message,
          endpoint: error.endpoint,
          path: 'shared-resources/models',
        });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening models folder', {
          error: error.message,
          path: 'shared-resources/models',
        });
      } else {
        logger.error('Unknown error opening models folder', {
          error,
          path: 'shared-resources/models',
        });
      }
    }
  };

  const minimizeWindow = () => {
    void windowAPI.minimize();
  };

  const closeWindow = () => {
    if (isAPIAvailable()) {
      void api.close_window();
    } else {
      window.close();
    }
  };

  return {
    closeWindow,
    minimizeWindow,
    openModelsRoot,
  };
}
