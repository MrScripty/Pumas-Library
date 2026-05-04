import { act, renderHook } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelInfo, RemoteModelInfo } from '../types/apps';
import type { DownloadStatus } from './modelDownloadState';

const {
  deleteModelMock,
  getRelatedModelsMock,
  isApiAvailableMock,
  openUrlMock,
  resumePartialDownloadMock,
} = vi.hoisted(() => ({
  deleteModelMock: vi.fn(),
  getRelatedModelsMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  openUrlMock: vi.fn(),
  resumePartialDownloadMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    open_url: openUrlMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

vi.mock('../api/models', () => ({
  modelsAPI: {
    deleteModel: deleteModelMock,
    getRelatedModels: getRelatedModelsMock,
    resumePartialDownload: resumePartialDownloadMock,
  },
}));

import { useModelLibraryActions } from './useModelLibraryActions';

const relatedModel = (overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo => ({
  repoId: 'org/related-model',
  name: 'Related Model',
  developer: 'org',
  kind: 'text-generation',
  formats: ['gguf'],
  quants: ['Q4_K_M'],
  url: 'https://huggingface.co/org/related-model',
  ...overrides,
});

const partialModel = (overrides: Partial<ModelInfo> = {}): ModelInfo => ({
  id: 'llm/org-model',
  name: 'Org Model',
  category: 'llm',
  modelDir: '/models/org-model',
  repoId: 'org/model',
  ...overrides,
});

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function renderLibraryActions(options?: {
  downloadErrors?: Record<string, string>;
  downloadStatusByRepo?: Record<string, DownloadStatus>;
  cancelDownload?: (repoId: string) => Promise<void> | void;
  onModelsImported?: () => void;
  startDownload?: (
    repoId: string,
    downloadId: string,
    details?: { modelName?: string; modelType?: string }
  ) => void;
}) {
  const cancelDownload = options?.cancelDownload ?? vi.fn();
  const onModelsImported = options?.onModelsImported ?? vi.fn();
  const startDownload = options?.startDownload ?? vi.fn();

  const hook = renderHook(() => {
    const [downloadErrors, setDownloadErrors] = useState<Record<string, string>>(
      options?.downloadErrors ?? {}
    );

    return {
      downloadErrors,
      ...useModelLibraryActions({
        downloadStatusByRepo: options?.downloadStatusByRepo ?? {},
        cancelDownload,
        onModelsImported,
        setDownloadErrors,
        startDownload,
      }),
    };
  });

  return {
    ...hook,
    cancelDownload,
    onModelsImported,
    startDownload,
  };
}

describe('useModelLibraryActions', () => {
  const windowOpenSpy = vi.spyOn(window, 'open').mockImplementation(() => null);

  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    getRelatedModelsMock.mockResolvedValue({
      success: true,
      models: [relatedModel()],
    });
    resumePartialDownloadMock.mockResolvedValue({
      success: true,
      action: 'resume',
      download_id: 'dl-123',
    });
    deleteModelMock.mockResolvedValue({
      success: true,
    });
  });

  afterEach(() => {
    windowOpenSpy.mockClear();
  });

  it('opens remote URLs through the backend when available and falls back to window.open otherwise', async () => {
    const { result, rerender } = renderHook(
      (apiAvailable: boolean) => {
        isApiAvailableMock.mockReturnValue(apiAvailable);
        return useModelLibraryActions({
          downloadStatusByRepo: {},
          cancelDownload: vi.fn(),
          setDownloadErrors: vi.fn(),
          startDownload: vi.fn(),
        });
      },
      {
        initialProps: true,
      }
    );

    act(() => {
      result.current.openRemoteUrl('https://huggingface.co/org/model');
    });

    expect(openUrlMock).toHaveBeenCalledWith('https://huggingface.co/org/model');
    expect(windowOpenSpy).not.toHaveBeenCalled();

    rerender(false);

    act(() => {
      result.current.openRemoteUrl('https://example.com/fallback');
    });

    expect(windowOpenSpy).toHaveBeenCalledWith(
      'https://example.com/fallback',
      '_blank',
      'noopener'
    );
  });

  it('loads related models on first expand and reuses the loaded result on later toggles', async () => {
    const relatedDeferred = createDeferred<{
      success: boolean;
      models: RemoteModelInfo[];
    }>();
    getRelatedModelsMock.mockReturnValueOnce(relatedDeferred.promise);

    const { result } = renderLibraryActions();

    await act(async () => {
      result.current.handleToggleRelated('llm/org-model');
      await Promise.resolve();
    });

    expect(result.current.expandedRelated.has('llm/org-model')).toBe(true);
    expect(result.current.relatedModelsById['llm/org-model']).toEqual({
      status: 'loading',
      models: [],
    });
    expect(getRelatedModelsMock).toHaveBeenCalledTimes(1);
    expect(getRelatedModelsMock).toHaveBeenCalledWith('llm/org-model', 25);

    await act(async () => {
      relatedDeferred.resolve({
        success: true,
        models: [relatedModel()],
      });
      await Promise.resolve();
    });

    expect(result.current.relatedModelsById['llm/org-model']).toEqual({
      status: 'loaded',
      models: [relatedModel()],
    });

    act(() => {
      result.current.handleToggleRelated('llm/org-model');
    });

    expect(result.current.expandedRelated.has('llm/org-model')).toBe(false);

    act(() => {
      result.current.handleToggleRelated('llm/org-model');
    });

    expect(result.current.expandedRelated.has('llm/org-model')).toBe(true);
    expect(getRelatedModelsMock).toHaveBeenCalledTimes(1);
  });

  it('surfaces an unavailable related-models state when the API is offline', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const { result } = renderLibraryActions();

    await act(async () => {
      result.current.handleToggleRelated('llm/offline-model');
      await Promise.resolve();
    });

    expect(result.current.relatedModelsById['llm/offline-model']).toEqual({
      status: 'error',
      models: [],
      error: 'Related models unavailable.',
    });
    expect(getRelatedModelsMock).not.toHaveBeenCalled();
  });

  it('clears stale errors, tracks recovering repo IDs, and starts resumed partial downloads', async () => {
    const recoveryDeferred = createDeferred<{
      success: boolean;
      action: string;
      download_id: string;
    }>();
    resumePartialDownloadMock.mockReturnValueOnce(recoveryDeferred.promise);

    const startDownload = vi.fn();
    const { result } = renderLibraryActions({
      downloadErrors: {
        'org/model': 'Old failure',
      },
      startDownload,
    });

    let recoveryPromise: Promise<void> | undefined;
    await act(async () => {
      recoveryPromise = result.current.handleRecoverPartialDownload(partialModel());
      await Promise.resolve();
    });

    expect(result.current.downloadErrors).toEqual({});
    expect(result.current.recoveringPartialRepoIds.has('org/model')).toBe(true);
    expect(resumePartialDownloadMock).toHaveBeenCalledWith('org/model', '/models/org-model');

    await act(async () => {
      recoveryDeferred.resolve({
        success: true,
        action: 'resume',
        download_id: 'dl-123',
      });
      await recoveryPromise;
    });

    expect(startDownload).toHaveBeenCalledWith('org/model', 'dl-123', {
      repoId: 'org/model',
      modelName: 'Org Model',
      modelType: 'llm',
    });
    expect(result.current.recoveringPartialRepoIds.size).toBe(0);
  });

  it('maps partial recovery failures into user-facing repo errors', async () => {
    resumePartialDownloadMock.mockResolvedValueOnce({
      success: false,
      action: 'none',
      reason_code: 'rate_limited',
      error: 'backend said no',
    });

    const startDownload = vi.fn();
    const { result } = renderLibraryActions({
      startDownload,
    });

    await act(async () => {
      await result.current.handleRecoverPartialDownload(partialModel());
    });

    expect(startDownload).not.toHaveBeenCalled();
    expect(result.current.downloadErrors).toEqual({
      'org/model': 'HuggingFace rate-limited the request. Try again shortly.',
    });
    expect(result.current.recoveringPartialRepoIds.size).toBe(0);
  });

  it('cancels matching downloads before delete and refreshes models after successful deletion', async () => {
    const cancelDownload = vi.fn().mockResolvedValue(undefined);
    const onModelsImported = vi.fn();
    const { result } = renderLibraryActions({
      cancelDownload,
      onModelsImported,
      downloadStatusByRepo: {
        'org/model::Q4': {
          downloadId: 'dl-123',
          status: 'downloading',
          progress: 55,
          repoId: 'org/model',
        },
        'other/repo': {
          downloadId: 'dl-999',
          status: 'completed',
          progress: 100,
        },
      },
    });

    await act(async () => {
      await result.current.handleDeleteModel('llm/org/model');
    });

    expect(cancelDownload).toHaveBeenCalledWith('org/model::Q4');
    expect(deleteModelMock).toHaveBeenCalledWith('llm/org/model');
    expect(onModelsImported).toHaveBeenCalledTimes(1);
  });
});
