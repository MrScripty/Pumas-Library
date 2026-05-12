import userEvent from '@testing-library/user-event';
import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../types/api-runtime-profiles';
import { ModelRuntimeRouteEditor } from './ModelRuntimeRouteEditor';

const {
  clearModelRuntimeRouteMock,
  refreshRuntimeProfilesMock,
  runtimeProfileState,
  saveModelRuntimeRouteMock,
  serveDialogMock,
} = vi.hoisted(() => ({
  clearModelRuntimeRouteMock: vi.fn(),
  refreshRuntimeProfilesMock: vi.fn(),
  runtimeProfileState: {
    profiles: [] as RuntimeProfileConfig[],
    routes: [] as ModelRuntimeRoute[],
  },
  saveModelRuntimeRouteMock: vi.fn(),
  serveDialogMock: vi.fn(),
}));

vi.mock('../hooks/useRuntimeProfiles', () => ({
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

vi.mock('./model-serve/runtimeRouteMutations', () => ({
  clearModelRuntimeRoute: clearModelRuntimeRouteMock,
  saveModelRuntimeRoute: saveModelRuntimeRouteMock,
}));

vi.mock('./ModelServeDialog', () => ({
  ModelServeDialog: (props: unknown) => {
    serveDialogMock(props);
    return <div>Serve page</div>;
  },
}));

function llamaProfile(): RuntimeProfileConfig {
  return {
    profile_id: 'llama-cpu',
    provider: 'llama_cpp',
    provider_mode: 'llama_cpp_dedicated',
    management_mode: 'managed',
    name: 'llama.cpp CPU',
    enabled: true,
    device: { mode: 'cpu' },
    scheduler: { auto_load: false },
  };
}

function onnxProfile(): RuntimeProfileConfig {
  return {
    profile_id: 'onnx-cpu',
    provider: 'onnx_runtime',
    provider_mode: 'onnx_serve',
    management_mode: 'managed',
    name: 'ONNX CPU',
    enabled: true,
    device: { mode: 'cpu' },
    scheduler: { auto_load: false },
  };
}

function renderEditor() {
  return render(
    <ModelRuntimeRouteEditor
      modelId="models/shared"
      modelName="Shared Model"
      primaryFile="model.onnx"
    />
  );
}

describe('ModelRuntimeRouteEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeProfileState.profiles = [llamaProfile(), onnxProfile()];
    runtimeProfileState.routes = [
      {
        provider: 'llama_cpp',
        model_id: 'models/shared',
        profile_id: 'llama-cpu',
        auto_load: true,
      },
      {
        provider: 'onnx_runtime',
        model_id: 'models/shared',
        profile_id: 'onnx-cpu',
        auto_load: false,
      },
    ];
    refreshRuntimeProfilesMock.mockResolvedValue(undefined);
    saveModelRuntimeRouteMock.mockResolvedValue(undefined);
    clearModelRuntimeRouteMock.mockResolvedValue(undefined);
  });

  it('clears the route for the selected profile provider instead of a model-only route', async () => {
    const user = userEvent.setup();
    renderEditor();

    const profileSelect = screen.getByLabelText('Runtime Profile');
    expect(profileSelect).toHaveValue('');

    await user.selectOptions(profileSelect, 'onnx-cpu');
    await user.click(screen.getByRole('button', { name: 'Clear' }));

    await waitFor(() => {
      expect(clearModelRuntimeRouteMock).toHaveBeenCalledWith('onnx_runtime', 'models/shared');
    });
    expect(refreshRuntimeProfilesMock).toHaveBeenCalledTimes(1);
  });

  it('saves a route using the provider from the selected profile', async () => {
    const user = userEvent.setup();
    renderEditor();

    await user.selectOptions(screen.getByLabelText('Runtime Profile'), 'llama-cpu');
    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(saveModelRuntimeRouteMock).toHaveBeenCalledWith({
        provider: 'llama_cpp',
        modelId: 'models/shared',
        profileId: 'llama-cpu',
        autoLoad: true,
      });
    });
  });

  it('opens serving options with the selected profile provider filter', async () => {
    const user = userEvent.setup();
    renderEditor();

    await user.selectOptions(screen.getByLabelText('Runtime Profile'), 'onnx-cpu');
    await user.click(screen.getByRole('button', { name: 'Serve' }));

    expect(screen.getByText('Serve page')).toBeInTheDocument();
    expect(serveDialogMock).toHaveBeenCalledWith(
      expect.objectContaining({
        initialProfileId: 'onnx-cpu',
        providerFilter: 'onnx_runtime',
      })
    );
  });
});
