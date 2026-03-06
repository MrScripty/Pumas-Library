import { useCallback } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';
import { useManagedProcess } from './useManagedProcess';

const logger = getLogger('useComfyUIProcess');

export function useComfyUIProcess(isRunning: boolean) {
  const {
    launchError,
    launchLogPath,
    isStarting,
    isStopping,
    startProcess,
    stopProcess,
    openLogPath,
  } = useManagedProcess({
    appName: 'ComfyUI',
    isRunning,
    launch: () => api.launch_comfyui(),
    stop: () => api.stop_comfyui(),
    onLaunchSuccess: useCallback(async (result: Awaited<ReturnType<typeof api.launch_comfyui>>) => {
      if (!result.ready || !isAPIAvailable()) {
        return;
      }

      try {
        await api.open_url('http://127.0.0.1:8188');
      } catch (error) {
        logger.warn('Failed to open browser', { error });
      }
    }, []),
  });

  return {
    launchError,
    launchLogPath,
    isStarting,
    isStopping,
    launchComfyUI: startProcess,
    stopComfyUI: stopProcess,
    openLogPath,
  };
}
