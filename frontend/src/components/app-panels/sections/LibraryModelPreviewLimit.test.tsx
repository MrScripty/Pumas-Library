import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelCategory, ModelInfo } from '../../../types/apps';
import { LIBRARY_MODEL_PREVIEW_LIMIT } from './libraryModelPreviewLimit';
import { OllamaModelSection } from './OllamaModelSection';
import { TorchModelSlotsSection } from './TorchModelSlotsSection';

const { isApiAvailableMock } = vi.hoisted(() => ({
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../../../api/adapter', () => ({
  api: {},
  getElectronAPI: () => undefined,
  isAPIAvailable: isApiAvailableMock,
}));

function buildModelGroups(extension: string, total: number): ModelCategory[] {
  const models: ModelInfo[] = Array.from({ length: total }, (_, index) => ({
    id: `model-${index}`,
    name: `Model ${index}`,
    category: 'test',
    path: `/models/model-${index}.${extension}`,
  }));

  return [
    {
      category: 'test',
      models,
    },
  ];
}

describe('library model previews', () => {
  beforeEach(() => {
    isApiAvailableMock.mockReturnValue(false);
  });

  it('limits Torch SafeTensors models to the preview count named by the overflow copy', () => {
    render(
      <TorchModelSlotsSection
        connectionUrl="http://localhost:8000"
        isRunning={true}
        modelGroups={buildModelGroups('safetensors', LIBRARY_MODEL_PREVIEW_LIMIT + 1)}
      />
    );

    expect(screen.getByText('Model 0')).toBeInTheDocument();
    expect(screen.getByText(`Model ${LIBRARY_MODEL_PREVIEW_LIMIT - 1}`)).toBeInTheDocument();
    expect(screen.queryByText(`Model ${LIBRARY_MODEL_PREVIEW_LIMIT}`)).not.toBeInTheDocument();
    expect(screen.getByText(`Showing first ${LIBRARY_MODEL_PREVIEW_LIMIT} of 21 models`))
      .toBeInTheDocument();
  });

  it('limits Ollama GGUF models to the preview count named by the overflow copy', () => {
    render(
      <OllamaModelSection
        connectionUrl="http://localhost:11434"
        isRunning={true}
        modelGroups={buildModelGroups('gguf', LIBRARY_MODEL_PREVIEW_LIMIT + 1)}
      />
    );

    expect(screen.getByText('Model 0')).toBeInTheDocument();
    expect(screen.getByText(`Model ${LIBRARY_MODEL_PREVIEW_LIMIT - 1}`)).toBeInTheDocument();
    expect(screen.queryByText(`Model ${LIBRARY_MODEL_PREVIEW_LIMIT}`)).not.toBeInTheDocument();
    expect(screen.getByText(`Showing first ${LIBRARY_MODEL_PREVIEW_LIMIT} of 21 models`))
      .toBeInTheDocument();
  });
});
