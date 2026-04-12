import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  getNetworkStatusMock,
} = vi.hoisted(() => ({
  getNetworkStatusMock: vi.fn(),
}));

vi.mock('../api/import', () => ({
  importAPI: {
    getNetworkStatus: getNetworkStatusMock,
  },
}));

import { useNetworkStatus } from './useNetworkStatus';

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('useNetworkStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    getNetworkStatusMock.mockResolvedValue({
      success: true,
      is_offline: false,
      success_rate: 100,
      circuit_breaker_rejections: 0,
      circuit_states: {},
      total_requests: 0,
      failed_requests: 0,
    });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('loads network status on mount and derives offline and rate-limit flags', async () => {
    getNetworkStatusMock.mockResolvedValueOnce({
      success: true,
      is_offline: true,
      success_rate: 42,
      circuit_breaker_rejections: 3,
      circuit_states: {
        huggingface: 'open',
      },
      total_requests: 12,
      failed_requests: 7,
    });

    const { result } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    expect(getNetworkStatusMock).toHaveBeenCalledTimes(1);
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

  it('treats zero-request responses as healthy even without an explicit success rate', async () => {
    getNetworkStatusMock.mockResolvedValueOnce({
      success: true,
      is_offline: false,
      circuit_breaker_rejections: 0,
      circuit_states: {},
      total_requests: 0,
      failed_requests: 0,
    });

    const { result } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.successRate).toBe(100);
    expect(result.current.isRateLimited).toBe(false);
  });

  it('polls on the interval and supports manual refresh without overlapping in-flight requests', async () => {
    const firstRequest = createDeferred<{
      success: boolean;
      is_offline: boolean;
      success_rate: number;
      circuit_breaker_rejections: number;
      circuit_states: Record<string, string>;
      total_requests: number;
      failed_requests: number;
    }>();

    getNetworkStatusMock
      .mockReturnValueOnce(firstRequest.promise)
      .mockResolvedValueOnce({
        success: true,
        is_offline: false,
        success_rate: 88,
        circuit_breaker_rejections: 1,
        circuit_states: {
          huggingface: 'closed',
        },
        total_requests: 8,
        failed_requests: 1,
      });

    const { result } = renderHook(() => useNetworkStatus());

    expect(getNetworkStatusMock).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.refresh();
    });

    expect(getNetworkStatusMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(5000);
      await Promise.resolve();
    });

    expect(getNetworkStatusMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      firstRequest.resolve({
        success: true,
        is_offline: false,
        success_rate: 91,
        circuit_breaker_rejections: 0,
        circuit_states: {
          huggingface: 'half-open',
        },
        total_requests: 11,
        failed_requests: 1,
      });
      await Promise.resolve();
    });

    expect(result.current.successRate).toBe(91);
    expect(result.current.circuitStates).toEqual({ huggingface: 'half-open' });

    await act(async () => {
      vi.advanceTimersByTime(5000);
      await Promise.resolve();
    });

    expect(getNetworkStatusMock).toHaveBeenCalledTimes(2);
    expect(result.current.successRate).toBe(88);
    expect(result.current.circuitBreakerRejections).toBe(1);
  });

  it('surfaces backend and thrown errors without leaving loading stuck', async () => {
    getNetworkStatusMock
      .mockResolvedValueOnce({
        success: false,
        error: 'backend unavailable',
      })
      .mockRejectedValueOnce(new Error('socket closed'));

    const { result } = renderHook(() => useNetworkStatus());

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.error).toBe('backend unavailable');
    expect(result.current.isLoading).toBe(false);

    act(() => {
      result.current.refresh();
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.error).toBe('socket closed');
    expect(result.current.isLoading).toBe(false);
  });
});
