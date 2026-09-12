import { renderHook, act, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ModelServeValidationResponse,
  ServeModelRequest,
  ServeModelResponse,
  ServedModelStatus,
} from '../../types/api-serving';
import { useModelServingActions } from './useModelServingActions';

const {
  getElectronAPIMock,
  unserveModelMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  unserveModelMock: vi.fn(),
}));

vi.mock('../../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

function servedModels(): ServedModelStatus[] {
  return [
    {
      model_id: 'models/chat',
      model_alias: 'chat-cpu',
      provider: 'llama_cpp',
      profile_id: 'llama-cpu',
      load_state: 'loaded',
      device_mode: 'cpu',
      keep_loaded: true,
    },
    {
      model_id: 'models/chat',
      model_alias: 'chat-gpu',
      provider: 'llama_cpp',
      profile_id: 'llama-gpu',
      load_state: 'loaded',
      device_mode: 'gpu',
      keep_loaded: true,
    },
  ];
}

describe('useModelServingActions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    unserveModelMock.mockResolvedValue({
      success: true,
      unloaded: true,
    });
    getElectronAPIMock.mockReturnValue({
      unserve_model: unserveModelMock,
    });
  });

  it('targets the served instance for the selected profile when unloading', async () => {
    const statuses = servedModels();
    const { result } = renderHook(() =>
      useModelServingActions('models/chat', { profileId: 'llama-gpu' }, statuses)
    );

    await waitFor(() => {
      expect(result.current.servedStatus?.profile_id).toBe('llama-gpu');
    });
    await act(async () => {
      await result.current.unloadModel();
    });

    expect(unserveModelMock).toHaveBeenCalledWith({
      model_id: 'models/chat',
      provider: 'llama_cpp',
      profile_id: 'llama-gpu',
      model_alias: 'chat-gpu',
    });
  });

  it.each(['loading', 'failed'] as const)(
    'does not project a matching %s status row as Loaded',
    async (loadState) => {
      const statuses: ServedModelStatus[] = [
        {
          model_id: 'models/huihui-27b',
          model_alias: 'huihui-27b',
          provider: 'llama_cpp',
          profile_id: 'eidetic-gpu',
          load_state: loadState,
          device_mode: 'gpu',
          context_size: 18000,
          keep_loaded: true,
        },
      ];
      const { result } = renderHook(() =>
        useModelServingActions(
          'models/huihui-27b',
          { profileId: 'eidetic-gpu' },
          statuses
        )
      );

      await waitFor(() => expect(result.current.message).not.toBe('Loaded on eidetic-gpu'));
      expect(result.current.servedStatus).toBeNull();
    }
  );

  it('keeps an exact context-18000 load rejection out of Loaded state', async () => {
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
      loaded: false,
      loaded_models_unchanged: true,
      status: null,
      load_error: {
        code: 'provider_load_failed',
        message: 'exact router load rejection',
        severity: 'non_critical',
        provider: 'llama_cpp',
        model_id: 'models/huihui-27b',
        profile_id: 'eidetic-gpu',
      },
      snapshot: null,
    });
    getElectronAPIMock.mockReturnValue({
      unserve_model: unserveModelMock,
      validate_model_serving_config: validateModelServingConfig,
      serve_model: serveModel,
    });
    const { result } = renderHook(() =>
      useModelServingActions('models/huihui-27b', { profileId: 'eidetic-gpu' }, [])
    );

    await act(async () => {
      await result.current.serveModel({
        provider: 'llama_cpp',
        profile_id: 'eidetic-gpu',
        device_mode: 'gpu',
        device_id: null,
        gpu_layers: -1,
        tensor_split: null,
        context_size: 18000,
        keep_loaded: true,
        model_alias: null,
      });
    });

    expect(validateModelServingConfig.mock.calls[0]?.[0].config.context_size).toBe(18000);
    expect(serveModel).toHaveBeenCalledTimes(1);
    expect(result.current.servedStatus).toBeNull();
    expect(result.current.message).toBeNull();
    expect(result.current.serveError?.message).toBe('exact router load rejection');
  });

  it.each(['empty', 'failed'] as const)(
    'clears stale Loaded state for a subsequent %s snapshot without replacing a serving failure message',
    async (nextSnapshot) => {
      const loaded = servedModels().find((status) => status.profile_id === 'llama-gpu');
      expect(loaded).toBeDefined();
      if (!loaded) {
        return;
      }
      const validateModelServingConfig = vi.fn().mockResolvedValue({
        success: true,
        valid: true,
        errors: [],
        warnings: [],
      });
      const serveModel = vi.fn().mockRejectedValue(new Error('exact serving failure'));
      getElectronAPIMock.mockReturnValue({
        unserve_model: unserveModelMock,
        validate_model_serving_config: validateModelServingConfig,
        serve_model: serveModel,
      });
      const { result, rerender } = renderHook(
        ({ statuses }: { statuses: ServedModelStatus[] }) =>
          useModelServingActions('models/chat', { profileId: 'llama-gpu' }, statuses),
        { initialProps: { statuses: [loaded] } }
      );
      await waitFor(() => expect(result.current.message).toBe('Loaded on llama-gpu'));

      await act(async () => {
        await result.current.serveModel({
          provider: 'llama_cpp',
          profile_id: 'llama-gpu',
          device_mode: 'gpu',
          context_size: 18000,
          keep_loaded: true,
        });
      });
      expect(result.current.message).toBe('exact serving failure');

      rerender({
        statuses: nextSnapshot === 'empty' ? [] : [{ ...loaded, load_state: 'failed' }],
      });
      await waitFor(() => expect(result.current.servedStatus).toBeNull());
      expect(result.current.message).toBe('exact serving failure');
    }
  );

  it.each(['empty', 'failed'] as const)(
    'clears a successful Loaded notice when a subsequent authoritative snapshot is %s',
    async (nextSnapshot) => {
      const loaded = servedModels().find((status) => status.profile_id === 'llama-gpu');
      expect(loaded).toBeDefined();
      if (!loaded) {
        return;
      }
      const validateModelServingConfig = vi.fn<
        (_request: ServeModelRequest) => Promise<ModelServeValidationResponse>
      >().mockResolvedValue({ success: true, valid: true, errors: [], warnings: [] });
      const serveModel = vi.fn<(_request: ServeModelRequest) => Promise<ServeModelResponse>>()
        .mockResolvedValue({
          success: true,
          loaded: true,
          loaded_models_unchanged: false,
          status: loaded,
          load_error: null,
          snapshot: null,
        });
      getElectronAPIMock.mockReturnValue({
        unserve_model: unserveModelMock,
        validate_model_serving_config: validateModelServingConfig,
        serve_model: serveModel,
      });
      const { result, rerender } = renderHook(
        ({ statuses }: { statuses: ServedModelStatus[] }) =>
          useModelServingActions('models/chat', { profileId: 'llama-gpu' }, statuses),
        { initialProps: { statuses: [] as ServedModelStatus[] } }
      );

      await act(async () => {
        await result.current.serveModel({
          provider: 'llama_cpp',
          profile_id: 'llama-gpu',
          device_mode: 'gpu',
          context_size: 18000,
          keep_loaded: true,
        });
      });
      expect(result.current.message).toBe('Loaded');

      rerender({
        statuses: nextSnapshot === 'empty' ? [] : [{ ...loaded, load_state: 'failed' }],
      });
      await waitFor(() => expect(result.current.servedStatus).toBeNull());
      expect(result.current.message).toBeNull();
    }
  );
});
