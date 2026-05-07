import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../types/apps';
import type { ModelLibraryUpdateNotification, ModelRecord } from '../types/api';

const {
  getModelsMock,
  getElectronAPIMock,
  groupModelRecordsMock,
  isApiAvailableMock,
  scanSharedStorageMock,
  searchModelsFTSMock,
} = vi.hoisted(() => ({
  getModelsMock: vi.fn(),
  getElectronAPIMock: vi.fn(),
  groupModelRecordsMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  scanSharedStorageMock: vi.fn(),
  searchModelsFTSMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
  isAPIAvailable: isApiAvailableMock,
}));

vi.mock('../api/models', () => ({
  modelsAPI: {
    getModels: getModelsMock,
    scanSharedStorage: scanSharedStorageMock,
  },
}));

vi.mock('../api/import', () => ({
  importAPI: {
    searchModelsFTS: searchModelsFTSMock,
  },
}));

vi.mock('../utils/libraryModels', () => ({
  groupModelRecords: groupModelRecordsMock,
}));

import { useModels } from './useModels';

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const makeRecord = (id: string, modelType = 'checkpoint'): ModelRecord => ({
  id,
  path: `/models/${id}.gguf`,
  modelType,
  officialName: id,
  tags: [],
  hashes: {},
  metadata: {},
  updatedAt: '2026-04-12T00:00:00Z',
});

