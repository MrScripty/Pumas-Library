import { createElement } from 'react';
import { act, render, renderHook, screen } from '@testing-library/react';
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
import type { DownloadProgressOutcome } from '../generated/desktop-contract';
import { useActiveModelDownload } from './useActiveModelDownload';
import { Header } from '../components/Header';

function progressOutcome(overrides: Partial<DownloadProgressOutcome>): DownloadProgressOutcome {
  return {
    downloadId: 'dl-1', status: 'queued', repoId: null, selectedArtifactId: null,
    libraryModelId: null, progress: null, downloadedBytes: null, totalBytes: null, speed: null,
    etaSeconds: null, modelName: null, modelType: null, retryAttempt: null, retryLimit: null,
    retrying: null, nextRetryDelaySeconds: null, error: null, ...overrides,
  };
}

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

  it.each(['completed', 'empty'] as const)('keeps a pushed %s snapshot when the older startup list resolves late', async (terminal) => {
    let resolveList!: (value: { success: true; downloads: DownloadProgressOutcome[] }) => void;
    listModelDownloadsMock.mockReturnValueOnce(new Promise(resolve => { resolveList = resolve; }));
    const { result, unmount } = renderHook(() => useActiveModelDownload());
    act(() => downloadUpdateCallback?.({
      cursor: 'download:2', snapshot: { cursor: 'download:2', revision: 2,
        downloads: terminal === 'empty' ? [] : [progressOutcome({ status: 'completed', progress: 1 })],
      }, stale_cursor: false, snapshot_required: false,
    }));
    await act(async () => resolveList({ success: true, downloads: [
      progressOutcome({ status: 'downloading', progress: 0.42 }),
    ] }));
    expect(result.current.activeDownload).toBeNull();
    expect(result.current.activeDownloadCount).toBe(0);
    expect(listModelDownloadsMock).toHaveBeenCalledTimes(1);
    unmount();
    expect(unsubscribeMock).toHaveBeenCalledTimes(1);
  });

  it('keeps the rendered header active at byte completion and idle only after terminal status, even with a late startup list', async () => {
    let resolveList!: (value: { success: true; downloads: DownloadProgressOutcome[] }) => void;
    listModelDownloadsMock.mockReturnValueOnce(new Promise(resolve => { resolveList = resolve; }));
    function DownloadHeader() {
      const { activeDownload, activeDownloadCount } = useActiveModelDownload();
      return createElement(Header, {
        launcherUpdateAvailable: false, onMinimize: vi.fn(), onClose: vi.fn(),
        networkAvailable: true, modelLibraryLoaded: true,
        activeModelDownload: activeDownload, activeModelDownloadCount: activeDownloadCount,
      });
    }
    const { unmount } = render(createElement(DownloadHeader));
    const push = (status: 'downloading' | 'completed') => downloadUpdateCallback?.({
      cursor: 'download:2', snapshot: { cursor: 'download:2', revision: 2,
        downloads: [progressOutcome({ status, progress: 1, downloadedBytes: 1000, totalBytes: 1000 })],
      }, stale_cursor: false, snapshot_required: false,
    });
    act(() => push('downloading'));
    expect(screen.getByRole('status')).toHaveTextContent('Downloading 1 model');
    act(() => push('completed'));
    expect(screen.getByRole('status')).toHaveTextContent('Network online · model library ready');
    await act(async () => resolveList({ success: true, downloads: [progressOutcome({ status: 'downloading', progress: 0.42 })] }));
    expect(screen.getByRole('status')).toHaveTextContent('Network online · model library ready');
    unmount();
    expect(unsubscribeMock).toHaveBeenCalledTimes(1);
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
            progressOutcome({
              repoId: 'repo-b',
              downloadId: 'dl-b',
              status: 'downloading',
              progress: 60,
              downloadedBytes: 600,
              totalBytes: 1000,
              speed: 128,
              etaSeconds: 5,
            }),
            progressOutcome({
              repoId: 'repo-a',
              downloadId: 'dl-a',
              status: 'queued',
              progress: 10,
            }),
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
            progressOutcome({
              repoId: 'repo-a',
              downloadId: 'dl-done',
              status: 'completed',
              progress: 100,
            }),
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
