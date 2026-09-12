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

  it('does not treat a loaded row from another provider as the selected target', async () => {
    const status = servedModels()[0];
    expect(status).toBeDefined();
    if (!status) return;
    const { result } = renderHook(() =>
      useModelServingActions(
        'models/chat',
        { profileId: 'llama-cpu', provider: 'ollama' },
        [status]
      )
    );

    await waitFor(() => expect(result.current.servedStatus).toBeNull());
  });

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
    'keeps a late start failure from replacing Loaded before a subsequent %s snapshot',
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
      expect(result.current.message).toBe('Loaded on llama-gpu');

      rerender({
        statuses: nextSnapshot === 'empty' ? [] : [{ ...loaded, load_state: 'failed' }],
      });
      await waitFor(() => expect(result.current.servedStatus).toBeNull());
      expect(result.current.message).toBeNull();
    }
  );

  it('keeps a pending stop pending across a refreshed loaded observation', async () => {
    const loaded = servedModels()[0];
    expect(loaded).toBeDefined();
    if (!loaded) return;
    let finishStop: (value: { success: true; unloaded: true }) => void = () => undefined;
    unserveModelMock.mockImplementation(
      () => new Promise((resolve) => {
        finishStop = resolve;
      })
    );
    const { result, rerender } = renderHook(
      ({ statuses }: { statuses: ServedModelStatus[] }) =>
        useModelServingActions('models/chat', { profileId: 'llama-cpu' }, statuses),
      { initialProps: { statuses: [loaded] } }
    );
    await waitFor(() => expect(result.current.servedStatus).not.toBeNull());

    let pendingStop: Promise<void> | undefined;
    act(() => {
      pendingStop = result.current.unloadModel();
    });
    expect(result.current.actionPhase).toBe('stopping');
    rerender({ statuses: [{ ...loaded }] });
    expect(result.current.actionPhase).toBe('stopping');

    await act(async () => {
      finishStop({ success: true, unloaded: true });
      await pendingStop;
    });
  });

  it('does not restore loaded from a late start acknowledgement after a failed observation', async () => {
    const loaded = servedModels()[0];
    expect(loaded).toBeDefined();
    if (!loaded) return;
    let finishServe: (value: ServeModelResponse) => void = () => undefined;
    getElectronAPIMock.mockReturnValue({
      unserve_model: unserveModelMock,
      validate_model_serving_config: vi.fn().mockResolvedValue({
        success: true,
        valid: true,
        errors: [],
        warnings: [],
      }),
      serve_model: vi.fn(() => new Promise((resolve) => {
        finishServe = resolve;
      })),
    });
    const { result, rerender } = renderHook(
      ({ statuses }: { statuses: ServedModelStatus[] }) =>
        useModelServingActions('models/chat', { profileId: 'llama-cpu' }, statuses),
      { initialProps: { statuses: [] as ServedModelStatus[] } }
    );
    let pendingStart: Promise<void> | undefined;
    await act(async () => {
      pendingStart = result.current.serveModel({
        provider: 'llama_cpp',
        profile_id: 'llama-cpu',
        device_mode: 'cpu',
        keep_loaded: true,
      });
      await Promise.resolve();
    });
    rerender({ statuses: [{ ...loaded, load_state: 'failed' }] });
    await act(async () => {
      finishServe({
        success: true,
        loaded: true,
        loaded_models_unchanged: false,
        status: loaded,
      });
      await pendingStart;
    });
    expect(result.current.servedStatus).toBeNull();
  });

  it.each(['empty', 'failed'] as const)(
    'clears a successful Loaded notice when a subsequent authoritative snapshot is %s',
    async (nextSnapshot) => {
      const loaded = servedModels().find((status) => status.profile_id === 'llama-gpu');
      expect(loaded).toBeDefined();
      if (!loaded) {
        return;
      }
      const { result, rerender } = renderHook(
        ({ statuses }: { statuses: ServedModelStatus[] }) =>
          useModelServingActions('models/chat', { profileId: 'llama-gpu' }, statuses),
        { initialProps: { statuses: [loaded] } }
      );

      await waitFor(() => expect(result.current.message).toBe('Loaded on llama-gpu'));

      rerender({
        statuses: nextSnapshot === 'empty' ? [] : [{ ...loaded, load_state: 'failed' }],
      });
      await waitFor(() => expect(result.current.servedStatus).toBeNull());
      expect(result.current.message).toBeNull();
    }
  );

  it.each(['acknowledged', 'timed out'] as const)(
    'settles a %s stop when its target disappears while another target remains',
    async (outcome) => {
      const statuses = servedModels();
      if (outcome === 'timed out') {
        unserveModelMock.mockRejectedValueOnce(new Error('timeout'));
      }
      const { result, rerender } = renderHook(
        ({ rows }) => useModelServingActions('models/chat', { profileId: 'llama-cpu' }, rows),
        { initialProps: { rows: statuses } }
      );
      await act(async () => { await result.current.unloadModel(); });
      expect(result.current.actionPhase).toBe(outcome === 'timed out' ? 'uncertain' : 'stopping');
      rerender({ rows: statuses.filter((row) => row.profile_id !== 'llama-cpu') });
      expect(result.current.actionPhase).toBe('idle');
      expect(result.current.servedStatus).toBeNull();
      expect(unserveModelMock).toHaveBeenCalledTimes(1);
    }
  );

  it.each(['timeout', 'malformed acknowledgement'] as const)(
    'keeps a start uncertain after %s and a fresh absent observation',
    async (outcome) => {
      const serveModel = outcome === 'timeout'
        ? vi.fn().mockRejectedValue(new Error('timeout'))
        : vi.fn().mockResolvedValue({ success: true, loaded: 'false' });
      getElectronAPIMock.mockReturnValue({
        validate_model_serving_config: vi.fn().mockResolvedValue({ success: true, valid: true, errors: [] }),
        serve_model: serveModel,
      });
      const { result, rerender } = renderHook(
        ({ rows }) => useModelServingActions('models/chat', { profileId: 'llama-cpu' }, rows),
        { initialProps: { rows: [] as ServedModelStatus[] } }
      );
      await act(async () => {
        await result.current.serveModel({
          profile_id: 'llama-cpu', provider: 'llama_cpp', device_mode: 'cpu', keep_loaded: true,
        });
      });
      expect(result.current.actionPhase).toBe('uncertain');
      rerender({ rows: [] });
      expect(result.current.actionPhase).toBe('uncertain');
      expect(serveModel).toHaveBeenCalledTimes(1);
    }
  );


  it.each(['requested', 'loading', 'unloading'] as const)(
    'keeps an observed %s operation unavailable for another start',
    (loadState) => {
      const rows = servedModels().map((row) => ({ ...row, load_state: loadState }));
      const { result } = renderHook(() =>
        useModelServingActions('models/chat', { profileId: 'llama-cpu' }, rows)
      );
      expect(result.current.actionPhase).toBe('uncertain');
      expect(result.current.servedStatus).toBeNull();
    }
  );

  it('keeps an exact interrupted target unavailable without affecting another target', () => {
    const interrupted: ServedModelStatus = {
      model_id: 'models/chat',
      model_alias: 'chat-cpu',
      provider: 'llama_cpp',
      profile_id: 'llama-cpu',
      load_state: 'failed',
      device_mode: 'cpu',
      keep_loaded: true,
      last_error: {
        code: 'unknown',
        severity: 'critical',
        message: 'Serving outcome unavailable',
      },
    };
    const exact = renderHook(() =>
      useModelServingActions('models/chat', { profileId: 'llama-cpu' }, [interrupted])
    );
    const unrelated = renderHook(() =>
      useModelServingActions('models/chat', { profileId: 'llama-gpu' }, [interrupted])
    );

    expect(exact.result.current.isUnavailable).toBe(true);
    expect(exact.result.current.actionPhase).toBe('uncertain');
    expect(unrelated.result.current.isUnavailable).toBe(false);
    expect(unrelated.result.current.actionPhase).toBe('idle');
  });

  it.each(['model', 'profile', 'provider', 'mode', 'alias', 'unmount'] as const)(
    'discards a late start completion after %s supersession',
    async (change) => {
      let finishServe: (value: unknown) => void = () => undefined;
      getElectronAPIMock.mockReturnValue({
        validate_model_serving_config: vi.fn().mockResolvedValue({ success: true, valid: true, errors: [] }),
        serve_model: vi.fn(() => new Promise((resolve) => { finishServe = resolve; })),
      });
      const initialProps = {
        modelId: 'models/chat', profileId: 'llama-cpu', provider: 'llama_cpp',
        providerMode: 'dedicated', modelAlias: 'chat-cpu',
      };
      const { result, rerender, unmount } = renderHook(
        ({ modelId, ...target }) => useModelServingActions(modelId, target),
        { initialProps }
      );
      let pending: Promise<void> | undefined;
      await act(async () => {
        pending = result.current.serveModel({
          profile_id: 'llama-cpu', provider: 'llama_cpp', device_mode: 'cpu', keep_loaded: true,
        });
        await Promise.resolve();
      });
      if (change === 'unmount') unmount();
      else rerender({
        ...initialProps,
        ...(change === 'model' ? { modelId: 'models/other' } : {}),
        ...(change === 'profile' ? { profileId: 'other-profile' } : {}),
        ...(change === 'provider' ? { provider: 'ollama' } : {}),
        ...(change === 'mode' ? { providerMode: 'router' } : {}),
        ...(change === 'alias' ? { modelAlias: 'other-alias' } : {}),
      });
      const beforeCompletion = result.current;
      await act(async () => {
        finishServe({ success: false, error: 'late failure' });
        await pending;
      });
      expect(result.current).toBe(beforeCompletion);
      expect(result.current.message).not.toBe('late failure');
    }
  );

});
