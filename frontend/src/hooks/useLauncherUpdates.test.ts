import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  CheckLauncherUpdatesResponse,
  LauncherVersionResponse,
} from '../types/api';
import { useLauncherUpdates } from './useLauncherUpdates';

const {
  checkLauncherUpdatesMock,
  getLauncherVersionMock,
  isApiAvailableMock,
  openUrlMock,
} = vi.hoisted(() => ({
  checkLauncherUpdatesMock: vi.fn<
    (_forceRefresh?: boolean) => Promise<CheckLauncherUpdatesResponse>
  >(),
  getLauncherVersionMock: vi.fn<() => Promise<LauncherVersionResponse>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  openUrlMock: vi.fn<(_url: string) => Promise<void>>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    check_launcher_updates: checkLauncherUpdatesMock,
    get_launcher_version: getLauncherVersionMock,
    open_url: openUrlMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

function launcherVersionResponse(): LauncherVersionResponse {
  return {
    success: true,
    version: '0.4.0',
    branch: 'main',
    isGitRepo: true,
  };
}

function updateResponse(
  overrides: Partial<CheckLauncherUpdatesResponse> = {}
): CheckLauncherUpdatesResponse {
  return {
    success: true,
    hasUpdate: false,
    currentCommit: 'current',
    latestCommit: 'latest',
    commitsBehind: 0,
    commits: [],
    ...overrides,
  };
}

describe('useLauncherUpdates', () => {
  beforeEach(() => {
    isApiAvailableMock.mockReturnValue(true);
    getLauncherVersionMock.mockResolvedValue(launcherVersionResponse());
    checkLauncherUpdatesMock.mockResolvedValue(updateResponse());
    openUrlMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('stores launcher update metadata from the backend', async () => {
    checkLauncherUpdatesMock.mockResolvedValue(updateResponse({
      hasUpdate: true,
      commitsBehind: 2,
      latestVersion: '0.4.1',
      releaseUrl: 'https://example.test/releases/0.4.1',
      downloadUrl: 'https://example.test/downloads/pumas.AppImage',
    }));
    const { result } = renderHook(() => useLauncherUpdates());

    await act(async () => {
      await result.current.checkLauncherVersion(true);
    });

    expect(getLauncherVersionMock).toHaveBeenCalledTimes(1);
    expect(checkLauncherUpdatesMock).toHaveBeenCalledWith(true);
    expect(result.current.launcherUpdateAvailable).toBe(true);
    expect(result.current.launcherUpdateState).toEqual({
      latestVersion: '0.4.1',
      releaseUrl: 'https://example.test/releases/0.4.1',
      downloadUrl: 'https://example.test/downloads/pumas.AppImage',
    });
  });

  it('opens the launcher update download URL before the release URL', async () => {
    checkLauncherUpdatesMock.mockResolvedValue(updateResponse({
      hasUpdate: true,
      latestVersion: '0.4.1',
      releaseUrl: 'https://example.test/releases/0.4.1',
      downloadUrl: 'https://example.test/downloads/pumas.AppImage',
    }));
    const { result } = renderHook(() => useLauncherUpdates());

    await act(async () => {
      await result.current.checkLauncherVersion();
    });
    await act(async () => {
      await result.current.openLauncherUpdate();
    });

    expect(openUrlMock).toHaveBeenCalledWith('https://example.test/downloads/pumas.AppImage');
  });

  it('clears stale launcher update metadata when no update remains', async () => {
    checkLauncherUpdatesMock.mockResolvedValueOnce(updateResponse({
      hasUpdate: true,
      latestVersion: '0.4.1',
      releaseUrl: 'https://example.test/releases/0.4.1',
    }));
    const { result } = renderHook(() => useLauncherUpdates());

    await act(async () => {
      await result.current.checkLauncherVersion();
    });

    expect(result.current.launcherUpdateAvailable).toBe(true);

    checkLauncherUpdatesMock.mockResolvedValue(updateResponse({ hasUpdate: false }));
    await act(async () => {
      await result.current.checkLauncherVersion();
    });

    expect(result.current.launcherUpdateAvailable).toBe(false);
    expect(result.current.launcherUpdateState).toBeNull();
  });

  it('does not call launcher update APIs when the bridge is unavailable', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const { result } = renderHook(() => useLauncherUpdates());

    await act(async () => {
      await result.current.checkLauncherUpdates();
      await result.current.openLauncherUpdate();
      const response = await result.current.checkLauncherVersion();
      expect(response.success).toBe(false);
    });

    expect(getLauncherVersionMock).not.toHaveBeenCalled();
    expect(checkLauncherUpdatesMock).not.toHaveBeenCalled();
    expect(openUrlMock).not.toHaveBeenCalled();
  });
});
