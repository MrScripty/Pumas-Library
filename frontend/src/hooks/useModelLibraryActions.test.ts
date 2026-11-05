import { act, renderHook } from '@testing-library/react';
import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
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
  provenance: 'catalog',
  isPartialDownload: true,
  recovery: { recoveryToken: `v1:${'a'.repeat(64)}`, repoId: 'org/model', selectedArtifactFiles: [] },
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
  it('uses the current model ticket once per model and refuses cached recovery authority', async () => {
    const request = createDeferred<{ success: boolean; action: string; download_id: string }>();
    resumePartialDownloadMock.mockReturnValueOnce(request.promise);
    const { result } = renderLibraryActions();
    let pending: Promise<void> | undefined;
    act(() => {
      pending = result.current.handleRecoverPartialDownload(partialModel());
      void result.current.handleRecoverPartialDownload(partialModel());
      void result.current.handleRecoverPartialDownload(partialModel({ provenance: 'cached' }));
    });
    expect(resumePartialDownloadMock).toHaveBeenCalledExactlyOnceWith('llm/org-model', `v1:${'a'.repeat(64)}`);
    await act(async () => {
      request.resolve({ success: true, action: 'resume', download_id: 'dl-123' });
      await pending;
    });
  });

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

  it('opens remote URLs only through the desktop bridge', async () => {
    const openWindow = vi.fn();
    vi.stubGlobal('open', openWindow);
    const { result, rerender } = renderHook(
      (apiAvailable: boolean) => {
        isApiAvailableMock.mockReturnValue(apiAvailable);
        return useModelLibraryActions({
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

    rerender(false);
    openUrlMock.mockClear();

    act(() => {
      result.current.openRemoteUrl('https://example.com/no-bridge');
    });

    expect(openUrlMock).not.toHaveBeenCalled();
    expect(openWindow).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
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

  it('clears stale errors, tracks recovering model IDs, and starts resumed partial downloads', async () => {
    const recoveryDeferred = createDeferred<{
      success: boolean;
      action: string;
      download_id: string;
    }>();
    resumePartialDownloadMock.mockReturnValueOnce(recoveryDeferred.promise);

    const startDownload = vi.fn();
    const { result } = renderLibraryActions({
      downloadErrors: {
        'llm/org-model': 'Old failure',
      },
      startDownload,
    });

    let recoveryPromise: Promise<void> | undefined;
    await act(async () => {
      recoveryPromise = result.current.handleRecoverPartialDownload(partialModel());
      await Promise.resolve();
    });

    expect(result.current.downloadErrors).toEqual({});
    expect(result.current.recoveringPartialModelIds.has('llm/org-model')).toBe(true);
    expect(resumePartialDownloadMock).toHaveBeenCalledWith('llm/org-model', `v1:${'a'.repeat(64)}`);

    await act(async () => {
      recoveryDeferred.resolve({
        success: true,
        action: 'resume',
        download_id: 'dl-123',
      });
      await recoveryPromise;
    });

    expect(startDownload).toHaveBeenCalledWith('dl-123', 'dl-123', {
      libraryModelId: 'llm/org-model',
      repoId: 'org/model',
      modelName: 'Org Model',
      modelType: 'llm',
    });
    expect(result.current.recoveringPartialModelIds.size).toBe(0);
  });

  it('maps partial recovery failures into user-facing model errors', async () => {
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
      'llm/org-model': 'HuggingFace rate-limited the request. Try again shortly.',
    });
    expect(result.current.recoveringPartialModelIds.size).toBe(0);
  });

  it('deletes the exact model without cancelling a download based on repository or quant labels', async () => {
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

    expect(cancelDownload).not.toHaveBeenCalled();
    expect(deleteModelMock).toHaveBeenCalledWith('llm/org/model');
    expect(onModelsImported).toHaveBeenCalledTimes(1);
  });

  it('owns pending deletion per exact model and allows retry after failure', async () => {
    const first = createDeferred<{ success: boolean; error?: string }>();
    const other = createDeferred<{ success: boolean; error?: string }>();
    deleteModelMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(other.promise);
    const { result, onModelsImported } = renderLibraryActions();
    let pending!: Promise<void>;
    let otherPending!: Promise<void>;
    act(() => {
      pending = result.current.handleDeleteModel('weights-original');
      void result.current.handleDeleteModel('weights-original');
      otherPending = result.current.handleDeleteModel('weights-outq2');
    });
    expect(deleteModelMock.mock.calls).toEqual([['weights-original'], ['weights-outq2']]);
    expect(result.current.deletionStateByModelId).toEqual({
      'weights-original': { status: 'deleting' },
      'weights-outq2': { status: 'deleting' },
    });
    await act(async () => {
      first.resolve({ success: false, error: 'Download library root is busy' });
      await pending;
    });
    expect(result.current.deletionStateByModelId['weights-original']).toEqual({
      status: 'error', message: 'Download library root is busy',
    });
    expect(result.current.deletionStateByModelId['weights-outq2']).toEqual({ status: 'deleting' });
    expect(onModelsImported).not.toHaveBeenCalled();
    await act(async () => {
      await result.current.handleDeleteModel('weights-original');
    });
    expect(result.current.deletionStateByModelId['weights-original']).toBeUndefined();
    expect(onModelsImported).toHaveBeenCalledTimes(1);
    await act(async () => {
      other.resolve({ success: true });
      await otherPending;
    });
    expect(result.current.deletionStateByModelId).toEqual({});
    expect(onModelsImported).toHaveBeenCalledTimes(2);
  });

  it.each([
    { outcome: { success: false }, message: 'The library refused the deletion.' },
    { rejection: 'unexpected transport failure', message: 'The deletion request failed.' },
  ])('reports unavailable failure detail without fabricating success: $message', async ({ outcome, rejection, message }) => {
    if (outcome) deleteModelMock.mockResolvedValueOnce(outcome);
    else deleteModelMock.mockRejectedValueOnce(rejection);
    const { result, onModelsImported } = renderLibraryActions();
    await act(async () => { await result.current.handleDeleteModel('model-1'); });
    expect(result.current.deletionStateByModelId['model-1']).toEqual({ status: 'error', message });
    expect(onModelsImported).not.toHaveBeenCalled();
  });
});
