import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  cancelModelDownloadMock,
  getElectronAPIMock,
  isApiAvailableMock,
  listModelDownloadsMock,
  pauseModelDownloadMock,
  resumeModelDownloadMock,
} = vi.hoisted(() => ({
  cancelModelDownloadMock: vi.fn(),
  getElectronAPIMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  listModelDownloadsMock: vi.fn(),
  pauseModelDownloadMock: vi.fn(),
  resumeModelDownloadMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    cancel_model_download: cancelModelDownloadMock,
    list_model_downloads: listModelDownloadsMock,
    pause_model_download: pauseModelDownloadMock,
    resume_model_download: resumeModelDownloadMock,
  },
  getElectronAPI: getElectronAPIMock,
  isAPIAvailable: isApiAvailableMock,
}));

import type { ModelDownloadUpdateNotification } from '../types/api';
import { useModelDownloads } from './useModelDownloads';

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useModelDownloads', () => {
  let downloadUpdateCallback: ((notification: ModelDownloadUpdateNotification) => void) | null;
  let unsubscribeMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    downloadUpdateCallback = null;
    unsubscribeMock = vi.fn();
    isApiAvailableMock.mockReturnValue(true);
    getElectronAPIMock.mockReturnValue({
      onModelDownloadUpdate: vi.fn((callback) => {
        downloadUpdateCallback = callback;
        return unsubscribeMock;
      }),
    });
    listModelDownloadsMock.mockResolvedValue({
      success: true,
      downloads: [],
    });
    cancelModelDownloadMock.mockResolvedValue({ success: true });
    pauseModelDownloadMock.mockResolvedValue({ success: true });
    resumeModelDownloadMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('restores tracked downloads and repo-level errors on startup', async () => {
    listModelDownloadsMock.mockResolvedValueOnce({
      success: true,
      downloads: [
        {
          repoId: 'repo-paused',
          selectedArtifactId: 'repo-paused::Q4',
          downloadId: 'dl-paused',
          status: 'paused',
          progress: 42,
          modelName: 'Paused Model',
          modelType: 'checkpoint',
        },
        {
          repoId: 'repo-error',
          downloadId: 'dl-error',
          status: 'error',
          progress: 90,
          error: 'Disk full',
        },
        {
          repoId: 'repo-done',
          downloadId: 'dl-done',
          status: 'completed',
          progress: 100,
        },
      ],
    });

    const { result } = renderHook(() => useModelDownloads());

    await flushMicrotasks();

    expect(listModelDownloadsMock).toHaveBeenCalledTimes(1);
    expect(result.current.downloadStatusByRepo['repo-paused::Q4']).toEqual({
      downloadId: 'dl-paused',
      status: 'paused',
      progress: 42,
      repoId: 'repo-paused',
      selectedArtifactId: 'repo-paused::Q4',
      artifactId: undefined,
      downloadedBytes: undefined,
      totalBytes: undefined,
      speed: undefined,
      etaSeconds: undefined,
      modelName: 'Paused Model',
      modelType: 'checkpoint',
      retryAttempt: undefined,
      retryLimit: undefined,
      retrying: undefined,
      nextRetryDelaySeconds: undefined,
    });
    expect(result.current.downloadStatusByRepo['repo-error']).toEqual({
      downloadId: 'dl-error',
      status: 'error',
      progress: 90,
      repoId: 'repo-error',
      selectedArtifactId: undefined,
      artifactId: undefined,
      downloadedBytes: undefined,
      totalBytes: undefined,
      speed: undefined,
      etaSeconds: undefined,
      modelName: undefined,
      modelType: undefined,
      retryAttempt: undefined,
      retryLimit: undefined,
      retrying: undefined,
      nextRetryDelaySeconds: undefined,
    });
    expect(result.current.downloadErrors).toEqual({
      'repo-error': 'Disk full',
    });
    expect(result.current.hasActiveDownloads).toBe(false);
  });

  it('applies pushed backend progress updates after a local download begins', async () => {
    listModelDownloadsMock.mockResolvedValueOnce({
      success: true,
      downloads: [],
    });

    const { result } = renderHook(() => useModelDownloads());

    await flushMicrotasks();

    act(() => {
      result.current.startDownload('repo-a', 'dl-1', {
        modelName: 'Model A',
        modelType: 'checkpoint',
      });
    });

    expect(result.current.downloadStatusByRepo['repo-a']).toEqual({
      downloadId: 'dl-1',
      status: 'queued',
      progress: 0,
      repoId: 'repo-a',
      selectedArtifactId: undefined,
      artifactId: undefined,
      modelName: 'Model A',
      modelType: 'checkpoint',
    });

    await act(async () => {
      downloadUpdateCallback?.({
        cursor: 'download:2',
        snapshot: {
          cursor: 'download:2',
          revision: 2,
          downloads: [
            {
              repoId: 'repo-a',
              artifactId: 'repo-a::Q4',
              downloadId: 'dl-1',
              status: 'downloading',
              progress: 55,
              downloadedBytes: 550,
              totalBytes: 1000,
              speed: 32,
              etaSeconds: 14,
            },
          ],
        },
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    expect(listModelDownloadsMock).toHaveBeenCalledTimes(1);
    expect(result.current.downloadStatusByRepo['repo-a::Q4']).toEqual(
      expect.objectContaining({
        downloadId: 'dl-1',
        status: 'downloading',
        progress: 55,
        repoId: 'repo-a',
        artifactId: 'repo-a::Q4',
        downloadedBytes: 550,
        totalBytes: 1000,
        speed: 32,
        etaSeconds: 14,
      })
    );
    expect(result.current.hasActiveDownloads).toBe(true);
  });

  it('does not install a polling interval and unsubscribes on unmount', async () => {
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    const { unmount } = renderHook(() => useModelDownloads());

    await flushMicrotasks();

    expect(setIntervalSpy).not.toHaveBeenCalled();

    unmount();
    expect(unsubscribeMock).toHaveBeenCalledTimes(1);
    setIntervalSpy.mockRestore();
  });

  it('tracks same-repo artifact downloads independently and blocks duplicate same-artifact starts', async () => {
    const { result } = renderHook(() => useModelDownloads());

    await flushMicrotasks();

    act(() => {
      result.current.startDownload('org/model::Q4', 'dl-q4', {
        repoId: 'org/model',
        artifactId: 'org/model::Q4',
        modelName: 'Model Q4',
      });
      result.current.startDownload('org/model::Q8', 'dl-q8', {
        repoId: 'org/model',
        selectedArtifactId: 'org/model::Q8',
        modelName: 'Model Q8',
      });
    });

    expect(result.current.downloadStatusByRepo['org/model::Q4']).toEqual({
      downloadId: 'dl-q4',
      status: 'queued',
      progress: 0,
      repoId: 'org/model',
      selectedArtifactId: undefined,
      artifactId: 'org/model::Q4',
      modelName: 'Model Q4',
      modelType: undefined,
    });
    expect(result.current.downloadStatusByRepo['org/model::Q8']).toEqual({
      downloadId: 'dl-q8',
      status: 'queued',
      progress: 0,
      repoId: 'org/model',
      selectedArtifactId: 'org/model::Q8',
      artifactId: undefined,
      modelName: 'Model Q8',
      modelType: undefined,
    });

    act(() => {
      result.current.startDownload('org/model::Q4', 'dl-q4-duplicate', {
        repoId: 'org/model',
        artifactId: 'org/model::Q4',
        modelName: 'Duplicate Q4',
      });
    });

    expect(result.current.downloadStatusByRepo['org/model::Q4']).toEqual({
      downloadId: 'dl-q4',
      status: 'queued',
      progress: 0,
      repoId: 'org/model',
      selectedArtifactId: undefined,
      artifactId: 'org/model::Q4',
      modelName: 'Model Q4',
      modelType: undefined,
    });
    expect(Object.keys(result.current.downloadStatusByRepo).sort()).toEqual([
      'org/model::Q4',
      'org/model::Q8',
    ]);
  });

  it('clears stale errors, protects active downloads from duplicate starts, and routes pause/cancel actions', async () => {
    const { result } = renderHook(() => useModelDownloads());

    await flushMicrotasks();

    act(() => {
      result.current.setDownloadErrors({
        'repo-a': 'Old failure',
      });
      result.current.startDownload('repo-a', 'dl-1', {
        modelName: 'Model A',
      });
    });

    expect(result.current.downloadErrors).toEqual({});
    expect(result.current.downloadStatusByRepo['repo-a']).toEqual({
      downloadId: 'dl-1',
      status: 'queued',
      progress: 0,
      repoId: 'repo-a',
      selectedArtifactId: undefined,
      artifactId: undefined,
      modelName: 'Model A',
      modelType: undefined,
    });

    act(() => {
      result.current.startDownload('repo-a', 'dl-2', {
        modelName: 'Replacement Model',
      });
    });

    expect(result.current.downloadStatusByRepo['repo-a']).toEqual({
      downloadId: 'dl-1',
      status: 'queued',
      progress: 0,
      repoId: 'repo-a',
      selectedArtifactId: undefined,
      artifactId: undefined,
      modelName: 'Model A',
      modelType: undefined,
    });

    await act(async () => {
      await result.current.pauseDownload('repo-a');
    });

    expect(pauseModelDownloadMock).toHaveBeenCalledWith('dl-1');
    expect(result.current.downloadStatusByRepo['repo-a']).toEqual(
      expect.objectContaining({
        downloadId: 'dl-1',
        status: 'pausing',
      })
    );

    await act(async () => {
      await result.current.cancelDownload('repo-a');
    });

    expect(cancelModelDownloadMock).toHaveBeenCalledWith('dl-1');
    expect(result.current.downloadStatusByRepo['repo-a']).toEqual(
      expect.objectContaining({
        downloadId: 'dl-1',
        status: 'cancelling',
      })
    );
  });

  it('marks resumed downloads as failed when the backend resume request rejects', async () => {
    listModelDownloadsMock.mockResolvedValueOnce({
      success: true,
      downloads: [
        {
          repoId: 'repo-paused',
          downloadId: 'dl-paused',
          status: 'paused',
          progress: 25,
        },
      ],
    });
    resumeModelDownloadMock.mockResolvedValueOnce({
      success: false,
      error: 'Resume blocked',
    });

    const { result } = renderHook(() => useModelDownloads());

    await flushMicrotasks();

    act(() => {
      result.current.setDownloadErrors({
        'repo-paused': 'Old failure',
      });
    });

    await act(async () => {
      await result.current.resumeDownload('repo-paused');
    });

    expect(resumeModelDownloadMock).toHaveBeenCalledWith('dl-paused');
    expect(result.current.downloadStatusByRepo['repo-paused']).toEqual(
      expect.objectContaining({
        downloadId: 'dl-paused',
        status: 'error',
      })
    );
    expect(result.current.downloadErrors).toEqual({
      'repo-paused': 'Resume blocked',
    });
  });
});
