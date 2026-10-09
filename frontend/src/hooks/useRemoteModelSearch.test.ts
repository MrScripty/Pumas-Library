import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo, RemoteSearchSource } from '../types/apps';

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

function cachedModel(): RemoteModelInfo {
  return baseRemoteModel({ modelCard: { pumas_discovery: {
    source: 'anonymous-hf-detail-cache', source_url: 'https://huggingface.co/api/models/acme/model-a',
    observed_at: '2026-01-01T00:00:00Z', freshness: 'stale', fresh_until: '2026-01-02T00:00:00Z',
    revision_observed: 'a'.repeat(40), discovery_only: true, visibility_observed: 'public-ungated',
  } } });
}

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
        repoId: 'acme/model-a',
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

    expect(searchHfModelsMock).toHaveBeenCalledWith('mistral', null, 25, 0);
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

  it('browses with explicit cached mode, zero hydration and no tree lookup', async () => {
    searchHfModelsMock.mockResolvedValue({ success: true, models: [cachedModel()] });
    const { result } = renderHook(() => useRemoteModelSearch({ enabled: true, searchQuery: '', source: 'cached' }));
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    expect(searchHfModelsMock).toHaveBeenCalledWith('cache:', null, 25, 0);
    expect(result.current.results[0]?.cachedDiscovery?.freshness).toBe('stale');
    await act(async () => { await result.current.hydrateModelDetails(cachedModel()); });
    expect(getHfDownloadDetailsMock).not.toHaveBeenCalled();
  });

  it('cancels queued searches and hides results immediately when the source switches', async () => {
    searchHfModelsMock.mockResolvedValue({ success: true, models: [cachedModel()] });
    const { result, rerender } = renderHook((props: { source: RemoteSearchSource; searchQuery: string }) =>
      useRemoteModelSearch({ enabled: true, ...props }), { initialProps: { source: 'cached', searchQuery: 'first' } });
    rerender({ source: 'cached', searchQuery: 'second' });
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    expect(searchHfModelsMock).toHaveBeenCalledTimes(1);
    expect(searchHfModelsMock).toHaveBeenCalledWith('cache:second', null, 25, 0);
    expect(result.current.results).toHaveLength(1);
    rerender({ source: 'huggingface', searchQuery: 'second' });
    expect(result.current.results).toEqual([]);
    expect(result.current.isCachedSearch).toBe(false);
  });

  it('ignores a late cached reply after switching to ordinary search', async () => {
    let release!: (value: unknown) => void;
    searchHfModelsMock.mockReturnValueOnce(new Promise((resolve) => { release = resolve; }));
    const { result, rerender } = renderHook((props: { source: RemoteSearchSource }) =>
      useRemoteModelSearch({ enabled: true, searchQuery: 'same', ...props }), { initialProps: { source: 'cached' } });
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    rerender({ source: 'huggingface' });
    searchHfModelsMock.mockResolvedValue({ success: true, models: [baseRemoteModel({ name: 'Ordinary result' })] });
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    await act(async () => { release({ success: true, models: [cachedModel()] }); });
    expect(result.current.results[0]?.name).toBe('Ordinary result');
    expect(result.current.results[0]?.cachedDiscovery).toBeUndefined();
  });

  it('rejects missing cached provenance and strips spoofed annotations from ordinary results', async () => {
    searchHfModelsMock.mockResolvedValue({ success: true, models: [baseRemoteModel()] });
    const { result, rerender } = renderHook((props: { source: RemoteSearchSource }) =>
      useRemoteModelSearch({ enabled: true, searchQuery: 'same', ...props }), { initialProps: { source: 'cached' } });
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    expect(result.current.error).toBe('Cached search returned incomplete provenance.');
    expect(result.current.results).toEqual([]);
    searchHfModelsMock.mockResolvedValue({ success: true, models: [cachedModel()] });
    rerender({ source: 'huggingface' });
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    expect(result.current.results[0]?.cachedDiscovery).toBeUndefined();
  });

  it('recognizes the preserved explicit cache prefix and ignores completion after unmount', async () => {
    let release!: (value: unknown) => void;
    searchHfModelsMock.mockReturnValueOnce(new Promise((resolve) => { release = resolve; }));
    const { result, unmount } = renderHook(() => useRemoteModelSearch({ enabled: true, searchQuery: 'cache:alpha' }));
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    expect(result.current.isCachedSearch).toBe(true);
    expect(searchHfModelsMock).toHaveBeenCalledWith('cache:alpha', null, 25, 0);
    unmount();
    await act(async () => { release({ success: true, models: [cachedModel()] }); });
  });

});
