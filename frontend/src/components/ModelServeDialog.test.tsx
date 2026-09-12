import { useState } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { RuntimeProfilesSnapshot } from '../types/api-runtime-profiles';
import type {
  ModelServeValidationResponse,
  ServeModelRequest,
  ServeModelResponse,
  ServedModelStatus,
} from '../types/api-serving';
import { ModelServeDialog } from './ModelServeDialog';

const { getElectronAPIMock, useRuntimeProfilesMock, useServingStatusMock } = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  useRuntimeProfilesMock: vi.fn(),
  useServingStatusMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

vi.mock('../hooks/useRuntimeProfiles', () => ({
  useRuntimeProfiles: useRuntimeProfilesMock,
}));

vi.mock('../hooks/useServingStatus', () => ({
  useServingStatus: useServingStatusMock,
}));

const snapshot: RuntimeProfilesSnapshot = {
    schema_version: 1,
    cursor: 'runtime-profiles:1',
    profiles: [
      {
        profile_id: 'ollama-default',
        provider: 'ollama',
        provider_mode: 'ollama_serve',
        management_mode: 'managed',
        name: 'Ollama Default',
        enabled: true,
        endpoint_url: 'http://127.0.0.1:11434/',
        port: 11434,
        device: { mode: 'auto' },
        scheduler: { auto_load: true },
      },
      {
        profile_id: 'emily-llama',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'Emily Llama',
        enabled: true,
        endpoint_url: null,
        port: null,
        device: { mode: 'gpu', gpu_layers: 32 },
        scheduler: { auto_load: true },
      },
      {
        profile_id: 'cpu-llama',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'CPU Llama',
        enabled: true,
        endpoint_url: null,
        port: null,
        device: { mode: 'cpu' },
        scheduler: { auto_load: true },
      },
      {
        profile_id: 'router-llama',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_router',
        management_mode: 'managed',
        name: 'Router Llama',
        enabled: true,
        endpoint_url: 'http://127.0.0.1:18080',
        port: 18080,
        device: { mode: 'gpu', gpu_layers: 20, tensor_split: [1, 1] },
        scheduler: { auto_load: true },
      },
    ],
    routes: [],
    statuses: [],
    default_profile_id: 'ollama-default',
  };

beforeEach(() => {
    vi.clearAllMocks();
    useRuntimeProfilesMock.mockReturnValue({
      snapshot,
      profiles: snapshot.profiles,
      routes: snapshot.routes,
      statuses: snapshot.statuses,
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });
    useServingStatusMock.mockReturnValue({
      snapshot: null,
      servedModels: [],
      endpoint: null,
      cursor: null,
      error: null,
      controlObservation: { kind: 'known', rows: [] },
      refreshServingStatus: vi.fn(),
    });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: vi.fn().mockResolvedValue({
        success: true,
        snapshot: {
          cursor: 'serving:0',
          endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
          served_models: [],
          recent_errors: [],
        },
      }),
    });
  });

