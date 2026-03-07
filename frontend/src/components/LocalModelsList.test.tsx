import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { LocalModelsList } from './LocalModelsList';
import type { ModelCategory } from '../types/apps';

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
});
