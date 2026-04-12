import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  CacheStatus,
  InstallNetworkStatus,
  InstallationProgress,
  VersionInfo,
  VersionRelease,
  VersionStatus,
} from '../types/versions';

const {
  isApiAvailableMock,
  useInstallationManagerMock,
  useVersionFetchingMock,
} = vi.hoisted(() => ({
  isApiAvailableMock: vi.fn<() => boolean>(),
  useInstallationManagerMock: vi.fn(),
  useVersionFetchingMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  isAPIAvailable: isApiAvailableMock,
}));

vi.mock('./useVersionFetching', () => ({
  useVersionFetching: useVersionFetchingMock,
}));

vi.mock('./useInstallationManager', () => ({
  useInstallationManager: useInstallationManagerMock,
}));

import { useVersions } from './useVersions';

const availableVersions: VersionRelease[] = [
  {
    tagName: 'v1.2.3',
    name: 'Version 1.2.3',
    publishedAt: '2026-04-12T00:00:00Z',
    prerelease: false,
  },
];

const versionStatus: VersionStatus = {
  installedCount: 1,
  activeVersion: 'v1.2.3',
  defaultVersion: 'v1.2.3',
  versions: {
    'v1.2.3': {
      isActive: true,
      dependencies: {
        installed: ['torch'],
        missing: [],
      },
    },
  },
};

const cacheStatus: CacheStatus = {
  has_cache: true,
  is_valid: true,
  is_fetching: false,
};

