import { useCallback } from 'react';

interface UseAppProcessActionsOptions {
  comfyUIRunning: boolean;
  launchComfyUI: () => Promise<void>;
  stopComfyUI: () => Promise<void>;
  launchLogPath?: string | null;
  openLogPath: (path: string) => Promise<void>;
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
  refetchStatus: (forceRefresh?: boolean, includeProcessStatus?: boolean) => Promise<void>;
}

export function useAppProcessActions({
  comfyUIRunning,
  launchComfyUI,
  stopComfyUI,
  launchLogPath,
  openLogPath,
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
      void refetchStatus(false, true);
    }, 1200);
  }, [refetchStatus]);

  const toggleComfyUI = useCallback(async () => {
    if (comfyUIRunning) {
      await stopComfyUI();
    } else {
      await launchComfyUI();
    }
    await refetchStatus(false, true);
    scheduleStatusRefresh();
  }, [comfyUIRunning, launchComfyUI, refetchStatus, scheduleStatusRefresh, stopComfyUI]);

  const toggleOllama = useCallback(async () => {
    if (ollamaRunning) {
      await stopOllama();
    } else {
      await launchOllama();
    }
    await refetchStatus(false, true);
    scheduleStatusRefresh();
  }, [launchOllama, ollamaRunning, refetchStatus, scheduleStatusRefresh, stopOllama]);

  const toggleTorch = useCallback(async () => {
    if (torchRunning) {
      await stopTorch();
    } else {
      await launchTorch();
    }
    await refetchStatus(false, true);
    scheduleStatusRefresh();
  }, [launchTorch, refetchStatus, scheduleStatusRefresh, stopTorch, torchRunning]);

  const handleLaunchApp = useCallback(async (appId: string) => {
    if (appId === 'comfyui' && !comfyUIRunning) {
      await toggleComfyUI();
    } else if (appId === 'ollama' && !ollamaRunning) {
      await toggleOllama();
    } else if (appId === 'torch' && !torchRunning) {
      await toggleTorch();
    }
  }, [comfyUIRunning, ollamaRunning, toggleComfyUI, toggleOllama, toggleTorch, torchRunning]);

  const handleStopApp = useCallback(async (appId: string) => {
    if (appId === 'comfyui' && comfyUIRunning) {
      await toggleComfyUI();
    } else if (appId === 'ollama' && ollamaRunning) {
      await toggleOllama();
    } else if (appId === 'torch' && torchRunning) {
      await toggleTorch();
    }
  }, [comfyUIRunning, ollamaRunning, toggleComfyUI, toggleOllama, toggleTorch, torchRunning]);

  const handleOpenLog = useCallback(async (appId: string) => {
    if (appId === 'comfyui' && launchLogPath) {
      await openLogPath(launchLogPath);
    } else if (appId === 'ollama' && ollamaLaunchLogPath) {
      await openOllamaLogPath(ollamaLaunchLogPath);
    } else if (appId === 'torch' && torchLaunchLogPath) {
      await openTorchLogPath(torchLaunchLogPath);
    }
  }, [
    launchLogPath,
    ollamaLaunchLogPath,
    openLogPath,
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
