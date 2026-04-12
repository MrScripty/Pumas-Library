import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../types/apps';
import type { ModelRecord } from '../types/api';

const {
  getModelsMock,
  groupModelRecordsMock,
  isApiAvailableMock,
  scanSharedStorageMock,
  searchModelsFTSMock,
} = vi.hoisted(() => ({
  getModelsMock: vi.fn(),
  groupModelRecordsMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  scanSharedStorageMock: vi.fn(),
  searchModelsFTSMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
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
});
