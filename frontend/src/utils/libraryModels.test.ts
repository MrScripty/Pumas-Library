import { describe, expect, it } from 'vitest';
import { groupCatalogModels, projectCatalogModel } from './libraryModels';
import type { CatalogModel } from '../generated/desktop-contract';

function catalog(overrides: Partial<CatalogModel> = {}): CatalogModel {
  return {
    id: 'llm/example', modelDir: '/models/example', modelType: 'llm',
    inputModalities: ['text'], outputModalities: ['text'], adapter: false,
    displayName: 'Example', format: 'gguf', quantization: 'Q4_K_M', sizeBytes: 1234,
    displayDate: '2026-03-06T00:00:00Z', dependencyCount: 1, relatedAvailable: true,
    artifact: { state: 'complete' }, integrity: { state: 'clean' }, ...overrides,
  };
}

describe('catalog projection', () => {
  it('uses explicit catalog fields without reading raw model metadata', () => {
    expect(projectCatalogModel(catalog())).toMatchObject({
      id: 'llm/example', name: 'Example', category: 'llm', modelDir: '/models/example',
      format: 'gguf', quant: 'Q4_K_M', size: 1234, date: '2026-03-06T00:00:00Z',
      dependencyCount: 1, hasDependencies: true, isPartialDownload: false,
      hasIntegrityIssue: false, provenance: 'catalog', relatedAvailable: true,
    });
  });

  it('preserves partial progress and exact ticket separately from integrity', () => {
    const recovery = { recoveryToken: `v1:${'a'.repeat(64)}`, repoId: 'org/example', selectedArtifactFiles: ['weights.gguf'] };
    const model = projectCatalogModel(catalog({
      artifact: { state: 'partial', downloadProgressFraction: 0.42, reasons: ['part_file_present'], recovery },
      integrity: { state: 'duplicate', count: 2, otherModelIds: ['llm/other'] },
    }));
    expect(model).toMatchObject({
      isPartialDownload: true, downloadProgress: 0.42, recovery, hasIntegrityIssue: true,
      integrityIssueMessage: 'Duplicate artifact entries detected (2 paths). Run library reconciliation.',
    });
  });

  it('does not invent progress, recovery permission, or conversion support', () => {
    const model = projectCatalogModel(catalog({
      format: 'onnx', artifact: { state: 'partial', reasons: ['expected_files_missing'] },
    }));
    expect(model.downloadProgress).toBeUndefined();
    expect(model.recovery).toBeUndefined();
    expect(model.primaryFormat).toBeUndefined();
  });

  it('groups catalog rows by their declared model type', () => {
    expect(groupCatalogModels([catalog(), catalog({ id: 'audio/example', modelType: 'audio' })])
      .map((group) => group.category)).toEqual(['llm', 'audio']);
  });

  it('orders groups by use, with unclassified and future categories last', () => {
    const order = groupCatalogModels([
      catalog({ id: 'u', modelType: 'unknown' }),
      catalog({ id: 'f', modelType: 'future-kind' }),
      catalog({ id: 'v', modelType: 'video' }),
      catalog({ id: 'd', modelType: 'decision' }),
      catalog({ id: 'l', modelType: 'llm' }),
    ]).map((group) => group.category);
    expect(order).toEqual(['llm', 'decision', 'video', 'unknown', 'future-kind']);
  });

  it('carries declared modalities and the adapter flag into rows', () => {
    expect(projectCatalogModel(catalog({
      modelType: 'video', inputModalities: ['text', 'image'], outputModalities: ['video'], adapter: true,
    }))).toMatchObject({ category: 'video', inputModalities: ['text', 'image'], outputModalities: ['video'], adapter: true });
  });
});
