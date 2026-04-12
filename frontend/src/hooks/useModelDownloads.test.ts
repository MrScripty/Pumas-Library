import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  cancelModelDownloadMock,
  isApiAvailableMock,
  listModelDownloadsMock,
  pauseModelDownloadMock,
  resumeModelDownloadMock,
} = vi.hoisted(() => ({
  cancelModelDownloadMock: vi.fn(),
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
  isAPIAvailable: isApiAvailableMock,
}));

import { useModelDownloads } from './useModelDownloads';

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useModelDownloads', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
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
    expect(result.current.downloadStatusByRepo['repo-paused']).toEqual({
      downloadId: 'dl-paused',
      status: 'paused',
      progress: 42,
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

  it('starts polling after a local download begins and applies backend progress updates', async () => {
    listModelDownloadsMock
      .mockResolvedValueOnce({
        success: true,
        downloads: [],
      })
      .mockResolvedValueOnce({
        success: true,
        downloads: [
          {
            repoId: 'repo-a',
            downloadId: 'dl-1',
            status: 'downloading',
            progress: 55,
            downloadedBytes: 550,
            totalBytes: 1000,
            speed: 32,
            etaSeconds: 14,
          },
        ],
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
      modelName: 'Model A',
      modelType: 'checkpoint',
    });

    await act(async () => {
      vi.advanceTimersByTime(800);
      await Promise.resolve();
    });

    expect(listModelDownloadsMock).toHaveBeenCalledTimes(2);
    expect(result.current.downloadStatusByRepo['repo-a']).toEqual(
      expect.objectContaining({
        downloadId: 'dl-1',
        status: 'downloading',
        progress: 55,
        downloadedBytes: 550,
        totalBytes: 1000,
        speed: 32,
        etaSeconds: 14,
      })
    );
    expect(result.current.hasActiveDownloads).toBe(true);
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
