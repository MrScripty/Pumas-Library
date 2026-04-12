import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { LocalModelsList } from './LocalModelsList';
import type { ModelCategory, RelatedModelsState } from '../types/apps';

vi.mock('./ModelMetadataModal', () => ({
  ModelMetadataModal: ({
    modelId,
    modelName,
  }: {
    modelId: string;
    modelName: string;
    onClose: () => void;
  }) => (
    <div data-testid="metadata-modal">
      {modelId}:{modelName}
    </div>
  ),
}));

const modelGroups: ModelCategory[] = [
  {
    category: 'llm',
    models: [
      {
        id: 'llm/llama/test-model',
        name: 'Test Model',
        category: 'llm',
        modelDir: '/tmp/models/llm/llama/test-model',
        format: 'gguf',
        quant: 'Q4_K_M',
        size: 1024 ** 3,
        hasDependencies: true,
        dependencyCount: 1,
        primaryFormat: 'gguf',
      },
    ],
  },
];

describe('LocalModelsList', () => {
  it('renders format, quant, size, and dependency badge for local models', () => {
    render(
      <LocalModelsList
        modelGroups={modelGroups}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="comfyui"
        totalModels={1}
        hasFilters={false}
        relatedModelsById={{}}
        expandedRelated={new Set()}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={vi.fn()}
      />
    );

    expect(screen.getByText('GGUF')).toBeInTheDocument();
    expect(screen.getByText('Q4_K_M')).toBeInTheDocument();
    expect(screen.getByText('1.00 GB')).toBeInTheDocument();
    expect(screen.getByText('Deps')).toBeInTheDocument();
    expect(screen.queryByText('Format')).not.toBeInTheDocument();
    expect(screen.queryByText('Quant')).not.toBeInTheDocument();
    expect(screen.queryByText('Size')).not.toBeInTheDocument();
  });

  it('opens the metadata modal on ctrl-click of a model name', () => {
    render(
      <LocalModelsList
        modelGroups={modelGroups}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="comfyui"
        totalModels={1}
        hasFilters={false}
        relatedModelsById={{}}
        expandedRelated={new Set()}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /test model/i }), { ctrlKey: true });

    expect(screen.getByTestId('metadata-modal')).toHaveTextContent(
      'llm/llama/test-model:Test Model'
    );
  });

  it('renders expanded related models and opens the selected related URL', () => {
    const onOpenRelatedUrl = vi.fn();
    const relatedModelsById: Record<string, RelatedModelsState> = {
      'llm/llama/test-model': {
        status: 'loaded',
        models: [
          {
            repoId: 'org/related-model',
            name: 'Related Model',
            developer: 'org',
            kind: 'text-generation',
            formats: ['gguf'],
            quants: ['Q4_K_M'],
            url: 'https://huggingface.co/org/related-model',
          },
        ],
      },
    };

    render(
      <LocalModelsList
        modelGroups={[
          {
            category: 'llm',
            models: [
              {
                ...modelGroups[0]!.models[0]!,
                relatedAvailable: true,
              },
            ],
          },
        ]}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="comfyui"
        totalModels={1}
        hasFilters={false}
        relatedModelsById={relatedModelsById}
        expandedRelated={new Set(['llm/llama/test-model'])}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={onOpenRelatedUrl}
      />
    );

    expect(screen.getByText('Related models')).toBeInTheDocument();
    expect(screen.getByText('Related Model')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Open' }));

    expect(onOpenRelatedUrl).toHaveBeenCalledWith(
      'https://huggingface.co/org/related-model'
    );
  });
});
