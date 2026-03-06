import { api } from '../api/adapter';
import { useManagedProcess } from './useManagedProcess';

export function useOllamaProcess(isRunning: boolean) {
  const {
    launchError,
    launchLogPath,
    isStarting,
    isStopping,
    startProcess,
    stopProcess,
    openLogPath,
  } = useManagedProcess({
    appName: 'Ollama',
    isRunning,
    launch: () => api.launch_ollama(),
    stop: () => api.stop_ollama(),
  });

  return {
    launchError,
    launchLogPath,
    isStarting,
    isStopping,
    launchOllama: startProcess,
    stopOllama: stopProcess,
    openLogPath,
  };
}
