import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';
import type { GetHFDownloadDetailsResponse, HFDownloadDetails } from '../types/api-models';

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

function requireRemoteModel(model: RemoteModelInfo | undefined): RemoteModelInfo {
  if (model === undefined) {
    throw new TypeError('Expected search result model');
  }

  return model;
}

async function renderResults(models: RemoteModelInfo[]) {
  searchHfModelsMock.mockResolvedValueOnce({ success: true, models });
  const hook = renderHook(() => useRemoteModelSearch({ enabled: true, searchQuery: 'fixture' }));
  await act(async () => {
    await vi.advanceTimersByTimeAsync(300);
  });
  return hook;
}

describe('useRemoteModelSearch hydration', () => {
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
        repoId: 'acme/model-a',
        downloadOptions: [
          {
            quant: 'Q4_K_M',
            sizeBytes: 4096,
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

  it('preserves exact quant and file-group identities without changing another repository', async () => {
    const details: HFDownloadDetails = {
      repoId: 'acme/model-a',
      totalSizeBytes: null,
      downloadOptions: [
        { quant: 'Q4_K_M', sizeBytes: 300, fileGroup: { filenames: ['Q4/part-02.gguf', 'Q4/part-01.gguf'], shardCount: 2, label: 'Q4 parts' } },
        { quant: 'Q8_0', sizeBytes: null, fileGroup: { filenames: ['Q8/model.gguf'], shardCount: 1, label: 'Q8 model' } },
      ],
    };
    getHfDownloadDetailsMock.mockResolvedValueOnce({ success: true, details });
    const other = baseRemoteModel({ repoId: 'other/model-a', name: 'Model A', totalSizeBytes: 72 });
    const { result } = await renderResults([baseRemoteModel({ quants: ['Q4_K_M', 'Q8_0'] }), other]);
    await act(async () => {
      await result.current.hydrateModelDetails(requireRemoteModel(result.current.results[0]));
    });
    expect(getHfDownloadDetailsMock).toHaveBeenCalledWith('acme/model-a', ['Q4_K_M', 'Q8_0']);
    expect(result.current.results[0]?.downloadOptions).toEqual(details.downloadOptions);
    expect(result.current.results[0]?.totalSizeBytes).toBeNull();
    expect(result.current.results[0]?.quants).toEqual(['Q4_K_M', 'Q8_0']);
    expect(result.current.results[1]).toEqual(other);
  });

  it('refuses details for another repository without applying them to either row', async () => {
    const original = baseRemoteModel({ downloadOptions: [{ quant: 'Q4_K_M', sizeBytes: null }], totalSizeBytes: null });
    const other = baseRemoteModel({ repoId: 'other/model-a' });
    getHfDownloadDetailsMock.mockResolvedValueOnce({
      success: true,
      details: { repoId: other.repoId, downloadOptions: [{ quant: 'Q8_0', sizeBytes: 10 }], totalSizeBytes: 10 },
    });
    const { result } = await renderResults([original, other]);
    await act(async () => { await result.current.hydrateModelDetails(original); });
    expect(result.current.results).toEqual([original, other]);
    expect(result.current.hydratingRepoIds.size).toBe(0);
  });

  it.each(['failure', 'rejection'])('preserves the unhydrated row on a details %s', async (outcome) => {
    const original = baseRemoteModel({ downloadOptions: [{ quant: 'Q4_K_M', sizeBytes: null }], totalSizeBytes: null });
    if (outcome === 'failure') {
      getHfDownloadDetailsMock.mockResolvedValueOnce({ success: false, error: 'Could not load Hugging Face download details.' });
    } else {
      getHfDownloadDetailsMock.mockRejectedValueOnce(new Error('fixture unavailable'));
    }
    const { result } = await renderResults([original]);
    await act(async () => { await result.current.hydrateModelDetails(original); });
    expect(result.current.results).toEqual([original]);
    expect(result.current.hydratingRepoIds.size).toBe(0);
  });

  it('applies successful null sizes and empty options without keeping previous values', async () => {
    const original = baseRemoteModel({ totalSizeBytes: 0, downloadOptions: [{ quant: 'Q4_K_M', sizeBytes: 0 }] });
    getHfDownloadDetailsMock.mockResolvedValueOnce({ success: true, details: { repoId: original.repoId, totalSizeBytes: null, downloadOptions: [] } });
    const { result } = await renderResults([original]);
    await act(async () => { await result.current.hydrateModelDetails(original); });
    expect(result.current.results[0]?.totalSizeBytes).toBeNull();
    expect(result.current.results[0]?.downloadOptions).toEqual([]);
  });

  it('does not remove a newer same-repository hydration when an old generation settles', async () => {
    const first = createDeferred<GetHFDownloadDetailsResponse>();
    const second = createDeferred<GetHFDownloadDetailsResponse>();
    searchHfModelsMock.mockResolvedValue({ success: true, models: [baseRemoteModel()] });
    getHfDownloadDetailsMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const { result, rerender } = renderHook(({ searchQuery }) => useRemoteModelSearch({ enabled: true, searchQuery }), { initialProps: { searchQuery: 'first' } });
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    let oldRequest: Promise<void> | undefined;
    await act(async () => { oldRequest = result.current.hydrateModelDetails(requireRemoteModel(result.current.results[0])); });
    rerender({ searchQuery: 'second' });
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    let newRequest: Promise<void> | undefined;
    await act(async () => { newRequest = result.current.hydrateModelDetails(requireRemoteModel(result.current.results[0])); });
    await act(async () => {
      first.resolve({ success: true, details: { repoId: 'acme/model-a', totalSizeBytes: 10, downloadOptions: [] } });
      await oldRequest;
    });
    expect(result.current.results[0]?.totalSizeBytes).toBeUndefined();
    expect(result.current.hydratingRepoIds.has('acme/model-a')).toBe(true);
    let repeatedRequest: Promise<void> | undefined;
    await act(async () => { repeatedRequest = result.current.hydrateModelDetails(requireRemoteModel(result.current.results[0])); });
    expect(getHfDownloadDetailsMock).toHaveBeenCalledTimes(2);
    await act(async () => {
      second.resolve({ success: true, details: { repoId: 'acme/model-a', totalSizeBytes: 20, downloadOptions: [] } });
      await newRequest;
      await repeatedRequest;
    });
    expect(result.current.results[0]?.totalSizeBytes).toBe(20);
    expect(result.current.hydratingRepoIds.size).toBe(0);
  });

  it('hydrates missing download details once per repo and tracks hydration state', async () => {
    const hydrationDeferred = createDeferred<GetHFDownloadDetailsResponse>();

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

    const target = requireRemoteModel(result.current.results[0]);

    let firstHydration: Promise<void> | undefined;
    let secondHydration: Promise<void> | undefined;
    await act(async () => {
      firstHydration = result.current.hydrateModelDetails(target);
      secondHydration = result.current.hydrateModelDetails(target);
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
          repoId: 'acme/hydrate-me',
          downloadOptions: [
            {
              quant: 'Q4_K_M',
              sizeBytes: 4096,
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
    const hydrationDeferred = createDeferred<GetHFDownloadDetailsResponse>();

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

    const staleTarget = requireRemoteModel(result.current.results[0]);

    let hydrationPromise: Promise<void> | undefined;
    await act(async () => {
      hydrationPromise = result.current.hydrateModelDetails(staleTarget);
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
          repoId: 'acme/original',
          downloadOptions: [
            {
              quant: 'Q4_K_M',
              sizeBytes: 9999,
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
