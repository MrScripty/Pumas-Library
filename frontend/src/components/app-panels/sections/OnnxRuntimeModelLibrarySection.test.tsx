import userEvent from '@testing-library/user-event';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../../../types/apps';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import type { ServedModelStatus, ServingEndpointStatus } from '../../../types/api-serving';
import { OnnxRuntimeModelLibrarySection } from './OnnxRuntimeModelLibrarySection';

const {
  clearModelRuntimeRouteMock,
  getElectronAPIMock,
  refreshRuntimeProfilesMock,
  runtimeProfileState,
  serveDialogMock,
  serveModelMock,
  setModelRuntimeRouteMock,
  validateModelServingConfigMock,
} = vi.hoisted(() => ({
  clearModelRuntimeRouteMock: vi.fn(),
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

function renderSection(
  modelGroups: ModelCategory[],
  servedModels: ServedModelStatus[] = [],
  servingEndpoint: ServingEndpointStatus | null = null
) {
  return render(
    <OnnxRuntimeModelLibrarySection
      excludedModels={new Set()}
      modelGroups={modelGroups}
      servingEndpoint={servingEndpoint}
      servedModels={servedModels}
      starredModels={new Set()}
      onToggleLink={vi.fn()}
      onToggleStar={vi.fn()}
    />
  );
}

describe('OnnxRuntimeModelLibrarySection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeProfileState.profiles = [];
    runtimeProfileState.routes = [];
    refreshRuntimeProfilesMock.mockResolvedValue(undefined);
    setModelRuntimeRouteMock.mockResolvedValue({
      success: true,
      snapshot_required: true,
    });
    clearModelRuntimeRouteMock.mockResolvedValue({
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
      clear_model_runtime_route: clearModelRuntimeRouteMock,
      validate_model_serving_config: validateModelServingConfigMock,
      serve_model: serveModelMock,
    });
  });

  it('renders only ONNX Runtime compatible local models', () => {
    renderSection([
      {
        category: 'Embedding',
        models: [
          {
            id: 'models/nomic',
            name: 'Nomic ONNX',
            category: 'Embedding',
            primaryFormat: 'onnx',
            format: 'onnx',
          },
          {
            id: 'models/llama',
            name: 'Llama GGUF',
            category: 'Chat',
            primaryFormat: 'gguf',
            format: 'gguf',
          },
          {
            id: 'models/artifact',
            name: 'Artifact ONNX',
            category: 'Embedding',
            selectedArtifactFiles: ['model.onnx'],
          },
        ],
      },
    ]);

    expect(screen.getByRole('heading', { name: 'ONNX Runtime Library' })).toBeInTheDocument();
    expect(screen.getByText('Nomic ONNX')).toBeInTheDocument();
    expect(screen.getByText('Artifact ONNX')).toBeInTheDocument();
    expect(screen.queryByText('Llama GGUF')).not.toBeInTheDocument();
  });

  it('shows an empty state when no compatible ONNX models exist', () => {
    renderSection([
      {
        category: 'Chat',
        models: [
          {
            id: 'models/llama',
            name: 'Llama GGUF',
            category: 'Chat',
            primaryFormat: 'gguf',
          },
        ],
      },
    ]);

    expect(
      screen.getByText('No local ONNX models are available for ONNX Runtime.')
    ).toBeInTheDocument();
  });

  it('filters compatible ONNX models by local search', async () => {
    const user = userEvent.setup();
    renderSection([
      {
        category: 'Embedding',
        models: [
          {
            id: 'models/alpha',
            name: 'Alpha Embedding',
            category: 'Embedding',
            primaryFormat: 'onnx',
          },
          {
            id: 'models/beta',
            name: 'Beta Encoder',
            category: 'Embedding',
            primaryFormat: 'onnx',
          },
        ],
      },
    ]);

    await user.type(screen.getByLabelText('Search ONNX Runtime models'), 'beta');

    expect(screen.queryByText('Alpha Embedding')).not.toBeInTheDocument();
    expect(screen.getByText('Beta Encoder')).toBeInTheDocument();
    expect(screen.queryByText('Download')).not.toBeInTheDocument();
  });

  it('persists a selected ONNX Runtime profile route for a model row', async () => {
    const user = userEvent.setup();
    runtimeProfileState.profiles = [
      {
        profile_id: 'onnx-cpu',
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        name: 'ONNX CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
      {
        profile_id: 'llama-cpu',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'llama.cpp CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
    ];

    renderSection([
      {
        category: 'Embedding',
        models: [
          {
            id: 'models/nomic',
            name: 'Nomic ONNX',
            category: 'Embedding',
            primaryFormat: 'onnx',
          },
        ],
      },
    ]);

    const profileSelect = screen.getByLabelText('ONNX Runtime profile for Nomic ONNX');
    await user.selectOptions(profileSelect, 'onnx-cpu');
    screen.getByRole('button', { name: 'Save ONNX Runtime route' }).focus();
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(setModelRuntimeRouteMock).toHaveBeenCalledWith({
        provider: 'onnx_runtime',
        model_id: 'models/nomic',
        profile_id: 'onnx-cpu',
        auto_load: true,
      });
    });
    expect(refreshRuntimeProfilesMock).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('llama.cpp CPU')).not.toBeInTheDocument();
  });

  it('clears a saved ONNX Runtime route when the profile selection is removed', async () => {
    const user = userEvent.setup();
    runtimeProfileState.profiles = [
      {
        profile_id: 'onnx-cpu',
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        name: 'ONNX CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        provider: 'onnx_runtime',
        model_id: 'models/nomic',
        profile_id: 'onnx-cpu',
        auto_load: true,
      },
    ];

    renderSection([
      {
        category: 'Embedding',
        models: [
          {
            id: 'models/nomic',
            name: 'Nomic ONNX',
            category: 'Embedding',
            primaryFormat: 'onnx',
          },
        ],
      },
    ]);

    const profileSelect = screen.getByLabelText('ONNX Runtime profile for Nomic ONNX');
    await user.selectOptions(profileSelect, '');
    await user.click(screen.getByRole('button', { name: 'Save ONNX Runtime route' }));

    await waitFor(() => {
      expect(clearModelRuntimeRouteMock).toHaveBeenCalledWith('onnx_runtime', 'models/nomic');
    });
    expect(refreshRuntimeProfilesMock).toHaveBeenCalledTimes(1);
  });

  it('quick serves with the saved ONNX Runtime profile route', async () => {
    const user = userEvent.setup();
    runtimeProfileState.profiles = [
      {
        profile_id: 'onnx-cpu',
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        name: 'ONNX CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        provider: 'onnx_runtime',
        model_id: 'models/nomic',
        profile_id: 'onnx-cpu',
        auto_load: true,
      },
    ];

    renderSection([
      {
        category: 'Embedding',
        models: [
          {
            id: 'models/nomic',
            name: 'Nomic ONNX',
            category: 'Embedding',
            primaryFormat: 'onnx',
          },
        ],
      },
    ]);

    await user.click(
      screen.getByRole('button', { name: 'Quick serve with selected ONNX Runtime profile' })
    );

    await waitFor(() => {
      expect(validateModelServingConfigMock).toHaveBeenCalledWith({
        model_id: 'models/nomic',
        config: expect.objectContaining({
          provider: 'onnx_runtime',
          profile_id: 'onnx-cpu',
          device_mode: 'cpu',
          keep_loaded: true,
          model_alias: null,
        }),
      });
    });
    expect(serveModelMock).toHaveBeenCalledWith({
      model_id: 'models/nomic',
      config: expect.objectContaining({
        profile_id: 'onnx-cpu',
      }),
    });
    expect(screen.getByText('Loaded')).toBeInTheDocument();
    expect(serveDialogMock).not.toHaveBeenCalled();
  });

  it('saves a newly selected ONNX Runtime profile before quick serving', async () => {
    const user = userEvent.setup();
    runtimeProfileState.profiles = [
      {
        profile_id: 'onnx-cpu',
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        name: 'ONNX CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
    ];

    renderSection([
      {
        category: 'Embedding',
        models: [
          {
            id: 'models/nomic',
            name: 'Nomic ONNX',
            category: 'Embedding',
            primaryFormat: 'onnx',
          },
        ],
      },
    ]);

    await user.selectOptions(
      screen.getByLabelText('ONNX Runtime profile for Nomic ONNX'),
      'onnx-cpu'
    );
    await user.click(
      screen.getByRole('button', { name: 'Quick serve with selected ONNX Runtime profile' })
    );

    await waitFor(() => {
      expect(setModelRuntimeRouteMock).toHaveBeenCalledWith({
        provider: 'onnx_runtime',
        model_id: 'models/nomic',
        profile_id: 'onnx-cpu',
        auto_load: true,
      });
    });
    expect(validateModelServingConfigMock).toHaveBeenCalledWith({
      model_id: 'models/nomic',
      config: expect.objectContaining({
        provider: 'onnx_runtime',
        profile_id: 'onnx-cpu',
        device_mode: 'cpu',
      }),
    });
    expect(serveModelMock).toHaveBeenCalledWith({
      model_id: 'models/nomic',
      config: expect.objectContaining({
        profile_id: 'onnx-cpu',
      }),
    });
  });

  it('opens the serve page with the saved ONNX Runtime profile for serving options', async () => {
    const user = userEvent.setup();
    runtimeProfileState.profiles = [
      {
        profile_id: 'onnx-cpu',
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        name: 'ONNX CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        provider: 'onnx_runtime',
        model_id: 'models/nomic',
        profile_id: 'onnx-cpu',
        auto_load: true,
      },
    ];

    renderSection([
      {
        category: 'Embedding',
        models: [
          {
            id: 'models/nomic',
            name: 'Nomic ONNX',
            category: 'Embedding',
            primaryFormat: 'onnx',
          },
        ],
      },
    ]);

    await user.click(screen.getByRole('button', { name: 'Serving options' }));

    expect(screen.getByText('Serve page')).toBeInTheDocument();
    expect(serveDialogMock).toHaveBeenCalledWith(
      expect.objectContaining({
        initialProfileId: 'onnx-cpu',
        providerFilter: 'onnx_runtime',
      })
    );
  });

  it('shows backend-confirmed ONNX loaded state from served model snapshots', () => {
    runtimeProfileState.profiles = [
      {
        profile_id: 'onnx-cpu',
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        name: 'ONNX CPU',
        enabled: true,
        device: { mode: 'cpu' },
        scheduler: { auto_load: false },
      },
    ];
    runtimeProfileState.routes = [
      {
        provider: 'onnx_runtime',
        model_id: 'models/nomic',
        profile_id: 'onnx-cpu',
        auto_load: true,
      },
    ];

    renderSection(
      [
        {
          category: 'Embedding',
          models: [
            {
              id: 'models/nomic',
              name: 'Nomic ONNX',
              category: 'Embedding',
              primaryFormat: 'onnx',
            },
          ],
        },
      ],
      [
        {
          model_id: 'models/nomic',
          model_alias: 'nomic',
          provider: 'onnx_runtime',
          profile_id: 'onnx-cpu',
          load_state: 'loaded',
          device_mode: 'cpu',
          keep_loaded: true,
          endpoint_url: 'http://127.0.0.1:3456/v1',
        },
      ],
      {
        endpoint_mode: 'pumas_gateway',
        endpoint_url: 'http://127.0.0.1:3456/v1',
        model_count: 1,
      }
    );

    expect(screen.getByText('1 compatible local model - Pumas gateway')).toBeInTheDocument();
    expect(screen.getByText('Loaded 1')).toHaveAttribute(
      'title',
      'http://127.0.0.1:3456/v1'
    );
    expect(
      screen.getByRole('button', { name: 'Already loaded on selected profile' })
    ).toBeDisabled();
  });
});
