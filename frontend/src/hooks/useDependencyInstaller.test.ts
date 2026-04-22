import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BaseResponse } from '../types/api';
import { useDependencyInstaller } from './useDependencyInstaller';

const {
  installDepsMock,
  isApiAvailableMock,
} = vi.hoisted(() => ({
  installDepsMock: vi.fn<() => Promise<BaseResponse>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    install_deps: installDepsMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

describe('useDependencyInstaller', () => {
  beforeEach(() => {
    isApiAvailableMock.mockReturnValue(true);
    installDepsMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('installs dependencies and refreshes status', async () => {
    const refetchStatus = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDependencyInstaller({ refetchStatus }));

    await act(async () => {
      await result.current.installDependencies();
    });

    expect(installDepsMock).toHaveBeenCalledTimes(1);
    expect(refetchStatus).toHaveBeenCalledTimes(1);
    expect(result.current.isInstallingDeps).toBe(false);
  });

  it('exposes pending install state while the backend call is active', async () => {
    let resolveInstall: (_response: BaseResponse) => void = () => undefined;
    const installPromise = new Promise<BaseResponse>((resolve) => {
      resolveInstall = resolve;
    });
    installDepsMock.mockReturnValue(installPromise);
    const refetchStatus = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDependencyInstaller({ refetchStatus }));
    let action: Promise<void> = Promise.resolve();

    act(() => {
      action = result.current.installDependencies();
    });

    expect(result.current.isInstallingDeps).toBe(true);

    await act(async () => {
      resolveInstall({ success: true });
      await action;
    });

    expect(result.current.isInstallingDeps).toBe(false);
    expect(refetchStatus).toHaveBeenCalledTimes(1);
  });

  it('resets install state when dependency installation fails', async () => {
    installDepsMock.mockRejectedValue(new Error('install failed'));
    const refetchStatus = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDependencyInstaller({ refetchStatus }));

    await act(async () => {
      await result.current.installDependencies();
    });

    expect(result.current.isInstallingDeps).toBe(false);
    expect(refetchStatus).not.toHaveBeenCalled();
  });

  it('does not call dependency APIs when the bridge is unavailable', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const refetchStatus = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDependencyInstaller({ refetchStatus }));

    await act(async () => {
      await result.current.installDependencies();
    });

    expect(installDepsMock).not.toHaveBeenCalled();
    expect(refetchStatus).not.toHaveBeenCalled();
    expect(result.current.isInstallingDeps).toBe(false);
  });
});
