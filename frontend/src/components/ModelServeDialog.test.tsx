import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { RuntimeProfilesSnapshot } from '../types/api-runtime-profiles';
import type {
  ModelServeValidationResponse,
  ServeModelRequest,
  ServeModelResponse,
} from '../types/api-serving';
import { ModelServeDialog } from './ModelServeDialog';

const { getElectronAPIMock, useRuntimeProfilesMock } = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  useRuntimeProfilesMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

vi.mock('../hooks/useRuntimeProfiles', () => ({
  useRuntimeProfiles: useRuntimeProfilesMock,
}));

describe('ModelServeDialog', () => {
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
      screen.getByText('Cannot serve yet: Only GGUF models can be served locally in this flow.')
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

  it('uses router profile placement without per-model overrides', () => {
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
    expect(screen.queryByRole('spinbutton', { name: /context/i })).not.toBeInTheDocument();
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
    expect(screen.getByText('Loaded')).toBeInTheDocument();
  });

  it('requires a unique alias when the same model is served on another profile', async () => {
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
          cursor: 'serving:1',
          endpoint: { endpoint_mode: 'pumas_gateway', model_count: 1 },
          served_models: [
            {
              model_id: 'model-duplicate',
              model_alias: 'duplicate-cpu',
              provider: 'llama_cpp',
              profile_id: 'cpu-llama',
              load_state: 'loaded',
              device_mode: 'cpu',
              keep_loaded: true,
            },
          ],
          last_errors: [],
        },
      }),
      validate_model_serving_config: validateModelServingConfig,
      serve_model: serveModel,
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
