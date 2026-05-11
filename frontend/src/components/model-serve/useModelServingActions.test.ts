import { renderHook, act } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ServingStatusSnapshot } from '../../types/api-serving';
import { useModelServingActions } from './useModelServingActions';

const {
  getElectronAPIMock,
  getServingStatusMock,
  unserveModelMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  getServingStatusMock: vi.fn(),
  unserveModelMock: vi.fn(),
}));

vi.mock('../../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

function snapshot(): ServingStatusSnapshot {
  return {
    schema_version: 1,
    cursor: 'serving:1',
    endpoint: {
      endpoint_mode: 'pumas_gateway',
      model_count: 2,
    },
    served_models: [
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
    ],
    last_errors: [],
  };
}

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useModelServingActions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getServingStatusMock.mockResolvedValue({
      success: true,
      snapshot: snapshot(),
    });
    unserveModelMock.mockResolvedValue({
      success: true,
      unloaded: true,
    });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: getServingStatusMock,
      unserve_model: unserveModelMock,
    });
  });

  it('targets the served instance for the selected profile when unloading', async () => {
    const { result } = renderHook(() =>
      useModelServingActions('models/chat', { profileId: 'llama-gpu' })
    );

    await flushMicrotasks();
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
});
