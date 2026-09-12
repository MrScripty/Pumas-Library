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
  it('renders cached models as display-only without model action controls', () => {
    const callback = vi.fn();
    render(<LocalModelsList
      modelGroups={[{ category: 'llm', models: [{
        id: 'cached', name: 'Test Model', category: 'llm', format: 'gguf',
        provenance: 'cached', relatedAvailable: true,
      }] }]}
      starredModels={new Set()} excludedModels={new Set()} selectedAppId="ollama"
      totalModels={1} hasFilters={false} relatedModelsById={{}} expandedRelated={new Set()}
      onToggleStar={callback} onToggleLink={callback} onToggleRelated={callback}
      onOpenRelatedUrl={callback} onServeModel={callback} onDeleteModel={callback}
      onRecoverPartialDownload={callback} onConvertModel={callback}
    />);
    expect(screen.getByText('Test Model')).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Test Model' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Star')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Convert model format' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /delete|link|serve|load/i })).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Test Model'), { ctrlKey: true });
    expect(screen.queryByTestId('metadata-modal')).not.toBeInTheDocument();
    expect(callback).not.toHaveBeenCalled();
  });
  it('renders format, quant, size, and dependency badge for local models', () => {
    render(
      <LocalModelsList
        modelGroups={modelGroups}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="ollama"
        totalModels={1}
        hasFilters={false}
        relatedModelsById={{}}
        expandedRelated={new Set()}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={vi.fn()}
        onServeModel={vi.fn()}
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

  it('renders backend-confirmed loaded state for served models', () => {
    render(
      <LocalModelsList
        modelGroups={modelGroups}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="llama-cpp"
        servedModels={[
          {
            model_id: 'llm/llama/test-model',
            model_alias: 'llm/llama/test-model',
            provider: 'llama_cpp',
            profile_id: 'llama-profile',
            load_state: 'loaded',
            device_mode: 'cpu',
            keep_loaded: true,
            endpoint_url: 'http://127.0.0.1:20617/',
          },
        ]}
        totalModels={1}
        hasFilters={false}
        relatedModelsById={{}}
        expandedRelated={new Set()}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={vi.fn()}
        onServeModel={vi.fn()}
      />
    );

    expect(screen.getByText('Loaded')).toBeInTheDocument();
    expect(screen.getByText(/http:\/\/127.0.0.1:20617\//)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /unload model/i })).toBeEnabled();
  });

  it('renders backend-confirmed loading state for a local model', () => {
    render(
      <LocalModelsList
        modelGroups={modelGroups}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="llama-cpp"
        servedModels={[
          {
            model_id: 'llm/llama/test-model',
            model_alias: 'llm/llama/test-model',
            provider: 'llama_cpp',
            profile_id: 'llama-profile',
            load_state: 'loading',
            device_mode: 'cpu',
            keep_loaded: true,
            endpoint_url: null,
          },
        ]}
        totalModels={1}
        hasFilters={false}
        relatedModelsById={{}}
        expandedRelated={new Set()}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={vi.fn()}
        onServeModel={vi.fn()}
      />
    );

    expect(screen.getByText('Loading')).toBeInTheDocument();
    expect(screen.queryByText('Loaded')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /loading model/i })).toBeDisabled();
  });

  it.each(['loaded-first', 'loading-first'] as const)(
    'keeps Loaded actionable and also shows Loading with concurrent instances (%s)',
    (order) => {
      const loaded: NonNullable<React.ComponentProps<typeof LocalModelsList>['servedModels']>[number] = {
        model_id: 'llm/llama/test-model',
        model_alias: 'loaded-instance',
        provider: 'llama_cpp',
        profile_id: 'loaded-profile',
        load_state: 'loaded',
        device_mode: 'cpu',
        keep_loaded: true,
      };
      const loading = {
        ...loaded,
        model_alias: 'loading-instance',
        profile_id: 'loading-profile',
        load_state: 'loading' as const,
      };

      render(
        <LocalModelsList
          modelGroups={modelGroups}
          starredModels={new Set()}
          excludedModels={new Set()}
          onToggleStar={vi.fn()}
          onToggleLink={vi.fn()}
          selectedAppId="llama-cpp"
          servedModels={order === 'loaded-first' ? [loaded, loading] : [loading, loaded]}
          totalModels={1}
          hasFilters={false}
          relatedModelsById={{}}
          expandedRelated={new Set()}
          onToggleRelated={vi.fn()}
          onOpenRelatedUrl={vi.fn()}
          onServeModel={vi.fn()}
        />
      );

      expect(screen.getByText('Loaded')).toBeInTheDocument();
      expect(screen.getByText('Loading')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /unload model/i })).toBeEnabled();
      expect(screen.queryByRole('button', { name: /loading model/i })).not.toBeInTheDocument();
    }
  );

  it('does not render backend compatibility badges on local model rows', () => {
    const firstModel = modelGroups[0]?.models[0];
    if (firstModel === undefined) {
      throw new TypeError('Expected a model fixture');
    }

    render(
      <LocalModelsList
        modelGroups={[
          {
            category: 'llm',
            models: [
              {
                ...firstModel,
                compatibleEngines: ['mlx', 'vllm'],
              } as typeof firstModel,
            ],
          },
        ]}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="ollama"
        totalModels={1}
        hasFilters={false}
        relatedModelsById={{}}
        expandedRelated={new Set()}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={vi.fn()}
      />
    );

    expect(screen.queryByText('MLX')).not.toBeInTheDocument();
    expect(screen.queryByText('vLLM')).not.toBeInTheDocument();
  });

  it('opens the metadata modal on ctrl-click of a model name', () => {
    render(
      <LocalModelsList
        modelGroups={modelGroups}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="ollama"
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
    const firstModel = modelGroups[0]?.models[0];
    if (firstModel === undefined) {
      throw new TypeError('Expected a model fixture');
    }

    render(
      <LocalModelsList
        modelGroups={[
          {
            category: 'llm',
            models: [
              {
                ...firstModel,
                relatedAvailable: true,
              },
            ],
          },
        ]}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="ollama"
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

  it('renders a centered existing-library action when the local library is empty', () => {
    const onChooseExistingLibrary = vi.fn();

    render(
      <LocalModelsList
        modelGroups={[]}
        starredModels={new Set()}
        excludedModels={new Set()}
        onToggleStar={vi.fn()}
        onToggleLink={vi.fn()}
        selectedAppId="ollama"
        totalModels={0}
        hasFilters={false}
        relatedModelsById={{}}
        expandedRelated={new Set()}
        onToggleRelated={vi.fn()}
        onOpenRelatedUrl={vi.fn()}
        onChooseExistingLibrary={onChooseExistingLibrary}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /use existing library/i }));

    expect(onChooseExistingLibrary).toHaveBeenCalledTimes(1);
  });
});
