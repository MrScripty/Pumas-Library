import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { useStatus } from './useStatus';
import { useNetworkStatus } from './useNetworkStatus';
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

describe('useStatus', () => {
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
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
      circuit_breaker_rejections: 0,
      retries: 0,
      success_rate: 1,
      circuit_states: {},
      is_offline: false,
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
  let onStatusTelemetryUpdateMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    telemetryCallback = null;
    unsubscribeMock = vi.fn();
    onStatusTelemetryUpdateMock = vi.fn(
      (callback: (notification: StatusTelemetryUpdateNotification) => void) => {
        telemetryCallback = callback;
        return unsubscribeMock;
      }
    );
    isApiAvailableMock.mockReturnValue(true);
    getStatusTelemetrySnapshotMock.mockResolvedValue(snapshot);
    getElectronAPIMock.mockReturnValue({
      onStatusTelemetryUpdate: onStatusTelemetryUpdateMock,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('queues forced refreshes instead of overlapping snapshot requests', async () => {
    vi.useRealTimers();

    let resolveSnapshot: (() => void) | null = null;
    let firstCall = true;
    getStatusTelemetrySnapshotMock.mockImplementation(() => {
      if (firstCall) {
        firstCall = false;
        return new Promise((resolve) => {
          resolveSnapshot = () => resolve(snapshot);
        });
      }
      return Promise.resolve(snapshot);
    });

    const { result } = renderHook(() => useStatus({ initialLoad: false }));

    let firstRefresh!: Promise<void>;
    await act(async () => {
      firstRefresh = result.current.refetch(false, true);
    });

    expect(getStatusTelemetrySnapshotMock).toHaveBeenCalledTimes(1);

    let secondRefresh!: Promise<void>;
    await act(async () => {
      secondRefresh = result.current.refetch(false, true);
      expect(getStatusTelemetrySnapshotMock).toHaveBeenCalledTimes(1);
      resolveSnapshot?.();
      await firstRefresh;
      await secondRefresh;
    });

    await waitFor(() => {
      expect(getStatusTelemetrySnapshotMock).toHaveBeenCalledTimes(2);
    });
  });

  it('applies pushed telemetry updates without polling', async () => {
    const setIntervalSpy = vi.spyOn(global, 'setInterval');
    const updatedSnapshot: StatusTelemetrySnapshot = {
      ...snapshot,
      cursor: 'status-telemetry:2',
      revision: 2,
      network: {
        ...snapshot.network,
        is_offline: true,
      },
      status: {
        ...snapshot.status,
        message: 'updated',
      },
    };

    const { result } = renderHook(() => useStatus({ initialLoad: true }));

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.status?.message).toBe('ready');

    act(() => {
      telemetryCallback?.({
        cursor: updatedSnapshot.cursor,
        snapshot: updatedSnapshot,
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    expect(result.current.status?.message).toBe('updated');
    expect(result.current.networkAvailable).toBe(false);
    expect(setIntervalSpy).not.toHaveBeenCalled();

    setIntervalSpy.mockRestore();
  });

  it('cleans up telemetry subscription and delayed loading timers on unmount', async () => {
    const clearTimeoutSpy = vi.spyOn(global, 'clearTimeout');

    const { unmount } = renderHook(() => useStatus({ initialLoad: true }));

    await act(async () => {
      await Promise.resolve();
    });

    unmount();

    expect(unsubscribeMock).toHaveBeenCalled();
    expect(clearTimeoutSpy).toHaveBeenCalled();

    clearTimeoutSpy.mockRestore();
  });

  it('shares one telemetry snapshot load and subscription across status hooks', async () => {
    const statusHook = renderHook(() => useStatus({ initialLoad: true }));
    const networkHook = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getStatusTelemetrySnapshotMock).toHaveBeenCalledTimes(1);
    expect(onStatusTelemetryUpdateMock).toHaveBeenCalledTimes(1);
    expect(statusHook.result.current.status?.message).toBe('ready');
    expect(networkHook.result.current.successRate).toBe(100);

    statusHook.unmount();
    expect(unsubscribeMock).not.toHaveBeenCalled();

    networkHook.unmount();
    expect(unsubscribeMock).toHaveBeenCalledTimes(1);
  });

  it('keeps enriched status fields when pushed telemetry omits them', async () => {
    const { result } = renderHook(() => useStatus({ initialLoad: true }));

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    act(() => {
      telemetryCallback?.({
        cursor: 'status-telemetry:2',
        snapshot: {
          ...snapshot,
          cursor: 'status-telemetry:2',
          revision: 2,
          status: {
            success: true,
            version: 'test',
            message: 'lightweight update',
            comfyui_running: true,
            ollama_running: false,
            torch_running: false,
            last_launch_error: null,
            last_launch_log: null,
          } as StatusTelemetrySnapshot['status'],
        },
        stale_cursor: false,
        snapshot_required: false,
      });
    });

    expect(result.current.status?.message).toBe('lightweight update');
    expect(result.current.status?.deps_ready).toBe(true);
    expect(result.current.status?.patched).toBe(true);
    expect(result.current.status?.menu_shortcut).toBe(true);
    expect(result.current.status?.desktop_shortcut).toBe(true);
    expect(result.current.status?.shortcut_version).toBeNull();
  });
});
