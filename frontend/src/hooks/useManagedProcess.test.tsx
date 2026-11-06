import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useManagedProcess } from './useManagedProcess';
import type { BaseResponse, LaunchResponse } from '../types/api';

const { openPathMock, isApiAvailableMock } = vi.hoisted(() => ({
  openPathMock: vi.fn<(_path: string) => Promise<void>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    open_path: openPathMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

describe('useManagedProcess', () => {
  beforeEach(() => {
    isApiAvailableMock.mockReturnValue(true);
    openPathMock.mockResolvedValue();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('owns transition state until running status confirms startup', async () => {
    const launch = vi.fn<() => Promise<LaunchResponse>>().mockResolvedValue({
      success: true,
      log_path: '/tmp/test.log',
    });
    const stop = vi.fn<() => Promise<BaseResponse>>().mockResolvedValue({ success: true });

    const { result, rerender } = renderHook(
      ({ isRunning }) => useManagedProcess({
        appName: 'Torch',
        isRunning,
        launch,
        stop,
      }),
      { initialProps: { isRunning: false } }
    );

    await act(async () => {
      await result.current.startProcess();
    });

    expect(result.current.isStarting).toBe(true);
    expect(result.current.launchLogPath).toBe('/tmp/test.log');

    rerender({ isRunning: true });

    expect(result.current.isStarting).toBe(false);
  });

  it('clears failed stop transitions without caller intervention', async () => {
    const launch = vi.fn<() => Promise<LaunchResponse>>().mockResolvedValue({ success: true });
    const stop = vi.fn<() => Promise<BaseResponse>>().mockResolvedValue({
      success: false,
      error: 'failed',
    });

    const { result } = renderHook(() => useManagedProcess({
      appName: 'Ollama',
      isRunning: true,
      launch,
      stop,
    }));

    await act(async () => {
      await result.current.stopProcess();
    });

    expect(result.current.isStopping).toBe(false);
    expect(result.current.launchError).toBe('Failed to stop Ollama');
  });

  it('names why a stop left recorded processes running', async () => {
    const launch = vi.fn<() => Promise<LaunchResponse>>().mockResolvedValue({ success: true });
    const stop = vi.fn<() => Promise<BaseResponse & { skipped: { pid?: number; reason: string }[] }>>()
      .mockResolvedValue({
        success: false,
        skipped: [
          { pid: 4242, reason: 'skipped: not owned by Pumas' },
          { reason: 'the PID record is unreadable' },
        ],
      });

    const { result } = renderHook(() => useManagedProcess({
      appName: 'Torch',
      isRunning: true,
      launch,
      stop,
    }));

    await act(async () => {
      await result.current.stopProcess();
    });

    expect(result.current.launchError).toBe(
      'Failed to stop Torch: skipped: not owned by Pumas; the PID record is unreadable'
    );
  });
});
