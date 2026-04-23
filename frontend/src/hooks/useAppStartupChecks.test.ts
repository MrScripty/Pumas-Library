import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppStartupChecks } from './useAppStartupChecks';

describe('useAppStartupChecks', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('waits for the API before fetching disk space and checking launcher updates', async () => {
    const checkLauncherVersion = vi.fn().mockResolvedValue(undefined);
    const fetchDiskSpace = vi.fn();
    const refetchStatus = vi.fn();
    const isApiAvailable = vi.fn()
      .mockReturnValueOnce(false)
      .mockReturnValueOnce(true);

    renderHook(() => useAppStartupChecks({
      checkLauncherVersion,
      fetchDiskSpace,
      isApiAvailable,
      refetchStatus,
    }));

    expect(fetchDiskSpace).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(fetchDiskSpace).toHaveBeenCalledTimes(1);
    expect(checkLauncherVersion).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });

    expect(checkLauncherVersion).toHaveBeenCalledWith(false);
  });

  it('clears scheduled update checks on unmount', () => {
    const checkLauncherVersion = vi.fn().mockResolvedValue(undefined);
    const { unmount } = renderHook(() => useAppStartupChecks({
      checkLauncherVersion,
      fetchDiskSpace: vi.fn(),
      isApiAvailable: () => true,
      refetchStatus: vi.fn(),
    }));

    unmount();

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(checkLauncherVersion).not.toHaveBeenCalled();
  });

  it('refetches status when an active version is available and the API is ready', () => {
    const refetchStatus = vi.fn();

    renderHook(() => useAppStartupChecks({
      activeVersion: 'v1',
      checkLauncherVersion: vi.fn().mockResolvedValue(undefined),
      fetchDiskSpace: vi.fn(),
      isApiAvailable: () => true,
      refetchStatus,
    }));

    expect(refetchStatus).toHaveBeenCalledWith(false);
  });
});
