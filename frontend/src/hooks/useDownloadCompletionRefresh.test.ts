import { renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadStatus } from './modelDownloadState';
import { useDownloadCompletionRefresh } from './useDownloadCompletionRefresh';

function downloadStatus(status: DownloadStatus['status']): DownloadStatus {
  return {
    downloadId: 'download-1',
    status,
    progress: status === 'completed' ? 1 : 0.5,
  };
}

describe('useDownloadCompletionRefresh', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('refreshes models after a tracked download completes', () => {
    const onModelsImported = vi.fn();
    const { rerender } = renderHook(
      ({ statuses }) => useDownloadCompletionRefresh({
        delayMs: 100,
        downloadStatusByRepo: statuses,
        onModelsImported,
      }),
      { initialProps: { statuses: { 'org/model': downloadStatus('downloading') } } }
    );

    rerender({ statuses: { 'org/model': downloadStatus('completed') } });

    expect(onModelsImported).not.toHaveBeenCalled();
    vi.advanceTimersByTime(100);

    expect(onModelsImported).toHaveBeenCalledTimes(1);
  });

  it('refreshes models when a queued or downloading entry disappears', () => {
    const onModelsImported = vi.fn();
    const { rerender } = renderHook(
      ({ statuses }) => useDownloadCompletionRefresh({
        delayMs: 100,
        downloadStatusByRepo: statuses,
        onModelsImported,
      }),
      {
        initialProps: {
          statuses: { 'org/model': downloadStatus('queued') } as Record<string, DownloadStatus>,
        },
      }
    );

    rerender({ statuses: {} as Record<string, DownloadStatus> });
    vi.advanceTimersByTime(100);

    expect(onModelsImported).toHaveBeenCalledTimes(1);
  });

  it('does not refresh for an initially completed download', () => {
    const onModelsImported = vi.fn();
    renderHook(() => useDownloadCompletionRefresh({
      delayMs: 100,
      downloadStatusByRepo: { 'org/model': downloadStatus('completed') },
      onModelsImported,
    }));

    vi.advanceTimersByTime(100);

    expect(onModelsImported).not.toHaveBeenCalled();
  });

  it('clears a pending refresh timer on unmount', () => {
    const onModelsImported = vi.fn();
    const { rerender, unmount } = renderHook(
      ({ statuses }) => useDownloadCompletionRefresh({
        delayMs: 100,
        downloadStatusByRepo: statuses,
        onModelsImported,
      }),
      { initialProps: { statuses: { 'org/model': downloadStatus('downloading') } } }
    );

    rerender({ statuses: { 'org/model': downloadStatus('completed') } });
    unmount();
    vi.advanceTimersByTime(100);

    expect(onModelsImported).not.toHaveBeenCalled();
  });
});
