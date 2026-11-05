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

  it('describes the ring progress to assistive technology', () => {
    const { rerender } = render(
      <LocalModelDownloadActions
        model={createModel({ downloadStatus: 'downloading' })}
        rowState={createRowState({ canPause: true, isActiveDownload: true })}
        onPauseDownload={vi.fn()}
      />
    );
    const button = screen.getByRole('button', { name: 'Pause download' });
    expect(button).toHaveAccessibleDescription('Download 55% complete');
    expect(button.querySelector('.download-progress-ring')).toHaveAttribute('aria-hidden', 'true');

    rerender(
      <LocalModelDownloadActions
        model={createModel({ downloadStatus: 'queued' })}
        rowState={createRowState({ isActiveDownload: true, isQueued: true })}
      />
    );
    expect(screen.getAllByRole('button')[0]).toHaveAccessibleDescription('Download queued');
  });
});
