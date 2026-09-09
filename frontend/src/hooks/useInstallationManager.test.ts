import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import type { RuntimeInstallationProgress } from '../generated/desktop-contract';
import type { InstallationProgress, VersionRelease } from '../types/versions';

const {
  cancelInstallationApiMock,
  getInstallationProgressMock,
  installVersionApiMock,
  isApiAvailableMock,
  openActiveInstallMock,
  openPathMock,
  getVersionInfoMock,
  normalizeInstallationProgressMock,
  resetInstallationProgressTrackingMock,
  removeVersionApiMock,
  switchVersionApiMock,
} = vi.hoisted(() => ({
  cancelInstallationApiMock: vi.fn<(_appId: string) => Promise<{ success: boolean; error?: string }>>(),
  getInstallationProgressMock: vi.fn<(
    _appId: string
  ) => Promise<InstallationProgress | RuntimeInstallationProgress | null>>(),
  installVersionApiMock: vi.fn<(_tag: string, _appId: string) => Promise<{ success: boolean; error?: string }>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  openActiveInstallMock: vi.fn<() => Promise<boolean>>(),
  openPathMock: vi.fn<(_path: string) => Promise<boolean>>(),
  getVersionInfoMock: vi.fn<(_tag: string) => Promise<unknown>>(),
  normalizeInstallationProgressMock: vi.fn(),
  resetInstallationProgressTrackingMock: vi.fn<(_state: unknown) => void>(),
  removeVersionApiMock: vi.fn<(_tag: string, _appId: string) => Promise<{ success: boolean }>>(),
  switchVersionApiMock: vi.fn<(_tag: string, _appId: string) => Promise<{ success: boolean }>>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    cancel_installation: cancelInstallationApiMock,
    get_installation_progress: getInstallationProgressMock,
    install_version: installVersionApiMock,
    remove_version: removeVersionApiMock,
    switch_version: switchVersionApiMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

vi.mock('./useInstallationAccess', () => ({
  useInstallationAccess: () => ({
    getVersionInfo: getVersionInfoMock,
    openActiveInstall: openActiveInstallMock,
    openPath: openPathMock,
  }),
}));

vi.mock('./installationProgressTracking', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./installationProgressTracking')>();
  return {
    ...actual,
    normalizeInstallationProgress: normalizeInstallationProgressMock,
    projectInstallationProgress: (
      progress: InstallationProgress | RuntimeInstallationProgress
    ) => 'startedAt' in progress ? actual.projectInstallationProgress(progress) : progress,
    resetInstallationProgressTracking: resetInstallationProgressTrackingMock,
  };
});

import { useInstallationManager } from './useInstallationManager';

interface Deferred<T> {
  promise: Promise<T>;
  reject: (error: unknown) => void;
  resolve: (value: T) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, reject, resolve };
}

const availableVersions: VersionRelease[] = [
  {
    tagName: 'v1.2.3',
    name: 'Version 1.2.3',
    publishedAt: '2026-04-12T00:00:00Z',
    prerelease: false,
    totalSize: 4096,
    archiveSize: 2048,
  },
];
const installingAvailableVersions = availableVersions.map((release) => ({
  ...release,
  installing: true,
}));

const activeProgress: InstallationProgress = {
  tag: 'v1.2.3',
  started_at: '2026-04-12T00:00:00Z',
  stage: 'download',
  stage_progress: 50,
  overall_progress: 25,
  current_item: 'archive.zip',
  download_speed: 1024,
  eta_seconds: 30,
  total_size: 4096,
  downloaded_bytes: 1024,
  dependency_count: null,
  completed_dependencies: 0,
  completed_items: [],
  error: null,
};

const activeWireProgress: RuntimeInstallationProgress = {
  tag: activeProgress.tag,
  startedAt: activeProgress.started_at,
  stage: activeProgress.stage,
  stageProgress: activeProgress.stage_progress,
  overallProgress: activeProgress.overall_progress,
  currentItem: activeProgress.current_item,
  downloadSpeed: activeProgress.download_speed,
  etaSeconds: activeProgress.eta_seconds,
  totalSize: activeProgress.total_size,
  downloadedBytes: activeProgress.downloaded_bytes,
  dependencyCount: activeProgress.dependency_count,
  completedDependencies: activeProgress.completed_dependencies,
  completedItems: [],
  error: activeProgress.error,
  completedAt: null,
  success: null,
  logPath: null,
};

function progressFor(tag: string): InstallationProgress {
  return { ...activeProgress, tag };
}

describe('useInstallationManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
    normalizeInstallationProgressMock.mockImplementation((progress: InstallationProgress) => ({
      adjustedProgress: { ...progress, eta_seconds: 15 },
      networkStatus: 'downloading',
    }));
    cancelInstallationApiMock.mockResolvedValue({ success: true });
    installVersionApiMock.mockResolvedValue({ success: true });
    switchVersionApiMock.mockResolvedValue({ success: true });
    removeVersionApiMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('discovers manager-owned progress from an installing release hint', async () => {
    getInstallationProgressMock.mockResolvedValue(activeWireProgress);

    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions: installingAvailableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await Promise.resolve();
    });

    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);
    expect(getInstallationProgressMock).toHaveBeenCalledWith('torch');
    expect(result.current.installingTag).toBe('v1.2.3');
    expect(result.current.installationProgress).toEqual({
      ...activeProgress,
      eta_seconds: 15,
      completed_at: undefined,
      success: undefined,
      log_path: null,
    });
    expect(result.current.installNetworkStatus).toBe('downloading');
  });

  it('starts polling a requested install only after the backend accepts its lifecycle', async () => {
    const installAdmission = deferred<{ success: boolean; error?: string }>();
    installVersionApiMock.mockReturnValue(installAdmission.promise);
    getInstallationProgressMock.mockResolvedValue(activeProgress);

    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    let installation: Promise<boolean> | undefined;
    await act(async () => {
      installation = result.current.installVersion('v1.2.3');
      await Promise.resolve();
    });

    expect(getInstallationProgressMock).not.toHaveBeenCalled();

    await act(async () => {
      installAdmission.resolve({ success: true });
      await installation;
      await Promise.resolve();
    });

    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);
    expect(getInstallationProgressMock).toHaveBeenCalledWith('torch');
    expect(result.current.installingTag).toBe('v1.2.3');
    expect(result.current.installationProgress?.tag).toBe('v1.2.3');
  });

  it('never overlaps progress requests and schedules the next poll only after settlement', async () => {
    const firstProgress = deferred<InstallationProgress | null>();
    const secondProgress = deferred<InstallationProgress | null>();
    let activeRequests = 0;
    let maximumActiveRequests = 0;
    getInstallationProgressMock
      .mockImplementationOnce(async () => {
        activeRequests += 1;
        maximumActiveRequests = Math.max(maximumActiveRequests, activeRequests);
        try {
          return await firstProgress.promise;
        } finally {
          activeRequests -= 1;
        }
      })
      .mockImplementationOnce(async () => {
        activeRequests += 1;
        maximumActiveRequests = Math.max(maximumActiveRequests, activeRequests);
        try {
          return await secondProgress.promise;
        } finally {
          activeRequests -= 1;
        }
      });

    const { result } = renderHook(() => useInstallationManager({
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
    });

    await act(async () => {
      vi.advanceTimersByTime(4000);
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      firstProgress.resolve(activeProgress);
      await firstProgress.promise;
      await Promise.resolve();
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(800);
      await Promise.resolve();
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(2);
    expect(maximumActiveRequests).toBe(1);

    await act(async () => {
      secondProgress.resolve(activeProgress);
      await secondProgress.promise;
    });
  });

  it('serializes a new app lifecycle behind an old request and ignores the old completion', async () => {
    const oldProgress = deferred<InstallationProgress | null>();
    const newProgress = deferred<InstallationProgress | null>();
    getInstallationProgressMock
      .mockReturnValueOnce(oldProgress.promise)
      .mockReturnValueOnce(newProgress.promise);

    const { result, rerender } = renderHook(
      ({ appId }) => useInstallationManager({
        appId,
        availableVersions,
        onRefreshVersions: vi.fn(),
      }),
      { initialProps: { appId: 'torch' } }
    );

    await act(async () => {
      await result.current.installVersion('old-tag');
    });
    expect(getInstallationProgressMock).toHaveBeenCalledWith('torch');

    rerender({ appId: 'ollama' });
    await act(async () => {
      await result.current.installVersion('new-tag');
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      oldProgress.resolve({
        ...progressFor('old-tag'),
        completed_at: '2026-04-12T00:05:00Z',
        success: true,
      });
      await oldProgress.promise;
      await Promise.resolve();
    });
    expect(getInstallationProgressMock).toHaveBeenNthCalledWith(2, 'ollama');
    expect(result.current.installationProgress).toBeNull();

    await act(async () => {
      newProgress.resolve(progressFor('new-tag'));
      await newProgress.promise;
      await Promise.resolve();
    });
    expect(result.current.installingTag).toBe('new-tag');
    expect(result.current.installationProgress?.tag).toBe('new-tag');
  });

  it('supersedes an old tag lifecycle without overlapping its request', async () => {
    const oldProgress = deferred<InstallationProgress | null>();
    const newProgress = deferred<InstallationProgress | null>();
    getInstallationProgressMock
      .mockReturnValueOnce(oldProgress.promise)
      .mockReturnValueOnce(newProgress.promise);

    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.installVersion('old-tag');
      await result.current.installVersion('new-tag');
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      oldProgress.resolve(progressFor('old-tag'));
      await oldProgress.promise;
      await Promise.resolve();
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(2);
    expect(result.current.installationProgress).toBeNull();

    await act(async () => {
      newProgress.resolve(progressFor('new-tag'));
      await newProgress.promise;
      await Promise.resolve();
    });
    expect(result.current.installingTag).toBe('new-tag');
    expect(result.current.installationProgress?.tag).toBe('new-tag');
  });

  it('prevents a completion after disable or unmount from mutating state or restarting polling', async () => {
    const disabledProgress = deferred<InstallationProgress | null>();
    getInstallationProgressMock.mockReturnValueOnce(disabledProgress.promise);

    const { result, rerender, unmount } = renderHook(
      ({ enabled }) => useInstallationManager({
        enabled,
        availableVersions,
        onRefreshVersions: vi.fn(),
      }),
      { initialProps: { enabled: true } }
    );

    await act(async () => {
      await result.current.installVersion('v1.2.3');
    });
    rerender({ enabled: false });

    await act(async () => {
      disabledProgress.resolve(activeProgress);
      await disabledProgress.promise;
      await Promise.resolve();
    });
    expect(result.current.installingTag).toBeNull();
    expect(result.current.installationProgress).toBeNull();

    unmount();
    await act(async () => {
      vi.advanceTimersByTime(4000);
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);
  });

  it('observes a rejected request after unmount without scheduling more work', async () => {
    const pendingProgress = deferred<InstallationProgress | null>();
    getInstallationProgressMock.mockReturnValueOnce(pendingProgress.promise);

    const { result, unmount } = renderHook(() => useInstallationManager({
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
    });
    unmount();

    await act(async () => {
      pendingProgress.reject(new APIError('late failure', 'get_installation_progress'));
      await pendingProgress.promise.catch(() => undefined);
      await Promise.resolve();
      vi.advanceTimersByTime(4000);
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);
  });

  it('refreshes versions and retains successful terminal progress for presentation', async () => {
    const onRefreshVersions = vi.fn().mockResolvedValue(undefined);
    getInstallationProgressMock
      .mockResolvedValueOnce(activeProgress)
      .mockResolvedValueOnce({
        ...activeProgress,
        completed_at: '2026-04-12T00:05:00Z',
        success: true,
      });

    const { result } = renderHook(() => useInstallationManager({
      availableVersions,
      onRefreshVersions,
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
      await Promise.resolve();
    });
    expect(result.current.installationProgress?.tag).toBe('v1.2.3');

    await act(async () => {
      vi.advanceTimersByTime(800);
      await Promise.resolve();
    });

    expect(onRefreshVersions).toHaveBeenCalledTimes(1);
    expect(result.current.installingTag).toBeNull();
    expect(result.current.installationProgress).toEqual({
      ...activeProgress,
      completed_at: '2026-04-12T00:05:00Z',
      eta_seconds: 15,
      success: true,
    });
    expect(result.current.installNetworkStatus).toBe('idle');
  });

  it('preserves terminal failure for presentation', async () => {
    const failedProgress: InstallationProgress = {
      ...activeProgress,
      completed_at: '2026-04-12T00:05:00Z',
      success: false,
      error: 'Archive checksum mismatch',
    };
    normalizeInstallationProgressMock.mockImplementation((progress: InstallationProgress) => ({
      adjustedProgress: progress,
      networkStatus: 'failed',
    }));
    getInstallationProgressMock.mockResolvedValue(failedProgress);

    const { result } = renderHook(() => useInstallationManager({
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
      await Promise.resolve();
    });

    expect(result.current.installingTag).toBeNull();
    expect(result.current.installNetworkStatus).toBe('failed');
    expect(result.current.installationProgress?.error).toBe('Archive checksum mismatch');
  });

  it('preserves terminal cancellation separately and scopes its action to the app', async () => {
    const cancelledProgress: InstallationProgress = {
      ...activeProgress,
      completed_at: '2026-04-12T00:05:00Z',
      success: false,
      error: 'User cancelled installation',
    };
    normalizeInstallationProgressMock.mockImplementation((progress: InstallationProgress) => ({
      adjustedProgress: progress,
      networkStatus: 'failed',
    }));
    getInstallationProgressMock.mockResolvedValue(cancelledProgress);

    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
      await result.current.cancelInstallation();
      await Promise.resolve();
    });

    expect(cancelInstallationApiMock).toHaveBeenCalledWith('torch');
    expect(result.current.installingTag).toBeNull();
    expect(result.current.installNetworkStatus).toBe('failed');
    expect(result.current.installationProgress?.error).toContain('cancel');
  });

  it('keeps polling after cancellation is requested until progress becomes terminal', async () => {
    getInstallationProgressMock.mockResolvedValue(activeProgress);
    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
      await Promise.resolve();
      await result.current.cancelInstallation();
    });

    expect(result.current.installingTag).toBe('v1.2.3');
    expect(result.current.installationProgress?.completed_at).toBeUndefined();
    await act(async () => {
      vi.advanceTimersByTime(800);
      await Promise.resolve();
    });
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(2);
  });

  it('resets transient state when the install request fails', async () => {
    const pendingProgress = deferred<InstallationProgress | null>();
    getInstallationProgressMock.mockReturnValueOnce(pendingProgress.promise);
    installVersionApiMock.mockResolvedValue({ success: false, error: 'install denied' });

    const { result } = renderHook(() => useInstallationManager({
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await expect(act(async () => result.current.installVersion('v1.2.3'))).rejects.toBeInstanceOf(APIError);
    expect(result.current.installingTag).toBeNull();
    expect(result.current.installationProgress).toBeNull();
    expect(result.current.installNetworkStatus).toBe('idle');

    await act(async () => {
      pendingProgress.resolve(activeProgress);
      await pendingProgress.promise;
    });
  });

  it('keeps version actions and installation access behind the manager Interface', async () => {
    const onRefreshVersions = vi.fn().mockResolvedValue(undefined);
    openActiveInstallMock.mockResolvedValue(true);
    openPathMock.mockResolvedValue(true);
    getVersionInfoMock.mockResolvedValue({ tag: 'v1.2.3', installed: true, size: null });

    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions,
      onRefreshVersions,
    }));

    await act(async () => {
      await result.current.switchVersion('v1.2.3');
      await result.current.removeVersion('v1.2.3');
      await result.current.openActiveInstall();
      await result.current.openPath('/tmp/v1.2.3');
      await result.current.getVersionInfo('v1.2.3');
    });

    expect(switchVersionApiMock).toHaveBeenCalledWith('v1.2.3', 'torch');
    expect(removeVersionApiMock).toHaveBeenCalledWith('v1.2.3', 'torch');
    expect(onRefreshVersions).toHaveBeenCalledTimes(2);
    expect(openActiveInstallMock).toHaveBeenCalledTimes(1);
    expect(openPathMock).toHaveBeenCalledWith('/tmp/v1.2.3');
    expect(getVersionInfoMock).toHaveBeenCalledWith('v1.2.3');
  });

  it('awaits the version refresh after a confirmed switch', async () => {
    const refresh = deferred<undefined>();
    const onRefreshVersions = vi.fn(() => refresh.promise);
    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch', availableVersions, onRefreshVersions,
    }));

    let settled = false;
    const switching = result.current.switchVersion('v1.2.3').then(value => {
      settled = true;
      return value;
    });
    await Promise.resolve();

    expect(settled).toBe(false);
    expect(switchVersionApiMock).toHaveBeenCalledTimes(1);
    expect(onRefreshVersions).toHaveBeenCalledTimes(1);

    refresh.resolve(undefined);
    await expect(switching).resolves.toBe(true);
  });

  it('rejects a false switch confirmation without refreshing or retrying', async () => {
    switchVersionApiMock.mockResolvedValue({ success: false });
    const onRefreshVersions = vi.fn();
    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch', availableVersions, onRefreshVersions,
    }));

    await expect(result.current.switchVersion('v1.2.3')).rejects.toMatchObject({
      name: 'APIError', endpoint: 'switch_version', message: 'Failed to switch version',
    });
    expect(switchVersionApiMock).toHaveBeenCalledTimes(1);
    expect(onRefreshVersions).not.toHaveBeenCalled();
  });

  it('propagates a post-switch refresh failure without repeating selection', async () => {
    const refreshError = new Error('refresh failed');
    const onRefreshVersions = vi.fn().mockRejectedValue(refreshError);
    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch', availableVersions, onRefreshVersions,
    }));

    await expect(result.current.switchVersion('v1.2.3')).rejects.toBe(refreshError);
    expect(switchVersionApiMock).toHaveBeenCalledTimes(1);
    expect(onRefreshVersions).toHaveBeenCalledTimes(1);
  });

  it('awaits the version refresh after a confirmed removal', async () => {
    const refresh = deferred<undefined>();
    const onRefreshVersions = vi.fn(() => refresh.promise);
    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch', availableVersions, onRefreshVersions,
    }));

    let settled = false;
    const removal = result.current.removeVersion('v1.2.3').then(value => {
      settled = true;
      return value;
    });
    await Promise.resolve();

    expect(settled).toBe(false);
    expect(removeVersionApiMock).toHaveBeenCalledTimes(1);
    expect(onRefreshVersions).toHaveBeenCalledTimes(1);

    refresh.resolve(undefined);
    await expect(removal).resolves.toBe(true);
  });

  it('rejects a false removal confirmation without refreshing or retrying', async () => {
    removeVersionApiMock.mockResolvedValue({ success: false });
    const onRefreshVersions = vi.fn();
    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch', availableVersions, onRefreshVersions,
    }));

    await expect(result.current.removeVersion('v1.2.3')).rejects.toMatchObject({
      name: 'APIError', endpoint: 'remove_version', message: 'Failed to remove version',
    });
    expect(removeVersionApiMock).toHaveBeenCalledTimes(1);
    expect(onRefreshVersions).not.toHaveBeenCalled();
  });

  it('propagates a post-removal refresh failure without repeating removal', async () => {
    const refreshError = new Error('refresh failed');
    const onRefreshVersions = vi.fn().mockRejectedValue(refreshError);
    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch', availableVersions, onRefreshVersions,
    }));

    await expect(result.current.removeVersion('v1.2.3')).rejects.toBe(refreshError);
    expect(removeVersionApiMock).toHaveBeenCalledTimes(1);
    expect(onRefreshVersions).toHaveBeenCalledTimes(1);
  });
});