describe('ModelServeDialog configuration', () => {
  it('uses the shared modal lifecycle without applying it to page mode', async () => {
    function DialogHarness() {
      const [isOpen, setIsOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setIsOpen(true)}>Open serving</button>
          {isOpen && (
            <ModelServeDialog
              model={{
                id: 'modal-model',
                name: 'Modal Model',
                category: 'local',
                primaryFormat: 'gguf',
              }}
              initialProfileId="emily-llama"
              onClose={() => setIsOpen(false)}
            />
          )}
        </>
      );
    }

    render(<DialogHarness />);
    const trigger = screen.getByRole('button', { name: 'Open serving' });
    trigger.focus();
    fireEvent.click(trigger);
    expect(await screen.findByRole('dialog', { name: 'Serve Modal Model' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveFocus());

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Serve Modal Model' })).not.toBeInTheDocument();
    });
    expect(trigger).toHaveFocus();

    trigger.focus();
    render(
      <ModelServeDialog
        model={{
          id: 'page-model',
          name: 'Page Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        displayMode="page"
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );
    expect(screen.queryByRole('dialog', { name: 'Serve Page Model' })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it('uses the route editor selected profile when opening the dialog', async () => {
    render(
      <ModelServeDialog
        model={{
          id: 'model-1',
          name: 'Model One',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('emily-llama');
    expect(screen.getByText('Ready to serve Model One with Emily Llama.')).toBeInTheDocument();
  });

  it('explains why the serve action is blocked', () => {
    render(
      <ModelServeDialog
        model={{
          id: 'model-2',
          name: 'Model Two',
          category: 'local',
          primaryFormat: 'safetensors',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(
      screen.getByText('Cannot serve yet: Only GGUF models can be served with the selected provider.')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start serving' })).toBeEnabled();
  });

  it('hides GPU-only controls for CPU dedicated profiles and defaults context', () => {
    render(
      <ModelServeDialog
        model={{
          id: 'model-3',
          name: 'Model Three',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        displayMode="page"
        initialProfileId="cpu-llama"
        onBack={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('cpu-llama');
    expect(screen.getByRole('combobox', { name: /model device/i })).toHaveValue('cpu');
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096);
    expect(screen.queryByRole('spinbutton', { name: /model gpu layers/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: /model tensor split/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: /device id/i })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start serving' })).toBeEnabled();
  });

  it('uses router profile placement with launch-level context', () => {
    render(
      <ModelServeDialog
        model={{
          id: 'model-router',
          name: 'Router Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        displayMode="page"
        initialProfileId="router-llama"
        onBack={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('router-llama');
    expect(
      screen.getByText('Model placement comes from the selected runtime target.')
    ).toBeInTheDocument();
    expect(screen.queryByRole('combobox', { name: /model device/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('spinbutton', { name: /model gpu layers/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: /model tensor split/i })).not.toBeInTheDocument();
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096);
    expect(screen.getByRole('button', { name: 'Start serving' })).toBeEnabled();
  });

  it('prefers a running llama.cpp target for GGUF models when no route is selected', () => {
    useRuntimeProfilesMock.mockReturnValue({
      snapshot: {
        ...snapshot,
        statuses: [
          {
            profile_id: 'emily-llama',
            state: 'running',
            endpoint_url: 'http://127.0.0.1:18080',
            pid: 1234,
            log_path: null,
            last_error: null,
          },
        ],
      },
      profiles: snapshot.profiles,
      routes: snapshot.routes,
      statuses: [
        {
          profile_id: 'emily-llama',
          state: 'running',
          endpoint_url: 'http://127.0.0.1:18080',
          pid: 1234,
          log_path: null,
          last_error: null,
        },
      ],
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-4',
          name: 'Model Four',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('emily-llama');
    expect(screen.getByText('Ready to serve Model Four with Emily Llama.')).toBeInTheDocument();
  });

  it('uses a saved ONNX route when opening the ONNX-filtered serve dialog', () => {
    const onnxProfile = {
      profile_id: 'onnx-cpu',
      provider: 'onnx_runtime' as const,
      provider_mode: 'onnx_serve' as const,
      management_mode: 'managed' as const,
      name: 'ONNX CPU',
      enabled: true,
      endpoint_url: null,
      port: null,
      device: { mode: 'cpu' as const },
      scheduler: { auto_load: true },
    };
    useRuntimeProfilesMock.mockReturnValue({
      snapshot: {
        ...snapshot,
        profiles: [...snapshot.profiles, onnxProfile],
        routes: [
          {
            provider: 'onnx_runtime',
            model_id: 'embeddings/nomic/model',
            profile_id: 'onnx-cpu',
            auto_load: true,
          },
        ],
        statuses: [
          {
            profile_id: 'onnx-cpu',
            state: 'running',
            endpoint_url: null,
            pid: null,
            log_path: null,
            last_error: null,
          },
        ],
      },
      profiles: [...snapshot.profiles, onnxProfile],
      routes: [
        {
          provider: 'onnx_runtime',
          model_id: 'embeddings/nomic/model',
          profile_id: 'onnx-cpu',
          auto_load: true,
        },
      ],
      statuses: [
        {
          profile_id: 'onnx-cpu',
          state: 'running',
          endpoint_url: null,
          pid: null,
          log_path: null,
          last_error: null,
        },
      ],
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'embeddings/nomic/model',
          name: 'Nomic ONNX',
          category: 'local',
          format: 'onnx',
        }}
        providerFilter="onnx_runtime"
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('onnx-cpu');
    expect(screen.getByText('Ready to serve Nomic ONNX with ONNX CPU.')).toBeInTheDocument();
  });

  it('does not fall back to the first ONNX profile when no ONNX route is saved', () => {
    const onnxProfile = {
      profile_id: 'onnx-cpu',
      provider: 'onnx_runtime' as const,
      provider_mode: 'onnx_serve' as const,
      management_mode: 'managed' as const,
      name: 'ONNX CPU',
      enabled: true,
      endpoint_url: null,
      port: null,
      device: { mode: 'cpu' as const },
      scheduler: { auto_load: true },
    };
    useRuntimeProfilesMock.mockReturnValue({
      snapshot: {
        ...snapshot,
        profiles: [...snapshot.profiles, onnxProfile],
      },
      profiles: [...snapshot.profiles, onnxProfile],
      routes: [],
      statuses: [],
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'embeddings/nomic/model',
          name: 'Nomic ONNX',
          category: 'local',
          format: 'onnx',
        }}
        providerFilter="onnx_runtime"
        onClose={vi.fn()}
      />
    );

    expect(
      screen.getByText('Cannot serve yet: Select a runtime target before serving.')
    ).toBeInTheDocument();
    expect(screen.getByText('Provider')).toBeInTheDocument();
    expect(screen.getAllByText('none').length).toBeGreaterThan(0);
  });
});

describe('ModelServeDialog actions', () => {
  it('does not present a stale profile state after profile refresh fails', () => {
    useRuntimeProfilesMock.mockReturnValue({
      snapshot,
      profiles: snapshot.profiles,
      routes: snapshot.routes,
      statuses: [{ profile_id: 'emily-llama', state: 'stopped' }],
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: 'profile refresh unavailable',
      refreshRuntimeProfiles: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-stale-status',
          name: 'Model Stale Status',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(screen.getByText(/profile refresh unavailable/)).toBeInTheDocument();
    expect(screen.getByText('unknown')).toBeInTheDocument();
    expect(screen.queryByText('stopped')).not.toBeInTheDocument();
  });

  it.each(['false outcome', 'transport rejection'] as const)(
    'refreshes profile status once after a serve %s without retrying',
    async (scenario) => {
      const refreshRuntimeProfiles = vi.fn();
      useRuntimeProfilesMock.mockImplementation(() => {
        const [profiles, setProfiles] = useState(snapshot.profiles);
        refreshRuntimeProfiles.mockImplementation(async () => {
          setProfiles((current) => current.map((profile) => ({ ...profile })));
        });
        return {
          snapshot: { ...snapshot, profiles },
          profiles,
          routes: snapshot.routes,
          statuses: snapshot.statuses,
          defaultProfileId: snapshot.default_profile_id,
          cursor: snapshot.cursor,
          isLoading: false,
          error: null,
          refreshRuntimeProfiles,
        };
      });
      const serveModel = vi.fn();
      if (scenario === 'transport rejection') {
        serveModel.mockRejectedValue(new Error('transport unavailable'));
      } else {
        serveModel.mockResolvedValue({
          success: true,
          loaded: false,
          loaded_models_unchanged: true,
          status: null,
          load_error: {
            code: 'provider_load_failed',
            message: 'exact load failure',
            severity: 'non_critical',
            provider: 'llama_cpp',
            model_id: 'model-refresh-failure',
            profile_id: 'emily-llama',
          },
          snapshot: null,
        });
      }
      getElectronAPIMock.mockReturnValue({
        get_serving_status: vi.fn().mockResolvedValue({
          success: true,
          snapshot: {
            cursor: 'serving:0',
            endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
            served_models: [],
            recent_errors: [],
          },
        }),
        validate_model_serving_config: vi.fn().mockResolvedValue({
          success: true,
          valid: true,
          errors: [],
          warnings: [],
        }),
        serve_model: serveModel,
      });

      render(
        <ModelServeDialog
          model={{
            id: 'model-refresh-failure',
            name: 'Model Refresh Failure',
            category: 'local',
            primaryFormat: 'gguf',
          }}
          initialProfileId="router-llama"
          onClose={vi.fn()}
        />
      );

      const contextInput = screen.getByRole('spinbutton', { name: /context/i });
      fireEvent.change(contextInput, { target: { value: '8192' } });
      fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));
      await waitFor(() => expect(refreshRuntimeProfiles).toHaveBeenCalledTimes(1));
      expect(serveModel).toHaveBeenCalledTimes(1);
      expect(contextInput).toHaveValue(8192);
    }
  );

  it('refreshes profile status after serving and unloading', async () => {
    const refreshRuntimeProfiles = vi.fn();
    const refreshServingStatus = vi.fn();
    const loadedStatus: ServedModelStatus = {
      model_id: 'model-refresh',
      model_alias: 'model-refresh',
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loaded',
      device_mode: 'gpu',
      keep_loaded: true,
    };
    useServingStatusMock.mockImplementation(() => {
      const [rows, setRows] = useState<ServedModelStatus[]>([]);
      refreshServingStatus.mockImplementation(async () => {
        setRows((current) => current.length === 0 ? [loadedStatus] : []);
      });
      return {
        snapshot: null,
        servedModels: rows,
        endpoint: null,
        cursor: null,
        error: null,
        controlObservation: { kind: 'known', rows },
        refreshServingStatus,
      };
    });
    useRuntimeProfilesMock.mockImplementation(() => {
      const [refreshCount, setRefreshCount] = useState(0);
      refreshRuntimeProfiles.mockImplementation(async () => setRefreshCount((count) => count + 1));
      return {
        snapshot,
        profiles: snapshot.profiles,
        routes: snapshot.routes,
        statuses: [
          {
            profile_id: 'emily-llama',
            state: refreshCount === 1 ? 'running' : 'stopped',
          },
        ],
        defaultProfileId: snapshot.default_profile_id,
        cursor: snapshot.cursor,
        isLoading: false,
        error: null,
        refreshRuntimeProfiles,
      };
    });
    const serveModel = vi.fn().mockResolvedValue({
      success: true,
      loaded: true,
      loaded_models_unchanged: false,
      status: loadedStatus,
      load_error: null,
      snapshot: null,
    });
    const unserveModel = vi.fn().mockResolvedValue({
      success: true,
      unloaded: true,
      snapshot: null,
    });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: vi.fn().mockResolvedValue({
        success: true,
        snapshot: {
          cursor: 'serving:0',
          endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
          served_models: [],
          recent_errors: [],
        },
      }),
      validate_model_serving_config: vi.fn().mockResolvedValue({
        success: true,
        valid: true,
        errors: [],
        warnings: [],
      }),
      serve_model: serveModel,
      unserve_model: unserveModel,
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-refresh',
          name: 'Model Refresh',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(screen.getByText('stopped')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));
    await waitFor(() => expect(screen.getByText('running')).toBeInTheDocument());
    expect(refreshRuntimeProfiles).toHaveBeenCalledTimes(1);

    fireEvent.click(await screen.findByRole('button', { name: 'Stop serving' }));
    await waitFor(() => expect(screen.getByText('stopped')).toBeInTheDocument());
    expect(unserveModel).toHaveBeenCalledTimes(1);
    expect(refreshRuntimeProfiles).toHaveBeenCalledTimes(2);
  });

  it('calls serve_model when start serving is clicked', async () => {
    const validateModelServingConfig = vi.fn<
      (_request: ServeModelRequest) => Promise<ModelServeValidationResponse>
    >().mockResolvedValue({
      success: true,
      valid: true,
      errors: [],
      warnings: [],
    });
    const serveModel = vi.fn<(_request: ServeModelRequest) => Promise<ServeModelResponse>>()
      .mockResolvedValue({
      success: true,
      loaded: true,
      loaded_models_unchanged: false,
      status: {
        model_id: 'model-5',
        model_alias: 'model-five',
        provider: 'llama_cpp',
        profile_id: 'emily-llama',
        load_state: 'loaded',
        device_mode: 'gpu',
        keep_loaded: true,
      },
      load_error: null,
      snapshot: null,
    });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: vi.fn().mockResolvedValue({
        success: true,
        snapshot: {
          cursor: 'serving:0',
          endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
          served_models: [],
          recent_errors: [],
        },
      }),
      validate_model_serving_config: validateModelServingConfig,
      serve_model: serveModel,
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-5',
          name: 'Model Five',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));

    await waitFor(() => expect(serveModel).toHaveBeenCalledTimes(1));
    const request = validateModelServingConfig.mock.calls[0]?.[0];
    expect(request).toMatchObject({
      model_id: 'model-5',
    });
    expect(request?.config.provider).toBe('llama_cpp');
    expect(request?.config.profile_id).toBe('emily-llama');
    expect(request?.config.device_mode).toBe('gpu');
    expect(request?.config.gpu_layers).toBe(32);
    expect(request?.config.context_size).toBe(4096);
    expect(request?.config.keep_loaded).toBe(true);
    expect(screen.getByRole('button', { name: 'Starting...' })).toBeDisabled();
  });

  it('passes context size for router profile serving', async () => {
    const validateModelServingConfig = vi.fn<
      (_request: ServeModelRequest) => Promise<ModelServeValidationResponse>
    >().mockResolvedValue({
      success: true,
      valid: true,
      errors: [],
      warnings: [],
    });
    const serveModel = vi.fn<(_request: ServeModelRequest) => Promise<ServeModelResponse>>()
      .mockResolvedValue({
        success: true,
        loaded: true,
        loaded_models_unchanged: false,
        status: null,
        load_error: null,
        snapshot: null,
      });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: vi.fn().mockResolvedValue({
        success: true,
        snapshot: {
          cursor: 'serving:0',
          endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
          served_models: [],
          recent_errors: [],
        },
      }),
      validate_model_serving_config: validateModelServingConfig,
      serve_model: serveModel,
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-router-serve',
          name: 'Router Serve Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="router-llama"
        onClose={vi.fn()}
      />
    );

    fireEvent.change(screen.getByRole('spinbutton', { name: /context/i }), {
      target: { value: '8192' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));

    await waitFor(() => expect(serveModel).toHaveBeenCalledTimes(1));
    expect(validateModelServingConfig.mock.calls[0]?.[0].config).toMatchObject({
      provider: 'llama_cpp',
      profile_id: 'router-llama',
      device_mode: 'gpu',
      context_size: 8192,
    });
    expect(validateModelServingConfig.mock.calls[0]?.[0].config.gpu_layers).toBeNull();
    expect(validateModelServingConfig.mock.calls[0]?.[0].config.tensor_split).toBeNull();
  });

  it('preserves an edited router context after the serve refresh returns the same profile', async () => {
    const refreshRuntimeProfiles = vi.fn();
    useRuntimeProfilesMock.mockImplementation(() => {
      const [profiles, setProfiles] = useState(snapshot.profiles);
      refreshRuntimeProfiles.mockImplementation(async () => {
        setProfiles((current) => current.map((profile) => ({ ...profile })));
      });
      return {
        snapshot: { ...snapshot, profiles },
        profiles,
        routes: snapshot.routes,
        statuses: snapshot.statuses,
        defaultProfileId: snapshot.default_profile_id,
        cursor: snapshot.cursor,
        isLoading: false,
        error: null,
        refreshRuntimeProfiles,
      };
    });
    const validateModelServingConfig = vi.fn<
      (_request: ServeModelRequest) => Promise<ModelServeValidationResponse>
    >().mockResolvedValue({ success: true, valid: true, errors: [], warnings: [] });
    const serveModel = vi.fn<(_request: ServeModelRequest) => Promise<ServeModelResponse>>()
      .mockResolvedValue({
        success: true,
        loaded: true,
        loaded_models_unchanged: false,
        status: null,
        load_error: null,
        snapshot: null,
      });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: vi.fn().mockResolvedValue({
        success: true,
        snapshot: {
          cursor: 'serving:0',
          endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
          served_models: [],
          recent_errors: [],
        },
      }),
      validate_model_serving_config: validateModelServingConfig,
      serve_model: serveModel,
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-router-context-refresh',
          name: 'Router Context Refresh',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="router-llama"
        onClose={vi.fn()}
      />
    );

    const contextInput = screen.getByRole('spinbutton', { name: /context/i });
    fireEvent.change(contextInput, { target: { value: '18000' } });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Keep loaded' }));
    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));

    await waitFor(() => expect(refreshRuntimeProfiles).toHaveBeenCalledTimes(1));
    expect(validateModelServingConfig.mock.calls[0]?.[0].config.context_size).toBe(18000);
    expect(contextInput).toHaveValue(18000);
    expect(screen.getByRole('checkbox', { name: 'Keep loaded' })).not.toBeChecked();
  });

  it('preserves dedicated-profile placement edits across a same-target snapshot clone', () => {
    const model = {
      id: 'model-dedicated-refresh',
      name: 'Dedicated Refresh',
      category: 'local' as const,
      primaryFormat: 'gguf' as const,
    };
    const props = {
      model,
      initialProfileId: 'emily-llama',
      onClose: vi.fn(),
    };
    const { rerender } = render(<ModelServeDialog {...props} />);

    fireEvent.change(screen.getByRole('combobox', { name: 'Model device' }), {
      target: { value: 'hybrid' },
    });
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Model GPU layers' }), {
      target: { value: '47' },
    });
    fireEvent.change(screen.getByRole('textbox', { name: 'Model tensor split' }), {
      target: { value: '3,1' },
    });
    fireEvent.change(screen.getByRole('spinbutton', { name: /context/i }), {
      target: { value: '12288' },
    });

    const clonedProfiles = snapshot.profiles.map((profile) => ({
      ...profile,
      device: { ...profile.device },
    }));
    useRuntimeProfilesMock.mockReturnValue({
      snapshot: { ...snapshot, profiles: clonedProfiles },
      profiles: clonedProfiles,
      routes: snapshot.routes,
      statuses: snapshot.statuses,
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });
    rerender(<ModelServeDialog {...props} />);

    expect(screen.getByRole('combobox', { name: 'Model device' })).toHaveValue('hybrid');
    expect(screen.getByRole('spinbutton', { name: 'Model GPU layers' })).toHaveValue(47);
    expect(screen.getByRole('textbox', { name: 'Model tensor split' })).toHaveValue('3,1');
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(12288);
  });

  it('initializes a fresh draft when the semantic model target changes', async () => {
    const firstModel = {
      id: 'model-router-target-one',
      name: 'Router Target One',
      category: 'local' as const,
      primaryFormat: 'gguf' as const,
    };
    const { rerender } = render(
      <ModelServeDialog
        model={firstModel}
        initialProfileId="router-llama"
        onClose={vi.fn()}
      />
    );

    fireEvent.change(screen.getByRole('spinbutton', { name: /context/i }), {
      target: { value: '8192' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Keep loaded' }));
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(8192);
    expect(screen.getByRole('checkbox', { name: 'Keep loaded' })).not.toBeChecked();

    rerender(
      <ModelServeDialog
        model={{ ...firstModel, id: 'model-router-target-two', name: 'Router Target Two' }}
        initialProfileId="router-llama"
        onClose={vi.fn()}
      />
    );

    await waitFor(() =>
      expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096)
    );
    expect(screen.getByRole('checkbox', { name: 'Keep loaded' })).toBeChecked();

    fireEvent.change(screen.getByRole('spinbutton', { name: /context/i }), {
      target: { value: '16384' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Keep loaded' }));
    fireEvent.change(screen.getByRole('combobox', { name: 'Runtime target' }), {
      target: { value: 'emily-llama' },
    });

    await waitFor(() =>
      expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096)
    );
    expect(screen.getByRole('checkbox', { name: 'Keep loaded' })).toBeChecked();
  });

  it('requires a unique alias when the same model is served on another profile', async () => {
    const servedModels: ServedModelStatus[] = [
      {
        model_id: 'model-duplicate',
        model_alias: 'duplicate-cpu',
        provider: 'llama_cpp',
        profile_id: 'cpu-llama',
        load_state: 'loaded',
        device_mode: 'cpu',
        keep_loaded: true,
      },
    ];
    const validateModelServingConfig = vi.fn<
      (_request: ServeModelRequest) => Promise<ModelServeValidationResponse>
    >().mockResolvedValue({
      success: true,
      valid: true,
      errors: [],
      warnings: [],
    });
    const serveModel = vi.fn<(_request: ServeModelRequest) => Promise<ServeModelResponse>>()
      .mockResolvedValue({
        success: true,
        loaded: true,
        loaded_models_unchanged: false,
        status: null,
        load_error: null,
        snapshot: null,
      });
    getElectronAPIMock.mockReturnValue({
      validate_model_serving_config: validateModelServingConfig,
      serve_model: serveModel,
    });
    useServingStatusMock.mockReturnValue({
      snapshot: null,
      servedModels,
      endpoint: { endpoint_mode: 'pumas_gateway', model_count: 1 },
      cursor: 'serving:1',
      error: null,
      controlObservation: { kind: 'known', rows: servedModels },
      refreshServingStatus: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-duplicate',
          name: 'Duplicate Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(
      await screen.findByText(
        'This model is already served on another profile. Use a unique alias for this instance.'
      )
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));
    expect(serveModel).not.toHaveBeenCalled();

    fireEvent.change(screen.getByRole('textbox', { name: /gateway alias/i }), {
      target: { value: 'duplicate-gpu' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));

    await waitFor(() => expect(serveModel).toHaveBeenCalledTimes(1));
    expect(validateModelServingConfig.mock.calls[0]?.[0].config.model_alias).toBe('duplicate-gpu');
  });

  it('keeps start serving actionable while profile refresh is loading', async () => {
    useRuntimeProfilesMock.mockReturnValue({
      snapshot,
      profiles: snapshot.profiles,
      routes: snapshot.routes,
      statuses: snapshot.statuses,
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: true,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });
    const validateModelServingConfig = vi.fn<
      (_request: ServeModelRequest) => Promise<ModelServeValidationResponse>
    >().mockResolvedValue({
      success: true,
      valid: true,
      errors: [],
      warnings: [],
    });
    const serveModel = vi.fn<(_request: ServeModelRequest) => Promise<ServeModelResponse>>()
      .mockResolvedValue({
      success: true,
      loaded: true,
      loaded_models_unchanged: false,
      status: null,
      load_error: null,
      snapshot: null,
    });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: vi.fn().mockResolvedValue({
        success: true,
        snapshot: {
          cursor: 'serving:0',
          endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
          served_models: [],
          recent_errors: [],
        },
      }),
      validate_model_serving_config: validateModelServingConfig,
      serve_model: serveModel,
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-loading',
          name: 'Model Loading',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    const startButton = screen.getByRole('button', { name: 'Start serving' });
    expect(startButton).toBeEnabled();
    fireEvent.click(startButton);

    await waitFor(() => expect(serveModel).toHaveBeenCalledTimes(1));
  });

  it('shows a disabled Loading control when a fresh dialog observes the model loading', async () => {
    const loadingStatus: ServedModelStatus = {
      model_id: 'model-known-loading',
      model_alias: 'model-known-loading',
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loading',
      device_mode: 'gpu',
      keep_loaded: true,
      context_size: 20000,
    };
    useServingStatusMock.mockReturnValue({
      snapshot: null,
      servedModels: [loadingStatus],
      endpoint: null,
      cursor: 'serving:loading',
      error: null,
      controlObservation: { kind: 'known', rows: [loadingStatus] },
      refreshServingStatus: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-known-loading',
          name: 'Known Loading Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(await screen.findByRole('button', { name: 'Loading' })).toBeDisabled();
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(20000);
    expect(screen.queryByRole('button', { name: 'Start serving' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Stop serving' })).not.toBeInTheDocument();
  });

  it('hydrates a delayed active request context without overwriting a same-target edit', async () => {
    let controlRows: Array<{
      model_id: string;
      model_alias: string;
      provider: 'llama_cpp';
      profile_id: string;
      load_state: 'loading';
      context_size: number;
    }> = [];
    useServingStatusMock.mockImplementation(() => ({
      snapshot: null,
      servedModels: controlRows.map((row) => ({
        ...row,
        device_mode: 'gpu' as const,
        keep_loaded: true,
      })),
      endpoint: null,
      cursor: 'serving:delayed',
      error: null,
      controlObservation: { kind: 'known' as const, rows: controlRows },
      refreshServingStatus: vi.fn(),
    }));
    const model = {
      id: 'model-delayed-context',
      name: 'Delayed Context Model',
      category: 'local' as const,
      primaryFormat: 'gguf' as const,
    };
    const props = { model, initialProfileId: 'emily-llama', onClose: vi.fn() };
    const { rerender } = render(<ModelServeDialog {...props} />);
    await waitFor(() =>
      expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096)
    );

    controlRows = [{
      model_id: model.id,
      model_alias: model.id,
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loading',
      context_size: 20000,
    }];
    rerender(<ModelServeDialog {...props} />);
    await waitFor(() =>
      expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(20000)
    );

    fireEvent.change(screen.getByRole('spinbutton', { name: /context/i }), {
      target: { value: '18000' },
    });
    const [currentRow] = controlRows;
    expect(currentRow).toBeDefined();
    if (!currentRow) return;
    controlRows = [{ ...currentRow, context_size: 24000 }];
    rerender(<ModelServeDialog {...props} />);
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(18000);
  });

  it('preserves a context edit made before the first exact active row arrives', async () => {
    let controlRows: Array<{
      model_id: string;
      provider: 'llama_cpp';
      profile_id: string;
      load_state: 'loading';
      context_size: number;
    }> = [];
    useServingStatusMock.mockImplementation(() => ({
      snapshot: null,
      servedModels: [],
      endpoint: null,
      cursor: 'serving:late-context',
      error: null,
      controlObservation: { kind: 'known' as const, rows: controlRows },
      refreshServingStatus: vi.fn(),
    }));
    const model = {
      id: 'model-edited-before-context',
      name: 'Edited Before Context Model',
      category: 'local' as const,
      primaryFormat: 'gguf' as const,
    };
    const props = { model, initialProfileId: 'emily-llama', onClose: vi.fn() };
    const { rerender } = render(<ModelServeDialog {...props} />);
    const contextInput = await screen.findByRole('spinbutton', { name: /context/i });
    fireEvent.change(contextInput, { target: { value: '18000' } });

    controlRows = [{
      model_id: model.id,
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loading',
      context_size: 20000,
    }];
    rerender(<ModelServeDialog {...props} />);

    expect(contextInput).toHaveValue(18000);
  });

  it('hydrates an already-loaded target from the requested context', async () => {
    const loadedStatus: ServedModelStatus = {
      model_id: 'model-loaded-context',
      model_alias: 'model-loaded-context',
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loaded',
      device_mode: 'gpu',
      context_size: 20000,
      keep_loaded: true,
    };
    useServingStatusMock.mockReturnValue({
      snapshot: null,
      servedModels: [loadedStatus],
      endpoint: null,
      cursor: 'serving:loaded',
      error: null,
      controlObservation: { kind: 'known', rows: [loadedStatus] },
      refreshServingStatus: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: loadedStatus.model_id,
          name: 'Loaded Context Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(await screen.findByRole('button', { name: 'Stop serving' })).toBeEnabled();
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(20000);
  });

  it('excludes the previous target row after the model changes', async () => {
    const firstModel = {
      id: 'model-old-context',
      name: 'Old Context Model',
      category: 'local' as const,
      primaryFormat: 'gguf' as const,
    };
    const oldRow: ServedModelStatus = {
      model_id: firstModel.id,
      model_alias: firstModel.id,
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loading',
      device_mode: 'gpu',
      context_size: 20000,
      keep_loaded: true,
    };
    useServingStatusMock.mockReturnValue({
      snapshot: null,
      servedModels: [oldRow],
      endpoint: null,
      cursor: 'serving:old-target',
      error: null,
      controlObservation: { kind: 'known', rows: [oldRow] },
      refreshServingStatus: vi.fn(),
    });
    const { rerender } = render(
      <ModelServeDialog model={firstModel} initialProfileId="emily-llama" onClose={vi.fn()} />
    );
    await waitFor(() =>
      expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(20000)
    );

    rerender(
      <ModelServeDialog
        model={{ ...firstModel, id: 'model-new-context', name: 'New Context Model' }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );
    await waitFor(() =>
      expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096)
    );
  });

  it('does not hydrate context from raw status when the control observation is malformed', async () => {
    const rawStatus: ServedModelStatus = {
      model_id: 'model-malformed-context',
      model_alias: 'model-malformed-context',
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loading',
      device_mode: 'gpu',
      context_size: 20000,
      keep_loaded: true,
    };
    useServingStatusMock.mockReturnValue({
      snapshot: null,
      servedModels: [rawStatus],
      endpoint: null,
      cursor: 'serving:malformed-context',
      error: 'Serving status response was malformed',
      controlObservation: {
        kind: 'unavailable',
        message: 'Serving status response was malformed',
      },
      refreshServingStatus: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: rawStatus.model_id,
          name: 'Malformed Context Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    await waitFor(() =>
      expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096)
    );
  });

  it('keeps Start disabled when a fresh dialog observes an interrupted serving outcome', async () => {
    const interruptedStatus: ServedModelStatus = {
      model_id: 'model-interrupted',
      model_alias: 'model-interrupted',
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'failed',
      device_mode: 'gpu',
      keep_loaded: true,
      last_error: {
        code: 'unknown',
        severity: 'critical',
        message: 'Serving outcome unavailable',
      },
    };
    useServingStatusMock.mockReturnValue({
      snapshot: null,
      servedModels: [interruptedStatus],
      endpoint: null,
      cursor: 'serving:interrupted',
      error: null,
      controlObservation: {
        kind: 'known',
        rows: [{ ...interruptedStatus, last_error: { code: 'unknown' } }],
      },
      refreshServingStatus: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-interrupted',
          name: 'Interrupted Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(
      await screen.findByRole('button', { name: 'Serving status unavailable' })
    ).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Start serving' })).not.toBeInTheDocument();
  });

  it.each([
    ['provider_load_failed', 'Start serving'],
    ['unknown', 'Serving status unavailable'],
  ] as const)(
    'projects a fresh observed Loading to Failed(%s) sequence as %s',
    async (errorCode, expectedLabel) => {
      let status: ServedModelStatus = {
        model_id: 'model-loading-terminal',
        model_alias: 'model-loading-terminal',
        provider: 'llama_cpp',
        profile_id: 'emily-llama',
        load_state: 'loading',
        device_mode: 'gpu',
        keep_loaded: true,
      };
      useServingStatusMock.mockImplementation(() => ({
        snapshot: null,
        servedModels: [status],
        endpoint: null,
        cursor: `serving:${status.load_state}`,
        error: null,
        controlObservation: {
          kind: 'known',
          rows: [{
            model_id: status.model_id,
            model_alias: status.model_alias,
            provider: status.provider,
            profile_id: status.profile_id,
            load_state: status.load_state,
            ...(status.last_error ? { last_error: { code: status.last_error.code } } : {}),
          }],
        },
        refreshServingStatus: vi.fn(),
      }));

      const model = {
        id: 'model-loading-terminal',
        name: 'Loading Terminal Model',
        category: 'local' as const,
        primaryFormat: 'gguf' as const,
      };
      const { rerender } = render(
        <ModelServeDialog model={model} initialProfileId="emily-llama" onClose={vi.fn()} />
      );
      expect(await screen.findByRole('button', { name: 'Loading' })).toBeDisabled();

      status = {
        ...status,
        load_state: 'failed',
        last_error: {
          code: errorCode,
          severity: errorCode === 'unknown' ? 'critical' : 'non_critical',
          message: 'terminal load result',
        },
      };
      rerender(
        <ModelServeDialog model={model} initialProfileId="emily-llama" onClose={vi.fn()} />
      );

      const terminalButton = await screen.findByRole('button', { name: expectedLabel });
      if (errorCode === 'unknown') {
        expect(terminalButton).toBeDisabled();
      } else {
        expect(terminalButton).toBeEnabled();
      }
    }
  );

  it('switches the single serving control to Stop when status reports the model loaded during a pending start', async () => {
    let rejectServe: (reason: Error) => void = () => undefined;
    const serveModel = vi.fn<(_request: ServeModelRequest) => Promise<ServeModelResponse>>()
      .mockImplementation(
        () => new Promise((_resolve, reject) => {
          rejectServe = reject;
        })
      );
    let servedModels: ServedModelStatus[] = [];
    useServingStatusMock.mockImplementation(() => ({
      snapshot: null,
      servedModels,
      endpoint: null,
      cursor: null,
      error: null,
      controlObservation: { kind: 'known', rows: servedModels },
      refreshServingStatus: vi.fn(),
    }));
    getElectronAPIMock.mockReturnValue({
      validate_model_serving_config: vi.fn().mockResolvedValue({
        success: true,
        valid: true,
        errors: [],
        warnings: [],
      }),
      serve_model: serveModel,
      unserve_model: vi.fn(),
    });

    const { rerender } = render(
      <ModelServeDialog
        model={{
          id: 'model-pending-loaded',
          name: 'Pending Loaded Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));
    expect(await screen.findByRole('button', { name: 'Starting...' })).toBeDisabled();
    await waitFor(() => expect(serveModel).toHaveBeenCalledTimes(1));

    servedModels = [{
      model_id: 'model-pending-loaded',
      model_alias: 'model-pending-loaded',
      provider: 'llama_cpp',
      profile_id: 'emily-llama',
      load_state: 'loaded',
      device_mode: 'gpu',
      keep_loaded: true,
    }];
    rerender(
      <ModelServeDialog
        model={{
          id: 'model-pending-loaded',
          name: 'Pending Loaded Model',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    const stopButton = await screen.findByRole('button', { name: 'Stop serving' });
    expect(stopButton).toBeEnabled();
    expect(screen.queryByRole('button', { name: 'Start serving' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Starting...' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Unload' })).not.toBeInTheDocument();

    rejectServe(new Error('Serving request timed out'));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Stop serving' })).toBeEnabled());
    expect(screen.queryByRole('button', { name: 'Start serving' })).not.toBeInTheDocument();
  });

  it('shows feedback when the serving API is unavailable', async () => {
    getElectronAPIMock.mockReturnValue(null);

    render(
      <ModelServeDialog
        model={{
          id: 'model-6',
          name: 'Model Six',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Start serving' }));

    expect(
      await screen.findByText('Serving API is not available in this app session.')
    ).toBeInTheDocument();
  });
});
