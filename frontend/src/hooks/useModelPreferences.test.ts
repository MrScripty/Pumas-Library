import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BaseResponse, LinkExclusionsResponse } from '../types/api';
import { useModelPreferences } from './useModelPreferences';

const {
  getLinkExclusionsMock,
  isApiAvailableMock,
  setModelLinkExclusionMock,
} = vi.hoisted(() => ({
  getLinkExclusionsMock: vi.fn<(_appId: string) => Promise<LinkExclusionsResponse>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  setModelLinkExclusionMock: vi.fn<
    (_modelId: string, _appId: string, _excluded: boolean) => Promise<BaseResponse>
  >(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_link_exclusions: getLinkExclusionsMock,
    set_model_link_exclusion: setModelLinkExclusionMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

function linkExclusionsResponse(excludedModelIds: string[]): LinkExclusionsResponse {
  return {
    success: true,
    excluded_model_ids: excludedModelIds,
  };
}

describe('useModelPreferences', () => {
  beforeEach(() => {
    isApiAvailableMock.mockReturnValue(true);
    getLinkExclusionsMock.mockResolvedValue(linkExclusionsResponse([]));
    setModelLinkExclusionMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('loads backend-owned link exclusions for the selected app', async () => {
    getLinkExclusionsMock.mockResolvedValue(linkExclusionsResponse(['model-a']));
    const { result } = renderHook(() => useModelPreferences({ selectedAppId: 'torch' }));

    await waitFor(() => {
      expect(result.current.excludedModels.has('model-a')).toBe(true);
    });

    expect(getLinkExclusionsMock).toHaveBeenCalledWith('torch');
  });

  it('toggles local starred models without calling the backend', async () => {
    const { result } = renderHook(() => useModelPreferences({ selectedAppId: 'comfyui' }));

    await act(async () => {
      result.current.toggleStar('model-a');
    });

    expect(result.current.starredModels.has('model-a')).toBe(true);

    await act(async () => {
      result.current.toggleStar('model-a');
    });

    expect(result.current.starredModels.has('model-a')).toBe(false);
    expect(setModelLinkExclusionMock).not.toHaveBeenCalled();
  });

  it('persists optimistic link exclusion changes to the active app', async () => {
    const { result } = renderHook(() => useModelPreferences({ selectedAppId: 'ollama' }));

    await act(async () => {
      result.current.toggleLink('model-a');
    });

    expect(result.current.excludedModels.has('model-a')).toBe(true);
    expect(setModelLinkExclusionMock).toHaveBeenCalledWith('model-a', 'ollama', true);
  });

  it('rolls back optimistic link exclusion changes when persistence fails', async () => {
    let rejectPersistence: (_error: Error) => void = () => undefined;
    const persistence = new Promise<BaseResponse>((_resolve, reject) => {
      rejectPersistence = reject;
    });
    setModelLinkExclusionMock.mockReturnValue(persistence);
    const { result } = renderHook(() => useModelPreferences({ selectedAppId: 'comfyui' }));

    await act(async () => {
      result.current.toggleLink('model-a');
    });

    expect(result.current.excludedModels.has('model-a')).toBe(true);

    await act(async () => {
      rejectPersistence(new Error('write failed'));
      await persistence.catch(() => undefined);
    });

    await waitFor(() => {
      expect(result.current.excludedModels.has('model-a')).toBe(false);
    });
  });

  it('does not call link APIs when the bridge is unavailable', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const { result } = renderHook(() => useModelPreferences({ selectedAppId: null }));

    await act(async () => {
      result.current.toggleLink('model-a');
    });

    expect(getLinkExclusionsMock).not.toHaveBeenCalled();
    expect(setModelLinkExclusionMock).not.toHaveBeenCalled();
    expect(result.current.excludedModels.has('model-a')).toBe(true);
  });
});
