import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  DetectShardedSetsResponse,
  EmbeddedMetadataResponse,
  FileTypeValidationResponse,
  HFMetadataLookupResponse,
  ImportBatchResponse,
  ImportPathClassification,
} from '../../types/api';

const {
  classifyImportPathsMock,
  detectShardedSetsMock,
  getEmbeddedMetadataMock,
  importBatchMock,
  lookupHFMetadataForBundleDirectoryMock,
  lookupHFMetadataMock,
  validateFileTypeMock,
} = vi.hoisted(() => ({
  classifyImportPathsMock: vi.fn<(_paths: string[]) => Promise<ImportPathClassification[]>>(),
  detectShardedSetsMock: vi.fn<(_paths: string[]) => Promise<DetectShardedSetsResponse>>(),
  getEmbeddedMetadataMock: vi.fn<(_path: string) => Promise<EmbeddedMetadataResponse>>(),
  importBatchMock: vi.fn<(_specs: unknown[]) => Promise<ImportBatchResponse>>(),
  lookupHFMetadataForBundleDirectoryMock: vi.fn<(_path: string) => Promise<HFMetadataLookupResponse>>(),
  lookupHFMetadataMock: vi.fn<(_filename: string, _path?: string | null) => Promise<HFMetadataLookupResponse>>(),
  validateFileTypeMock: vi.fn<(_path: string) => Promise<FileTypeValidationResponse>>(),
}));

vi.mock('../../api/import', () => ({
  importAPI: {
    classifyImportPaths: classifyImportPathsMock,
    detectShardedSets: detectShardedSetsMock,
    getEmbeddedMetadata: getEmbeddedMetadataMock,
    importBatch: importBatchMock,
    lookupHFMetadata: lookupHFMetadataMock,
    lookupHFMetadataForBundleDirectory: lookupHFMetadataForBundleDirectoryMock,
    validateFileType: validateFileTypeMock,
  },
}));

import { useModelImportWorkflow } from './useModelImportWorkflow';

async function flushEffects(times = 1) {
  for (let index = 0; index < times; index += 1) {
    await act(async () => {
      await Promise.resolve();
    });
  }
}

const classifyResults: ImportPathClassification[] = [
  {
    path: '/imports/model-00001-of-00002.gguf',
    kind: 'single_file',
    suggested_family: 'qwen',
    suggested_official_name: 'Qwen 3 8B',
    model_type: 'llm',
    bundle_format: null,
    pipeline_class: null,
    component_manifest: null,
    reasons: [],
    candidates: [],
  },
  {
    path: '/imports/model-00002-of-00002.gguf',
    kind: 'single_file',
    suggested_family: 'qwen',
    suggested_official_name: 'Qwen 3 8B',
    model_type: 'llm',
    bundle_format: null,
    pipeline_class: null,
    component_manifest: null,
    reasons: [],
    candidates: [],
  },
  {
    path: '/imports/container',
    kind: 'multi_model_container',
    suggested_family: null,
    suggested_official_name: null,
    model_type: null,
    bundle_format: null,
    pipeline_class: null,
    component_manifest: null,
    reasons: ['contains multiple models'],
    candidates: [],
  },
  {
    path: '/imports/ambiguous',
    kind: 'ambiguous',
    suggested_family: null,
    suggested_official_name: null,
    model_type: null,
    bundle_format: null,
    pipeline_class: null,
    component_manifest: null,
    reasons: ['conflicting metadata'],
    candidates: [],
  },
];

