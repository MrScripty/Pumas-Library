import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';

const {
  getHfDownloadDetailsMock,
  isApiAvailableMock,
  searchHfModelsMock,
} = vi.hoisted(() => ({
  getHfDownloadDetailsMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  searchHfModelsMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_hf_download_details: getHfDownloadDetailsMock,
    search_hf_models: searchHfModelsMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

import { useRemoteModelSearch } from './useRemoteModelSearch';

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

const baseRemoteModel = (overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo => ({
  repoId: 'acme/model-a',
  name: 'Model A',
  developer: 'acme',
  kind: 'text-generation',
  formats: ['gguf'],
  quants: ['Q4_K_M'],
  url: 'https://huggingface.co/acme/model-a',
  ...overrides,
});

describe('useRemoteModelSearch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
    searchHfModelsMock.mockResolvedValue({
      success: true,
      models: [],
    });
    getHfDownloadDetailsMock.mockResolvedValue({
      success: true,
      details: {
        downloadOptions: [
          {
            quant: 'Q4_K_M',
            sizeBytes: 4096,
            fileGroup: null,
          },
        ],
        totalSizeBytes: 4096,
      },
    });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('debounces remote searches, populates results, and derives unique kinds', async () => {
    searchHfModelsMock.mockResolvedValueOnce({
      success: true,
      models: [
        baseRemoteModel(),
        baseRemoteModel({
          repoId: 'acme/model-b',
          name: 'Model B',
          kind: 'vision',
        }),
        baseRemoteModel({
          repoId: 'acme/model-c',
          name: 'Model C',
          kind: 'unknown',
        }),
      ],
    });

    const { result } = renderHook(() => useRemoteModelSearch({
      enabled: true,
      searchQuery: 'mistral',
    }));

    expect(result.current.isLoading).toBe(false);

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(searchHfModelsMock).toHaveBeenCalledWith('mistral', null, 25, 6);
    expect(result.current.results).toHaveLength(3);
    expect(result.current.kinds).toEqual(['all', 'text-generation', 'vision']);
    expect(result.current.error).toBeNull();
    expect(result.current.isLoading).toBe(false);
  });

  it('clears results without searching when the query is blank or the hook is disabled', async () => {
    const { result, rerender } = renderHook(
      (props: { enabled: boolean; searchQuery: string }) => useRemoteModelSearch(props),
      {
        initialProps: {
          enabled: true,
          searchQuery: '   ',
        },
      }
    );

    await flushMicrotasks();

    expect(result.current.results).toEqual([]);
    expect(searchHfModelsMock).not.toHaveBeenCalled();

    rerender({
      enabled: false,
      searchQuery: 'llama',
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(searchHfModelsMock).not.toHaveBeenCalled();
  });

  it('reports an unavailable search API after the debounce window', async () => {
    isApiAvailableMock.mockReturnValue(false);

    const { result } = renderHook(() => useRemoteModelSearch({
      enabled: true,
      searchQuery: 'llama',
    }));

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(result.current.error).toBe('Hugging Face search is unavailable.');
    expect(result.current.results).toEqual([]);
    expect(searchHfModelsMock).not.toHaveBeenCalled();
  });

  it('hydrates missing download details once per repo and tracks hydration state', async () => {
    const hydrationDeferred = createDeferred<{
      success: boolean;
      details: {
        downloadOptions: Array<{ quant: string; sizeBytes: number; fileGroup: null }>;
        totalSizeBytes: number;
      };
    }>();

    searchHfModelsMock.mockResolvedValueOnce({
      success: true,
      models: [
        baseRemoteModel({
          repoId: 'acme/hydrate-me',
          downloadOptions: [
            {
              quant: 'Q4_K_M',
              sizeBytes: null,
              fileGroup: null,
            },
          ],
          totalSizeBytes: null,
        }),
      ],
    });
    getHfDownloadDetailsMock.mockReturnValueOnce(hydrationDeferred.promise);

    const { result } = renderHook(() => useRemoteModelSearch({
      enabled: true,
      searchQuery: 'hydrate',
    }));

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    const target = result.current.results[0];
    expect(target).toBeDefined();

    let firstHydration: Promise<void> | undefined;
    let secondHydration: Promise<void> | undefined;
    await act(async () => {
      firstHydration = result.current.hydrateModelDetails(target!);
      secondHydration = result.current.hydrateModelDetails(target!);
      await Promise.resolve();
    });

    expect(firstHydration).toBeDefined();
    expect(secondHydration).toBeDefined();
    expect(result.current.hydratingRepoIds.has('acme/hydrate-me')).toBe(true);
    expect(getHfDownloadDetailsMock).toHaveBeenCalledTimes(1);
    expect(getHfDownloadDetailsMock).toHaveBeenCalledWith('acme/hydrate-me', ['Q4_K_M']);

    await act(async () => {
      hydrationDeferred.resolve({
        success: true,
        details: {
          downloadOptions: [
            {
              quant: 'Q4_K_M',
              sizeBytes: 4096,
              fileGroup: null,
            },
          ],
          totalSizeBytes: 4096,
        },
      });
      await firstHydration;
    });

    expect(result.current.hydratingRepoIds.size).toBe(0);
    expect(result.current.results[0]).toEqual(
      expect.objectContaining({
        repoId: 'acme/hydrate-me',
        totalSizeBytes: 4096,
        downloadOptions: [
          expect.objectContaining({
            quant: 'Q4_K_M',
            sizeBytes: 4096,
          }),
        ],
      })
    );
  });

  it('ignores stale hydration results after a new search generation starts', async () => {
    const hydrationDeferred = createDeferred<{
      success: boolean;
      details: {
        downloadOptions: Array<{ quant: string; sizeBytes: number; fileGroup: null }>;
        totalSizeBytes: number;
      };
    }>();

    searchHfModelsMock
      .mockResolvedValueOnce({
        success: true,
        models: [
          baseRemoteModel({
            repoId: 'acme/original',
            downloadOptions: [
              {
                quant: 'Q4_K_M',
                sizeBytes: null,
                fileGroup: null,
              },
            ],
            totalSizeBytes: null,
          }),
        ],
      })
      .mockResolvedValueOnce({
        success: true,
        models: [
          baseRemoteModel({
            repoId: 'acme/new-search',
            name: 'New Search',
            kind: 'embedding',
            totalSizeBytes: 1234,
          }),
        ],
      });

    getHfDownloadDetailsMock.mockReturnValueOnce(hydrationDeferred.promise);

    const { result, rerender } = renderHook(
      (props: { searchQuery: string }) => useRemoteModelSearch({
        enabled: true,
        searchQuery: props.searchQuery,
      }),
      {
        initialProps: { searchQuery: 'first' },
      }
    );

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    const staleTarget = result.current.results[0];
    expect(staleTarget).toBeDefined();

    let hydrationPromise: Promise<void> | undefined;
    await act(async () => {
      hydrationPromise = result.current.hydrateModelDetails(staleTarget!);
      await Promise.resolve();
    });

    rerender({ searchQuery: 'second' });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await Promise.resolve();
    });

    expect(result.current.results).toEqual([
      expect.objectContaining({
        repoId: 'acme/new-search',
        totalSizeBytes: 1234,
      }),
    ]);

    await act(async () => {
      hydrationDeferred.resolve({
        success: true,
        details: {
          downloadOptions: [
            {
              quant: 'Q4_K_M',
              sizeBytes: 9999,
              fileGroup: null,
            },
          ],
          totalSizeBytes: 9999,
        },
      });
      await hydrationPromise;
    });

    expect(result.current.results).toEqual([
      expect.objectContaining({
        repoId: 'acme/new-search',
        totalSizeBytes: 1234,
      }),
    ]);
    expect(result.current.hydratingRepoIds.size).toBe(0);
  });
});
