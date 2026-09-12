import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  getElectronAPIMock,
  getServingStatusMock,
  listServingStatusUpdatesSinceMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  getServingStatusMock: vi.fn(),
  listServingStatusUpdatesSinceMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

import type {
  ServingStatusSnapshot,
  ServingStatusUpdateFeed,
} from '../types/api-serving';
import { useServingStatus } from './useServingStatus';
import { profileControlObservation } from './servingStatusProjection';

function createSnapshot(cursor: string): ServingStatusSnapshot {
  return {
    schema_version: 1,
    cursor,
    endpoint: {
      endpoint_mode: 'pumas_gateway',
      endpoint_url: 'http://127.0.0.1:11434/v1',
      model_count: 0,
    },
    served_models: [],
    last_errors: [],
  };
}

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useServingStatus', () => {
  let servingStatusCallback: ((feed: ServingStatusUpdateFeed) => void) | null;
  let servingStatusErrorCallback: ((message: string) => void) | null;
  let unsubscribeMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    servingStatusCallback = null;
    servingStatusErrorCallback = null;
    unsubscribeMock = vi.fn();
    getServingStatusMock.mockResolvedValue({
      success: true,
      snapshot: createSnapshot('serving:1'),
    });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: getServingStatusMock,
      list_serving_status_updates_since: listServingStatusUpdatesSinceMock,
      onServingStatusUpdate: vi.fn((
        callback: (feed: ServingStatusUpdateFeed) => void,
        onError?: (message: string) => void
      ) => {
        servingStatusCallback = callback;
        servingStatusErrorCallback = onError ?? null;
        return unsubscribeMock;
      }),
    });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('loads the initial snapshot and subscribes without polling', async () => {
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    const { result, unmount } = renderHook(() => useServingStatus());

    await flushMicrotasks();

    expect(getServingStatusMock).toHaveBeenCalledTimes(1);
    expect(listServingStatusUpdatesSinceMock).not.toHaveBeenCalled();
    expect(setIntervalSpy).not.toHaveBeenCalled();
    expect(result.current.cursor).toBe('serving:1');

    unmount();
    expect(unsubscribeMock).toHaveBeenCalledTimes(1);
    setIntervalSpy.mockRestore();
  });

  it('keeps a healthy profile authoritative while another profile is unavailable', () => {
    const rows = [
      { model_id: 'models/healthy', profile_id: 'healthy', provider: 'llama_cpp', load_state: 'loaded' },
      { model_id: 'models/stale', profile_id: 'offline', provider: 'llama_cpp', load_state: 'loaded' },
    ] as const;
    const profiles = [
      { profile_id: 'healthy', generation: 2, observation_state: 'current', catalog_state: 'current', pending_model_ids: [], last_error: null },
      { profile_id: 'offline', generation: 3, observation_state: 'unavailable', catalog_state: 'uncertain', pending_model_ids: [], last_error: 'router unreachable' },
    ] as const;

    expect(profileControlObservation({ kind: 'known', rows: [...rows] }, profiles, 'healthy')).toEqual({
      kind: 'known', rows: [...rows],
    });
    expect(profileControlObservation({ kind: 'known', rows: [...rows] }, profiles, 'offline')).toEqual({
      kind: 'unavailable', message: 'router unreachable',
    });
  });

  it('admits a missing router profile list as a legacy snapshot', async () => {
    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();
    expect(result.current.routerProfiles).toEqual([]);
    expect(result.current.controlObservation).toEqual({ kind: 'known', rows: [] });
  });

  it('retains the prior raw snapshot but withdraws control authority after malformed router metadata', async () => {
    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();
    getServingStatusMock.mockResolvedValueOnce({
      success: true,
      snapshot: { ...createSnapshot('serving:bad-router'), router_profiles: [{ profile_id: 'broken' }] },
    });
    await act(async () => result.current.refreshServingStatus());
    expect(result.current.cursor).toBe('serving:1');
    expect(result.current.controlObservation).toEqual({
      kind: 'unavailable', message: 'Serving status response was malformed',
    });
  });

  it('rejects duplicate router profile metadata rather than applying inconsistent authority', async () => {
    const duplicate = {
      profile_id: 'duplicate', generation: 1, observation_state: 'current',
      catalog_state: 'current', pending_model_ids: [], last_error: null,
    } as const;
    getServingStatusMock.mockResolvedValueOnce({
      success: true,
      snapshot: {
        ...createSnapshot('serving:duplicate-router'),
        router_profiles: [duplicate, { ...duplicate, observation_state: 'unavailable' }],
      },
    });
    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();
    expect(result.current.controlObservation).toEqual({
      kind: 'unavailable', message: 'Serving status response was malformed',
    });
  });

  it('rejects an entire control observation when one served-model row is malformed', async () => {
    getServingStatusMock.mockResolvedValue({
      success: true,
      snapshot: {
        ...createSnapshot('serving:malformed'),
        served_models: [{
          model_id: 'models/chat',
          profile_id: '',
          provider: 'llama_cpp',
          load_state: 'loaded',
        }],
      },
    });

    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();

    expect(result.current.controlObservation).toEqual({
      kind: 'unavailable',
      message: 'Serving status response was malformed',
    });
  });

  it('admits only the error code needed for interrupted serving control', async () => {
    getServingStatusMock.mockResolvedValue({
      success: true,
      snapshot: {
        ...createSnapshot('serving:interrupted'),
        served_models: [{
          model_id: 'models/chat',
          model_alias: 'chat',
          profile_id: 'llama-cpu',
          provider: 'llama_cpp',
          load_state: 'failed',
          last_error: {
            code: 'unknown',
            severity: 'critical',
            message: 'outcome unavailable',
          },
        }],
      },
    });

    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();

    expect(result.current.controlObservation).toEqual({
      kind: 'known',
      rows: [{
        model_id: 'models/chat',
        model_alias: 'chat',
        profile_id: 'llama-cpu',
        provider: 'llama_cpp',
        load_state: 'failed',
        last_error: { code: 'unknown' },
      }],
    });
  });

  it('rejects a malformed served-model error code', async () => {
    getServingStatusMock.mockResolvedValue({
      success: true,
      snapshot: {
        ...createSnapshot('serving:bad-error'),
        served_models: [{
          model_id: 'models/chat',
          profile_id: 'llama-cpu',
          provider: 'llama_cpp',
          load_state: 'failed',
          last_error: { code: 'not-a-serving-error' },
        }],
      },
    });

    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();

    expect(result.current.controlObservation).toEqual({
      kind: 'unavailable',
      message: 'Serving status response was malformed',
    });
  });

  it.each([0, -1, 1.5, 0x1_0000_0000])(
    'rejects malformed served-model context size %s',
    async (contextSize) => {
      getServingStatusMock.mockResolvedValue({
        success: true,
        snapshot: {
          ...createSnapshot('serving:bad-context'),
          served_models: [{
            model_id: 'models/chat',
            profile_id: 'llama-cpu',
            provider: 'llama_cpp',
            load_state: 'loading',
            context_size: contextSize,
          }],
        },
      });

      const { result } = renderHook(() => useServingStatus());
      await flushMicrotasks();

      expect(result.current.controlObservation).toEqual({
        kind: 'unavailable',
        message: 'Serving status response was malformed',
      });
    }
  );

  it.each([undefined, null, 0xffff_ffff])(
    'admits optional served-model context size %s',
    async (contextSize) => {
      getServingStatusMock.mockResolvedValue({
        success: true,
        snapshot: {
          ...createSnapshot('serving:context'),
          served_models: [{
            model_id: 'models/chat',
            profile_id: 'llama-cpu',
            provider: 'llama_cpp',
            load_state: 'loading',
            ...(contextSize !== undefined ? { context_size: contextSize } : {}),
          }],
        },
      });

      const { result } = renderHook(() => useServingStatus());
      await flushMicrotasks();

      expect(result.current.controlObservation).toEqual({
        kind: 'known',
        rows: [{
          model_id: 'models/chat',
          profile_id: 'llama-cpu',
          provider: 'llama_cpp',
          load_state: 'loading',
          ...(contextSize !== undefined ? { context_size: contextSize } : {}),
        }],
      });
    }
  );

  it('does not admit a non-boolean success value as a known empty observation', async () => {
    getServingStatusMock.mockResolvedValue({
      success: 'false',
      snapshot: createSnapshot('serving:not-success'),
    });

    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();

    expect(result.current.controlObservation.kind).toBe('unavailable');
  });

  it('keeps a read failure authoritative until a later admitted snapshot completes', async () => {
    let resolveRefresh: ((value: ReturnType<typeof getServingStatusMock>) => void) | null = null;
    getServingStatusMock.mockRejectedValueOnce(new Error('status timed out'));
    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();
    expect(result.current.controlObservation).toEqual({
      kind: 'unavailable',
      message: 'status timed out',
    });

    getServingStatusMock.mockImplementationOnce(
      () => new Promise((resolve) => {
        resolveRefresh = resolve;
      })
    );
    let refreshPromise: Promise<void> | undefined;
    act(() => {
      refreshPromise = result.current.refreshServingStatus();
    });
    expect(result.current.controlObservation.kind).toBe('unavailable');

    await act(async () => {
      resolveRefresh?.({ success: true, snapshot: createSnapshot('serving:recovered') });
      await refreshPromise;
    });
    expect(result.current.controlObservation).toEqual({ kind: 'known', rows: [] });
  });

  it('refreshes the backend-owned snapshot when a pushed update requires it', async () => {
    getServingStatusMock
      .mockResolvedValueOnce({
        success: true,
        snapshot: createSnapshot('serving:1'),
      })
      .mockResolvedValueOnce({
        success: true,
        snapshot: createSnapshot('serving:2'),
      });

    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();

    await act(async () => {
      servingStatusCallback?.({
        cursor: 'serving:2',
        events: [],
        stale_cursor: false,
        snapshot_required: true,
      });
      await Promise.resolve();
    });

    expect(getServingStatusMock).toHaveBeenCalledTimes(2);
    expect(listServingStatusUpdatesSinceMock).not.toHaveBeenCalled();
    expect(result.current.cursor).toBe('serving:2');
  });

  it('surfaces pushed subscription errors without creating a polling timer', async () => {
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    const { result } = renderHook(() => useServingStatus());

    await flushMicrotasks();

    act(() => {
      servingStatusErrorCallback?.('Serving-status stream failed: connection refused');
    });

    expect(result.current.error).toBe('Serving-status stream failed: connection refused');
    expect(listServingStatusUpdatesSinceMock).not.toHaveBeenCalled();
    expect(setIntervalSpy).not.toHaveBeenCalled();
    setIntervalSpy.mockRestore();
  });

  it('keeps stream recovery unavailable until a fresh admitted read completes', async () => {
    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();
    act(() => {
      servingStatusErrorCallback?.('Serving-status stream failed');
    });
    expect(result.current.controlObservation.kind).toBe('unavailable');

    let finishRead: (value: unknown) => void = () => undefined;
    getServingStatusMock.mockImplementationOnce(() => new Promise((resolve) => { finishRead = resolve; }));
    act(() => {
      servingStatusCallback?.({
        cursor: 'serving:1',
        events: [],
        stale_cursor: false,
        snapshot_required: false,
      });
    });
    expect(result.current.controlObservation.kind).toBe('unavailable');
    await act(async () => {
      finishRead({ success: true, snapshot: createSnapshot('serving:recovered') });
      await Promise.resolve();
    });
    expect(result.current.controlObservation).toEqual({ kind: 'known', rows: [] });
  });

  it('reports an unavailable push bridge without falling back to update polling', async () => {
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    getElectronAPIMock.mockReturnValue({
      get_serving_status: getServingStatusMock,
      list_serving_status_updates_since: listServingStatusUpdatesSinceMock,
    });

    const { result } = renderHook(() => useServingStatus());
    await flushMicrotasks();

    expect(result.current.error).toBe('Serving status push subscription unavailable');
    expect(listServingStatusUpdatesSinceMock).not.toHaveBeenCalled();
    expect(setIntervalSpy).not.toHaveBeenCalled();
    setIntervalSpy.mockRestore();
  });
});
