import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppProcessActions } from './useAppProcessActions';

function createOptions() {
  return {
    comfyUIRunning: false,
    launchComfyUI: vi.fn().mockResolvedValue(undefined),
    stopComfyUI: vi.fn().mockResolvedValue(undefined),
    launchLogPath: '/logs/comfyui.log',
    openLogPath: vi.fn().mockResolvedValue(undefined),
    ollamaRunning: false,
    launchOllama: vi.fn().mockResolvedValue(undefined),
    stopOllama: vi.fn().mockResolvedValue(undefined),
    ollamaLaunchLogPath: '/logs/ollama.log',
    openOllamaLogPath: vi.fn().mockResolvedValue(undefined),
    torchRunning: false,
    launchTorch: vi.fn().mockResolvedValue(undefined),
    stopTorch: vi.fn().mockResolvedValue(undefined),
    torchLaunchLogPath: '/logs/torch.log',
    openTorchLogPath: vi.fn().mockResolvedValue(undefined),
    refetchStatus: vi.fn().mockResolvedValue(undefined),
  };
}

describe('useAppProcessActions', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('launches a stopped app and schedules a follow-up status refresh', async () => {
    const options = createOptions();
    const { result } = renderHook(() => useAppProcessActions(options));

    await act(async () => {
      await result.current.handleLaunchApp('comfyui');
    });

    expect(options.launchComfyUI).toHaveBeenCalledTimes(1);
    expect(options.stopComfyUI).not.toHaveBeenCalled();
    expect(options.refetchStatus).toHaveBeenCalledWith(false, true);

    await act(async () => {
      vi.advanceTimersByTime(1200);
    });

    expect(options.refetchStatus).toHaveBeenCalledTimes(2);
  });

  it('stops a running app and routes log opens by app id', async () => {
    const options = createOptions();
    options.ollamaRunning = true;
    const { result } = renderHook(() => useAppProcessActions(options));

    await act(async () => {
      await result.current.handleStopApp('ollama');
      await result.current.handleOpenLog('torch');
    });

    expect(options.stopOllama).toHaveBeenCalledTimes(1);
    expect(options.launchOllama).not.toHaveBeenCalled();
    expect(options.openTorchLogPath).toHaveBeenCalledWith('/logs/torch.log');
    expect(options.openLogPath).not.toHaveBeenCalled();
  });
});
