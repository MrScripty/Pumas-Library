import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { useStatus } from './useStatus';
import type {
  GetLibraryStatusResponse,
  NetworkStatusResponse,
  StatusResponse,
  SystemResourcesResponse,
} from '../types/api';

const {
  getStatusMock,
  getSystemResourcesMock,
  getNetworkStatusMock,
  getLibraryStatusMock,
  isApiAvailableMock,
} = vi.hoisted(() => ({
  getStatusMock: vi.fn<() => Promise<StatusResponse>>(),
  getSystemResourcesMock: vi.fn<() => Promise<SystemResourcesResponse>>(),
  getNetworkStatusMock: vi.fn<() => Promise<NetworkStatusResponse>>(),
  getLibraryStatusMock: vi.fn<() => Promise<GetLibraryStatusResponse>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_status: getStatusMock,
    get_system_resources: getSystemResourcesMock,
    get_network_status: getNetworkStatusMock,
    get_library_status: getLibraryStatusMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

describe('useStatus', () => {
  const statusResponse: StatusResponse = {
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
  };

  const systemResourcesResponse: SystemResourcesResponse = {
    success: true,
    resources: {
      cpu: { usage: 0 },
      gpu: { usage: 0, memory: 0, memory_total: 1 },
      ram: { usage: 0, total: 1 },
      disk: { usage: 0, total: 1, free: 1 },
    },
  };

  const networkStatusResponse: NetworkStatusResponse = {
    success: true,
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    circuit_breaker_rejections: 0,
    retries: 0,
    success_rate: 1,
    circuit_states: {},
    is_offline: false,
  };

  const libraryStatusResponse: GetLibraryStatusResponse = {
    success: true,
    indexing: false,
    deep_scan_in_progress: false,
    model_count: 0,
  };

  beforeEach(() => {
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
    getStatusMock.mockResolvedValue(statusResponse);
    getSystemResourcesMock.mockResolvedValue(systemResourcesResponse);
    getNetworkStatusMock.mockResolvedValue(networkStatusResponse);
    getLibraryStatusMock.mockResolvedValue(libraryStatusResponse);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('queues forced refreshes instead of overlapping requests', async () => {
    vi.useRealTimers();

    let resolveStatus: (() => void) | null = null;
    let firstCall = true;
    getStatusMock.mockImplementation(() => {
      if (firstCall) {
        firstCall = false;
        return new Promise((resolve) => {
          resolveStatus = () => resolve(statusResponse);
        });
      }
      return Promise.resolve(statusResponse);
    });

    const { result } = renderHook(() => useStatus({ initialLoad: false, pollInterval: 10_000 }));

    let firstRefresh!: Promise<void>;
    await act(async () => {
      firstRefresh = result.current.refetch(false, true);
    });

    expect(getStatusMock).toHaveBeenCalledTimes(1);

    let secondRefresh!: Promise<void>;
    await act(async () => {
      secondRefresh = result.current.refetch(false, true);
      expect(getStatusMock).toHaveBeenCalledTimes(1);
      resolveStatus?.();
      await firstRefresh;
      await secondRefresh;
    });

    await waitFor(() => {
      expect(getStatusMock).toHaveBeenCalledTimes(2);
    });
  });

  it('cleans up polling and delayed loading timers on unmount', async () => {
    const clearIntervalSpy = vi.spyOn(global, 'clearInterval');
    const clearTimeoutSpy = vi.spyOn(global, 'clearTimeout');

    const { unmount } = renderHook(() => useStatus({ initialLoad: true, pollInterval: 10_000 }));

    await act(async () => {
      await Promise.resolve();
    });

    unmount();

    expect(clearIntervalSpy).toHaveBeenCalled();
    expect(clearTimeoutSpy).toHaveBeenCalled();

    clearIntervalSpy.mockRestore();
    clearTimeoutSpy.mockRestore();
  });
});
