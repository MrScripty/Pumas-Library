import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import type { InstallationProgress, VersionRelease } from '../types/versions';

const {
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
  getInstallationProgressMock: vi.fn<(_appId: string) => Promise<InstallationProgress | null>>(),
  installVersionApiMock: vi.fn<(_tag: string, _appId: string) => Promise<{ success: boolean; error?: string }>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  openActiveInstallMock: vi.fn<() => Promise<boolean>>(),
  openPathMock: vi.fn<(_path: string) => Promise<boolean>>(),
  getVersionInfoMock: vi.fn<(_tag: string) => Promise<unknown>>(),
  normalizeInstallationProgressMock: vi.fn<
    (
      progress: InstallationProgress,
      availableVersions: VersionRelease[],
      trackerState: unknown,
      now: number
    ) => { adjustedProgress: InstallationProgress; networkStatus: 'idle' | 'downloading' | 'stalled' | 'failed' }
  >(),
  resetInstallationProgressTrackingMock: vi.fn<(_state: unknown) => void>(),
  removeVersionApiMock: vi.fn<(_tag: string, _appId: string) => Promise<{ success: boolean; error?: string }>>(),
  switchVersionApiMock: vi.fn<(_tag: string, _appId: string) => Promise<{ success: boolean; error?: string }>>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
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

vi.mock('./installationProgressTracking', () => ({
  normalizeInstallationProgress: normalizeInstallationProgressMock,
  resetInstallationProgressTracking: resetInstallationProgressTrackingMock,
}));

import { useInstallationManager } from './useInstallationManager';

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

describe('useInstallationManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    normalizeInstallationProgressMock.mockImplementation((progress) => ({
      adjustedProgress: {
        ...progress,
        eta_seconds: 15,
      },
      networkStatus: 'stalled',
    }));
    installVersionApiMock.mockResolvedValue({ success: true });
    switchVersionApiMock.mockResolvedValue({ success: true });
    removeVersionApiMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('normalizes active installation progress and exposes installation access helpers', async () => {
    getInstallationProgressMock.mockResolvedValue(activeProgress);
    openActiveInstallMock.mockResolvedValue(true);
    openPathMock.mockResolvedValue(true);
    getVersionInfoMock.mockResolvedValue({ path: '/tmp/v1.2.3' });

    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    let fetched: InstallationProgress | null = null;
    await act(async () => {
      fetched = await result.current.fetchInstallationProgress();
    });

    expect(getInstallationProgressMock).toHaveBeenCalledWith('torch');
    expect(normalizeInstallationProgressMock).toHaveBeenCalled();
    expect(fetched).toEqual({
      ...activeProgress,
      eta_seconds: 15,
    });
    expect(result.current.installingTag).toBe('v1.2.3');
    expect(result.current.installationProgress).toEqual({
      ...activeProgress,
      eta_seconds: 15,
    });
    expect(result.current.installNetworkStatus).toBe('stalled');

    await act(async () => {
      await result.current.openActiveInstall();
      await result.current.openPath('/tmp/v1.2.3');
      await result.current.getVersionInfo('v1.2.3');
    });

    expect(openActiveInstallMock).toHaveBeenCalledTimes(1);
    expect(openPathMock).toHaveBeenCalledWith('/tmp/v1.2.3');
    expect(getVersionInfoMock).toHaveBeenCalledWith('v1.2.3');
  });

  it('clears install state and refreshes versions when installation progress completes', async () => {
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
      await result.current.fetchInstallationProgress();
    });

    expect(result.current.installingTag).toBe('v1.2.3');
    expect(result.current.installationProgress).not.toBeNull();

    await act(async () => {
      await result.current.fetchInstallationProgress();
    });

    expect(result.current.installingTag).toBeNull();
    expect(result.current.installationProgress).toBeNull();
    expect(result.current.installNetworkStatus).toBe('idle');
    expect(resetInstallationProgressTrackingMock).toHaveBeenCalled();
    expect(onRefreshVersions).toHaveBeenCalledTimes(1);
  });

  it('preserves failed completed progress so the UI can show the failed install', async () => {
    const failedProgress: InstallationProgress = {
      ...activeProgress,
      completed_at: '2026-04-12T00:05:00Z',
      success: false,
      error: 'Could not find Ollama binary in extracted archive',
      log_path: '/tmp/install-ollama.log',
    };
    normalizeInstallationProgressMock.mockImplementation((progress) => ({
      adjustedProgress: progress,
      networkStatus: 'failed',
    }));
    getInstallationProgressMock
      .mockResolvedValueOnce(activeProgress)
      .mockResolvedValueOnce(failedProgress);

    const { result } = renderHook(() => useInstallationManager({
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.fetchInstallationProgress();
    });

    await act(async () => {
      await result.current.fetchInstallationProgress();
    });

    expect(result.current.installingTag).toBeNull();
    expect(result.current.installationProgress).toEqual(failedProgress);
    expect(result.current.installNetworkStatus).toBe('failed');
  });

  it('resets transient install state when installVersion fails before polling begins', async () => {
    installVersionApiMock.mockResolvedValue({
      success: false,
      error: 'install denied',
    });

    const { result } = renderHook(() => useInstallationManager({
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    let caughtError: unknown;
    await act(async () => {
      try {
        await result.current.installVersion('v1.2.3');
      } catch (error) {
        caughtError = error;
      }
    });

    expect(caughtError).toBeInstanceOf(APIError);
    expect(result.current.installingTag).toBeNull();
    expect(result.current.installationProgress).toBeNull();
    expect(result.current.installNetworkStatus).toBe('idle');
  });

  it('starts install polling after a successful install request', async () => {
    vi.useFakeTimers();
    const onRefreshVersions = vi.fn().mockResolvedValue(undefined);
    getInstallationProgressMock.mockResolvedValue(activeProgress);

    const { result } = renderHook(() => useInstallationManager({
      appId: 'torch',
      availableVersions,
      onRefreshVersions,
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
    });

    expect(installVersionApiMock).toHaveBeenCalledWith('v1.2.3', 'torch');
    expect(getInstallationProgressMock).toHaveBeenCalledTimes(1);
    expect(onRefreshVersions).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(800);
    });

    expect(getInstallationProgressMock).toHaveBeenCalledTimes(2);
  });

  it('keeps polling when install progress is not initialized yet', async () => {
    vi.useFakeTimers();
    getInstallationProgressMock.mockResolvedValue(null);

    const { result } = renderHook(() => useInstallationManager({
      appId: 'llama-cpp',
      availableVersions,
      onRefreshVersions: vi.fn(),
    }));

    await act(async () => {
      await result.current.installVersion('v1.2.3');
    });

    expect(result.current.installingTag).toBe('v1.2.3');
    expect(result.current.installationProgress).toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(800);
    });

    expect(getInstallationProgressMock).toHaveBeenCalledTimes(2);
    expect(result.current.installingTag).toBe('v1.2.3');
  });
});
