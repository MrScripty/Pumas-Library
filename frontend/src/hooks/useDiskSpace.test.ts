import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';

const {
  getDiskSpaceMock,
  isApiAvailableMock,
} = vi.hoisted(() => ({
  getDiskSpaceMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_disk_space: getDiskSpaceMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

import { useDiskSpace } from './useDiskSpace';

describe('useDiskSpace', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    getDiskSpaceMock.mockResolvedValue({
      success: true,
      percent: 73,
    });
  });

  it('updates disk-space percent when the backend succeeds', async () => {
    const { result } = renderHook(() => useDiskSpace());

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(getDiskSpaceMock).toHaveBeenCalledTimes(1);
    expect(result.current.diskSpacePercent).toBe(73);
  });

  it('does nothing when the API is unavailable', async () => {
    isApiAvailableMock.mockReturnValue(false);

    const { result } = renderHook(() => useDiskSpace());

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(getDiskSpaceMock).not.toHaveBeenCalled();
    expect(result.current.diskSpacePercent).toBe(0);
  });

  it('swallows backend failures and leaves the last disk-space value intact', async () => {
    getDiskSpaceMock
      .mockResolvedValueOnce({
        success: true,
        percent: 64,
      })
      .mockRejectedValueOnce(new APIError('disk error', 'get_disk_space'));

    const { result } = renderHook(() => useDiskSpace());

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(result.current.diskSpacePercent).toBe(64);

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(result.current.diskSpacePercent).toBe(64);
  });
});