const installationProgress: InstallationProgress = {
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

describe('useVersions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();

    isApiAvailableMock.mockReturnValue(true);
    useVersionFetchingMock.mockReturnValue({
      installedVersions: ['v1.2.3'],
      activeVersion: 'v1.2.3',
      availableVersions,
      versionStatus,
      defaultVersion: 'v1.2.3',
      cacheStatus,
      isLoading: false,
      error: null,
      isRateLimited: false,
      rateLimitRetryAfter: null,
      fetchInstalledVersions: vi.fn().mockResolvedValue(undefined),
      fetchActiveVersion: vi.fn().mockResolvedValue(undefined),
      fetchVersionStatus: vi.fn().mockResolvedValue(undefined),
      refreshAll: vi.fn().mockResolvedValue(undefined),
      fetchAvailableVersions: vi.fn().mockResolvedValue(undefined),
      setDefaultVersion: vi.fn().mockResolvedValue(undefined),
    });
    useInstallationManagerMock.mockReturnValue({
      installingTag: null,
      installationProgress,
      installNetworkStatus: 'downloading' as InstallNetworkStatus,
      switchVersion: vi.fn().mockResolvedValue(true),
      installVersion: vi.fn().mockResolvedValue(true),
      removeVersion: vi.fn().mockResolvedValue(true),
      getVersionInfo: vi.fn().mockResolvedValue({ path: '/tmp/v1.2.3' } as VersionInfo),
      openPath: vi.fn().mockResolvedValue(true),
      openActiveInstall: vi.fn().mockResolvedValue(true),
      fetchInstallationProgress: vi.fn().mockResolvedValue(installationProgress),
    });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('waits for the API and triggers the top-level refresh when availability appears', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const refreshAllMock = vi.fn().mockResolvedValue(undefined);
    useVersionFetchingMock.mockReturnValue({
      ...useVersionFetchingMock.mock.results[0]?.value,
      installedVersions: ['v1.2.3'],
      activeVersion: 'v1.2.3',
      availableVersions,
      versionStatus,
      defaultVersion: 'v1.2.3',
      cacheStatus,
      isLoading: false,
      error: null,
      isRateLimited: false,
      rateLimitRetryAfter: null,
      fetchInstalledVersions: vi.fn().mockResolvedValue(undefined),
      fetchActiveVersion: vi.fn().mockResolvedValue(undefined),
      fetchVersionStatus: vi.fn().mockResolvedValue(undefined),
      refreshAll: refreshAllMock,
      fetchAvailableVersions: vi.fn().mockResolvedValue(undefined),
      setDefaultVersion: vi.fn().mockResolvedValue(undefined),
    });

    renderHook(() => useVersions({ appId: 'torch' }));

    expect(refreshAllMock).not.toHaveBeenCalled();

    isApiAvailableMock.mockReturnValue(true);

    await act(async () => {
      vi.advanceTimersByTime(100);
    });

    expect(refreshAllMock).toHaveBeenCalledTimes(1);
  });

  it('refreshes installing tag from fetching state and lets install-manager state override it', async () => {
    let capturedInstallingTagUpdate: ((tag: string | null) => void) | undefined;
    let managerInstallingTag: string | null = null;

    useVersionFetchingMock.mockImplementation((options: { onInstallingTagUpdate?: (tag: string | null) => void }) => {
      capturedInstallingTagUpdate = options.onInstallingTagUpdate;
      return {
        installedVersions: ['v1.2.3'],
        activeVersion: 'v1.2.3',
        availableVersions,
        versionStatus,
        defaultVersion: 'v1.2.3',
        cacheStatus,
        isLoading: false,
        error: null,
        isRateLimited: false,
        rateLimitRetryAfter: null,
        fetchInstalledVersions: vi.fn().mockResolvedValue(undefined),
        fetchActiveVersion: vi.fn().mockResolvedValue(undefined),
        fetchVersionStatus: vi.fn().mockResolvedValue(undefined),
        refreshAll: vi.fn().mockResolvedValue(undefined),
        fetchAvailableVersions: vi.fn().mockResolvedValue(undefined),
        setDefaultVersion: vi.fn().mockResolvedValue(undefined),
      };
    });

    useInstallationManagerMock.mockImplementation(() => ({
      installingTag: managerInstallingTag,
      installationProgress,
      installNetworkStatus: 'downloading' as InstallNetworkStatus,
      switchVersion: vi.fn().mockResolvedValue(true),
      installVersion: vi.fn().mockResolvedValue(true),
      removeVersion: vi.fn().mockResolvedValue(true),
      getVersionInfo: vi.fn().mockResolvedValue(null),
      openPath: vi.fn().mockResolvedValue(true),
      openActiveInstall: vi.fn().mockResolvedValue(true),
      fetchInstallationProgress: vi.fn().mockResolvedValue(installationProgress),
    }));

    const { result, rerender } = renderHook(() => useVersions({ appId: 'torch' }));

    act(() => {
      capturedInstallingTagUpdate?.('v1.2.3');
    });

    expect(result.current.installingTag).toBe('v1.2.3');

    managerInstallingTag = 'v2.0.0';
    rerender();

    expect(result.current.installingTag).toBe('v2.0.0');
  });

  it('wires installation refreshes through the fetching helpers', async () => {
    const fetchInstalledVersions = vi.fn().mockResolvedValue(undefined);
    const fetchActiveVersion = vi.fn().mockResolvedValue(undefined);
    const fetchVersionStatus = vi.fn().mockResolvedValue(undefined);
    let capturedOnRefreshVersions: (() => Promise<void>) | undefined;

    useVersionFetchingMock.mockReturnValue({
      installedVersions: ['v1.2.3'],
      activeVersion: 'v1.2.3',
      availableVersions,
      versionStatus,
      defaultVersion: 'v1.2.3',
      cacheStatus,
      isLoading: false,
      error: null,
      isRateLimited: false,
      rateLimitRetryAfter: null,
      fetchInstalledVersions,
      fetchActiveVersion,
      fetchVersionStatus,
      refreshAll: vi.fn().mockResolvedValue(undefined),
      fetchAvailableVersions: vi.fn().mockResolvedValue(undefined),
      setDefaultVersion: vi.fn().mockResolvedValue(undefined),
    });

    useInstallationManagerMock.mockImplementation((options: { onRefreshVersions: () => Promise<void> }) => {
      capturedOnRefreshVersions = options.onRefreshVersions;
      return {
        installingTag: null,
        installationProgress,
        installNetworkStatus: 'downloading' as InstallNetworkStatus,
        switchVersion: vi.fn().mockResolvedValue(true),
        installVersion: vi.fn().mockResolvedValue(true),
        removeVersion: vi.fn().mockResolvedValue(true),
        getVersionInfo: vi.fn().mockResolvedValue(null),
        openPath: vi.fn().mockResolvedValue(true),
        openActiveInstall: vi.fn().mockResolvedValue(true),
        fetchInstallationProgress: vi.fn().mockResolvedValue(installationProgress),
      };
    });

    renderHook(() => useVersions({ appId: 'torch' }));

    await act(async () => {
      await capturedOnRefreshVersions?.();
    });

    expect(fetchInstalledVersions).toHaveBeenCalledTimes(1);
    expect(fetchActiveVersion).toHaveBeenCalledTimes(1);
    expect(fetchVersionStatus).toHaveBeenCalledTimes(1);
  });
});
