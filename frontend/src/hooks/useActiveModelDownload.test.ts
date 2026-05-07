import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  getElectronAPIMock,
  isApiAvailableMock,
  listModelDownloadsMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  listModelDownloadsMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    list_model_downloads: listModelDownloadsMock,
  },
  getElectronAPI: getElectronAPIMock,
  isAPIAvailable: isApiAvailableMock,
}));

import type { ModelDownloadUpdateNotification } from '../types/api';
import { useActiveModelDownload } from './useActiveModelDownload';

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useActiveModelDownload', () => {
  let downloadUpdateCallback: ((notification: ModelDownloadUpdateNotification) => void) | null;
  let unsubscribeMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    downloadUpdateCallback = null;
    unsubscribeMock = vi.fn();
    isApiAvailableMock.mockReturnValue(true);
    getElectronAPIMock.mockReturnValue({
      onModelDownloadUpdate: vi.fn((callback: (notification: ModelDownloadUpdateNotification) => void) => {
        downloadUpdateCallback = callback;
        return unsubscribeMock;
      }),
    });
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

  it('reports aggregate speed across active downloads', async () => {
    listModelDownloadsMock.mockResolvedValueOnce({
      success: true,
      downloads: [
        {
          repoId: 'repo-a',
          downloadId: 'dl-a',
          status: 'downloading',
          progress: 45,
          downloadedBytes: 450,
          totalBytes: 1000,
          speed: 64,
          etaSeconds: 12,
        },
        {
          repoId: 'repo-b',
          downloadId: 'dl-b',
          status: 'downloading',
          progress: 20,
          downloadedBytes: 200,
          totalBytes: 1000,
          speed: 32,
          etaSeconds: 20,
        },
      ],
    });

    const { result } = renderHook(() => useActiveModelDownload());

    await flushMicrotasks();

    expect(result.current.activeDownloadCount).toBe(2);
    expect(result.current.activeDownload?.downloadId).toBe('dl-a');
    expect(result.current.activeDownload?.speed).toBe(96);
  });

  it('refreshes the active download from pushed snapshots', async () => {
    listModelDownloadsMock.mockResolvedValueOnce({
      success: true,
      downloads: [
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
      downloadUpdateCallback?.({
        cursor: 'download:2',
        snapshot: {
          cursor: 'download:2',
          revision: 2,
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
        },
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    expect(listModelDownloadsMock).toHaveBeenCalledTimes(1);
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

  it('clears active download state when pushed snapshot is empty', async () => {
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

    await act(async () => {
      downloadUpdateCallback?.({
        cursor: 'download:2',
        snapshot: {
          cursor: 'download:2',
          revision: 2,
          downloads: [],
        },
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    expect(result.current.activeDownloadCount).toBe(0);
    expect(result.current.activeDownload).toBeNull();
  });

  it('clears the active selection when pushed snapshot has no active downloads', async () => {
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

    await act(async () => {
      downloadUpdateCallback?.({
        cursor: 'download:2',
        snapshot: {
          cursor: 'download:2',
          revision: 2,
          downloads: [
            {
              repoId: 'repo-a',
              downloadId: 'dl-done',
              status: 'completed',
              progress: 100,
            },
          ],
        },
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    expect(result.current.activeDownloadCount).toBe(0);
    expect(result.current.activeDownload).toBeNull();
  });

  it('does not install a polling interval and unsubscribes on unmount', async () => {
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    const { unmount } = renderHook(() => useActiveModelDownload());

    await flushMicrotasks();

    expect(setIntervalSpy).not.toHaveBeenCalled();

    unmount();
    expect(unsubscribeMock).toHaveBeenCalledTimes(1);
    setIntervalSpy.mockRestore();
  });
});