describe('useModelImportWorkflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('classifies import paths and derives sharded and review state', async () => {
    classifyImportPathsMock.mockResolvedValue(classifyResults);
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
    const onImportComplete = vi.fn();
    const importPaths = classifyResults.map((entry) => entry.path);

    const { result } = renderHook(() => useModelImportWorkflow({
      importPaths,
      onImportComplete,
    }));

    await flushEffects(3);

    expect(result.current.step).toBe('review');
    expect(result.current.entries).toHaveLength(2);
    expect(result.current.reviewFindings).toHaveLength(2);
    expect(result.current.shardedSets).toHaveLength(1);

    expect(result.current.blockedFindings).toEqual([
      expect.objectContaining({ path: '/imports/ambiguous', kind: 'ambiguous' }),
    ]);
    expect(result.current.containerFindings).toEqual([
      expect.objectContaining({ path: '/imports/container', kind: 'multi_model_container' }),
    ]);
    expect(result.current.entries.every((entry) => entry.shardedSetKey === 'qwen')).toBe(true);
    expect(result.current.standaloneEntries).toHaveLength(0);
    expect(detectShardedSetsMock).toHaveBeenCalledWith([
      '/imports/model-00001-of-00002.gguf',
      '/imports/model-00002-of-00002.gguf',
    ]);
    expect(onImportComplete).not.toHaveBeenCalled();
  });

  it('uses embedded metadata matches for gguf files and still advances lookup progress', async () => {
    classifyImportPathsMock.mockResolvedValue([classifyResults[0]!]);
    detectShardedSetsMock.mockResolvedValue({ success: true, groups: {} });
    validateFileTypeMock.mockResolvedValue({
      success: true,
      valid: true,
      detected_type: 'gguf',
    });
    getEmbeddedMetadataMock.mockResolvedValue({
      success: true,
      file_type: 'gguf',
      metadata: {
        'general.repo_url': 'https://huggingface.co/Qwen/Qwen3-8B-GGUF',
      },
    });
    const onImportComplete = vi.fn();
    const importPaths = ['/imports/model-00001-of-00002.gguf'];

    const { result } = renderHook(() => useModelImportWorkflow({
      importPaths,
      onImportComplete,
    }));

    await flushEffects(3);

    expect(result.current.step).toBe('review');
    expect(result.current.entries).toHaveLength(1);

    await act(async () => {
      result.current.proceedToLookup();
    });

    await flushEffects(4);

    expect(result.current.step).toBe('lookup');
    expect(result.current.lookupProgress).toEqual({ current: 1, total: 1 });
    expect(result.current.entries[0]?.metadataStatus).toBe('found');
    expect(result.current.entries[0]?.hfMetadata?.repo_id).toBe('Qwen/Qwen3-8B-GGUF');

    expect(lookupHFMetadataMock).not.toHaveBeenCalled();
    expect(lookupHFMetadataForBundleDirectoryMock).not.toHaveBeenCalled();
    expect(onImportComplete).not.toHaveBeenCalled();
  });

  it('imports successful batches and reports completion counts', async () => {
    classifyImportPathsMock.mockResolvedValue([classifyResults[0]!]);
    detectShardedSetsMock.mockResolvedValue({ success: true, groups: {} });
    importBatchMock.mockResolvedValue({
      success: true,
      imported: 1,
      failed: 0,
      results: [
        {
          path: '/imports/model-00001-of-00002.gguf',
          success: true,
          model_id: 'qwen/qwen-3-8b',
          security_tier: 'safe',
        },
      ],
    });
    const onImportComplete = vi.fn();
    const importPaths = ['/imports/model-00001-of-00002.gguf'];

    const { result } = renderHook(() => useModelImportWorkflow({
      importPaths,
      onImportComplete,
    }));

    await flushEffects(3);

    expect(result.current.step).toBe('review');
    expect(result.current.entries).toHaveLength(1);

    await act(async () => {
      await result.current.startImport();
    });

    await flushEffects(2);

    expect(result.current.step).toBe('complete');
    expect(result.current.importedCount).toBe(1);
    expect(result.current.failedCount).toBe(0);
    expect(result.current.entries[0]?.status).toBe('success');

    expect(importBatchMock).toHaveBeenCalledWith([
      expect.objectContaining({
        path: '/imports/model-00001-of-00002.gguf',
        family: 'qwen',
        official_name: 'Qwen 3 8B',
        model_type: 'llm',
      }),
    ]);
    expect(onImportComplete).toHaveBeenCalledTimes(1);
  });
});
