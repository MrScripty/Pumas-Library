import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';
import { RelatedModelsPanel } from './RelatedModelsPanel';

function createRelatedModel(overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo {
  return {
    repoId: 'org/related-model',
    name: 'Related Model',
    developer: 'org',
    kind: 'text-generation',
    formats: ['gguf'],
    quants: ['Q4_K_M'],
    url: 'https://huggingface.co/org/related-model',
    ...overrides,
  };
}

describe('RelatedModelsPanel', () => {
  it('renders the loading state for unresolved related models', () => {
    render(
      <RelatedModelsPanel
        relatedModels={[]}
        relatedStatus="loading"
        onOpenRelatedUrl={vi.fn()}
      />
    );

    expect(screen.getByText('Related models')).toBeInTheDocument();
    expect(screen.getByText('Looking up related models...')).toBeInTheDocument();
  });

  it('renders an error message when related lookup fails', () => {
    render(
      <RelatedModelsPanel
        error="Hugging Face lookup failed."
        relatedModels={[]}
        relatedStatus="error"
        onOpenRelatedUrl={vi.fn()}
      />
    );

    expect(screen.getByText('Hugging Face lookup failed.')).toBeInTheDocument();
  });

  it('renders an empty state after a completed lookup with no matches', () => {
    render(
      <RelatedModelsPanel
        relatedModels={[]}
        relatedStatus="loaded"
        onOpenRelatedUrl={vi.fn()}
      />
    );

    expect(screen.getByText('No related models found.')).toBeInTheDocument();
  });

  it('renders related models and opens the selected URL', () => {
    const onOpenRelatedUrl = vi.fn();

    render(
      <RelatedModelsPanel
        relatedModels={[createRelatedModel()]}
        relatedStatus="loaded"
        onOpenRelatedUrl={onOpenRelatedUrl}
      />
    );

    expect(screen.getByText('Related Model')).toBeInTheDocument();
    expect(screen.getByText('org')).toBeInTheDocument();
    expect(screen.getByText('1')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Open' }));

    expect(onOpenRelatedUrl).toHaveBeenCalledWith(
      'https://huggingface.co/org/related-model'
    );
  });
});
