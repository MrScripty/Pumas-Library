import { act, renderHook } from '@testing-library/react';
import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { EmbeddedMetadataResponse } from '../../types/api';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';

const { getEmbeddedMetadataMock } = vi.hoisted(() => ({
  getEmbeddedMetadataMock: vi.fn<(_path: string) => Promise<EmbeddedMetadataResponse>>(),
}));

vi.mock('../../api/import', () => ({
  importAPI: {
    getEmbeddedMetadata: getEmbeddedMetadataMock,
  },
}));

import { useEmbeddedMetadataToggles } from './useEmbeddedMetadataToggles';

async function flushEffects(times = 1) {
  for (let index = 0; index < times; index += 1) {
    await act(async () => {
      await Promise.resolve();
    });
  }
}

function createEntry(overrides: Partial<ImportEntryStatus> = {}): ImportEntryStatus {
  return {
    path: '/imports/model.gguf',
    originPath: '/imports/model.gguf',
    filename: 'model.gguf',
    kind: 'single_file',
    status: 'pending',
    securityTier: 'safe',
    securityAcknowledged: true,
    metadataStatus: 'pending',
    suggestedFamily: 'imported',
    suggestedOfficialName: 'model',
    ...overrides,
  };
}

function useHarness(initialEntries: ImportEntryStatus[]) {
  const [entries, setEntries] = useState(initialEntries);
  return {
    entries,
    ...useEmbeddedMetadataToggles({ setEntries }),
  };
}

describe('useEmbeddedMetadataToggles', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads embedded metadata the first time an unloaded single file is shown', async () => {
    let resolveMetadata:
      | ((response: EmbeddedMetadataResponse) => void)
      | undefined;
    getEmbeddedMetadataMock.mockImplementation(() => new Promise((resolve) => {
      resolveMetadata = resolve;
    }));

    const { result } = renderHook(() => useHarness([createEntry()]));

    await act(async () => {
      await result.current.toggleMetadataSource('/imports/model.gguf');
    });

    expect(result.current.showEmbeddedMetadata.has('/imports/model.gguf')).toBe(true);
    expect(result.current.entries[0]?.embeddedMetadataStatus).toBe('pending');

    await act(async () => {
      resolveMetadata?.({
        success: true,
        file_type: 'gguf',
        metadata: {
          'general.name': 'Test Model',
        },
      });
    });
    await flushEffects(2);

    expect(getEmbeddedMetadataMock).toHaveBeenCalledWith('/imports/model.gguf');
    expect(result.current.entries[0]?.embeddedMetadataStatus).toBe('loaded');
    expect(result.current.entries[0]?.embeddedMetadata).toEqual({
      'general.name': 'Test Model',
    });
  });

  it('does not reload embedded metadata that is already available', async () => {
    const { result } = renderHook(() => useHarness([
      createEntry({
        embeddedMetadataStatus: 'loaded',
        embeddedMetadata: {
          'general.name': 'Cached Model',
        },
      }),
    ]));

    await act(async () => {
      await result.current.toggleMetadataSource('/imports/model.gguf');
    });

    expect(result.current.showEmbeddedMetadata.has('/imports/model.gguf')).toBe(true);
    expect(getEmbeddedMetadataMock).not.toHaveBeenCalled();
  });

  it('records unsupported embedded metadata lookups without retrying on the next toggle', async () => {
    getEmbeddedMetadataMock.mockResolvedValue({
      success: false,
      file_type: 'unsupported',
      metadata: null,
    });

    const { result } = renderHook(() => useHarness([createEntry()]));

    await act(async () => {
      await result.current.toggleMetadataSource('/imports/model.gguf');
    });
    await flushEffects(2);

    expect(result.current.entries[0]?.embeddedMetadataStatus).toBe('unsupported');

    await act(async () => {
      await result.current.toggleMetadataSource('/imports/model.gguf');
      await result.current.toggleMetadataSource('/imports/model.gguf');
    });

    expect(getEmbeddedMetadataMock).toHaveBeenCalledTimes(1);
  });

  it('toggles all embedded metadata visibility independently', () => {
    const { result } = renderHook(() => useHarness([createEntry()]));

    act(() => {
      result.current.toggleShowAllEmbeddedMetadata('/imports/model.gguf');
    });

    expect(result.current.showAllEmbeddedMetadata.has('/imports/model.gguf')).toBe(true);

    act(() => {
      result.current.toggleShowAllEmbeddedMetadata('/imports/model.gguf');
    });

    expect(result.current.showAllEmbeddedMetadata.has('/imports/model.gguf')).toBe(false);
  });
});
