import { renderHook, act, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ServedModelStatus } from '../../types/api-serving';
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
});
