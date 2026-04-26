import { describe, expect, it } from 'vitest';
import { getLocalModelRowState } from './LocalModelRowState';
import type { ModelInfo } from '../types/apps';

function createModel(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'model-1',
    name: 'Test Model',
    category: 'llm',
    ...overrides,
  };
}

describe('getLocalModelRowState', () => {
  it('retains the progress ring for partial downloads that are no longer active', () => {
    const rowState = getLocalModelRowState({
      excludedModels: new Set(),
      expandedRelated: new Set(),
      model: createModel({
        isDownloading: false,
        isPartialDownload: true,
        downloadProgress: 0.42,
      }),
      relatedModelsById: {},
      starredModels: new Set(),
      canConvertModel: true,
      canPauseDownload: true,
      canRecoverDownload: true,
      canResumeDownload: true,
    });

    expect(rowState.hasRetainedProgressRing).toBe(true);
    expect(rowState.ringDegrees).toBe(151);
  });

  it('falls back to a full retained ring when indexed partial downloads have no numeric progress', () => {
    const rowState = getLocalModelRowState({
      excludedModels: new Set(),
      expandedRelated: new Set(),
      model: createModel({
        isDownloading: false,
        isPartialDownload: true,
      }),
      relatedModelsById: {},
      starredModels: new Set(),
      canConvertModel: true,
      canPauseDownload: true,
      canRecoverDownload: true,
      canResumeDownload: true,
    });

    expect(rowState.hasRetainedProgressRing).toBe(true);
    expect(rowState.ringDegrees).toBe(360);
  });

  it('does not retain the progress ring while a download is active', () => {
    const rowState = getLocalModelRowState({
      excludedModels: new Set(),
      expandedRelated: new Set(),
      model: createModel({
        isDownloading: true,
        isPartialDownload: true,
        downloadProgress: 0.42,
        downloadRepoId: 'org/model',
        downloadStatus: 'downloading',
      }),
      relatedModelsById: {},
      starredModels: new Set(),
      canConvertModel: true,
      canPauseDownload: true,
      canRecoverDownload: true,
      canResumeDownload: true,
    });

    expect(rowState.isActiveDownload).toBe(true);
    expect(rowState.hasRetainedProgressRing).toBe(false);
  });
});
