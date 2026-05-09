import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../../../types/apps';
import type { ServedModelStatus } from '../../../types/api-serving';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import { LlamaCppModelLibrarySection } from './LlamaCppModelLibrarySection';

const {
  getElectronAPIMock,
  refreshRuntimeProfilesMock,
  runtimeProfileState,
  serveDialogMock,
  serveModelMock,
  setModelRuntimeRouteMock,
  validateModelServingConfigMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  refreshRuntimeProfilesMock: vi.fn(),
  runtimeProfileState: {
    profiles: [] as RuntimeProfileConfig[],
    routes: [] as ModelRuntimeRoute[],
  },
  serveDialogMock: vi.fn(),
  serveModelMock: vi.fn(),
  setModelRuntimeRouteMock: vi.fn(),
  validateModelServingConfigMock: vi.fn(),
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

vi.mock('../../ModelServeDialog', () => ({
  ModelServeDialog: (props: unknown) => {
    serveDialogMock(props);
    return <div>Serve page</div>;
  },
}));

function renderSection(modelGroups: ModelCategory[], servedModels: ServedModelStatus[] = []) {
  return render(
    <LlamaCppModelLibrarySection
      excludedModels={new Set()}
      modelGroups={modelGroups}
      servedModels={servedModels}
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
    validateModelServingConfigMock.mockResolvedValue({
      success: true,
      valid: true,
      errors: [],
      warnings: [],
    });
    serveModelMock.mockResolvedValue({
      success: true,
      loaded: true,
      loaded_models_unchanged: false,
      status: null,
    });
    getElectronAPIMock.mockReturnValue({
      set_model_runtime_route: setModelRuntimeRouteMock,
      clear_model_runtime_route: vi.fn(),
      validate_model_serving_config: validateModelServingConfigMock,
      serve_model: serveModelMock,
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

  it('filters compatible models by local search without showing remote download mode', () => {
    renderSection([
      {
        category: 'Chat',
        models: [
          {
            id: 'models/alpha',
            name: 'Alpha Chat',
            category: 'Chat',
            primaryFormat: 'gguf',
          },
          {
            id: 'models/beta',
            name: 'Beta Embedding',
            category: 'Embedding',
            primaryFormat: 'gguf',
          },
        ],
      },
    ]);

    fireEvent.change(screen.getByLabelText('Search llama.cpp models'), {
      target: { value: 'beta' },
    });

    expect(screen.queryByText('Alpha Chat')).not.toBeInTheDocument();
    expect(screen.getByText('Beta Embedding')).toBeInTheDocument();
    expect(screen.queryByText('Download')).not.toBeInTheDocument();
  });

  it('shows backend-confirmed placement and failed load state ahead of requested placement', () => {
    runtimeProfileState.profiles = [
      {
        profile_id: 'llama-gpu',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'Emily GPU',
        enabled: true,
        device: { mode: 'gpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        model_id: 'models/llama-loaded',
        profile_id: 'llama-gpu',
        auto_load: true,
      },
      {
        model_id: 'models/llama-failed',
        profile_id: 'llama-gpu',
        auto_load: true,
      },
    ];
    renderSection(
      [
        {
          category: 'Chat',
          models: [
            {
              id: 'models/llama-loaded',
              name: 'Loaded Llama',
              category: 'Chat',
              primaryFormat: 'gguf',
            },
            {
              id: 'models/llama-failed',
              name: 'Failed Llama',
              category: 'Chat',
              primaryFormat: 'gguf',
            },
          ],
        },
      ],
      [
        {
          model_id: 'models/llama-loaded',
          model_alias: 'loaded-llama',
          provider: 'llama_cpp',
          profile_id: 'llama-gpu',
          load_state: 'loaded',
          device_mode: 'gpu',
          keep_loaded: true,
        },
        {
          model_id: 'models/llama-failed',
          model_alias: 'failed-llama',
          provider: 'llama_cpp',
          profile_id: 'llama-gpu',
          load_state: 'failed',
          device_mode: 'gpu',
          keep_loaded: true,
          last_error: {
            code: 'provider_load_failed',
            severity: 'non_critical',
            message: 'Vulkan memory allocation failed',
          },
        },
      ]
    );

    expect(screen.getByText('Loaded 1')).toBeInTheDocument();
    expect(screen.getByText('Failed')).toHaveAttribute('title', 'Vulkan memory allocation failed');
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

  it('quick serves with the saved llama.cpp profile route', async () => {
    runtimeProfileState.profiles = [
      {
        profile_id: 'llama-gpu',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'Emily GPU',
        enabled: true,
        device: { mode: 'gpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        model_id: 'models/llama-gguf',
        profile_id: 'llama-gpu',
        auto_load: true,
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

    fireEvent.click(
      screen.getByRole('button', { name: 'Quick serve with selected llama.cpp profile' })
    );

    await waitFor(() => {
      expect(validateModelServingConfigMock).toHaveBeenCalledWith({
        model_id: 'models/llama-gguf',
        config: expect.objectContaining({
          provider: 'llama_cpp',
          profile_id: 'llama-gpu',
          device_mode: 'gpu',
          context_size: 4096,
          keep_loaded: true,
          model_alias: null,
        }),
      });
    });
    expect(serveModelMock).toHaveBeenCalledWith({
      model_id: 'models/llama-gguf',
      config: expect.objectContaining({
        profile_id: 'llama-gpu',
      }),
    });
    expect(screen.getByText('Loaded')).toBeInTheDocument();
    expect(serveDialogMock).not.toHaveBeenCalled();
  });

  it('opens the serve page with the saved llama.cpp profile locked to llama.cpp for serving options', () => {
    runtimeProfileState.profiles = [
      {
        profile_id: 'llama-gpu',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'Emily GPU',
        enabled: true,
        device: { mode: 'gpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        model_id: 'models/llama-gguf',
        profile_id: 'llama-gpu',
        auto_load: true,
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

    fireEvent.click(screen.getByRole('button', { name: 'Serving options' }));

    expect(screen.getByText('Serve page')).toBeInTheDocument();
    expect(serveDialogMock).toHaveBeenCalledWith(
      expect.objectContaining({
        initialProfileId: 'llama-gpu',
        providerFilter: 'llama_cpp',
      })
    );
  });

  it('opens the serve page for duplicate alias validation instead of serving', async () => {
    runtimeProfileState.profiles = [
      {
        profile_id: 'llama-gpu',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'Emily GPU',
        enabled: true,
        device: { mode: 'gpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        model_id: 'models/llama-gguf',
        profile_id: 'llama-gpu',
        auto_load: true,
      },
    ];
    validateModelServingConfigMock.mockResolvedValue({
      success: true,
      valid: false,
      errors: [
        {
          code: 'duplicate_model_alias',
          severity: 'non_critical',
          message: 'Alias is already loaded.',
        },
      ],
      warnings: [],
    });

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

    fireEvent.click(
      screen.getByRole('button', { name: 'Quick serve with selected llama.cpp profile' })
    );

    await waitFor(() => {
      expect(screen.getByText('Serve page')).toBeInTheDocument();
    });
    expect(serveModelMock).not.toHaveBeenCalled();
  });
});
