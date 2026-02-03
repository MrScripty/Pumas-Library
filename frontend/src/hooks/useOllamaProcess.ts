/**
 * Ollama process management hook
 *
 * Handles launching, stopping, and monitoring Ollama server process.
 */

import { useState, useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('useOllamaProcess');

export function useOllamaProcess() {
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchLogPath, setLaunchLogPath] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [isStopping, setIsStopping] = useState(false);

  const launchOllama = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setIsStarting(true);
    try {
      const result = await api.launch_ollama();

      if (result.success) {
        setLaunchError(null);
        setLaunchLogPath(result.log_path || null);
      } else {
        const errMsg = result.error || 'Failed to launch Ollama';
        setLaunchError(errMsg);
        setLaunchLogPath(result.log_path || null);
      }
    } catch (error) {
      const errMsg = 'Error trying to launch Ollama';
      setLaunchError(errMsg);
      if (error instanceof APIError) {
        logger.error('API error launching Ollama', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error launching Ollama', { error: error.message });
      } else {
        logger.error('Unknown error launching Ollama', { error });
      }
    }
    // NOTE: isStarting is cleared by caller via clearStartingState() after status confirms
  }, []);

  const stopOllama = useCallback(async () => {
    if (!isAPIAvailable()) {
      return;
    }

    setIsStopping(true);
    try {
      const result = await api.stop_ollama();

      if (result.success) {
        setLaunchError(null);
      } else {
        const errMsg = 'Failed to stop Ollama';
        setLaunchError(errMsg);
      }
    } catch (error) {
      const errMsg = 'Error trying to stop Ollama';
      setLaunchError(errMsg);
      if (error instanceof APIError) {
        logger.error('API error stopping Ollama', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error stopping Ollama', { error: error.message });
      } else {
        logger.error('Unknown error stopping Ollama', { error });
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
    launchOllama,
    stopOllama,
    clearStartingState,
    clearStoppingState,
    openLogPath,
  };
}
