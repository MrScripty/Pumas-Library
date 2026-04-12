import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import type { VersionInfo } from '../types/versions';

const {
  getVersionInfoMock,
  isApiAvailableMock,
  openActiveInstallMock,
  openPathMock,
} = vi.hoisted(() => ({
  getVersionInfoMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  openActiveInstallMock: vi.fn(),
  openPathMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_version_info: getVersionInfoMock,
    open_active_install: openActiveInstallMock,
    open_path: openPathMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

import { useInstallationAccess } from './useInstallationAccess';

const versionInfo: VersionInfo = {
  path: '/tmp/pumas/torch/v1.2.3',
  installedDate: '2026-04-12T00:00:00Z',
  releaseTag: 'v1.2.3',
  pythonVersion: '3.12.2',
};

describe('useInstallationAccess', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    openPathMock.mockResolvedValue({ success: true });
    openActiveInstallMock.mockResolvedValue({ success: true });
    getVersionInfoMock.mockResolvedValue({
      success: true,
      info: versionInfo,
    });
  });

  it('opens arbitrary paths, the active install, and version info when the API is available', async () => {
    const { result } = renderHook(() => useInstallationAccess({
      isEnabled: true,
      resolvedAppId: 'torch',
    }));

    await expect(result.current.openPath('/tmp/pumas/torch')).resolves.toBe(true);
    await expect(result.current.openActiveInstall()).resolves.toBe(true);
    await expect(result.current.getVersionInfo('v1.2.3')).resolves.toEqual(versionInfo);

    expect(openPathMock).toHaveBeenCalledWith('/tmp/pumas/torch');
    expect(openActiveInstallMock).toHaveBeenCalledWith('torch');
    expect(getVersionInfoMock).toHaveBeenCalledWith('v1.2.3', 'torch');
  });

  it('throws API errors before calling the backend when access is unavailable', async () => {
    isApiAvailableMock.mockReturnValue(false);

    const { result } = renderHook(() => useInstallationAccess({
      isEnabled: true,
      resolvedAppId: 'torch',
    }));

    await expect(result.current.openPath('/tmp/pumas/torch')).rejects.toBeInstanceOf(APIError);
    await expect(result.current.openActiveInstall()).rejects.toBeInstanceOf(APIError);
    await expect(result.current.getVersionInfo('v1.2.3')).rejects.toBeInstanceOf(APIError);

    expect(openPathMock).not.toHaveBeenCalled();
    expect(openActiveInstallMock).not.toHaveBeenCalled();
    expect(getVersionInfoMock).not.toHaveBeenCalled();
  });

  it('blocks app-scoped installation access when the hook is disabled', async () => {
    const { result } = renderHook(() => useInstallationAccess({
      isEnabled: false,
      resolvedAppId: 'torch',
    }));

    await expect(result.current.openActiveInstall()).rejects.toEqual(
      expect.objectContaining({
        endpoint: 'open_active_install',
        message: 'API not available',
      })
    );
    await expect(result.current.getVersionInfo('v1.2.3')).rejects.toEqual(
      expect.objectContaining({
        endpoint: 'get_version_info',
        message: 'API not available',
      })
    );

    expect(openActiveInstallMock).not.toHaveBeenCalled();
    expect(getVersionInfoMock).not.toHaveBeenCalled();
  });

  it('surfaces backend failures for path and version info requests', async () => {
    openPathMock.mockResolvedValueOnce({
      success: false,
      error: 'Permission denied',
    });
    getVersionInfoMock.mockResolvedValueOnce({
      success: false,
      error: 'Version missing',
    });

    const { result } = renderHook(() => useInstallationAccess({
      isEnabled: true,
      resolvedAppId: 'torch',
    }));

    await expect(result.current.openPath('/tmp/pumas/torch')).rejects.toEqual(
      expect.objectContaining({
        endpoint: 'open_path',
        message: 'Permission denied',
      })
    );
    await expect(result.current.getVersionInfo('v9.9.9')).rejects.toEqual(
      expect.objectContaining({
        endpoint: 'get_version_info',
        message: 'Version missing',
      })
    );
  });
});
