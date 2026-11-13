import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { InstallationProgress } from './useVersions';
import { useInstallationProgress } from './useInstallationProgress';

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
  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('projects external progress and clears it when the manager clears', async () => {
    const { result, rerender } = renderHook(
      ({ externalProgress }) => useInstallationProgress({ externalProgress }),
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-type-assertion -- widens the inferred type that later updates rely on
      { initialProps: { externalProgress: baseProgress as InstallationProgress | null } }
    );

    expect(result.current.progress).toEqual(baseProgress);

    rerender({ externalProgress: null });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.progress).toBeNull();
  });

  it('tracks failed installs until the same tag succeeds', async () => {
    const failedProgress: InstallationProgress = {
      ...baseProgress,
      completed_at: '2026-04-12T00:05:00Z',
      success: false,
      error: 'Install failed',
      log_path: '/tmp/install.log',
    };

    const { result, rerender } = renderHook(
      ({ externalProgress }) => useInstallationProgress({ externalProgress }),
      { initialProps: { externalProgress: failedProgress } }
    );

    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.failedInstall).toEqual({
      tag: 'v1.2.3',
      log: '/tmp/install.log',
    });

    rerender({
      externalProgress: {
        ...failedProgress,
        success: true,
        error: null,
      },
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.failedInstall).toBeNull();
  });

  it('keeps cancellation distinct from failure and expires its notice', async () => {
    vi.useFakeTimers();
    const cancelledProgress: InstallationProgress = {
      ...baseProgress,
      completed_at: '2026-04-12T00:05:00Z',
      success: false,
      error: 'User cancelled installation',
    };

    const { result } = renderHook(() => useInstallationProgress({
      externalProgress: cancelledProgress,
    }));

    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.cancellationNotice).toBe('Installation canceled');
    expect(result.current.failedInstall).toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(result.current.cancellationNotice).toBeNull();
  });

  it('supports an immediate local cancellation notice without polling', () => {
    vi.useFakeTimers();
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    const { result } = renderHook(() => useInstallationProgress({
      externalProgress: null,
    }));

    act(() => {
      result.current.showCancellationNotice();
    });

    expect(result.current.cancellationNotice).toBe('Installation canceled');
    expect(setIntervalSpy).not.toHaveBeenCalled();
    setIntervalSpy.mockRestore();
  });
});
