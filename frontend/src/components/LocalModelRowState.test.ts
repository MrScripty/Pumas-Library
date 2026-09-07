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
      canPauseDownload: true,
      canRecoverDownload: true,
      canResumeDownload: true,
    });

    expect(rowState.hasRetainedProgressRing).toBe(true);
    expect(rowState.ringDegrees).toBe(151);
  });

  it('does not show a complete ring when indexed partial downloads have no numeric progress', () => {
    const rowState = getLocalModelRowState({
      excludedModels: new Set(),
      expandedRelated: new Set(),
      model: createModel({
        isDownloading: false,
        isPartialDownload: true,
      }),
      relatedModelsById: {},
      starredModels: new Set(),
      canPauseDownload: true,
      canRecoverDownload: true,
      canResumeDownload: true,
    });

    expect(rowState.hasRetainedProgressRing).toBe(true);
    expect(rowState.ringDegrees).toBe(0);
  });

  it('does not infer recovery authority from a partial model without a current ticket', () => {
    const rowState = getLocalModelRowState({
      excludedModels: new Set(),
      expandedRelated: new Set(),
      model: createModel({
        isDownloading: false,
        isPartialDownload: true,
        modelDir: '/models/partial',
      }),
      relatedModelsById: {},
      starredModels: new Set(),
      canPauseDownload: true,
      canRecoverDownload: true,
      canResumeDownload: true,
    });

    expect(rowState.canRecoverPartial).toBe(false);
    expect(rowState.partialError).toBe(
      'Resume is unavailable until current recovery information is available.'
    );
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
      canPauseDownload: true,
      canRecoverDownload: true,
      canResumeDownload: true,
    });

    expect(rowState.isActiveDownload).toBe(true);
    expect(rowState.hasRetainedProgressRing).toBe(false);
  });
});
