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

  it('starts unknown rather than reporting 0% before the first reading', () => {
    const { result } = renderHook(() => useDiskSpace());

    expect(result.current.diskSpacePercent).toBeNull();
    expect(result.current.diskSpaceError).toBeNull();
  });

  it('reports the backend as unavailable instead of a 0% reading', async () => {
    isApiAvailableMock.mockReturnValue(false);

    const { result } = renderHook(() => useDiskSpace());

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(getDiskSpaceMock).not.toHaveBeenCalled();
    expect(result.current.diskSpacePercent).toBeNull();
    expect(result.current.diskSpaceError).toBe('The desktop backend is not available.');
  });

  it('keeps the last valid reading and records why a refresh failed', async () => {
    getDiskSpaceMock
      .mockResolvedValueOnce({
        success: true,
        percent: 64,
      })
      .mockRejectedValueOnce(new APIError('disk error', 'get_disk_space'))
      .mockResolvedValueOnce({ success: false, error: 'volume missing' })
      .mockResolvedValueOnce({ success: true, percent: 64 });

    const { result } = renderHook(() => useDiskSpace());

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(result.current.diskSpacePercent).toBe(64);

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(result.current.diskSpacePercent).toBe(64);
    expect(result.current.diskSpaceError).toBe('disk error');

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(result.current.diskSpaceError).toBe('volume missing');

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(result.current.diskSpacePercent).toBe(64);
    expect(result.current.diskSpaceError).toBeNull();
  });

  it('rejects a successful response without a usable percent', async () => {
    getDiskSpaceMock.mockResolvedValueOnce({ success: true });

    const { result } = renderHook(() => useDiskSpace());

    await act(async () => {
      await result.current.fetchDiskSpace();
    });

    expect(result.current.diskSpacePercent).toBeNull();
    expect(result.current.diskSpaceError).toBe('The backend returned an invalid disk usage value.');
  });
});
