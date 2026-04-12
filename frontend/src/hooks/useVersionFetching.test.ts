import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import type { CacheStatus, VersionRelease, VersionStatus } from '../types/versions';

const {
  fetchAvailableVersionsMock,
  isApiAvailableMock,
  setDefaultVersionApiMock,
  getActiveVersionMock,
  getDefaultVersionMock,
  getInstalledVersionsMock,
  getVersionStatusMock,
  useAvailableVersionStateMock,
} = vi.hoisted(() => ({
  fetchAvailableVersionsMock: vi.fn<(_forceRefresh?: boolean) => Promise<void>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  setDefaultVersionApiMock: vi.fn<
    (_tag: string | null, _appId: string) => Promise<{ success: boolean; error?: string }>
  >(),
  getActiveVersionMock: vi.fn<
    (_appId: string) => Promise<{ success: boolean; version?: string | null; error?: string }>
  >(),
  getDefaultVersionMock: vi.fn<
    (_appId: string) => Promise<{ success: boolean; version?: string | null }>
  >(),
  getInstalledVersionsMock: vi.fn<
    (_appId: string) => Promise<{ success: boolean; versions?: string[]; error?: string }>
  >(),
  getVersionStatusMock: vi.fn<
    (_appId: string) => Promise<{ success: boolean; status?: VersionStatus | null; error?: string }>
  >(),
  useAvailableVersionStateMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_active_version: getActiveVersionMock,
    get_default_version: getDefaultVersionMock,
    get_installed_versions: getInstalledVersionsMock,
    get_version_status: getVersionStatusMock,
    set_default_version: setDefaultVersionApiMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

vi.mock('./useAvailableVersionState', () => ({
  useAvailableVersionState: useAvailableVersionStateMock,
}));

import { useVersionFetching } from './useVersionFetching';

const availableVersions: VersionRelease[] = [
  {
    tagName: 'v1.2.3',
    name: 'Version 1.2.3',
    publishedAt: '2026-04-12T00:00:00Z',
    prerelease: false,
  },
];

const cacheStatus: CacheStatus = {
  has_cache: true,
  is_valid: true,
  is_fetching: false,
};

const versionStatus: VersionStatus = {
  installedCount: 2,
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

describe('useVersionFetching', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    getInstalledVersionsMock.mockResolvedValue({
      success: true,
      versions: ['v1.0.0', 'v1.2.3'],
    });
    getActiveVersionMock.mockResolvedValue({
      success: true,
      version: 'v1.2.3',
    });
    getDefaultVersionMock.mockResolvedValue({
      success: true,
      version: 'v1.2.3',
    });
    getVersionStatusMock.mockResolvedValue({
      success: true,
      status: versionStatus,
    });
    setDefaultVersionApiMock.mockResolvedValue({ success: true });
    fetchAvailableVersionsMock.mockResolvedValue(undefined);
    useAvailableVersionStateMock.mockReturnValue({
      availableVersions,
      cacheStatus,
      fetchAvailableVersions: fetchAvailableVersionsMock,
      isRateLimited: false,
      rateLimitRetryAfter: null,
    });
  });

  it('refreshes installed, active, default, status, and available version state together', async () => {
    const onInstallingTagUpdate = vi.fn();

    const { result } = renderHook(() => useVersionFetching({
      appId: 'torch',
      onInstallingTagUpdate,
    }));

    await act(async () => {
      await result.current.refreshAll(true);
    });

    expect(getInstalledVersionsMock).toHaveBeenCalledWith('torch');
    expect(getActiveVersionMock).toHaveBeenCalledWith('torch');
    expect(getDefaultVersionMock).toHaveBeenCalledWith('torch');
    expect(getVersionStatusMock).toHaveBeenCalledWith('torch');
    expect(fetchAvailableVersionsMock).toHaveBeenCalledWith(true);

    expect(result.current.installedVersions).toEqual(['v1.0.0', 'v1.2.3']);
    expect(result.current.activeVersion).toBe('v1.2.3');
    expect(result.current.defaultVersion).toBe('v1.2.3');
    expect(result.current.versionStatus).toEqual(versionStatus);
    expect(result.current.availableVersions).toEqual(availableVersions);
    expect(result.current.cacheStatus).toEqual(cacheStatus);
    expect(result.current.isLoading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('updates default version and refetches version status after a successful change', async () => {
    const updatedStatus: VersionStatus = {
      ...versionStatus,
      defaultVersion: 'v2.0.0',
    };

    getVersionStatusMock
      .mockResolvedValueOnce({
        success: true,
        status: versionStatus,
      })
      .mockResolvedValueOnce({
        success: true,
        status: updatedStatus,
      });

    const { result } = renderHook(() => useVersionFetching({
      appId: 'torch',
    }));

    await act(async () => {
      await result.current.fetchVersionStatus();
      await result.current.setDefaultVersion('v2.0.0');
    });

    expect(setDefaultVersionApiMock).toHaveBeenCalledWith('v2.0.0', 'torch');
    expect(getVersionStatusMock).toHaveBeenCalledTimes(2);
    expect(result.current.defaultVersion).toBe('v2.0.0');
    expect(result.current.versionStatus).toEqual(updatedStatus);
  });

  it('surfaces available-version fetch errors during refresh without leaving loading stuck', async () => {
    fetchAvailableVersionsMock.mockRejectedValue(new Error('network down'));

    const { result } = renderHook(() => useVersionFetching({
      appId: 'torch',
    }));

    await act(async () => {
      await result.current.refreshAll(true);
    });

    expect(result.current.error).toBe('network down');
    expect(result.current.isLoading).toBe(false);
  });

  it('throws when default version changes are attempted without API availability', async () => {
    isApiAvailableMock.mockReturnValue(false);

    const { result } = renderHook(() => useVersionFetching({
      appId: 'torch',
    }));

    await expect(result.current.setDefaultVersion('v2.0.0')).rejects.toBeInstanceOf(APIError);
    expect(setDefaultVersionApiMock).not.toHaveBeenCalled();
  });
});
