import userEvent from '@testing-library/user-event';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../../../types/apps';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import { OnnxRuntimeModelLibrarySection } from './OnnxRuntimeModelLibrarySection';

const {
  clearModelRuntimeRouteMock,
  getElectronAPIMock,
  refreshRuntimeProfilesMock,
  runtimeProfileState,
  setModelRuntimeRouteMock,
} = vi.hoisted(() => ({
  clearModelRuntimeRouteMock: vi.fn(),
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
    <OnnxRuntimeModelLibrarySection
      excludedModels={new Set()}
      modelGroups={modelGroups}
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
    getElectronAPIMock.mockReturnValue({
      set_model_runtime_route: setModelRuntimeRouteMock,
      clear_model_runtime_route: clearModelRuntimeRouteMock,
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
    await user.click(screen.getByRole('button', { name: 'Save ONNX Runtime route' }));

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
});
