import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelInfo } from '../types/apps';
import type { LocalModelRowState } from './LocalModelRowState';
import { LocalModelDownloadActions } from './LocalModelDownloadActions';

function createModel(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'model-1',
    name: 'Test Model',
    category: 'llm',
    downloadRepoId: 'org/model',
    ...overrides,
  };
}

function createRowState(overrides: Partial<LocalModelRowState> = {}): LocalModelRowState {
  return {
    canPause: false,
    canRecoverPartial: false,
    canResume: false,
    canShowRelated: false,
    hasRetainedProgressRing: false,
    isActiveDownload: false,
    isConvertible: false,
    isDownloading: true,
    isExpanded: false,
    isLinked: true,
    isPartialDownload: false,
    isPaused: false,
    isQueued: false,
    isRecoveringPartial: false,
    isStarred: false,
    relatedModels: [],
    relatedStatus: 'idle',
    ringDegrees: 198,
    ...overrides,
  };
}

describe('LocalModelDownloadActions', () => {
  it('keeps the ring visible for paused downloads', () => {
    render(
      <LocalModelDownloadActions
        model={createModel({ downloadStatus: 'paused' })}
        rowState={createRowState({
          canResume: true,
          isPaused: true,
        })}
        onResumeDownload={vi.fn()}
      />
    );

    const button = screen.getByTitle(/resume download/i);
    expect(button.querySelector('.download-progress-ring.is-paused')).not.toBeNull();
  });
});
