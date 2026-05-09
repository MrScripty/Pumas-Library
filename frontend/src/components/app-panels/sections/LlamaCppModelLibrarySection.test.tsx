import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../../../types/apps';
import { LlamaCppModelLibrarySection } from './LlamaCppModelLibrarySection';

vi.mock('../../ModelMetadataModal', () => ({
  ModelMetadataModal: () => null,
}));

function renderSection(modelGroups: ModelCategory[]) {
  return render(
    <LlamaCppModelLibrarySection
      excludedModels={new Set()}
      modelGroups={modelGroups}
      servedModels={[]}
      starredModels={new Set()}
      onToggleLink={vi.fn()}
      onToggleStar={vi.fn()}
    />
  );
}

describe('LlamaCppModelLibrarySection', () => {
  it('renders only llama.cpp compatible local models', () => {
    renderSection([
      {
        category: 'Chat',
        models: [
          {
            id: 'models/llama-gguf',
            name: 'Llama GGUF',
            category: 'Chat',
            primaryFormat: 'gguf',
            format: 'gguf',
          },
          {
            id: 'models/diffusion',
            name: 'Diffusion Safetensors',
            category: 'Checkpoint',
            primaryFormat: 'safetensors',
            format: 'safetensors',
          },
          {
            id: 'models/artifact-gguf',
            name: 'Artifact GGUF',
            category: 'Embedding',
            selectedArtifactFiles: ['model.Q4_K_M.gguf'],
          },
        ],
      },
    ]);

    expect(screen.getByRole('heading', { name: 'llama.cpp Library' })).toBeInTheDocument();
    expect(screen.getByText('Llama GGUF')).toBeInTheDocument();
    expect(screen.getByText('Artifact GGUF')).toBeInTheDocument();
    expect(screen.queryByText('Diffusion Safetensors')).not.toBeInTheDocument();
  });

  it('shows an empty state when no compatible GGUF models exist', () => {
    renderSection([
      {
        category: 'Images',
        models: [
          {
            id: 'models/image',
            name: 'Image Model',
            category: 'Images',
            format: 'safetensors',
          },
        ],
      },
    ]);

    expect(screen.getByText('No local GGUF models are available for llama.cpp.')).toBeInTheDocument();
  });
});
