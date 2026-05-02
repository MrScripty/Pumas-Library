import { describe, expect, it } from 'vitest';
import { groupModelRecords, mapModelRecordToInfo } from './libraryModels';
import type { ModelRecord } from '../types/api';

function makeModelRecord(overrides: Partial<ModelRecord> = {}): ModelRecord {
  return {
    id: 'llm/llama/test-model',
    path: '/tmp/models/llm/llama/test-model',
    modelType: 'llm',
    officialName: 'Test Model',
    cleanedName: 'test-model',
    tags: [],
    hashes: {},
    metadata: {
      size_bytes: 1234,
      added_date: '2026-03-06T00:00:00Z',
      repo_id: 'example/test-model',
      primary_format: 'gguf',
      quantization: 'Q4_K_M',
      dependency_bindings: [{ profile_id: 'llama-cpp-runtime' }],
      conversion_source: {
        source_model_id: 'llm/llama/source-model',
        source_format: 'safetensors',
        target_format: 'gguf',
        conversion_date: '2026-03-06T00:00:00Z',
        was_dequantized: false,
      },
    },
    updatedAt: '2026-03-06T00:00:00Z',
    ...overrides,
  };
}

describe('mapModelRecordToInfo', () => {
  it('maps canonical indexed metadata into row display fields', () => {
    const info = mapModelRecordToInfo(makeModelRecord());

    expect(info.id).toBe('llm/llama/test-model');
    expect(info.modelDir).toBe('/tmp/models/llm/llama/test-model');
    expect(info.format).toBe('gguf');
    expect(info.quant).toBe('Q4_K_M');
    expect(info.size).toBe(1234);
    expect(info.date).toBe('2026-03-06T00:00:00Z');
    expect(info.repoId).toBe('example/test-model');
    expect(info.hasDependencies).toBe(true);
    expect(info.dependencyCount).toBe(1);
    expect(info.primaryFormat).toBe('gguf');
  });

  it('keeps non-convertible formats out of the convert action field', () => {
    const info = mapModelRecordToInfo(
      makeModelRecord({
        id: 'vision/onnx/test-model',
        modelType: 'vision',
        metadata: {
          primary_format: 'onnx',
          quantization: null,
          dependency_bindings: [],
        },
      })
    );

    expect(info.format).toBe('onnx');
    expect(info.primaryFormat).toBeUndefined();
    expect(info.hasDependencies).toBe(false);
  });

  it('uses dedicated row fields instead of cleaned metadata duplicates', () => {
    const info = mapModelRecordToInfo(
      makeModelRecord({
        modelType: '',
        metadata: {
          model_type: 'legacy-metadata-type',
          conversion_source: {
            source_format: 'safetensors',
            was_dequantized: true,
          },
        },
      })
    );

    expect(info.category).toBe('uncategorized');
    expect(info.wasDequantized).toBeUndefined();
    expect(info.convertedFrom).toBeUndefined();
  });
});

describe('groupModelRecords', () => {
  it('groups mapped records by model type', () => {
    const groups = groupModelRecords([
      makeModelRecord(),
      makeModelRecord({
        id: 'audio/kittentts/test-model',
        modelType: 'audio',
        officialName: 'Audio Model',
      }),
    ]);

    expect(groups).toHaveLength(2);
    expect(groups[0]?.models[0]?.name).toBeDefined();
    expect(groups.map((group) => group.category).sort()).toEqual(['audio', 'llm']);
  });
});
