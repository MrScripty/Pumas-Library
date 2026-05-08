import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { UseVersionsResult } from './useVersions';
import { useVersions } from './useVersions';
import { useSelectedAppVersions } from './useSelectedAppVersions';

vi.mock('./useVersions', () => ({
  useVersions: vi.fn(),
}));

const useVersionsMock = vi.mocked(useVersions);

function createVersions(overrides: Partial<UseVersionsResult> = {}): UseVersionsResult {
  return {
    activeVersion: null,
    availableVersions: [],
    cacheStatus: {
      has_cache: false,
      is_fetching: false,
      is_valid: false,
    },
    defaultVersion: null,
    error: null,
    fetchInstallationProgress: vi.fn(),
    getVersionInfo: vi.fn(),
    installNetworkStatus: 'idle',
    installationProgress: null,
    installedVersions: [],
    installingTag: null,
    installVersion: vi.fn(),
    isLoading: false,
    isRateLimited: false,
    openActiveInstall: vi.fn(),
    openPath: vi.fn(),
    rateLimitRetryAfter: null,
    refreshAll: vi.fn(),
    refreshAvailableVersions: vi.fn(),
    removeVersion: vi.fn(),
    setDefaultVersion: vi.fn(),
    switchVersion: vi.fn(),
    versionStatus: null,
    ...overrides,
  };
}

describe('useSelectedAppVersions', () => {
  beforeEach(() => {
    useVersionsMock.mockReset();
    useVersionsMock.mockImplementation((options = {}) => createVersions({
      activeVersion: `${options.appId}-active`,
      installedVersions: [`${options.appId}-installed`],
    }));
  });

  it('tracks available versions only for the selected supported app', () => {
    const { result } = renderHook(() => useSelectedAppVersions('ollama'));

    expect(useVersionsMock).toHaveBeenNthCalledWith(1, {
      appId: 'comfyui',
      trackAvailableVersions: false,
    });
    expect(useVersionsMock).toHaveBeenNthCalledWith(2, {
      appId: 'ollama',
      trackAvailableVersions: true,
    });
    expect(useVersionsMock).toHaveBeenNthCalledWith(3, {
      appId: 'llama-cpp',
      trackAvailableVersions: false,
    });
    expect(useVersionsMock).toHaveBeenNthCalledWith(4, {
      appId: 'torch',
      trackAvailableVersions: false,
    });
    expect(result.current.appVersions.appId).toBe('ollama');
    expect(result.current.appVersions.activeVersion).toBe('ollama-active');
    expect(result.current.ollamaInstalledVersions).toEqual(['ollama-installed']);
    expect(result.current.llamaCppInstalledVersions).toEqual(['llama-cpp-installed']);
  });

  it('routes selected llama.cpp version state to the panel', () => {
    const { result } = renderHook(() => useSelectedAppVersions('llama-cpp'));

    expect(result.current.appVersions.appId).toBe('llama-cpp');
    expect(result.current.appVersions.activeVersion).toBe('llama-cpp-active');
  });

  it('returns unsupported version state for an unsupported selection', () => {
    const { result } = renderHook(() => useSelectedAppVersions(null));

    expect(result.current.appVersions.isSupported).toBe(false);
    expect(result.current.comfyInstalledVersions).toEqual(['comfyui-installed']);
    expect(result.current.appVersions.installedVersions).toEqual([]);
  });
});
