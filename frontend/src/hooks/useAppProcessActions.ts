import { useCallback } from 'react';

interface UseAppProcessActionsOptions {
  ollamaRunning: boolean;
  launchOllama: () => Promise<void>;
  stopOllama: () => Promise<void>;
  ollamaLaunchLogPath?: string | null;
  openOllamaLogPath: (path: string) => Promise<void>;
  torchRunning: boolean;
  launchTorch: () => Promise<void>;
  stopTorch: () => Promise<void>;
  torchLaunchLogPath?: string | null;
  openTorchLogPath: (path: string) => Promise<void>;
  refetchStatus: () => Promise<void>;
}

export function useAppProcessActions({
  ollamaRunning,
  launchOllama,
  stopOllama,
  ollamaLaunchLogPath,
  openOllamaLogPath,
  torchRunning,
  launchTorch,
  stopTorch,
  torchLaunchLogPath,
  openTorchLogPath,
  refetchStatus,
}: UseAppProcessActionsOptions) {
  const scheduleStatusRefresh = useCallback(() => {
    window.setTimeout(() => {
      void refetchStatus();
    }, 1200);
  }, [refetchStatus]);

  const toggleOllama = useCallback(async () => {
    if (ollamaRunning) {
      await stopOllama();
    } else {
      await launchOllama();
    }
    await refetchStatus();
    scheduleStatusRefresh();
  }, [launchOllama, ollamaRunning, refetchStatus, scheduleStatusRefresh, stopOllama]);

  const toggleTorch = useCallback(async () => {
    if (torchRunning) {
      await stopTorch();
    } else {
      await launchTorch();
    }
    await refetchStatus();
    scheduleStatusRefresh();
  }, [launchTorch, refetchStatus, scheduleStatusRefresh, stopTorch, torchRunning]);

  const handleLaunchApp = useCallback(async (appId: string) => {
    if (appId === 'ollama' && !ollamaRunning) {
      await toggleOllama();
    } else if (appId === 'torch' && !torchRunning) {
      await toggleTorch();
    }
  }, [ollamaRunning, toggleOllama, toggleTorch, torchRunning]);

  const handleStopApp = useCallback(async (appId: string) => {
    if (appId === 'ollama' && ollamaRunning) {
      await toggleOllama();
    } else if (appId === 'torch' && torchRunning) {
      await toggleTorch();
    }
  }, [ollamaRunning, toggleOllama, toggleTorch, torchRunning]);

  const handleOpenLog = useCallback(async (appId: string) => {
    if (appId === 'ollama' && ollamaLaunchLogPath) {
      await openOllamaLogPath(ollamaLaunchLogPath);
    } else if (appId === 'torch' && torchLaunchLogPath) {
      await openTorchLogPath(torchLaunchLogPath);
    }
  }, [
    ollamaLaunchLogPath,
    openOllamaLogPath,
    openTorchLogPath,
    torchLaunchLogPath,
  ]);

  return {
    handleLaunchApp,
    handleOpenLog,
    handleStopApp,
  };
}
