import { act, renderHook } from '@testing-library/react';
import { useMemo, useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DetectShardedSetsResponse } from '../../types/api';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';

const { detectShardedSetsMock } = vi.hoisted(() => ({
  detectShardedSetsMock: vi.fn<(_paths: string[]) => Promise<DetectShardedSetsResponse>>(),
}));

vi.mock('../../api/import', () => ({
  importAPI: {
    detectShardedSets: detectShardedSetsMock,
  },
}));

import { useShardedSetDetection } from './useShardedSetDetection';

async function flushEffects(times = 1) {
  for (let index = 0; index < times; index += 1) {
    await act(async () => {
      await Promise.resolve();
    });
  }
}

function createEntry(path: string, overrides: Partial<ImportEntryStatus> = {}): ImportEntryStatus {
  return {
    path,
    originPath: path,
    filename: path.split('/').pop() ?? path,
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
  const fileEntries = useMemo(
    () => entries.filter((entry) => entry.kind === 'single_file'),
    [entries]
  );
  return {
    entries,
    ...useShardedSetDetection({ fileEntries, setEntries }),
  };
}

describe('useShardedSetDetection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('detects complete sharded sets and annotates matching file entries', async () => {
    detectShardedSetsMock.mockResolvedValue({
      success: true,
      groups: {
        qwen: {
          files: [
            '/imports/model-00001-of-00002.gguf',
            '/imports/model-00002-of-00002.gguf',
          ],
          validation: {
            complete: true,
            missing_shards: [],
            total_expected: 2,
            total_found: 2,
          },
        },
      },
    });

    const { result } = renderHook(() => useHarness([
      createEntry('/imports/model-00001-of-00002.gguf'),
      createEntry('/imports/model-00002-of-00002.gguf'),
    ]));

    await flushEffects(3);

    expect(detectShardedSetsMock).toHaveBeenCalledWith([
      '/imports/model-00001-of-00002.gguf',
      '/imports/model-00002-of-00002.gguf',
    ]);
    expect(result.current.shardedSets).toEqual([
      expect.objectContaining({ key: 'qwen', complete: true, expanded: false }),
    ]);
    expect(result.current.entries.every((entry) => entry.shardedSetKey === 'qwen')).toBe(true);
  });

  it('toggles expanded state for detected sharded sets', async () => {
    detectShardedSetsMock.mockResolvedValue({
      success: true,
      groups: {
        qwen: {
          files: ['/imports/a.gguf', '/imports/b.gguf'],
          validation: {
            complete: true,
            missing_shards: [],
            total_expected: 2,
            total_found: 2,
          },
        },
      },
    });

    const { result } = renderHook(() => useHarness([
      createEntry('/imports/a.gguf'),
      createEntry('/imports/b.gguf'),
    ]));

    await flushEffects(3);

    act(() => {
      result.current.toggleShardedSet('qwen');
    });

    expect(result.current.shardedSets[0]?.expanded).toBe(true);
  });

  it('clears sharded set state without mutating entries', async () => {
    detectShardedSetsMock.mockResolvedValue({
      success: true,
      groups: {
        qwen: {
          files: ['/imports/a.gguf', '/imports/b.gguf'],
          validation: {
            complete: true,
            missing_shards: [],
            total_expected: 2,
            total_found: 2,
          },
        },
      },
    });

    const { result } = renderHook(() => useHarness([
      createEntry('/imports/a.gguf'),
      createEntry('/imports/b.gguf'),
    ]));

    await flushEffects(3);

    act(() => {
      result.current.clearShardedSets();
    });

    expect(result.current.shardedSets).toEqual([]);
    expect(result.current.entries).toHaveLength(2);
  });
});
