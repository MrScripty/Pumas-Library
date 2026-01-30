/**
 * ComfyUI process management hook
 *
 * Handles launching, stopping, and monitoring ComfyUI process.
 */

import { useState, useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useComfyUIProcess');

export function useComfyUIProcess() {
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchLogPath, setLaunchLogPath] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [isStopping, setIsStopping] = useState(false);

  const launchComfyUI = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setIsStarting(true);
    try {
      const result = await api.launch_comfyui();

      if (result.success) {
        setLaunchError(null);
        setLaunchLogPath(result.log_path || null);
        // Open browser when server is ready
        if (result.ready) {
          try {
            await api.open_url('http://127.0.0.1:8188');
          } catch (e) {
            logger.warn('Failed to open browser', { error: e });
          }
        }
      } else {
        const errMsg = result.error || 'Failed to launch ComfyUI';
        setLaunchError(errMsg);
        setLaunchLogPath(result.log_path || null);
      }
    } catch (error) {
      const errMsg = 'Error trying to launch ComfyUI';
      setLaunchError(errMsg);
      if (error instanceof APIError) {
        logger.error('API error launching ComfyUI', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error launching ComfyUI', { error: error.message });
      } else {
        logger.error('Unknown error launching ComfyUI', { error });
      }
    }
    // NOTE: isStarting is cleared by caller via clearStartingState() after status confirms
  }, []);

  const stopComfyUI = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setIsStopping(true);
    try {
      const result = await api.stop_comfyui();

      if (result.success) {
        setLaunchError(null);
      } else {
        const errMsg = 'Failed to stop ComfyUI';
        setLaunchError(errMsg);
      }
    } catch (error) {
      const errMsg = 'Error trying to stop ComfyUI';
      setLaunchError(errMsg);
      if (error instanceof APIError) {
        logger.error('API error stopping ComfyUI', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error stopping ComfyUI', { error: error.message });
      } else {
        logger.error('Unknown error stopping ComfyUI', { error });
      }
    }
    // NOTE: isStopping is cleared by caller via clearStoppingState() after status confirms
  }, []);

  // Clear transition states - called by App.tsx after status is updated
  const clearStartingState = useCallback(() => {
    setIsStarting(false);
  }, []);

  const clearStoppingState = useCallback(() => {
    setIsStopping(false);
  }, []);

  const openLogPath = useCallback(async (path: string | null | undefined) => {
    if (!path || !isAPIAvailable()) {
      return;
    }

    try {
      await api.open_path(path);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening log path', { error: error.message, endpoint: error.endpoint, path });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening log path', { error: error.message, path });
      } else {
        logger.error('Unknown error opening log path', { error, path });
      }
    }
  }, []);

  return {
    launchError,
    launchLogPath,
    isStarting,
    isStopping,
    launchComfyUI,
    stopComfyUI,
    clearStartingState,
    clearStoppingState,
    openLogPath,
  };
}
