import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelInfo } from '../types/apps';
import type { LocalModelRowState } from './LocalModelRowState';
import { LocalModelInstalledActions } from './LocalModelInstalledActions';

function createModel(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'model-1',
    name: 'Test Model',
    category: 'llm',
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
    isDownloading: false,
    isExpanded: false,
    isLinked: true,
    isPartialDownload: false,
    isPaused: false,
    isQueued: false,
    isRecoveringPartial: false,
    isStarred: false,
    relatedModels: [],
    relatedStatus: 'idle',
    ringDegrees: 0,
    ...overrides,
  };
}

describe('LocalModelInstalledActions', () => {
  it.each<{ format: NonNullable<ModelInfo['primaryFormat']>; partial: boolean; integrity: boolean }>([
    { format: 'gguf', partial: true, integrity: false },
    { format: 'gguf', partial: false, integrity: true },
    { format: 'onnx', partial: false, integrity: false },
  ])('does not offer format conversion for $format partial=$partial integrity=$integrity', ({ format, partial, integrity }) => {
    render(<LocalModelInstalledActions model={createModel({ primaryFormat: format, hasIntegrityIssue: integrity })}
      rowState={createRowState({ isPartialDownload: partial })} selectedAppId={null}
      onToggleLink={vi.fn()} onConvertModel={vi.fn()} />);
    expect(screen.queryByRole('button', { name: 'Convert model format' })).not.toBeInTheDocument();
  });

  it('renders a grey retained progress ring for resumable partial downloads', () => {
    render(
      <LocalModelInstalledActions
        model={createModel({ isPartialDownload: true })}
        rowState={createRowState({
          canRecoverPartial: true,
          hasRetainedProgressRing: true,
          isPartialDownload: true,
          ringDegrees: 144,
        })}
        selectedAppId="ollama"
        onRecoverPartialDownload={vi.fn()}
        onToggleLink={vi.fn()}
      />
    );

    const button = screen.getByRole('button', { name: /resume partial download/i });
    expect(button.querySelector('.download-progress-ring.is-retained')).not.toBeNull();
  });

  it('renders a disabled grey retained progress ring when the partial download is not recoverable', () => {
    render(
      <LocalModelInstalledActions
        model={createModel({ isPartialDownload: true })}
        rowState={createRowState({
          canRecoverPartial: false,
          hasRetainedProgressRing: true,
          isPartialDownload: true,
          ringDegrees: 360,
        })}
        selectedAppId="ollama"
        onRecoverPartialDownload={vi.fn()}
        onToggleLink={vi.fn()}
      />
    );

    const button = screen.getByRole('button', { name: /partial download/i });
    expect(button).toBeDisabled();
    expect(button.querySelector('.download-progress-ring.is-retained')).not.toBeNull();
  });

  it('renders a serve action for installed models', () => {
    render(
      <LocalModelInstalledActions
        model={createModel({ primaryFormat: 'gguf' })}
        rowState={createRowState()}
        selectedAppId="ollama"
        onServeModel={vi.fn()}
        onToggleLink={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /serve model/i })).toBeEnabled();
  });

  it('marks the serve action when the model is loaded', () => {
    render(
      <LocalModelInstalledActions
        model={createModel({ primaryFormat: 'gguf' })}
        rowState={createRowState()}
        selectedAppId="llama-cpp"
        servedStatus={{
          model_id: 'model-1',
          model_alias: 'model-1',
          provider: 'llama_cpp',
          profile_id: 'llama-profile',
          load_state: 'loaded',
          device_mode: 'cpu',
          keep_loaded: true,
        }}
        onServeModel={vi.fn()}
        onToggleLink={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /unload model/i })).toBeEnabled();
  });
});
