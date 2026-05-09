import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../../../types/apps';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import { LlamaCppModelLibrarySection } from './LlamaCppModelLibrarySection';

const {
  getElectronAPIMock,
  refreshRuntimeProfilesMock,
  runtimeProfileState,
  setModelRuntimeRouteMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  refreshRuntimeProfilesMock: vi.fn(),
  runtimeProfileState: {
    profiles: [] as RuntimeProfileConfig[],
    routes: [] as ModelRuntimeRoute[],
  },
  setModelRuntimeRouteMock: vi.fn(),
}));

vi.mock('../../../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

vi.mock('../../../hooks/useRuntimeProfiles', () => ({
  useRuntimeProfiles: () => ({
    snapshot: null,
    profiles: runtimeProfileState.profiles,
    routes: runtimeProfileState.routes,
    statuses: [],
    defaultProfileId: null,
    cursor: null,
    isLoading: false,
    error: null,
    refreshRuntimeProfiles: refreshRuntimeProfilesMock,
  }),
}));

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
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeProfileState.profiles = [];
    runtimeProfileState.routes = [];
    refreshRuntimeProfilesMock.mockResolvedValue(undefined);
    setModelRuntimeRouteMock.mockResolvedValue({
      success: true,
      snapshot_required: true,
    });
    getElectronAPIMock.mockReturnValue({
      set_model_runtime_route: setModelRuntimeRouteMock,
      clear_model_runtime_route: vi.fn(),
    });
  });

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

  it('persists a selected llama.cpp profile route for a model row', async () => {
    runtimeProfileState.profiles = [
      {
        profile_id: 'llama-cpu',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'Emily CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
      {
        profile_id: 'ollama-default',
        provider: 'ollama',
        provider_mode: 'ollama_serve',
        management_mode: 'managed',
        name: 'Ollama',
        enabled: true,
        device: { mode: 'auto' },
        scheduler: { auto_load: false },
      },
    ];
    renderSection([
      {
        category: 'Chat',
        models: [
          {
            id: 'models/llama-gguf',
            name: 'Llama GGUF',
            category: 'Chat',
            primaryFormat: 'gguf',
          },
        ],
      },
    ]);

    const profileSelect = screen.getByLabelText('llama.cpp profile for Llama GGUF');
    fireEvent.change(profileSelect, { target: { value: 'llama-cpu' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save llama.cpp route' }));

    await waitFor(() => {
      expect(setModelRuntimeRouteMock).toHaveBeenCalledWith({
        model_id: 'models/llama-gguf',
        profile_id: 'llama-cpu',
        auto_load: true,
      });
    });
    expect(refreshRuntimeProfilesMock).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('Ollama')).not.toBeInTheDocument();
  });
});
