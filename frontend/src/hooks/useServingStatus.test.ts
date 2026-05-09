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
