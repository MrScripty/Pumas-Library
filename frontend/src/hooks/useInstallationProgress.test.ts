import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import type { InstallationProgress } from './useVersions';

const {
  getInstallationProgressMock,
} = vi.hoisted(() => ({
  getInstallationProgressMock: vi.fn<
    (_appId: string) => Promise<InstallationProgress | null>
  >(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_installation_progress: getInstallationProgressMock,
  },
}));

import { useInstallationProgress } from './useInstallationProgress';

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

const baseProgress: InstallationProgress = {
  tag: 'v1.2.3',
  started_at: '2026-04-12T00:00:00Z',
  stage: 'download',
  stage_progress: 50,
  overall_progress: 25,
  current_item: 'archive.zip',
  download_speed: 1024,
  eta_seconds: 30,
  total_size: 4096,
  downloaded_bytes: 1024,
  dependency_count: null,
  completed_dependencies: 0,
  completed_items: [],
  error: null,
};

describe('useInstallationProgress', () => {
  beforeEach(() => {
    getInstallationProgressMock.mockReset();
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('syncs external progress and tracks failed installs until the same tag succeeds', async () => {
    const failedProgress: InstallationProgress = {
      ...baseProgress,
      completed_at: '2026-04-12T00:05:00Z',
      success: false,
      error: 'Install failed',
      log_path: '/tmp/install.log',
    };

    const { result, rerender } = renderHook(
      (props: { externalProgress?: InstallationProgress | null }) => useInstallationProgress({
        installingVersion: 'v1.2.3',
        externalProgress: props.externalProgress,
        onRefreshProgress: async () => null,
      }),
      {
        initialProps: { externalProgress: failedProgress },
      }
    );

    await flushMicrotasks();

    expect(result.current.progress).toEqual(failedProgress);
    expect(result.current.failedInstall).toEqual({
      tag: 'v1.2.3',
      log: '/tmp/install.log',
    });

    const successfulProgress: InstallationProgress = {
      ...failedProgress,
      success: true,
      error: null,
    };

    rerender({ externalProgress: successfulProgress });

    await flushMicrotasks();

    expect(result.current.failedInstall).toBeNull();
  });

  it('polls local installation progress, reports cancellation, and stops polling after completion', async () => {
    vi.useFakeTimers();

    const cancelledCompletedProgress: InstallationProgress = {
      ...baseProgress,
      completed_at: '2026-04-12T00:06:00Z',
      success: false,
      error: 'User cancelled installation',
    };

    getInstallationProgressMock
      .mockResolvedValueOnce(cancelledCompletedProgress)
      .mockResolvedValue(cancelledCompletedProgress);

    const { result } = renderHook(() => useInstallationProgress({
      appId: 'torch',
      installingVersion: 'v1.2.3',
    }));

    await flushMicrotasks();

    expect(result.current.progress).toEqual(cancelledCompletedProgress);
    expect(result.current.cancellationNotice).toBe('Installation canceled');

    expect(getInstallationProgressMock).toHaveBeenCalledWith('torch');
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(1000);
    });

    expect(getInstallationProgressMock).toHaveBeenCalledTimes(2);

    await act(async () => {
      vi.advanceTimersByTime(1000);
    });

    expect(getInstallationProgressMock).toHaveBeenCalledTimes(2);

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(result.current.cancellationNotice).toBeNull();
  });

  it('skips local polling when external refresh orchestration is provided', async () => {
    vi.useFakeTimers();

    const { result } = renderHook(() => useInstallationProgress({
      installingVersion: 'v1.2.3',
      externalProgress: baseProgress,
      onRefreshProgress: async () => null,
    }));

    await flushMicrotasks();

    expect(result.current.progress).toEqual(baseProgress);

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(getInstallationProgressMock).not.toHaveBeenCalled();
  });

  it('handles manual cancellation notices and swallows API polling errors', async () => {
    vi.useFakeTimers();

    getInstallationProgressMock.mockRejectedValue(new APIError('failed', 'get_installation_progress'));

    const { result } = renderHook(() => useInstallationProgress({
      installingVersion: 'v1.2.3',
    }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.progress).toBeNull();

    act(() => {
      result.current.showCancellationNotice();
    });

    expect(result.current.cancellationNotice).toBe('Installation canceled');

    await act(async () => {
      vi.advanceTimersByTime(3000);
    });

    expect(result.current.cancellationNotice).toBeNull();
  });
});
