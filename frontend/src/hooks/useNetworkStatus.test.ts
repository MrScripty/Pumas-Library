import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { StatusTelemetrySnapshot, StatusTelemetryUpdateNotification } from '../types/api';

const {
  getStatusTelemetrySnapshotMock,
  getElectronAPIMock,
  isApiAvailableMock,
} = vi.hoisted(() => ({
  getStatusTelemetrySnapshotMock: vi.fn<() => Promise<StatusTelemetrySnapshot>>(),
  getElectronAPIMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_status_telemetry_snapshot: getStatusTelemetrySnapshotMock,
  },
  getElectronAPI: getElectronAPIMock,
  isAPIAvailable: isApiAvailableMock,
}));

import { useNetworkStatus } from './useNetworkStatus';

describe('useNetworkStatus', () => {
  const snapshot: StatusTelemetrySnapshot = {
    cursor: 'status-telemetry:1',
    revision: 1,
    sampled_at: '2026-05-06T00:00:00Z',
    source_state: 'ready',
    status: {
      success: true,
      version: 'test',
      deps_ready: true,
      patched: true,
      menu_shortcut: true,
      desktop_shortcut: true,
      shortcut_version: null,
      message: 'ready',
      comfyui_running: false,
      ollama_running: false,
      torch_running: false,
      last_launch_error: null,
      last_launch_log: null,
    },
    resources: {
      cpu: { usage: 0 },
      gpu: { usage: 0, memory: 0, memory_total: 1 },
      ram: { usage: 0, total: 1 },
      disk: { usage: 0, total: 1, free: 1 },
    },
    network: {
      success: true,
      is_offline: false,
      success_rate: 1,
      circuit_breaker_rejections: 0,
      circuit_states: {},
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
      retries: 0,
    },
    library: {
      success: true,
      indexing: false,
      deep_scan_in_progress: false,
      model_count: 0,
    },
    model_library_loaded: true,
  };

  let telemetryCallback: ((notification: StatusTelemetryUpdateNotification) => void) | null;
  let unsubscribeMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.clearAllMocks();
    telemetryCallback = null;
    unsubscribeMock = vi.fn();
    isApiAvailableMock.mockReturnValue(true);
    getStatusTelemetrySnapshotMock.mockResolvedValue(snapshot);
    getElectronAPIMock.mockReturnValue({
      onStatusTelemetryUpdate: vi.fn((callback) => {
        telemetryCallback = callback;
        return unsubscribeMock;
      }),
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('loads network status from telemetry on mount and derives flags', async () => {
    getStatusTelemetrySnapshotMock.mockResolvedValueOnce({
      ...snapshot,
      network: {
        ...snapshot.network,
        is_offline: true,
        success_rate: 0.42,
        circuit_breaker_rejections: 3,
        circuit_states: {
          huggingface: 'open',
        },
        total_requests: 12,
        failed_requests: 7,
      },
    });

    const { result } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    expect(getStatusTelemetrySnapshotMock).toHaveBeenCalledTimes(1);
    expect(result.current.isOffline).toBe(true);
    expect(result.current.isRateLimited).toBe(true);
    expect(result.current.successRate).toBe(42);
    expect(result.current.circuitBreakerRejections).toBe(3);
    expect(result.current.circuitStates).toEqual({ huggingface: 'open' });
    expect(result.current.totalRequests).toBe(12);
    expect(result.current.failedRequests).toBe(7);
    expect(result.current.error).toBeNull();
    expect(result.current.isLoading).toBe(false);
  });

  it('treats zero-request responses as healthy', async () => {
    const { result } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.successRate).toBe(100);
    expect(result.current.isRateLimited).toBe(false);
  });

  it('applies pushed telemetry updates without polling', async () => {
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    const { result } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      telemetryCallback?.({
        cursor: 'status-telemetry:2',
        snapshot: {
          ...snapshot,
          cursor: 'status-telemetry:2',
          revision: 2,
          network: {
            ...snapshot.network,
            success_rate: 88,
            circuit_breaker_rejections: 1,
            circuit_states: {
              huggingface: 'closed',
            },
            total_requests: 8,
            failed_requests: 1,
          },
        },
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    expect(result.current.successRate).toBe(88);
    expect(result.current.circuitBreakerRejections).toBe(1);
    expect(result.current.circuitStates).toEqual({ huggingface: 'closed' });
    expect(setIntervalSpy).not.toHaveBeenCalled();

    setIntervalSpy.mockRestore();
  });

  it('supports manual refresh and surfaces thrown errors', async () => {
    getStatusTelemetrySnapshotMock
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(new Error('socket closed'));

    const { result } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.error).toBeNull();

    act(() => {
      result.current.refresh();
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.error).toBe('socket closed');
    expect(result.current.isLoading).toBe(false);
  });

  it('unsubscribes on unmount', async () => {
    const { unmount } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    unmount();

    expect(unsubscribeMock).toHaveBeenCalled();
  });
});
