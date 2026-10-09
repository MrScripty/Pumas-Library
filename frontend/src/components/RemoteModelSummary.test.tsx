import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';
import { RemoteModelSummary } from './RemoteModelSummary';

function createModel(overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo {
  return {
    repoId: 'org/test-model',
    name: 'Test Model',
    developer: 'org',
    kind: 'text-generation',
    formats: ['gguf'],
    quants: ['Q4_K_M'],
    downloadOptions: [
      {
        quant: 'Q4_K_M',
        sizeBytes: 2 * 1024 ** 3,
        fileGroup: null,
      },
    ],
    url: 'https://huggingface.co/org/test-model',
    releaseDate: '2026-01-01T00:00:00Z',
    downloads: 1234,
    compatibleEngines: ['ollama', 'llama.cpp'],
    ...overrides,
  };
}

describe('RemoteModelSummary', () => {
  it('shows stale cached source, exact time and observed revision without readiness badges', () => {
    render(<RemoteModelSummary model={createModel({ compatibleEngines: [], cachedDiscovery: {
      sourceUrl: 'https://huggingface.co/api/models/org/test-model', observedAt: '2026-01-01T00:00:00Z',
      freshness: 'stale', freshUntil: '2026-01-02T00:00:00Z', revisionObserved: 'a'.repeat(40),
    } })} quantLabels={[]} isHydratingDetails={false} />);
    expect(screen.getByText('Cached observation · stale')).toBeInTheDocument();
    expect(screen.getByText(/Source: https:\/\/huggingface.co\/api\/models\/org\/test-model/)).toBeInTheDocument();
    expect(screen.getByText('2026-01-01T00:00:00Z')).toHaveAttribute('dateTime', '2026-01-01T00:00:00Z');
    expect(screen.getByText('a'.repeat(40))).toBeInTheDocument();
    expect(screen.queryByText('ollama')).not.toBeInTheDocument();
  });

  it('labels cache TTL at search time and missing revision without claiming current catalog data', () => {
    render(<RemoteModelSummary model={createModel({ cachedDiscovery: {
      sourceUrl: 'https://huggingface.co/api/models/org/test-model', observedAt: '2026-01-01T00:00:00Z',
      freshness: 'fresh', freshUntil: null, revisionObserved: null,
    } })} quantLabels={[]} isHydratingDetails={false} />);
    expect(screen.getByText('Cached observation · within TTL at search')).toBeInTheDocument();
    expect(screen.getByText('Not recorded')).toBeInTheDocument();
    expect(screen.getByText(/does not confirm current access/)).toBeInTheDocument();
  });
  it('renders model metadata, download details, and engine badges', () => {
    render(
      <RemoteModelSummary
        model={createModel()}
        quantLabels={['Q4_K_M']}
        isHydratingDetails={false}
      />
    );

    expect(screen.getByText('Test Model')).toBeInTheDocument();
    expect(screen.getByText('gguf')).toBeInTheDocument();
    expect(screen.getByText('Q4_K_M')).toBeInTheDocument();
    expect(screen.getByText('2.00 GB')).toBeInTheDocument();
    expect(screen.getByText('1,234')).toBeInTheDocument();
    expect(screen.getByText('ollama')).toBeInTheDocument();
    expect(screen.getByText('llama.cpp')).toBeInTheDocument();
  });

  it('renders MLX and vLLM compatibility tags for Hugging Face search results', () => {
    render(
      <RemoteModelSummary
        model={createModel({
          formats: ['safetensors'],
          compatibleEngines: ['transformers', 'vllm', 'mlx'],
        })}
        quantLabels={[]}
        isHydratingDetails={false}
      />
    );

    expect(screen.getByText('transformers')).toBeInTheDocument();
    expect(screen.getByText('vllm')).toBeInTheDocument();
    expect(screen.getByText('mlx')).toBeInTheDocument();
  });

  it('searches by developer when the developer action is available', () => {
    const onSearchDeveloper = vi.fn();

    render(
      <RemoteModelSummary
        model={createModel()}
        quantLabels={[]}
        isHydratingDetails={false}
        onSearchDeveloper={onSearchDeveloper}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /org/i }));

    expect(onSearchDeveloper).toHaveBeenCalledWith('org');
  });

  it('renders auth and retry affordances for download errors', () => {
    const onHfAuthClick = vi.fn();

    render(
      <RemoteModelSummary
        model={createModel()}
        quantLabels={[]}
        isHydratingDetails={true}
        modelError="401 unauthorized"
        retryHint="Retrying attempt 1/unlimited"
        onHfAuthClick={onHfAuthClick}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /sign in to huggingface/i }));

    expect(screen.getByText('401 unauthorized')).toBeInTheDocument();
    expect(screen.getByText('Retrying attempt 1/unlimited')).toBeInTheDocument();
    expect(onHfAuthClick).toHaveBeenCalledTimes(1);
  });
});
