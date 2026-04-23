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

});
