import { api } from '../api/adapter';
import { useManagedProcess } from './useManagedProcess';

export function useTorchProcess(isRunning: boolean) {
  const {
    launchError,
    launchLogPath,
    isStarting,
    isStopping,
    startProcess,
    stopProcess,
    openLogPath,
  } = useManagedProcess({
    appName: 'Torch',
    isRunning,
    launch: () => api.launch_torch(),
    stop: () => api.stop_torch(),
  });

  return {
    launchError,
    launchLogPath,
    isStarting,
    isStopping,
    launchTorch: startProcess,
    stopTorch: stopProcess,
    openLogPath,
  };
}