function grouped(ids: string[], category = 'grouped'): ModelCategory[] {
  return [
    {
      category,
      models: ids.map((id) => ({
        id,
        name: id,
        category,
      })),
    },
  ];
}

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useModels', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    getElectronAPIMock.mockReturnValue(null);
    isApiAvailableMock.mockReturnValue(true);
    getModelsMock.mockResolvedValue({
      success: true,
      models: {
        alpha: makeRecord('alpha'),
      },
    });
    scanSharedStorageMock.mockResolvedValue({
      success: true,
      result: {
        modelsFound: 1,
      },
    });
    searchModelsFTSMock.mockResolvedValue({
      success: true,
      models: [makeRecord('search-hit')],
      query_time_ms: 12,
    });
    groupModelRecordsMock.mockImplementation((records: ModelRecord[]) =>
      grouped(records.map((record) => record.id))
    );
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('fetches and groups models on mount when the API is available', async () => {
    const { result } = renderHook(() => useModels());

    await flushMicrotasks();

    expect(getModelsMock).toHaveBeenCalledTimes(1);
    expect(groupModelRecordsMock).toHaveBeenCalledWith([makeRecord('alpha')]);
    expect(result.current.modelGroups).toEqual(grouped(['alpha']));
  });

  it('rescans shared storage and refreshes grouped models after a successful scan', async () => {
    getModelsMock
      .mockResolvedValueOnce({
        success: true,
        models: {
          alpha: makeRecord('alpha'),
        },
      })
      .mockResolvedValueOnce({
        success: true,
        models: {
          beta: makeRecord('beta'),
        },
      });

    const { result } = renderHook(() => useModels());

    await flushMicrotasks();

    await act(async () => {
      await result.current.scanModels();
    });

    expect(scanSharedStorageMock).toHaveBeenCalledTimes(1);
    expect(getModelsMock).toHaveBeenCalledTimes(2);
    expect(result.current.modelGroups).toEqual(grouped(['beta']));
  });

  it('shows cached search results immediately, revalidates in the background, and notifies when they change', async () => {
    searchModelsFTSMock
      .mockResolvedValueOnce({
        success: true,
        models: [makeRecord('alpha-result')],
        query_time_ms: 10,
      })
      .mockResolvedValueOnce({
        success: true,
        models: [makeRecord('beta-result')],
        query_time_ms: 15,
      });

    const { result } = renderHook(() => useModels());

    await flushMicrotasks();

    act(() => {
      result.current.searchModelsFTS('alpha', 'checkpoint', ['tag-b', 'tag-a']);
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['alpha-result']));
    expect(result.current.searchQueryTime).toBe(10);
    expect(result.current.hasNewResults).toBe(false);

    act(() => {
      result.current.searchModelsFTS('alpha', 'checkpoint', ['tag-a', 'tag-b']);
    });

    expect(result.current.modelGroups).toEqual(grouped(['alpha-result']));
    expect(result.current.isSearching).toBe(false);
    expect(result.current.isRevalidating).toBe(true);

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['beta-result']));
    expect(result.current.searchQueryTime).toBe(15);
    expect(result.current.hasNewResults).toBe(true);

    act(() => {
      result.current.dismissNewResults();
    });

    expect(result.current.hasNewResults).toBe(false);
  });

  it('ignores stale search responses from older queries and from cleared searches', async () => {
    const firstSearch = createDeferred<{
      success: boolean;
      models: ModelRecord[];
      query_time_ms: number;
    }>();
    const secondSearch = createDeferred<{
      success: boolean;
      models: ModelRecord[];
      query_time_ms: number;
    }>();

    searchModelsFTSMock
      .mockReturnValueOnce(firstSearch.promise)
      .mockReturnValueOnce(secondSearch.promise);

    const { result } = renderHook(() => useModels());

    await flushMicrotasks();

    act(() => {
      result.current.searchModelsFTS('first');
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(result.current.isSearching).toBe(true);

    act(() => {
      result.current.searchModelsFTS('second');
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    await act(async () => {
      secondSearch.resolve({
        success: true,
        models: [makeRecord('second-result')],
        query_time_ms: 18,
      });
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['second-result']));
    expect(result.current.searchQueryTime).toBe(18);
    expect(result.current.isSearching).toBe(false);

    await act(async () => {
      firstSearch.resolve({
        success: true,
        models: [makeRecord('first-result')],
        query_time_ms: 9,
      });
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['second-result']));
    expect(result.current.searchQueryTime).toBe(18);

    const clearedSearch = createDeferred<{
      success: boolean;
      models: ModelRecord[];
      query_time_ms: number;
    }>();
    searchModelsFTSMock.mockReturnValueOnce(clearedSearch.promise);
    getModelsMock.mockResolvedValueOnce({
      success: true,
      models: {
        reset: makeRecord('reset'),
      },
    });

    act(() => {
      result.current.searchModelsFTS('third');
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    act(() => {
      result.current.searchModelsFTS('   ');
    });

    await flushMicrotasks();

    expect(getModelsMock).toHaveBeenCalledTimes(2);
    expect(result.current.modelGroups).toEqual(grouped(['reset']));

    await act(async () => {
      clearedSearch.resolve({
        success: true,
        models: [makeRecord('stale-third')],
        query_time_ms: 6,
      });
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['reset']));
  });

  it('refreshes grouped models after a debounced model-library update notification', async () => {
    let notifyModelLibraryUpdate: ((notification: unknown) => void) | null = null;
    const unsubscribe = vi.fn();

    getElectronAPIMock.mockReturnValue({
      onModelLibraryUpdate: vi.fn((callback: (notification: ModelLibraryUpdateNotification) => void) => {
        notifyModelLibraryUpdate = (notification) =>
          callback(notification as ModelLibraryUpdateNotification);
        return unsubscribe;
      }),
    });
    getModelsMock
      .mockResolvedValueOnce({
        success: true,
        models: {
          alpha: makeRecord('alpha'),
        },
      })
      .mockResolvedValueOnce({
        success: true,
        models: {
          beta: makeRecord('beta'),
        },
      });

    const { result } = renderHook(() => useModels());

    await flushMicrotasks();

    expect(result.current.modelGroups).toEqual(grouped(['alpha']));
    expect(notifyModelLibraryUpdate).not.toBeNull();

    act(() => {
      notifyModelLibraryUpdate?.({
        cursor: 'model-library-updates:2',
        stale_cursor: false,
        snapshot_required: false,
      });
      vi.advanceTimersByTime(249);
    });

    expect(getModelsMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(1);
      await Promise.resolve();
    });

    expect(getModelsMock).toHaveBeenCalledTimes(2);
    expect(result.current.modelGroups).toEqual(grouped(['beta']));
  });

  it('ignores invalid model-library update notifications', async () => {
    let notifyModelLibraryUpdate: ((notification: unknown) => void) | null = null;

    getElectronAPIMock.mockReturnValue({
      onModelLibraryUpdate: vi.fn((callback: (notification: ModelLibraryUpdateNotification) => void) => {
        notifyModelLibraryUpdate = (notification) =>
          callback(notification as ModelLibraryUpdateNotification);
        return vi.fn();
      }),
    });

    renderHook(() => useModels());

    await flushMicrotasks();

    act(() => {
      notifyModelLibraryUpdate?.({
        cursor: 'model-library-updates:2',
        stale_cursor: false,
      });
      vi.advanceTimersByTime(250);
    });

    expect(getModelsMock).toHaveBeenCalledTimes(1);
  });

  it('cleans up model-library update subscriptions and pending debounce timers on unmount', async () => {
    let notifyModelLibraryUpdate: ((notification: unknown) => void) | null = null;
    const unsubscribe = vi.fn();

    getElectronAPIMock.mockReturnValue({
      onModelLibraryUpdate: vi.fn((callback: (notification: ModelLibraryUpdateNotification) => void) => {
        notifyModelLibraryUpdate = (notification) =>
          callback(notification as ModelLibraryUpdateNotification);
        return unsubscribe;
      }),
    });

    const { unmount } = renderHook(() => useModels());

    await flushMicrotasks();

    act(() => {
      notifyModelLibraryUpdate?.({
        cursor: 'model-library-updates:2',
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    unmount();

    act(() => {
      vi.advanceTimersByTime(250);
    });

    expect(unsubscribe).toHaveBeenCalledTimes(1);
    expect(getModelsMock).toHaveBeenCalledTimes(1);
  });

  it('revalidates the active FTS search after model-library update notifications', async () => {
    let notifyModelLibraryUpdate: ((notification: unknown) => void) | null = null;

    getElectronAPIMock.mockReturnValue({
      onModelLibraryUpdate: vi.fn((callback: (notification: ModelLibraryUpdateNotification) => void) => {
        notifyModelLibraryUpdate = (notification) =>
          callback(notification as ModelLibraryUpdateNotification);
        return vi.fn();
      }),
    });
    searchModelsFTSMock
      .mockResolvedValueOnce({
        success: true,
        models: [makeRecord('first-search')],
        query_time_ms: 10,
      })
      .mockResolvedValueOnce({
        success: true,
        models: [makeRecord('refreshed-search')],
        query_time_ms: 11,
      });

    const { result } = renderHook(() => useModels());

    await flushMicrotasks();

    act(() => {
      result.current.searchModelsFTS('qwen', 'checkpoint', ['tag-a']);
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['first-search']));

    act(() => {
      notifyModelLibraryUpdate?.({
        cursor: 'model-library-updates:2',
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    await act(async () => {
      vi.advanceTimersByTime(250);
      await Promise.resolve();
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(searchModelsFTSMock).toHaveBeenCalledTimes(2);
    expect(searchModelsFTSMock).toHaveBeenLastCalledWith('qwen', 100, 0, 'checkpoint', ['tag-a']);
    expect(getModelsMock).toHaveBeenCalledTimes(1);
    expect(result.current.modelGroups).toEqual(grouped(['refreshed-search']));
  });

  it('discards stale model list responses when search becomes active', async () => {
    const initialFetch = createDeferred<{
      success: boolean;
      models: Record<string, ModelRecord>;
    }>();

    getModelsMock.mockReturnValueOnce(initialFetch.promise);
    searchModelsFTSMock.mockResolvedValueOnce({
      success: true,
      models: [makeRecord('search-result')],
      query_time_ms: 10,
    });

    const { result } = renderHook(() => useModels());

    act(() => {
      result.current.searchModelsFTS('active-query');
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['search-result']));

    await act(async () => {
      initialFetch.resolve({
        success: true,
        models: {
          stale: makeRecord('stale'),
        },
      });
      await Promise.resolve();
    });

    expect(result.current.modelGroups).toEqual(grouped(['search-result']));
  });
});
