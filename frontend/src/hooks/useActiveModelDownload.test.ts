import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  isApiAvailableMock,
  listModelDownloadsMock,
} = vi.hoisted(() => ({
  isApiAvailableMock: vi.fn<() => boolean>(),
  listModelDownloadsMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    list_model_downloads: listModelDownloadsMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

import { useActiveModelDownload } from './useActiveModelDownload';

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useActiveModelDownload', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
    listModelDownloadsMock.mockResolvedValue({
      success: true,
      downloads: [],
    });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('selects the highest-priority active download and exposes the active count', async () => {
    listModelDownloadsMock.mockResolvedValueOnce({
      success: true,
      downloads: [
        {
          repoId: 'repo-queued',
          downloadId: 'dl-queued',
          status: 'queued',
          progress: 90,
        },
        {
          repoId: 'repo-active',
          downloadId: 'dl-active',
          status: 'downloading',
          progress: 45,
          downloadedBytes: 450,
          totalBytes: 1000,
          speed: 64,
          etaSeconds: 12,
        },
        {
          repoId: 'repo-other',
          downloadId: 'dl-pausing',
          status: 'pausing',
          progress: 30,
        },
        {
          repoId: 'repo-complete',
          downloadId: 'dl-done',
          status: 'completed',
          progress: 100,
        },
      ],
    });

    const { result } = renderHook(() => useActiveModelDownload());

    await flushMicrotasks();

    expect(listModelDownloadsMock).toHaveBeenCalledTimes(1);
    expect(result.current.activeDownloadCount).toBe(3);
    expect(result.current.activeDownload).toEqual({
      downloadId: 'dl-active',
      repoId: 'repo-active',
      status: 'downloading',
      progress: 45,
      downloadedBytes: 450,
      totalBytes: 1000,
      speed: 64,
      etaSeconds: 12,
    });
  });

  it('refreshes the active download on the polling interval', async () => {
    listModelDownloadsMock
      .mockResolvedValueOnce({
        success: true,
        downloads: [
          {
            repoId: 'repo-a',
            downloadId: 'dl-a',
            status: 'queued',
            progress: 10,
          },
        ],
      })
      .mockResolvedValueOnce({
        success: true,
        downloads: [
          {
            repoId: 'repo-b',
            downloadId: 'dl-b',
            status: 'downloading',
            progress: 60,
            downloadedBytes: 600,
            totalBytes: 1000,
            speed: 128,
            etaSeconds: 5,
          },
          {
            repoId: 'repo-a',
            downloadId: 'dl-a',
            status: 'queued',
            progress: 10,
          },
        ],
      });

    const { result } = renderHook(() => useActiveModelDownload());

    await flushMicrotasks();

    expect(result.current.activeDownload).toEqual({
      downloadId: 'dl-a',
      repoId: 'repo-a',
      status: 'queued',
      progress: 10,
      downloadedBytes: null,
      totalBytes: null,
      speed: null,
      etaSeconds: null,
    });

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(listModelDownloadsMock).toHaveBeenCalledTimes(2);
    expect(result.current.activeDownloadCount).toBe(2);
    expect(result.current.activeDownload).toEqual({
      downloadId: 'dl-b',
      repoId: 'repo-b',
      status: 'downloading',
      progress: 60,
      downloadedBytes: 600,
      totalBytes: 1000,
      speed: 128,
      etaSeconds: 5,
    });
  });

  it('clears active download state when the API becomes unavailable', async () => {
    listModelDownloadsMock.mockResolvedValueOnce({
      success: true,
      downloads: [
        {
          repoId: 'repo-a',
          downloadId: 'dl-a',
          status: 'downloading',
          progress: 35,
        },
      ],
    });

    const { result } = renderHook(() => useActiveModelDownload());

    await flushMicrotasks();

    expect(result.current.activeDownloadCount).toBe(1);
    expect(result.current.activeDownload).not.toBeNull();

    isApiAvailableMock.mockReturnValue(false);

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(result.current.activeDownloadCount).toBe(0);
    expect(result.current.activeDownload).toBeNull();
  });

  it('clears the active selection when backend polling reports no active downloads', async () => {
    listModelDownloadsMock
      .mockResolvedValueOnce({
        success: true,
        downloads: [
          {
            repoId: 'repo-a',
            downloadId: 'dl-a',
            status: 'downloading',
            progress: 35,
          },
        ],
      })
      .mockResolvedValueOnce({
        success: false,
        downloads: [],
      })
      .mockResolvedValueOnce({
        success: true,
        downloads: [
          {
            repoId: 'repo-a',
            downloadId: 'dl-done',
            status: 'completed',
            progress: 100,
          },
        ],
      });

    const { result } = renderHook(() => useActiveModelDownload());

    await flushMicrotasks();

    expect(result.current.activeDownloadCount).toBe(1);

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(result.current.activeDownloadCount).toBe(0);
    expect(result.current.activeDownload).toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(result.current.activeDownloadCount).toBe(0);
    expect(result.current.activeDownload).toBeNull();
  });
});
